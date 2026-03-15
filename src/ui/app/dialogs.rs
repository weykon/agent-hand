use super::*;

impl App {




    /// Open the AI analysis mode picker dialog (Max tier).

    pub(super) fn open_fork_dialog(&mut self) {
        let Some(parent) = self.selected_session() else {
            return;
        };

        let title = format!("{} (fork)", parent.title);

        self.dialog = Some(Dialog::Fork(ForkDialog {
            parent_session_id: parent.id.clone(),
            project_path: parent.project_path.clone(),
            title: TextInput::with_text(title),
            group_path: TextInput::with_text(parent.group_path.clone()),
            field: ForkField::Title,
        }));
        self.state = AppState::Dialog;
    }

    pub(super) fn open_create_group_dialog(&mut self) {
        let mut all_groups: Vec<String> = self
            .groups
            .all_groups()
            .into_iter()
            .map(|g| g.path)
            .collect();
        all_groups.sort();
        all_groups.dedup();

        let mut d = CreateGroupDialog {
            input: TextInput::new(),
            all_groups,
            matches: Vec::new(),
            selected: 0,
        };
        d.update_matches();

        self.dialog = Some(Dialog::CreateGroup(d));
        self.state = AppState::Dialog;
    }

    pub(super) fn open_move_group_dialog(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
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

        let mut d = MoveGroupDialog {
            session_id: s.id.clone(),
            title: s.title.clone(),
            input: TextInput::with_text(s.group_path.clone()),
            all_groups,
            matches: Vec::new(),
            selected: 0,
        };
        d.update_matches();

        self.dialog = Some(Dialog::MoveGroup(d));
        self.state = AppState::Dialog;
    }

    pub(super) fn open_rename_session_dialog(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
        };

        let cli_sid = s.cli_session_id().unwrap_or("").to_string();

