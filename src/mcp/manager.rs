// Placeholder for MCP manager
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::error::{Error, Result};
use crate::mcp::MCPConfig;
use crate::session::Storage;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MCPFile {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: HashMap<String, MCPConfig>,
}

pub struct MCPManager;

impl MCPManager {
    pub fn new() -> Self {
        Self
    }

    pub fn global_pool_path() -> Result<PathBuf> {
        Ok(Storage::get_agent_deck_dir()?.join("mcp.json"))
    }

    pub async fn load_global_pool() -> Result<HashMap<String, MCPConfig>> {
        let path = Self::global_pool_path()?;
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(&path).await?;
        let file: MCPFile =
            serde_json::from_str(&content).map_err(|e| Error::mcp(e.to_string()))?;
        Ok(file.mcp_servers)
    }

    pub async fn load_project_mcp(project_path: &Path) -> Result<HashMap<String, MCPConfig>> {
        let path = project_path.join(".mcp.json");
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(&path).await?;
        let file: MCPFile =
            serde_json::from_str(&content).map_err(|e| Error::mcp(e.to_string()))?;
        Ok(file.mcp_servers)
    }

    pub async fn write_project_mcp(
        project_path: &Path,
        mcp_servers: &HashMap<String, MCPConfig>,
    ) -> Result<()> {
        let path = project_path.join(".mcp.json");
        let tmp = project_path.join(".mcp.json.tmp");

        let file = MCPFile {
            mcp_servers: mcp_servers.clone(),
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| Error::mcp(e.to_string()))?;

        fs::write(&tmp, json).await?;
        fs::rename(&tmp, &path).await?;

        Ok(())
    }
}

impl Default for MCPManager {
    fn default() -> Self {
        Self::new()
    }
}
