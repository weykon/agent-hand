use super::*;

impl App {

    #[cfg(feature = "pro")]
    pub fn get_selected_viewer_session(&self) -> Option<String> {
        self.pro.viewer_sessions.keys()
            .nth(self.pro.viewer_panel_selected)
            .cloned()
    }

    /// Connect to a shared session as a viewer via tmux-on-viewer.
    ///
    /// Creates a tmux session running `pty-viewer` (WebSocket ↔ stdio bridge),
    /// then stores minimal state. The actual tmux attach happens separately via
    /// `perform_viewer_attach()` which is called from the event loop.
    #[cfg(feature = "pro")]
    pub async fn connect_viewer(&mut self, relay_url: &str, room_id: &str, viewer_token: &str) -> Result<()> {
        // Set status to Connecting
        let session_info = ViewerSessionInfo {
            room_id: room_id.to_string(),
            relay_url: relay_url.to_string(),
            viewer_token: viewer_token.to_string(),
            connected_at: std::time::SystemTime::now(),
            status: ViewerSessionStatus::Connecting,
            session_name: None,
        };
        self.pro.viewer_sessions.insert(room_id.to_string(), session_info);

        // Extract display name and access token from auth token
        let viewer_display_name = self.auth_token.as_ref().map(|t| {
            t.email.split('@').next().unwrap_or(&t.email).to_string()
        });
        let viewer_user_token = self.auth_token.as_ref().map(|t| t.access_token.clone());

        // Create tmux viewer session running pty-viewer
        let session_name = self.tmux.create_viewer_session(
            room_id,
            relay_url,
            viewer_token,
            None, // host session name not known yet
            viewer_user_token.as_deref(),
            viewer_display_name.as_deref(),
        ).await?;

        // Update session status
        if let Some(session) = self.pro.viewer_sessions.get_mut(room_id) {
            session.status = ViewerSessionStatus::Connected;
        }

        self.pro.viewer_state = Some(ViewerState {
            room_id: room_id.to_string(),
            session_name: session_name.clone(),
            host_session_name: None,
            connected: true,
            viewer_identity: viewer_display_name,
            has_rw_control: false,
            connected_at: Some(Instant::now()),
            tmux_pid: None,
        });

        self.state = AppState::ViewerMode;
        Ok(())
    }

    /// Attach to the viewer tmux session (blocking). Called from the event loop.
    ///
    /// This suspends the TUI, attaches to tmux (user sees native terminal rendering),
    /// and returns when the user detaches (Ctrl+Q). Then we restore the TUI.
    #[cfg(feature = "pro")]
    pub(super) async fn perform_viewer_attach(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let session_name = match &self.pro.viewer_state {
            Some(vs) => vs.session_name.clone(),
            None => return Ok(()),
        };

        // Suspend TUI (same pattern as perform_attach for regular sessions)
        disable_raw_mode()?;
        if self.mouse_captured {
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
        } else {
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        }
        terminal.show_cursor()?;

        // Attach to viewer tmux session (BLOCKING — user sees native terminal)
        let attach_result = self.tmux.attach_session(&session_name).await;

        // Restore TUI
        enable_raw_mode()?;
        if self.mouse_captured {
            execute!(
                terminal.backend_mut(),
                EnterAlternateScreen,
                EnableMouseCapture
            )?;
        } else {
            execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        }
        terminal.clear()?;

        // User detached from tmux — check if the session is still alive
        let session_alive = self.tmux.session_exists(&session_name).unwrap_or(false);

        if !session_alive {
            // pty-viewer exited (room closed, auth failed, etc.) — clean up
            self.disconnect_viewer();
        } else {
            // User just detached (Ctrl+Q) — return to TUI session list but keep session alive
            self.state = AppState::Normal;
        }

        // We don't propagate attach_result errors (non-zero exit from tmux attach is normal on detach)
        let _ = attach_result;
        Ok(())
    }

    /// Disconnect from a viewed session and return to normal mode.
    #[cfg(feature = "pro")]
    pub fn disconnect_viewer(&mut self) {
        if let Some(vs) = self.pro.viewer_state.take() {
            // Update session status to Disconnected
            if let Some(session) = self.pro.viewer_sessions.get_mut(&vs.room_id) {
                session.status = ViewerSessionStatus::Disconnected;
            }

            // Kill the viewer tmux session (best-effort)
            let tmux = self.tmux.clone();
            let session_name = vs.session_name.clone();
            tokio::spawn(async move {
                let _ = tmux.kill_session(&session_name).await;
            });

            // Clean up stats file
            let room_id = vs.room_id.clone();
            tokio::spawn(async move {
                let stats_path = format!("/tmp/agenthand-viewer-{}.json", room_id);
                let _ = tokio::fs::remove_file(&stats_path).await;
            });
        }
        self.state = AppState::Normal;
    }

