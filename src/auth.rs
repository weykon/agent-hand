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

    /// Whether this token represents a pro (purchased) user.
    pub fn is_pro(&self) -> bool {
        self.has_feature("upgrade")
    }

    /// Gate a premium feature: returns Ok(()) if authorized, Err otherwise.
    /// Usage: `AuthToken::require_feature("sharing")?;`
    pub fn require_feature(feature: &str) -> Result<()> {
        match Self::load() {
            None => Err(crate::Error::InvalidInput(
                format!("Requires license for '{}'. Run `agent-hand login`.", feature),
            )),
            Some(t) if !t.has_feature(feature) => Err(crate::Error::InvalidInput(
                format!("Feature '{}' requires a plan upgrade. Visit https://agent-hand.dev", feature),
            )),
            Some(_) => Ok(()),
        }
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

    /// Query the server for current features and update the local token.
    /// Returns Ok(true) if features changed, Ok(false) if unchanged.
    pub async fn refresh(&mut self) -> Result<bool> {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{AUTH_SERVER}/auth/status"))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| crate::Error::InvalidInput(format!("Network error: {e}")))?;

        if !resp.status().is_success() {
            return Err(crate::Error::InvalidInput(
                "Failed to query account status".to_string(),
            ));
        }

        let status: StatusResponse = resp
            .json()
            .await
            .map_err(|e| crate::Error::InvalidInput(format!("Invalid response: {e}")))?;

        if !status.valid {
            return Err(crate::Error::InvalidInput(
                "Token is no longer valid".to_string(),
            ));
        }

        let new_features = status.features.unwrap_or_default();
        let new_purchased_at = status.purchased_at.unwrap_or_default();
        let changed = self.features != new_features || self.purchased_at != new_purchased_at;

        if changed {
            self.features = new_features;
            self.purchased_at = new_purchased_at;
            self.save()?;
        }

        Ok(changed)
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

#[derive(Debug, Deserialize)]
pub struct StatusResponse {
    pub valid: bool,
    pub email: Option<String>,
    pub features: Option<Vec<String>>,
    pub purchased_at: Option<String>,
}
