use std::collections::HashMap;
use std::sync::Arc;

use tokio::process::Command;

use crate::error::Result;

use super::cache::SessionCache;
use super::SESSION_PREFIX;

const TMUX_SERVER_NAME: &str = "agentdeck_rs";

/// Tmux manager - handles all tmux operations
#[derive(Debug)]
pub struct TmuxManager {
    cache: Arc<SessionCache>,
}

impl TmuxManager {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(SessionCache::new()),
        }
    }

    fn tmux_cmd(&self) -> Command {
        let mut cmd = Command::new("tmux");
        cmd.args(["-L", TMUX_SERVER_NAME]);
        cmd
    }

    async fn ensure_ctrl_q_detach(&self) {
        // Best-effort: bind Ctrl+Q to detach (like Go agent-deck) on our dedicated tmux server.
        let _ = self
            .tmux_cmd()
            .args(["bind-key", "-n", "C-q", "detach-client"])
            .status()
            .await;
    }

    /// Check if tmux is available
    pub async fn is_available() -> Result<bool> {
        let output = Command::new("tmux").arg("-V").output().await;
        Ok(output.is_ok())
    }

    /// Refresh session cache from tmux
    /// Call this ONCE per tick, then use cached methods
    pub async fn refresh_cache(&self) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&[
                "list-sessions",
                "-F",
                "#{session_name}\t#{session_activity}",
            ])
            .output()
            .await?;

        if !output.status.success() {
            // tmux not running or no sessions - clear cache
            self.cache.clear();
            return Ok(());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sessions = HashMap::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                let name = parts[0].to_string();
                let activity = parts[1].parse::<i64>().unwrap_or(0);
                sessions.insert(name, activity);
            }
        }

        self.cache.update(sessions);
        Ok(())
    }

    /// Check if session exists (from cache)
    pub fn session_exists(&self, name: &str) -> Option<bool> {
        self.cache.exists(name)
    }

    /// Get session activity (from cache)
    pub fn session_activity(&self, name: &str) -> Option<i64> {
        self.cache.activity(name)
    }

    /// Register a newly created session in cache
    pub fn register_session(&self, name: String) {
        self.cache.register(name);
    }

    /// Get tmux session name for a session ID
    pub fn session_name(id: &str) -> String {
        format!("{}{}", SESSION_PREFIX, id)
    }

    /// Create a new tmux session
    pub async fn create_session(
        &self,
        name: &str,
        working_dir: &str,
        command: Option<&str>,
    ) -> Result<()> {
        let mut cmd = self.tmux_cmd();
        cmd.args(&[
            "new-session",
            "-d", // Detached
            "-s",
            name,
            "-c",
            working_dir,
        ]);

        if let Some(command) = command {
            cmd.arg(command);
        }

        let output = cmd.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If the session already exists (cache can be stale), treat it as success.
            if stderr.contains("duplicate session") {
                self.ensure_ctrl_q_detach().await;
                self.register_session(name.to_string());
                return Ok(());
            }
            return Err(crate::Error::tmux(format!(
                "Failed to create session: {}",
                stderr
            )));
        }

        // Set Ctrl+Q binding after tmux server is running
        self.ensure_ctrl_q_detach().await;

        // Register in cache immediately
        self.register_session(name.to_string());

        Ok(())
    }

    /// Kill a tmux session
    pub async fn kill_session(&self, name: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&["kill-session", "-t", name])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to kill session: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Capture pane content (for status detection)
    pub async fn capture_pane(&self, name: &str, lines: usize) -> Result<String> {
        let output = self
            .tmux_cmd()
            .args(&[
                "capture-pane",
                "-t",
                name,
                "-p", // Print to stdout
                "-S",
                &format!("-{}", lines), // Start line
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(String::new());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Send keys to a session
    pub async fn send_keys(&self, name: &str, keys: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&["send-keys", "-t", name, keys, "Enter"])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to send keys: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Attach to a session (blocking)
    pub async fn attach_session(&self, name: &str) -> Result<()> {
        self.ensure_ctrl_q_detach().await;

        let status = self
            .tmux_cmd()
            .args(&["attach-session", "-t", name])
            .status()
            .await?;

        if !status.success() {
            return Err(crate::Error::tmux("Failed to attach to session"));
        }

        Ok(())
    }

    /// List all agent-deck sessions
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        let output = self
            .tmux_cmd()
            .args(&["list-sessions", "-F", "#{session_name}"])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let sessions: Vec<String> = stdout
            .lines()
            .filter(|line| line.starts_with(SESSION_PREFIX))
            .map(|s| s.to_string())
            .collect();

        Ok(sessions)
    }
}

impl Default for TmuxManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_name() {
        assert_eq!(TmuxManager::session_name("abc123"), "agentdeck_rs_abc123");
    }

    #[tokio::test]
    async fn test_tmux_available() {
        // This will fail in CI without tmux, but useful for local testing
        let available = TmuxManager::is_available().await.unwrap_or(false);
        println!("Tmux available: {}", available);
    }
}
