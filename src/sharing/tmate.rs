use std::collections::HashMap;

/// Manages tmate companion processes for session sharing.
///
/// Each shared session gets a tmate process that mirrors the tmux pane
/// and provides SSH/web URLs for remote access.
pub struct TmateManager {
    /// Active tmate processes keyed by session ID
    processes: HashMap<String, TmateProcess>,
}

struct TmateProcess {
    socket_path: String,
    #[allow(dead_code)]
    child: Option<tokio::process::Child>,
}

impl TmateManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    /// Check if tmate binary is available on the system
    pub async fn is_available() -> bool {
        tokio::process::Command::new("tmate")
            .arg("-V")
            .output()
            .await
            .is_ok()
    }

    /// Check if a session is currently being shared
    pub fn is_sharing(&self, session_id: &str) -> bool {
        self.processes.contains_key(session_id)
    }

    /// Stop sharing a session
    pub async fn stop_sharing(&mut self, session_id: &str) -> crate::Result<()> {
        if let Some(mut process) = self.processes.remove(session_id) {
            if let Some(ref mut child) = process.child {
                let _ = child.kill().await;
            }
            // Clean up socket
            let _ = tokio::fs::remove_file(&process.socket_path).await;
        }
        Ok(())
    }

    /// Stop all active sharing sessions
    pub async fn stop_all(&mut self) -> crate::Result<()> {
        let ids: Vec<String> = self.processes.keys().cloned().collect();
        for id in ids {
            self.stop_sharing(&id).await?;
        }
        Ok(())
    }
}
