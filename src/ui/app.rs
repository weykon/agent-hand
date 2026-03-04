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
    TmuxManager,
};

use super::{
    AppState, CreateGroupDialog,
    DeleteConfirmDialog, DeleteGroupChoice, DeleteGroupDialog, Dialog, ForkDialog, ForkField,
    MoveGroupDialog, NewSessionDialog, NewSessionField, RenameGroupDialog, RenameSessionDialog,
    SessionEditField, SettingsDialog, SettingsField, TagPickerDialog, TagSpec, TextInput, TreeItem,
};

#[cfg(feature = "pro")]
use super::{CreateRelationshipDialog, CreateRelationshipField, ShareDialog};

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
    relationship_snapshot_counts: HashMap<String, usize>,
    tree: Vec<TreeItem>,
    selected_index: usize,

    // Active sessions panel (premium)
    active_panel_focused: bool,
    active_panel_selected: usize,

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

    // Event-driven status detection (from Claude Code hooks)
    event_receiver: Option<crate::hooks::EventReceiver>,

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
    config: crate::config::ConfigFile,

    // Auth
    auth_token: Option<crate::auth::AuthToken>,

    // Vim-style navigation (pro only)
    #[cfg(feature = "pro")]
    list_state: ratatui::widgets::ListState,
    #[cfg(feature = "pro")]
    jump_lines: usize,

    // Viewer mode state (pro only) — for viewing shared terminal sessions
    #[cfg(feature = "pro")]
    viewer_state: Option<ViewerState>,

    // Sound notifications (pro only) — plays sounds on status transitions
    #[cfg(feature = "pro")]
    notification_manager: crate::pro::notification::NotificationManager,

    // Active relay clients keyed by session_id (pro only) — kept alive for streaming
    #[cfg(feature = "pro")]
    relay_clients: HashMap<String, Arc<crate::pro::collab::client::RelayClient>>,

    // AI summarizer (Max tier)
    #[cfg(feature = "max")]
    summarizer: Option<crate::ai::Summarizer>,
    #[cfg(feature = "max")]
    /// Summaries received from background AI tasks, displayed in preview.
    summary_results: HashMap<String, String>,
}

/// State for viewing a shared terminal session via relay.
#[cfg(feature = "pro")]
pub struct ViewerState {
    /// Name of the session being viewed.
    pub session_name: String,
    /// Current terminal content (raw bytes with ANSI escapes).
    pub terminal_content: Arc<tokio::sync::Mutex<Vec<u8>>>,
    /// Current terminal dimensions.
    pub terminal_size: Arc<tokio::sync::Mutex<(u16, u16)>>,
    /// Number of viewers (including self).
    pub viewer_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Whether the connection is active.
    pub connected: Arc<std::sync::atomic::AtomicBool>,
    /// Handle to the viewer task.
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl App {
    const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);
    const NAVIGATION_SETTLE: Duration = Duration::from_millis(300);
    const STATUS_REFRESH: Duration = Duration::from_secs(1);
    const CACHE_REFRESH: Duration = Duration::from_secs(2);
    const STATUS_COOLDOWN: Duration = Duration::from_secs(2);
    const STATUS_FALLBACK: Duration = Duration::from_secs(10);

