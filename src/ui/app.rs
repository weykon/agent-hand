use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::{Mutex, RwLock};

use crate::error::Result;
use crate::session::{GroupTree, Instance, Relationship, Status, Storage};
use crate::tmux::{
    ptmx::{spawn_ptmx_monitor, SharedPtmxState},
    TmuxManager, SESSION_PREFIX,
};

use super::{
    AppState, CreateGroupDialog, CreateRelationshipDialog, CreateRelationshipField,
    DeleteConfirmDialog, DeleteGroupChoice, DeleteGroupDialog, Dialog, ForkDialog, ForkField,
    MoveGroupDialog, NewSessionDialog, NewSessionField, RenameGroupDialog, RenameSessionDialog,
    SessionEditField, ShareDialog, TagPickerDialog, TagSpec, TextInput, TreeItem,
};

/// Main TUI application
pub struct App {
    // Terminal state
    width: u16,
    height: u16,

    // Application state
    state: AppState,
    should_quit: bool,

    // Data
    sessions: Vec<Instance>,
    sessions_by_id: HashMap<String, usize>,
    groups: GroupTree,
    relationships: Vec<Relationship>,
    selected_relationship_index: usize,
    tree: Vec<TreeItem>,
    selected_index: usize,

    // UI state
    help_visible: bool,
    preview: String,
    preview_cache: HashMap<String, String>,

    // Search state
    search_query: String,
    search_results: Vec<String>,
    search_selected: usize,

    // Dialog state
    dialog: Option<Dialog>,

    // Deferred actions that require terminal access
    pending_attach: Option<String>,

    // Keybindings (configurable via ~/.agent-hand/config.json)
    keybindings: crate::config::KeyBindings,

    // Navigation/perf
    last_navigation_time: Instant,
    is_navigating: bool,
    pending_preview_id: Option<String>,
    last_status_refresh: Instant,
    last_cache_refresh: Instant,

    // Status/probing
    previous_statuses: HashMap<String, Status>,
    last_tmux_activity: HashMap<String, i64>,
    last_tmux_activity_change: HashMap<String, Instant>,
    last_status_probe: HashMap<String, Instant>,
    last_seen_detach_at: Option<String>,
    force_probe_tmux: Option<String>,

    // PTY monitoring (background task + shared state)
    ptmx_state: crate::tmux::ptmx::SharedPtmxState,
    _ptmx_task: tokio::task::JoinHandle<()>,
    cached_ptmx_total: u32,
    cached_ptmx_max: u32,

    // UI animation
    tick_count: u64,
    attention_ttl: Duration,

    // Backend
    storage: Arc<Mutex<Storage>>,
    tmux: Arc<TmuxManager>,
    analytics: crate::analytics::ActivityTracker,

    // Auth
    auth_token: Option<crate::auth::AuthToken>,
}

