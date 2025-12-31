use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;

use super::detector::{PromptDetector, Tool};
use super::manager::TmuxManager;
use crate::error::Result;

/// Status of a tmux session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Running,  // Actively working
    Waiting,  // Needs user input
    Idle,     // Ready for commands
    Error,    // Session doesn't exist or error
    Starting, // Being created
}

/// Wrapper around a tmux session
#[derive(Debug)]
pub struct TmuxSession {
    name: String,
    working_dir: PathBuf,
    tool: Tool,
    manager: Arc<TmuxManager>,
    status: Arc<RwLock<SessionStatus>>,
    last_activity: Arc<RwLock<Option<SystemTime>>>,
}

impl TmuxSession {
    pub fn new(name: String, working_dir: PathBuf, tool: Tool, manager: Arc<TmuxManager>) -> Self {
        Self {
            name,
            working_dir,
            tool,
            manager,
            status: Arc::new(RwLock::new(SessionStatus::Idle)),
            last_activity: Arc::new(RwLock::new(None)),
        }
    }

    /// Get session name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get working directory
    pub fn working_dir(&self) -> &PathBuf {
        &self.working_dir
    }

    /// Get tool type
    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Get current status
    pub fn status(&self) -> SessionStatus {
        *self.status.read()
    }

    /// Set status
    pub fn set_status(&self, status: SessionStatus) {
        *self.status.write() = status;
    }

    /// Check if session exists in tmux
    pub fn exists(&self) -> bool {
        self.manager.session_exists(&self.name).unwrap_or(false)
    }

    /// Update status by checking tmux pane content
    pub async fn update_status(&self) -> Result<SessionStatus> {
        // Check if session exists
        if !self.exists() {
            self.set_status(SessionStatus::Error);
            return Ok(SessionStatus::Error);
        }

        // Capture recent pane content
        let content = self.manager.capture_pane(&self.name, 50).await?;

        // Use prompt detector to determine state
        let detector = PromptDetector::new(self.tool);
        let has_prompt = detector.has_prompt(&content);

        // Check for activity changes
        let activity = self.manager.session_activity(&self.name);
        let last_activity = *self.last_activity.read();

        let new_status = if has_prompt {
            SessionStatus::Waiting
        } else if let (Some(current), Some(last)) = (activity, last_activity) {
            let last_secs = last
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            if current > last_secs {
                // Activity changed - running
                SessionStatus::Running
            } else {
                // No activity - idle
                SessionStatus::Idle
            }
        } else {
            // No previous activity data - assume idle
            SessionStatus::Idle
        };

        // Update activity timestamp
        if let Some(activity) = activity {
            *self.last_activity.write() =
                Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(activity as u64));
        }

        self.set_status(new_status);
        Ok(new_status)
    }

    /// Start the session (create in tmux)
    pub async fn start(&self, command: Option<&str>) -> Result<()> {
        self.set_status(SessionStatus::Starting);

        self.manager
            .create_session(&self.name, self.working_dir.to_str().unwrap(), command)
            .await?;

        self.set_status(SessionStatus::Idle);
        Ok(())
    }

    /// Stop the session (kill in tmux)
    pub async fn stop(&self) -> Result<()> {
        self.manager.kill_session(&self.name).await?;
        self.set_status(SessionStatus::Error);
        Ok(())
    }

    /// Send keys to the session
    pub async fn send_keys(&self, keys: &str) -> Result<()> {
        self.manager.send_keys(&self.name, keys).await
    }

    /// Attach to the session
    pub async fn attach(&self) -> Result<()> {
        self.manager.attach_session(&self.name).await
    }

    /// Get pane content (for debugging or output extraction)
    pub async fn get_content(&self, lines: usize) -> Result<String> {
        self.manager.capture_pane(&self.name, lines).await
    }
}
