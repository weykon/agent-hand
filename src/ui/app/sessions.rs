use super::*;

impl App {
    fn session_indices_grouped_by_path(
        &self,
    ) -> (
        Vec<usize>,
        std::collections::BTreeMap<String, Vec<usize>>,
    ) {
        use std::collections::BTreeMap;

        let mut by_group: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut ungrouped: Vec<usize> = Vec::new();

        for (i, s) in self.sessions.iter().enumerate() {
            if s.group_path.is_empty() {
                ungrouped.push(i);
            } else {
                by_group.entry(s.group_path.clone()).or_default().push(i);
            }
        }

        ungrouped.sort_by(|a, b| self.sessions[*a].title.cmp(&self.sessions[*b].title));
        for v in by_group.values_mut() {
            v.sort_by(|a, b| self.sessions[*a].title.cmp(&self.sessions[*b].title));
        }

        (ungrouped, by_group)
    }

    pub(super) fn ordered_session_indices_by_group_baseline(&self) -> Vec<usize> {
        let (ungrouped, by_group) = self.session_indices_grouped_by_path();
        let mut ordered = ungrouped;

        let mut roots: Vec<String> = self
            .groups
            .all_groups()
            .into_iter()
            .map(|g| g.path)
            .filter(|p| !p.contains('/'))
            .collect();
        roots.sort();

        fn visit(
            app: &App,
            ordered: &mut Vec<usize>,
            by_group: &std::collections::BTreeMap<String, Vec<usize>>,
            path: &str,
        ) {
            let mut children = app.groups.children(path);
            children.sort();
            for c in children {
                visit(app, ordered, by_group, &c);
            }

            if let Some(sessions) = by_group.get(path) {
                ordered.extend(sessions.iter().copied());
            }
        }

        for r in roots {
            visit(self, &mut ordered, &by_group, &r);
        }

        // Fallback for sessions whose group_path exists in session data but is
        // missing from the current group tree for any reason.
        let mut seen: std::collections::HashSet<usize> = ordered.iter().copied().collect();
        for sessions in by_group.values() {
            for &idx in sessions {
                if seen.insert(idx) {
                    ordered.push(idx);
                }
            }
        }

        ordered
    }

    pub(super) fn build_resume_command_for_session(
        &self,
        session: &crate::session::Instance,
        cli_session_id: &str,
    ) -> Result<String> {
        let original_command = session.command.clone();
        let skip_perms = match session.tool {
            crate::tmux::Tool::Claude => {
                self.config.claude.dangerously_skip_permissions
                    || original_command.contains("--dangerously-skip-permissions")
            }
            crate::tmux::Tool::Codex => {
                self.config.codex.full_auto
                    || original_command.contains("--full-auto")
            }
            crate::tmux::Tool::Gemini => {
                self.config.gemini.yolo
                    || original_command.contains("--yolo")
                    || original_command.contains("-y")
            }
            _ => false,
        };
        let extra_flags = crate::tmux::resume_adapter::extract_preserve_flags(&original_command);

        match crate::tmux::resume_adapter::build_resume_command(
            session.tool,
            cli_session_id,
            skip_perms,
            &extra_flags,
        ) {
            Some(spec) => Ok(spec.command),
            None => Err(crate::Error::config(
                "resume not supported for this tool type",
            )),
        }
    }


    /// Resolve the canvas group from the current tree selection.
    /// If a group is selected, returns that group path.
    /// If a session is selected, returns that session's group_path.
    /// Falls back to "default".
    #[cfg(feature = "pro")]
    pub(super) fn resolve_canvas_group(&self) -> String {
        match self.selected_tree_item() {
            Some(TreeItem::Group { path, .. }) => path.clone(),
            Some(TreeItem::Session { id, .. } | TreeItem::Relationship { id, .. }) => {
                self.sessions.iter()
                    .find(|s| s.id == *id)
                    .map(|s| s.group_path.clone())
                    .unwrap_or_else(|| "default".to_string())
            }
            None => "default".to_string(),
        }
    }

