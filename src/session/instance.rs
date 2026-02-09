use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::tmux::{SessionStatus, TmuxManager, TmuxSession, Tool};

/// Session status (persisted)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Waiting,
    Idle,
    Error,
    Starting,
}

impl From<SessionStatus> for Status {
    fn from(status: SessionStatus) -> Self {
        match status {
            SessionStatus::Running => Status::Running,
            SessionStatus::Waiting => Status::Waiting,
            SessionStatus::Idle => Status::Idle,
            SessionStatus::Error => Status::Error,
            SessionStatus::Starting => Status::Starting,
        }
    }
}

/// Optional UI label color (persisted)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LabelColor {
    Gray,
    Magenta,
    Cyan,
    Green,
    Yellow,
    Red,
    Blue,
}

impl Default for LabelColor {
    fn default() -> Self {
        Self::Gray
    }
}

/// Session instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub title: String,
    pub project_path: PathBuf,
    pub group_path: String,
    pub parent_session_id: Option<String>,
    pub command: String,
    #[serde(default)]
    pub tool: Tool,

    // Optional UI label
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub label_color: LabelColor,

    pub status: Status,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: Option<DateTime<Utc>>,

    /// Last time this session was detected as Running (for Ready indicator)
    #[serde(default)]
    pub last_running_at: Option<DateTime<Utc>>,

    /// Last time this session entered Waiting (for priority target selection)
    #[serde(default)]
    pub last_waiting_at: Option<DateTime<Utc>>,

    // Claude integration
    pub claude_session_id: Option<String>,
    pub claude_detected_at: Option<DateTime<Utc>>,

    // Gemini integration
    pub gemini_session_id: Option<String>,
    pub gemini_detected_at: Option<DateTime<Utc>>,

    // Non-serialized fields
    #[serde(skip)]
    tmux_session: Option<Arc<TmuxSession>>,

    /// Number of /dev/ptmx FDs held by this session's process tree (runtime-only).
    #[serde(skip)]
    pub ptmx_count: u32,
}

impl Instance {
    /// Create a new session instance
    pub fn new(title: String, project_path: PathBuf) -> Self {
        let id = generate_id();
        let group_path = extract_group_path(&project_path);

        Self {
            id,
            title,
            project_path,
            group_path,
            parent_session_id: None,
            command: String::new(),
            tool: Tool::Shell,
            label: String::new(),
            label_color: LabelColor::Gray,
            status: Status::Idle,
            created_at: Utc::now(),
            last_accessed_at: None,
            last_running_at: None,
            last_waiting_at: None,
            claude_session_id: None,
            claude_detected_at: None,
            gemini_session_id: None,
            gemini_detected_at: None,
            tmux_session: None,
            ptmx_count: 0,
        }
    }

    /// Create with explicit group path
    pub fn with_group(title: String, project_path: PathBuf, group_path: String) -> Self {
        let mut instance = Self::new(title, project_path);
        instance.group_path = group_path;
        instance
    }

    /// Create with tool
    pub fn with_tool(title: String, project_path: PathBuf, tool: Tool) -> Self {
        let mut instance = Self::new(title, project_path);
        instance.tool = tool;
        instance
    }

    /// Get tmux session name
    pub fn tmux_name(&self) -> String {
        TmuxManager::session_name(&self.id)
    }

    /// Mark as accessed
    pub fn mark_accessed(&mut self) {
        self.last_accessed_at = Some(Utc::now());
    }

    /// Check if this is a sub-session
    pub fn is_sub_session(&self) -> bool {
        self.parent_session_id.is_some()
    }

    /// Set parent session
    pub fn set_parent(&mut self, parent_id: String) {
        self.parent_session_id = Some(parent_id);
    }

    /// Clear parent
    pub fn clear_parent(&mut self) {
        self.parent_session_id = None;
    }

    /// Initialize tmux session wrapper
    pub fn init_tmux(&mut self, manager: Arc<TmuxManager>) {
        let tmux_session = Arc::new(TmuxSession::new(
            self.tmux_name(),
            self.project_path.clone(),
            self.tool,
            manager,
        ));
        self.tmux_session = Some(tmux_session);
    }

    /// Get tmux session (if initialized)
    pub fn tmux(&self) -> Option<&Arc<TmuxSession>> {
        self.tmux_session.as_ref()
    }

    /// Update status from tmux
    pub async fn update_status(&mut self) -> crate::Result<()> {
        if let Some(tmux) = &self.tmux_session {
            let status = tmux.update_status().await?;
            self.status = status.into();
        }
        Ok(())
    }

    /// Start the session
    pub async fn start(&mut self) -> crate::Result<()> {
        if let Some(tmux) = &self.tmux_session {
            let cmd = if self.command.is_empty() {
                None
            } else {
                Some(self.command.as_str())
            };
            tmux.start(cmd).await?;
            self.status = Status::Idle;
        }
        Ok(())
    }

    /// Stop the session
    pub async fn stop(&mut self) -> crate::Result<()> {
        if let Some(tmux) = &self.tmux_session {
            tmux.stop().await?;
            self.status = Status::Error;
        }
        Ok(())
    }

    /// Attach to the session
    pub async fn attach(&mut self) -> crate::Result<()> {
        self.mark_accessed();
        if let Some(tmux) = &self.tmux_session {
            tmux.attach().await?;
        }
        Ok(())
    }

    /// Check if session exists in tmux
    pub fn exists(&self) -> bool {
        self.tmux_session
            .as_ref()
            .map(|t| t.exists())
            .unwrap_or(false)
    }
}

/// Generate a unique session ID
fn generate_id() -> String {
    // Use first 12 chars of UUID for shorter IDs
    Uuid::new_v4().to_string()[..12].to_string()
}

/// Extract group path from project path
/// E.g., /home/user/projects/work/app -> projects/work
fn extract_group_path(path: &PathBuf) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    let path_str = path.to_str().unwrap_or("");
    let home_str = home.to_str().unwrap_or("");

    if path_str.starts_with(home_str) {
        let relative = path_str.strip_prefix(home_str).unwrap_or("");
        let parts: Vec<&str> = relative.trim_start_matches('/').split('/').collect();

        // Use first 2 directory levels as group
        if parts.len() >= 2 {
            format!("{}/{}", parts[0], parts[1])
        } else if parts.len() == 1 {
            parts[0].to_string()
        } else {
            String::new()
        }
    } else {
        // Not under home dir - use first directory
        let parts: Vec<&str> = path_str.trim_start_matches('/').split('/').collect();
        if !parts.is_empty() {
            parts[0].to_string()
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id = generate_id();
        assert_eq!(id.len(), 12);
    }

    #[test]
    fn test_extract_group_path() {
        let path = PathBuf::from("/home/user/projects/work/app");
        let group = extract_group_path(&path);
        println!("Group: {}", group);
        assert!(!group.is_empty());
    }

    #[test]
    fn test_is_sub_session() {
        let mut instance = Instance::new("test".to_string(), PathBuf::from("/tmp"));
        assert!(!instance.is_sub_session());

        instance.set_parent("parent-id".to_string());
        assert!(instance.is_sub_session());
    }
}
