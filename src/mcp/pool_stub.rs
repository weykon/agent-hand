use std::path::{Path, PathBuf};

use tokio::fs;

use crate::error::{Error, Result};
use crate::mcp::MCPConfig;
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

    pub async fn is_running(_name: &str) -> bool {
        false
    }

    pub async fn start(_name: &str) -> Result<()> {
        Err(Error::mcp(
            "MCP pool is not supported on Windows (requires Unix domain sockets)",
        ))
    }

    pub async fn stop(_name: &str) -> Result<()> {
        Err(Error::mcp(
            "MCP pool is not supported on Windows (requires Unix domain sockets)",
        ))
    }

    pub async fn load_pool_config(name: &str) -> Result<MCPConfig> {
        let all = crate::mcp::MCPManager::load_global_pool().await?;
        all.get(name)
            .cloned()
            .ok_or_else(|| Error::mcp(format!("unknown MCP server: {name}")))
    }

    pub async fn list_available() -> Result<Vec<String>> {
        let all = crate::mcp::MCPManager::load_global_pool().await?;
        let mut names: Vec<String> = all.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    pub async fn serve(_name: &str) -> Result<()> {
        Err(Error::mcp(
            "MCP pool is not supported on Windows (requires Unix domain sockets)",
        ))
    }
}

pub fn pooled_mcp_config(_name: &str, _sock: &Path, base: &MCPConfig) -> MCPConfig {
    // On Windows we don't have Unix domain sockets; just return the base config.
    base.clone()
}

// Keep fs referenced so this file matches the unix module surface (and avoids unused warnings when
// future code adds shared helpers).
#[allow(dead_code)]
async fn _touch_for_rustfmt(_p: &Path) -> Result<()> {
    let _ = fs::read(_p).await;
    Ok(())
}