    pub(super) fn group_session_ids(&self, group_path: &str) -> Vec<String> {
        let prefix = format!("{}/", group_path);
        self.sessions
            .iter()
            .filter(|s| s.group_path == group_path || s.group_path.starts_with(&prefix))
            .map(|s| s.id.clone())
            .collect()
    }

    /// Create a relationship workspace session (sessionC) when a relationship
    /// between sessionA and sessionB is established.
    #[cfg(feature = "pro")]
    pub(super) async fn create_relationship_session(
        &mut self,
        relationship: &crate::session::Relationship,
        session_a: &crate::session::Instance,
        session_b: &crate::session::Instance,
    ) -> Result<String> {
        let title = relationship
            .label
            .as_deref()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .unwrap_or_else(|| format!("{} ⇄ {}", session_a.title, session_b.title));

        let group_path = if session_a.group_path.is_empty() {
            "relationships".to_string()
        } else {
            format!("{}/relationships", session_a.group_path)
        };

        let mut inst = Instance::new(title, session_a.project_path.clone());
        inst.group_path = group_path.clone();
        inst.tool = crate::tmux::Tool::Shell;
        inst.relationship_id = Some(relationship.id.clone());

        let session_id = inst.id.clone();

        let storage = self.storage.lock().await;
        let (mut instances, mut tree, relationships) = storage.load().await?;
        tree.create_group(group_path.clone());
        // Auto-expand the relationships sub-group
        let parts: Vec<&str> = group_path.split('/').collect();
        for i in 1..=parts.len() {
            let p = parts[..i].join("/");
            tree.set_expanded(&p, true);
        }
        instances.push(inst);
        storage.save(&instances, &tree, &relationships).await?;

        Ok(session_id)
    }

    pub(super) async fn create_fork_session(
        &mut self,
        parent_session_id: &str,
        project_path: std::path::PathBuf,
        title: &str,
        group_path: &str,
    ) -> Result<String> {
        let parent = self
            .session_by_id(parent_session_id)
            .cloned()
            .ok_or_else(|| crate::Error::InvalidInput("Parent session not found".to_string()))?;

        let title = if title.trim().is_empty() {
            format!("{} (fork)", parent.title)
        } else {
            title.trim().to_string()
        };
        let group_path = group_path.trim().to_string();

        let mut inst = Instance::new(title, project_path);
        inst.group_path = group_path;
        inst.command = parent.command.clone();
        inst.tool = parent.tool;
        inst.parent_session_id = Some(parent_session_id.to_string());

        // Copy parent's CLI session IDs so fork can auto-resume
        inst.claude_session_id = parent.claude_session_id.clone();
        inst.claude_detected_at = parent.claude_detected_at;
        inst.codex_session_id = parent.codex_session_id.clone();
        inst.codex_detected_at = parent.codex_detected_at;
        inst.gemini_session_id = parent.gemini_session_id.clone();
        inst.gemini_detected_at = parent.gemini_detected_at;
        inst.pending_cli_session_id = parent.pending_cli_session_id.clone();

        let storage = self.storage.lock().await;
        let (mut instances, tree, relationships) = storage.load().await?;
        instances.push(inst.clone());
        storage.save(&instances, &tree, &relationships).await?;

        Ok(inst.id)
    }

    pub(super) async fn apply_create_group(&mut self, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();
        if group_path.is_empty() {
            return Ok(());
        }

        let storage = self.storage.lock().await;
        let (instances, mut tree, relationships) = storage.load().await?;

        tree.create_group(group_path.to_string());

        let parts: Vec<&str> = group_path.split('/').collect();
        for i in 1..=parts.len() {
            let p = parts[..i].join("/");
            tree.set_expanded(&p, true);
        }

        storage.save(&instances, &tree, &relationships).await?;
        Ok(())
    }

    pub(super) async fn apply_delete_group_prefix(&mut self, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();
        if group_path.is_empty() {
            return Ok(());
        }

        let storage = self.storage.lock().await;
        let (instances, mut tree, relationships) = storage.load().await?;

        tree.delete_group_prefix(group_path);

        storage.save(&instances, &tree, &relationships).await?;
        Ok(())
    }

