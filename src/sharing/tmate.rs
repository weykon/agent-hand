use std::collections::HashMap;

use chrono::Utc;
use tokio::process::Command as TokioCommand;

use super::{ShareLink, SharePermission, SharingState};

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
        TokioCommand::new("tmate")
            .arg("-V")
            .output()
            .await
            .is_ok()
    }

    /// Check if a session is currently being shared
    pub fn is_sharing(&self, session_id: &str) -> bool {
        self.processes.contains_key(session_id)
    }

    /// Start sharing a session via tmate.
    ///
    /// Spawns a tmate process that attaches to the given tmux session in
    /// read-only mode, waits for initialization, then queries the generated
    /// SSH/web URLs and returns them as a `SharingState`.
    pub async fn start_sharing(
        &mut self,
        session_id: &str,
        tmux_session_name: &str,
        permission: SharePermission,
        auto_expire_minutes: Option<u64>,
    ) -> crate::Result<SharingState> {
        let socket = format!("/tmp/tmate-{}.sock", session_id);

        // Clean up old socket if exists
        let _ = tokio::fs::remove_file(&socket).await;

        // Spawn tmate attached to the target tmux session
        let cmd = format!(
            "tmux -L agentdeck_rs attach -t {} -r",
            tmux_session_name
        );
        let child = TokioCommand::new("tmate")
            .args(["-S", &socket, "new-session", "-d", &cmd])
            .spawn()
            .map_err(|e| crate::Error::Other(format!("Failed to start tmate: {}", e)))?;

        // Wait for tmate to initialize and register with the server
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Query URLs from the tmate session
        let ssh_url = Self::query_tmate_var(&socket, "#{tmate_ssh}")
            .await
            .unwrap_or_default();
        let ssh_ro_url = Self::query_tmate_var(&socket, "#{tmate_ssh_ro}")
            .await
            .unwrap_or_default();
        let web_url = Self::query_tmate_var(&socket, "#{tmate_web}").await.ok();
        let web_ro_url = Self::query_tmate_var(&socket, "#{tmate_web_ro}")
            .await
            .ok();

        // Build links based on requested permission
        let mut links = Vec::new();

        // Always include read-only link
        if !ssh_ro_url.is_empty() {
            links.push(ShareLink {
                permission: SharePermission::ReadOnly,
                ssh_url: ssh_ro_url,
                web_url: web_ro_url,
                created_at: Utc::now(),
                expires_at: None,
            });
        }

        // Include read-write link only if permission allows
        if permission == SharePermission::ReadWrite && !ssh_url.is_empty() {
            links.push(ShareLink {
                permission: SharePermission::ReadWrite,
                ssh_url,
                web_url,
                created_at: Utc::now(),
                expires_at: None,
            });
        }

        let state = SharingState {
            active: true,
            tmate_socket: socket.clone(),
            links,
            default_permission: permission,
            started_at: Utc::now(),
            auto_expire_minutes,
        };

        self.processes.insert(
            session_id.to_string(),
            TmateProcess {
                socket_path: socket,
                child: Some(child),
            },
        );

        Ok(state)
    }

    /// Query a tmate format variable from an existing socket.
    async fn query_tmate_var(socket: &str, var: &str) -> crate::Result<String> {
        let output = TokioCommand::new("tmate")
            .args(["-S", socket, "display", "-p", var])
            .output()
            .await
            .map_err(|e| crate::Error::Other(format!("tmate query failed: {}", e)))?;

        if !output.status.success() {
            return Err(crate::Error::Other("tmate display failed".to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Re-query URLs from an existing tmate socket for the given session.
    pub async fn get_urls(&self, session_id: &str) -> Option<Vec<ShareLink>> {
        let process = self.processes.get(session_id)?;
        let socket = &process.socket_path;

        let ssh_url = Self::query_tmate_var(socket, "#{tmate_ssh}")
            .await
            .unwrap_or_default();
        let ssh_ro_url = Self::query_tmate_var(socket, "#{tmate_ssh_ro}")
            .await
            .unwrap_or_default();
        let web_url = Self::query_tmate_var(socket, "#{tmate_web}").await.ok();
        let web_ro_url = Self::query_tmate_var(socket, "#{tmate_web_ro}")
            .await
            .ok();

        let mut links = Vec::new();

        if !ssh_ro_url.is_empty() {
            links.push(ShareLink {
                permission: SharePermission::ReadOnly,
                ssh_url: ssh_ro_url,
                web_url: web_ro_url,
                created_at: Utc::now(),
                expires_at: None,
            });
        }

        if !ssh_url.is_empty() {
            links.push(ShareLink {
                permission: SharePermission::ReadWrite,
                ssh_url,
                web_url,
                created_at: Utc::now(),
                expires_at: None,
            });
        }

        if links.is_empty() {
            None
        } else {
            Some(links)
        }
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
