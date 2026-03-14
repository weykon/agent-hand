use super::*;

impl App {

    /// Handle keyboard input
    pub(super) async fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        // Any key during startup skips the logo animation
        if self.state == AppState::Startup {
            self.state = AppState::Normal;
            self.startup_started_at = None;
            self.startup_phase = crate::ui::transition::StartupPhase::Done;
            return Ok(());
        }

        match self.state {
            AppState::Startup => unreachable!(), // handled above
            AppState::Normal => self.handle_normal_key(key, modifiers).await,
            AppState::Search => self.handle_search_key(key, modifiers).await,
            AppState::Dialog => self.handle_dialog_key(key, modifiers).await,
            AppState::Help => self.handle_help_key(key),
            #[cfg(feature = "pro")]
            AppState::Relationships => self.handle_relationships_key(key, modifiers).await,
            #[cfg(feature = "pro")]
            AppState::ViewerMode => self.handle_viewer_key(key, modifiers).await,
            AppState::Chat => self.handle_chat_key(key, modifiers).await,
        }
    }

    /// Handle keys in normal mode
    pub(super) async fn handle_normal_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        // Handle onboarding welcome screen
        if self.show_onboarding {
            if key == KeyCode::Enter && modifiers == KeyModifiers::NONE {
                self.dismiss_onboarding();
                // Save first_launch = false to config
                self.config.first_launch = Some(false);
                let _ = self.config.save();
                return Ok(());
            }
            // Block all other keys while onboarding is shown
            return Ok(());
        }

        // Dismiss AI summary overlay: Esc closes, 'C' adds to canvas, j/k scroll, 'A' reopens picker
        #[cfg(feature = "max")]
        if self.max.show_ai_summary_overlay {
            match key {
                KeyCode::Esc => {
                    self.max.show_ai_summary_overlay = false;
                    self.max.summary_overlay_scroll = 0;
                    return Ok(());
                }
                // 'C' key: add summary to canvas
                KeyCode::Char('C') | KeyCode::Char('c') => {
                    if let Some(id) = self.max.ai_summary_overlay_id.clone() {
                        self.add_summary_to_canvas(&id);
                    }
                    self.max.show_ai_summary_overlay = false;
                    self.max.summary_overlay_scroll = 0;
                    return Ok(());
                }
                // Scroll down
                KeyCode::Down | KeyCode::Char('j') => {
                    self.max.summary_overlay_scroll = self.max.summary_overlay_scroll.saturating_add(1);
                    return Ok(());
                }
                // Scroll up
                KeyCode::Up | KeyCode::Char('k') => {
                    self.max.summary_overlay_scroll = self.max.summary_overlay_scroll.saturating_sub(1);
                    return Ok(());
                }
                // Page down
                KeyCode::PageDown | KeyCode::Char('d') => {
                    self.max.summary_overlay_scroll = self.max.summary_overlay_scroll.saturating_add(10);
                    return Ok(());
                }
                // Page up
                KeyCode::PageUp | KeyCode::Char('u') => {
                    self.max.summary_overlay_scroll = self.max.summary_overlay_scroll.saturating_sub(10);
                    return Ok(());
                }
                _ => {
                    // 'A' key: let it fall through to reopen the picker
                    if !self.keybindings.matches("summarize", &key, modifiers) {
                        self.max.show_ai_summary_overlay = false;
                        self.max.summary_overlay_scroll = 0;
                    }
                }
            }
        }

        // Dismiss AI diagram overlay: Esc closes, 'C' adds to canvas, j/k scroll, 'A' reopens picker
        #[cfg(feature = "max")]
        if self.max.show_ai_diagram_overlay {
            match key {
                KeyCode::Esc => {
                    self.max.show_ai_diagram_overlay = false;
                    self.max.diagram_overlay_scroll = 0;
                    return Ok(());
                }
                // 'C' key: add diagram to canvas
                KeyCode::Char('C') | KeyCode::Char('c') => {
                    if let Some(id) = self.max.ai_diagram_overlay_id.clone() {
                        self.add_diagram_to_canvas(&id);
                    }
                    self.max.show_ai_diagram_overlay = false;
                    self.max.diagram_overlay_scroll = 0;
                    return Ok(());
                }
                // Scroll down
                KeyCode::Down | KeyCode::Char('j') => {
                    self.max.diagram_overlay_scroll = self.max.diagram_overlay_scroll.saturating_add(1);
                    return Ok(());
                }
                // Scroll up
                KeyCode::Up | KeyCode::Char('k') => {
                    self.max.diagram_overlay_scroll = self.max.diagram_overlay_scroll.saturating_sub(1);
                    return Ok(());
                }
                // Page down
                KeyCode::PageDown | KeyCode::Char('d') => {
                    self.max.diagram_overlay_scroll = self.max.diagram_overlay_scroll.saturating_add(10);
                    return Ok(());
                }
                // Page up
                KeyCode::PageUp | KeyCode::Char('u') => {
                    self.max.diagram_overlay_scroll = self.max.diagram_overlay_scroll.saturating_sub(10);
                    return Ok(());
                }
                _ => {
                    // 'A' key: let it fall through to reopen the picker
                    if !self.keybindings.matches("summarize", &key, modifiers) {
                        self.max.show_ai_diagram_overlay = false;
                        self.max.diagram_overlay_scroll = 0;
                    }
                }
            }
        }

        // Dismiss behavior analysis overlay: Esc closes, j/k scroll, 'B' falls through
        #[cfg(feature = "max")]
        if self.max.show_behavior_overlay {
            match key {
                KeyCode::Esc => {
                    self.max.show_behavior_overlay = false;
                    self.max.behavior_overlay_scroll = 0;
                    return Ok(());
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.max.behavior_overlay_scroll = self.max.behavior_overlay_scroll.saturating_add(1);
                    return Ok(());
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.max.behavior_overlay_scroll = self.max.behavior_overlay_scroll.saturating_sub(1);
                    return Ok(());
                }
                KeyCode::PageDown | KeyCode::Char('d') => {
                    self.max.behavior_overlay_scroll = self.max.behavior_overlay_scroll.saturating_add(10);
                    return Ok(());
                }
                KeyCode::PageUp | KeyCode::Char('u') => {
                    self.max.behavior_overlay_scroll = self.max.behavior_overlay_scroll.saturating_sub(10);
                    return Ok(());
                }
                _ => {
                    if !self.keybindings.matches("behavior_analysis", &key, modifiers) {
                        self.max.show_behavior_overlay = false;
                        self.max.behavior_overlay_scroll = 0;
                    }
                }
            }
        }

        if self.keybindings.matches("quit", &key, modifiers) {
            self.transition_engine.request_transition();
            self.dialog = Some(Dialog::QuitConfirm);
            self.state = AppState::Dialog;
            return Ok(());
        }

        if self.keybindings.matches("settings", &key, modifiers) {
            self.transition_engine.request_transition();
            self.dialog = Some(Dialog::Settings(SettingsDialog::new(&self.config, &self.keybindings)));
            self.state = AppState::Dialog;
            return Ok(());
        }

        // Tab: cycle panel focus (Pro: tree → active → viewer → tree)
        // Canvas is only toggled via 'p' (canvas_toggle keybinding), not Tab.
        #[cfg(feature = "pro")]
        if key == KeyCode::Tab && modifiers == KeyModifiers::NONE {
            self.transition_engine.request_transition();
            let is_pro = self.auth_token.as_ref().map_or(false, |t| t.is_pro());
            let active_count = self.active_sessions().len();
            let viewer_count = self.pro.viewer_sessions.len();

            if self.canvas_focused {
                // If user is in canvas and presses Tab, just leave canvas → tree
                self.canvas_focused = false;
                return Ok(());
            }

            if is_pro && (active_count > 0 || viewer_count > 0) {
                if self.active_panel_focused {
                    // active → viewer (if has viewers) or tree
                    if viewer_count > 0 {
                        self.active_panel_focused = false;
                        self.pro.viewer_panel_focused = true;
                        if self.pro.viewer_panel_selected >= viewer_count {
                            self.pro.viewer_panel_selected = viewer_count.saturating_sub(1);
                        }
                    } else {
                        let active = self.active_sessions();
                        if let Some(session) = active.get(self.active_panel_selected) {
                            let id = session.id.clone();
                            self.active_panel_focused = false;
                            self.focus_tree_on_session_id(&id);
                        } else {
                            self.active_panel_focused = false;
                        }
                    }
                } else if self.pro.viewer_panel_focused {
                    self.pro.viewer_panel_focused = false;
                } else {
                    // tree → active (if has active sessions)
                    if active_count > 0 {
                        self.active_panel_focused = true;
                        if self.active_panel_selected >= active_count {
                            self.active_panel_selected = active_count.saturating_sub(1);
                        }
                    } else if viewer_count > 0 {
                        self.pro.viewer_panel_focused = true;
                        if self.pro.viewer_panel_selected >= viewer_count {
                            self.pro.viewer_panel_selected = viewer_count.saturating_sub(1);
                        }
                    }
                }
                return Ok(());
            }

            // Pro but no active/viewer sessions: Tab does nothing (use 'p' for canvas)
            return Ok(());
        }

        // When canvas is focused, route keys to canvas input handler (Pro only)
        #[cfg(feature = "pro")]
        if self.canvas_focused {
            // p: toggle back to tree (session list)
            if self.keybindings.matches("canvas_toggle", &key, modifiers) {
                self.canvas_focused = false;
                return Ok(());
            }
            // Enter on a session-linked node: jump to that session in the tree
            // (but not during edit mode — Enter commits the edit instead)
            if key == KeyCode::Enter && modifiers == KeyModifiers::NONE && !self.canvas_state.is_editing() {
                if let Some(sid) = self.canvas_state.session_id_at_cursor() {
                    self.canvas_focused = false;
                    if let Some(pos) = self.tree.iter().position(|item| {
                        matches!(item, crate::ui::TreeItem::Session { id, .. } | crate::ui::TreeItem::Relationship { id, .. } if id == &sid)
                    }) {
                        self.selected_index = pos;
                        self.on_navigation();
                        self.preview.clear();
                    }
                    return Ok(());
                }
                // Not a session node — fall through to normal canvas Enter (toggle select)
            }
            // When a relationship edge is selected, intercept ops before canvas handler
            if let Some(rel_id) = self.canvas_state.selected_edge_relationship_id().map(|s| s.to_string()) {
                match (key, modifiers) {
                    // c: capture context for this relationship
                    (KeyCode::Char('c'), KeyModifiers::NONE) => {
                        if self.auth_token.as_ref().is_some_and(|t| t.is_pro()) {
                            self.capture_relationship_context(rel_id).await?;
                        }
                        return Ok(());
                    }
                    // a: annotate relationship
                    (KeyCode::Char('a'), KeyModifiers::NONE) => {
                        if self.auth_token.as_ref().is_some_and(|t| t.is_pro()) {
                            let dialog = crate::ui::AnnotateDialog {
                                relationship_id: rel_id,
                                note: crate::ui::TextInput::new(),
                            };
                            self.dialog = Some(Dialog::Annotate(dialog));
                            self.state = AppState::Dialog;
                        }
                        return Ok(());
                    }
                    // d: delete this relationship (and its edge + workspace session)
                    (KeyCode::Char('d'), KeyModifiers::NONE) => {
                        // Delete associated relationship workspace sessions
                        let rel_id_clone = rel_id.clone();
                        let workspace_ids: Vec<String> = self.sessions.iter()
                            .filter(|s| s.relationship_id.as_deref() == Some(&rel_id_clone))
                            .map(|s| s.id.clone())
                            .collect();
                        for ws_id in workspace_ids {
                            let _ = self.delete_session(&ws_id, true).await;
                        }

                        crate::session::relationships::remove_relationship(
                            &mut self.relationships,
                            &rel_id,
                        );
                        self.canvas_state.selected_edge = None;
                        self.canvas_state.sync_relationship_edges(&self.relationships);
                        let storage = self.storage.lock().await;
                        storage.save(&self.sessions, &self.groups, &self.relationships).await?;
                        drop(storage);
                        self.refresh_sessions().await?;
                        return Ok(());
                    }
                    // Ctrl+N: new session from context
                    (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                        if self.auth_token.as_ref().is_some_and(|t| t.is_pro()) {
                            if let Some(rel) = self.relationships.iter().find(|r| r.id == rel_id).cloned() {
                                let profile = self.storage.lock().await.profile().to_string();
                                let collector = crate::pro::context::ContextCollector::new(&profile);
                                let a_title = self.session_by_id(&rel.session_a_id)
                                    .map(|s| s.title.clone())
                                    .unwrap_or_default();
                                let b_title = self.session_by_id(&rel.session_b_id)
                                    .map(|s| s.title.clone())
                                    .unwrap_or_default();
                                let context = collector.build_relationship_context(
                                    &rel.id,
                                    rel.label.as_deref(),
                                    &a_title,
                                    &b_title,
                                ).await.unwrap_or_default();
                                let dialog = crate::ui::NewFromContextDialog {
                                    relationship_id: rel_id,
                                    context_preview: context,
                                    title: TextInput::new(),
                                    injection_method: crate::ui::ContextInjectionMethod::InitialPrompt,
                                };
                                self.dialog = Some(Dialog::NewFromContext(dialog));
                                self.state = AppState::Dialog;
                            }
                        }
                        return Ok(());
                    }
                    _ => {} // Fall through to normal canvas handling
                }
            }

            // View switching keys (1-4, Tab, BackTab) — only when not editing/adding
            if !self.canvas_state.is_editing() && !self.canvas_state.adding_node {
                use crate::ui::canvas::CanvasView;
                match (key, modifiers) {
                    (KeyCode::Char('1'), KeyModifiers::NONE) => {
                        self.transition_engine.request_transition();
                        self.switch_canvas_view(CanvasView::User);
                        return Ok(());
                    }
                    (KeyCode::Char('2'), KeyModifiers::NONE) => {
                        self.transition_engine.request_transition();
                        self.switch_canvas_view(CanvasView::Agent);
                        return Ok(());
                    }
                    (KeyCode::Tab, KeyModifiers::NONE) if self.canvas_state.is_projection_view() => {
                        self.transition_engine.request_transition();
                        self.switch_canvas_view(self.canvas_state.current_view.next());
                        return Ok(());
                    }
                    (KeyCode::BackTab, _) if self.canvas_state.is_projection_view() => {
                        self.transition_engine.request_transition();
                        self.switch_canvas_view(self.canvas_state.current_view.prev());
                        return Ok(());
                    }
                    // Cycle agent namespaces with [ and ]
                    (KeyCode::Char('['), KeyModifiers::NONE) if self.canvas_state.current_view == CanvasView::Agent => {
                        self.canvas_state.prev_namespace();
                        return Ok(());
                    }
                    (KeyCode::Char(']'), KeyModifiers::NONE) if self.canvas_state.current_view == CanvasView::Agent => {
                        self.canvas_state.next_namespace();
                        return Ok(());
                    }
                    _ => {}
                }

                // Enter on a scheduler review node → open human review dialog
                // Enter on a scheduler followup node → open proposal action dialog
                if key == KeyCode::Enter
                    && modifiers == KeyModifiers::NONE
                    && self.canvas_state.current_view == CanvasView::Agent
                {
                    if let Some(node_id) = self.canvas_state.node_id_at_cursor() {
                        if node_id.starts_with("sched_review_") {
                            if let Some(rec) = self.find_review_record(&node_id) {
                                let dialog = crate::ui::HumanReviewDialog {
                                    record_id: rec.id.clone(),
                                    reason: rec.reason.clone(),
                                    source_session_id: rec.source_session_id.clone(),
                                    urgency: format!("{:?}", rec.urgency_level),
                                    targets: rec.target_session_ids.clone(),
                                    created_at: std::time::Instant::now(),
                                };
                                self.dialog = Some(Dialog::HumanReview(dialog));
                                self.state = AppState::Dialog;
                                return Ok(());
                            }
                        } else if node_id.starts_with("sched_followup_") {
                            if let Some((rec, status)) = self.find_proposal_record(&node_id) {
                                let dialog = crate::ui::ProposalActionDialog {
                                    proposal_id: rec.id.clone(),
                                    reason: rec.reason.clone(),
                                    source_session_id: rec.source_session_id.clone(),
                                    urgency: format!("{:?}", rec.urgency_level),
                                    targets: rec.target_session_ids.clone(),
                                    current_status: status,
                                    created_at: std::time::Instant::now(),
                                };
                                self.dialog = Some(Dialog::ProposalAction(dialog));
                                self.state = AppState::Dialog;
                                return Ok(());
                            }
                        }
                    }
                }

                // Enter on a WASM plugin node → dispatch click event to WASM plugin
                #[cfg(feature = "wasm")]
                if key == KeyCode::Enter
                    && modifiers == KeyModifiers::NONE
                    && self.canvas_state.current_view == CanvasView::Agent
                {
                    if let Some(node_id) = self.canvas_state.node_id_at_cursor() {
                        if node_id.starts_with("wasm_") {
                            self.dispatch_wasm_canvas_event("node_click", Some(node_id));
                            return Ok(());
                        }
                    }
                }

                // In projection views, block editing/adding operations
                if self.canvas_state.is_projection_view() {
                    match key {
                        KeyCode::Char('a') | KeyCode::Char('d') | KeyCode::Char('e')
                        | KeyCode::Char('c') | KeyCode::Char('x') => {
                            // Silently ignore edit operations in projection views
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }

            // Use approximate canvas panel dimensions (55% width, full height minus borders)
            let canvas_cols = (self.width * 55 / 100).saturating_sub(2);
            let canvas_rows = self.height.saturating_sub(5);

            // Esc unfocuses canvas if nothing to cancel inside
            if key == KeyCode::Esc && modifiers == KeyModifiers::NONE {
                let consumed = crate::ui::canvas::input::handle_canvas_input(
                    &mut self.canvas_state,
                    key,
                    modifiers,
                    canvas_cols,
                    canvas_rows,
                );
                if !consumed {
                    self.canvas_focused = false;
                }
                return Ok(());
            }
            let consumed = crate::ui::canvas::input::handle_canvas_input(
                &mut self.canvas_state,
                key,
                modifiers,
                canvas_cols,
                canvas_rows,
            );
            if consumed {
                return Ok(());
            }
            // If not consumed, fall through to normal handling
        }

        // When active panel is focused, intercept navigation keys
        #[cfg(feature = "pro")]
        if self.active_panel_focused {
            let active: Vec<String> = self.active_sessions()
                .iter()
                .map(|s| s.id.clone())
                .collect();

            // If no active sessions remain, defocus the panel
            if active.is_empty() {
                self.active_panel_focused = false;
            } else {
                match key {
                    KeyCode::Up | KeyCode::Char('k') if modifiers == KeyModifiers::NONE => {
                        self.active_panel_selected = self.active_panel_selected.saturating_sub(1);
                        return Ok(());
                    }
                    KeyCode::Down | KeyCode::Char('j') if modifiers == KeyModifiers::NONE => {
                        let max = active.len().saturating_sub(1);
                        if self.active_panel_selected >= max {
                            self.active_panel_focused = false;
                            self.selected_index = 0;
                            self.enforce_scrolloff();
                            self.on_navigation();
                            self.preview.clear();
                        } else {
                            self.active_panel_selected += 1;
                        }
                        return Ok(());
                    }
                    KeyCode::Right if modifiers == KeyModifiers::NONE => {
                        if let Some(id) = active.get(self.active_panel_selected) {
                            let id = id.clone();
                            self.active_panel_focused = false;
                            self.focus_tree_on_session_id(&id);
                        }
                        return Ok(());
                    }
                    KeyCode::Enter if modifiers == KeyModifiers::NONE => {
                        if let Some(id) = active.get(self.active_panel_selected) {
                            let id = id.clone();
                            self.active_panel_focused = false;
                            self.last_attach_source = Some(super::AttachSource::ActivePanel);
                            self.queue_attach_by_id(&id).await?;
                        }
                        return Ok(());
                    }
                    KeyCode::Esc | KeyCode::Tab if modifiers == KeyModifiers::NONE => {
                        self.active_panel_focused = false;
                        return Ok(());
                    }
                    _ => {}
                }
                // Swallow all other keys while panel is focused
                return Ok(());
            }
        }

        // When viewer panel is focused, intercept navigation keys
        #[cfg(feature = "pro")]
        if self.pro.viewer_panel_focused {
            let viewer_count = self.pro.viewer_sessions.len();

            // If no viewer sessions remain, defocus the panel
            if viewer_count == 0 {
                self.pro.viewer_panel_focused = false;
            } else {
                match key {
                    KeyCode::Up | KeyCode::Char('k') if modifiers == KeyModifiers::NONE => {
                        if self.pro.viewer_panel_selected > 0 {
                            self.pro.viewer_panel_selected -= 1;
                        }
                        return Ok(());
                    }
                    KeyCode::Down | KeyCode::Char('j') if modifiers == KeyModifiers::NONE => {
                        let max = viewer_count.saturating_sub(1);
                        if self.pro.viewer_panel_selected < max {
                            self.pro.viewer_panel_selected += 1;
                        }
                        return Ok(());
                    }
                    KeyCode::Char('d') if modifiers == KeyModifiers::NONE => {
                        if let Some(room_id) = self.get_selected_viewer_session() {
                            if let Some(session_info) = self.pro.viewer_sessions.get(&room_id) {
                                let dialog = crate::ui::dialogs::DisconnectViewerDialog::new(
                                    session_info.room_id.clone(),
                                    session_info.relay_url.clone(),
                                );
                                self.dialog = Some(Dialog::DisconnectViewer(dialog));
                                self.state = AppState::Dialog;
                            }
                        }
                        return Ok(());
                    }
                    KeyCode::Enter if modifiers == KeyModifiers::NONE => {
                        if let Some(room_id) = self.get_selected_viewer_session() {
                            if let Some(session_info) = self.pro.viewer_sessions.get(&room_id) {
                                match session_info.status {
                                    ViewerSessionStatus::Connected => {
                                        self.state = AppState::ViewerMode;
                                    }
                                    ViewerSessionStatus::Disconnected => {
                                        if let Err(e) = self.reconnect_viewer(&room_id).await {
                                            eprintln!("Reconnect failed: {}", e);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        return Ok(());
                    }
                    KeyCode::Esc | KeyCode::Tab if modifiers == KeyModifiers::NONE => {
                        self.pro.viewer_panel_focused = false;
                        return Ok(());
                    }
                    _ => {}
                }
                // Swallow all other keys while panel is focused
                return Ok(());
            }
        }

        // Navigation
        if self.keybindings.matches("up", &key, modifiers) {
            #[cfg(feature = "pro")]
            {
                if self.selected_index == 0 {
                    let is_pro = self.auth_token.as_ref().map_or(false, |t| t.is_pro());
                    let active_count = self.active_sessions().len();
                    if is_pro && active_count > 0 {
                        self.active_panel_focused = true;
                        self.active_panel_selected = active_count.saturating_sub(1);
                        return Ok(());
                    }
                }
            }
            self.move_selection_up();
            self.enforce_scrolloff();
            self.on_navigation();
            self.preview.clear();
            return Ok(());
        }
        if self.keybindings.matches("down", &key, modifiers) {
            self.move_selection_down();
            self.enforce_scrolloff();
            self.on_navigation();
            self.preview.clear();
            return Ok(());
        }

        #[cfg(feature = "pro")]
        {
            if self.keybindings.matches("half_page_down", &key, modifiers) {
                self.move_half_page_down();
                self.enforce_scrolloff();
                self.on_navigation();
                self.preview.clear();
                return Ok(());
            }
            if self.keybindings.matches("half_page_up", &key, modifiers) {
                self.move_half_page_up();
                self.enforce_scrolloff();
                self.on_navigation();
                self.preview.clear();
                return Ok(());
            }
        }

        if self.keybindings.matches("jump_priority", &key, modifiers) {
            if let Some(id) = self.priority_session_id() {
                // Preserve current panel context for return
                self.last_attach_source = if self.active_panel_focused {
                    Some(super::AttachSource::ActivePanel)
                } else {
                    Some(super::AttachSource::TreePanel)
                };
                self.queue_attach_by_id(&id).await?;
            }
            return Ok(());
        }

        // Actions
        if self.keybindings.matches("select", &key, modifiers) {
            if self.toggle_selected_group(None).await? {
                self.preview.clear();
            } else {
                self.last_attach_source = Some(super::AttachSource::TreePanel);
                self.queue_attach_selected().await?;
            }
            return Ok(());
        }
        if self.keybindings.matches("collapse", &key, modifiers) {
            let _ = self.toggle_selected_group(Some(false)).await?;
            return Ok(());
        }
        if self.keybindings.matches("expand", &key, modifiers) {
            let _ = self.toggle_selected_group(Some(true)).await?;
            return Ok(());
        }
        if self.keybindings.matches("toggle_group", &key, modifiers) {
            let _ = self.toggle_selected_group(None).await?;
            return Ok(());
        }
        if self.keybindings.matches("start", &key, modifiers) {
            self.activity.push_default(super::activity::ActivityOp::StartingSession);
            self.start_selected().await?;
            self.activity.complete(super::activity::ActivityOp::StartingSession);
            return Ok(());
        }
        if self.keybindings.matches("stop", &key, modifiers) {
            self.activity.push_default(super::activity::ActivityOp::KillingSession);
            self.stop_selected().await?;
            self.activity.complete(super::activity::ActivityOp::KillingSession);
            return Ok(());
        }
        if self.keybindings.matches("refresh", &key, modifiers) {
            self.activity.push_default(super::activity::ActivityOp::RefreshingSessions);
            self.refresh_sessions().await?;
            self.activity.complete(super::activity::ActivityOp::RefreshingSessions);
            return Ok(());
        }
        if self.keybindings.matches("rename", &key, modifiers) {
            if matches!(self.selected_tree_item(), Some(TreeItem::Group { .. })) {
                self.open_rename_group_dialog();
            } else if self.selected_session().is_some() {
                self.open_rename_session_dialog();
            }
            return Ok(());
        }

        // Boost: manually mark a session as "active" for attention_ttl duration
        if self.keybindings.matches("boost", &key, modifiers) {
            if let Some(session) = self.selected_session() {
                let id = session.id.clone();
                if let Some(&idx) = self.sessions_by_id.get(&id) {
                    self.sessions[idx].last_running_at = Some(chrono::Utc::now());
                    // Persist the change
                    let storage = self.storage.lock().await;
                    let _ = storage.save(&self.sessions, &self.groups, &self.relationships).await;
                }
            }
            return Ok(());
        }

        // AI Analysis (Max tier) — 'A' key
        // Opens a picker dialog to choose analysis mode (Summary or ASCII Diagram).
        // If overlay is visible, dismiss it first.
        #[cfg(feature = "max")]
        if self.keybindings.matches("summarize", &key, modifiers) {
            // Dismiss any visible overlay first
            self.max.show_ai_summary_overlay = false;
            self.max.show_ai_diagram_overlay = false;

            if self.max.summarizer.is_some() {
                self.open_ai_analysis_dialog();
            } else {
                self.preview = "AI Analysis requires Max subscription.\nVisit https://weykon.github.io/agent-hand".to_string();
            }
            return Ok(());
        }

        // Behavior Analysis (Max tier) — 'B' key
        // Opens a dialog to analyze the user's prompt patterns for the selected session.
        #[cfg(feature = "max")]
        if self.keybindings.matches("behavior_analysis", &key, modifiers) {
            // Dismiss any visible overlay first
            if self.max.show_behavior_overlay {
                self.max.show_behavior_overlay = false;
                return Ok(());
            }

            if self.max.summarizer.is_none() {
                self.preview = "Behavior Analysis requires Max subscription + AI provider.\nConfigure in Settings > AI.".to_string();
                return Ok(());
            }

            if let Some(session) = self.selected_session() {
                let sid = session.id.clone();
                let title = session.title.clone();
                let count = self.collected_prompts_count(&sid);
                if count == 0 {
                    self.preview = if self.prompt_collection_enabled() {
                        "No prompts collected yet for this session.\nSubmit some prompts first, then try again.".to_string()
                    } else {
                        "Prompt collection is disabled.\nEnable it in Settings > General > Prompt Collection.".to_string()
                    };
                    return Ok(());
                }
                self.dialog = Some(Dialog::BehaviorAnalysis(
                    BehaviorAnalysisDialog::new(sid, title, count),
                ));
                self.state = AppState::Dialog;
            } else {
                self.preview = "No session selected.".to_string();
            }
            return Ok(());
        }

        if self.keybindings.matches("new_session", &key, modifiers) {
            let default_path = std::env::current_dir()?;

            let default_group = match self.selected_tree_item() {
                Some(TreeItem::Group { path, .. }) => path.clone(),
                _ => self
                    .selected_session()
                    .map(|s| s.group_path.clone())
                    .unwrap_or_default(),
            };

            let mut all_groups: Vec<String> = self
                .groups
                .all_groups()
                .into_iter()
                .map(|g| g.path)
                .collect();
            all_groups.sort();
            all_groups.dedup();
            all_groups.insert(0, String::new());

            self.dialog = Some(Dialog::NewSession(NewSessionDialog::new(
                default_path,
                default_group,
                all_groups,
            )));
            self.state = AppState::Dialog;
            return Ok(());
        }

        if self.keybindings.matches("delete", &key, modifiers) {
            if let Some(session) = self.selected_session() {
                self.dialog = Some(Dialog::DeleteConfirm(DeleteConfirmDialog {
                    session_id: session.id.clone(),
                    title: session.title.clone(),
                    kill_tmux: true,
                }));
                self.state = AppState::Dialog;
            } else if let Some(TreeItem::Group { path, .. }) = self.selected_tree_item() {
                let path = path.clone();
                let session_ids = self.group_session_ids(&path);
                if session_ids.is_empty() {
                    self.apply_delete_group_prefix(&path).await?;
                    self.refresh_sessions().await?;
                } else {
                    self.dialog = Some(Dialog::DeleteGroup(DeleteGroupDialog {
                        group_path: path,
                        session_count: session_ids.len(),
                        choice: DeleteGroupChoice::DeleteGroupKeepSessions,
                    }));
                    self.state = AppState::Dialog;
                }
            }
            return Ok(());
        }

        if self.keybindings.matches("fork", &key, modifiers) {
            if self.selected_session().is_some() {
                self.open_fork_dialog();
            }
            return Ok(());
        }

        if self.keybindings.matches("create_group", &key, modifiers) {
            self.open_create_group_dialog();
            return Ok(());
        }

        if self.keybindings.matches("move", &key, modifiers) {
            if self.selected_session().is_some() {
                self.open_move_group_dialog();
            }
            return Ok(());
        }

        if self.keybindings.matches("tag", &key, modifiers) {
            if self.selected_session().is_some() {
                self.open_tag_picker_dialog();
            }
            return Ok(());
        }

        // a: add selected session to canvas (Pro only)
        // Only adds if the session belongs to the current canvas group.
        #[cfg(feature = "pro")]
        if self.keybindings.matches("add_to_canvas", &key, modifiers) {
            if let Some(crate::ui::TreeItem::Session { ref id, .. } | crate::ui::TreeItem::Relationship { ref id, .. }) = self.tree.get(self.selected_index) {
                let id = id.clone();
                if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
                    if session.group_path == self.pro.canvas_group {
                        let status_str = format!("{:?}", session.status).to_lowercase();
                        self.canvas_state.add_session_node(&session.id, &session.title, &status_str);
                    }
                }
            }
            return Ok(());
        }

        // p: Pro = toggle canvas focus, Free = refresh preview
        #[cfg(feature = "pro")]
        if self.keybindings.matches("canvas_toggle", &key, modifiers) {
            self.canvas_focused = !self.canvas_focused;
            return Ok(());
        }
        #[cfg(not(feature = "pro"))]
        if self.keybindings.matches("canvas_toggle", &key, modifiers) {
            self.refresh_preview_cache_selected().await?;
            return Ok(());
        }

        if self.keybindings.matches("search", &key, modifiers) {
            self.state = AppState::Search;
            self.search_query.clear();
            self.search_results.clear();
            self.search_selected = 0;
            self.update_search_results();
            return Ok(());
        }

        // Ctrl+T: toggle chat panel
        if self.keybindings.matches("chat_toggle", &key, modifiers) {
            self.chat_visible = !self.chat_visible;
            if self.chat_visible {
                // Ensure a conversation exists
                if self.chat_conversation_id.is_none() {
                    if let Some(ref mut svc) = self.chat_service {
                        let conv_id = svc.create_conversation(None);
                        self.chat_conversation_id = Some(conv_id);
                    }
                }
                self.state = AppState::Chat;
            } else {
                self.state = AppState::Normal;
            }
            return Ok(());
        }

        if self.keybindings.matches("help", &key, modifiers) {
            self.help_visible = !self.help_visible;
            self.state = if self.help_visible {
                AppState::Help
            } else {
                AppState::Normal
            };
            return Ok(());
        }

        if self.keybindings.matches("restart", &key, modifiers) {
            if self.selected_session().is_some() {
                self.activity.push(super::activity::ActivityOp::StartingSession, "Restarting session...");
                self.restart_selected().await?;
                self.activity.complete(super::activity::ActivityOp::StartingSession);
            }
            return Ok(());
        }

        if self.keybindings.matches("resume", &key, modifiers) {
            if self.selected_session().is_some() {
                self.resume_selected().await?;
            }
            return Ok(());
        }

        #[cfg(feature = "pro")]
        if self.keybindings.matches("skills_browser", &key, modifiers) {
            if self.auth_token.as_ref().is_some_and(|t| t.is_pro()) {
                self.open_skills_manager();
            }
            return Ok(());
        }

        // Ctrl+E: Max = toggle relationship edges on canvas, Pro = Relationships panel
        #[cfg(feature = "pro")]
        if key == KeyCode::Char('e') && modifiers == KeyModifiers::CONTROL {
            if self.auth_token.as_ref().is_some_and(|t| t.is_max()) {
                // Max: toggle relationship edge visibility on canvas
                self.canvas_state.show_relationship_edges = !self.canvas_state.show_relationship_edges;
                if !self.canvas_focused {
                    self.canvas_focused = true;
                }
            } else if self.auth_token.as_ref().is_some_and(|t| t.is_pro()) {
                // Pro: existing relationships panel
                self.refresh_snapshot_counts_async().await;
                self.state = AppState::Relationships;
            }
            return Ok(());
        }

        // J: Join a shared session via URL (Max tier)
        #[cfg(feature = "pro")]
        if key == KeyCode::Char('J') && modifiers == KeyModifiers::SHIFT {
            if crate::auth::AuthToken::require_max("sharing").is_ok() {
                let mut join_d = crate::ui::dialogs::JoinSessionDialog::new();
                join_d.viewer_identity = self.auth_token.as_ref().map(|t| t.email.clone());
                self.dialog = Some(Dialog::JoinSession(join_d));
                self.state = AppState::Dialog;
            }
            return Ok(());
        }

        // S: Share selected session (Max tier)
        #[cfg(feature = "pro")]
        if key == KeyCode::Char('S') && modifiers == KeyModifiers::SHIFT {
            if let Some(inst) = self.selected_session() {
                if crate::auth::AuthToken::require_max("sharing").is_ok() {
                    let already_sharing = inst.sharing.is_some()
                        && inst.sharing.as_ref().is_some_and(|s| s.active);
                    let sharing_cfg = crate::config::ConfigFile::load()
                        .await
                        .ok()
                        .flatten()
                        .map(|c| c.sharing().clone())
                        .unwrap_or_default();
                    let default_perm = match sharing_cfg.default_permission.as_str() {
                        "rw" => crate::sharing::SharePermission::ReadWrite,
                        _ => crate::sharing::SharePermission::ReadOnly,
                    };
                    let mut expire_input = TextInput::new();
                    if let Some(mins) = sharing_cfg.auto_expire_minutes {
                        expire_input.set_text(&mins.to_string());
                    }
                    // Restore relay URL/room_id from existing relay client when already sharing
                    let (relay_share_url, relay_room_id) = if already_sharing {
                        // Try to get from active relay client first
                        if let Some(client) = self.pro.relay_clients.get(&inst.id) {
                            (client.share_url().await, client.room_id().await)
                        }
                        // Fallback: restore from persisted sharing state
                        else if let Some(ref sharing) = inst.sharing {
                            let url = sharing.links.first()
                                .and_then(|link| link.web_url.clone());
                            (url, None) // room_id cannot be recovered, but URL can
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    };
                    let web_url = relay_share_url.clone();
                    let dialog = ShareDialog {
                        session_id: inst.id.clone(),
                        session_title: inst.title.clone(),
                        permission: default_perm,
                        expire_minutes: expire_input,
                        ssh_url: None,
                        web_url,
                        already_sharing,
                        relay_share_url,
                        relay_room_id,
                        copy_feedback_at: None,
                        selected_viewer: None,
                        status_message: None,
                    };
                    self.dialog = Some(Dialog::Share(dialog));
                    self.state = AppState::Dialog;
                }
            }
            return Ok(());
        }

        Ok(())
    }

    // handle_relationships_key defined in pro/src/ui/keys_pro.rs

    pub(super) async fn handle_search_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        match key {
            KeyCode::Esc => {
                self.state = AppState::Normal;
            }
            KeyCode::Enter => {
                if let Some(id) = self.search_results.get(self.search_selected).cloned() {
                    self.focus_session(&id).await?;
                }
                self.state = AppState::Normal;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_search_results();
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.state = AppState::Normal;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.search_results.is_empty() {
                    if self.search_selected == 0 {
                        self.search_selected = self.search_results.len() - 1;
                    } else {
                        self.search_selected -= 1;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.search_results.is_empty() {
                    self.search_selected = (self.search_selected + 1) % self.search_results.len();
                }
            }
            KeyCode::Char(ch) => {
                if !modifiers.contains(KeyModifiers::CONTROL) {
                    self.search_query.push(ch);
                    self.update_search_results();
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub(super) async fn handle_chat_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        match key {
            KeyCode::Esc => {
                self.chat_visible = false;
                self.state = AppState::Normal;
            }
            KeyCode::Enter => {
                let input = self.chat_input.trim().to_string();
                if !input.is_empty() {
                    if let (Some(ref mut svc), Some(ref conv_id)) =
                        (&mut self.chat_service, &self.chat_conversation_id)
                    {
                        let _ = svc.send_message(conv_id, &input, None);
                    }
                    self.chat_input.clear();
                }
            }
            KeyCode::Backspace => {
                self.chat_input.pop();
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.chat_visible = false;
                self.state = AppState::Normal;
            }
            KeyCode::Up => {
                self.chat_scroll = self.chat_scroll.saturating_add(1);
            }
            KeyCode::Down => {
                self.chat_scroll = self.chat_scroll.saturating_sub(1);
            }
            KeyCode::PageUp => {
                self.chat_scroll = self.chat_scroll.saturating_add(10);
            }
            KeyCode::PageDown => {
                self.chat_scroll = self.chat_scroll.saturating_sub(10);
            }
            KeyCode::Char(ch) => {
                if !modifiers.contains(KeyModifiers::CONTROL) {
                    self.chat_input.push(ch);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) async fn handle_dialog_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        let Some(dialog) = self.dialog.as_mut() else {
            self.state = AppState::Normal;
            return Ok(());
        };

        match dialog {
            Dialog::NewSession(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Tab => {
                    // Tab is reserved for Path completion/suggestions (no field cycling).
                    if d.field == NewSessionField::Path {
                        if d.path_suggestions_visible {
                            d.apply_selected_path_suggestion();
                        } else {
                            d.complete_path_or_cycle(false);
                        }
                    }
                }
                KeyCode::BackTab => {
                    // No Shift-Tab field cycling.
                    if d.field == NewSessionField::Path && d.path_suggestions_visible {
                        d.complete_path_or_cycle(true);
                    }
                }
                KeyCode::Up | KeyCode::Down => {
                    if d.field == NewSessionField::Group {
                        if d.group_matches.is_empty() {
                            return Ok(());
                        }
                        if matches!(key, KeyCode::Up) {
                            if d.group_selected == 0 {
                                d.group_selected = d.group_matches.len() - 1;
                            } else {
                                d.group_selected -= 1;
                            }
                        } else {
                            d.group_selected = (d.group_selected + 1) % d.group_matches.len();
                        }
                    } else if d.field == NewSessionField::Path && d.path_suggestions_visible {
                        d.complete_path_or_cycle(matches!(key, KeyCode::Up));
                    }
                }
                KeyCode::Enter => {
                    if d.field == NewSessionField::Path && d.path_suggestions_visible {
                        d.apply_selected_path_suggestion();
                    } else if d.field != NewSessionField::Group {
                        d.clear_path_suggestions();
                        d.path_dirty = false;
                        d.field = match d.field {
                            NewSessionField::Path => NewSessionField::Title,
                            NewSessionField::Title => NewSessionField::Group,
                            NewSessionField::Group => NewSessionField::Group,
                        };
                    } else {
                        if let Some(sel) = d.selected_group_value() {
                            d.group_path.set_text(sel.to_string());
                            d.update_group_matches();
                        } else {
                            d.group_path
                                .set_text(d.group_path.text().trim().to_string());
                            d.update_group_matches();
                        }

                        self.activity.push_default(super::activity::ActivityOp::CreatingSession);
                        self.create_session_from_dialog().await?;
                        self.dialog = None;
                        self.state = AppState::Normal;
                        self.refresh_sessions().await?;
                        self.activity.complete(super::activity::ActivityOp::CreatingSession);
                    }
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Backspace => {
                    match d.field {
                        NewSessionField::Path => {
                            d.path.backspace();
                            d.clear_path_suggestions();
                            d.path_dirty = true;
                            d.path_last_edit = Instant::now();
                        }
                        NewSessionField::Title => {
                            d.title.backspace();
                        }
                        NewSessionField::Group => {
                            d.group_path.backspace();
                            d.update_group_matches();
                        }
                    };
                }
                KeyCode::Delete => {
                    match d.field {
                        NewSessionField::Path => {
                            d.path.delete();
                            d.clear_path_suggestions();
                            d.path_dirty = true;
                            d.path_last_edit = Instant::now();
                        }
                        NewSessionField::Title => {
                            d.title.delete();
                        }
                        NewSessionField::Group => {
                            d.group_path.delete();
                            d.update_group_matches();
                        }
                    };
                }
                KeyCode::Left => match d.field {
                    NewSessionField::Path => {
                        if d.path_suggestions_visible {
                            d.complete_path_or_cycle(true);
                        } else {
                            d.path.move_left();
                        }
                    }
                    NewSessionField::Title => {
                        d.title.move_left();
                    }
                    NewSessionField::Group => {
                        if !d.group_matches.is_empty() {
                            if d.group_selected == 0 {
                                d.group_selected = d.group_matches.len() - 1;
                            } else {
                                d.group_selected -= 1;
                            }
                        } else {
                            d.group_path.move_left();
                        }
                    }
                },
                KeyCode::Right => match d.field {
                    NewSessionField::Path => {
                        if d.path_suggestions_visible {
                            d.complete_path_or_cycle(false);
                        } else {
                            d.path.move_right();
                        }
                    }
                    NewSessionField::Title => {
                        d.title.move_right();
                    }
                    NewSessionField::Group => {
                        if !d.group_matches.is_empty() {
                            d.group_selected = (d.group_selected + 1) % d.group_matches.len();
                        } else {
                            d.group_path.move_right();
                        }
                    }
                },
                KeyCode::Home => match d.field {
                    NewSessionField::Path => d.path.move_home(),
                    NewSessionField::Title => d.title.move_home(),
                    NewSessionField::Group => d.group_path.move_home(),
                },
                KeyCode::End => match d.field {
                    NewSessionField::Path => d.path.move_end(),
                    NewSessionField::Title => d.title.move_end(),
                    NewSessionField::Group => d.group_path.move_end(),
                },
                KeyCode::Char(ch) => {
                    if modifiers.contains(KeyModifiers::CONTROL) {
                        return Ok(());
                    }

                    if d.field == NewSessionField::Group {
                        match ch {
                            'k' => {
                                if !d.group_matches.is_empty() {
                                    if d.group_selected == 0 {
                                        d.group_selected = d.group_matches.len() - 1;
                                    } else {
                                        d.group_selected -= 1;
                                    }
                                }
                                return Ok(());
                            }
                            'j' => {
                                if !d.group_matches.is_empty() {
                                    d.group_selected =
                                        (d.group_selected + 1) % d.group_matches.len();
                                }
                                return Ok(());
                            }
                            _ => {}
                        }
                    }

                    match d.field {
                        NewSessionField::Path => {
                            d.path.insert(ch);
                            d.clear_path_suggestions();
                            d.path_dirty = true;
                            d.path_last_edit = Instant::now();
                        }
                        NewSessionField::Title => d.title.insert(ch),
                        NewSessionField::Group => {
                            d.group_path.insert(ch);
                            d.update_group_matches();
                        }
                    }
                }
                _ => {}
            },
            Dialog::QuitConfirm => match key {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.should_quit = true;
                }
                _ => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
            },
            Dialog::DeleteConfirm(d) => match key {
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    d.kill_tmux = !d.kill_tmux;
                }
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let session_id = d.session_id.clone();
                    let kill_tmux = d.kill_tmux;
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.activity.push_default(super::activity::ActivityOp::KillingSession);
                    self.delete_session(&session_id, kill_tmux).await?;
                    self.refresh_sessions().await?;
                    self.activity.complete(super::activity::ActivityOp::KillingSession);
                }
                _ => {}
            },
            Dialog::DeleteGroup(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    d.choice = match d.choice {
                        DeleteGroupChoice::DeleteGroupKeepSessions => {
                            DeleteGroupChoice::DeleteGroupAndSessions
                        }
                        DeleteGroupChoice::Cancel => DeleteGroupChoice::DeleteGroupKeepSessions,
                        DeleteGroupChoice::DeleteGroupAndSessions => DeleteGroupChoice::Cancel,
                    };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    d.choice = match d.choice {
                        DeleteGroupChoice::DeleteGroupKeepSessions => DeleteGroupChoice::Cancel,
                        DeleteGroupChoice::Cancel => DeleteGroupChoice::DeleteGroupAndSessions,
                        DeleteGroupChoice::DeleteGroupAndSessions => {
                            DeleteGroupChoice::DeleteGroupKeepSessions
                        }
                    };
                }
                KeyCode::Char('1') => d.choice = DeleteGroupChoice::DeleteGroupKeepSessions,
                KeyCode::Char('2') => d.choice = DeleteGroupChoice::Cancel,
                KeyCode::Char('3') => d.choice = DeleteGroupChoice::DeleteGroupAndSessions,
                KeyCode::Enter => {
                    let group_path = d.group_path.clone();
                    let choice = d.choice;
                    self.dialog = None;
                    self.state = AppState::Normal;
                    match choice {
                        DeleteGroupChoice::DeleteGroupKeepSessions => {
                            self.apply_delete_group_keep_sessions(&group_path).await?;
                        }
                        DeleteGroupChoice::Cancel => {}
                        DeleteGroupChoice::DeleteGroupAndSessions => {
                            self.apply_delete_group_and_sessions(&group_path).await?;
                        }
                    }
                    self.refresh_sessions().await?;
                }
                _ => {}
            },
            Dialog::Fork(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Tab => {
                    d.field = match d.field {
                        ForkField::Title => ForkField::Group,
                        ForkField::Group => ForkField::Title,
                    };
                }
                KeyCode::Enter => {
                    if d.field == ForkField::Title {
                        d.field = ForkField::Group;
                    } else {
                        let parent_session_id = d.parent_session_id.clone();
                        let project_path = d.project_path.clone();
                        let title = d.title.text().to_string();
                        let group_path = d.group_path.text().to_string();
                        self.dialog = None;
                        self.state = AppState::Normal;
                        let new_id = self
                            .create_fork_session(
                                &parent_session_id,
                                project_path,
                                &title,
                                &group_path,
                            )
                            .await?;
                        self.refresh_sessions().await?;
                        self.focus_session(&new_id).await?;
                    }
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Backspace => match d.field {
                    ForkField::Title => {
                        d.title.backspace();
                    }
                    ForkField::Group => {
                        d.group_path.backspace();
                    }
                },
                KeyCode::Delete => match d.field {
                    ForkField::Title => {
                        d.title.delete();
                    }
                    ForkField::Group => {
                        d.group_path.delete();
                    }
                },
                KeyCode::Left => match d.field {
                    ForkField::Title => d.title.move_left(),
                    ForkField::Group => d.group_path.move_left(),
                },
                KeyCode::Right => match d.field {
                    ForkField::Title => d.title.move_right(),
                    ForkField::Group => d.group_path.move_right(),
                },
                KeyCode::Home => match d.field {
                    ForkField::Title => d.title.move_home(),
                    ForkField::Group => d.group_path.move_home(),
                },
                KeyCode::End => match d.field {
                    ForkField::Title => d.title.move_end(),
                    ForkField::Group => d.group_path.move_end(),
                },
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        match d.field {
                            ForkField::Title => d.title.insert(ch),
                            ForkField::Group => d.group_path.insert(ch),
                        }
                    }
                }
                _ => {}
            },
            Dialog::RenameGroup(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Enter => {
                    let old_path = d.old_path.clone();
                    let new_path = d.new_path.text().to_string();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.apply_rename_group(&old_path, &new_path).await?;
                    self.refresh_sessions().await?;
                    self.focus_group(&new_path).await?;
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Backspace => {
                    d.new_path.backspace();
                }
                KeyCode::Delete => {
                    d.new_path.delete();
                }
                KeyCode::Left => {
                    d.new_path.move_left();
                }
                KeyCode::Right => {
                    d.new_path.move_right();
                }
                KeyCode::Home => {
                    d.new_path.move_home();
                }
                KeyCode::End => {
                    d.new_path.move_end();
                }
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        d.new_path.insert(ch);
                    }
                }
                _ => {}
            },
            Dialog::RenameSession(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Tab => {
                    d.field = match d.field {
                        SessionEditField::Title => SessionEditField::Label,
                        SessionEditField::Label => SessionEditField::Color,
                        SessionEditField::Color => SessionEditField::SessionId,
                        SessionEditField::SessionId => SessionEditField::Title,
                    };
                }
                KeyCode::Enter => {
                    if d.field != SessionEditField::SessionId {
                        d.field = match d.field {
                            SessionEditField::Title => SessionEditField::Label,
                            SessionEditField::Label => SessionEditField::Color,
                            SessionEditField::Color => SessionEditField::SessionId,
                            SessionEditField::SessionId => SessionEditField::Title,
                        };
                        return Ok(());
                    }

                    let session_id = d.session_id.clone();
                    let old_title = d.old_title.clone();
                    let title = d.new_title.text().to_string();
                    let label = d.label.text().to_string();
                    let label_color = d.label_color;
                    let cli_sid = d.cli_session_id.text().to_string();
                    let cli_sid_opt = if cli_sid.trim().is_empty() {
                        None
                    } else {
                        Some(cli_sid.as_str())
                    };
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.apply_edit_session(
                        &session_id, &old_title, &title, &label, label_color, cli_sid_opt,
                    )
                        .await?;
                    self.refresh_sessions().await?;
                    self.focus_session(&session_id).await?;
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Backspace => match d.field {
                    SessionEditField::Title => {
                        d.new_title.backspace();
                    }
                    SessionEditField::Label => {
                        d.label.backspace();
                    }
                    SessionEditField::SessionId => {
                        d.cli_session_id.backspace();
                    }
                    SessionEditField::Color => {}
                },
                KeyCode::Delete => match d.field {
                    SessionEditField::Title => {
                        d.new_title.delete();
                    }
                    SessionEditField::Label => {
                        d.label.delete();
                    }
                    SessionEditField::SessionId => {
                        d.cli_session_id.delete();
                    }
                    SessionEditField::Color => {}
                },
                KeyCode::Left => {
                    if d.field == SessionEditField::Color {
                        d.label_color = match d.label_color {
                            crate::session::LabelColor::Gray => crate::session::LabelColor::Blue,
                            crate::session::LabelColor::Magenta => crate::session::LabelColor::Gray,
                            crate::session::LabelColor::Cyan => crate::session::LabelColor::Magenta,
                            crate::session::LabelColor::Green => crate::session::LabelColor::Cyan,
                            crate::session::LabelColor::Yellow => crate::session::LabelColor::Green,
                            crate::session::LabelColor::Red => crate::session::LabelColor::Yellow,
                            crate::session::LabelColor::Blue => crate::session::LabelColor::Red,
                        };
                    } else {
                        match d.field {
                            SessionEditField::Title => d.new_title.move_left(),
                            SessionEditField::Label => d.label.move_left(),
                            SessionEditField::SessionId => d.cli_session_id.move_left(),
                            SessionEditField::Color => {}
                        }
                    }
                }
                KeyCode::Right => {
                    if d.field == SessionEditField::Color {
                        d.label_color = match d.label_color {
                            crate::session::LabelColor::Gray => crate::session::LabelColor::Magenta,
                            crate::session::LabelColor::Magenta => crate::session::LabelColor::Cyan,
                            crate::session::LabelColor::Cyan => crate::session::LabelColor::Green,
                            crate::session::LabelColor::Green => crate::session::LabelColor::Yellow,
                            crate::session::LabelColor::Yellow => crate::session::LabelColor::Red,
                            crate::session::LabelColor::Red => crate::session::LabelColor::Blue,
                            crate::session::LabelColor::Blue => crate::session::LabelColor::Gray,
                        };
                    } else {
                        match d.field {
                            SessionEditField::Title => d.new_title.move_right(),
                            SessionEditField::Label => d.label.move_right(),
                            SessionEditField::SessionId => d.cli_session_id.move_right(),
                            SessionEditField::Color => {}
                        }
                    }
                }
                KeyCode::Home => match d.field {
                    SessionEditField::Title => d.new_title.move_home(),
                    SessionEditField::Label => d.label.move_home(),
                    SessionEditField::SessionId => d.cli_session_id.move_home(),
                    SessionEditField::Color => {}
                },
                KeyCode::End => match d.field {
                    SessionEditField::Title => d.new_title.move_end(),
                    SessionEditField::Label => d.label.move_end(),
                    SessionEditField::SessionId => d.cli_session_id.move_end(),
                    SessionEditField::Color => {}
                },
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        if d.field == SessionEditField::Color {
                            match ch {
                                'h' => {
                                    d.label_color = match d.label_color {
                                        crate::session::LabelColor::Gray => {
                                            crate::session::LabelColor::Blue
                                        }
                                        crate::session::LabelColor::Magenta => {
                                            crate::session::LabelColor::Gray
                                        }
                                        crate::session::LabelColor::Cyan => {
                                            crate::session::LabelColor::Magenta
                                        }
                                        crate::session::LabelColor::Green => {
                                            crate::session::LabelColor::Cyan
                                        }
                                        crate::session::LabelColor::Yellow => {
                                            crate::session::LabelColor::Green
                                        }
                                        crate::session::LabelColor::Red => {
                                            crate::session::LabelColor::Yellow
                                        }
                                        crate::session::LabelColor::Blue => {
                                            crate::session::LabelColor::Red
                                        }
                                    };
                                }
                                'l' => {
                                    d.label_color = match d.label_color {
                                        crate::session::LabelColor::Gray => {
                                            crate::session::LabelColor::Magenta
                                        }
                                        crate::session::LabelColor::Magenta => {
                                            crate::session::LabelColor::Cyan
                                        }
                                        crate::session::LabelColor::Cyan => {
                                            crate::session::LabelColor::Green
                                        }
                                        crate::session::LabelColor::Green => {
                                            crate::session::LabelColor::Yellow
                                        }
                                        crate::session::LabelColor::Yellow => {
                                            crate::session::LabelColor::Red
                                        }
                                        crate::session::LabelColor::Red => {
                                            crate::session::LabelColor::Blue
                                        }
                                        crate::session::LabelColor::Blue => {
                                            crate::session::LabelColor::Gray
                                        }
                                    };
                                }
                                _ => {}
                            }
                        } else {
                            match d.field {
                                SessionEditField::Title => d.new_title.insert(ch),
                                SessionEditField::Label => d.label.insert(ch),
                                SessionEditField::SessionId => d.cli_session_id.insert(ch),
                                SessionEditField::Color => {}
                            }
                        }
                    }
                }
                _ => {}
            },
            Dialog::CreateGroup(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !d.matches.is_empty() {
                        if d.selected == 0 {
                            d.selected = d.matches.len() - 1;
                        } else {
                            d.selected -= 1;
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !d.matches.is_empty() {
                        d.selected = (d.selected + 1) % d.matches.len();
                    }
                }
                KeyCode::Enter => {
                    let group_path = d
                        .selected_value()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| d.input.text().trim().to_string());
                    self.dialog = None;
                    self.state = AppState::Normal;
                    if group_path.trim().is_empty() {
                        return Ok(());
                    }
                    self.apply_create_group(&group_path).await?;
                    self.refresh_sessions().await?;
                    self.focus_group(&group_path).await?;
                }
                KeyCode::Backspace => {
                    d.input.backspace();
                    d.update_matches();
                }
                KeyCode::Delete => {
                    d.input.delete();
                    d.update_matches();
                }
                KeyCode::Left => {
                    d.input.move_left();
                }
                KeyCode::Right => {
                    d.input.move_right();
                }
                KeyCode::Home => {
                    d.input.move_home();
                }
                KeyCode::End => {
                    d.input.move_end();
                }
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        d.input.insert(ch);
                        d.update_matches();
                    }
                }
                _ => {}
            },
            Dialog::MoveGroup(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !d.matches.is_empty() {
                        if d.selected == 0 {
                            d.selected = d.matches.len() - 1;
                        } else {
                            d.selected -= 1;
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !d.matches.is_empty() {
                        d.selected = (d.selected + 1) % d.matches.len();
                    }
                }
                KeyCode::Enter => {
                    let session_id = d.session_id.clone();
                    let group_path = d
                        .selected_value()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| d.input.text().trim().to_string());
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.apply_move_group(&session_id, &group_path).await?;
                    self.refresh_sessions().await?;
                    self.focus_session(&session_id).await?;
                }
                KeyCode::Backspace => {
                    d.input.backspace();
                    d.update_matches();
                }
                KeyCode::Delete => {
                    d.input.delete();
                    d.update_matches();
                }
                KeyCode::Left => {
                    d.input.move_left();
                }
                KeyCode::Right => {
                    d.input.move_right();
                }
                KeyCode::Home => {
                    d.input.move_home();
                }
                KeyCode::End => {
                    d.input.move_end();
                }
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        d.input.insert(ch);
                        d.update_matches();
                    }
                }
                _ => {}
            },
            Dialog::TagPicker(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if !d.tags.is_empty() {
                        if d.selected == 0 {
                            d.selected = d.tags.len() - 1;
                        } else {
                            d.selected -= 1;
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !d.tags.is_empty() {
                        d.selected = (d.selected + 1) % d.tags.len();
                    }
                }
                KeyCode::Enter => {
                    let session_id = d.session_id.clone();
                    let tag = d.tags.get(d.selected).cloned();
                    self.dialog = None;
                    self.state = AppState::Normal;

                    let Some(tag) = tag else {
                        return Ok(());
                    };
                    let Some(s) = self.session_by_id(&session_id) else {
                        return Ok(());
                    };
                    let old_title = s.title.clone();
                    self.apply_edit_session(
                        &session_id,
                        &old_title,
                        &old_title,
                        &tag.name,
                        tag.color,
                        None,
                    )
                    .await?;
                    self.refresh_sessions().await?;
                    self.focus_session(&session_id).await?;
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::CreateRelationship(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Tab => {
                    d.cycle_relation_type();
                }
                KeyCode::BackTab => {
                    d.field = match d.field {
                        CreateRelationshipField::Search => CreateRelationshipField::Label,
                        CreateRelationshipField::Label => CreateRelationshipField::Search,
                    };
                }
                KeyCode::Up => {
                    if d.selected > 0 {
                        d.selected -= 1;
                    }
                }
                KeyCode::Down => {
                    if !d.matches.is_empty() && d.selected < d.matches.len() - 1 {
                        d.selected += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some((b_id, _b_title)) = d.selected_session().cloned() {
                        let a_id = d.session_a_id.clone();
                        let label = if d.label.text().trim().is_empty() {
                            None
                        } else {
                            Some(d.label.text().trim().to_string())
                        };
                        let mut rel = crate::session::Relationship::new(
                            d.relation_type,
                            a_id.clone(),
                            b_id.clone(),
                        );
                        if let Some(l) = label {
                            rel = rel.with_label(l);
                        }
                        crate::session::relationships::add_relationship(
                            &mut self.relationships,
                            rel.clone(),
                        );

                        // Clone session data needed for relationship session creation
                        let session_a = self.session_by_id(&a_id).cloned();
                        let session_b = self.session_by_id(&b_id).cloned();

                        let storage = self.storage.lock().await;
                        storage
                            .save(&self.sessions, &self.groups, &self.relationships)
                            .await?;
                        drop(storage);

                        // Auto-create relationship workspace session (sessionC)
                        if let (Some(sa), Some(sb)) = (session_a, session_b) {
                            if let Ok(session_c_id) = self.create_relationship_session(&rel, &sa, &sb).await {
                                // Refresh to pick up new session + group
                                self.refresh_sessions().await?;

                                // Auto-populate canvas with A, B, C nodes
                                let sc = self.session_by_id(&session_c_id).cloned();
                                if let Some(ref sc_inst) = sc {
                                    let a_status = format!("{:?}", sa.status).to_lowercase();
                                    let b_status = format!("{:?}", sb.status).to_lowercase();
                                    let c_status = format!("{:?}", sc_inst.status).to_lowercase();
                                    self.canvas_state.add_session_node(&sa.id, &sa.title, &a_status);
                                    self.canvas_state.add_session_node(&sb.id, &sb.title, &b_status);
                                    self.canvas_state.add_session_node(&session_c_id, &sc_inst.title, &c_status);
                                    // Edges are handled automatically by sync_relationship_edges on next tick
                                }
                            }
                        }

                        let _ = self.analytics.record_premium_event(
                            crate::analytics::EventType::RelationshipCreate,
                            &a_id,
                            "",
                        ).await;
                        self.dialog = None;
                        self.state = AppState::Relationships;
                    }
                }
                KeyCode::Backspace => match d.field {
                    CreateRelationshipField::Search => {
                        d.search_input.backspace();
                        d.update_matches();
                    }
                    CreateRelationshipField::Label => {
                        d.label.backspace();
                    }
                },
                KeyCode::Char(c) => match d.field {
                    CreateRelationshipField::Search => {
                        d.search_input.insert(c);
                        d.update_matches();
                    }
                    CreateRelationshipField::Label => {
                        d.label.insert(c);
                    }
                },
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::Share(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Tab => {
                    // Only allow permission toggle before sharing starts
                    if !d.already_sharing {
                        d.permission = match d.permission {
                            crate::sharing::SharePermission::ReadOnly => {
                                crate::sharing::SharePermission::ReadWrite
                            }
                            crate::sharing::SharePermission::ReadWrite => {
                                crate::sharing::SharePermission::ReadOnly
                            }
                        };
                    }
                }
                KeyCode::Enter => {
                    // Guard: ignore Enter while a connection is in progress
                    let is_connecting = d.status_message.as_ref().is_some_and(|m| !m.starts_with('✓') && !m.starts_with('✗'));
                    if is_connecting {
                        // Do nothing — connection already in progress
                    } else if d.already_sharing {
                        // Stop sharing — try relay cleanup first, then tmate
                        if let Some(ref room_id) = d.relay_room_id {
                            // Relay sharing — stop client and pipe-pane
                            let tmux_name = self.sessions_by_id
                                .get(&d.session_id)
                                .and_then(|&idx| self.sessions.get(idx))
                                .map(|s| s.tmux_name())
                                .unwrap_or_else(|| TmuxManager::session_name_legacy(&d.session_id));
                            if let Some(client) = self.pro.relay_clients.remove(&d.session_id) {
                                client.stop(&tmux_name).await;
                            } else {
                                let _ = self.tmux.stop_pipe_pane(&tmux_name).await;
                            }
                            // Remove from ledger
                            let mut ledger = crate::pro::collab::ledger::RoomLedger::load();
                            ledger.remove(room_id);
                        } else {
                            // Tmate sharing
                            let mut mgr = crate::pro::tmate::TmateManager::from_config().await;
                            let _ = mgr.stop_sharing(&d.session_id).await;
                        }
                        d.already_sharing = false;
                        d.ssh_url = None;
                        d.web_url = None;
                        d.relay_share_url = None;
                        d.relay_room_id = None;
                        if let Some(inst) =
                            self.sessions.iter_mut().find(|s| s.id == d.session_id)
                        {
                            inst.sharing = None;
                        }
                        let storage = self.storage.lock().await;
                        storage
                            .save(&self.sessions, &self.groups, &self.relationships)
                            .await?;
                        drop(storage);
                        let _ = self.analytics.record_premium_event(
                            crate::analytics::EventType::ShareStop,
                            &d.session_id,
                            &d.session_title,
                        ).await;
                    } else {
                        // Start sharing — try relay first (non-blocking), fall back to tmate
                        let sharing_cfg = crate::config::ConfigFile::load()
                            .await
                            .ok()
                            .flatten()
                            .map(|c| c.sharing().clone())
                            .unwrap_or_default();

                        let sid = d.session_id.clone();
                        let title = d.session_title.clone();
                        let perm = d.permission;
                        let expire: Option<u64> = d
                            .expire_minutes
                            .text()
                            .parse::<u64>()
                            .ok()
                            .filter(|&v| v > 0);
                        let tmux_name = self.sessions_by_id
                            .get(&sid)
                            .and_then(|&idx| self.sessions.get(idx))
                            .map(|s| s.tmux_name())
                            .unwrap_or_else(|| TmuxManager::session_name_legacy(&sid));

                        // Check if relay is available (config override or auth token)
                        let has_relay = sharing_cfg.relay_server_url.is_some()
                            || self.auth_token.is_some();

                        if has_relay {
                            // Spawn relay connection as background task — returns immediately
                            // so the spinner renders on the next frame (250ms)
                            d.status_message = Some("Creating room...".to_string());

                            let relay_url_override = sharing_cfg.relay_server_url.clone();
                            let discovery_url = sharing_cfg.relay_discovery_url.clone();
                            let auth_token = self.auth_token.as_ref().unwrap().access_token.clone();
                            let tmux_name_owned = tmux_name.clone();

                            let (tx, rx) = tokio::sync::oneshot::channel();
                            self.pro.share_task_rx = Some(rx);
                            self.activity.push_default(super::activity::ActivityOp::StartingShare);

                            tokio::spawn(async move {
                                use super::ShareTaskResult;
                                use super::ShareTaskError;

                                let result: std::result::Result<ShareTaskResult, ShareTaskError> = async {
                                    // 1. Discover relay (or use config override)
                                    let relay = match relay_url_override {
                                        Some(url) => url,
                                        None => {
                                            crate::pro::collab::client::RelayClient::discover_relay(
                                                &discovery_url,
                                                &auth_token,
                                            )
                                            .await
                                            .ok_or_else(|| ShareTaskError {
                                                message: "No relay server available".into(),
                                            })?
                                        }
                                    };

                                    // 2. Create room
                                    let client = Arc::new(
                                        crate::pro::collab::client::RelayClient::new(
                                            relay.clone(),
                                            auth_token,
                                        ),
                                    );
                                    let perm_str = perm.to_string();
                                    let room = client
                                        .create_room(&sid, &perm_str, expire)
                                        .await
                                        .map_err(|e| ShareTaskError {
                                            message: format!("Room creation failed: {}", e),
                                        })?;

                                    // 3. Start streaming
                                    client
                                        .start_streaming(&tmux_name_owned)
                                        .await
                                        .map_err(|e| ShareTaskError {
                                            message: format!("Connection failed: {}", e),
                                        })?;

                                    Ok(ShareTaskResult {
                                        session_id: sid,
                                        session_title: title,
                                        relay_client: client,
                                        relay_url: relay,
                                        room_id: room.room_id,
                                        share_url: room.share_url,
                                        host_token: room.host_token,
                                        permission: perm,
                                        expire_minutes: expire,
                                    })
                                }
                                .await;

                                let _ = tx.send(result);
                            });
                        } else if crate::pro::tmate::TmateManager::is_available().await {
                            // Tmate fallback — stays blocking (rare path)
                            let mut mgr = crate::pro::tmate::TmateManager::from_config().await;
                            match mgr
                                .start_sharing(&sid, &tmux_name, perm, expire)
                                .await
                            {
                                Ok(state) => {
                                    let ssh = state
                                        .links
                                        .iter()
                                        .find(|l| l.permission == d.permission)
                                        .or_else(|| state.links.first())
                                        .map(|l| l.ssh_url.clone())
                                        .unwrap_or_default();
                                    let web = state
                                        .links
                                        .iter()
                                        .find(|l| l.permission == d.permission)
                                        .or_else(|| state.links.first())
                                        .and_then(|l| l.web_url.clone());
                                    d.ssh_url = Some(ssh);
                                    d.web_url = web;
                                    d.already_sharing = true;
                                    if let Some(inst) = self
                                        .sessions
                                        .iter_mut()
                                        .find(|s| s.id == d.session_id)
                                    {
                                        inst.sharing = Some(state);
                                    }
                                    let storage = self.storage.lock().await;
                                    storage
                                        .save(
                                            &self.sessions,
                                            &self.groups,
                                            &self.relationships,
                                        )
                                        .await?;
                                    let _ = self.analytics.record_premium_event(
                                        crate::analytics::EventType::ShareStart,
                                        &d.session_id,
                                        &d.session_title,
                                    ).await;
                                }
                                Err(e) => {
                                    tracing::warn!("tmate sharing failed: {}", e);
                                    d.web_url = Some(format!("tmate error: {}", e));
                                }
                            }
                        } else {
                            d.web_url = Some("No sharing backend available. Configure relay_server_url or install tmate.".to_string());
                        }
                    }
                }
                KeyCode::Char('c') => {
                    // Copy the best available URL: relay > web > ssh
                    let url = d.relay_share_url.as_ref()
                        .or(d.web_url.as_ref())
                        .or(d.ssh_url.as_ref());
                    if let Some(url) = url {
                        let copy_cmd = if cfg!(target_os = "macos") {
                            "pbcopy"
                        } else {
                            "xclip"
                        };
                        let copy_args: &[&str] = if cfg!(target_os = "macos") {
                            &[]
                        } else {
                            &["-selection", "clipboard"]
                        };
                        let copy_result = std::process::Command::new(copy_cmd)
                            .args(copy_args)
                            .stdin(std::process::Stdio::piped())
                            .spawn()
                            .and_then(|mut child| {
                                use std::io::Write;
                                if let Some(ref mut stdin) = child.stdin {
                                    stdin.write_all(url.as_bytes())?;
                                }
                                child.wait()
                            });
                        if copy_result.is_ok() {
                            d.copy_feedback_at = Some(Instant::now());
                        }
                        #[cfg(feature = "pro")]
                        {
                            let msg = if copy_result.is_ok() {
                                "URL copied to clipboard"
                            } else {
                                "Failed to copy URL"
                            };
                            let color = if copy_result.is_ok() {
                                ratatui::style::Color::Green
                            } else {
                                ratatui::style::Color::Red
                            };
                            self.pro.toast_notifications.push(ToastNotification {
                                message: msg.to_string(),
                                created_at: Instant::now(),
                                color,
                            });
                        }
                    }
                }
                KeyCode::Backspace => {
                    d.expire_minutes.backspace();
                }
                KeyCode::Char(ch) if ch.is_ascii_digit() => {
                    d.expire_minutes.insert(ch);
                }
                KeyCode::Up => {
                    // Navigate viewer list
                    if d.already_sharing {
                        let viewer_count = self.pro.relay_clients.get(&d.session_id)
                            .map(|c| c.viewers().len())
                            .unwrap_or(0);
                        if viewer_count > 0 {
                            d.selected_viewer = Some(match d.selected_viewer {
                                Some(i) if i > 0 => i - 1,
                                _ => viewer_count.saturating_sub(1),
                            });
                        }
                    }
                }
                KeyCode::Down => {
                    if d.already_sharing {
                        let viewer_count = self.pro.relay_clients.get(&d.session_id)
                            .map(|c| c.viewers().len())
                            .unwrap_or(0);
                        if viewer_count > 0 {
                            d.selected_viewer = Some(match d.selected_viewer {
                                Some(i) if i + 1 < viewer_count => i + 1,
                                _ => 0,
                            });
                        }
                    }
                }
                KeyCode::Char('d') => {
                    // Revoke selected viewer's RW control
                    if d.already_sharing {
                        if let Some(idx) = d.selected_viewer {
                            if let Some(client) = self.pro.relay_clients.get(&d.session_id) {
                                let viewers = client.viewers();
                                if let Some(v) = viewers.get(idx) {
                                    if v.permission == "rw" {
                                        let vid = v.viewer_id.clone();
                                        let name = v.display_name.clone();
                                        client.revoke_control(&vid).await;
                                        self.pro.toast_notifications.push(ToastNotification {
                                            message: format!("Revoked RW from {}", name),
                                            created_at: Instant::now(),
                                            color: ratatui::style::Color::Yellow,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::Annotate(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Relationships;
                }
                KeyCode::Enter => {
                    let note_text = d.note.text().trim().to_string();
                    if !note_text.is_empty() {
                        let rel_id = d.relationship_id.clone();
                        // Find the relationship to get session IDs
                        if let Some(rel) = self.relationships.iter().find(|r| r.id == rel_id) {
                            let snapshot = crate::session::context::ContextSnapshot::annotation(
                                &rel.session_a_id,
                                note_text,
                            ).with_relationship(&rel_id);
                            let profile = self.storage.lock().await.profile().to_string();
                            let collector = crate::pro::context::ContextCollector::new(&profile);
                            let _ = collector.save_snapshot(&snapshot).await;
                        }
                    }
                    self.dialog = None;
                    self.state = AppState::Relationships;
                }
                KeyCode::Backspace => {
                    d.note.backspace();
                }
                KeyCode::Char(c) => {
                    d.note.insert(c);
                }
                _ => {}
            },
            #[cfg(feature = "pro")]
            Dialog::NewFromContext(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Relationships;
                }
                KeyCode::Tab => {
                    d.injection_method = d.injection_method.cycle();
                }
                KeyCode::Enter => {
                    let title = d.title.text().trim().to_string();
                    if title.is_empty() {
                        // Don't create if title is empty
                    } else {
                        let relationship_id = d.relationship_id.clone();
                        let context = d.context_preview.clone();
                        let injection_method = d.injection_method;

                        // Find the relationship to get session IDs
                        let rel = self
                            .relationships
                            .iter()
                            .find(|r| r.id == relationship_id)
                            .cloned();

                        if let Some(rel) = rel {
                            // Get project path from session_a
                            let project_path = self
                                .session_by_id(&rel.session_a_id)
                                .map(|s| s.project_path.clone());

                            if let Some(project_path) = project_path {
                                let group_path = self
                                    .session_by_id(&rel.session_a_id)
                                    .map(|s| s.group_path.clone())
                                    .unwrap_or_default();

                                // Create the new Instance
                                let session_title = title.clone();
                                let mut inst = Instance::new(title, project_path.clone());
                                inst.group_path = group_path;
                                inst.tool = crate::tmux::Tool::Shell;
                                let new_id = inst.id.clone();
                                let tmux_name = inst.tmux_name();

                                // Save the instance to storage
                                {
                                    let storage = self.storage.lock().await;
                                    let (mut instances, tree, relationships) =
                                        storage.load().await?;
                                    instances.push(inst);
                                    storage.save(&instances, &tree, &relationships).await?;
                                }

                                // Create the tmux session
                                let working_dir =
                                    project_path.to_str().unwrap_or("/tmp").to_string();
                                self.tmux
                                    .create_session(&tmux_name, &working_dir, None, Some(&session_title))
                                    .await?;

                                // Inject context based on method
                                match injection_method {
                                    crate::ui::ContextInjectionMethod::InitialPrompt => {
                                        // Send context as the first message via send-keys
                                        let escaped = context.replace('\'', "'\\''");
                                        let cmd = format!("echo '{}'", escaped);
                                        let _ =
                                            self.tmux.send_keys(&tmux_name, &cmd).await;
                                    }
                                    crate::ui::ContextInjectionMethod::ClaudeMd => {
                                        // Write context to {project_path}/CLAUDE.md
                                        let claude_md = project_path.join("CLAUDE.md");
                                        let _ =
                                            tokio::fs::write(&claude_md, &context).await;
                                    }
                                    crate::ui::ContextInjectionMethod::EnvironmentVariable => {
                                        // Export AGENT_HAND_CONTEXT in the tmux session
                                        let escaped = context.replace('\'', "'\\''");
                                        let cmd = format!(
                                            "export AGENT_HAND_CONTEXT='{}'",
                                            escaped
                                        );
                                        let _ =
                                            self.tmux.send_keys(&tmux_name, &cmd).await;
                                    }
                                }

                                // Add relationships linking new session to both source sessions
                                let rel_a = crate::session::Relationship::new(
                                    crate::session::RelationType::Peer,
                                    rel.session_a_id.clone(),
                                    new_id.clone(),
                                );
                                crate::session::relationships::add_relationship(
                                    &mut self.relationships,
                                    rel_a,
                                );

                                let rel_b = crate::session::Relationship::new(
                                    crate::session::RelationType::Peer,
                                    rel.session_b_id.clone(),
                                    new_id.clone(),
                                );
                                crate::session::relationships::add_relationship(
                                    &mut self.relationships,
                                    rel_b,
                                );

                                // Save relationships
                                {
                                    let storage = self.storage.lock().await;
                                    storage
                                        .save(
                                            &self.sessions,
                                            &self.groups,
                                            &self.relationships,
                                        )
                                        .await?;
                                }

                                self.dialog = None;
                                self.state = AppState::Relationships;
                                self.refresh_sessions().await?;
                            } else {
                                self.dialog = None;
                                self.state = AppState::Relationships;
                            }
                        } else {
                            self.dialog = None;
                            self.state = AppState::Relationships;
                        }
                    }
                }
                KeyCode::Backspace => {
                    d.title.backspace();
                }
                KeyCode::Char(c) => {
                    d.title.insert(c);
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::JoinSession(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Enter => {
                    if !d.connecting {
                        let url_text = d.url_input.text().to_string();
                        if let Some((relay_url, room_id, token)) =
                            crate::ui::dialogs::JoinSessionDialog::parse_share_url(&url_text)
                        {
                            d.connecting = true;
                            d.status = Some("Connecting...".to_string());
                            d.validation_hint = None;
                            let relay = relay_url.clone();
                            let rid = room_id.clone();
                            let tok = token.clone();
                            // Keep dialog open during connection attempt
                            match self.connect_viewer(&relay, &rid, &tok).await {
                                Ok(()) => {
                                    // Success — dialog is replaced by ViewerMode (connect_viewer sets state)
                                    self.dialog = None;
                                }
                                Err(e) => {
                                    // Show user-friendly error in the dialog
                                    if let Some(Dialog::JoinSession(ref mut d)) = self.dialog {
                                        d.connecting = false;
                                        let msg = e.to_string();
                                        let friendly = if msg.contains("timeout") || msg.contains("Timeout") {
                                            "Connection timed out. Check your network.".to_string()
                                        } else if msg.contains("404") || msg.contains("not found") {
                                            "Session not found. Link may have expired.".to_string()
                                        } else if msg.contains("401") || msg.contains("auth") {
                                            "Access denied. Token may be invalid.".to_string()
                                        } else if msg.contains("WebSocket") || msg.contains("Connection refused") {
                                            "Relay server unreachable.".to_string()
                                        } else {
                                            format!("Connection failed: {}", e)
                                        };
                                        d.status = Some(friendly);
                                    }
                                }
                            }
                        } else {
                            d.status = Some("Invalid URL format. Expected: https://.../share/ROOM_ID?token=TOKEN".to_string());
                        }
                    }
                }
                KeyCode::Backspace => {
                    d.url_input.backspace();
                    d.validation_hint = live_url_validation_hint(d.url_input.text());
                }
                KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
                    // Clipboard paste — platform-aware with Wayland fallback
                    let clipboard_text = if cfg!(target_os = "macos") {
                        std::process::Command::new("pbpaste").output().ok()
                    } else {
                        // Try wl-paste (Wayland) first, fall back to xclip (X11)
                        std::process::Command::new("wl-paste")
                            .arg("--no-newline")
                            .output()
                            .ok()
                            .filter(|o| o.status.success())
                            .or_else(|| {
                                std::process::Command::new("xclip")
                                    .args(["-selection", "clipboard", "-o"])
                                    .output()
                                    .ok()
                            })
                    };
                    if let Some(output) = clipboard_text {
                        if output.status.success() {
                            if let Ok(text) = String::from_utf8(output.stdout) {
                                let trimmed = text.trim();
                                for c in trimmed.chars() {
                                    d.url_input.insert(c);
                                }
                                d.validation_hint = live_url_validation_hint(d.url_input.text());
                            }
                        }
                    }
                }
                KeyCode::Char(c) => {
                    d.url_input.insert(c);
                    d.validation_hint = live_url_validation_hint(d.url_input.text());
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::ControlRequest(ref d) => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let sid = d.session_id.clone();
                    let vid = d.viewer_id.clone();
                    let name = d.display_name.clone();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    // Approve the control request
                    if let Some(client) = self.pro.relay_clients.get(&sid) {
                        client.respond_control(&vid, true).await;
                    }
                    self.pro.toast_notifications.push(ToastNotification {
                        message: format!("Granted RW control to {}", name),
                        created_at: Instant::now(),
                        color: ratatui::style::Color::Green,
                    });
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    let sid = d.session_id.clone();
                    let vid = d.viewer_id.clone();
                    let name = d.display_name.clone();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    // Deny the control request
                    if let Some(client) = self.pro.relay_clients.get(&sid) {
                        client.respond_control(&vid, false).await;
                    }
                    self.pro.toast_notifications.push(ToastNotification {
                        message: format!("Denied control request from {}", name),
                        created_at: Instant::now(),
                        color: ratatui::style::Color::Yellow,
                    });
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::HumanReview(ref d) => match key {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    let record_id = d.record_id.clone();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.approve_human_review(&record_id);
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    let record_id = d.record_id.clone();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.dismiss_human_review(&record_id);
                }
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::ProposalAction(ref d) => match key {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    let proposal_id = d.proposal_id.clone();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.accept_followup_proposal(&proposal_id);
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    let proposal_id = d.proposal_id.clone();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.reject_followup_proposal(&proposal_id, "Rejected by user");
                }
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::ConfirmInjection(ref mut d) => match key {
                KeyCode::Up => {
                    if d.cursor > 0 {
                        d.cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if d.cursor + 1 < d.targets.len() {
                        d.cursor += 1;
                    }
                }
                KeyCode::Char(' ') => {
                    d.toggle_current();
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    d.select_all();
                }
                KeyCode::Enter => {
                    let targets: Vec<crate::ui::InjectionTarget> = d.selected_targets()
                        .into_iter().cloned().collect();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    let count = self.execute_proposal_injection(&targets);
                    self.pro.toast_notifications.push(super::ToastNotification {
                        message: format!("Injected into {} session(s)", count),
                        created_at: Instant::now(),
                        color: ratatui::style::Color::Green,
                    });
                }
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::DisconnectViewer(ref mut d) => match key {
                KeyCode::Up => {
                    if d.selected_option > 0 {
                        d.selected_option -= 1;
                    }
                }
                KeyCode::Down => {
                    if d.selected_option < 2 {
                        d.selected_option += 1;
                    }
                }
                KeyCode::Enter => {
                    let room_id = d.room_id.clone();
                    let option = d.selected_option;
                    self.dialog = None;
                    self.state = AppState::Normal;

                    match option {
                        0 => {
                            // Disconnect only
                            self.disconnect_viewer_session(&room_id, false).await;
                        }
                        1 => {
                            // Disconnect and delete
                            self.disconnect_viewer_session(&room_id, true).await;
                        }
                        2 => {
                            // Cancel - do nothing
                        }
                        _ => {}
                    }
                }
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                _ => {}
            },

            Dialog::PackBrowser(ref mut d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    d.move_selection(-1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    d.move_selection(1);
                }
                KeyCode::Enter => {
                    if !d.installing && !d.loading {
                        self.install_selected_pack().await;
                    }
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::OrphanedRooms(ref mut d) => match key {
                KeyCode::Esc => {
                    // Dismiss — clean up ledger entries so they don't reappear.
                    // The rooms will expire on the relay server via its 30s grace period.
                    {
                        let mut ledger = crate::pro::collab::ledger::RoomLedger::load();
                        for room in &d.rooms {
                            ledger.remove(&room.room_id);
                        }
                    }
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    d.move_selection(-1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    d.move_selection(1);
                }
                KeyCode::Enter | KeyCode::Char('d') => {
                    // Close the selected room
                    if let Some(room) = d.rooms.get(d.selected_index).cloned() {
                        let closed = crate::pro::collab::client::RelayClient::close_room(
                            &room.relay_url,
                            &room.room_id,
                            &room.host_token,
                        ).await;
                        if closed {
                            let mut ledger = crate::pro::collab::ledger::RoomLedger::load();
                            ledger.remove(&room.room_id);
                        }
                        d.rooms.retain(|r| r.room_id != room.room_id);
                        if d.selected_index >= d.rooms.len() && !d.rooms.is_empty() {
                            d.selected_index = d.rooms.len() - 1;
                        }
                        if d.rooms.is_empty() {
                            self.dialog = None;
                            self.state = AppState::Normal;
                        }
                    }
                }
                KeyCode::Char('r') => {
                    // Reconnect to the selected room
                    if let Some(room) = d.rooms.get(d.selected_index).cloned() {
                        let sid = room.session_id.clone();
                        // Find the tmux session's name
                        let tmux_name = self.sessions_by_id
                            .get(&sid)
                            .and_then(|&idx| self.sessions.get(idx))
                            .map(|s| s.tmux_name())
                            .unwrap_or_else(|| crate::tmux::TmuxManager::session_name_legacy(&sid));

                        // Create RelayClient and reconnect
                        let auth_token = self.auth_token
                            .as_ref()
                            .map(|a| a.access_token.clone())
                            .unwrap_or_default();
                        let client = Arc::new(crate::pro::collab::client::RelayClient::new(
                            room.relay_url.clone(),
                            auth_token,
                        ));
                        client.reconnect_room(
                            &room.room_id,
                            &room.session_id,
                            &room.host_token,
                            "", // share_url not stored in OrphanedRoomInfo; rebuild from ledger
                        ).await;

                        match client.start_streaming(&tmux_name).await {
                            Ok(()) => {
                                tracing::info!("Reconnected to orphaned room {} for session {}", room.room_id, sid);
                                self.pro.relay_clients.insert(sid.clone(), client);

                                // Restore sharing state on the session
                                if let Some(&idx) = self.sessions_by_id.get(&sid) {
                                    if let Some(inst) = self.sessions.get_mut(idx) {
                                        // Rebuild share URL from ledger
                                        let ledger = crate::pro::collab::ledger::RoomLedger::load();
                                        let share_url = ledger.entries.iter()
                                            .find(|e| e.room_id == room.room_id)
                                            .map(|e| e.share_url.clone());

                                        inst.sharing = Some(crate::sharing::SharingState {
                                            active: true,
                                            tmate_socket: String::new(),
                                            links: share_url.iter().map(|url| crate::sharing::ShareLink {
                                                permission: crate::sharing::SharePermission::ReadOnly,
                                                ssh_url: String::new(),
                                                web_url: Some(url.clone()),
                                                created_at: chrono::Utc::now(),
                                                expires_at: None,
                                            }).collect(),
                                            default_permission: crate::sharing::SharePermission::ReadOnly,
                                            started_at: chrono::Utc::now(),
                                            auto_expire_minutes: None,
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to reconnect to room {}: {}", room.room_id, e);
                            }
                        }

                        // Remove this room from the dialog
                        d.rooms.retain(|r| r.room_id != room.room_id);
                        if d.selected_index >= d.rooms.len() && !d.rooms.is_empty() {
                            d.selected_index = d.rooms.len() - 1;
                        }
                        if d.rooms.is_empty() {
                            self.dialog = None;
                            self.state = AppState::Normal;
                        }
                    }
                }
                KeyCode::Char('a') => {
                    // Close all rooms
                    let rooms: Vec<_> = d.rooms.drain(..).collect();
                    for room in &rooms {
                        let closed = crate::pro::collab::client::RelayClient::close_room(
                            &room.relay_url,
                            &room.room_id,
                            &room.host_token,
                        ).await;
                        if closed {
                            let mut ledger = crate::pro::collab::ledger::RoomLedger::load();
                            ledger.remove(&room.room_id);
                        }
                    }
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                _ => {}
            },

            #[cfg(feature = "pro")]
            Dialog::SkillsManager(ref mut d) => {
                use crate::ui::dialogs::{SkillsManagerMode, CreateSkillWizard, CreateSkillField, GroupInputMode};

                // If group input overlay is active, handle its keys first
                if let Some(ref mut gi) = d.group_input {
                    match key {
                        KeyCode::Esc => {
                            if gi.mode == GroupInputMode::LinkWithGroup {
                                // Esc during link = link without group
                                let skill_name = gi.skill_name.clone();
                                d.group_input = None;
                                self.confirm_link_with_group(&skill_name, None).await?;
                            } else {
                                // Esc during edit = cancel
                                d.group_input = None;
                            }
                        }
                        KeyCode::Enter => {
                            let group = if gi.input.is_empty() {
                                None
                            } else {
                                Some(gi.input.text().to_string())
                            };
                            let skill_name = gi.skill_name.clone();
                            let mode = gi.mode;
                            d.group_input = None;
                            match mode {
                                GroupInputMode::LinkWithGroup => {
                                    self.confirm_link_with_group(&skill_name, group).await?;
                                }
                                GroupInputMode::EditGroup => {
                                    self.set_skill_group(&skill_name, group).await?;
                                }
                            }
                        }
                        KeyCode::Backspace => { gi.input.backspace(); }
                        KeyCode::Left => { gi.input.move_left(); }
                        KeyCode::Right => { gi.input.move_right(); }
                        KeyCode::Char(c) => { gi.input.insert(c); }
                        _ => {}
                    }
                    return Ok(());
                }

                // If search input is active, handle its keys
                if let Some(ref mut search) = d.search_input {
                    match key {
                        KeyCode::Esc => {
                            d.search_input = None;
                            // Reset cursor to beginning after closing search
                            d.cursor = crate::ui::dialogs::SkillsCursor { section_idx: 0, item_idx: None };
                        }
                        KeyCode::Backspace => { search.backspace(); }
                        KeyCode::Enter => {
                            // Close search bar but keep the filter text active
                            // (user can see filtered results and navigate)
                            d.toggle_detail();
                        }
                        KeyCode::Up | KeyCode::Char('k') if key == KeyCode::Up => {
                            d.move_cursor(-1);
                        }
                        KeyCode::Down | KeyCode::Char('j') if key == KeyCode::Down => {
                            d.move_cursor(1);
                        }
                        KeyCode::Char(c) => {
                            search.insert(c);
                            // Reset cursor when search text changes
                            d.cursor = crate::ui::dialogs::SkillsCursor { section_idx: 0, item_idx: None };
                        }
                        _ => {}
                    }
                    return Ok(());
                }

                // If create wizard is active, handle its keys
                if let Some(ref mut wizard) = d.create_wizard {
                    match key {
                        KeyCode::Esc => { d.create_wizard = None; }
                        KeyCode::Tab | KeyCode::BackTab => {
                            wizard.active_field = match wizard.active_field {
                                CreateSkillField::Name => CreateSkillField::Description,
                                CreateSkillField::Description => CreateSkillField::Name,
                            };
                        }
                        KeyCode::Enter => {
                            if wizard.active_field == CreateSkillField::Description {
                                self.create_skill_from_wizard().await?;
                            } else {
                                wizard.active_field = CreateSkillField::Description;
                            }
                        }
                        KeyCode::Backspace => {
                            match wizard.active_field {
                                CreateSkillField::Name => wizard.name.backspace(),
                                CreateSkillField::Description => wizard.description.backspace(),
                            };
                        }
                        KeyCode::Left => {
                            match wizard.active_field {
                                CreateSkillField::Name => wizard.name.move_left(),
                                CreateSkillField::Description => wizard.description.move_left(),
                            };
                        }
                        KeyCode::Right => {
                            match wizard.active_field {
                                CreateSkillField::Name => wizard.name.move_right(),
                                CreateSkillField::Description => wizard.description.move_right(),
                            };
                        }
                        KeyCode::Char(c) => {
                            match wizard.active_field {
                                CreateSkillField::Name => wizard.name.insert(c),
                                CreateSkillField::Description => wizard.description.insert(c),
                            };
                        }
                        _ => {}
                    }
                    return Ok(());
                }

                // If init wizard is active, handle its keys
                if let Some(ref mut init_wiz) = d.init_wizard {
                    use crate::ui::dialogs::InitWizardStatus;
                    match key {
                        KeyCode::Esc => {
                            d.init_wizard = None;
                        }
                        KeyCode::Enter => {
                            match init_wiz.status {
                                InitWizardStatus::InputRepoName => {
                                    self.init_skills_from_wizard().await?;
                                }
                                InitWizardStatus::Done(_) | InitWizardStatus::Failed(_)
                                | InitWizardStatus::GhNotInstalled | InitWizardStatus::GhNotAuthenticated => {
                                    // Dismiss
                                    if let Some(Dialog::SkillsManager(ref mut d)) = self.dialog {
                                        d.init_wizard = None;
                                    }
                                }
                                _ => {}
                            }
                        }
                        KeyCode::Backspace => { init_wiz.repo_name.backspace(); }
                        KeyCode::Left => { init_wiz.repo_name.move_left(); }
                        KeyCode::Right => { init_wiz.repo_name.move_right(); }
                        KeyCode::Home => { init_wiz.repo_name.move_home(); }
                        KeyCode::End => { init_wiz.repo_name.move_end(); }
                        KeyCode::Char(c) => {
                            if init_wiz.status == InitWizardStatus::InputRepoName {
                                init_wiz.repo_name.insert(c);
                            }
                        }
                        _ => {}
                    }
                    return Ok(());
                }

                match key {
                    KeyCode::Esc => {
                        if d.multi_select {
                            // First Esc exits multi-select mode
                            d.multi_select = false;
                            d.selected_skills.clear();
                        } else {
                            self.dialog = None;
                            self.state = AppState::Normal;
                        }
                    }
                    // Navigation
                    KeyCode::Up | KeyCode::Char('k') => {
                        d.move_cursor(-1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        d.move_cursor(1);
                    }
                    // Mode switching
                    KeyCode::Char('H') => {
                        d.switch_mode(SkillsManagerMode::Library);
                    }
                    KeyCode::Char('L') => {
                        d.switch_mode(SkillsManagerMode::Linked);
                    }
                    KeyCode::Char('?') => {
                        d.switch_mode(SkillsManagerMode::Help);
                    }
                    // Section collapse/expand (Space) — in multi-select, toggles selection
                    KeyCode::Char(' ') => {
                        if d.multi_select {
                            if let Some(name) = d.selected_skill_name() {
                                if !d.selected_skills.remove(&name) {
                                    d.selected_skills.insert(name);
                                }
                            }
                        } else {
                            d.toggle_section_collapse();
                        }
                    }
                    // Detail expand (Enter)
                    KeyCode::Enter => {
                        d.toggle_detail();
                    }
                    // Search (/)
                    KeyCode::Char('/') => {
                        d.search_input = Some(TextInput::new());
                        d.cursor = crate::ui::dialogs::SkillsCursor { section_idx: 0, item_idx: None };
                    }
                    // Multi-select toggle (v)
                    KeyCode::Char('v') => {
                        d.multi_select = !d.multi_select;
                        d.selected_skills.clear();
                    }
                    // Set group (G)
                    KeyCode::Char('G') => {
                        if let Some(skill) = d.selected_skill() {
                            if skill.linked {
                                let skill_name = skill.name.clone();
                                d.group_input = Some(crate::ui::dialogs::GroupInputState {
                                    input: TextInput::new(),
                                    skill_name,
                                    mode: GroupInputMode::EditGroup,
                                });
                            }
                        }
                    }
                    // Link (i) — in multi-select, batch link
                    KeyCode::Char('i') => {
                        if d.multi_select {
                            self.batch_link_selected_skills().await?;
                        } else {
                            self.link_selected_skill().await?;
                        }
                    }
                    // Unlink (x) — in multi-select, batch unlink
                    KeyCode::Char('x') => {
                        if d.multi_select {
                            self.batch_unlink_selected_skills().await?;
                        } else {
                            self.unlink_selected_skill().await?;
                        }
                    }
                    // Sync (S)
                    KeyCode::Char('S') => {
                        self.sync_skills_repo().await?;
                    }
                    // Create (C)
                    KeyCode::Char('C') => {
                        if let Some(Dialog::SkillsManager(ref mut d)) = self.dialog {
                            d.create_wizard = Some(CreateSkillWizard {
                                name: TextInput::new(),
                                description: TextInput::new(),
                                active_field: CreateSkillField::Name,
                            });
                        }
                    }
                    // Init repo (I) — only when no repo configured
                    KeyCode::Char('I') => {
                        if let Some(Dialog::SkillsManager(ref mut d)) = self.dialog {
                            if !d.repo_configured && d.init_wizard.is_none() {
                                d.init_wizard = Some(crate::ui::dialogs::InitSkillsWizard {
                                    repo_name: TextInput::with_text("agent-skills".to_string()),
                                    status: crate::ui::dialogs::InitWizardStatus::InputRepoName,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            },

            #[cfg(feature = "max")]
            Dialog::AiAnalysis(ref mut d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    d.move_selection(-1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    d.move_selection(1);
                }
                KeyCode::Enter => {
                    let mode = d.selected_mode();
                    let session_id = d.session_id.clone();
                    let session_title = d.session_title.clone();

                    // Close the dialog
                    self.dialog = None;
                    self.state = AppState::Normal;

                    // Dispatch based on mode
                    match mode {
                        AiAnalysisMode::Summary => {
                            self.run_ai_summary(session_id, session_title).await;
                        }
                        AiAnalysisMode::AsciiDiagram => {
                            self.run_ai_diagram(session_id, session_title).await;
                        }
                        AiAnalysisMode::CanvasDiagram => {
                            self.run_ai_canvas_diagram(session_id, session_title).await;
                        }
                    }
                }
                _ => {}
            },

            #[cfg(feature = "max")]
            Dialog::BehaviorAnalysis(ref mut d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Enter => {
                    let session_id = d.session_id.clone();
                    let session_title = d.session_title.clone();
                    let intent_text = d.intent.text().to_string();
                    let user_intent = if intent_text.trim().is_empty() {
                        None
                    } else {
                        Some(intent_text)
                    };

                    self.dialog = None;
                    self.state = AppState::Normal;

                    self.run_behavior_analysis(session_id, session_title, user_intent).await;
                }
                KeyCode::Backspace => { d.intent.backspace(); }
                KeyCode::Delete => { d.intent.delete(); }
                KeyCode::Left => { d.intent.move_left(); }
                KeyCode::Right => { d.intent.move_right(); }
                KeyCode::Home => { d.intent.move_home(); }
                KeyCode::End => { d.intent.move_end(); }
                KeyCode::Char(c) => { d.intent.insert(c); }
                _ => {}
            },

            Dialog::Settings(d) => {
                // Key capture mode: next keypress becomes the new binding
                if d.key_capturing {
                    match key {
                        KeyCode::Esc => {
                            d.key_capturing = false;
                        }
                        _ => {
                            if let Some(action) = d.field.key_action() {
                                let effective_mods = if let KeyCode::Char(c) = key {
                                    if c.is_ascii_uppercase() {
                                        modifiers - crossterm::event::KeyModifiers::SHIFT
                                    } else {
                                        modifiers
                                    }
                                } else {
                                    modifiers
                                };
                                let new_spec = crate::config::KeySpec {
                                    code: key,
                                    modifiers: effective_mods,
                                };
                                d.key_bindings.insert(action, vec![new_spec]);
                                d.key_capturing = false;
                                d.dirty = true;
                            }
                        }
                    }
                    return Ok(());
                }

                if d.editing {
                    // Hook status edit mode: up/down cycles tools, Enter toggles
                    #[cfg(feature = "pro")]
                    if d.field == SettingsField::NotifHookStatus {
                        match key {
                            KeyCode::Esc => {
                                d.editing = false;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                d.cycle_hook_tool(-1);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                d.cycle_hook_tool(1);
                            }
                            KeyCode::Enter => {
                                d.toggle_selected_hook();
                            }
                            _ => {}
                        }
                        return Ok(());
                    }

                    // Edit mode: route keys based on field type
                    let is_selector = d.field.is_selector();

                    if is_selector {
                        // Selector edit mode: ←/→ cycles, Enter/Esc exits
                        match key {
                            KeyCode::Esc | KeyCode::Enter => {
                                d.editing = false;
                            }
                            KeyCode::Left | KeyCode::Char('h') => match d.field {
                                SettingsField::AiProvider => d.cycle_provider(-1),
                                SettingsField::DefaultPermission => d.toggle_permission(),
                                SettingsField::AnimationsEnabled => {
                                    d.animations_enabled = !d.animations_enabled;
                                    d.dirty = true;
                                }
                                SettingsField::PromptCollection => {
                                    d.prompt_collection = !d.prompt_collection;
                                    d.dirty = true;
                                }
                                SettingsField::AnalyticsEnabled => {
                                    d.analytics_enabled = !d.analytics_enabled;
                                    d.dirty = true;
                                }
                                SettingsField::MouseCapture => {
                                    d.mouse_capture_mode = (d.mouse_capture_mode + 2) % 3; // cycle backward
                                    d.dirty = true;
                                }
                                SettingsField::Language => {
                                    d.language_idx = (d.language_idx + 1) % 2; // cycle backward (0<->1)
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifAutoRegister => {
                                    d.hook_auto_register = !d.hook_auto_register;
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifEnabled => {
                                    d.notif_enabled = !d.notif_enabled;
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifSoundPack => d.cycle_pack(-1),
                                #[cfg(feature = "pro")]
                                SettingsField::NotifOnComplete => {
                                    d.notif_on_complete = !d.notif_on_complete;
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifOnInput => {
                                    d.notif_on_input = !d.notif_on_input;
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifOnError => {
                                    d.notif_on_error = !d.notif_on_error;
                                    d.dirty = true;
                                }
                                _ => {}
                            },
                            KeyCode::Right | KeyCode::Char('l') => match d.field {
                                SettingsField::AiProvider => d.cycle_provider(1),
                                SettingsField::DefaultPermission => d.toggle_permission(),
                                SettingsField::AnimationsEnabled => {
                                    d.animations_enabled = !d.animations_enabled;
                                    d.dirty = true;
                                }
                                SettingsField::PromptCollection => {
                                    d.prompt_collection = !d.prompt_collection;
                                    d.dirty = true;
                                }
                                SettingsField::AnalyticsEnabled => {
                                    d.analytics_enabled = !d.analytics_enabled;
                                    d.dirty = true;
                                }
                                SettingsField::MouseCapture => {
                                    d.mouse_capture_mode = (d.mouse_capture_mode + 1) % 3; // cycle forward
                                    d.dirty = true;
                                }
                                SettingsField::Language => {
                                    d.language_idx = (d.language_idx + 1) % 2; // cycle forward (0<->1)
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifAutoRegister => {
                                    d.hook_auto_register = !d.hook_auto_register;
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifEnabled => {
                                    d.notif_enabled = !d.notif_enabled;
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifSoundPack => d.cycle_pack(1),
                                #[cfg(feature = "pro")]
                                SettingsField::NotifOnComplete => {
                                    d.notif_on_complete = !d.notif_on_complete;
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifOnInput => {
                                    d.notif_on_input = !d.notif_on_input;
                                    d.dirty = true;
                                }
                                #[cfg(feature = "pro")]
                                SettingsField::NotifOnError => {
                                    d.notif_on_error = !d.notif_on_error;
                                    d.dirty = true;
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    } else {
                        // TextInput edit mode
                        match key {
                            KeyCode::Esc | KeyCode::Enter => {
                                d.editing = false;
                            }
                            KeyCode::Backspace => {
                                if let Some(input) = d.active_input() {
                                    input.backspace();
                                    d.dirty = true;
                                }
                            }
                            KeyCode::Delete => {
                                if let Some(input) = d.active_input() {
                                    input.delete();
                                    d.dirty = true;
                                }
                            }
                            KeyCode::Left => {
                                if let Some(input) = d.active_input() {
                                    input.move_left();
                                }
                            }
                            KeyCode::Right => {
                                if let Some(input) = d.active_input() {
                                    input.move_right();
                                }
                            }
                            KeyCode::Home => {
                                if let Some(input) = d.active_input() {
                                    input.move_home();
                                }
                            }
                            KeyCode::End => {
                                if let Some(input) = d.active_input() {
                                    input.move_end();
                                }
                            }
                            KeyCode::Char(c) => {
                                if let Some(input) = d.active_input() {
                                    input.insert(c);
                                    d.dirty = true;
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    // Navigation mode: ←/→ always switch tabs
                    match key {
                        KeyCode::Esc => {
                            // Auto-save if settings were modified
                            if let Some(Dialog::Settings(d)) = self.dialog.as_ref() {
                                if d.dirty {
                                    self.apply_settings().await?;
                                }
                            }
                            self.transition_engine.request_transition();
                            self.dialog = None;
                            self.state = AppState::Normal;
                        }
                        KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => {
                            self.apply_settings().await?;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                                d.move_field(-1);
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                                d.move_field(1);
                            }
                        }
                        KeyCode::Left | KeyCode::Char('h') => {
                            if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                                d.switch_tab(-1);
                            }
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                                d.switch_tab(1);
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                                match d.field {
                                    // Selectors: Enter activates edit mode
                                    f if f.is_selector() => {
                                        d.editing = true;
                                    }
                                    // Test: trigger test
                                    SettingsField::AiTest => {
                                        self.test_ai_connection().await;
                                    }
                                    // Hook status: enter edit mode (tool selection)
                                    #[cfg(feature = "pro")]
                                    SettingsField::NotifHookStatus => {
                                        d.editing = true;
                                    }
                                    // Test sound: play a test notification
                                    SettingsField::NotifTestSound => {
                                        self.test_notification_sound();
                                    }
                                    // Pack browser: handled after match to avoid borrow issues
                                    SettingsField::NotifPackLink => {}
                                    // Key binding fields: Enter activates capture mode
                                    f if f.is_key_binding() => {
                                        d.key_capturing = true;
                                    }
                                    // Text inputs: Enter activates edit mode
                                    f if f.is_text_input() => {
                                        d.editing = true;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Deferred: open pack browser after Settings match arm releases borrows
        if key == KeyCode::Enter {
            if let Some(Dialog::Settings(d)) = self.dialog.as_ref() {
                if !d.editing && d.field == SettingsField::NotifPackLink {
                    self.open_pack_browser().await;
                }
            }
        }

        Ok(())
    }

    /// Handle keys in help mode
    pub(super) fn handle_help_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                self.help_visible = false;
                self.state = AppState::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    // AI key handlers (run_ai_summary, run_behavior_analysis, run_ai_diagram, etc.)
    // are defined in pro/src/ui/keys_max.rs
}