impl App {
    const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);
    const NAVIGATION_SETTLE: Duration = Duration::from_millis(300);
    const STATUS_REFRESH: Duration = Duration::from_secs(1);
    const CACHE_REFRESH: Duration = Duration::from_secs(2);
    const STATUS_COOLDOWN: Duration = Duration::from_secs(2);
    const STATUS_FALLBACK: Duration = Duration::from_secs(10);

    const DEFAULT_READY_TTL: Duration = Duration::from_secs(40 * 60);

    /// Create new application
    pub async fn new(profile: &str) -> Result<Self> {
        let storage = Storage::new(profile).await?;
        let (mut sessions, groups, relationships) = storage.load().await?;
        // Status is derived from tmux probes; the persisted value can be stale across restarts.
        // Reset to avoid treating old Running→Idle as a fresh completion.
        for s in &mut sessions {
            s.status = Status::Idle;
        }

        let tmux = TmuxManager::new();

        // Clean up orphaned tmux sessions (exist in tmux but not in storage).
        // This prevents PTY leaks from sessions that were deleted but whose tmux
        // process was not properly killed.
        {
            let known_ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
            let killed = tmux.cleanup_orphaned_sessions(&known_ids).await;
            if killed > 0 {
                tracing::info!("Cleaned up {} orphaned tmux session(s)", killed);
            }
        }

        let keybindings = crate::config::KeyBindings::load_or_default().await;
        let analytics = crate::analytics::ActivityTracker::new(profile).await;

        // Get system PTY limit once at startup.
        let system_ptmx_max = crate::tmux::ptmx::get_ptmx_max().await;

        let cfg = crate::config::ConfigFile::load().await.ok().flatten();
        let attention_ttl = Duration::from_secs(
            cfg.as_ref()
                .map(|c| c.ready_ttl_minutes())
                .unwrap_or(Self::DEFAULT_READY_TTL.as_secs() / 60)
                * 60,
        );

        // Create shared PTY state and spawn background monitor
        let ptmx_state: SharedPtmxState = Arc::new(RwLock::new(
            crate::tmux::ptmx::PtmxState {
                system_max: system_ptmx_max,
                ..Default::default()
            }
        ));
        let ptmx_task = spawn_ptmx_monitor(system_ptmx_max, Arc::clone(&ptmx_state));

        let mut app = Self {
            width: 0,
            height: 0,
            state: AppState::Normal,
            should_quit: false,
            sessions,
            sessions_by_id: HashMap::new(),
            groups,
            relationships,
            selected_relationship_index: 0,
            tree: Vec::new(),
            selected_index: 0,
            help_visible: false,
            preview: String::new(),
            preview_cache: HashMap::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            dialog: None,
            pending_attach: None,
            keybindings,
            last_navigation_time: Instant::now(),
            is_navigating: false,
            pending_preview_id: None,
            last_status_refresh: Instant::now(),
            last_cache_refresh: Instant::now(),
            previous_statuses: HashMap::new(),
            last_tmux_activity: HashMap::new(),
            last_tmux_activity_change: HashMap::new(),
            last_status_probe: HashMap::new(),
            last_seen_detach_at: None,
            force_probe_tmux: None,
            tick_count: 0,
            attention_ttl,
            storage: Arc::new(Mutex::new(storage)),
            tmux: Arc::new(tmux),
            analytics,
            ptmx_state,
            _ptmx_task: ptmx_task,
            cached_ptmx_total: 0,
            cached_ptmx_max: system_ptmx_max,
            auth_token: crate::auth::AuthToken::load(),
        };

        app.ensure_groups_exist();
        app.rebuild_tree();
        app.rebuild_sessions_index();

        // Prime tmux cache/status so initial render isn't stale
        app.tmux.ensure_server().await;
        let _ = app.tmux.refresh_cache().await;
        app.last_cache_refresh = Instant::now();
        let _ = app.refresh_statuses().await;
        app.last_status_refresh = Instant::now();
        let _ = app.update_preview().await;

        Ok(app)
    }

    /// Run the TUI application
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.clear()?;

        // Run event loop
        let result = self.event_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Main event loop
    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let tick_rate = Duration::from_millis(250);

        // Initial preview/status
        self.on_navigation();

        loop {
            // Draw UI
            terminal.draw(|f| {
                self.width = f.area().width;
                self.height = f.area().height;
                super::render::draw(f, self);
            })?;

            // Handle events
            if event::poll(tick_rate)? {
                match event::read()? {
                    CrosstermEvent::Key(key) => {
                        self.handle_key(key.code, key.modifiers).await?;
                    }
                    CrosstermEvent::Resize(_, _) => {
                        // Next draw will re-render with new size
                    }
                    _ => {}
                }
            } else {
                // Tick event
                self.tick().await?;
            }

            if let Some(name) = self.pending_attach.take() {
                // Record analytics: session enter
                if let Some(session) = self.find_session_by_tmux_name(&name) {
                    let _ = self
                        .analytics
                        .record_enter(&session.id, &session.title)
                        .await;
                }

                self.perform_attach(terminal, &name).await?;
                let _ = self.cache_preview_by_tmux_name(&name).await;
                self.refresh_sessions().await?;
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn on_navigation(&mut self) {
        self.last_navigation_time = Instant::now();
        self.is_navigating = true;
        self.pending_preview_id = self.selected_session().map(|s| s.id.clone());
    }

    async fn tick(&mut self) -> Result<()> {
        self.tick_count = self.tick_count.wrapping_add(1);

        if self.is_navigating && self.last_navigation_time.elapsed() > Self::NAVIGATION_SETTLE {
            self.is_navigating = false;
        }

        // Debounced path suggestions in New Session dialog
        if self.state == AppState::Dialog {
            if let Some(Dialog::NewSession(d)) = self.dialog.as_mut() {
                if d.field == NewSessionField::Path
                    && d.path_dirty
                    && d.path_last_edit.elapsed() >= Duration::from_millis(250)
                {
                    d.path_dirty = false;
                    d.update_path_suggestions();
                }
            }
        }

        // Auto-expire sharing sessions (check every ~10 ticks = ~2.5s)
        if self.tick_count % 10 == 0 {
            let mut expired_ids = Vec::new();
            for inst in &self.sessions {
                if let Some(ref sharing) = inst.sharing {
                    if sharing.active && sharing.should_auto_expire() {
                        expired_ids.push(inst.id.clone());
                    }
                }
            }
            if !expired_ids.is_empty() {
                let mut mgr = crate::sharing::tmate::TmateManager::new();
                for id in &expired_ids {
                    let _ = mgr.stop_sharing(id).await;
                    if let Some(inst) = self.sessions.iter_mut().find(|s| &s.id == id) {
                        inst.sharing = None;
                    }
                }
                let storage = self.storage.lock().await;
                storage
                    .save(&self.sessions, &self.groups, &self.relationships)
                    .await?;
            }
        }

        // Cheap preview for non-session selections
        if self.selected_session().is_none() {
            return self.update_preview().await;
        }

        if !self.is_navigating {
            if self.last_cache_refresh.elapsed() >= Self::CACHE_REFRESH {
                self.tmux.refresh_cache().await?;
                self.last_cache_refresh = Instant::now();
            }

            if self.last_status_refresh.elapsed() >= Self::STATUS_REFRESH {
                if let Ok(Some(detach_at)) = self
                    .tmux
                    .get_environment_global("AGENTHAND_LAST_DETACH_AT")
                    .await
                {
                    if self.last_seen_detach_at.as_deref() != Some(detach_at.as_str()) {
                        self.last_seen_detach_at = Some(detach_at);
                        // Use cached session name (written by Ctrl+Q binding).
                        if let Ok(Some(name)) = self
                            .tmux
                            .get_environment_global("AGENTHAND_LAST_SESSION")
                            .await
                        {
                            self.force_probe_tmux = Some(name.clone());

                            // Record analytics: session exit (Ctrl+Q detach)
                            if let Some(session) = self.find_session_by_tmux_name(&name) {
                                let _ = self
                                    .analytics
                                    .record_exit(&session.id, &session.title)
                                    .await;
                            }
                        }
                    }
                }

                self.refresh_statuses().await?;
                self.last_status_refresh = Instant::now();
            }
        }

        // Update PTY counts from background task (non-blocking)
        // The background task scans every 30 minutes, we just read the cached state
        {
            let state = self.ptmx_state.read().await;
            for session in &mut self.sessions {
                session.ptmx_count = state.per_session.get(&session.id).copied().unwrap_or(0);
            }
            // Update cached values for synchronous getters
            self.cached_ptmx_total = state.system_total;
            self.cached_ptmx_max = state.system_max;
        }

        if self.pending_preview_id.is_some()
            && self.last_navigation_time.elapsed() >= Self::PREVIEW_DEBOUNCE
        {
            self.pending_preview_id = None;
            self.update_preview().await?;
        }

        Ok(())
    }

    async fn refresh_statuses(&mut self) -> Result<()> {
        let now = Instant::now();
        let selected_id = self.selected_session().map(|s| s.id.clone());

        // Collect session IDs that transition from Running to Idle/Waiting for auto-capture
        let mut running_to_done: Vec<String> = Vec::new();

        for session in &mut self.sessions {
            let tmux_session = TmuxManager::session_name(&session.id);
            if !self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                session.status = Status::Idle;
                self.last_tmux_activity.remove(&session.id);
                self.last_tmux_activity_change.remove(&session.id);
                self.last_status_probe.remove(&session.id);
                continue;
            }

            let activity = self.tmux.session_activity(&tmux_session).unwrap_or(0);
            let prev_activity = self.last_tmux_activity.get(&session.id).copied();

            // Track activity changes (but don't infer Running from it - attach/detach also changes activity)
            let activity_changed = prev_activity.is_some_and(|a| activity > a);
            if activity_changed || prev_activity.is_none() {
                self.last_tmux_activity.insert(session.id.clone(), activity);
                if activity_changed {
                    self.last_tmux_activity_change
                        .insert(session.id.clone(), now);
                }
            }

            // Decide whether to probe this session
            let need_fallback_probe = self
                .last_status_probe
                .get(&session.id)
                .is_none_or(|t| now.duration_since(*t) >= Self::STATUS_FALLBACK);

            let activity_settled = self
                .last_tmux_activity_change
                .get(&session.id)
                .is_some_and(|t| now.duration_since(*t) >= Self::STATUS_COOLDOWN);

            let is_selected = selected_id.as_deref() == Some(session.id.as_str());

            // Probe when:
            // - Fallback timer expired (infrequent probe for all sessions)
            // - Selected session with recent activity that has settled
            // - Activity just changed (something happened, check it)
            // - First observation (need initial status)
            let force_probe = self.force_probe_tmux.as_deref() == Some(tmux_session.as_str());
            let should_probe = force_probe
                || need_fallback_probe
                || (is_selected && activity_settled)
                || activity_changed
                || prev_activity.is_none();

            if !should_probe {
                continue;
            }

            let content = self
                .tmux
                .capture_pane(&tmux_session, 15)
                .await
                .unwrap_or_default();
            let detector = crate::tmux::PromptDetector::new(session.tool);
            let new_status = if detector.has_prompt(&content) {
                Status::Waiting
            } else if detector.is_busy(&content) {
                Status::Running
            } else {
                Status::Idle
            };

            let prev_status = session.status;
            let now_utc = chrono::Utc::now();

            // Record last_running_at when we detect Running or when Running just ended.
            if new_status == Status::Running
                || (prev_status == Status::Running && new_status == Status::Idle)
            {
                session.last_running_at = Some(now_utc);
            }

            // Record last_waiting_at on transition into Waiting
            if new_status == Status::Waiting && prev_status != Status::Waiting {
                session.last_waiting_at = Some(chrono::Utc::now());
            }

            // Detect Running -> Idle/Waiting transition using previous_statuses
            let tracked_prev = self.previous_statuses.get(&session.id).copied();
            if tracked_prev == Some(Status::Running)
                && (new_status == Status::Idle || new_status == Status::Waiting)
            {
                running_to_done.push(session.id.clone());
            }

            // Update previous_statuses tracking
            self.previous_statuses.insert(session.id.clone(), new_status);

            session.status = new_status;
            self.last_status_probe.insert(session.id.clone(), now);
            if force_probe {
                self.force_probe_tmux = None;
            }
        }

        // Auto-capture context for sessions that transitioned from Running to Idle/Waiting
        if !running_to_done.is_empty()
            && crate::auth::AuthToken::require_feature("auto_context").is_ok()
        {
            let profile = {
                let storage = self.storage.lock().await;
                storage.profile().to_string()
            };
            let collector = crate::session::context::ContextCollector::new(&profile);

            for session_id in &running_to_done {
                let rels = crate::session::relationships::find_relationships_for_session(
                    &self.relationships,
                    session_id,
                );
                if rels.is_empty() {
                    continue;
                }

                // Capture pane output once for this session
                let tmux_name = TmuxManager::session_name(session_id);
                let pane_content = self
                    .tmux
                    .capture_pane(&tmux_name, 200)
                    .await
                    .unwrap_or_default();
                if pane_content.is_empty() {
                    continue;
                }

                // Save a snapshot for each relationship this session is part of
                for rel in rels {
                    let snapshot =
                        crate::session::context::ContextSnapshot::pane_capture(
                            session_id,
                            pane_content.clone(),
                        )
                        .with_relationship(&rel.id)
                        .with_tags(vec![
                            "auto_capture".to_string(),
                            "status_transition".to_string(),
                        ]);
                    let _ = collector.save_snapshot(&snapshot).await;
                }
            }
        }

        // Persist last_running_at changes
        {
            let storage = self.storage.lock().await;
            storage.save(&self.sessions, &self.groups, &self.relationships).await?;
        }
        Ok(())
    }

    async fn cache_preview_by_tmux_name(&mut self, tmux_name: &str) -> Result<()> {
        let Some(id) = tmux_name.strip_prefix(SESSION_PREFIX) else {
            return Ok(());
        };
        self.cache_preview_for_id(id).await
    }

    async fn cache_preview_for_id(&mut self, id: &str) -> Result<()> {
        let tmux_session = TmuxManager::session_name(id);
        if !self.tmux.session_exists(&tmux_session).unwrap_or(false) {
            self.preview_cache.remove(id);
            return Ok(());
        }

        let content = self
            .tmux
            .capture_pane(&tmux_session, 120)
            .await
            .unwrap_or_default();
        if !content.is_empty() {
            self.preview_cache.insert(id.to_string(), content);
        }
        Ok(())
    }

    async fn refresh_preview_cache_selected(&mut self) -> Result<()> {
        let Some(id) = self.selected_session().map(|s| s.id.clone()) else {
            return Ok(());
        };

        self.cache_preview_for_id(&id).await?;
        self.update_preview().await
    }

    /// Handle keyboard input
    async fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        match self.state {
            AppState::Normal => self.handle_normal_key(key, modifiers).await,
            AppState::Search => self.handle_search_key(key, modifiers).await,
            AppState::Dialog => self.handle_dialog_key(key, modifiers).await,
            AppState::Help => self.handle_help_key(key),
            AppState::Relationships => self.handle_relationships_key(key, modifiers).await,
        }
    }

    /// Handle keys in normal mode
    async fn handle_normal_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if self.keybindings.matches("quit", &key, modifiers) {
            self.should_quit = true;
            return Ok(());
        }

        // Navigation
        if self.keybindings.matches("up", &key, modifiers) {
            self.move_selection_up();
            self.on_navigation();
            self.preview.clear();
            return Ok(());
        }
        if self.keybindings.matches("down", &key, modifiers) {
            self.move_selection_down();
            self.on_navigation();
            self.preview.clear();
            return Ok(());
        }

        if self.keybindings.matches("jump_priority", &key, modifiers) {
            if let Some(id) = self.priority_session_id() {
                self.queue_attach_by_id(&id).await?;
            }
            return Ok(());
        }

        // Actions
        if self.keybindings.matches("select", &key, modifiers) {
            if self.toggle_selected_group(None).await? {
                self.preview.clear();
            } else {
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
            self.start_selected().await?;
            return Ok(());
        }
        if self.keybindings.matches("stop", &key, modifiers) {
            self.stop_selected().await?;
            return Ok(());
        }
        if self.keybindings.matches("refresh", &key, modifiers) {
            self.refresh_sessions().await?;
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

        if self.keybindings.matches("preview_refresh", &key, modifiers) {
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
                self.restart_selected().await?;
            }
            return Ok(());
        }

        // Ctrl+R: toggle Relationships view (Premium)
        // Note: Ctrl+R is already used for refresh. Use Ctrl+E instead.
        if key == KeyCode::Char('e') && modifiers == KeyModifiers::CONTROL {
            if crate::auth::AuthToken::require_feature("relationships").is_ok() {
                self.state = AppState::Relationships;
            }
            return Ok(());
        }

        // S: Share selected session (Premium)
        if key == KeyCode::Char('S') && modifiers == KeyModifiers::SHIFT {
            if let Some(inst) = self.selected_session() {
                if crate::auth::AuthToken::require_feature("sharing").is_ok() {
                    let already_sharing = inst.sharing.is_some()
                        && inst.sharing.as_ref().is_some_and(|s| s.active);
                    let dialog = ShareDialog {
                        session_id: inst.id.clone(),
                        session_title: inst.title.clone(),
                        permission: crate::sharing::SharePermission::ReadOnly,
                        expire_minutes: TextInput::new(),
                        ssh_url: None,
                        web_url: None,
                        already_sharing,
                    };
                    self.dialog = Some(Dialog::Share(dialog));
                    self.state = AppState::Dialog;
                }
            }
            return Ok(());
        }

        Ok(())
    }

    /// Handle keys in Relationships view
    async fn handle_relationships_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> Result<()> {
        match key {
            // Ctrl+E or Esc: back to Normal
            KeyCode::Char('e') if modifiers == KeyModifiers::CONTROL => {
                self.state = AppState::Normal;
            }
            KeyCode::Esc => {
                self.state = AppState::Normal;
            }
            // q: quit
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            // Navigation
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_relationship_index > 0 {
                    self.selected_relationship_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.relationships.is_empty()
                    && self.selected_relationship_index < self.relationships.len() - 1
                {
                    self.selected_relationship_index += 1;
                }
            }
            // n: new relationship
            KeyCode::Char('n') if modifiers == KeyModifiers::NONE => {
                if crate::auth::AuthToken::require_feature("relationships").is_ok() {
                    self.open_create_relationship_dialog();
                }
            }
            // d: delete selected relationship
            KeyCode::Char('d') if modifiers == KeyModifiers::NONE => {
                if let Some(rel) = self.relationships.get(self.selected_relationship_index) {
                    let rel_id = rel.id.clone();
                    crate::session::relationships::remove_relationship(
                        &mut self.relationships,
                        &rel_id,
                    );
                    if self.selected_relationship_index >= self.relationships.len()
                        && self.selected_relationship_index > 0
                    {
                        self.selected_relationship_index -= 1;
                    }
                    // Save
                    let storage = self.storage.lock().await;
                    storage
                        .save(&self.sessions, &self.groups, &self.relationships)
                        .await?;
                }
            }
            // c: capture context for selected relationship
            KeyCode::Char('c') if modifiers == KeyModifiers::NONE => {
                if let Some(rel) = self.relationships.get(self.selected_relationship_index) {
                    if crate::auth::AuthToken::require_feature("context_collection").is_ok() {
                        let rel_id = rel.id.clone();
                        self.capture_relationship_context(rel_id).await?;
                    }
                }
            }
            // a: annotate selected relationship
            KeyCode::Char('a') if modifiers == KeyModifiers::NONE => {
                if let Some(rel) = self.relationships.get(self.selected_relationship_index) {
                    if crate::auth::AuthToken::require_feature("context_collection").is_ok() {
                        let dialog = crate::ui::AnnotateDialog {
                            relationship_id: rel.id.clone(),
                            note: TextInput::new(),
                        };
                        self.dialog = Some(Dialog::Annotate(dialog));
                        self.state = AppState::Dialog;
                    }
                }
            }
            // Ctrl+N: new session from context
            KeyCode::Char('n') if modifiers == KeyModifiers::CONTROL => {
                if let Some(rel) = self.relationships.get(self.selected_relationship_index) {
                    if crate::auth::AuthToken::require_feature("context_injection").is_ok() {
                        let profile = self.storage.lock().await.profile().to_string();
                        let collector = crate::session::context::ContextCollector::new(&profile);
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
                            relationship_id: rel.id.clone(),
                            context_preview: context,
                            title: TextInput::new(),
                            injection_method: crate::ui::ContextInjectionMethod::InitialPrompt,
                        };
                        self.dialog = Some(Dialog::NewFromContext(dialog));
                        self.state = AppState::Dialog;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn open_create_relationship_dialog(&mut self) {
        if let Some(inst) = self.selected_session() {
            let session_a_id = inst.id.clone();
            let session_a_title = inst.title.clone();
            let all_sessions: Vec<(String, String)> = self
                .sessions
                .iter()
                .map(|s| (s.id.clone(), s.title.clone()))
                .collect();

            let mut dialog = CreateRelationshipDialog {
                relation_type: crate::session::RelationType::Peer,
                session_a_id,
                session_a_title,
                search_input: TextInput::new(),
                all_sessions,
                matches: Vec::new(),
                selected: 0,
                label: TextInput::new(),
                field: CreateRelationshipField::Search,
            };
            dialog.update_matches();
            self.dialog = Some(Dialog::CreateRelationship(dialog));
            self.state = AppState::Dialog;
        }
    }

    async fn capture_relationship_context(&mut self, relationship_id: String) -> Result<()> {
        let rel = self
            .relationships
            .iter()
            .find(|r| r.id == relationship_id);
        let rel = match rel {
            Some(r) => r.clone(),
            None => return Ok(()),
        };

        let profile = {
            let storage = self.storage.lock().await;
            storage.profile().to_string()
        };
        let collector = crate::session::context::ContextCollector::new(&profile);

        // Capture pane output for session A
        if let Some(tmux) = self.session_by_id(&rel.session_a_id).and_then(|s| s.tmux()) {
            if let Ok(output) = tmux.capture_pane().await {
                let snap = crate::session::context::ContextSnapshot::pane_capture(
                    &rel.session_a_id,
                    output,
                )
                .with_relationship(&relationship_id)
                .with_tags(vec!["session_a".to_string()]);
                let _ = collector.save_snapshot(&snap).await;
            }
        }

        // Capture pane output for session B
        if let Some(tmux) = self.session_by_id(&rel.session_b_id).and_then(|s| s.tmux()) {
            if let Ok(output) = tmux.capture_pane().await {
                let snap = crate::session::context::ContextSnapshot::pane_capture(
                    &rel.session_b_id,
                    output,
                )
                .with_relationship(&relationship_id)
                .with_tags(vec!["session_b".to_string()]);
                let _ = collector.save_snapshot(&snap).await;
            }
        }

        Ok(())
    }

    async fn handle_search_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
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

    async fn handle_dialog_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
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

                        self.create_session_from_dialog().await?;
                        self.dialog = None;
                        self.state = AppState::Normal;
                        self.refresh_sessions().await?;
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
                    self.delete_session(&session_id, kill_tmux).await?;
                    self.refresh_sessions().await?;
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
                        SessionEditField::Color => SessionEditField::Title,
                    };
                }
                KeyCode::Enter => {
                    if d.field != SessionEditField::Color {
                        d.field = match d.field {
                            SessionEditField::Title => SessionEditField::Label,
                            SessionEditField::Label => SessionEditField::Color,
                            SessionEditField::Color => SessionEditField::Title,
                        };
                        return Ok(());
                    }

                    let session_id = d.session_id.clone();
                    let old_title = d.old_title.clone();
                    let title = d.new_title.text().to_string();
                    let label = d.label.text().to_string();
                    let label_color = d.label_color;
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.apply_edit_session(&session_id, &old_title, &title, &label, label_color)
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
                    SessionEditField::Color => {}
                },
                KeyCode::Delete => match d.field {
                    SessionEditField::Title => {
                        d.new_title.delete();
                    }
                    SessionEditField::Label => {
                        d.label.delete();
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
                            SessionEditField::Color => {}
                        }
                    }
                }
                KeyCode::Home => match d.field {
                    SessionEditField::Title => d.new_title.move_home(),
                    SessionEditField::Label => d.label.move_home(),
                    SessionEditField::Color => {}
                },
                KeyCode::End => match d.field {
                    SessionEditField::Title => d.new_title.move_end(),
                    SessionEditField::Label => d.label.move_end(),
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
                    )
                    .await?;
                    self.refresh_sessions().await?;
                    self.focus_session(&session_id).await?;
                }
                _ => {}
            },

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
                        let label = if d.label.text().trim().is_empty() {
                            None
                        } else {
                            Some(d.label.text().trim().to_string())
                        };
                        let mut rel = crate::session::Relationship::new(
                            d.relation_type,
                            d.session_a_id.clone(),
                            b_id,
                        );
                        if let Some(l) = label {
                            rel = rel.with_label(l);
                        }
                        crate::session::relationships::add_relationship(
                            &mut self.relationships,
                            rel,
                        );
                        let storage = self.storage.lock().await;
                        storage
                            .save(&self.sessions, &self.groups, &self.relationships)
                            .await?;
                        drop(storage);
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

            Dialog::Share(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Tab => {
                    d.permission = match d.permission {
                        crate::sharing::SharePermission::ReadOnly => {
                            crate::sharing::SharePermission::ReadWrite
                        }
                        crate::sharing::SharePermission::ReadWrite => {
                            crate::sharing::SharePermission::ReadOnly
                        }
                    };
                }
                KeyCode::Enter => {
                    if d.already_sharing {
                        // Stop sharing
                        let mut mgr = crate::sharing::tmate::TmateManager::new();
                        let _ = mgr.stop_sharing(&d.session_id).await;
                        d.already_sharing = false;
                        d.ssh_url = None;
                        d.web_url = None;
                        if let Some(inst) =
                            self.sessions.iter_mut().find(|s| s.id == d.session_id)
                        {
                            inst.sharing = None;
                        }
                        let storage = self.storage.lock().await;
                        storage
                            .save(&self.sessions, &self.groups, &self.relationships)
                            .await?;
                    } else {
                        // Start sharing
                        if crate::sharing::tmate::TmateManager::is_available().await {
                            let mut mgr = crate::sharing::tmate::TmateManager::new();
                            let tmux_name =
                                format!("{}_{}", SESSION_PREFIX, d.session_id);
                            let expire: Option<u64> = d
                                .expire_minutes
                                .text()
                                .parse::<u64>()
                                .ok()
                                .filter(|&v| v > 0);
                            match mgr
                                .start_sharing(
                                    &d.session_id,
                                    &tmux_name,
                                    d.permission,
                                    expire,
                                )
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
                                }
                                Err(_e) => {}
                            }
                        }
                    }
                }
                KeyCode::Char('c') => {
                    if let Some(ref url) = d.ssh_url {
                        let _ = std::process::Command::new("pbcopy")
                            .stdin(std::process::Stdio::piped())
                            .spawn()
                            .and_then(|mut child| {
                                use std::io::Write;
                                if let Some(ref mut stdin) = child.stdin {
                                    stdin.write_all(url.as_bytes())?;
                                }
                                child.wait()
                            });
                    }
                }
                KeyCode::Backspace => {
                    d.expire_minutes.backspace();
                }
                KeyCode::Char(ch) if ch.is_ascii_digit() => {
                    d.expire_minutes.insert(ch);
                }
                _ => {}
            },

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
                            let collector = crate::session::context::ContextCollector::new(&profile);
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
            Dialog::NewFromContext(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Relationships;
                }
                KeyCode::Tab => {
                    d.injection_method = d.injection_method.cycle();
                }
                KeyCode::Enter => {
                    // Create new session from context - for now just close
                    // Full implementation requires creating a tmux session with injected context
                    self.dialog = None;
                    self.state = AppState::Relationships;
                }
                KeyCode::Backspace => {
                    d.title.backspace();
                }
                KeyCode::Char(c) => {
                    d.title.insert(c);
                }
                _ => {}
            },
        }

        Ok(())
    }

    fn group_session_ids(&self, group_path: &str) -> Vec<String> {
        let prefix = format!("{}/", group_path);
        self.sessions
            .iter()
            .filter(|s| s.group_path == group_path || s.group_path.starts_with(&prefix))
            .map(|s| s.id.clone())
            .collect()
    }

    fn open_fork_dialog(&mut self) {
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

    fn open_create_group_dialog(&mut self) {
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

    fn open_move_group_dialog(&mut self) {
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

    fn open_rename_session_dialog(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
        };

        self.dialog = Some(Dialog::RenameSession(RenameSessionDialog {
            session_id: s.id.clone(),
            old_title: s.title.clone(),
            new_title: TextInput::with_text(s.title.clone()),
            label: TextInput::with_text(s.label.clone()),
            label_color: s.label_color,
            field: SessionEditField::Title,
        }));
        self.state = AppState::Dialog;
    }

    fn collect_existing_tags(&self) -> Vec<TagSpec> {
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

    fn open_tag_picker_dialog(&mut self) {
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

    fn open_rename_group_dialog(&mut self) {
        let Some(TreeItem::Group { path, .. }) = self.selected_tree_item() else {
            return;
        };

        self.dialog = Some(Dialog::RenameGroup(RenameGroupDialog {
            old_path: path.clone(),
            new_path: TextInput::with_text(path.clone()),
        }));
        self.state = AppState::Dialog;
    }

    async fn create_fork_session(
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

        let storage = self.storage.lock().await;
        let (mut instances, tree, relationships) = storage.load().await?;
        instances.push(inst.clone());
        storage.save(&instances, &tree, &relationships).await?;

        Ok(inst.id)
    }

    async fn apply_create_group(&mut self, group_path: &str) -> Result<()> {
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

    async fn apply_delete_group_prefix(&mut self, group_path: &str) -> Result<()> {
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

    async fn apply_delete_group_keep_sessions(&mut self, group_path: &str) -> Result<()> {
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

    async fn apply_delete_group_and_sessions(&mut self, group_path: &str) -> Result<()> {
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
                let tmux_name = TmuxManager::session_name(&inst.id);
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

    async fn apply_move_group(&mut self, session_id: &str, group_path: &str) -> Result<()> {
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

    async fn apply_edit_session(
        &mut self,
        session_id: &str,
        old_title: &str,
        new_title: &str,
        label: &str,
        label_color: crate::session::LabelColor,
    ) -> Result<()> {
        let title = new_title.trim();
        let title = if title.is_empty() { old_title } else { title };
        let label = label.trim();

        let storage = self.storage.lock().await;
        let (mut instances, tree, relationships) = storage.load().await?;

        if let Some(inst) = instances.iter_mut().find(|s| s.id == session_id) {
            inst.title = title.to_string();
            inst.label = label.to_string();
            inst.label_color = label_color;
        }

        storage.save(&instances, &tree, &relationships).await?;
        Ok(())
    }

    async fn apply_rename_group(&mut self, old_path: &str, new_path: &str) -> Result<()> {
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
        Ok(())
    }

    async fn create_session_from_dialog(&mut self) -> Result<()> {
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

    async fn delete_session(&mut self, session_id: &str, kill_tmux: bool) -> Result<()> {
        let tmux_name = TmuxManager::session_name(session_id);

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

    /// Handle keys in help mode
    fn handle_help_key(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                self.help_visible = false;
                self.state = AppState::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    fn ensure_groups_exist(&mut self) {
        for s in &self.sessions {
            if !s.group_path.is_empty() {
                self.groups.create_group(s.group_path.clone());
            }
        }
    }

    fn rebuild_sessions_index(&mut self) {
        self.sessions_by_id = self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();
    }

    fn rebuild_tree(&mut self) {
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

        let mut items: Vec<TreeItem> = Vec::new();

        // Root sessions
        for si in ungrouped {
            items.push(TreeItem::Session {
                id: self.sessions[si].id.clone(),
                depth: 0,
            });
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
            by_group: &BTreeMap<String, Vec<usize>>,
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
                    items.push(TreeItem::Session {
                        id: app.sessions[si].id.clone(),
                        depth: depth + 1,
                    });
                }
            }
        }

        for r in roots {
            visit(self, &mut items, &by_group, &r, 0);
        }

        self.tree = items;
    }

    async fn toggle_selected_group(&mut self, desired: Option<bool>) -> Result<bool> {
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

    fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
        if query.is_empty() {
            return Some(0);
        }

        let q = query.to_lowercase();
        let t = text.to_lowercase();

        let mut score: i32 = 0;
        let mut last_match: Option<usize> = None;
        let mut pos = 0usize;

        for ch in q.chars() {
            if let Some(found) = t[pos..].find(ch) {
                let idx = pos + found;
                score += 10;
                if let Some(prev) = last_match {
                    if idx == prev + 1 {
                        score += 15; // contiguous bonus
                    } else {
                        score -= (idx.saturating_sub(prev) as i32).min(10);
                    }
                } else {
                    score -= idx.min(15) as i32; // earlier is better
                }
                last_match = Some(idx);
                pos = idx + ch.len_utf8();
            } else {
                return None;
            }
        }

        Some(score)
    }

    fn update_search_results(&mut self) {
        let q = self.search_query.trim();
        if q.is_empty() {
            self.search_results.clear();
            self.search_selected = 0;
            return;
        }

        let mut scored: Vec<(i32, String)> = Vec::new();
        for s in &self.sessions {
            let hay = format!(
                "{} {} {}",
                s.title,
                s.group_path,
                s.project_path.to_string_lossy()
            );
            if let Some(score) = Self::fuzzy_score(q, &hay) {
                scored.push((score, s.id.clone()));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.search_results = scored.into_iter().map(|(_, id)| id).take(50).collect();
        if self.search_selected >= self.search_results.len() {
            self.search_selected = 0;
        }
    }

    async fn focus_session(&mut self, id: &str) -> Result<()> {
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
            TreeItem::Session { id: sid, .. } => sid == id,
            _ => false,
        }) {
            self.selected_index = idx;
            self.preview.clear();
            self.update_preview().await?;
        }

        Ok(())
    }

    async fn focus_group(&mut self, path: &str) -> Result<()> {
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
    fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down
    fn move_selection_down(&mut self) {
        if self.selected_index + 1 < self.tree.len() {
            self.selected_index += 1;
        }
    }

    fn selected_tree_item(&self) -> Option<&TreeItem> {
        self.tree.get(self.selected_index)
    }

    /// Get selected session (if selection is a session row)
    pub fn selected_session(&self) -> Option<&Instance> {
        let TreeItem::Session { id, .. } = self.selected_tree_item()? else {
            return None;
        };
        let &idx = self.sessions_by_id.get(id)?;
        self.sessions.get(idx)
    }

    fn priority_session_id(&self) -> Option<String> {
        // Priority: Waiting (!) newest first, else Ready (✓) newest first.
        if let Some(s) = self
            .sessions
            .iter()
            .filter(|s| s.status == Status::Waiting)
            .max_by_key(|s| s.last_waiting_at.unwrap_or(s.created_at))
        {
            return Some(s.id.clone());
        }

        self.sessions
            .iter()
            .filter(|s| s.status == Status::Idle && self.is_attention_active(&s.id))
            .max_by_key(|s| s.last_running_at.unwrap_or(s.created_at))
            .map(|s| s.id.clone())
    }

    async fn queue_attach_by_id(&mut self, id: &str) -> Result<()> {
        if let Some(pos) = self
            .tree
            .iter()
            .position(|item| matches!(item, TreeItem::Session { id: sid, .. } if sid == id))
        {
            self.selected_index = pos;
            self.on_navigation();
            self.preview.clear();
        }

        let Some(&idx) = self.sessions_by_id.get(id) else {
            return Ok(());
        };
        let session = self.sessions[idx].clone();

        let tmux_session = TmuxManager::session_name(&session.id);
        if !self.tmux.session_exists(&tmux_session).unwrap_or(false) {
            let _ = self
                .tmux
                .create_session(
                    &tmux_session,
                    &session.project_path.to_string_lossy(),
                    if session.command.trim().is_empty() {
                        None
                    } else {
                        Some(session.command.as_str())
                    },
                )
                .await;
        }

        if self.tmux.session_exists(&tmux_session).unwrap_or(false) {
            self.pending_attach = Some(tmux_session);
        }
        Ok(())
    }

    /// Find session by tmux session name (e.g. "agentdeck_rs_abc123")
    fn find_session_by_tmux_name(&self, tmux_name: &str) -> Option<Instance> {
        let id = tmux_name.strip_prefix(SESSION_PREFIX)?;
        let &idx = self.sessions_by_id.get(id)?;
        self.sessions.get(idx).cloned()
    }

    /// Queue attach to selected session (performed in event loop)
    async fn queue_attach_selected(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let tmux_session = TmuxManager::session_name(&session.id);

            if !self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                self.start_selected().await?;
            }

            if self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                self.pending_attach = Some(tmux_session);
            }
        }
        Ok(())
    }

    async fn perform_attach(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        name: &str,
    ) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        let attach_result = self.tmux.attach_session(name).await;

        enable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture
        )?;
        terminal.clear()?;

        attach_result
    }

    /// Start selected session
    async fn start_selected(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let tmux_session = TmuxManager::session_name(&session.id);

            if !self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                if let Err(e) = self
                    .tmux
                    .create_session(
                        &tmux_session,
                        &session.project_path.to_string_lossy(),
                        if session.command.trim().is_empty() {
                            None
                        } else {
                            Some(session.command.as_str())
                        },
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
    async fn stop_selected(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let tmux_session = TmuxManager::session_name(&session.id);

            if self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                self.tmux.kill_session(&tmux_session).await?;
                self.refresh_sessions().await?;
            }
        }
        Ok(())
    }

    /// Restart selected session
    async fn restart_selected(&mut self) -> Result<()> {
        self.stop_selected().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        self.start_selected().await?;
        Ok(())
    }

    /// Refresh sessions data
    async fn refresh_sessions(&mut self) -> Result<()> {
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

    async fn update_preview(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let tmux_session = TmuxManager::session_name(&session.id);

            if self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                if let Some(cached) = self.preview_cache.get(&session.id) {
                    self.preview = cached.clone();
                } else {
                    let ptmx_line = if session.ptmx_count > 0 {
                        format!("PTY FDs: {}\n", session.ptmx_count)
                    } else {
                        String::new()
                    };
                    self.preview = format!(
                        "{}\n\nPath: {}\nLabel: {}\n{}\nPreview not cached. Press 'p' to capture a snapshot.",
                        session.title,
                        session.project_path.to_string_lossy(),
                        session.label,
                        ptmx_line
                    );
                }
            } else {
                let ptmx_line = if session.ptmx_count > 0 {
                    format!("PTY FDs: {}\n", session.ptmx_count)
                } else {
                    String::new()
                };
                self.preview = format!(
                    "{}\n\nPath: {}\nLabel: {}\n{}\nNot running. Press 's' to start, Enter to start+attach.",
                    session.title,
                    session.project_path.to_string_lossy(),
                    session.label,
                    ptmx_line
                );
            }

            return Ok(());
        }

        if let Some(TreeItem::Group { path, name, .. }) = self.selected_tree_item() {
            let direct = self
                .sessions
                .iter()
                .filter(|s| s.group_path == *path)
                .count();
            let prefix = format!("{}/", path);
            let total = self
                .sessions
                .iter()
                .filter(|s| s.group_path == *path || s.group_path.starts_with(&prefix))
                .count();

            self.preview = format!(
                "Group: {}\nPath: {}\nExpanded: {}\n\n{} sessions ({} direct)",
                name,
                path,
                self.groups.is_expanded(path),
                total,
                direct
            );
            return Ok(());
        }

        self.preview.clear();
        Ok(())
    }

    // Getters for rendering
    pub fn sessions(&self) -> &[Instance] {
        &self.sessions
    }

    pub fn tree(&self) -> &[TreeItem] {
        &self.tree
    }

    pub fn selected_item(&self) -> Option<&TreeItem> {
        self.tree.get(self.selected_index)
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn session_by_id(&self, id: &str) -> Option<&Instance> {
        let &idx = self.sessions_by_id.get(id)?;
        self.sessions.get(idx)
    }

    pub fn is_group_expanded(&self, path: &str) -> bool {
        self.groups.is_expanded(path)
    }

    pub fn group_has_children(&self, path: &str) -> bool {
        self.groups.has_children(path)
    }

    pub fn help_visible(&self) -> bool {
        self.help_visible
    }

    pub fn preview(&self) -> &str {
        &self.preview
    }

    pub fn state(&self) -> AppState {
        self.state
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    pub fn search_matches(&self) -> usize {
        self.search_results.len()
    }

    pub fn search_results(&self) -> &[String] {
        &self.search_results
    }

    pub fn search_selected(&self) -> usize {
        self.search_selected
    }

    pub fn new_session_dialog(&self) -> Option<&NewSessionDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::NewSession(d)) => Some(d),
            _ => None,
        }
    }

    pub fn delete_confirm_dialog(&self) -> Option<&DeleteConfirmDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::DeleteConfirm(d)) => Some(d),
            _ => None,
        }
    }

    pub fn delete_group_dialog(&self) -> Option<&DeleteGroupDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::DeleteGroup(d)) => Some(d),
            _ => None,
        }
    }

    pub fn fork_dialog(&self) -> Option<&ForkDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::Fork(d)) => Some(d),
            _ => None,
        }
    }

    pub fn create_group_dialog(&self) -> Option<&CreateGroupDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::CreateGroup(d)) => Some(d),
            _ => None,
        }
    }

    pub fn move_group_dialog(&self) -> Option<&MoveGroupDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::MoveGroup(d)) => Some(d),
            _ => None,
        }
    }

    pub fn rename_group_dialog(&self) -> Option<&RenameGroupDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::RenameGroup(d)) => Some(d),
            _ => None,
        }
    }

    pub fn rename_session_dialog(&self) -> Option<&RenameSessionDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::RenameSession(d)) => Some(d),
            _ => None,
        }
    }

    pub fn tag_picker_dialog(&self) -> Option<&TagPickerDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::TagPicker(d)) => Some(d),
            _ => None,
        }
    }

    pub fn share_dialog(&self) -> Option<&ShareDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::Share(d)) => Some(d),
            _ => None,
        }
    }

    pub fn create_relationship_dialog(&self) -> Option<&CreateRelationshipDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::CreateRelationship(d)) => Some(d),
            _ => None,
        }
    }

    pub fn annotate_dialog(&self) -> Option<&crate::ui::AnnotateDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::Annotate(d)) => Some(d),
            _ => None,
        }
    }

    pub fn new_from_context_dialog(&self) -> Option<&crate::ui::NewFromContextDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::NewFromContext(d)) => Some(d),
            _ => None,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn is_attention_active(&self, id: &str) -> bool {
        // Ready (✓) if last_running_at is within ATTENTION_TTL
        self.session_by_id(id)
            .and_then(|s| s.last_running_at)
            .is_some_and(|t| {
                let elapsed = chrono::Utc::now().signed_duration_since(t);
                elapsed < chrono::Duration::from_std(self.attention_ttl).unwrap_or_default()
            })
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    pub fn system_ptmx_total(&self) -> u32 {
        self.cached_ptmx_total
    }

    pub fn system_ptmx_max(&self) -> u32 {
        self.cached_ptmx_max
    }

    pub fn auth_token(&self) -> Option<&crate::auth::AuthToken> {
        self.auth_token.as_ref()
    }

    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    pub fn selected_relationship_index(&self) -> usize {
        self.selected_relationship_index
    }
}
