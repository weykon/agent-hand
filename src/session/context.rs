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

/// Manages context snapshot collection and storage (JSONL files per relationship).
///
/// Context snapshots are stored separately from sessions.json to avoid bloat:
///   ~/.agent-hand/profiles/{profile}/context/{relationship_id}.jsonl
pub struct ContextCollector {
    profile: String,
}

impl ContextCollector {
    pub fn new(profile: &str) -> Self {
        Self {
            profile: profile.to_string(),
        }
    }

    /// Get the context directory for this profile
    fn context_dir(&self) -> crate::Result<std::path::PathBuf> {
        let base = crate::session::Storage::get_agent_hand_dir()?;
        Ok(base
            .join("profiles")
            .join(&self.profile)
            .join("context"))
    }

    /// Save a snapshot to the appropriate JSONL file
    pub async fn save_snapshot(&self, snapshot: &ContextSnapshot) -> crate::Result<()> {
        use tokio::fs;
        use tokio::io::AsyncWriteExt;

        let dir = self.context_dir()?;
        fs::create_dir_all(&dir).await?;

        let filename = if let Some(ref rel_id) = snapshot.relationship_id {
            format!("{}.jsonl", rel_id)
        } else {
            format!("session_{}.jsonl", snapshot.session_id)
        };

        let path = dir.join(filename);
        let mut line = serde_json::to_string(snapshot)?;
        line.push('\n');

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        file.write_all(line.as_bytes()).await?;

        Ok(())
    }

    /// Load all snapshots for a relationship
    pub async fn load_relationship_snapshots(
        &self,
        relationship_id: &str,
    ) -> crate::Result<Vec<ContextSnapshot>> {
        use tokio::fs;

        let path = self.context_dir()?.join(format!("{}.jsonl", relationship_id));
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path).await?;
        let snapshots: Vec<ContextSnapshot> = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        Ok(snapshots)
    }

    /// Build a combined relationship context document from snapshots
    pub async fn build_relationship_context(
        &self,
        relationship_id: &str,
        relationship_label: Option<&str>,
        session_a_title: &str,
        session_b_title: &str,
    ) -> crate::Result<String> {
        let snapshots = self.load_relationship_snapshots(relationship_id).await?;

        let mut doc = String::new();
        doc.push_str("=== Relationship Context ===\n");
        if let Some(label) = relationship_label {
            doc.push_str(&format!("Label: {}\n", label));
        }
        doc.push('\n');

        // Group snapshots by session
        let a_snaps: Vec<_> = snapshots
            .iter()
            .filter(|s| s.tags.contains(&"session_a".to_string()))
            .collect();
        let b_snaps: Vec<_> = snapshots
            .iter()
            .filter(|s| s.tags.contains(&"session_b".to_string()))
            .collect();
        let notes: Vec<_> = snapshots
            .iter()
            .filter(|s| s.snapshot_type == SnapshotType::Annotation)
            .collect();

        doc.push_str(&format!("--- Session A: \"{}\" ---\n", session_a_title));
        for snap in &a_snaps {
            doc.push_str(&format!("  [{}] {}\n", snap.captured_at.format("%H:%M"), snap.content));
        }

        doc.push_str(&format!("\n--- Session B: \"{}\" ---\n", session_b_title));
        for snap in &b_snaps {
            doc.push_str(&format!("  [{}] {}\n", snap.captured_at.format("%H:%M"), snap.content));
        }

        if !notes.is_empty() {
            doc.push_str("\n--- Notes ---\n");
            for note in &notes {
                doc.push_str(&format!("  \"{}\"\n", note.content));
            }
        }

        Ok(doc)
    }

    /// Delete all snapshots for a relationship
    pub async fn delete_relationship_snapshots(
        &self,
        relationship_id: &str,
    ) -> crate::Result<()> {
        let path = self.context_dir()?.join(format!("{}.jsonl", relationship_id));
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }
}
