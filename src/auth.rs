use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const AUTH_SERVER: &str = "https://auth.asymptai.com";

fn auth_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".agent-hand").join("auth.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub access_token: String,
    pub email: String,
    pub features: Vec<String>,
    pub purchased_at: String,
}

impl AuthToken {
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }

    pub fn load() -> Option<Self> {
        let path = auth_file_path()?;
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn save(&self) -> Result<()> {
        let path = auth_file_path().ok_or_else(|| {
            crate::Error::InvalidInput("Cannot determine home directory".to_string())
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn delete() -> Result<()> {
        if let Some(path) = auth_file_path() {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub code: String,
    pub url: String,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct DeviceTokenResponse {
    pub status: String,
    pub access_token: Option<String>,
    pub email: Option<String>,
    pub features: Option<Vec<String>>,
    pub purchased_at: Option<String>,
}