        self.dialog = Some(Dialog::RenameSession(RenameSessionDialog {
            session_id: s.id.clone(),
            old_title: s.title.clone(),
            new_title: TextInput::with_text(s.title.clone()),
            label: TextInput::with_text(s.label.clone()),
            label_color: s.label_color,
            cli_session_id: TextInput::with_text(cli_sid),
            field: SessionEditField::Title,
        }));
        self.state = AppState::Dialog;
    }

    pub(super) fn collect_existing_tags(&self) -> Vec<TagSpec> {
        let mut out: Vec<TagSpec> = Vec::new();
        let mut seen: std::collections::HashMap<String, ()> = std::collections::HashMap::new();
        for s in &self.sessions {
            let name = s.label.trim();
            if name.is_empty() {
                continue;
            }
            let key = format!("{}|{:?}", name, s.label_color);
            if seen.insert(key, ()).is_none() {
                out.push(TagSpec {
                    name: name.to_string(),
                    color: s.label_color,
                });
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    }

    pub(super) fn open_tag_picker_dialog(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
        };

        let tags = self.collect_existing_tags();
        let mut selected = 0usize;
        if !tags.is_empty() {
            if let Some(i) = tags
                .iter()
                .position(|t| t.name == s.label && t.color == s.label_color)
            {
                selected = i;
            }
        }

        self.dialog = Some(Dialog::TagPicker(TagPickerDialog {
            session_id: s.id.clone(),
            tags,
            selected,
        }));
        self.state = AppState::Dialog;
    }

    pub(super) fn open_rename_group_dialog(&mut self) {
        let Some(TreeItem::Group { path, .. }) = self.selected_tree_item() else {
            return;
        };

        self.dialog = Some(Dialog::RenameGroup(RenameGroupDialog {
            old_path: path.clone(),
            new_path: TextInput::with_text(path.clone()),
        }));
        self.state = AppState::Dialog;
    }

    /// Apply settings from the dialog: update config, save to disk, hot-reload subsystems.
    pub(super) async fn apply_settings(&mut self) -> Result<()> {
        let Some(Dialog::Settings(d)) = self.dialog.as_ref() else {
            return Ok(());
        };

        // Update AI config
        #[cfg(feature = "pro")]
        {
            if let Some(name) = d.ai_provider_names.get(d.ai_provider_idx) {
                self.config.ai.provider = name.clone();
            }
            self.config.ai.api_key = d.ai_api_key.text().to_string();
            self.config.ai.model = d.ai_model.text().to_string();
            let base_url = d.ai_base_url.text().trim().to_string();
            self.config.ai.base_url = if base_url.is_empty() {
                None
            } else {
                Some(base_url)
            };
            self.config.ai.summary_lines = d
                .ai_summary_lines
                .text()
                .trim()
                .parse()
                .unwrap_or(200);
        }

        // Update sharing config
        let relay = d.relay_url.text().trim().to_string();
        self.config.sharing.relay_server_url = if relay.is_empty() {
            None
        } else {
            Some(relay)
        };
        self.config.sharing.default_permission = d.default_permission.clone();
        let expire = d.auto_expire.text().trim().to_string();
        self.config.sharing.auto_expire_minutes = if expire.is_empty() {
            None
        } else {
            expire.parse().ok()
        };

        // Update hooks config
        {
            self.config.hooks.auto_register = d.hook_auto_register;
        }

        // Update notification config
        {
            if let Some(pack_name) = d.notif_pack_names.get(d.notif_pack_idx) {
                self.config.notification.sound_pack = pack_name.clone();
            }
            self.config.notification.enabled = d.notif_enabled;
            self.config.notification.on_task_complete = d.notif_on_complete;
            self.config.notification.on_input_required = d.notif_on_input;
            self.config.notification.on_error = d.notif_on_error;
            let vol_pct: f32 = d.notif_volume.text().trim().parse().unwrap_or(50.0);
            self.config.notification.volume = (vol_pct / 100.0).clamp(0.0, 1.0);
        }

        // Update general config
        self.config.animations_enabled = Some(d.animations_enabled);
        self.transition_engine.set_enabled(d.animations_enabled);
        self.config.claude.user_prompt_logging = d.prompt_collection;
        self.config.analytics.enabled = d.analytics_enabled;
        self.config.jump_lines = Some(
            d.jump_lines.text().trim().parse().unwrap_or(10),
        );
        self.config.scroll_padding = Some(
            d.scroll_padding.text().trim().parse().unwrap_or(5),
        );
        self.config.ready_ttl_minutes = Some(
            d.ready_ttl.text().trim().parse().unwrap_or(40),
        );
        let mouse_str = match d.mouse_capture_mode {
            0 => "auto",
            1 => "on",
            _ => "off",
        };
        self.config.mouse_capture = Some(mouse_str.to_string());
        self.mouse_capture_changed = true;

        // Update auto-permission flags
        self.config.claude.dangerously_skip_permissions = d.claude_skip_perms;
        self.config.codex.full_auto = d.codex_full_auto;
        self.config.gemini.yolo = d.gemini_yolo;

        // Update language config
        let lang = match d.language_idx {
            1 => crate::i18n::Language::Chinese,
            _ => crate::i18n::Language::English,
        };
        self.language = lang;
        self.config.language = Some(lang.to_str().to_string());

        // Update keybindings config
        for (action, specs) in &d.key_bindings {
            self.config.set_keybinding(action, specs);
        }

        // Save to disk
        self.config.save()?;

        // Hot-reload: update keybindings
        for (action, specs) in &d.key_bindings {
            self.keybindings.set_binding(action, specs.clone());
        }

        // Hot-reload: update attention TTL
        self.attention_ttl =
            Duration::from_secs(self.config.ready_ttl_minutes() * 60);

        // Hot-reload: update jump_lines & scroll_padding
        #[cfg(feature = "pro")]
        {
            self.pro.jump_lines = self.config.jump_lines();
        }
        self.scroll_padding = self.config.scroll_padding();

        // Hot-reload: update shared notification config (background sound task picks it up)
        {
            if let Ok(mut cfg) = self.sound_config.write() {
                *cfg = self.config.notification().clone();
            }
        }

        // Hot-reload: recreate AI summarizer with new config
        #[cfg(feature = "pro")]
        {
            let is_max = self
                .auth_token
                .as_ref()
                .map_or(false, |t| t.is_max());
            if is_max {
                self.max.summarizer =
                    crate::ai::Summarizer::from_config(self.config.ai());
            }
        }

        // Close dialog
        self.dialog = None;
        self.state = AppState::Normal;
        Ok(())
    }

    /// Open the pack browser dialog and fetch pack list in background.
    pub(super) async fn open_pack_browser(&mut self) {
        let mut browser = crate::ui::dialogs::PackBrowserDialog::new();

        // Fetch pack list
        let relay_url = self.config.sharing.relay_server_url.clone();
        match crate::notification::registry::fetch_pack_list(relay_url.as_deref()).await {
            Ok(packs) => {
                let count = packs.len();
                let installed = packs.iter().filter(|p| p.installed).count();
                browser.packs = packs;
                browser.loading = false;
                browser.status = format!("{} packs available ({} installed)", count, installed);
            }
            Err(e) => {
                browser.loading = false;
                browser.status = format!("Failed to load: {}", e);
            }
        }

        self.dialog = Some(Dialog::PackBrowser(browser));
        self.state = AppState::Dialog;
    }

    /// Install the selected pack from the pack browser.
    pub(super) async fn install_selected_pack(&mut self) {
        let Some(Dialog::PackBrowser(ref mut d)) = self.dialog else {
            return;
        };
        let Some(pack) = d.packs.get(d.selected).cloned() else {
            return;
        };
        if pack.installed {
            d.status = format!("'{}' is already installed", pack.name);
            return;
        }

        d.installing = true;
        d.status = format!("Installing '{}'...", pack.name);

        // We need to drop the mutable borrow before the await
        let pack_name = pack.name.clone();

        match crate::notification::registry::install_pack(&pack_name, |_| {}).await {
            Ok(_) => {
                if let Some(Dialog::PackBrowser(ref mut d)) = self.dialog {
                    d.installing = false;
                    // Mark as installed
                    if let Some(p) = d.packs.get_mut(d.selected) {
                        p.installed = true;
                    }
                    let installed = d.packs.iter().filter(|p| p.installed).count();
                    d.status = format!(
                        "Installed '{}' ! ({}/{} installed)",
                        pack_name,
                        installed,
                        d.packs.len()
                    );
                }
            }
            Err(e) => {
                if let Some(Dialog::PackBrowser(ref mut d)) = self.dialog {
                    d.installing = false;
                    d.status = format!("Install failed: {}", e);
                }
            }
        }
    }

    /// Test sound playback from settings dialog.
    pub(super) fn test_notification_sound(&mut self) {
        let Some(Dialog::Settings(d)) = self.dialog.as_mut() else {
            return;
        };

        // Try to load and play from the currently selected pack
        let pack_name = d.notif_pack_names
            .get(d.notif_pack_idx)
            .cloned()
            .unwrap_or_default();

        if pack_name.is_empty() {
            d.notif_test_status = Some("✗ No sound pack selected".to_string());
            return;
        }

        let pack = crate::notification::SoundPack::load(&pack_name);
        match pack {
            None => {
                d.notif_test_status = Some(format!("✗ Pack '{}' not found", pack_name));
            }
            Some(pack) => {
                // Try task.complete first (most recognizable), fallback to any category
                let sound = pack.pick_sound("task.complete")
                    .or_else(|| pack.pick_sound("session.start"))
                    .or_else(|| pack.pick_sound("input.required"));

                match sound {
                    Some(path) => {
                        let vol_text = d.notif_volume.text().to_string();
                        let volume = vol_text.parse::<f32>().unwrap_or(50.0) / 100.0;
                        crate::notification::sound::play_async(&path, volume);
                        d.notif_test_status = Some(format!("✓ Playing from '{}'", pack_name));
                    }
                    None => {
                        d.notif_test_status = Some(format!("✗ No sounds in pack '{}'", pack_name));
                    }
                }
            }
        }
    }

    /// Test AI connection from settings dialog.
    pub(super) async fn test_ai_connection(&mut self) {
        #[cfg(feature = "pro")]
        {
            let Some(Dialog::Settings(d)) = self.dialog.as_mut() else {
                return;
            };
            let provider_name = d
                .ai_provider_names
                .get(d.ai_provider_idx)
                .cloned()
                .unwrap_or_default();
            let api_key = d.ai_api_key.text().to_string();

            if provider_name.is_empty() || api_key.is_empty() {
                d.ai_test_status = Some("✗ Provider or API key not set".to_string());
                return;
            }

            d.ai_test_status = Some("Testing...".to_string());
            self.activity.push_default(super::activity::ActivityOp::TestingAIConnection);

            let meta = ai_api_provider::provider_by_name(&provider_name);
            if meta.is_none() {
                d.ai_test_status = Some(format!("✗ Unknown provider: {provider_name}"));
                return;
            }
            let meta = meta.unwrap();

            let mut config = ai_api_provider::ApiConfig::new(meta.provider, api_key);
            let model_override = d.ai_model.text().trim().to_string();
            if !model_override.is_empty() {
                config.model = model_override;
            }
            let base_url_text = d.ai_base_url.text().trim().to_string();
            if !base_url_text.is_empty() {
                config.base_url = Some(base_url_text);
            }
            config.max_tokens = 16;

            let client = ai_api_provider::ApiClient::new();
            let messages = vec![ai_api_provider::ChatMessage {
                role: "user".to_string(),
                content: "Say hi in one word.".to_string(),
            }];

            match client.chat(&config, &messages).await {
                Ok(_) => {
                    if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                        d.ai_test_status =
                            Some(format!("✓ Connected ({})", provider_name));
                    }
                }
                Err(e) => {
                    if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                        d.ai_test_status =
                            Some(format!("✗ {}", e));
                    }
                }
            }
            self.activity.complete(super::activity::ActivityOp::TestingAIConnection);
        }

        #[cfg(not(feature = "max"))]
        {
            if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                d.ai_test_status = Some("✗ AI requires Max tier build".to_string());
            }
        }
    }

    // Pro/Max dialog handlers in pro/src/ui/dialog_handlers*.rs
}
