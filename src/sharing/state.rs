use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::permissions::SharePermission;

/// A single share link (SSH or web URL)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareLink {
    pub permission: SharePermission,
    pub ssh_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

/// State of sharing for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharingState {
    pub active: bool,
    pub tmate_socket: String,
    pub links: Vec<ShareLink>,
    pub default_permission: SharePermission,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_expire_minutes: Option<u64>,
}

impl SharingState {
    /// Check if any links have expired
    pub fn has_expired_links(&self) -> bool {
        let now = Utc::now();
        self.links.iter().any(|link| {
            link.expires_at
                .is_some_and(|exp| exp < now)
        })
    }

    /// Check if the entire sharing session should expire
    pub fn should_auto_expire(&self) -> bool {
        if let Some(minutes) = self.auto_expire_minutes {
            let elapsed = Utc::now()
                .signed_duration_since(self.started_at)
                .num_minutes();
            elapsed >= minutes as i64
        } else {
            false
        }
    }
}