    pub(super) async fn apply_delete_group_keep_sessions(&mut self, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();
        if group_path.is_empty() {
            return Ok(());
        }

        let prefix = format!("{}/", group_path);

        let storage = self.storage.lock().await;
        let (mut instances, mut tree, relationships) = storage.load().await?;

        for inst in instances.iter_mut() {
            if inst.group_path == group_path || inst.group_path.starts_with(&prefix) {
                inst.group_path.clear();
            }
        }

        tree.delete_group_prefix(group_path);
        storage.save(&instances, &tree, &relationships).await?;
        Ok(())
    }

    pub(super) async fn apply_delete_group_and_sessions(&mut self, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();
        if group_path.is_empty() {
            return Ok(());
        }

        let prefix = format!("{}/", group_path);

        let storage = self.storage.lock().await;
        let (mut instances, mut tree, relationships) = storage.load().await?;

        // Kill tmux sessions (best-effort) before removing from storage.
        for inst in instances.iter() {
            if inst.group_path == group_path || inst.group_path.starts_with(&prefix) {
                let tmux_name = inst.tmux_name();
                if self.tmux.session_exists(&tmux_name).unwrap_or(false) {
                    let _ = self.tmux.kill_session(&tmux_name).await;
                }
            }
        }

        instances.retain(|s| !(s.group_path == group_path || s.group_path.starts_with(&prefix)));

        tree.delete_group_prefix(group_path);
        storage.save(&instances, &tree, &relationships).await?;
        Ok(())
    }

    pub(super) async fn apply_move_group(&mut self, session_id: &str, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();

        let storage = self.storage.lock().await;
        let (mut instances, mut tree, relationships) = storage.load().await?;

        if let Some(inst) = instances.iter_mut().find(|s| s.id == session_id) {
            inst.group_path = group_path.to_string();
        }

        if !group_path.is_empty() {
            tree.create_group(group_path.to_string());

            // Auto-expand so it becomes visible immediately.
            let parts: Vec<&str> = group_path.split('/').collect();
            for i in 1..=parts.len() {
                let p = parts[..i].join("/");
                tree.set_expanded(&p, true);
            }
        }

        storage.save(&instances, &tree, &relationships).await?;
        Ok(())
    }

    pub(super) async fn apply_edit_session(
        &mut self,
        session_id: &str,
        old_title: &str,
        new_title: &str,
        label: &str,
        label_color: crate::session::LabelColor,
        cli_session_id_override: Option<&str>,
    ) -> Result<()> {
        let title = new_title.trim();
        let title = if title.is_empty() { old_title } else { title };
        let label = label.trim();

        let storage = self.storage.lock().await;
        let (mut instances, tree, relationships) = storage.load().await?;

        if let Some(inst) = instances.iter_mut().find(|s| s.id == session_id) {
            let old_tmux_name = inst.tmux_name();
            inst.title = title.to_string();
            inst.label = label.to_string();
            inst.label_color = label_color;

            // Apply manual CLI session ID override if provided
            if let Some(sid) = cli_session_id_override {
                let sid = sid.trim();
                if !sid.is_empty() {
                    inst.set_cli_session_id(sid, chrono::Utc::now());
                }
            }

            // Update tmux session name if title changed
            if title != old_title {
                let new_tmux_name = TmuxManager::build_session_name(title, &inst.id);
                inst.tmux_session_name = Some(new_tmux_name.clone());

                // Rename the live tmux session if it exists
                if self.tmux.session_exists(&old_tmux_name).unwrap_or(false) {
                    let _ = self.tmux.rename_session(&old_tmux_name, &new_tmux_name).await;
                    let _ = self.tmux.set_session_title(&new_tmux_name, title).await;
                }
            }
        }

        storage.save(&instances, &tree, &relationships).await?;
        Ok(())
    }

