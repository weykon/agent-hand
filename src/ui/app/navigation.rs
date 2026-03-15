use super::*;

impl App {

    pub(super) fn on_navigation(&mut self) {
        self.last_navigation_time = Instant::now();
        self.is_navigating = true;
        self.pending_preview_id = self.selected_session().map(|s| s.id.clone());
    }

    pub(super) async fn focus_session(&mut self, id: &str) -> Result<()> {
        let group_path = match self.session_by_id(id) {
            Some(s) => s.group_path.clone(),
            None => return Ok(()),
        };

        // Auto-expand groups so the session becomes visible
        if !group_path.is_empty() {
            let parts: Vec<&str> = group_path.split('/').collect();
            for i in 1..=parts.len() {
                let p = parts[..i].join("/");
                self.groups.set_expanded(&p, true);
            }

            let storage = self.storage.lock().await;
            storage.save(&self.sessions, &self.groups, &self.relationships).await?;
            drop(storage);
        }

        self.rebuild_tree();

        if let Some((idx, _)) = self.tree.iter().enumerate().find(|(_, item)| match item {
            TreeItem::Session { id: sid, .. } | TreeItem::Relationship { id: sid, .. } => sid == id,
            _ => false,
        }) {
            self.selected_index = idx;
            self.preview.clear();
            self.update_preview().await?;
        }

        Ok(())
    }

    pub(super) async fn focus_group(&mut self, path: &str) -> Result<()> {
        self.rebuild_tree();

        if let Some((idx, _)) = self.tree.iter().enumerate().find(|(_, item)| match item {
            TreeItem::Group { path: p, .. } => p == path,
            _ => false,
        }) {
            self.selected_index = idx;
            self.preview.clear();
            self.update_preview().await?;
        }

        Ok(())
    }

