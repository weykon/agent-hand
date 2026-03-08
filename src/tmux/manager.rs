use std::collections::HashMap;
use std::sync::Arc;

use tokio::process::Command;

use crate::error::Result;

use super::cache::SessionCache;
use super::SESSION_PREFIX;

pub(crate) const TMUX_SERVER_NAME: &str = "agentdeck_rs";

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

    async fn ensure_server_bindings(&self) {
        // Best-effort: bind keys on our dedicated tmux server.
        let cfg = crate::config::ConfigFile::load().await.ok().flatten();

        let detach_key = cfg
            .as_ref()
            .and_then(|c| c.tmux_detach_key())
            .and_then(crate::config::parse_tmux_key)
            .unwrap_or_else(|| "C-q".to_string());

        let switch_key = cfg
            .as_ref()
            .and_then(|c| c.tmux_switcher_key())
            .and_then(crate::config::parse_tmux_key)
            .unwrap_or_else(|| "C-g".to_string());

        let jump_key = cfg
            .as_ref()
            .and_then(|c| c.tmux_jump_key())
            .and_then(|s| {
                let t = s.trim();
                if t.eq_ignore_ascii_case("off") || t.eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(t)
                }
            })
            .and_then(crate::config::parse_tmux_key)
            .unwrap_or_else(|| "C-n".to_string());

        // Check current bindings - skip if already correct (multi-instance safety)
        let current_detach = self
            .get_environment_global("AGENTHAND_DETACH_KEY")
            .await
            .ok()
            .flatten();
        let current_switch = self
            .get_environment_global("AGENTHAND_SWITCHER_KEY")
            .await
            .ok()
            .flatten();
        let current_jump = self
            .get_environment_global("AGENTHAND_JUMP_KEY")
            .await
            .ok()
            .flatten();

        let need_detach_bind = current_detach.as_deref() != Some(detach_key.as_str());
        let need_switch_bind = current_switch.as_deref() != Some(switch_key.as_str());
        let need_jump_rebind = current_jump.as_deref() != Some(jump_key.as_str());

        // Unbind previous custom keys if they differ
        if need_detach_bind {
            if let Some(old) = &current_detach {
                let _ = self
                    .tmux_cmd()
                    .args(["unbind-key", "-n", old.as_str()])
                    .status()
                    .await;
            }
        }
        if need_switch_bind {
            if let Some(old) = &current_switch {
                let _ = self
                    .tmux_cmd()
                    .args(["unbind-key", "-n", old.as_str()])
                    .status()
                    .await;
            }
        }
        if need_jump_rebind {
            if let Some(old) = &current_jump {
                let _ = self
                    .tmux_cmd()
                    .args(["unbind-key", "-n", old.as_str()])
                    .status()
                    .await;
            }
        }

        // Detach key - only bind if needed
        if need_detach_bind {
            let _ = self
                .tmux_cmd()
                .args([
                    "bind-key",
                    "-n",
                    detach_key.as_str(),
                    "set-environment",
                    "-g",
                    "AGENTHAND_LAST_SESSION",
                    "#{session_name}",
                    "\\;",
                    "set-environment",
                    "-g",
                    "AGENTHAND_LAST_DETACH_AT",
                    "#{client_activity}",
                    "\\;",
                    "detach-client",
                ])
                .status()
                .await;
            let _ = self
                .set_environment_global("AGENTHAND_DETACH_KEY", detach_key.as_str())
                .await;
        }

        // Popup switcher key - only bind if needed
        if need_switch_bind {
            let switch_bin = std::env::current_exe()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "agent-hand".to_string());
            let _ = self
                .tmux_cmd()
                .args([
                    "bind-key",
                    "-n",
                    switch_key.as_str(),
                    "display-popup",
                    "-E",
                    "-w",
                    "90%",
                    "-h",
                    "70%",
                    &switch_bin,
                    "switch",
                ])
                .status()
                .await;
            let _ = self
                .set_environment_global("AGENTHAND_SWITCHER_KEY", switch_key.as_str())
                .await;
        }

        // Jump-to-priority key (Ctrl+N by default) - call `agent-hand jump` which
        // computes the priority target and switches directly.
        {
            let jump_bin = std::env::current_exe()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "agent-hand".to_string());
            let jump_bin_escaped = jump_bin.replace('\'', "'\\''");
            let jump_cmd = format!("'{}'  jump", jump_bin_escaped);

            let _ = self
                .tmux_cmd()
                .args([
                    "bind-key",
                    "-n",
                    jump_key.as_str(),
                    "run-shell",
                    jump_cmd.as_str(),
                ])
                .status()
                .await;
            let _ = self
                .set_environment_global("AGENTHAND_JUMP_KEY", jump_key.as_str())
                .await;
        }

        // Ensure tmux popups see the active profile.
        if let Ok(profile) = std::env::var("AGENTHAND_PROFILE") {
            let _ = self
                .tmux_cmd()
                .args(["set-environment", "-g", "AGENTHAND_PROFILE", &profile])
                .status()
                .await;
        }

        // Enable mouse so scroll wheel drives tmux copy-mode/pane scrolling.
        let _ = self
            .tmux_cmd()
            .args(["set-option", "-g", "mouse", "on"])
            .status()
            .await;

        // Copy-mode defaults (dedicated server only). Default: vi.
        let copy_mode = cfg
            .as_ref()
            .and_then(|c| c.tmux_copy_mode())
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_else(|| "vi".to_string());

        if copy_mode != "off" && copy_mode != "none" {
            let mode_keys = if copy_mode == "emacs" { "emacs" } else { "vi" };
            let _ = self
                .tmux_cmd()
                .args(["set-option", "-g", "mode-keys", mode_keys])
                .status()
                .await;

            if mode_keys == "vi" {
                let _ = self
                    .tmux_cmd()
                    .args([
                        "bind-key",
                        "-T",
                        "copy-mode-vi",
                        "v",
                        "send",
                        "-X",
                        "begin-selection",
                    ])
                    .status()
                    .await;
                let _ = self
                    .tmux_cmd()
                    .args([
                        "bind-key",
                        "-T",
                        "copy-mode-vi",
                        "Space",
                        "send",
                        "-X",
                        "begin-selection",
                    ])
                    .status()
                    .await;
                let _ = self
                    .tmux_cmd()
                    .args([
                        "bind-key",
                        "-T",
                        "copy-mode-vi",
                        "V",
                        "send",
                        "-X",
                        "select-line",
                    ])
                    .status()
                    .await;
                // Pipe selected text to system clipboard on y/Enter/mouse-release.
                // copy-pipe-and-cancel = copy to tmux buffer + pipe to cmd + exit copy-mode.
                let copy_cmd = if cfg!(target_os = "macos") {
                    "pbcopy"
                } else {
                    "xclip -selection clipboard 2>/dev/null || xsel --clipboard 2>/dev/null"
                };
                for key in ["y", "Enter"] {
                    let _ = self
                        .tmux_cmd()
                        .args([
                            "bind-key",
                            "-T",
                            "copy-mode-vi",
                            key,
                            "send",
                            "-X",
                            "copy-pipe-and-cancel",
                            copy_cmd,
                        ])
                        .status()
                        .await;
                }
                // Mouse drag release: auto-copy selection to system clipboard.
                let _ = self
                    .tmux_cmd()
                    .args([
                        "bind-key",
                        "-T",
                        "copy-mode-vi",
                        "MouseDragEnd1Pane",
                        "send",
                        "-X",
                        "copy-pipe-and-cancel",
                        copy_cmd,
                    ])
                    .status()
                    .await;
            }
        }

        // Compact status-left badge driven by agent-hand's own status probing.
        let status_bin = std::env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "agent-hand".to_string());
        let status_bin_escaped = status_bin.replace('\'', "'\\''");
        let status_left = format!("#{{?@agenthand_title,#{{@agenthand_title}},#S}}  #('{}' statusline)", status_bin_escaped);
        let _ = self
            .tmux_cmd()
            .args(["set-option", "-g", "status-interval", "5"])
            .status()
            .await;
        let _ = self
            .tmux_cmd()
            .args(["set-option", "-g", "status-left-length", "80"])
            .status()
            .await;
        let _ = self
            .tmux_cmd()
            .args(["set-option", "-g", "status-left", status_left.as_str()])
            .status()
            .await;
    }

    /// Ensure our dedicated tmux server has required bindings/options.
    pub async fn ensure_server(&self) {
        self.ensure_server_bindings().await;
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

    /// Legacy tmux session name format (for backward compat with old sessions).
    pub fn session_name_legacy(id: &str) -> String {
        format!("{}{}", SESSION_PREFIX, id)
    }

    /// Build a human-readable tmux session name from title + ID.
    /// Format: `{sanitized_title}_{first_8_of_id}`
    pub fn build_session_name(title: &str, id: &str) -> String {
        let sanitized = sanitize_for_tmux(title);
        let short_id = &id[..id.len().min(8)];
        if sanitized.is_empty() {
            format!("session_{}", short_id)
        } else {
            format!("{}_{}", sanitized, short_id)
        }
    }

    /// Rename a tmux session
    pub async fn rename_session(&self, old_name: &str, new_name: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&["rename-session", "-t", old_name, new_name])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to rename session: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Get tmux session name for a session ID (legacy alias)
    #[deprecated(note = "Use Instance::tmux_name() or build_session_name() instead")]
    pub fn session_name(id: &str) -> String {
        Self::session_name_legacy(id)
    }

    /// Create a new tmux session.
    /// If `title` is provided, it is stored as `@agenthand_title` so the
    /// status bar shows the user-friendly name instead of the internal session id.
    pub async fn create_session(
        &self,
        name: &str,
        working_dir: &str,
        command: Option<&str>,
        title: Option<&str>,
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

        // Build the shell command
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

        if let Some(command) = command {
            cmd.arg(command);
        } else {
            // Login shell (no command)
            cmd.args([&shell, "-l"]);
        }

        let output = cmd.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If the session already exists (cache can be stale), treat it as success.
            if stderr.contains("duplicate session") {
                self.ensure_server_bindings().await;
                self.register_session(name.to_string());
                if let Some(t) = title {
                    let _ = self.set_session_title(name, t).await;
                }
                return Ok(());
            }
            return Err(crate::Error::tmux(format!(
                "Failed to create session: {}",
                stderr
            )));
        }

        // Set bindings after tmux server is running
        self.ensure_server_bindings().await;

        // Register in cache immediately
        self.register_session(name.to_string());

        // Stamp the friendly title so status-left can display it.
        if let Some(t) = title {
            let _ = self.set_session_title(name, t).await;
        }

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

    /// Capture pane content with ANSI escape codes for full visual fidelity.
    /// Used by the relay client to send terminal snapshots to viewers.
    #[cfg(feature = "pro")]
    pub async fn capture_pane_ansi(&self, name: &str) -> Result<Vec<u8>> {
        let output = self
            .tmux_cmd()
            .args(&[
                "capture-pane",
                "-t",
                name,
                "-p",  // Print to stdout
                "-e",  // Include escape sequences (ANSI colors, etc.)
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        Ok(output.stdout)
    }

    /// Capture the pane's visible screen plus scrollback history.
    /// Returns rendered ANSI text with `\n` line endings.
    /// `scrollback_lines`: how many lines of history to capture (e.g. 5000).
    #[cfg(feature = "pro")]
    pub async fn capture_pane_with_scrollback(
        &self,
        name: &str,
        scrollback_lines: usize,
    ) -> Result<Vec<u8>> {
        let start_line = format!("-{}", scrollback_lines);
        let output = self
            .tmux_cmd()
            .args(&[
                "capture-pane",
                "-t",
                name,
                "-p",  // Print to stdout
                "-e",  // Include escape sequences (ANSI colors, etc.)
                "-S",
                &start_line, // Start from N lines back in scrollback
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        Ok(output.stdout)
    }

    /// Start pipe-pane to stream PTY output to a named pipe / file.
    /// Returns the pipe path. The caller reads from this pipe and
    /// forwards bytes over WebSocket.
    #[cfg(feature = "pro")]
    pub async fn start_pipe_pane(&self, name: &str, pipe_path: &str) -> Result<()> {
        // Ensure the pipe path's parent directory exists
        if let Some(parent) = std::path::Path::new(pipe_path).parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let pipe_cmd = format!("cat >> {}", pipe_path);
        let output = self
            .tmux_cmd()
            .args(&[
                "pipe-pane",
                "-t",
                name,
                "-O",  // Only output (not input)
                &pipe_cmd,
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to start pipe-pane: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Stop pipe-pane on a session (passing empty string disables it).
    #[cfg(feature = "pro")]
    pub async fn stop_pipe_pane(&self, name: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&["pipe-pane", "-t", name])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to stop pipe-pane: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Get the current terminal size of a pane.
    #[cfg(feature = "pro")]
    pub async fn pane_size(&self, name: &str) -> Result<(u16, u16)> {
        let output = self
            .tmux_cmd()
            .args(&[
                "display-message",
                "-t",
                name,
                "-p",
                "#{pane_width} #{pane_height}",
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok((80, 24)); // fallback
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = text.trim().split_whitespace().collect();
        if parts.len() == 2 {
            let cols = parts[0].parse().unwrap_or(80);
            let rows = parts[1].parse().unwrap_or(24);
            Ok((cols, rows))
        } else {
            Ok((80, 24))
        }
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

    /// Send literal text to a tmux pane without appending Enter.
    /// Used for forwarding raw viewer input from relay collaboration.
    #[cfg(feature = "pro")]
    pub async fn send_keys_literal(&self, name: &str, text: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(&["send-keys", "-t", name, "-l", text])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to send literal keys: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Attach to a session (blocking)
    pub async fn attach_session(&self, name: &str) -> Result<()> {
        self.ensure_server_bindings().await;

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

    /// Set the user-visible title on a tmux session (stored as `@agenthand_title`).
    /// The status-left format reads this to display the friendly name.
    pub async fn set_session_title(&self, session_name: &str, title: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(["set-option", "-t", session_name, "@agenthand_title", title])
            .output()
            .await?;
        if !output.status.success() {
            // Non-fatal: the session may have been killed between check and set.
            eprintln!(
                "set_session_title({session_name}): {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    /// Set a global tmux environment variable on our dedicated server.
    pub async fn set_environment_global(&self, key: &str, value: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(["set-environment", "-g", key, value])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to set tmux env {key}: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Get a global tmux environment variable from our dedicated server.
    pub async fn get_environment_global(&self, key: &str) -> Result<Option<String>> {
        let output = self
            .tmux_cmd()
            .args(["show-environment", "-g", key])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(None);
        }

        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let prefix = format!("{key}=");
        if line.starts_with(&prefix) {
            Ok(Some(line[prefix.len()..].to_string()))
        } else {
            Ok(None)
        }
    }

    /// Switch current tmux client to a target session
    pub async fn switch_client(&self, name: &str) -> Result<()> {
        let output = self
            .tmux_cmd()
            .args(["switch-client", "-t", name])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::Error::tmux(format!(
                "Failed to switch client: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Kill orphaned tmux sessions that exist in tmux but not in the known set of tmux names.
    /// Returns the number of sessions killed.
    pub async fn cleanup_orphaned_sessions(&self, known_tmux_names: &[&str]) -> usize {
        let tmux_sessions = match self.list_sessions().await {
            Ok(s) => s,
            Err(_) => return 0,
        };

        let mut killed = 0;

        for tmux_name in &tmux_sessions {
            if !known_tmux_names.iter().any(|name| *name == tmux_name.as_str()) {
                if self.kill_session(tmux_name).await.is_ok() {
                    killed += 1;
                }
            }
        }

        killed
    }

    /// List all sessions on our dedicated tmux server
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
        // All sessions on our dedicated server (agentdeck_rs) belong to us.
        let sessions: Vec<String> = stdout
            .lines()
            .filter(|line| !line.is_empty())
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

/// Sanitize a title for use as a tmux session name component.
/// Tmux forbids dots, colons, and certain special chars in session names.
fn sanitize_for_tmux(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            _ => '-',
        })
        .collect();

    // Collapse consecutive dashes
    let mut result = String::new();
    let mut last_was_dash = true; // treat start as dash to trim leading
    for c in s.chars() {
        if c == '-' {
            if !last_was_dash {
                result.push(c);
            }
            last_was_dash = true;
        } else {
            result.push(c);
            last_was_dash = false;
        }
    }

    let result = result.trim_end_matches('-');
    if result.len() > 30 {
        result[..30]
            .trim_end_matches('-')
            .to_string()
    } else {
        result.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(deprecated)]
    #[test]
    fn test_session_name_legacy() {
        assert_eq!(TmuxManager::session_name("abc123"), "agentdeck_rs_abc123");
        assert_eq!(
            TmuxManager::session_name_legacy("abc123"),
            "agentdeck_rs_abc123"
        );
    }

    #[test]
    fn test_build_session_name() {
        assert_eq!(
            TmuxManager::build_session_name("My Project", "a1b2c3d4e5f6"),
            "my-project_a1b2c3d4"
        );
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(
            TmuxManager::build_session_name("hello.world:test!", "abcdef123456"),
            "hello-world-test_abcdef12"
        );
    }

    #[test]
    fn test_empty_title() {
        assert_eq!(
            TmuxManager::build_session_name("", "abcdef123456"),
            "session_abcdef12"
        );
    }

    #[tokio::test]
    async fn test_tmux_available() {
        let available = TmuxManager::is_available().await.unwrap_or(false);
        println!("Tmux available: {}", available);
    }
}