    pub(super) async fn apply_rename_group(&mut self, old_path: &str, new_path: &str) -> Result<()> {
        let old_path = old_path.trim();
        let new_path = new_path.trim();
        if old_path.is_empty() || new_path.is_empty() || old_path == new_path {
            return Ok(());
        }

        let storage = self.storage.lock().await;
        let (mut instances, mut tree, relationships) = storage.load().await?;

        let old_slash = format!("{}/", old_path);
        for inst in instances.iter_mut() {
            if inst.group_path == old_path || inst.group_path.starts_with(&old_slash) {
                let suffix = &inst.group_path[old_path.len()..];
                inst.group_path = format!("{new_path}{suffix}");
            }
        }

        tree.rename_prefix(old_path, new_path);
        storage.save(&instances, &tree, &relationships).await?;

        // Rename corresponding canvas files (Pro only)
        #[cfg(feature = "pro")]
        if let Some(ref dir) = self.pro.canvas_dir {
            use crate::ui::canvas::canvas_filename_for_group;
            if let Ok(entries) = std::fs::read_dir(dir) {
                let old_stem = canvas_filename_for_group(old_path);
                let old_prefix = format!("{old_stem}__");
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    let Some(base) = name_str.strip_suffix(".json") else { continue };
                    if base == old_stem {
                        // Exact match: rename the group's own canvas file
                        let new_stem = canvas_filename_for_group(new_path);
                        let new_file = dir.join(format!("{new_stem}.json"));
                        let _ = std::fs::rename(entry.path(), new_file);
                    } else if base.starts_with(&old_prefix) {
                        // Child group: e.g. "work__frontend__sub" → "newwork__frontend__sub"
                        let suffix = &base[old_stem.len()..];
                        let new_stem = canvas_filename_for_group(new_path);
                        let new_file = dir.join(format!("{new_stem}{suffix}.json"));
                        let _ = std::fs::rename(entry.path(), new_file);
                    }
                }
            }
            // Update current canvas_group if it was affected
            if self.pro.canvas_group == old_path || self.pro.canvas_group.starts_with(&old_slash) {
                let suffix = &self.pro.canvas_group[old_path.len()..];
                self.pro.canvas_group = format!("{new_path}{suffix}");
            }
        }