    /// Move selection up
    pub(super) fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    pub(super) fn move_selection_down(&mut self) {
        if self.tree.is_empty() {
            return;
        }
        if self.selected_index + 1 < self.tree.len() {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    /// Handle mouse events (scroll, click).
    pub(super) fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::{MouseEventKind, MouseButton};

        // Ignore mouse events when a dialog or help overlay is open
        if self.state == AppState::Dialog || self.help_visible {
            return;
        }

        // ViewerMode: tmux handles scroll natively — ignore mouse events here
        #[cfg(feature = "pro")]
        if self.state == AppState::ViewerMode {
            return;
        }

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.move_selection_up();
                self.on_navigation();
                self.enforce_scrolloff();
            }
            MouseEventKind::ScrollDown => {
                self.move_selection_down();
                self.on_navigation();
                self.enforce_scrolloff();
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_mouse_click(mouse.column, mouse.row);
            }
            _ => {}
        }
    }

    /// Map a mouse click (column, row) to the corresponding tree item index.
    ///
    /// Layout (vertical): Title (3 rows) | Content | StatusBar (3 rows).
    /// Content left panel is 45% width.
    /// The session tree has a 1-row border top, items, 1-row border bottom.
    pub(super) fn handle_mouse_click(&mut self, col: u16, row: u16) {
        // Layout: title=3, content=middle, status_bar=3
        let title_h: u16 = 3;
        let status_h: u16 = 3;
        let total = self.height;
        if total <= title_h + status_h {
            return;
        }
        let content_h = total - title_h - status_h;
        let content_top = title_h;

        // Session list occupies left 45% of content area
        let left_width = (self.width * 45) / 100;
        if col >= left_width {
            return; // Click is in preview pane
        }
        if row < content_top || row >= content_top + content_h {
            return; // Click is outside content area
        }

        // Pro: account for the active panel above the tree
        let tree_area_top;
        #[cfg(feature = "pro")]
        {
            let is_pro = self.auth_token.as_ref().map_or(false, |t| t.is_pro());
            let active_count = self.active_sessions().len();
            if is_pro && active_count > 0 {
                let max_panel_h = (content_h * 2 / 5).max(8);
                let panel_h = (active_count as u16 + 2).min(max_panel_h);
                tree_area_top = content_top + panel_h;
            } else {
                tree_area_top = content_top;
            }
        }
        #[cfg(not(feature = "pro"))]
        {
            tree_area_top = content_top;
        }

        if row < tree_area_top {
            return; // Click is in the active panel, not the tree
        }

        // Inside tree area: 1 row border top, then items
        let item_row = row.saturating_sub(tree_area_top + 1); // +1 for top border

        // Determine viewport offset (scroll position)
        let viewport_offset: usize;
        #[cfg(feature = "pro")]
        {
            viewport_offset = self.list_state.offset();
        }
        #[cfg(not(feature = "pro"))]
        {
            // Non-pro: ratatui auto-scrolls around selected; approximate offset
            let visible = self.height.saturating_sub(title_h + status_h + 2) as usize; // 2 for borders
            viewport_offset = if self.selected_index >= visible {
                self.selected_index.saturating_sub(visible / 2)
            } else {
                0
            };
        }

        let target_index = viewport_offset + item_row as usize;
        if target_index < self.tree.len() {
            self.selected_index = target_index;
            self.on_navigation();
            self.list_state.select(Some(target_index));
            self.enforce_scrolloff();
        }
    }

    /// Visible tree rows (total height minus header, status bar, borders)
    pub(super) fn visible_tree_height(&self) -> usize {
        self.height.saturating_sub(5) as usize
    }

    /// Jump cursor down (Ctrl+D)
    #[cfg(feature = "pro")]
    pub(super) fn move_half_page_down(&mut self) {
        let jump = self.pro.jump_lines.max(1);
        let max = self.tree.len().saturating_sub(1);
        self.selected_index = (self.selected_index + jump).min(max);
    }

    /// Jump cursor up (Ctrl+U)
    #[cfg(feature = "pro")]
    pub(super) fn move_half_page_up(&mut self) {
        let jump = self.pro.jump_lines.max(1);
        self.selected_index = self.selected_index.saturating_sub(jump);
    }

    /// Keep cursor ~SCROLLOFF lines from viewport edges (like vim `set scrolloff=5`)
    pub(super) fn enforce_scrolloff(&mut self) {
        const SCROLLOFF: usize = 10;
        let visible = self.visible_tree_height();
        if visible == 0 || self.tree.is_empty() {
            return;
        }

        let selected = self.selected_index;
        let offset = self.list_state.offset();

        // Cursor too close to top edge — scroll up
        if selected < offset + SCROLLOFF {
            let new_offset = selected.saturating_sub(SCROLLOFF);
            *self.list_state.offset_mut() = new_offset;
        }
        // Cursor too close to bottom edge — scroll down
        else if selected + SCROLLOFF >= offset + visible {
            let new_offset = (selected + SCROLLOFF + 1).saturating_sub(visible);
            let max_offset = self.tree.len().saturating_sub(visible);
            *self.list_state.offset_mut() = new_offset.min(max_offset);
        }

        self.list_state.select(Some(selected));
    }

    pub(super) fn selected_tree_item(&self) -> Option<&TreeItem> {
        self.tree.get(self.selected_index)
    }

    /// Get selected session (if selection is a session or relationship row)
    pub fn selected_session(&self) -> Option<&Instance> {
        let id = match self.selected_tree_item()? {
            TreeItem::Session { id, .. } => id,
            TreeItem::Relationship { id, .. } => id,
            _ => return None,
        };
        let &idx = self.sessions_by_id.get(id)?;
        self.sessions.get(idx)
    }

    /// Get the tmux session name for a session ID.
    /// Looks up the instance to use its stored tmux name, falling back to legacy format.
    pub(super) fn tmux_name_for_id(&self, id: &str) -> String {
        self.sessions_by_id
            .get(id)
            .and_then(|&idx| self.sessions.get(idx))
            .map(|s| s.tmux_name())
            .unwrap_or_else(|| TmuxManager::session_name_legacy(id))
    }

    pub(super) fn priority_session_id(&self) -> Option<String> {
        // Priority 1: Waiting (!) — needs user input, newest first.
        if let Some(s) = self
            .sessions
            .iter()
            .filter(|s| s.status == Status::Waiting)
            .max_by_key(|s| s.last_waiting_at.unwrap_or(s.created_at))
        {
            return Some(s.id.clone());
        }

        // Priority 2: Recently-idle with attention (✓) — just finished, newest first.
        if let Some(s) = self
            .sessions
            .iter()
            .filter(|s| s.status == Status::Idle && self.is_attention_active(&s.id))
            .max_by_key(|s| s.last_running_at.unwrap_or(s.created_at))
        {
            return Some(s.id.clone());
        }

        // Priority 3: Running sessions — currently active, newest first.
        self.sessions
            .iter()
            .filter(|s| s.status == Status::Running)
            .max_by_key(|s| s.last_running_at.unwrap_or(s.created_at))
            .map(|s| s.id.clone())
    }

    pub(super) async fn queue_attach_by_id(&mut self, id: &str) -> Result<()> {
        if let Some(pos) = self
            .tree
            .iter()
            .position(|item| matches!(item, TreeItem::Session { id: sid, .. } | TreeItem::Relationship { id: sid, .. } if sid == id))
        {
            self.selected_index = pos;
            self.on_navigation();
            self.preview.clear();
        }

        // Look up session — try index map first, fall back to linear scan
        // in case the index map is momentarily stale.
        let idx = if let Some(&i) = self.sessions_by_id.get(id) {
            i
        } else if let Some(i) = self.sessions.iter().position(|s| s.id == id) {
            i
        } else {
            return Ok(());
        };
        let session = self.sessions[idx].clone();

        let tmux_session = session.tmux_name();
        // Remember whether the session already existed before we try to create it.
        // `session_exists` can return Err on transient tmux failures; treat that
        // conservatively for creation (assume doesn't exist → try to create) but
        // optimistically for attach (see below).
        let existed_before = self.tmux.session_exists(&tmux_session).unwrap_or(false);
        if !existed_before {
            // Prefer resume if session has a stored CLI session ID
            let resume_cmd = session
                .cli_session_id()
                .and_then(|sid| {
                    self.build_resume_command_for_session(&session, sid).ok()
                });

            let cmd = resume_cmd.as_deref().or_else(|| {
                let c = session.command.as_str();
                if c.trim().is_empty() { None } else { Some(c) }
            });

            let _ = self
                .tmux
                .create_session(
                    &tmux_session,
                    &session.project_path.to_string_lossy(),
                    cmd,
                    Some(&session.title),
                )
                .await;
        }

        // Proceed with attach if the session existed before OR exists now.
        // This avoids a transient `session_exists` error after creation
        // blocking the attach for sessions that were already running.
        if existed_before || self.tmux.session_exists(&tmux_session).unwrap_or(false) {
            // Ensure the friendly title is stamped (covers pre-existing sessions too).
            let _ = self.tmux.set_session_title(&tmux_session, &session.title).await;
            self.pending_attach = Some(tmux_session);
        }
        Ok(())
    }

    /// Jump tree selection to the session with the given ID (without attaching).
    #[cfg(feature = "pro")]
    pub(super) fn focus_tree_on_session_id(&mut self, id: &str) {
        if let Some(pos) = self
            .tree
            .iter()
            .position(|item| matches!(item, TreeItem::Session { id: sid, .. } | TreeItem::Relationship { id: sid, .. } if sid == id))
        {
            self.selected_index = pos;
            self.enforce_scrolloff();
            self.on_navigation();
            self.preview.clear();
        }
    }

    /// Find session by tmux session name (matches against each session's tmux_name())
    pub(super) fn find_session_by_tmux_name(&self, tmux_name: &str) -> Option<Instance> {
        self.sessions
            .iter()
            .find(|s| s.tmux_name() == tmux_name)
            .cloned()
    }

    /// Queue attach to selected session (performed in event loop)
    pub(super) async fn queue_attach_selected(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let tmux_session = session.tmux_name();
            let title = session.title.clone();

            if !self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                self.start_selected().await?;
            }

            if self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                let _ = self.tmux.set_session_title(&tmux_session, &title).await;
                self.pending_attach = Some(tmux_session);
            }
        }
        Ok(())
    }

    pub(super) async fn perform_attach(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        name: &str,
    ) -> Result<()> {
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

        // Mark the attached session so the background sound task can respect quiet_when_focused
        #[cfg(feature = "pro")]
        if let Ok(mut g) = self.attached_session.write() {
            *g = Some(name.to_string());
        }

        let attach_result = self.tmux.attach_session(name).await;

        // Clear the attached session — user is back on dashboard
        #[cfg(feature = "pro")]
        if let Ok(mut g) = self.attached_session.write() {
            *g = None;
        }

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

        attach_result
    }
}