    /// Create new application
    pub async fn new(profile: &str) -> Result<Self> {
        let storage = Storage::new(profile).await?;
        let (mut sessions, groups, relationships) = storage.load().await?;
        // Status is derived from tmux probes; the persisted value can be stale across restarts.
        // Reset to avoid treating old Running→Idle as a fresh completion.
        // Also clear stale sharing state — relay rooms are ephemeral and won't survive TUI restart.
        for s in &mut sessions {
            s.status = Status::Idle;
            if s.sharing.as_ref().is_some_and(|sh| sh.active) {
                s.sharing = None;
            }
        }

        let tmux = TmuxManager::new();

        // Clean up orphaned tmux sessions (exist in tmux but not in storage).
        // This prevents PTY leaks from sessions that were deleted but whose tmux
        // process was not properly killed.
        {
            let known_names: Vec<String> = sessions.iter().map(|s| s.tmux_name()).collect();
            let known_refs: Vec<&str> = known_names.iter().map(|s| s.as_str()).collect();
            let killed = tmux.cleanup_orphaned_sessions(&known_refs).await;
            if killed > 0 {
                tracing::info!("Cleaned up {} orphaned tmux session(s)", killed);
            }
        }

        let keybindings = crate::config::KeyBindings::load_or_default().await;
        let analytics = crate::analytics::ActivityTracker::new(profile).await;

        // Get system PTY limit once at startup.
        let system_ptmx_max = crate::tmux::ptmx::get_ptmx_max().await;

        let config = crate::config::ConfigFile::load()
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let attention_ttl = Duration::from_secs(config.ready_ttl_minutes() * 60);

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
            relationship_snapshot_counts: HashMap::new(),
            tree: Vec::new(),
            selected_index: 0,
            active_panel_focused: false,
            active_panel_selected: 0,
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
            event_receiver: crate::hooks::EventReceiver::new().ok(),
            tick_count: 0,
            attention_ttl,
            storage: Arc::new(Mutex::new(storage)),
            tmux: Arc::new(tmux),
            analytics,
            config: config.clone(),
            ptmx_state,
            _ptmx_task: ptmx_task,
            cached_ptmx_total: 0,
            cached_ptmx_max: system_ptmx_max,
            auth_token: crate::auth::AuthToken::load(),
            #[cfg(feature = "pro")]
            list_state: ratatui::widgets::ListState::default(),
            #[cfg(feature = "pro")]
            jump_lines: config.jump_lines(),
            #[cfg(feature = "pro")]
            notification_manager: crate::pro::notification::NotificationManager::new(
                config.notification(),
            ),
            #[cfg(feature = "pro")]
            viewer_state: None,
            #[cfg(feature = "pro")]
            relay_clients: HashMap::new(),
            #[cfg(feature = "max")]
            summarizer: {
                let is_max = crate::auth::AuthToken::load().map_or(false, |t| t.is_max());
                if is_max {
                    crate::ai::Summarizer::from_config(config.ai())
                } else {
                    None
                }
            },
            #[cfg(feature = "max")]
            summary_results: HashMap::new(),
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

                // Pro: auto-focus active panel when returning from a detached session
                #[cfg(feature = "pro")]
                {
                    let is_pro = self.auth_token.as_ref().map_or(false, |t| t.is_pro());
                    let active_count = self.active_sessions().len();
                    if is_pro && active_count > 0 {
                        self.active_panel_focused = true;
                        if self.active_panel_selected >= active_count {
                            self.active_panel_selected = active_count.saturating_sub(1);
                        }
                    }
                }
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
        #[cfg(feature = "pro")]
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
                let mut mgr = crate::pro::tmate::TmateManager::from_config().await;
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

        // Poll AI summary results (non-blocking)
        #[cfg(feature = "max")]
        if let Some(ref mut summarizer) = self.summarizer {
            for result in summarizer.poll_results() {
                self.summary_results.insert(result.session_id.clone(), result.summary.clone());
                // Update preview if the summarized session is currently selected
                if self.selected_session().map(|s| s.id.as_str()) == Some(&result.session_id) {
                    self.preview = format!("🤖 AI Summary:\n\n{}", result.summary);
                }
            }
        }

        // Update PTY counts from background task (non-blocking)
        // The background task scans every 30 minutes, we just read the cached state
        {
            let state = self.ptmx_state.read().await;
            for session in &mut self.sessions {
                session.ptmx_count = state.per_session.get(&session.tmux_name()).copied().unwrap_or(0);
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

        // Collect session IDs that transition from Running to Idle/Waiting for auto-capture
        let mut running_to_done: Vec<String> = Vec::new();
        // Collect sessions that transitioned to Waiting or had errors (for notifications)
        #[cfg(feature = "pro")]
        let mut became_waiting: Vec<String> = Vec::new();
        #[cfg(feature = "pro")]
        let mut had_error: Vec<String> = Vec::new();

        // --- Phase 1: Process hook events (event-driven, precise) ---
        // Sessions updated by hook events are tracked so we can skip polling for them.
        let mut hook_updated: std::collections::HashSet<String> = std::collections::HashSet::new();

        if let Some(ref mut receiver) = self.event_receiver {
            let events = receiver.poll();
            for event in events {
                // Find the session matching this tmux session name
                let session = self
                    .sessions
                    .iter_mut()
                    .find(|s| s.tmux_name() == event.tmux_session);
                let Some(session) = session else {
                    continue;
                };

                let prev_status = session.status;
                let now_utc = chrono::Utc::now();

                use crate::hooks::HookEventKind;
                let new_status = match &event.kind {
                    HookEventKind::UserPromptSubmit => Status::Running,
                    HookEventKind::Stop => Status::Idle,
                    HookEventKind::Notification { notification_type } => {
                        match notification_type.as_str() {
                            "idle_prompt" => Status::Idle,
                            "elicitation_dialog" | "permission_prompt" => Status::Waiting,
                            _ => Status::Idle,
                        }
                    }
                    HookEventKind::PermissionRequest { .. } => Status::Waiting,
                    HookEventKind::ToolFailure { .. } => Status::Idle,
                    HookEventKind::SubagentStart => Status::Running,
                    HookEventKind::PreCompact => Status::Running,
                };

                // Record timestamps
                if new_status == Status::Running
                    || (prev_status == Status::Running && new_status == Status::Idle)
                {
                    session.last_running_at = Some(now_utc);
                }
                if new_status == Status::Waiting && prev_status != Status::Waiting {
                    session.last_waiting_at = Some(now_utc);
                }

                // Detect Running → Done transition
                let tracked_prev = self.previous_statuses.get(&session.id).copied();
                if tracked_prev == Some(Status::Running)
                    && (new_status == Status::Idle || new_status == Status::Waiting)
                {
                    running_to_done.push(session.id.clone());
                }

                // Track Waiting/Error transitions for notifications
                #[cfg(feature = "pro")]
                {
                    if new_status == Status::Waiting && prev_status != Status::Waiting {
                        became_waiting.push(session.id.clone());
                    }
                    if matches!(event.kind, HookEventKind::ToolFailure { .. }) {
                        had_error.push(session.id.clone());
                    }
                }

                self.previous_statuses
                    .insert(session.id.clone(), new_status);
                session.status = new_status;
                hook_updated.insert(session.id.clone());
            }
        }

        // --- Phase 2: Polling fallback for sessions without hook events ---
        // Only probe sessions that weren't updated by hooks this cycle.
        let selected_id = self.selected_session().map(|s| s.id.clone());

        for session in &mut self.sessions {
            if hook_updated.contains(&session.id) {
                continue; // Already updated by hook event
            }

            let tmux_session = session.tmux_name();
            if !self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                session.status = Status::Idle;
                self.last_tmux_activity.remove(&session.id);
                self.last_tmux_activity_change.remove(&session.id);
                self.last_status_probe.remove(&session.id);
                continue;
            }

            let activity = self.tmux.session_activity(&tmux_session).unwrap_or(0);
            let prev_activity = self.last_tmux_activity.get(&session.id).copied();

            let activity_changed = prev_activity.is_some_and(|a| activity > a);
            if activity_changed || prev_activity.is_none() {
                self.last_tmux_activity.insert(session.id.clone(), activity);
                if activity_changed {
                    self.last_tmux_activity_change
                        .insert(session.id.clone(), now);
                }
            }

            let need_fallback_probe = self
                .last_status_probe
                .get(&session.id)
                .is_none_or(|t| now.duration_since(*t) >= Self::STATUS_FALLBACK);

            let activity_settled = self
                .last_tmux_activity_change
                .get(&session.id)
                .is_some_and(|t| now.duration_since(*t) >= Self::STATUS_COOLDOWN);

            let is_selected = selected_id.as_deref() == Some(session.id.as_str());

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

            if new_status == Status::Running
                || (prev_status == Status::Running && new_status == Status::Idle)
            {
                session.last_running_at = Some(now_utc);
            }

            if new_status == Status::Waiting && prev_status != Status::Waiting {
                session.last_waiting_at = Some(chrono::Utc::now());
            }

            let tracked_prev = self.previous_statuses.get(&session.id).copied();
            if tracked_prev == Some(Status::Running)
                && (new_status == Status::Idle || new_status == Status::Waiting)
            {
                running_to_done.push(session.id.clone());
            }

            self.previous_statuses.insert(session.id.clone(), new_status);

            session.status = new_status;
            self.last_status_probe.insert(session.id.clone(), now);
            if force_probe {
                self.force_probe_tmux = None;
            }
        }

        // Auto-capture context for sessions that transitioned from Running to Idle/Waiting
        #[cfg(feature = "pro")]
        if !running_to_done.is_empty()
            && crate::auth::AuthToken::require_feature("auto_context").is_ok()
        {
            let profile = {
                let storage = self.storage.lock().await;
                storage.profile().to_string()
            };
            let collector = crate::pro::context::ContextCollector::new(&profile);

            for session_id in &running_to_done {
                let rels = crate::session::relationships::find_relationships_for_session(
                    &self.relationships,
                    session_id,
                );
                if rels.is_empty() {
                    continue;
                }

                // Capture pane output once for this session
                let tmux_name = self.tmux_name_for_id(session_id);
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

        // Sound notifications for status transitions (Pro)
        #[cfg(feature = "pro")]
        if crate::auth::AuthToken::require_feature("notification").is_ok()
            || crate::auth::AuthToken::require_max("notification").is_ok()
        {
            for session_id in &running_to_done {
                self.notification_manager.on_task_complete(session_id);
            }
            for session_id in &became_waiting {
                self.notification_manager.on_input_required(session_id);
            }
            for session_id in &had_error {
                self.notification_manager.on_error(session_id);
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
        let id = self
            .sessions
            .iter()
            .find(|s| s.tmux_name() == tmux_name)
            .map(|s| s.id.clone());
        let Some(id) = id else {
            return Ok(());
        };
        self.cache_preview_for_id(&id).await
    }

    async fn cache_preview_for_id(&mut self, id: &str) -> Result<()> {
        let tmux_session = self.tmux_name_for_id(id);
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
            #[cfg(feature = "pro")]
            AppState::Relationships => self.handle_relationships_key(key, modifiers).await,
            #[cfg(feature = "pro")]
            AppState::ViewerMode => self.handle_viewer_key(key, modifiers).await,
        }
    }

    /// Handle keys in normal mode
    async fn handle_normal_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        if self.keybindings.matches("quit", &key, modifiers) {
            self.dialog = Some(Dialog::QuitConfirm);
            self.state = AppState::Dialog;
            return Ok(());
        }

        if self.keybindings.matches("settings", &key, modifiers) {
            self.dialog = Some(Dialog::Settings(SettingsDialog::new(&self.config)));
            self.state = AppState::Dialog;
            return Ok(());
        }

        // Tab: toggle active panel focus (premium gate)
        #[cfg(feature = "pro")]
        if key == KeyCode::Tab && modifiers == KeyModifiers::NONE {
            let is_pro = self.auth_token.as_ref().map_or(false, |t| t.is_pro());
            let active_count = self.active_sessions().len();
            if is_pro && active_count > 0 {
                if self.active_panel_focused {
                    // Switching TO tree — sync tree selection to active panel session
                    let active = self.active_sessions();
                    if let Some(session) = active.get(self.active_panel_selected) {
                        let id = session.id.clone();
                        self.active_panel_focused = false;
                        self.focus_tree_on_session_id(&id);
                    } else {
                        self.active_panel_focused = false;
                    }
                } else {
                    // Switching TO active panel
                    self.active_panel_focused = true;
                    if self.active_panel_selected >= active_count {
                        self.active_panel_selected = active_count.saturating_sub(1);
                    }
                }
                return Ok(());
            }
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
            #[cfg(feature = "pro")]
            self.enforce_scrolloff();
            self.on_navigation();
            self.preview.clear();
            return Ok(());
        }
        if self.keybindings.matches("down", &key, modifiers) {
            self.move_selection_down();
            #[cfg(feature = "pro")]
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

        // AI Summarize (Max tier) — 'A' key
        #[cfg(feature = "max")]
        if self.keybindings.matches("summarize", &key, modifiers) {
            if let Some(summarizer) = &self.summarizer {
                if let Some(session) = self.selected_session() {
                    let id = session.id.clone();
                    let title = session.title.clone();
                    let tmux_name = session.tmux_name();
                    let lines = summarizer.capture_lines;
                    if self.tmux.session_exists(&tmux_name).unwrap_or(false) {
                        let content = self.tmux.capture_pane(&tmux_name, lines).await.unwrap_or_default();
                        if !content.is_empty() {
                            summarizer.summarize_session(id, title, content);
                            self.preview = "⏳ AI summarizing...".to_string();
                        }
                    }
                }
            } else {
                self.preview = "AI Summarize requires Max subscription.\nVisit https://weykon.github.io/agent-hand".to_string();
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

        // Ctrl+E: toggle Relationships view (Premium)
        #[cfg(feature = "pro")]
        if key == KeyCode::Char('e') && modifiers == KeyModifiers::CONTROL {
            if crate::auth::AuthToken::require_feature("relationships").is_ok() {
                self.refresh_snapshot_counts_async().await;
                self.state = AppState::Relationships;
            }
            return Ok(());
        }

        // J: Join a shared session via URL (Max tier)
        #[cfg(feature = "pro")]
        if key == KeyCode::Char('J') && modifiers == KeyModifiers::SHIFT {
            if crate::auth::AuthToken::require_max("sharing").is_ok() {
                self.dialog = Some(Dialog::JoinSession(
                    crate::ui::dialogs::JoinSessionDialog::new(),
                ));
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
                    let dialog = ShareDialog {
                        session_id: inst.id.clone(),
                        session_title: inst.title.clone(),
                        permission: default_perm,
                        expire_minutes: expire_input,
                        ssh_url: None,
                        web_url: None,
                        already_sharing,
                        relay_share_url: None,
                        relay_room_id: None,
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
    #[cfg(feature = "pro")]
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
                    drop(storage);
                    let _ = self.analytics.record_premium_event(
                        crate::analytics::EventType::RelationshipDelete,
                        &rel_id,
                        "",
                    ).await;
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

    #[cfg(feature = "pro")]
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

    #[cfg(feature = "pro")]
    async fn refresh_snapshot_counts_async(&mut self) {
        let profile = {
            let storage = self.storage.lock().await;
            storage.profile().to_string()
        };
        let collector = crate::pro::context::ContextCollector::new(&profile);
        self.relationship_snapshot_counts.clear();
        for rel in &self.relationships {
            let count = collector.count_relationship_snapshots(&rel.id);
            if count > 0 {
                self.relationship_snapshot_counts.insert(rel.id.clone(), count);
            }
        }
    }

    #[cfg(feature = "pro")]
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
        let collector = crate::pro::context::ContextCollector::new(&profile);

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

        let _ = self.analytics.record_premium_event(
            crate::analytics::EventType::ContextCapture,
            &relationship_id,
            "",
        ).await;

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
                        let _ = self.analytics.record_premium_event(
                            crate::analytics::EventType::RelationshipCreate,
                            &d.session_a_id,
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
                        // Stop sharing — try relay cleanup first, then tmate
                        if d.relay_room_id.is_some() {
                            // Relay sharing — stop client and pipe-pane
                            let tmux_name = self.sessions_by_id
                                .get(&d.session_id)
                                .and_then(|&idx| self.sessions.get(idx))
                                .map(|s| s.tmux_name())
                                .unwrap_or_else(|| TmuxManager::session_name_legacy(&d.session_id));
                            if let Some(client) = self.relay_clients.remove(&d.session_id) {
                                client.stop(&tmux_name).await;
                            } else {
                                let _ = self.tmux.stop_pipe_pane(&tmux_name).await;
                            }
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
                        // Start sharing — try relay first, fall back to tmate
                        let sharing_cfg = crate::config::ConfigFile::load()
                            .await
                            .ok()
                            .flatten()
                            .map(|c| c.sharing().clone())
                            .unwrap_or_default();
                        // Try manual override first, then discover from auth
                        let relay_url = match sharing_cfg.relay_server_url.clone() {
                            Some(url) => Some(url),
                            None => {
                                if let Some(auth) = &self.auth_token {
                                    crate::pro::collab::client::RelayClient::discover_relay(
                                        &sharing_cfg.relay_discovery_url,
                                        &auth.access_token,
                                    ).await
                                } else {
                                    None
                                }
                            }
                        };

                        let sid = d.session_id.clone();
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

                        if let Some(ref relay) = relay_url {
                            // Use relay server
                            if let Some(auth) = &self.auth_token {
                                let client = Arc::new(crate::pro::collab::client::RelayClient::new(
                                    relay.clone(),
                                    auth.access_token.clone(),
                                ));
                                let perm_str = perm.to_string();
                                match client.create_room(&sid, &perm_str, expire).await {
                                    Ok(room) => {
                                        // Start streaming
                                        if client.start_streaming(&tmux_name).await.is_ok() {
                                            // Store client to keep background tasks alive
                                            self.relay_clients.insert(sid.clone(), client);

                                            d.relay_share_url = Some(room.share_url.clone());
                                            d.relay_room_id = Some(room.room_id.clone());
                                            d.web_url = Some(room.share_url.clone());
                                            d.already_sharing = true;

                                            let state = crate::sharing::SharingState {
                                                active: true,
                                                tmate_socket: String::new(),
                                                links: vec![crate::sharing::ShareLink {
                                                    permission: perm,
                                                    ssh_url: String::new(),
                                                    web_url: Some(room.share_url),
                                                    created_at: chrono::Utc::now(),
                                                    expires_at: None,
                                                }],
                                                default_permission: perm,
                                                started_at: chrono::Utc::now(),
                                                auto_expire_minutes: expire,
                                            };

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
                                    }
                                    Err(_e) => {}
                                }
                            }
                        } else if crate::pro::tmate::TmateManager::is_available().await {
                            // Fall back to tmate
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
                                Err(_e) => {}
                            }
                        }
                    }
                }
                KeyCode::Char('c') => {
                    // Copy the best available URL: relay > web > ssh
                    let url = d.relay_share_url.as_ref()
                        .or(d.web_url.as_ref())
                        .or(d.ssh_url.as_ref());
                    if let Some(url) = url {
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
                            let relay = relay_url.clone();
                            let rid = room_id.clone();
                            let tok = token.clone();
                            self.dialog = None;
                            self.state = AppState::Normal;
                            if let Err(e) = self.connect_viewer(&relay, &rid, &tok).await {
                                tracing::warn!("Failed to join session: {}", e);
                            }
                        } else {
                            d.status = Some("Invalid URL. Expected: https://.../share/ROOM_ID?token=TOKEN".to_string());
                        }
                    }
                }
                KeyCode::Backspace => {
                    d.url_input.backspace();
                    d.status = None;
                }
                KeyCode::Char(c) => {
                    d.url_input.insert(c);
                    d.status = None;
                }
                _ => {}
            },

            Dialog::Settings(d) => {
                if d.editing {
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
                                SettingsField::AnalyticsEnabled => {
                                    d.analytics_enabled = !d.analytics_enabled;
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
                                SettingsField::AnalyticsEnabled => {
                                    d.analytics_enabled = !d.analytics_enabled;
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
                                    // Pack link: open in browser
                                    #[cfg(feature = "pro")]
                                    SettingsField::NotifPackLink => {
                                        let _ = open::that("https://peonping.com");
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
            let old_tmux_name = inst.tmux_name();
            inst.title = title.to_string();
            inst.label = label.to_string();
            inst.label_color = label_color;

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
        if self.tree.is_empty() {
            return;
        }
        if self.selected_index + 1 < self.tree.len() {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    /// Visible tree rows (total height minus header, status bar, borders)
    #[cfg(feature = "pro")]
    fn visible_tree_height(&self) -> usize {
        self.height.saturating_sub(5) as usize
    }

    /// Jump cursor down (Ctrl+D)
    #[cfg(feature = "pro")]
    fn move_half_page_down(&mut self) {
        let jump = self.jump_lines.max(1);
        let max = self.tree.len().saturating_sub(1);
        self.selected_index = (self.selected_index + jump).min(max);
    }

    /// Jump cursor up (Ctrl+U)
    #[cfg(feature = "pro")]
    fn move_half_page_up(&mut self) {
        let jump = self.jump_lines.max(1);
        self.selected_index = self.selected_index.saturating_sub(jump);
    }

    /// Keep cursor ~SCROLLOFF lines from viewport edges (like vim `set scrolloff=5`)
    #[cfg(feature = "pro")]
    fn enforce_scrolloff(&mut self) {
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

    /// Get the tmux session name for a session ID.
    /// Looks up the instance to use its stored tmux name, falling back to legacy format.
    fn tmux_name_for_id(&self, id: &str) -> String {
        self.sessions_by_id
            .get(id)
            .and_then(|&idx| self.sessions.get(idx))
            .map(|s| s.tmux_name())
            .unwrap_or_else(|| TmuxManager::session_name_legacy(id))
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
    fn focus_tree_on_session_id(&mut self, id: &str) {
        if let Some(pos) = self
            .tree
            .iter()
            .position(|item| matches!(item, TreeItem::Session { id: sid, .. } if sid == id))
        {
            self.selected_index = pos;
            self.enforce_scrolloff();
            self.on_navigation();
            self.preview.clear();
        }
    }

    /// Find session by tmux session name (matches against each session's tmux_name())
    fn find_session_by_tmux_name(&self, tmux_name: &str) -> Option<Instance> {
        self.sessions
            .iter()
            .find(|s| s.tmux_name() == tmux_name)
            .cloned()
    }

    /// Queue attach to selected session (performed in event loop)
    async fn queue_attach_selected(&mut self) -> Result<()> {
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
            let tmux_session = session.tmux_name();

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
    async fn stop_selected(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            let tmux_session = session.tmux_name();

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
            let tmux_session = session.tmux_name();

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

    #[cfg(feature = "pro")]
    pub fn list_state(&self) -> &ratatui::widgets::ListState {
        &self.list_state
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

    pub fn quit_confirm_dialog(&self) -> bool {
        matches!(self.dialog.as_ref(), Some(Dialog::QuitConfirm))
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

    pub fn settings_dialog(&self) -> Option<&SettingsDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::Settings(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn share_dialog(&self) -> Option<&ShareDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::Share(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn create_relationship_dialog(&self) -> Option<&CreateRelationshipDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::CreateRelationship(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn annotate_dialog(&self) -> Option<&crate::ui::AnnotateDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::Annotate(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn join_session_dialog(&self) -> Option<&crate::ui::JoinSessionDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::JoinSession(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
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

    pub fn active_panel_focused(&self) -> bool {
        self.active_panel_focused
    }

    pub fn active_panel_selected(&self) -> usize {
        self.active_panel_selected
    }

    /// Sessions that deserve attention: actively working OR recently finished (✓ ready).
    pub fn active_sessions(&self) -> Vec<&Instance> {
        self.sessions
            .iter()
            .filter(|s| !matches!(s.status, Status::Idle) || self.is_attention_active(&s.id))
            .collect()
    }

    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    pub fn selected_relationship_index(&self) -> usize {
        self.selected_relationship_index
    }

    pub fn snapshot_count(&self, relationship_id: &str) -> usize {
        self.relationship_snapshot_counts.get(relationship_id).copied().unwrap_or(0)
    }

    /// Connect to a shared session as a viewer via relay WebSocket.
    #[cfg(feature = "pro")]
    pub async fn connect_viewer(&mut self, relay_url: &str, room_id: &str, viewer_token: &str) -> Result<()> {
        use crate::pro::collab::protocol::ControlMessage;

        let terminal_content = Arc::new(tokio::sync::Mutex::new(Vec::<u8>::new()));
        let terminal_size = Arc::new(tokio::sync::Mutex::new((80u16, 24u16)));
        let viewer_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let connected = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let ws_url = format!("{}/ws/{}", relay_url, room_id)
            .replace("https://", "wss://")
            .replace("http://", "ws://");

        // Clone Arcs for the spawned task
        let content_clone = terminal_content.clone();
        let size_clone = terminal_size.clone();
        let count_clone = viewer_count.clone();
        let connected_clone = connected.clone();
        let token = viewer_token.to_string();

        let task_handle = tokio::spawn(async move {
            use futures_util::{SinkExt, StreamExt};
            use base64::Engine;

            let (ws_stream, _) = match tokio_tungstenite::connect_async(&ws_url).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Viewer WS connect failed: {}", e);
                    return;
                }
            };

            let (mut ws_write, mut ws_read) = ws_stream.split();

            // Send ViewerAuth
            let auth_msg = ControlMessage::ViewerAuth {
                token,
                user_token: None,
            };
            let json = match serde_json::to_string(&auth_msg) {
                Ok(j) => j,
                Err(e) => {
                    tracing::warn!("Failed to serialize viewer auth: {}", e);
                    return;
                }
            };
            if ws_write.send(tokio_tungstenite::tungstenite::Message::Text(json.into())).await.is_err() {
                return;
            }

            // Wait for AuthResult
            if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) = ws_read.next().await {
                if let Ok(ControlMessage::AuthResult { success, .. }) = serde_json::from_str(&text) {
                    if !success {
                        tracing::warn!("Viewer auth failed");
                        return;
                    }
                }
            }

            connected_clone.store(true, std::sync::atomic::Ordering::Relaxed);

            // Main receive loop
            while let Some(Ok(msg)) = ws_read.next().await {
                match msg {
                    tokio_tungstenite::tungstenite::Message::Binary(data) => {
                        // Append raw PTY output, cap at 2MB to prevent unbounded growth
                        // (snapshots will reset the buffer periodically)
                        let mut buf = content_clone.lock().await;
                        buf.extend_from_slice(&data);
                        const MAX_VIEWER_BUF: usize = 2 * 1024 * 1024;
                        if buf.len() > MAX_VIEWER_BUF {
                            // Keep only the last 1MB
                            let drain_to = buf.len() - (MAX_VIEWER_BUF / 2);
                            buf.drain(..drain_to);
                        }
                    }
                    tokio_tungstenite::tungstenite::Message::Text(text) => {
                        match serde_json::from_str::<ControlMessage>(&text) {
                            Ok(ControlMessage::Snapshot { cols, rows, data }) => {
                                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&data) {
                                    // Replace content with snapshot (keyframe)
                                    *content_clone.lock().await = decoded;
                                    *size_clone.lock().await = (cols, rows);
                                }
                            }
                            Ok(ControlMessage::Resize { cols, rows }) => {
                                *size_clone.lock().await = (cols, rows);
                            }
                            Ok(ControlMessage::ViewerCount { count }) => {
                                count_clone.store(count, std::sync::atomic::Ordering::Relaxed);
                            }
                            Ok(ControlMessage::RoomClosed { .. }) => {
                                connected_clone.store(false, std::sync::atomic::Ordering::Relaxed);
                                break;
                            }
                            _ => {}
                        }
                    }
                    tokio_tungstenite::tungstenite::Message::Close(_) => {
                        connected_clone.store(false, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    _ => {}
                }
            }

            connected_clone.store(false, std::sync::atomic::Ordering::Relaxed);
        });

        self.viewer_state = Some(ViewerState {
            session_name: format!("Room {}", &room_id[..8.min(room_id.len())]),
            terminal_content,
            terminal_size,
            viewer_count,
            connected,
            task_handle: Some(task_handle),
        });

        self.state = AppState::ViewerMode;
        Ok(())
    }

    /// Disconnect from a viewed session and return to normal mode.
    #[cfg(feature = "pro")]
    pub fn disconnect_viewer(&mut self) {
        if let Some(vs) = self.viewer_state.take() {
            if let Some(handle) = vs.task_handle {
                handle.abort();
            }
        }
        self.state = AppState::Normal;
    }

    /// Handle key events in viewer mode.
    #[cfg(feature = "pro")]
    async fn handle_viewer_key(&mut self, key: KeyCode, _modifiers: KeyModifiers) -> Result<()> {
        match key {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.disconnect_viewer();
            }
            _ => {}
        }
        Ok(())
    }

    /// Get the current viewer state (for rendering).
    #[cfg(feature = "pro")]
    pub fn viewer_state(&self) -> Option<&ViewerState> {
        self.viewer_state.as_ref()
    }

    /// Apply settings from the dialog: update config, save to disk, hot-reload subsystems.
    async fn apply_settings(&mut self) -> Result<()> {
        let Some(Dialog::Settings(d)) = self.dialog.as_ref() else {
            return Ok(());
        };

        // Update AI config
        #[cfg(feature = "max")]
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
        self.config.sharing.tmate_server_host = d.tmate_host.text().trim().to_string();
        self.config.sharing.tmate_server_port =
            d.tmate_port.text().trim().parse().unwrap_or(22);
        self.config.sharing.default_permission = d.default_permission.clone();
        let expire = d.auto_expire.text().trim().to_string();
        self.config.sharing.auto_expire_minutes = if expire.is_empty() {
            None
        } else {
            expire.parse().ok()
        };

        // Update notification config (Pro)
        #[cfg(feature = "pro")]
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
        self.config.analytics.enabled = d.analytics_enabled;
        self.config.jump_lines = Some(
            d.jump_lines.text().trim().parse().unwrap_or(10),
        );
        self.config.ready_ttl_minutes = Some(
            d.ready_ttl.text().trim().parse().unwrap_or(40),
        );

        // Save to disk
        self.config.save()?;

        // Hot-reload: update attention TTL
        self.attention_ttl =
            Duration::from_secs(self.config.ready_ttl_minutes() * 60);

        // Hot-reload: update jump_lines
        #[cfg(feature = "pro")]
        {
            self.jump_lines = self.config.jump_lines();
        }

        // Hot-reload: reload notification manager with new config
        #[cfg(feature = "pro")]
        {
            self.notification_manager.reload_pack(self.config.notification());
        }

        // Hot-reload: recreate AI summarizer with new config
        #[cfg(feature = "max")]
        {
            let is_max = self
                .auth_token
                .as_ref()
                .map_or(false, |t| t.is_max());
            if is_max {
                self.summarizer =
                    crate::ai::Summarizer::from_config(self.config.ai());
            }
        }

        // Close dialog
        self.dialog = None;
        self.state = AppState::Normal;
        Ok(())
    }

    /// Test AI connection from settings dialog.
    async fn test_ai_connection(&mut self) {
        #[cfg(feature = "max")]
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
        }

        #[cfg(not(feature = "max"))]
        {
            if let Some(Dialog::Settings(d)) = self.dialog.as_mut() {
                d.ai_test_status = Some("✗ AI requires Max tier build".to_string());
            }
        }
    }
}
