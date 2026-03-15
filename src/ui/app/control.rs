use super::*;
use crate::control::{
    ControlOp, ControlResponse, GroupInfo, RelationshipInfo, SessionInfo,
};

impl App {
    /// Handle a single control operation from the control socket.
    pub(super) async fn handle_control_op(&mut self, op: ControlOp) -> ControlResponse {
        match op {
            // ── Session CRUD ──────────────────────────────────────
            ControlOp::AddSession {
                path,
                title,
                group,
                command,
            } => self.ctrl_add_session(path, title, group, command).await,

            ControlOp::RemoveSession { id } => self.ctrl_remove_session(&id).await,

            ControlOp::ListSessions { group, tag, status } => {
                self.ctrl_list_sessions(group, tag, status)
            }

            ControlOp::SessionInfo { id } => self.ctrl_session_info(&id),

            // ── Session lifecycle ─────────────────────────────────
            ControlOp::StartSession { id } => self.ctrl_start_session(&id).await,
            ControlOp::StopSession { id } => self.ctrl_stop_session(&id).await,
            ControlOp::RestartSession { id } => self.ctrl_restart_session(&id).await,
            ControlOp::ResumeSession { id } => self.ctrl_resume_session(&id).await,
            ControlOp::InterruptSession { id } => self.ctrl_interrupt_session(&id).await,
            ControlOp::SendPrompt { id, text } => self.ctrl_send_prompt(&id, &text).await,

            // ── Session metadata ──────────────────────────────────
            ControlOp::RenameSession { id, title } => {
                self.ctrl_rename_session(&id, &title).await
            }
            ControlOp::SetLabel { id, label, color } => {
                self.ctrl_set_label(&id, &label, color).await
            }
            ControlOp::MoveSession { id, group } => {
                self.ctrl_move_session(&id, &group).await
            }
            ControlOp::AddTag { id, tag } => self.ctrl_add_tag(&id, &tag).await,
            ControlOp::RemoveTag { id, tag } => self.ctrl_remove_tag(&id, &tag).await,

            // ── Groups ────────────────────────────────────────────
            ControlOp::ListGroups => self.ctrl_list_groups(),
            ControlOp::CreateGroup { path } => self.ctrl_create_group(&path).await,
            ControlOp::DeleteGroup { path } => self.ctrl_delete_group(&path).await,
            ControlOp::RenameGroup { old_path, new_path } => {
                self.ctrl_rename_group(&old_path, &new_path).await
            }

            // ── Relationships (Pro) ────────────────────────────────
            #[cfg(feature = "pro")]
            ControlOp::AddRelationship {
                session_a,
                session_b,
                relation_type,
                label,
            } => self.ctrl_add_relationship(&session_a, &session_b, relation_type, label).await,

            #[cfg(not(feature = "pro"))]
            ControlOp::AddRelationship { .. } => ControlResponse::Error {
                message: "relationships require Pro".into(),
            },

            #[cfg(feature = "pro")]
            ControlOp::RemoveRelationship { id } => self.ctrl_remove_relationship(&id).await,

            #[cfg(not(feature = "pro"))]
            ControlOp::RemoveRelationship { .. } => ControlResponse::Error {
                message: "relationships require Pro".into(),
            },

            ControlOp::ListRelationships { session } => {
                self.ctrl_list_relationships(session)
            }

            // ── Session inspection ────────────────────────────────
            ControlOp::ReadPane { id, lines } => self.ctrl_read_pane(&id, lines).await,
            ControlOp::ReadProgress { id } => self.ctrl_read_progress(&id).await,

            // ── Status ────────────────────────────────────────────
            ControlOp::Status => self.ctrl_status(),

            // ── Batch ─────────────────────────────────────────────
            ControlOp::Batch { ops } => {
                let mut results = Vec::with_capacity(ops.len());
                for sub_op in ops {
                    // Use Box::pin to allow recursive async call
                    results.push(Box::pin(self.handle_control_op(sub_op)).await);
                }
                ControlResponse::BatchResult { results }
            }
        }
    }

    // ── Session CRUD ──────────────────────────────────────────────────────

