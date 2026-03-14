use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of context snapshot captured from a session
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotType {
    /// Raw terminal output (last N lines of pane)
    PaneCapture,
    /// Status transition log
    StatusHistory,
    /// User-written annotation/note
    Annotation,
    /// Diff between two captures
    Delta,
}

/// A single context snapshot from a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub id: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship_id: Option<String>,
    pub captured_at: DateTime<Utc>,
    pub snapshot_type: SnapshotType,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl ContextSnapshot {
    /// Create a new pane capture snapshot
    pub fn pane_capture(session_id: &str, content: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string()[..12].to_string(),
            session_id: session_id.to_string(),
            relationship_id: None,
            captured_at: Utc::now(),
            snapshot_type: SnapshotType::PaneCapture,
            content,
            summary: None,
            tags: Vec::new(),
        }
    }

    /// Create a user annotation snapshot
    pub fn annotation(session_id: &str, note: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string()[..12].to_string(),
            session_id: session_id.to_string(),
            relationship_id: None,
            captured_at: Utc::now(),
            snapshot_type: SnapshotType::Annotation,
            content: note,
            summary: None,
            tags: Vec::new(),
        }
    }

    /// Attach this snapshot to a relationship
    pub fn with_relationship(mut self, relationship_id: &str) -> Self {
        self.relationship_id = Some(relationship_id.to_string());
        self
    }

    /// Add tags to this snapshot
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

// ContextCollector business logic is in the pro module (crate::pro::context)
// when the `pro` feature is enabled.
