use std::collections::HashMap;
use std::sync::Arc;

use tokio::process::Command;

use crate::error::Result;

use super::cache::SessionCache;
use super::SESSION_PREFIX;

const TMUX_SERVER_NAME: &str = "agentdeck_rs";

/// Tmux manager - handles all tmux operations
#[derive(Debug)]
pub struct TmuxManager {
    cache: Arc<SessionCache>,
}

impl TmuxManager {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(SessionCache::new()),
        }
    }

    fn tmux_cmd(&self) -> Command {
        let mut cmd = Command::new("tmux");
        cmd.args(["-L", TMUX_SERVER_NAME]);
        cmd
    }

    async fn ensure_server_bindings(&self) {
        // Best-effort: bind keys on our dedicated tmux server.
        let cfg = crate::config::ConfigFile::load().await.ok().flatten();

        let detach_key = cfg
            .as_ref()
            .and_then(|c| c.tmux_detach_key())
            .and_then(crate::config::parse_tmux_key)
            .unwrap_or_else(|| "C-q".to_string());

        let switch_key = cfg
            .as_ref()
            .and_then(|c| c.tmux_switcher_key())
            .and_then(crate::config::parse_tmux_key)
            .unwrap_or_else(|| "C-g".to_string());

        let jump_key = cfg
            .as_ref()
            .and_then(|c| c.tmux_jump_key())
            .and_then(|s| {
                let t = s.trim();
                if t.eq_ignore_ascii_case("off") || t.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(t)
                }
            })
            .and_then(crate::config::parse_tmux_key)
            .unwrap_or_else(|| "C-n".to_string());

        // Check current bindings - skip if already correct (multi-instance safety)
        let current_detach = self
            .get_environment_global("AGENTHAND_DETACH_KEY")
            .await
            .ok()
            .flatten();
        let current_switch = self
            .get_environment_global("AGENTHAND_SWITCHER_KEY")
            .await
            .ok()
            .flatten();
        let current_jump = self
            .get_environment_global("AGENTHAND_JUMP_KEY")
            .await
            .ok()
            .flatten();

        let need_detach_bind = current_detach.as_deref() != Some(detach_key.as_str());
        let need_switch_bind = current_switch.as_deref() != Some(switch_key.as_str());
        let need_jump_bind = current_jump.as_deref() != Some(jump_key.as_str());

        // Unbind previous custom keys if they differ
        if need_detach_bind {
            if let Some(old) = &current_detach {
                let _ = self
                    .tmux_cmd()
                    .args(["unbind-key", "-n", old.as_str()])
                    .status()
                    .await;
            }
        }
        if need_switch_bind {
            if let Some(old) = &current_switch {
                let _ = self
                    .tmux_cmd()
                    .args(["unbind-key", "-n", old.as_str()])
                    .status()
                    .await;
            }
        }
        if need_jump_bind {
            if let Some(old) = &current_jump {
                let _ = self
                    .tmux_cmd()
                    .args(["unbind-key", "-n", old.as_str()])
                    .status()
                    .await;
            }
        }

        // Detach key - only bind if needed
        if need_detach_bind {
            let _ = self
                .tmux_cmd()
                .args([
                    "bind-key",
                    "-n",
                    detach_key.as_str(),
                    "set-environment",
                    "-g",
                    "AGENTHAND_LAST_SESSION",
                    "#{session_name}",
                    "\\;",
                    "set-environment",
                    "-g",
                    "AGENTHAND_LAST_DETACH_AT",
                    "#{client_activity}",
                    "\\;",
                    "detach-client",
                ])
                .status()
                .await;
            let _ = self
                .set_environment_global("AGENTHAND_DETACH_KEY", detach_key.as_str())
                .await;
        }

        // Popup switcher key - only bind if needed
        if need_switch_bind {
            let switch_bin = std::env::current_exe()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "agent-hand".to_string());
            let _ = self
                .tmux_cmd()
                .args([
                    "bind-key",
                    "-n",
                    switch_key.as_str(),
                    "display-popup",
                    "-E",
                    "-w",
                    "90%",
                    "-h",
                    "70%",
                    &switch_bin,
                    "switch",
                ])
                .status()
                .await;
            let _ = self
                .set_environment_global("AGENTHAND_SWITCHER_KEY", switch_key.as_str())
                .await;
        }

        // Jump-to-priority key (Ctrl+N by default) - only bind if needed
        if need_jump_bind {
            let _ = self
                .tmux_cmd()
                .args([
                    "bind-key",
                    "-n",
                    jump_key.as_str(),
                    "if",
                    "-F",
                    "#{!=:#{env:AGENTHAND_PRIORITY_SESSION},}",
                    "switch-client -t #{env:AGENTHAND_PRIORITY_SESSION}",
                    "display-message \"AH: no target\"",
                ])
                .status()
                .await;
            let _ = self
                .set_environment_global("AGENTHAND_JUMP_KEY", jump_key.as_str())
                .await;
        }

        // Ensure tmux popups see the active profile.
        if let Ok(profile) = std::env::var("AGENTHAND_PROFILE") {
            let _ = self
                .tmux_cmd()
                .args(["set-environment", "-g", "AGENTHAND_PROFILE", &profile])
                .status()
                .await;
        }

        // Enable mouse so scroll wheel drives tmux copy-mode/pane scrolling.
        let _ = self
            .tmux_cmd()
            .args(["set-option", "-g", "mouse", "on"])
            .status()
            .await;

        // Compact status-left badge driven by agent-hand's own status probing.
        let status_bin = std::env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "agent-hand".to_string());
        let status_bin_escaped = status_bin.replace('\'', "'\\''");
        let status_left = format!("#('{}' statusline)", status_bin_escaped);
        let _ = self
            .tmux_cmd()
            .args(["set-option", "-g", "status-interval", "5"])
            .status()
            .await;
        let _ = self
            .tmux_cmd()
            .args(["set-option", "-g", "status-left-length", "80"])
            .status()
            .await;
        let _ = self
            .tmux_cmd()
            .args(["set-option", "-g", "status-left", status_left.as_str()])
            .status()
            .await;
    }

    /// Check if tmux is available
    pub async fn is_available() -> Result<bool> {
        let output = Command::new("tmux").arg("-V").output().await;
        Ok(output.is_ok())
    }

    /// Refresh session cache from tmux
    /// Call this ONCE per tick, then use cached methods
    pub async fn refresh_cache(&self) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&[
                "list-sessions",
                "-F",
                "#{session_name}\t#{session_activity}",
            ])
            .output()
            .await?;

        if !output.status.success() {
            // tmux not running or no sessions - clear cache
            self.cache.clear();
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sessions = HashMap::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                let name = parts[0].to_string();
                let activity = parts[1].parse::<i64>().unwrap_or(0);
                sessions.insert(name, activity);
            }
        }

        self.cache.update(sessions);
        Ok(())
    }

    /// Check if session exists (from cache)
    pub fn session_exists(&self, name: &str) -> Option<bool> {
        self.cache.exists(name)
    }

    /// Get session activity (from cache)
    pub fn session_activity(&self, name: &str) -> Option<i64> {
        self.cache.activity(name)
    }

    /// Register a newly created session in cache
    pub fn register_session(&self, name: String) {
        self.cache.register(name);
    }

    /// Get tmux session name for a session ID
    pub fn session_name(id: &str) -> String {
        format!("{}{}", SESSION_PREFIX, id)
    }

    /// Create a new tmux session
    pub async fn create_session(
        &self,
        name: &str,
        working_dir: &str,
        command: Option<&str>,
    ) -> Result<()> {
        let mut cmd = self.tmux_cmd();
        cmd.args(&[
            "new-session",
            "-d", // Detached
            "-s",
            name,
            "-c",
            working_dir,
        ]);

        // Build the shell command
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        
        if let Some(command) = command {
            cmd.arg(command);
        } else {
            // Login shell (no command)
            cmd.args([&shell, "-l"]);
        }

        let output = cmd.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If the session already exists (cache can be stale), treat it as success.
            if stderr.contains("duplicate session") {
                self.ensure_server_bindings().await;
                self.register_session(name.to_string());
                return Ok(());
            }
            return Err(crate::Error::tmux(format!(
                "Failed to create session: {}",
                stderr
            )));
        }

        // Set bindings after tmux server is running
        self.ensure_server_bindings().await;

        // Register in cache immediately
        self.register_session(name.to_string());

        Ok(())
    }

    /// Kill a tmux session
    pub async fn kill_session(&self, name: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&["kill-session", "-t", name])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to kill session: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Capture pane content (for status detection)
    pub async fn capture_pane(&self, name: &str, lines: usize) -> Result<String> {
        let output = self
            .tmux_cmd()
            .args(&[
                "capture-pane",
                "-t",
                name,
                "-p", // Print to stdout
                "-S",
                &format!("-{}", lines), // Start line
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(String::new());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Send keys to a session
    pub async fn send_keys(&self, name: &str, keys: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&["send-keys", "-t", name, keys, "Enter"])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to send keys: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Attach to a session (blocking)
    pub async fn attach_session(&self, name: &str) -> Result<()> {
        self.ensure_server_bindings().await;

        let status = self
            .tmux_cmd()
            .args(&["attach-session", "-t", name])
            .status()
            .await?;

        if !status.success() {
            return Err(crate::Error::tmux("Failed to attach to session"));
        }

        Ok(())
    }

    /// Set a global tmux environment variable on our dedicated server.
    pub async fn set_environment_global(&self, key: &str, value: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(["set-environment", "-g", key, value])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to set tmux env {key}: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Get a global tmux environment variable from our dedicated server.
    pub async fn get_environment_global(&self, key: &str) -> Result<Option<String>> {
        let output = self
            .tmux_cmd()
            .args(["show-environment", "-g", key])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(None);
        }

        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let prefix = format!("{key}=");
        if line.starts_with(&prefix) {
            Ok(Some(line[prefix.len()..].to_string()))
        } else {
            Ok(None)
        }
    }

    /// Switch current tmux client to a target session
    pub async fn switch_client(&self, name: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(["switch-client", "-t", name])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to switch client: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// List all agent-deck sessions
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        let output = self
            .tmux_cmd()
            .args(&["list-sessions", "-F", "#{session_name}"])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let sessions: Vec<String> = stdout
            .lines()
            .filter(|line| line.starts_with(SESSION_PREFIX))
            .map(|s| s.to_string())
            .collect();

        Ok(sessions)
    }
}

impl Default for TmuxManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_name() {
        assert_eq!(TmuxManager::session_name("abc123"), "agentdeck_rs_abc123");
    }

    #[tokio::test]
    async fn test_tmux_available() {
        // This will fail in CI without tmux, but useful for local testing
        let available = TmuxManager::is_available().await.unwrap_or(false);
        println!("Tmux available: {}", available);
    }
}