    async fn ctrl_add_session(
        &mut self,
        path: String,
        title: Option<String>,
        group: Option<String>,
        command: Option<String>,
    ) -> ControlResponse {
        let project_path = std::path::PathBuf::from(&path);
        let title = title.unwrap_or_else(|| {
            project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });

        let mut instance = Instance::new(title, project_path);
        if let Some(g) = group {
            instance.group_path = g;
        }
        if let Some(c) = command {
            instance.command = c;
        }

        let id = instance.id.clone();

        let storage = self.storage.lock().await;
        let (mut instances, mut tree, relationships) = match storage.load().await {
            Ok(data) => data,
            Err(e) => return ControlResponse::Error { message: format!("storage error: {e}") },
        };

        if !instance.group_path.is_empty() {
            tree.create_group(instance.group_path.clone());
        }
        instances.push(instance);

        if let Err(e) = storage.save(&instances, &tree, &relationships).await {
            return ControlResponse::Error { message: format!("save error: {e}") };
        }
        drop(storage);

        let _ = self.refresh_sessions().await;

        ControlResponse::Ok {
            message: format!("session created: {id}"),
        }
    }

    async fn ctrl_remove_session(&mut self, id: &str) -> ControlResponse {
        if self.session_by_id(id).is_none() {
            return ControlResponse::Error {
                message: format!("session not found: {id}"),
            };
        }

        match self.delete_session(id, true).await {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("session removed: {id}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("delete error: {e}"),
            },
        }
    }

    fn ctrl_list_sessions(
        &self,
        group: Option<String>,
        tag: Option<String>,
        status: Option<String>,
    ) -> ControlResponse {
        let sessions: Vec<SessionInfo> = self
            .sessions
            .iter()
            .filter(|s| {
                if let Some(ref g) = group {
                    if s.group_path != *g && !s.group_path.starts_with(&format!("{g}/")) {
                        return false;
                    }
                }
                if let Some(ref t) = tag {
                    if !s.has_tag(t) {
                        return false;
                    }
                }
                if let Some(ref st) = status {
                    let session_status = format!("{:?}", s.status).to_lowercase();
                    if session_status != *st {
                        return false;
                    }
                }
                true
            })
            .map(SessionInfo::from_instance)
            .collect();

        ControlResponse::SessionList { sessions }
    }

    fn ctrl_session_info(&self, id: &str) -> ControlResponse {
        match self.session_by_id(id) {
            Some(inst) => ControlResponse::Session {
                session: SessionInfo::from_instance(inst),
            },
            None => ControlResponse::Error {
                message: format!("session not found: {id}"),
            },
        }
    }

    // ── Session lifecycle ─────────────────────────────────────────────────