        Ok(())
    }

    pub(super) async fn create_session_from_dialog(&mut self) -> Result<()> {
        let Some(Dialog::NewSession(d)) = self.dialog.as_ref() else {
            return Ok(());
        };

        let project_path = d.validate()?;
        let title = if d.title.text().trim().is_empty() {
            project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled")
                .to_string()
        } else {
            d.title.text().trim().to_string()
        };

        let storage = self.storage.lock().await;
        let (mut instances, mut tree, relationships) = storage.load().await?;

        let mut instance = Instance::new(title.clone(), project_path.clone());
        let group_path = d.group_path.text().trim();
        if !group_path.is_empty() {
            instance.group_path = group_path.to_string();
            tree.create_group(instance.group_path.clone());
        }

        instance.command.clear();
        instance.tool = crate::tmux::Tool::Shell;

        instances.push(instance);
        storage.save(&instances, &tree, &relationships).await?;

        Ok(())
    }

    pub(super) async fn delete_session(&mut self, session_id: &str, kill_tmux: bool) -> Result<()> {
        let tmux_name = self.tmux_name_for_id(session_id);

        if kill_tmux && self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            if let Err(e) = self.tmux.kill_session(&tmux_name).await {
                tracing::warn!("Failed to kill tmux session {}: {}", tmux_name, e);
            }
        }

        let storage = self.storage.lock().await;
        let (mut instances, tree, relationships) = storage.load().await?;
        let before = instances.len();
        instances.retain(|s| s.id != session_id);
        if instances.len() != before {
            storage.save(&instances, &tree, &relationships).await?;
        }

        Ok(())
    }

    pub(super) fn ensure_groups_exist(&mut self) {
        for s in &self.sessions {
            if !s.group_path.is_empty() {
                self.groups.create_group(s.group_path.clone());
            }
        }
    }

    pub(super) fn rebuild_sessions_index(&mut self) {
        self.sessions_by_id = self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();
    }

    pub(super) fn rebuild_tree(&mut self) {
        let (ungrouped, by_group) = self.session_indices_grouped_by_path();

        let mut items: Vec<TreeItem> = Vec::new();

        // Root sessions
        for si in ungrouped {
            let session = &self.sessions[si];
            if let Some(ref rel_id) = session.relationship_id {
                items.push(TreeItem::Relationship {
                    id: session.id.clone(),
                    rel_id: rel_id.clone(),
                    depth: 0,
                });
            } else {
                items.push(TreeItem::Session {
                    id: session.id.clone(),
                    depth: 0,
                });
            }
        }

        // Root groups
        let mut roots: Vec<String> = self
            .groups
            .all_groups()
            .into_iter()
            .map(|g| g.path)
            .filter(|p| !p.contains('/'))
            .collect();
        roots.sort();

        fn visit(
            app: &App,
            items: &mut Vec<TreeItem>,
            by_group: &std::collections::BTreeMap<String, Vec<usize>>,
            path: &str,
            depth: usize,
        ) {
            let name = app
                .groups
                .get_group(path)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| path.split('/').last().unwrap_or(path).to_string());

            items.push(TreeItem::Group {
                path: path.to_string(),
                name,
                depth,
            });

            if !app.groups.is_expanded(path) {
                return;
            }

            let mut children = app.groups.children(path);
            children.sort();
            for c in children {
                visit(app, items, by_group, &c, depth + 1);
            }

            if let Some(sessions) = by_group.get(path) {
                for &si in sessions {
                    let session = &app.sessions[si];
                    if let Some(ref rel_id) = session.relationship_id {
                        items.push(TreeItem::Relationship {
                            id: session.id.clone(),
                            rel_id: rel_id.clone(),
                            depth: depth + 1,
                        });
                    } else {
                        items.push(TreeItem::Session {
                            id: session.id.clone(),
                            depth: depth + 1,
                        });
                    }
                }
            }
        }

        for r in roots {
            visit(self, &mut items, &by_group, &r, 0);
        }

        self.tree = items;
    }

    pub(super) async fn toggle_selected_group(&mut self, desired: Option<bool>) -> Result<bool> {
        let path = match self.selected_tree_item() {
            Some(TreeItem::Group { path, .. }) => path.clone(),
            _ => return Ok(false),
        };

        let current = self.groups.is_expanded(&path);
        let next = desired.unwrap_or(!current);
        if next == current {
            return Ok(false);
        }

        self.groups.set_expanded(&path, next);

        let storage = self.storage.lock().await;
        storage.save(&self.sessions, &self.groups, &self.relationships).await?;
        drop(storage);

        self.rebuild_tree();
        Ok(true)
    }

    /// Start selected session
    pub(super) async fn start_selected(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let tmux_session = session.tmux_name();

            if !self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                // Prefer resume if session has a stored CLI session ID
                let resume_cmd = session
                    .cli_session_id()
                    .and_then(|sid| {
                        self.build_resume_command_for_session(session, sid).ok()
                    });

                let cmd = resume_cmd.as_deref().or_else(|| {
                    let c = session.command.as_str();
                    if c.trim().is_empty() { None } else { Some(c) }
                });

                if let Err(e) = self
                    .tmux
                    .create_session(
                        &tmux_session,
                        &session.project_path.to_string_lossy(),
                        cmd,
                        Some(&session.title),
                    )
                    .await
                {
                    self.preview = format!(
                        "{}\n\nPath: {}\nLabel: {}\n\nFailed to start tmux session:\n{}",
                        session.title,
                        session.project_path.to_string_lossy(),
                        session.label,
                        e
                    );
                    return Ok(());
                }

                self.refresh_sessions().await?;
            }
        }
        Ok(())
    }

    /// Stop selected session
    pub(super) async fn stop_selected(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let tmux_session = session.tmux_name();

            if self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                self.tmux.kill_session(&tmux_session).await?;
                self.refresh_sessions().await?;
            }
        }
        Ok(())
    }

    /// Resume selected session's CLI conversation.
    /// Only reconstructs a missing/stopped tmux pane; it does not inject a shell
    /// resume command into an already-live REPL.
    pub(super) async fn resume_selected(&mut self) -> Result<()> {
        let session = match self.selected_session() {
            Some(s) => s,
            None => return Ok(()),
        };

        let cli_session_id = session.cli_session_id().map(|s| s.to_string());
        let tmux_name = session.tmux_name();
        let project_path = session.project_path.to_string_lossy().to_string();
        let title = session.title.clone();

        let Some(sid) = cli_session_id else {
            let msg = if session.tool == crate::tmux::Tool::Shell {
                "Tool not detected yet — start the session and run a CLI tool first"
            } else {
                "No CLI session ID captured yet — interact with the session first"
            };
            self.preview = format!("{}\n\n{}", title, msg);
            self.set_info_bar(msg.to_string(), ratatui::style::Color::Yellow);
            return Ok(());
        };

        let resume_cmd = match self.build_resume_command_for_session(session, &sid) {
            Ok(cmd) => cmd,
            Err(_) => {
                self.preview = format!("{}\n\nResume not supported for this tool type.", title);
                self.set_info_bar(
                    "Resume unavailable for this tool type".to_string(),
                    ratatui::style::Color::Yellow,
                );
                return Ok(());
            }
        };

        if self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            self.preview = format!(
                "{}\n\nSession already exists in tmux.\nAttach with Enter to continue the live REPL.\nUse 'R' to rebuild the pane and resume the stored CLI conversation.\n\nSession: {}",
                title, sid
            );
            self.set_info_bar(
                "Session pane already exists — Enter attaches, R rebuilds with resume"
                    .to_string(),
                ratatui::style::Color::Cyan,
            );
        } else {
            self.tmux
                .create_session(
                    &tmux_name,
                    &project_path,
                    Some(&resume_cmd),
                    Some(&title),
                )
                .await?;
            self.refresh_sessions().await?;
            self.set_info_bar(
                format!("Resumed session from stored CLI ID: {}", sid),
                ratatui::style::Color::Green,
            );
        }

        Ok(())
    }

    /// Restart selected session (prefers rebuilding with resume if session ID is available)
    pub(super) async fn restart_selected(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let has_sid = session.cli_session_id().is_some();
            if has_sid {
                let tmux_name = session.tmux_name();
                let title = session.title.clone();
                let project_path = session.project_path.to_string_lossy().to_string();
                let sid = session.cli_session_id().unwrap().to_string();
                let resume_cmd = match self.build_resume_command_for_session(session, &sid) {
                    Ok(cmd) => cmd,
                    Err(_) => {
                        self.stop_selected().await?;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        self.start_selected().await?;
                        return Ok(());
                    }
                };

                if self.tmux.session_exists(&tmux_name).unwrap_or(false) {
                    self.stop_selected().await?;
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
                self.tmux
                    .create_session(&tmux_name, &project_path, Some(&resume_cmd), Some(&title))
                    .await?;
                self.refresh_sessions().await?;
                self.set_info_bar(
                    format!("Rebuilt pane and resumed CLI conversation: {}", sid),
                    ratatui::style::Color::Green,
                );
            } else {
                self.stop_selected().await?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                self.start_selected().await?;
            }
        }
        Ok(())
    }

    /// Refresh sessions data
    pub(super) async fn refresh_sessions(&mut self) -> Result<()> {
        let storage = self.storage.lock().await;
        let (sessions, groups, relationships) = storage.load().await?;
        drop(storage);

        self.sessions = sessions;
        self.groups = groups;
        self.relationships = relationships;

        self.ensure_groups_exist();
        self.rebuild_sessions_index();
        self.rebuild_tree();

        // Refresh tmux cache (rate-limited). tmux can fail transiently; avoid crashing the TUI.
        if self.last_cache_refresh.elapsed() >= Self::CACHE_REFRESH {
            let _ = self.tmux.refresh_cache().await;
            self.last_cache_refresh = Instant::now();
        }

        // Drop stale activity entries after reload
        self.last_tmux_activity
            .retain(|id, _| self.sessions_by_id.contains_key(id));

        // Update session statuses (rate-limited in refresh_statuses). Avoid crashing on tmux errors.
        let _ = self.refresh_statuses().await;
        self.last_status_refresh = Instant::now();

        // Clamp selected index
        if self.selected_index >= self.tree.len() && !self.tree.is_empty() {
            self.selected_index = self.tree.len() - 1;
        }

        if self.state == AppState::Search {
            self.update_search_results();
        }

        self.update_preview().await?;

        Ok(())
    }

    /// Poll the background relay share connection task.
    /// Called every tick (~250ms) to check if the spawned task has completed.
    #[cfg(feature = "pro")]
    pub(super) async fn poll_share_task(&mut self) -> Result<()> {
        let rx = match self.pro.share_task_rx.as_mut() {
            Some(rx) => rx,
            None => return Ok(()),
        };

        match rx.try_recv() as std::result::Result<std::result::Result<super::ShareTaskResult, super::ShareTaskError>, _> {
            Ok(Ok(result)) => {
                self.pro.share_task_rx = None;
                self.activity.complete(super::activity::ActivityOp::StartingShare);

                // Store relay client to keep background streaming alive
                self.pro.relay_clients.insert(result.session_id.clone(), result.relay_client);

                // Persist room credentials for orphan recovery
                {
                    let mut ledger = crate::pro::collab::ledger::RoomLedger::load();
                    ledger.add(crate::pro::collab::ledger::RoomLedgerEntry {
                        room_id: result.room_id.clone(),
                        session_id: result.session_id.clone(),
                        relay_url: result.relay_url.clone(),
                        host_token: result.host_token.clone(),
                        share_url: result.share_url.clone(),
                        created_at: chrono::Utc::now().to_rfc3339(),
                    });
                }

                // Update dialog UI
                if let Some(Dialog::Share(ref mut d)) = self.dialog {
                    d.relay_share_url = Some(result.share_url.clone());
                    d.relay_room_id = Some(result.room_id.clone());
                    d.web_url = Some(result.share_url.clone());
                    d.already_sharing = true;
                    d.status_message = None; // clear — "● Sharing active" takes over
                }

                // Update session sharing state
                let sharing_state = crate::sharing::SharingState {
                    active: true,
                    tmate_socket: String::new(),
                    links: vec![crate::sharing::ShareLink {
                        permission: result.permission,
                        ssh_url: String::new(),
                        web_url: Some(result.share_url),
                        created_at: chrono::Utc::now(),
                        expires_at: None,
                    }],
                    default_permission: result.permission,
                    started_at: chrono::Utc::now(),
                    auto_expire_minutes: result.expire_minutes,
                };

                if let Some(inst) = self
                    .sessions
                    .iter_mut()
                    .find(|s| s.id == result.session_id)
                {
                    inst.sharing = Some(sharing_state);
                }
                let storage = self.storage.lock().await;
                storage
                    .save(&self.sessions, &self.groups, &self.relationships)
                    .await?;
                drop(storage);
                let _ = self
                    .analytics
                    .record_premium_event(
                        crate::analytics::EventType::ShareStart,
                        &result.session_id,
                        &result.session_title,
                    )
                    .await;
            }
            Ok(Err(error)) => {
                self.pro.share_task_rx = None;
                self.activity.complete(super::activity::ActivityOp::StartingShare);
                if let Some(Dialog::Share(ref mut d)) = self.dialog {
                    d.status_message = Some(format!("✗ {}", error.message));
                    d.web_url = Some(format!("Error: {}", error.message));
                }
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                // Task still running — spinner continues
            }
            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                // Sender dropped without sending — task panicked or was cancelled
                self.pro.share_task_rx = None;
                self.activity.complete(super::activity::ActivityOp::StartingShare);
                if let Some(Dialog::Share(ref mut d)) = self.dialog {
                    d.status_message = Some("✗ Connection task failed".to_string());
                }
            }
        }
        Ok(())
    }
}