    /// Disconnect from a specific viewer session by room_id.
    /// If delete_session is true, also remove the session metadata.
    #[cfg(feature = "pro")]
    pub async fn disconnect_viewer_session(&mut self, room_id: &str, delete_session: bool) {
        // Update status to Disconnected
        if let Some(session) = self.pro.viewer_sessions.get_mut(room_id) {
            session.status = ViewerSessionStatus::Disconnected;
        }

        // If currently viewing this session, exit viewer mode
        if self.state == AppState::ViewerMode {
            if let Some(ref viewer_state) = self.pro.viewer_state {
                if viewer_state.room_id == room_id {
                    self.disconnect_viewer();
                }
            }
        }

        // Delete session metadata if requested
        if delete_session {
            self.pro.viewer_sessions.remove(room_id);
        }
    }

    /// Reconnect to a viewer session by room_id.
    #[cfg(feature = "pro")]
    pub async fn reconnect_viewer(&mut self, room_id: &str) -> Result<()> {
        // Get session info
        let session_info = self.pro.viewer_sessions.get(room_id)
            .ok_or_else(|| crate::error::Error::Other("Session not found".to_string()))?
            .clone();

        // Update status to Reconnecting
        if let Some(session) = self.pro.viewer_sessions.get_mut(room_id) {
            session.status = ViewerSessionStatus::Reconnecting;
        }

        // Reuse connect_viewer logic
        self.connect_viewer(&session_info.relay_url, &session_info.room_id, &session_info.viewer_token).await
    }

    /// Handle key events in viewer mode.
    ///
    /// With tmux-on-viewer, the TUI is in ViewerMode only briefly before
    /// `perform_viewer_attach` is called. After the user detaches from tmux,
    /// the state transitions back to Normal. This handler covers the transitional
    /// period and any post-attach state management.
    #[cfg(feature = "pro")]
    pub(super) async fn handle_viewer_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        // Ctrl+Q or Esc: return to dashboard
        if key == KeyCode::Char('q') && modifiers.contains(KeyModifiers::CONTROL) {
            self.disconnect_viewer();
            return Ok(());
        }
        if key == KeyCode::Esc {
            self.disconnect_viewer();
            return Ok(());
        }

        // 'd': disconnect and remove the viewer session
        if key == KeyCode::Char('d') {
            if let Some(ref vs) = self.pro.viewer_state {
                let room_id = vs.room_id.clone();
                self.disconnect_viewer_session(&room_id, false).await;
            }
            return Ok(());
        }

        // 'r': reconnect
        if key == KeyCode::Char('r') {
            if let Some(ref vs) = self.pro.viewer_state {
                let room_id = vs.room_id.clone();
                let _ = self.reconnect_viewer(&room_id).await;
            }
            return Ok(());
        }

        // Enter: re-attach to tmux viewer session
        if key == KeyCode::Enter {
            // perform_viewer_attach is called from the event loop, not here
            // We just stay in ViewerMode; the event loop will pick it up
            return Ok(());
        }

