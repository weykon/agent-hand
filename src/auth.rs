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
    /// Device fingerprint (added in v0.3.8). Absent in older auth.json files.
    #[serde(default)]
    pub device_id: Option<String>,
}

impl AuthToken {
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }

    /// Whether this token represents a pro (purchased) user.
    pub fn is_pro(&self) -> bool {
        self.has_feature("upgrade")
    }

    /// Whether this token represents a max (subscription) user.
    /// Max includes all Pro features plus AI-powered capabilities.
    pub fn is_max(&self) -> bool {
        self.has_feature("max")
    }

    /// Gate a premium feature: returns Ok(()) if authorized, Err otherwise.
    /// Usage: `AuthToken::require_feature("sharing")?;`
    pub fn require_feature(feature: &str) -> Result<()> {
        match Self::load() {
            None => Err(crate::Error::InvalidInput(
                format!("Requires license for '{}'. Run `agent-hand login`.", feature),
            )),
            Some(t) if !t.has_feature(feature) => Err(crate::Error::InvalidInput(
                format!("Feature '{}' requires a plan upgrade. Visit https://weykon.github.io/agent-hand", feature),
            )),
            Some(_) => Ok(()),
        }
    }

    /// Gate a Max-tier feature: returns Ok(()) if the user has Max subscription.
    /// Usage: `AuthToken::require_max("sharing")?;`
    pub fn require_max(label: &str) -> Result<()> {
        match Self::load() {
            None => Err(crate::Error::InvalidInput(
                format!("'{}' requires Max subscription. Run `agent-hand login`.", label),
            )),
            Some(t) if !t.is_max() => Err(crate::Error::InvalidInput(
                format!("'{}' requires Max subscription. Visit https://weykon.github.io/agent-hand", label),
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

    /// Send a heartbeat to register/refresh this device. Returns slot status.
    pub async fn heartbeat(&self) -> Result<HeartbeatResponse> {
        let device = crate::device::DeviceInfo::generate();
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{AUTH_SERVER}/api/heartbeat"))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&serde_json::json!({
                "device_id": device.device_id,
                "hostname": device.hostname,
                "os_arch": device.os_arch,
            }))
            .send()
            .await
            .map_err(|e| crate::Error::InvalidInput(format!("Heartbeat network error: {e}")))?;

        let status = resp.status();
        if status.as_u16() == 409 {
            let payload: HeartbeatErrorPayload = resp.json().await
                .map_err(|e| crate::Error::InvalidInput(format!("Heartbeat parse error: {e}")))?;
            return Ok(HeartbeatResponse::LimitExceeded {
                device_limit: payload.device_limit.unwrap_or(0),
                active_devices: payload.active_devices.unwrap_or(0),
                devices: payload.devices.unwrap_or_default(),
            });
        }

        if !status.is_success() {
            return Err(crate::Error::InvalidInput(
                format!("Heartbeat failed with status {}", status.as_u16()),
            ));
        }

        let payload: HeartbeatOkPayload = resp.json().await
            .map_err(|e| crate::Error::InvalidInput(format!("Heartbeat parse error: {e}")))?;

        Ok(HeartbeatResponse::Ok {
            device_limit: payload.device_limit.unwrap_or(0),
            active_devices: payload.active_devices.unwrap_or(0),
        })
    }

    /// Fetch the user's registered devices from the server.
    pub async fn list_devices(&self) -> Result<DevicesListResponse> {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{AUTH_SERVER}/api/devices"))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| crate::Error::InvalidInput(format!("Network error: {e}")))?;

        if !resp.status().is_success() {
            return Err(crate::Error::InvalidInput(
                "Failed to list devices".to_string(),
            ));
        }

        resp.json().await
            .map_err(|e| crate::Error::InvalidInput(format!("Parse error: {e}")))
    }

    /// Remove (unbind) a device by its full device_id.
    pub async fn remove_device(&self, device_id: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let resp = client
            .delete(format!("{AUTH_SERVER}/api/devices/{device_id}"))
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await
            .map_err(|e| crate::Error::InvalidInput(format!("Network error: {e}")))?;

        if !resp.status().is_success() {
            return Err(crate::Error::InvalidInput(
                "Failed to remove device".to_string(),
            ));
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

#[derive(Debug, Deserialize)]
pub struct StatusResponse {
    pub valid: bool,
    pub email: Option<String>,
    pub features: Option<Vec<String>>,
    pub purchased_at: Option<String>,
}

// ── Device slot types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSlotInfo {
    pub device_id: String,
    pub hostname: String,
    pub os_arch: String,
    pub last_seen: String,
}

#[derive(Debug)]
pub enum HeartbeatResponse {
    Ok {
        device_limit: usize,
        active_devices: usize,
    },
    LimitExceeded {
        device_limit: usize,
        active_devices: usize,
        devices: Vec<DeviceSlotInfo>,
    },
}

/// Raw JSON shape returned by the heartbeat endpoint (success case).
#[derive(Debug, Deserialize)]
struct HeartbeatOkPayload {
    device_limit: Option<usize>,
    active_devices: Option<usize>,
}

/// Raw JSON shape returned by the heartbeat endpoint (409 case).
#[derive(Debug, Deserialize)]
struct HeartbeatErrorPayload {
    device_limit: Option<usize>,
    active_devices: Option<usize>,
    devices: Option<Vec<DeviceSlotInfo>>,
}

/// Response from GET /api/devices.
#[derive(Debug, Deserialize)]
pub struct DevicesListResponse {
    pub devices: Vec<DeviceSlotInfo>,
    pub device_limit: usize,
    pub active_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backward_compat_old_auth_json_without_device_id() {
        let old_json = r#"{
            "access_token": "tok_abc",
            "email": "user@example.com",
            "features": ["upgrade"],
            "purchased_at": "2025-01-01"
        }"#;
        let token: AuthToken = serde_json::from_str(old_json).expect("should deserialize old format");
        assert_eq!(token.email, "user@example.com");
        assert!(token.device_id.is_none(), "missing field should default to None");
        assert!(token.is_pro());
    }

    #[test]
    fn new_auth_json_with_device_id() {
        let new_json = r#"{
            "access_token": "tok_abc",
            "email": "user@example.com",
            "features": ["upgrade", "max"],
            "purchased_at": "2025-01-01",
            "device_id": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        }"#;
        let token: AuthToken = serde_json::from_str(new_json).expect("should deserialize new format");
        assert_eq!(token.device_id.as_deref(), Some("abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"));
        assert!(token.is_max());
    }

    #[test]
    fn serialization_roundtrip() {
        let token = AuthToken {
            access_token: "tok".to_string(),
            email: "a@b.com".to_string(),
            features: vec!["upgrade".to_string()],
            purchased_at: "2025-01-01".to_string(),
            device_id: Some("deadbeef".repeat(8)),
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: AuthToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back.device_id, token.device_id);
        assert_eq!(back.email, token.email);
    }

    #[test]
    fn heartbeat_error_payload_deserializes() {
        let json = r#"{
            "error": "device_limit_exceeded",
            "device_limit": 1,
            "active_devices": 1,
            "devices": [{"device_id":"abc","hostname":"mac","os_arch":"macos-aarch64","last_seen":"2025-01-01"}]
        }"#;
        let payload: HeartbeatErrorPayload = serde_json::from_str(json).expect("should deserialize 409 payload");
        assert_eq!(payload.device_limit, Some(1));
        assert_eq!(payload.devices.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn heartbeat_ok_payload_deserializes() {
        let json = r#"{"ok":true,"device_limit":3,"active_devices":2}"#;
        let payload: HeartbeatOkPayload = serde_json::from_str(json).expect("should deserialize ok payload");
        assert_eq!(payload.device_limit, Some(3));
        assert_eq!(payload.active_devices, Some(2));
    }
}
