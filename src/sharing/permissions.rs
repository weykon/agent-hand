use serde::{Deserialize, Serialize};

/// Permission level for a shared session link
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SharePermission {
    /// View-only access (ro- prefixed SSH URL)
    ReadOnly,
    /// Full interactive access (rw SSH URL, Premium tier)
    ReadWrite,
}

impl Default for SharePermission {
    fn default() -> Self {
        Self::ReadOnly
    }
}

impl std::fmt::Display for SharePermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "ro"),
            Self::ReadWrite => write!(f, "rw"),
        }
    }
}