        Ok(())
    }

    /// Get the current viewer state (for rendering).
    #[cfg(feature = "pro")]
    pub fn viewer_state(&self) -> Option<&ViewerState> {
        self.pro.viewer_state.as_ref()
    }

    /// Get a relay client by session ID (for rendering viewer info).
    #[cfg(feature = "pro")]
    pub fn relay_client(&self, session_id: &str) -> Option<&Arc<crate::pro::collab::client::RelayClient>> {
        self.pro.relay_clients.get(session_id)
    }

    /// Check if the user is currently hosting any shared sessions.
    #[cfg(feature = "pro")]
    pub fn hosting_session_count(&self) -> usize {
        self.pro.relay_clients.len()
    }

    /// Poll all relay clients for pending control requests and show dialog for the first one.
    #[cfg(feature = "pro")]
    pub(super) async fn poll_control_requests(&mut self) {
        use crate::ui::dialogs::{ControlRequestDialog, Dialog};

        // Collect session IDs to check (avoid borrow issues)
        let session_ids: Vec<String> = self.pro.relay_clients.keys().cloned().collect();

        for sid in session_ids {
            if let Some(client) = self.pro.relay_clients.get(&sid) {
                // Take only one request at a time — remaining stay in the queue
                if let Some((viewer_id, display_name)) = client.take_one_control_request().await {
                    // Auto-approve returning viewers who were previously granted RW
                    if client.is_previously_approved(&display_name) {
                        client.respond_control(&viewer_id, true).await;
                        self.pro.toast_notifications.push(ToastNotification {
                            message: format!("{} auto-approved (returning viewer)", display_name),
                            created_at: Instant::now(),
                            color: ratatui::style::Color::Green,
                        });
                        continue;
                    }

                    // Find session title
                    let title = self.sessions.iter()
                        .find(|s| s.id == sid)
                        .map(|s| s.title.clone())
                        .unwrap_or_else(|| sid.clone());

                    self.dialog = Some(Dialog::ControlRequest(ControlRequestDialog {
                        session_id: sid,
                        session_title: title,
                        viewer_id,
                        display_name,
                        created_at: Instant::now(),
                    }));
                    self.state = AppState::Dialog;
                    break;
                }
            }
        }
    }

    /// Detect viewer join/leave by comparing current viewers with last known state.
    #[cfg(feature = "pro")]
    pub(super) fn detect_viewer_changes(&mut self) {
        let session_ids: Vec<String> = self.pro.relay_clients.keys().cloned().collect();

        for sid in session_ids {
            let current_viewers: Vec<String> = self.pro.relay_clients.get(&sid)
                .map(|c| c.viewers().iter().map(|v| v.display_name.clone()).collect())
                .unwrap_or_default();

            let previous = self.pro.last_known_viewers.entry(sid.clone()).or_default();

            // Detect joins
            for name in &current_viewers {
                if !previous.contains(name) {
                    let session_title = self.sessions.iter()
                        .find(|s| s.id == sid)
                        .map(|s| s.title.as_str())
                        .unwrap_or(&sid);
                    self.pro.toast_notifications.push(ToastNotification {
                        message: format!("{} joined {}", name, session_title),
                        created_at: Instant::now(),
                        color: ratatui::style::Color::Green,
                    });
                }
            }

            // Detect leaves
            for name in previous.iter() {
                if !current_viewers.contains(name) {
                    let session_title = self.sessions.iter()
                        .find(|s| s.id == sid)
                        .map(|s| s.title.as_str())
                        .unwrap_or(&sid);
                    self.pro.toast_notifications.push(ToastNotification {
                        message: format!("{} left {}", name, session_title),
                        created_at: Instant::now(),
                        color: ratatui::style::Color::Yellow,
                    });
                }
            }

            *self.pro.last_known_viewers.entry(sid.clone()).or_default() = current_viewers;

            // Detect RW controller changes
            let current_controller: Option<String> = self.pro.relay_clients.get(&sid)
                .map(|c| c.viewers().iter()
                    .find(|v| v.permission == "rw")
                    .map(|v| v.display_name.clone()))
                .unwrap_or(None);

            let prev_controller = self.pro.last_known_controller.get(&sid).cloned().flatten();
            if prev_controller != current_controller {
                let session_title = self.sessions.iter()
                    .find(|s| s.id == sid)
                    .map(|s| s.title.as_str())
                    .unwrap_or(&sid);
                match (&prev_controller, &current_controller) {
                    (None, Some(name)) => {
                        self.pro.toast_notifications.push(ToastNotification {
                            message: format!("{} now controls {}", name, session_title),
                            created_at: Instant::now(),
                            color: ratatui::style::Color::Cyan,
                        });
                    }
                    (Some(prev), None) => {
                        self.pro.toast_notifications.push(ToastNotification {
                            message: format!("{} released control of {}", prev, session_title),
                            created_at: Instant::now(),
                            color: ratatui::style::Color::DarkGray,
                        });
                    }
                    (Some(prev), Some(name)) if prev != name => {
                        self.pro.toast_notifications.push(ToastNotification {
                            message: format!("Control of {} passed to {}", session_title, name),
                            created_at: Instant::now(),
                            color: ratatui::style::Color::Cyan,
                        });
                    }
                    _ => {}
                }
                self.pro.last_known_controller.insert(sid, current_controller);
            }
        }
    }
}
