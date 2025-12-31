use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::fs;
use tokio::net::UnixListener;
use tokio::process::Command;

use crate::error::{Error, Result};
use crate::mcp::{MCPConfig, MCPManager};
use crate::session::Storage;

pub struct MCPPool;

impl MCPPool {
    pub fn pool_dir() -> Result<PathBuf> {
        Ok(Storage::get_agent_deck_dir()?.join("pool"))
    }

    pub fn socket_path(name: &str) -> Result<PathBuf> {
        Ok(Self::pool_dir()?.join(format!("{name}.sock")))
    }

    pub fn pid_path(name: &str) -> Result<PathBuf> {
        Ok(Self::pool_dir()?.join(format!("{name}.pid")))
    }

    pub fn log_path(name: &str) -> Result<PathBuf> {
        Ok(Self::pool_dir()?.join(format!("{name}.log")))
    }

    pub async fn is_running(name: &str) -> bool {
        let pid_path = match Self::pid_path(name) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let pid_str = match fs::read_to_string(&pid_path).await {
            Ok(s) => s,
            Err(_) => return false,
        };
        let pid = pid_str.trim();
        if pid.is_empty() {
            let _ = fs::remove_file(&pid_path).await;
            return false;
        }

        let alive = Command::new("kill")
            .arg("-0")
            .arg(pid)
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);

        if !alive {
            // cleanup stale artifacts
            let _ = fs::remove_file(&pid_path).await;
            if let Ok(sock) = Self::socket_path(name) {
                let _ = fs::remove_file(sock).await;
            }
        }

        alive
    }

    pub async fn start(name: &str) -> Result<()> {
        fs::create_dir_all(Self::pool_dir()?).await?;

        // If already running, do nothing.
        if Self::is_running(name).await {
            return Ok(());
        }

        // Clean stale files.
        let _ = fs::remove_file(Self::pid_path(name)?).await;
        let _ = fs::remove_file(Self::socket_path(name)?).await;

        let log = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(Self::log_path(name)?)
            .await?;
        let log2 = log.try_clone().await?;

        // Start detached-ish: redirect stdio so it doesn't hold the terminal.
        let mut cmd = Command::new(std::env::current_exe()?);
        cmd.arg("mcp")
            .arg("pool")
            .arg("serve")
            .arg(name)
            .stdin(std::process::Stdio::null())
            .stdout(log.into_std().await)
            .stderr(log2.into_std().await);

        let child = cmd.spawn().map_err(|e| Error::mcp(e.to_string()))?;
        let pid = child
            .id()
            .ok_or_else(|| Error::mcp("failed to get child pid"))?;

        fs::write(Self::pid_path(name)?, pid.to_string()).await?;
        Ok(())
    }

    pub async fn stop(name: &str) -> Result<()> {
        let pid_path = Self::pid_path(name)?;
        let pid_str = fs::read_to_string(&pid_path).await.unwrap_or_default();
        let pid = pid_str.trim().to_string();

        if !pid.is_empty() {
            let _ = Command::new("kill").arg("-TERM").arg(&pid).status().await;

            // Wait a bit for graceful shutdown.
            let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            loop {
                let alive = Command::new("kill")
                    .arg("-0")
                    .arg(&pid)
                    .status()
                    .await
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !alive {
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    let _ = Command::new("kill").arg("-KILL").arg(&pid).status().await;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        let _ = fs::remove_file(&pid_path).await;
        let _ = fs::remove_file(Self::socket_path(name)?).await;
        Ok(())
    }

    pub async fn load_pool_config(name: &str) -> Result<MCPConfig> {
        let all = MCPManager::load_global_pool().await?;
        all.get(name)
            .cloned()
            .ok_or_else(|| Error::mcp(format!("unknown MCP server: {name}")))
    }

    pub async fn list_available() -> Result<Vec<String>> {
        let all: HashMap<String, MCPConfig> = MCPManager::load_global_pool().await?;
        let mut names: Vec<String> = all.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    pub async fn serve(name: &str) -> Result<()> {
        fs::create_dir_all(Self::pool_dir()?).await?;

        let sock_path = Self::socket_path(name)?;
        if sock_path.exists() {
            let _ = fs::remove_file(&sock_path).await;
        }

        let listener = UnixListener::bind(&sock_path)?;

        // Persist pid so TUI/CLI can detect it.
        let pid = std::process::id();
        fs::write(Self::pid_path(name)?, pid.to_string()).await?;

        let cfg = Self::load_pool_config(name).await?;

        let mut child = spawn_child(&cfg)?;

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    let _ = child.kill().await;
                    break;
                }
                res = listener.accept() => {
                    let (stream, _) = res?;

                    // respawn if child exited
                    if let Ok(Some(_)) = child.try_wait() {
                        child = spawn_child(&cfg)?;
                    }

                    let stdin = child
                        .stdin
                        .as_mut()
                        .ok_or_else(|| Error::mcp("child stdin not available"))?;
                    let stdout = child
                        .stdout
                        .as_mut()
                        .ok_or_else(|| Error::mcp("child stdout not available"))?;

                    let (mut sock_r, mut sock_w) = tokio::io::split(stream);

                    let a = tokio::io::copy(&mut sock_r, stdin);
                    let b = tokio::io::copy(stdout, &mut sock_w);
                    tokio::pin!(a);
                    tokio::pin!(b);

                    tokio::select! {
                        _ = &mut a => {},
                        _ = &mut b => {},
                    }
                }
            }
        }

        let _ = fs::remove_file(Self::pid_path(name)?).await;
        let _ = fs::remove_file(Self::socket_path(name)?).await;
        Ok(())
    }
}

fn spawn_child(cfg: &MCPConfig) -> Result<tokio::process::Child> {
    let mut cmd = Command::new(&cfg.command);
    cmd.args(&cfg.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    for (k, v) in &cfg.env {
        cmd.env(k, v);
    }

    cmd.spawn().map_err(|e| Error::mcp(e.to_string()))
}

pub fn pooled_mcp_config(name: &str, sock: &Path, base: &MCPConfig) -> MCPConfig {
    let mut c = base.clone();
    c.command = "nc".to_string();
    c.args = vec!["-U".to_string(), sock.to_string_lossy().to_string()];
    c.env.clear();
    c.transport = Some("stdio".to_string());
    c.description = format!("{} (pooled)", name);
    c
}