    async fn ctrl_start_session(&mut self, id: &str) -> ControlResponse {
        let session = match self.session_by_id(id) {
            Some(s) => s,
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                }
            }
        };

        let tmux_name = session.tmux_name();
        if self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            return ControlResponse::Ok {
                message: format!("session already running: {id}"),
            };
        }

        let project_path = session.project_path.to_string_lossy().to_string();
        let command = if session.command.trim().is_empty() {
            None
        } else {
            Some(session.command.clone())
        };
        let title = session.title.clone();

        match self
            .tmux
            .create_session(
                &tmux_name,
                &project_path,
                command.as_deref(),
                Some(&title),
            )
            .await
        {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("session started: {id}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("start error: {e}"),
            },
        }
    }

    async fn ctrl_stop_session(&mut self, id: &str) -> ControlResponse {
        let session = match self.session_by_id(id) {
            Some(s) => s,
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                }
            }
        };

        let tmux_name = session.tmux_name();
        if !self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            return ControlResponse::Ok {
                message: format!("session not running: {id}"),
            };
        }

        match self.tmux.kill_session(&tmux_name).await {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("session stopped: {id}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("stop error: {e}"),
            },
        }
    }

    async fn ctrl_restart_session(&mut self, id: &str) -> ControlResponse {
        let can_resume = match self.session_by_id(id) {
            Some(s) => {
                s.cli_session_id().is_some()
            }
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                };
            }
        };

        if can_resume {
            let session = match self.session_by_id(id) {
                Some(s) => s,
                None => {
                    return ControlResponse::Error {
                        message: format!("session not found: {id}"),
                    };
                }
            };
            let tmux_name = session.tmux_name();
            let title = session.title.clone();
            let project_path = session.project_path.to_string_lossy().to_string();
            let sid = session.cli_session_id().unwrap().to_string();
            let resume_cmd = match self.build_resume_command_for_session(session, &sid) {
                Ok(cmd) => cmd,
                Err(e) => {
                    return ControlResponse::Error {
                        message: format!("resume build error: {e}"),
                    };
                }
            };

            if self.tmux.session_exists(&tmux_name).unwrap_or(false) {
                let stop_result = self.ctrl_stop_session(id).await;
                if matches!(stop_result, ControlResponse::Error { .. }) {
                    return stop_result;
                }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }

            match self
                .tmux
                .create_session(&tmux_name, &project_path, Some(&resume_cmd), Some(&title))
                .await
            {
                Ok(_) => {
                    let _ = self.refresh_sessions().await;
                    ControlResponse::Ok {
                        message: format!("session restarted with resume: {id}"),
                    }
                }
                Err(e) => ControlResponse::Error {
                    message: format!("restart resume create_session error: {e}"),
                },
            }
        } else {
            let stop_result = self.ctrl_stop_session(id).await;
            if matches!(stop_result, ControlResponse::Error { .. }) {
                return stop_result;
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            self.ctrl_start_session(id).await
        }
    }

    async fn ctrl_resume_session(&mut self, id: &str) -> ControlResponse {
        let session = match self.session_by_id(id) {
            Some(s) => s,
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                };
            }
        };

        let cli_session_id = match session.cli_session_id() {
            Some(sid) => sid.to_string(),
            None => {
                return ControlResponse::Error {
                    message: format!("no session ID available for resume: {id}"),
                };
            }
        };

        let tmux_name = session.tmux_name();
        let project_path = session.project_path.to_string_lossy().to_string();
        let title = session.title.clone();

        let resume_cmd = match self.build_resume_command_for_session(session, &cli_session_id) {
            Ok(cmd) => cmd,
            Err(_) => {
                return ControlResponse::Error {
                    message: format!("resume not supported for this tool type: {id}"),
                };
            }
        };

        if self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            ControlResponse::Ok {
                message:
                    format!("session pane already exists: {id} (attach instead, or restart to rebuild)"),
            }
        } else {
            match self
                .tmux
                .create_session(&tmux_name, &project_path, Some(&resume_cmd), Some(&title))
                .await
            {
                Ok(_) => {
                    let _ = self.refresh_sessions().await;
                    ControlResponse::Ok {
                        message: format!("session resumed (new tmux): {id}"),
                    }
                }
                Err(e) => ControlResponse::Error {
                    message: format!("resume create_session error: {e}"),
                },
            }
        }
    }

    async fn ctrl_interrupt_session(&mut self, id: &str) -> ControlResponse {
        let session = match self.session_by_id(id) {
            Some(s) => s,
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                };
            }
        };

        let tmux_name = session.tmux_name();
        if !self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            return ControlResponse::Error {
                message: format!("session not running (no tmux): {id}"),
            };
        }

        match self.tmux.send_interrupt(&tmux_name).await {
            Ok(_) => ControlResponse::Ok {
                message: format!("interrupt sent: {id}"),
            },
            Err(e) => ControlResponse::Error {
                message: format!("interrupt error: {e}"),
            },
        }
    }

    async fn ctrl_send_prompt(&mut self, id: &str, text: &str) -> ControlResponse {
        let session = match self.session_by_id(id) {
            Some(s) => s,
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                };
            }
        };

        // Status guard: only allow if Idle or Waiting
        match session.status {
            Status::Idle | Status::Waiting => {}
            _ => {
                return ControlResponse::Error {
                    message: format!(
                        "session must be idle or waiting to send prompt (current: {:?}): {id}",
                        session.status
                    ),
                };
            }
        }

        let tmux_name = session.tmux_name();
        if !self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            return ControlResponse::Error {
                message: format!("session not running (no tmux): {id}"),
            };
        }

        match self.tmux.send_keys(&tmux_name, text).await {
            Ok(_) => ControlResponse::Ok {
                message: format!("prompt sent: {id}"),
            },
            Err(e) => ControlResponse::Error {
                message: format!("send_keys error: {e}"),
            },
        }
    }

    // ── Session metadata ──────────────────────────────────────────────────

    async fn ctrl_rename_session(&mut self, id: &str, title: &str) -> ControlResponse {
        let old_title = match self.session_by_id(id) {
            Some(s) => s.title.clone(),
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                }
            }
        };

        let inst = self.session_by_id(id).unwrap();
        let label = inst.label.clone();
        let label_color = inst.label_color;

        match self
            .apply_edit_session(id, &old_title, title, &label, label_color, None)
            .await
        {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("session renamed: {id}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("rename error: {e}"),
            },
        }
    }

    async fn ctrl_set_label(
        &mut self,
        id: &str,
        label: &str,
        color: Option<crate::session::LabelColor>,
    ) -> ControlResponse {
        let inst = match self.session_by_id(id) {
            Some(s) => s,
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                }
            }
        };
        let title = inst.title.clone();
        let color = color.unwrap_or(inst.label_color);

        match self
            .apply_edit_session(id, &title, &title, label, color, None)
            .await
        {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("label set: {id}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("label error: {e}"),
            },
        }
    }

    async fn ctrl_move_session(&mut self, id: &str, group: &str) -> ControlResponse {
        if self.session_by_id(id).is_none() {
            return ControlResponse::Error {
                message: format!("session not found: {id}"),
            };
        }

        match self.apply_move_group(id, group).await {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("session moved to {group}: {id}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("move error: {e}"),
            },
        }
    }

    async fn ctrl_add_tag(&mut self, id: &str, tag: &str) -> ControlResponse {
        let storage = self.storage.lock().await;
        let (mut instances, tree, relationships) = match storage.load().await {
            Ok(data) => data,
            Err(e) => return ControlResponse::Error { message: format!("storage error: {e}") },
        };

        if let Some(inst) = instances.iter_mut().find(|s| s.id == id) {
            inst.add_tag(tag);
            if let Err(e) = storage.save(&instances, &tree, &relationships).await {
                return ControlResponse::Error { message: format!("save error: {e}") };
            }
            drop(storage);
            let _ = self.refresh_sessions().await;
            ControlResponse::Ok {
                message: format!("tag added: {tag}"),
            }
        } else {
            ControlResponse::Error {
                message: format!("session not found: {id}"),
            }
        }
    }

    async fn ctrl_remove_tag(&mut self, id: &str, tag: &str) -> ControlResponse {
        let storage = self.storage.lock().await;
        let (mut instances, tree, relationships) = match storage.load().await {
            Ok(data) => data,
            Err(e) => return ControlResponse::Error { message: format!("storage error: {e}") },
        };

        if let Some(inst) = instances.iter_mut().find(|s| s.id == id) {
            if inst.remove_tag(tag) {
                if let Err(e) = storage.save(&instances, &tree, &relationships).await {
                    return ControlResponse::Error { message: format!("save error: {e}") };
                }
                drop(storage);
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("tag removed: {tag}"),
                }
            } else {
                ControlResponse::Ok {
                    message: format!("tag not present: {tag}"),
                }
            }
        } else {
            ControlResponse::Error {
                message: format!("session not found: {id}"),
            }
        }
    }

    // ── Session inspection ────────────────────────────────────────────────

    async fn ctrl_read_pane(&self, id: &str, lines: usize) -> ControlResponse {
        let session = match self.session_by_id(id) {
            Some(s) => s,
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                }
            }
        };

        let tmux_name = session.tmux_name();
        if !self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            return ControlResponse::Error {
                message: format!("session not running (no tmux): {id}"),
            };
        }

        match self.tmux.capture_pane(&tmux_name, lines).await {
            Ok(content) => ControlResponse::TextContent { content },
            Err(e) => ControlResponse::Error {
                message: format!("capture_pane error: {e}"),
            },
        }
    }

    async fn ctrl_read_progress(&self, id: &str) -> ControlResponse {
        let session = match self.session_by_id(id) {
            Some(s) => s,
            None => {
                return ControlResponse::Error {
                    message: format!("session not found: {id}"),
                }
            }
        };

        let tmux_name = session.tmux_name();
        let progress_dir = match crate::session::Storage::get_agent_hand_dir() {
            Ok(d) => d,
            Err(e) => {
                return ControlResponse::Error {
                    message: format!("cannot resolve agent-hand dir: {e}"),
                }
            }
        };
        // Progress files are stored at ~/.agent-hand/profiles/{profile}/progress/{tmux_name}.md
        // Try default profile path
        let progress_file = progress_dir
            .join("profiles")
            .join("default")
            .join("progress")
            .join(format!("{}.md", tmux_name));

        match tokio::fs::read_to_string(&progress_file).await {
            Ok(content) => ControlResponse::TextContent { content },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ControlResponse::Error {
                message: format!("no progress file for session: {id}"),
            },
            Err(e) => ControlResponse::Error {
                message: format!("read progress error: {e}"),
            },
        }
    }

    // ── Groups ────────────────────────────────────────────────────────────

    fn ctrl_list_groups(&self) -> ControlResponse {
        let groups: Vec<GroupInfo> = self
            .groups
            .all_groups()
            .into_iter()
            .map(|g| {
                let session_count = self
                    .sessions
                    .iter()
                    .filter(|s| s.group_path == g.path || s.group_path.starts_with(&format!("{}/", g.path)))
                    .count();
                GroupInfo {
                    path: g.path,
                    name: g.name,
                    session_count,
                }
            })
            .collect();

        ControlResponse::GroupList { groups }
    }

    async fn ctrl_create_group(&mut self, path: &str) -> ControlResponse {
        match self.apply_create_group(path).await {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("group created: {path}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("create error: {e}"),
            },
        }
    }

    async fn ctrl_delete_group(&mut self, path: &str) -> ControlResponse {
        match self.apply_delete_group_keep_sessions(path).await {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("group deleted: {path}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("delete error: {e}"),
            },
        }
    }

    async fn ctrl_rename_group(&mut self, old_path: &str, new_path: &str) -> ControlResponse {
        match self.apply_rename_group(old_path, new_path).await {
            Ok(_) => {
                let _ = self.refresh_sessions().await;
                ControlResponse::Ok {
                    message: format!("group renamed: {old_path} -> {new_path}"),
                }
            }
            Err(e) => ControlResponse::Error {
                message: format!("rename error: {e}"),
            },
        }
    }

    // ── Relationships ─────────────────────────────────────────────────────

    #[cfg(feature = "pro")]
    async fn ctrl_add_relationship(
        &mut self,
        session_a: &str,
        session_b: &str,
        relation_type: Option<String>,
        label: Option<String>,
    ) -> ControlResponse {
        if session_a == session_b {
            return ControlResponse::Error {
                message: "cannot create relationship between a session and itself".to_string(),
            };
        }
        if self.session_by_id(session_a).is_none() {
            return ControlResponse::Error {
                message: format!("session not found: {session_a}"),
            };
        }
        if self.session_by_id(session_b).is_none() {
            return ControlResponse::Error {
                message: format!("session not found: {session_b}"),
            };
        }

        let rtype = relation_type
            .as_deref()
            .map(crate::control::parse_relation_type)
            .unwrap_or(crate::session::RelationType::Peer);

        let mut rel = crate::session::Relationship::new(
            rtype,
            session_a.to_string(),
            session_b.to_string(),
        );
        if let Some(l) = label {
            rel = rel.with_label(l);
        }
        let rel_id = rel.id.clone();

        crate::session::relationships::add_relationship(&mut self.relationships, rel);

        let storage = self.storage.lock().await;
        if let Err(e) = storage
            .save(&self.sessions, &self.groups, &self.relationships)
            .await
        {
            return ControlResponse::Error {
                message: format!("save error: {e}"),
            };
        }
        drop(storage);

        ControlResponse::Ok {
            message: format!("relationship created: {rel_id}"),
        }
    }

    #[cfg(feature = "pro")]
    async fn ctrl_remove_relationship(&mut self, id: &str) -> ControlResponse {
        let removed = crate::session::relationships::remove_relationship(
            &mut self.relationships,
            id,
        );
        if removed.is_none() {
            return ControlResponse::Error {
                message: format!("relationship not found: {id}"),
            };
        }

        let storage = self.storage.lock().await;
        if let Err(e) = storage
            .save(&self.sessions, &self.groups, &self.relationships)
            .await
        {
            return ControlResponse::Error {
                message: format!("save error: {e}"),
            };
        }
        drop(storage);

        ControlResponse::Ok {
            message: format!("relationship removed: {id}"),
        }
    }

    fn ctrl_list_relationships(
        &self,
        session: Option<String>,
    ) -> ControlResponse {
        let rels: Vec<RelationshipInfo> = self
            .relationships
            .iter()
            .filter(|r| {
                if let Some(ref s) = session {
                    r.involves_session(s)
                } else {
                    true
                }
            })
            .map(RelationshipInfo::from_relationship)
            .collect();

        ControlResponse::RelationshipList {
            relationships: rels,
        }
    }

    // ── Status ────────────────────────────────────────────────────────────

    fn ctrl_status(&self) -> ControlResponse {
        let total = self.sessions.len();
        let running = self
            .sessions
            .iter()
            .filter(|s| matches!(s.status, Status::Running))
            .count();
        let waiting = self
            .sessions
            .iter()
            .filter(|s| matches!(s.status, Status::Waiting))
            .count();
        let idle = self
            .sessions
            .iter()
            .filter(|s| matches!(s.status, Status::Idle))
            .count();
        let error = self
            .sessions
            .iter()
            .filter(|s| matches!(s.status, Status::Error))
            .count();

        ControlResponse::StatusReport {
            total,
            running,
            waiting,
            idle,
            error,
        }
    }
}
