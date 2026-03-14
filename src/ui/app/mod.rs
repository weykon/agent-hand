use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, EventStream, KeyCode,
        KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::{mpsc, Mutex, RwLock};

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
use super::{CreateRelationshipDialog, CreateRelationshipField, OrphanedRoomsDialog, ShareDialog};

#[cfg(feature = "max")]
use super::{AiAnalysisDialog, AiAnalysisMode, BehaviorAnalysisDialog};

pub(super) mod activity;
mod control;
mod sessions;
mod navigation;
mod keys;
mod dialogs;
mod search;
#[cfg(feature = "pro")]
#[path = "../../../pro/src/ui/viewer.rs"]
mod viewer;
pub(super) mod sound_task;
#[cfg(feature = "max")]
#[path = "../../../pro/src/ui/ws.rs"]
mod ws;

// Pro/Max key handlers — files live in pro/ but compile as submodules of this module
#[cfg(feature = "pro")]
#[path = "../../../pro/src/ui/keys_pro.rs"]
mod keys_pro;
#[cfg(feature = "max")]
#[path = "../../../pro/src/ui/keys_max.rs"]
mod keys_max;
#[cfg(feature = "pro")]
#[path = "../../../pro/src/ui/dialog_handlers.rs"]
mod dialog_handlers;
#[cfg(feature = "max")]
#[path = "../../../pro/src/ui/dialog_handlers_max.rs"]
mod dialog_handlers_max;

// Pro/Max types re-exported from pro module
#[cfg(feature = "pro")]
pub(super) use crate::pro::ui::types::{
    ShareTaskResult, ShareTaskError, OrphanedRoomInfo, ViewerSessionInfo,
    ViewerSessionStatus, ViewerState, ToastNotification,
    live_url_validation_hint, build_relationship_context,
};
#[cfg(feature = "pro")]
pub(super) use crate::pro::ui::app_state::ProAppState;
#[cfg(feature = "max")]
pub(super) use crate::pro::ui::app_state::MaxAppState;

/// Tracks which panel the user attached from, so Ctrl+Q return restores focus correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AttachSource {
    ActivePanel,
    TreePanel,
}

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

    // Canvas workflow editor
    canvas_state: crate::ui::canvas::CanvasState,
    canvas_focused: bool,
    canvas_rx: mpsc::UnboundedReceiver<crate::ui::canvas::CanvasRequest>,
    _canvas_socket: crate::ui::canvas::socket::CanvasSocketServer,

    // Control socket for external session/group/tag management
    control_rx: mpsc::UnboundedReceiver<crate::control::socket::ControlRequest>,
    _control_socket: crate::control::socket::ControlSocketServer,
    /// Per-profile runtime coordination directory.
    runtime_dir: std::path::PathBuf,
    language: crate::i18n::Language,
    show_onboarding: bool,

    // Search state
    search_query: String,
    search_results: Vec<String>,
    search_selected: usize,

    // Dialog state
    dialog: Option<Dialog>,

    // Deferred actions that require terminal access
    pending_attach: Option<String>,
    last_attach_source: Option<AttachSource>,

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

    // Hook socket receiver (push events from agent-hand-bridge binary via Unix socket)
    hook_rx: mpsc::UnboundedReceiver<crate::hooks::HookEvent>,
    _hook_socket: crate::hooks::HookSocketServer,
    /// Broadcast sender for forwarding JSONL fallback events to background subscribers (sound task).
    hook_broadcast_tx: tokio::sync::broadcast::Sender<crate::hooks::HookEvent>,
    pending_hook_events: Vec<crate::hooks::HookEvent>,

    // Info bar (version update / tier mismatch hint). Auto-expires after 8 seconds.
    info_bar_message: Option<(String, ratatui::style::Color, Instant)>,
    info_bar_rx: Option<tokio::sync::oneshot::Receiver<Option<(String, ratatui::style::Color)>>>,

    // PTY monitoring (background task + shared state)
    ptmx_state: crate::tmux::ptmx::SharedPtmxState,
    _ptmx_task: tokio::task::JoinHandle<()>,
    cached_ptmx_total: u32,
    cached_ptmx_max: u32,

    // Session ID scanner (background task + shared state)
    scan_state: crate::tmux::session_id_scanner::SharedScanState,
    _scan_task: tokio::task::JoinHandle<()>,

    // UI animation
    tick_count: u64,
    attention_ttl: Duration,

    // Transition animation engine
    transition_engine: crate::ui::transition::TransitionEngine,
    // Startup logo phase
    startup_phase: crate::ui::transition::StartupPhase,
    startup_started_at: Option<Instant>,

    // Async operation activity tracking (spinner in status bar)
    activity: activity::ActivityTracker,

    // Backend
    storage: Arc<Mutex<Storage>>,
    tmux: Arc<TmuxManager>,
    analytics: crate::analytics::ActivityTracker,
    config: crate::config::ConfigFile,

    // Auth
    auth_token: Option<crate::auth::AuthToken>,

    // Device slot heartbeat
    heartbeat_rx: Option<tokio::sync::oneshot::Receiver<crate::error::Result<crate::auth::HeartbeatResponse>>>,
    last_heartbeat: Instant,

    // List viewport state (scroll offset + selection)
    list_state: ratatui::widgets::ListState,
    scroll_padding: usize,

    // Mouse capture state
    mouse_captured: bool,
    /// Set when settings change mouse_capture; applied next event loop iteration.
    mouse_capture_changed: bool,

    // Sound notifications — background task plays sounds independently
    attached_session: sound_task::AttachedSession,
    sound_config: sound_task::SharedNotificationConfig,

    // Chat panel state
    chat_visible: bool,
    chat_service: Option<crate::chat::ChatService>,
    chat_input: String,
    chat_scroll: u16,
    chat_conversation_id: Option<String>,

    /// Action sender for dispatching WASM canvas events to the ActionExecutor.
    #[cfg(feature = "wasm")]
    action_tx: Option<mpsc::UnboundedSender<crate::agent::Action>>,

    // Pro-tier consolidated state
    #[cfg(feature = "pro")]
    pub(super) pro: ProAppState,

    // Max-tier consolidated state
    #[cfg(feature = "max")]
    pub(super) max: MaxAppState,
}


/// Check if a canvas op targets projection-prefixed nodes (ap_* or wasm_*).
fn is_projection_op(op: &crate::ui::canvas::CanvasOp) -> bool {
    use crate::ui::canvas::CanvasOp;
    match op {
        CanvasOp::AddNode { id, .. }
        | CanvasOp::RemoveNode { id }
        | CanvasOp::UpdateNode { id, .. }
        | CanvasOp::SetMetadata { node_id: id, .. } => {
            id.starts_with("ap_") || id.starts_with("wasm_")
        }
        CanvasOp::ClearPrefix { .. } => true,
        CanvasOp::Batch { ops } => ops.iter().all(|o| is_projection_op(o)),
        _ => false,
    }
}

/// Decide whether mouse capture should be enabled based on config + environment.
fn resolve_mouse_capture(config: &crate::config::ConfigFile) -> bool {
    use crate::config::MouseCaptureMode;
    match config.mouse_capture() {
        MouseCaptureMode::On => true,
        MouseCaptureMode::Off => false,
        MouseCaptureMode::Auto => {
            // Nested tmux: outer tmux grabs mouse events → disable
            if std::env::var("TMUX").is_ok() {
                return false;
            }
            // Apple Terminal has poor mouse support
            if std::env::var("TERM_PROGRAM")
                .map(|v| v == "Apple_Terminal")
                .unwrap_or(false)
            {
                return false;
            }
            true
        }
    }
}

impl App {
    const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);
    const NAVIGATION_SETTLE: Duration = Duration::from_millis(300);
    const STATUS_REFRESH: Duration = Duration::from_secs(1);
    const CACHE_REFRESH: Duration = Duration::from_secs(2);
    const STATUS_COOLDOWN: Duration = Duration::from_secs(2);
    const STATUS_FALLBACK: Duration = Duration::from_secs(10);

    /// Dynamic tick rate: 60 FPS during animations, 4 FPS otherwise.
    fn tick_rate(&self) -> Duration {
        if self.transition_engine.is_animating() || self.state == AppState::Startup {
            Duration::from_millis(16) // ~60 FPS
        } else {
            Duration::from_millis(250) // ~4 FPS
        }
    }

    /// Create new application
    pub async fn new(profile: &str) -> Result<Self> {
        let storage = Storage::new(profile).await?;
        let (mut sessions, groups, relationships) = storage.load().await?;
        // Status is derived from tmux probes; the persisted value can be stale across restarts.
        // Reset to avoid treating old Running→Idle as a fresh completion.
        // Also clear stale sharing state — relay rooms are ephemeral and won't survive TUI restart.
        // Enforce default group: sessions with empty group_path get assigned to "default".
        for s in &mut sessions {
            s.status = Status::Idle;
            if s.sharing.as_ref().is_some_and(|sh| sh.active) {
                s.sharing = None;
            }
            if s.group_path.is_empty() {
                s.group_path = "default".to_string();
            }
        }

        let tmux = TmuxManager::new(profile);

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
        let ptmx_task = spawn_ptmx_monitor(system_ptmx_max, Arc::clone(&ptmx_state), tmux.server_name().to_string());

        // Create shared session ID scanner state and spawn background scanner
        let scan_state: crate::tmux::session_id_scanner::SharedScanState =
            Arc::new(RwLock::new(crate::tmux::session_id_scanner::ScanState::default()));
        let scan_task = crate::tmux::session_id_scanner::spawn_session_id_scanner(
            Arc::clone(&scan_state),
            tmux.server_name().to_string(),
        );

        // Start canvas socket server for external tool communication
        let (canvas_rx, canvas_socket) = crate::ui::canvas::socket::CanvasSocketServer::start();

        // Start control socket server for external session/group/tag management
        let (control_rx, control_socket) = crate::control::socket::ControlSocketServer::start();

        // Start hook socket server for receiving events from agent-hand-bridge binary
        let (hook_rx, hook_socket) = crate::hooks::HookSocketServer::start();
        let hook_broadcast_tx = hook_socket.broadcast_tx();

        // Spawn background JSONL poller: polls hook-events.jsonl independently of the
        // main event loop (which blocks during tmux attach) and forwards events to the
        // broadcast channel so background subscribers (sound task) always receive them.
        {
            let tx = hook_broadcast_tx.clone();
            tokio::spawn(async move {
                let mut receiver = match crate::hooks::EventReceiver::new() {
                    Ok(r) => r,
                    Err(_) => return,
                };
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
                loop {
                    interval.tick().await;
                    for event in receiver.poll() {
                        let _ = tx.send(event);
                    }
                }
            });
        }

        // Spawn agent framework: SystemRunner + ActionExecutor (replaces sound_task)
        #[cfg(feature = "wasm")]
        let wasm_action_tx;
        let chat_response_rx;
        let (attached_session, sound_config) = {
            let attached: sound_task::AttachedSession =
                Arc::new(std::sync::RwLock::new(None));
            let notif_cfg: sound_task::SharedNotificationConfig =
                Arc::new(std::sync::RwLock::new(config.notification().clone()));
            {
                let system_rx = hook_socket.subscribe();
                let cfg_clone = Arc::clone(&notif_cfg);
                let attached_clone = Arc::clone(&attached);

                // Action channel: Systems produce → Executor consumes
                let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();

                // Clone action_tx for TUI → executor event dispatch (WASM canvas clicks)
                #[cfg(feature = "wasm")]
                {
                    wasm_action_tx = action_tx.clone();
                }

                let agent_hand_base = Storage::get_agent_hand_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from(".agent-hand"))
                    .join("profiles")
                    .join(profile);

                // Progress dir: ~/.agent-hand/profiles/{profile}/progress/
                let progress_dir = agent_hand_base.join("progress");

                // Runtime dir: ~/.agent-hand/profiles/{profile}/agent-runtime/
                let runtime_dir = agent_hand_base.join("agent-runtime");

                // Build SystemRunner with registered Systems
                let mut runner = crate::agent::runner::SystemRunner::new();
                runner.register(crate::agent::systems::sound::SoundSystem::new(
                    cfg_clone,
                    attached_clone,
                ));
                runner.register(crate::agent::systems::token_burst::TokenBurstSystem::new());
                runner.register(crate::agent::systems::progress::ProgressSystem);
                runner.register(crate::agent::systems::context::ContextGuardSystem::new(
                    config.context_bridge().clone(),
                    runtime_dir.clone(),
                ));
                runner.register(crate::agent::systems::chat::ChatSystem::new());

                // Executor handles side effects (sound playback, progress files, context injection, audit)
                #[allow(unused_mut)]
                let mut executor = crate::agent::runner::ActionExecutor::new(
                    Arc::clone(&notif_cfg),
                    progress_dir,
                    runtime_dir,
                );

                // Wire up WASM canvas op sender so executor can push ops to the TUI
                #[cfg(all(unix, feature = "wasm"))]
                {
                    executor.set_canvas_op_tx(canvas_socket.op_sender());
                }

                // Wire up chat response channel: executor → ChatService (via App)
                let (chat_tx, chat_rx) = mpsc::unbounded_channel::<crate::chat::ChatResponsePayload>();
                executor.set_chat_response_tx(chat_tx);
                chat_response_rx = chat_rx;

                tokio::spawn(runner.run(system_rx, action_tx));
                tokio::spawn(executor.run(action_rx));
            }
            (attached, notif_cfg)
        };

        // Start WebSocket data transport server (Max tier)
        #[cfg(feature = "max")]
        let (ws_broadcast_tx, ws_request_rx, ws_server) = {
            let (broadcast_tx, _) = tokio::sync::broadcast::channel::<crate::ws::BroadcastUpdate>(64);
            let ws_cfg = config.ws();
            let is_max = crate::auth::AuthToken::load().map_or(false, |t| t.is_max());
            if ws_cfg.enabled && is_max {
                match crate::ws::server::WsServer::start(ws_cfg, broadcast_tx.clone()).await {
                    Ok((rx, server)) => {
                        tracing::info!("WebSocket data transport started on ws://{}", server.addr);
                        (broadcast_tx, rx, Some(server))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to start WebSocket server: {e}");
                        let (_, rx) = mpsc::unbounded_channel();
                        (broadcast_tx, rx, None)
                    }
                }
            } else {
                let (_, rx) = mpsc::unbounded_channel();
                (broadcast_tx, rx, None)
            }
        };

        // Compute per-group canvas directory and load initial canvas (Pro only)
        #[cfg(feature = "pro")]
        let (initial_canvas_dir, initial_canvas_state) = {
            let canvas_dir = storage.canvas_dir();

            // Migration: move old global canvas.json → per-group _default.json
            let old_canvas_path = crate::session::Storage::get_agent_hand_dir()
                .ok()
                .map(|d| d.join("canvas.json"));
            let default_target = canvas_dir.join("_default.json");
            if let Some(old_path) = old_canvas_path.as_deref() {
                if old_path.exists() && !default_target.exists() {
                    let _ = std::fs::create_dir_all(&canvas_dir);
                    if std::fs::rename(old_path, &default_target).is_err() {
                        // Fallback: copy + delete
                        if std::fs::copy(old_path, &default_target).is_ok() {
                            let _ = std::fs::remove_file(old_path);
                        }
                    }
                    tracing::info!("Migrated canvas.json → {:?}", default_target);
                }
            }

            let state = crate::ui::canvas::CanvasState::load_for_group(&canvas_dir, "default");
            (Some(canvas_dir), state)
        };

        // Load persisted AI analysis results once
        #[cfg(feature = "max")]
        let (loaded_summaries, loaded_diagrams) = crate::session::Storage::get_agent_hand_dir()
            .ok()
            .map(|d| Self::load_ai_results(&d))
            .unwrap_or_default();

        // Build Pro state
        #[cfg(feature = "pro")]
        let pro_state = {
            let mut ps = ProAppState::new(&config);
            ps.canvas_dir = initial_canvas_dir;
            ps
        };

        // Build Max state
        #[cfg(feature = "max")]
        let max_state = MaxAppState::new(
            &config,
            loaded_summaries,
            loaded_diagrams,
            ws_broadcast_tx,
            ws_request_rx,
            ws_server,
        );

        let initial_state = if config.animations_enabled() {
            AppState::Startup
        } else {
            AppState::Normal
        };
        let mut app = Self {
            width: 0,
            height: 0,
            state: initial_state,
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
            canvas_state: {
                #[cfg(feature = "pro")]
                { initial_canvas_state }
                #[cfg(not(feature = "pro"))]
                { crate::ui::canvas::CanvasState::new() }
            },
            canvas_focused: false,
            canvas_rx,
            _canvas_socket: canvas_socket,
            control_rx,
            _control_socket: control_socket,
            runtime_dir: Storage::get_agent_hand_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from(".agent-hand"))
                .join("profiles")
                .join(profile)
                .join("agent-runtime"),
            language: config.language.as_ref()
                .map(|s| crate::i18n::Language::from_str(s))
                .unwrap_or_default(),
            show_onboarding: config.first_launch.unwrap_or(true),
            search_query: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            dialog: None,
            pending_attach: None,
            last_attach_source: None,
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
            hook_rx: hook_rx,
            _hook_socket: hook_socket,
            hook_broadcast_tx: hook_broadcast_tx.clone(),
            pending_hook_events: Vec::new(),
            info_bar_message: None,
            info_bar_rx: None,
            tick_count: 0,
            attention_ttl,
            transition_engine: crate::ui::transition::TransitionEngine::new(
                config.animations_enabled(),
            ),
            startup_phase: crate::ui::transition::StartupPhase::Logo,
            startup_started_at: None,
            activity: activity::ActivityTracker::default(),
            storage: Arc::new(Mutex::new(storage)),
            tmux: Arc::new(tmux),
            analytics,
            config: config.clone(),
            ptmx_state,
            _ptmx_task: ptmx_task,
            cached_ptmx_total: 0,
            cached_ptmx_max: system_ptmx_max,
            scan_state,
            _scan_task: scan_task,
            auth_token: crate::auth::AuthToken::load(),
            heartbeat_rx: None,
            last_heartbeat: Instant::now(),
            list_state: ratatui::widgets::ListState::default(),
            scroll_padding: config.scroll_padding(),
            mouse_captured: resolve_mouse_capture(&config),
            mouse_capture_changed: false,
            attached_session,
            sound_config,
            chat_visible: false,
            chat_service: Some(crate::chat::ChatService::new(
                hook_broadcast_tx.clone(),
                chat_response_rx,
            )),
            chat_input: String::new(),
            chat_scroll: 0,
            chat_conversation_id: None,
            #[cfg(feature = "wasm")]
            action_tx: Some(wasm_action_tx),
            #[cfg(feature = "pro")]
            pro: pro_state,
            #[cfg(feature = "max")]
            max: max_state,
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

        // Check for orphaned relay rooms from a previous session
        #[cfg(feature = "pro")]
        {
            let mut ledger = crate::pro::collab::ledger::RoomLedger::load();
            if !ledger.entries.is_empty() {
                let mut orphaned = Vec::new();
                let mut stale_ids = Vec::new();

                for entry in &ledger.entries {
                    match crate::pro::collab::client::RelayClient::check_room_status(
                        &entry.relay_url,
                        &entry.room_id,
                        &entry.host_token,
                    ).await {
                        Some(status) => {
                            orphaned.push(OrphanedRoomInfo {
                                room_id: entry.room_id.clone(),
                                session_id: status.session_id,
                                relay_url: entry.relay_url.clone(),
                                host_token: entry.host_token.clone(),
                                viewer_count: status.viewer_count,
                                created_at: status.created_at,
                            });
                        }
                        None => {
                            // Room no longer exists (404 or unreachable)
                            stale_ids.push(entry.room_id.clone());
                        }
                    }
                }

                // Remove stale entries from ledger
                for id in &stale_ids {
                    ledger.remove(id);
                }

                if !orphaned.is_empty() {
                    tracing::info!("Found {} orphaned relay room(s)", orphaned.len());
                    app.pro.orphaned_rooms = orphaned;
                }
            }
        }

        Ok(app)
    }

    /// Run the TUI application
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if self.mouse_captured {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        } else {
            execute!(stdout, EnterAlternateScreen)?;
        }
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.clear()?;

        // Run event loop
        let result = self.event_loop(&mut terminal).await;

        // Restore terminal
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

        result
    }

    /// Main event loop — multiplexes terminal events, canvas socket ops, and tick timer.
    async fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let mut event_stream = EventStream::new();
        // Dynamic tick rate: uses sleep_until instead of fixed interval.
        // During animations → 16ms (60 FPS); otherwise → 250ms (4 FPS).
        let mut next_tick = tokio::time::Instant::now() + self.tick_rate();

        // Initial preview/status
        self.on_navigation();

        // Show orphaned rooms dialog if any were detected at startup
        #[cfg(feature = "pro")]
        if !self.pro.orphaned_rooms.is_empty() {
            let rooms = std::mem::take(&mut self.pro.orphaned_rooms);
            self.dialog = Some(Dialog::OrphanedRooms(OrphanedRoomsDialog::new(rooms)));
            self.state = AppState::Dialog;
        }

        loop {
            // Hot-reload mouse capture state (triggered by settings save)
            if self.mouse_capture_changed {
                self.mouse_capture_changed = false;
                let want = resolve_mouse_capture(&self.config);
                if want != self.mouse_captured {
                    if want {
                        execute!(terminal.backend_mut(), EnableMouseCapture)?;
                    } else {
                        execute!(terminal.backend_mut(), DisableMouseCapture)?;
                    }
                    self.mouse_captured = want;
                }
            }

            // Draw UI
            terminal.draw(|f| {
                self.width = f.area().width;
                self.height = f.area().height;
                let area = f.area();

                super::render::draw(f, self);

                // Transition animation lifecycle
                if self.transition_engine.should_start_transition() {
                    self.transition_engine
                        .start_from_last_frame(f.buffer_mut(), area);
                } else if !self.transition_engine.is_animating() {
                    self.transition_engine.save_frame(f.buffer_mut(), area);
                }
                if self.transition_engine.is_animating() {
                    self.transition_engine.apply_frame(f.buffer_mut());
                }
            })?;

            // Multiplex: terminal events, canvas socket ops, tick timer
            tokio::select! {
                maybe_event = event_stream.next() => {
                    match maybe_event {
                        Some(Ok(CrosstermEvent::Key(key))) => {
                            self.handle_key(key.code, key.modifiers).await?;
                        }
                        Some(Ok(CrosstermEvent::Resize(_, _))) => {
                            // Cancel any in-progress animation on resize
                            self.transition_engine.cancel();
                            // Next draw will re-render with new size
                        }
                        Some(Ok(CrosstermEvent::Mouse(mouse))) => {
                            if self.mouse_captured {
                                self.handle_mouse_event(mouse);
                            }
                        }
                        Some(Err(_)) | None => {
                            // Stream ended or errored — treat as quit
                            break;
                        }
                        _ => {}
                    }
                }
                Some((op, reply_tx)) = self.canvas_rx.recv() => {
                    // Agent view: accept incoming ops (this is where agent/WASM ops arrive).
                    // User view: accept only non-projection ops (user canvas nodes).
                    let response = if self.canvas_state.is_projection_view() {
                        // Agent view — clear old projection nodes then apply new batch
                        self.canvas_state.handle_op(op)
                    } else {
                        // User view — reject projection-prefixed ops to protect user nodes
                        let dominated = matches!(&op,
                            crate::ui::canvas::CanvasOp::Batch { ops } if ops.iter().all(|o| is_projection_op(o)),
                        ) || is_projection_op(&op);
                        if dominated {
                            crate::ui::canvas::CanvasResponse::Ok {
                                message: "ignored: user view active".into(),
                            }
                        } else {
                            self.canvas_state.handle_op(op)
                        }
                    };
                    let _ = reply_tx.send(response);
                }
                Some((op, reply_tx)) = self.control_rx.recv() => {
                    let response = self.handle_control_op(op).await;
                    let _ = reply_tx.send(response);
                }
                Some(event) = self.hook_rx.recv() => {
                    self.pending_hook_events.push(event);
                }
                _ = tokio::time::sleep_until(next_tick) => {
                    self.tick().await?;
                    next_tick = tokio::time::Instant::now() + self.tick_rate();
                }
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

                // Pro: restore panel focus based on where the user attached from
                #[cfg(feature = "pro")]
                {
                    let is_pro = self.auth_token.as_ref().map_or(false, |t| t.is_pro());
                    let active_count = self.active_sessions().len();
                    let want_active = match self.last_attach_source {
                        Some(AttachSource::ActivePanel) => true,
                        Some(AttachSource::TreePanel) => false,
                        // Fallback: focus active panel if available (legacy behavior)
                        None => active_count > 0,
                    };
                    if is_pro && want_active && active_count > 0 {
                        self.active_panel_focused = true;
                        if self.active_panel_selected >= active_count {
                            self.active_panel_selected = active_count.saturating_sub(1);
                        }
                    } else {
                        self.active_panel_focused = false;
                    }
                    self.last_attach_source = None;
                }
            }

            // Auto-attach to viewer tmux session when entering ViewerMode
            #[cfg(feature = "pro")]
            if self.state == AppState::ViewerMode && self.pro.viewer_state.is_some() {
                self.perform_viewer_attach(terminal).await?;
            }

            if self.should_quit {
                // Auto-save canvas state for current group before exit (Pro only)
                #[cfg(feature = "pro")]
                if let Some(ref dir) = self.pro.canvas_dir {
                    if let Err(e) = self.canvas_state.save_for_group(dir, &self.pro.canvas_group) {
                        tracing::warn!("Failed to save canvas for group '{}': {}", self.pro.canvas_group, e);
                    }
                }
                // Auto-save AI analysis results before exit (Max only)
                #[cfg(feature = "max")]
                if let Ok(dir) = crate::session::Storage::get_agent_hand_dir() {
                    Self::save_ai_results(&dir, &self.max.summary_results, &self.max.diagram_results);
                }
                break;
            }
        }

        Ok(())
    }

    async fn tick(&mut self) -> Result<()> {
        self.tick_count = self.tick_count.wrapping_add(1);

        // Advance transition engine
        self.transition_engine.tick();

        // Startup logo phase progression
        if self.state == AppState::Startup {
            // Initialize startup timer on first tick
            if self.startup_started_at.is_none() {
                self.startup_started_at = Some(Instant::now());
            }
            if let Some(started) = self.startup_started_at {
                let elapsed = started.elapsed().as_millis() as u64;
                self.startup_phase = if elapsed < 1500 {
                    crate::ui::transition::StartupPhase::Logo
                } else if elapsed < 2000 {
                    crate::ui::transition::StartupPhase::FadeOut
                } else {
                    self.state = AppState::Normal;
                    self.startup_started_at = None;
                    self.transition_engine.request_transition();
                    crate::ui::transition::StartupPhase::Done
                };
            }
            return Ok(());
        }

        // Auto-expire stuck activity operations (safety net)
        self.activity.auto_expire();

        if self.is_navigating && self.last_navigation_time.elapsed() > Self::NAVIGATION_SETTLE {
            self.is_navigating = false;

            // Auto-switch canvas to the group under the cursor (debounced by NAVIGATION_SETTLE)
            #[cfg(feature = "pro")]
            {
                let new_group = self.resolve_canvas_group();
                self.switch_canvas_to_group(&new_group);
            }
        }

        // Info bar: spawn background check on first tick, poll on subsequent
        if self.tick_count == 1 && self.info_bar_rx.is_none() {
            let (tx, rx) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let _ = tx.send(crate::update::tui_update_hint().await);
            });
            self.info_bar_rx = Some(rx);
        }
        if let Some(ref mut rx) = self.info_bar_rx {
            if let Ok(result) = rx.try_recv() {
                self.info_bar_message = result.map(|(msg, color)| (msg, color, Instant::now()));
                self.info_bar_rx = None;
            }
        }

        // Device heartbeat: spawn on first tick for Pro/Max users
        if self.tick_count == 1 && self.heartbeat_rx.is_none() {
            if let Some(ref token) = self.auth_token {
                if token.is_pro() || token.is_max() {
                    let token_clone = token.clone();
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    tokio::spawn(async move {
                        let _ = tx.send(token_clone.heartbeat().await);
                    });
                    self.heartbeat_rx = Some(rx);
                    self.last_heartbeat = Instant::now();
                }
            }
        }
        // Poll heartbeat result
        if let Some(ref mut rx) = self.heartbeat_rx {
            if let Ok(result) = rx.try_recv() {
                self.heartbeat_rx = None;
                match result {
                    Ok(crate::auth::HeartbeatResponse::LimitExceeded { device_limit, active_devices, .. }) => {
                        self.set_info_bar(
                            format!(
                                "Device limit reached ({}/{}). Manage: https://agent-hand.dev/account",
                                active_devices, device_limit
                            ),
                            ratatui::style::Color::Yellow,
                        );
                    }
                    Ok(crate::auth::HeartbeatResponse::Ok { .. }) => { /* registered OK */ }
                    Err(e) => {
                        tracing::warn!("Heartbeat failed: {e}");
                    }
                }
            }
        }
        // Re-heartbeat every 24 hours
        if self.tick_count % 10 == 5
            && self.last_heartbeat.elapsed() > Duration::from_secs(86400)
            && self.heartbeat_rx.is_none()
        {
            if let Some(ref token) = self.auth_token {
                if token.is_pro() || token.is_max() {
                    let token_clone = token.clone();
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    tokio::spawn(async move {
                        let _ = tx.send(token_clone.heartbeat().await);
                    });
                    self.heartbeat_rx = Some(rx);
                    self.last_heartbeat = Instant::now();
                }
            }
        }

        // Sync session-linked canvas nodes every ~10 ticks (~2.5s) (Pro only)
        // Only sync sessions belonging to the current canvas group.
        // Only run in Relationship view — projection views are loaded on-demand.
        #[cfg(feature = "pro")]
        if self.tick_count % 10 == 0
            && self.canvas_state.current_view == crate::ui::canvas::CanvasView::User
        {
            let canvas_group = self.pro.canvas_group.clone();
            let session_data: Vec<(String, String, String)> = self.sessions.iter()
                .filter(|s| s.group_path == canvas_group)
                .map(|s| (s.id.clone(), s.title.clone(), format!("{:?}", s.status).to_lowercase()))
                .collect();
            self.canvas_state.sync_session_nodes(&session_data);

            // Max: sync relationship edges onto canvas (only where both sessions are in group)
            if self.auth_token.as_ref().is_some_and(|t| t.is_max()) {
                let group_relationships: Vec<_> = self.relationships.iter()
                    .filter(|r| {
                        let src_in = self.sessions.iter().any(|s| s.id == r.session_a_id && s.group_path == canvas_group);
                        let tgt_in = self.sessions.iter().any(|s| s.id == r.session_b_id && s.group_path == canvas_group);
                        src_in && tgt_in
                    })
                    .cloned()
                    .collect();
                self.canvas_state.sync_relationship_edges(&group_relationships);
            }
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
                    // Clean up relay client if present
                    if let Some(client) = self.pro.relay_clients.remove(id) {
                        let tmux_name = self.sessions.iter()
                            .find(|s| &s.id == id)
                            .map(|s| s.tmux_name())
                            .unwrap_or_else(|| TmuxManager::session_name_legacy(id));
                        client.stop(&tmux_name).await;
                    }
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

        // Poll for control requests from viewers (check every ~5 ticks)
        // Skip when in ViewerMode to avoid dialog overlay on viewer terminal
        #[cfg(feature = "pro")]
        if self.tick_count % 5 == 0 && self.dialog.is_none() && self.state != AppState::ViewerMode {
            self.poll_control_requests().await;
        }

        // Detect viewer join/leave and create toast notifications
        #[cfg(feature = "pro")]
        if self.tick_count % 4 == 0 {
            self.detect_viewer_changes();
            self.pro.toast_notifications.retain(|n| !n.is_expired());
            // Cap queue to prevent unbounded growth from rapid join/leave events
            if self.pro.toast_notifications.len() > 10 {
                self.pro.toast_notifications.drain(0..self.pro.toast_notifications.len() - 10);
            }
        }
        // Auto-expire info bar messages after 8 seconds
        if let Some((_, _, created_at)) = &self.info_bar_message {
            if created_at.elapsed() > Duration::from_secs(8) {
                self.info_bar_message = None;
            }
        }

        // Poll background share connection task (non-blocking share flow)
        #[cfg(feature = "pro")]
        self.poll_share_task().await?;

        // Sync session name from viewer state to ViewerSessionInfo (for panel display)
        #[cfg(feature = "pro")]
        if let Some(ref vs) = self.pro.viewer_state {
            if let Some(ref name) = vs.host_session_name {
                if let Some(session) = self.pro.viewer_sessions.get_mut(&vs.room_id) {
                    if session.session_name.is_none() {
                        session.session_name = Some(name.clone());
                    }
                }
            }
        }

        // With tmux-on-viewer, control request timeout and latency warnings
        // are handled by the pty-viewer process. No viewer-side tick logic needed.

        // Auto-timeout host-side control request dialog after 30 seconds
        #[cfg(feature = "pro")]
        if let Some(Dialog::ControlRequest(ref d)) = self.dialog {
            if d.created_at.elapsed() >= Duration::from_secs(30) {
                let sid = d.session_id.clone();
                let vid = d.viewer_id.clone();
                let name = d.display_name.clone();
                self.dialog = None;
                self.state = AppState::Normal;
                if let Some(client) = self.pro.relay_clients.get(&sid) {
                    client.respond_control(&vid, false).await;
                }
                self.pro.toast_notifications.push(ToastNotification {
                    message: format!("Control request from {} auto-denied (timeout)", name),
                    created_at: Instant::now(),
                    color: ratatui::style::Color::Yellow,
                });
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
        if let Some(ref mut summarizer) = self.max.summarizer {
            for result in summarizer.poll_results() {
                // Clear in-flight tracking for this session
                if self.max.summarizing_session_id.as_deref() == Some(&result.session_id) {
                    self.max.summarizing_session_id = None;
                }
                self.activity.complete(activity::ActivityOp::SummarizingAI);
                let sid = result.session_id.clone();
                self.max.summary_results.insert(result.session_id.clone(), result.summary.clone());
                // Update preview if the summarized session is currently selected (Free tier)
                if self.selected_session().map(|s| s.id.as_str()) == Some(&result.session_id) {
                    self.preview = format!("🤖 AI Summary:\n\n{}", result.summary);
                }
                // Auto-show overlay popup when result arrives
                self.max.show_ai_summary_overlay = true;
                self.max.ai_summary_overlay_id = Some(sid);
                self.max.summary_overlay_scroll = 0;
            }
        }

        // Poll AI diagram results (non-blocking)
        #[cfg(feature = "max")]
        if let Some(ref mut summarizer) = self.max.summarizer {
            for result in summarizer.poll_diagram_results() {
                if self.max.diagramming_session_id.as_deref() == Some(&result.session_id) {
                    self.max.diagramming_session_id = None;
                }
                self.activity.complete(activity::ActivityOp::GeneratingDiagram);
                let sid = result.session_id.clone();
                let is_compact = result.canvas_compact;

                if is_compact {
                    // Canvas-compact diagrams: auto-add to canvas, don't show overlay
                    let session_name = self.session_by_id(&sid)
                        .map(|s| s.title.clone())
                        .unwrap_or_else(|| "Diagram".to_string());
                    let label = format!("📊 {}", session_name);
                    let diagram = result.diagram.clone();
                    if let Some(existing_id) = self.canvas_state.find_ai_content_node(&sid, "canvas_diagram") {
                        self.canvas_state.update_content_node(&existing_id, &label, &diagram);
                    } else {
                        self.canvas_state.add_content_node(&label, &diagram, None, Some((&sid, "canvas_diagram")));
                    }
                    #[cfg(feature = "pro")]
                    {
                        self.canvas_focused = true;
                    }
                    self.preview = format!("📊 Canvas diagram added for: {}", session_name);
                } else {
                    // Full diagram: show overlay
                    self.max.diagram_results.insert(result.session_id.clone(), result);
                    self.max.show_ai_diagram_overlay = true;
                    self.max.ai_diagram_overlay_id = Some(sid);
                    self.max.diagram_overlay_scroll = 0;
                }
            }
        }

        // Poll behavior analysis results (non-blocking)
        #[cfg(feature = "max")]
        if let Some(ref mut summarizer) = self.max.summarizer {
            for result in summarizer.poll_behavior_results() {
                if self.max.analyzing_behavior_session_id.as_deref() == Some(&result.session_id) {
                    self.max.analyzing_behavior_session_id = None;
                }
                let sid = result.session_id.clone();
                self.max.behavior_analysis_results.insert(result.session_id, result.analysis);
                self.max.show_behavior_overlay = true;
                self.max.behavior_overlay_id = Some(sid);
                self.max.behavior_overlay_scroll = 0;
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

        // Session ID scanner: consume results and write new targets every ~20 ticks (~5s)
        if self.tick_count % 20 == 10 {
            // Read scan results and apply to sessions
            let results = {
                let mut guard = self.scan_state.write().await;
                std::mem::take(&mut guard.results)
            };

            let now = chrono::Utc::now();
            let mut changed = false;
            for result in results {
                if let Some(session) = self.sessions.iter_mut().find(|s| s.tmux_name() == result.tmux_session_name) {
                    // Upgrade tool type (Shell → detected tool)
                    if let Some(tool) = result.detected_tool {
                        if session.upgrade_tool(tool) {
                            tracing::info!(
                                "Scanner: upgraded {} tool to {:?}",
                                session.title,
                                tool
                            );
                            changed = true;
                        }
                    }

                    // Set session ID
                    if let Some(ref id) = result.detected_session_id {
                        if session.set_cli_session_id(id, now) {
                            tracing::info!(
                                "Scanner: detected session ID for {}: {}",
                                session.title,
                                id
                            );
                            changed = true;
                        }
                    }
                }
            }

            if changed {
                let storage = self.storage.lock().await;
                let _ = storage.save(&self.sessions, &self.groups, &self.relationships).await;
            }

            // Write new targets for next scan:
            // - Sessions missing IDs (initial detection)
            // - Running sessions (re-scan for freshness — CLI may have restarted)
            let targets: Vec<crate::tmux::session_id_scanner::ScanTarget> = self
                .sessions
                .iter()
                .filter(|s| s.cli_session_id().is_none() || s.status == Status::Running)
                .map(|s| crate::tmux::session_id_scanner::ScanTarget {
                    tmux_session_name: s.tmux_name(),
                    tool: s.tool,
                    project_path: s.project_path.clone(),
                    has_session_id: s.cli_session_id().is_some(),
                })
                .collect();

            if !targets.is_empty() {
                let mut guard = self.scan_state.write().await;
                guard.targets = targets;
            }
        }

        if self.pending_preview_id.is_some()
            && self.last_navigation_time.elapsed() >= Self::PREVIEW_DEBOUNCE
        {
            self.pending_preview_id = None;
            self.update_preview().await?;
        }

        // WebSocket: drain pending requests and broadcast changes (Max tier)
        #[cfg(feature = "max")]
        {
            // Drain all pending WebSocket requests (non-blocking)
            while let Ok((req, reply_tx)) = self.max.ws_request_rx.try_recv() {
                let response = self.handle_ws_request(req);
                let _ = reply_tx.send(response);
            }
            // Broadcast state changes to subscribed WebSocket clients
            self.broadcast_changes();
        }

        Ok(())
    }

    async fn refresh_statuses(&mut self) -> Result<()> {
        let now = Instant::now();

        // Collect session IDs that transition from Running to Idle/Waiting for auto-capture
        let mut running_to_done: Vec<String> = Vec::new();

        // --- Phase 1: Process hook events (event-driven, precise) ---
        // Track which sessions were updated by hooks and what status they got.
        // Running/Waiting from hooks is trusted; Idle is not (polling may detect activity).
        let mut hook_updated: HashMap<String, Status> = HashMap::new();
        #[cfg(feature = "pro")]
        let prompt_collect_on = self.prompt_collection_enabled();

        // Merge events from socket (push, instant) and JSONL poll (fallback).
        // Note: JSONL events are also polled by a background task that forwards
        // them to the broadcast channel (for sound notifications etc.), but we
        // still poll here for the main loop's status tracking.
        let mut all_events = std::mem::take(&mut self.pending_hook_events);
        if let Some(ref mut receiver) = self.event_receiver {
            all_events.extend(receiver.poll());
        }
        // Collect relationship context injection requests (processed after mutable loop)
        #[cfg(feature = "pro")]
        let mut rel_context_requests: Vec<(String, String)> = Vec::new(); // (session_id, rel_id)
        {
            for event in all_events {
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
                    HookEventKind::UserChat { .. } => {
                        // Sideband — don't change session status, skip status update
                        continue;
                    }
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

                // Capture CLI session_id if present in the hook event
                session.set_cli_session_id(&event.session_id, now_utc);

                // Collect user prompt text for behavior analysis (if enabled)
                #[cfg(feature = "pro")]
                if prompt_collect_on && matches!(event.kind, HookEventKind::UserPromptSubmit) {
                    if let Some(ref prompt_text) = event.prompt {
                        let sid = session.id.clone();
                        let dq = self.pro.collected_prompts.entry(sid).or_default();
                        dq.push_back((event.ts, prompt_text.clone()));
                        const MAX_PROMPTS_PER_SESSION: usize = 100;
                        while dq.len() > MAX_PROMPTS_PER_SESSION {
                            dq.pop_front();
                        }
                    }
                }

                // Queue relationship context injection for workspace sessions on prompt submit
                #[cfg(feature = "pro")]
                if matches!(event.kind, HookEventKind::UserPromptSubmit) {
                    if let Some(ref rel_id) = session.relationship_id {
                        rel_context_requests.push((session.id.clone(), rel_id.clone()));
                    }
                }

                self.previous_statuses
                    .insert(session.id.clone(), new_status);
                session.status = new_status;
                hook_updated.insert(session.id.clone(), new_status);
            }
        }

        // Process relationship context injections (outside mutable session borrow)
        #[cfg(feature = "pro")]
        for (session_id, rel_id) in rel_context_requests {
            if let Some(rel) = self.relationships.iter().find(|r| r.id == rel_id) {
                let a_id = rel.session_a_id.clone();
                let b_id = rel.session_b_id.clone();
                let a_title = self.sessions.iter().find(|s| s.id == a_id)
                    .map(|s| s.title.clone()).unwrap_or_default();
                let b_title = self.sessions.iter().find(|s| s.id == b_id)
                    .map(|s| s.title.clone()).unwrap_or_default();
                let a_tmux = self.sessions.iter().find(|s| s.id == a_id)
                    .map(|s| s.tmux_name()).unwrap_or_default();
                let b_tmux = self.sessions.iter().find(|s| s.id == b_id)
                    .map(|s| s.tmux_name()).unwrap_or_default();

                let a_pane = if !a_tmux.is_empty() {
                    self.tmux.capture_pane(&a_tmux, 50).await.unwrap_or_default()
                } else { String::new() };
                let b_pane = if !b_tmux.is_empty() {
                    self.tmux.capture_pane(&b_tmux, 50).await.unwrap_or_default()
                } else { String::new() };

                let context = build_relationship_context(&a_title, &a_pane, &b_title, &b_pane);
                let sidecar_dir = dirs::home_dir()
                    .unwrap_or_default()
                    .join(".agent-hand/profiles/default/agent-runtime/sidecar");
                let _ = std::fs::create_dir_all(&sidecar_dir);
                let sidecar_path = sidecar_dir.join(format!("{}.json", session_id));
                let sidecar_json = serde_json::json!({
                    "goal": context,
                    "now": "Relationship workspace — bridging two session contexts",
                });
                let _ = std::fs::write(&sidecar_path, sidecar_json.to_string());
            }
        }

        // --- Phase 2: Polling fallback for sessions without hook events ---
        // Only probe sessions that weren't updated by hooks this cycle.
        let selected_id = self.selected_session().map(|s| s.id.clone());

        for session in &mut self.sessions {
            match hook_updated.get(&session.id) {
                // Hook reports active state → trust it, skip polling
                Some(Status::Running | Status::Waiting | Status::Starting) => continue,
                // Hook reports Idle → allow polling to verify (may detect activity)
                Some(Status::Idle | Status::Error) | None => {}
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
                .capture_pane(&tmux_session, 35)
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
            && self.auth_token.as_ref().is_some_and(|t| t.is_pro())
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

        // Poll chat responses (non-blocking)
        if let Some(ref mut svc) = self.chat_service {
            let responses = svc.poll_responses();
            if !responses.is_empty() {
                // Responses are already recorded in ChatService conversation history
                // by poll_responses(). Nothing else to do here.
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

    async fn update_preview(&mut self) -> Result<()> {
        if let Some(session) = self.selected_session() {
            #[cfg(feature = "max")]
            let session_id = session.id.clone();
            let tmux_session = session.tmux_name();

            // Build resume hint if a CLI session ID has been captured
            let resume_hint = if let Some(sid) = session.cli_session_id() {
                format!("\nSession ID: {}\n  u = resume previous conversation\n  R = rebuild pane + resume", sid)
            } else if session.tool == crate::tmux::Tool::Shell {
                "\n  (tool not detected yet — interact with the session first)".to_string()
            } else {
                "\n  (no CLI session ID captured yet)".to_string()
            };

            if self.tmux.session_exists(&tmux_session).unwrap_or(false) {
                if let Some(cached) = self.preview_cache.get(&session.id) {
                    self.preview = cached.clone();
                } else {
                    let ptmx_line = if session.ptmx_count > 0 {
                        format!("PTY FDs: {}\n", session.ptmx_count)
                    } else {
                        String::new()
                    };
                    let status_str = format!("{:?}", session.status);
                    self.preview = format!(
                        "{}\n\nStatus: {}\nPath: {}\nLabel: {}\n{}{}",
                        session.title,
                        status_str,
                        session.project_path.to_string_lossy(),
                        session.label,
                        ptmx_line,
                        resume_hint
                    );
                }
            } else {
                let ptmx_line = if session.ptmx_count > 0 {
                    format!("PTY FDs: {}\n", session.ptmx_count)
                } else {
                    String::new()
                };
                let action_hint = if session.cli_session_id().is_some() {
                    "\n  Enter = start NEW conversation\n  s     = start session\n  a     = add to canvas"
                } else {
                    "\n  Enter = start session\n  s     = start session\n  a     = add to canvas"
                };
                self.preview = format!(
                    "{}\n\nStatus: Stopped\nPath: {}\nLabel: {}\n{}{}{}",
                    session.title,
                    session.project_path.to_string_lossy(),
                    session.label,
                    ptmx_line,
                    resume_hint,
                    action_hint,
                );
            }

            // Append relationship context for relationship workspace sessions
            #[cfg(feature = "pro")]
            if let Some(TreeItem::Relationship { rel_id, .. }) = self.selected_tree_item() {
                if let Some(rel) = self.relationships.iter().find(|r| r.id == *rel_id) {
                    let a_title = self.session_by_id(&rel.session_a_id)
                        .map(|s| s.title.as_str())
                        .unwrap_or("<unknown>");
                    let b_title = self.session_by_id(&rel.session_b_id)
                        .map(|s| s.title.as_str())
                        .unwrap_or("<unknown>");
                    let a_status = self.session_by_id(&rel.session_a_id)
                        .map(|s| format!("{:?}", s.status))
                        .unwrap_or_else(|| "?".to_string());
                    let b_status = self.session_by_id(&rel.session_b_id)
                        .map(|s| format!("{:?}", s.status))
                        .unwrap_or_else(|| "?".to_string());
                    self.preview.push_str(&format!(
                        "\n\n─── ⇄ Relationship ───\nSession A: {}  ({})\nSession B: {}  ({})\nType: {}",
                        a_title, a_status, b_title, b_status, rel.relation_type
                    ));
                }
            }

            // Append cached AI summary or in-flight indicator
            #[cfg(feature = "max")]
            {
                if let Some(summary) = self.max.summary_results.get(&session_id) {
                    self.preview.push_str(&format!(
                        "\n\n─── 🤖 AI Summary ───\n{}",
                        summary
                    ));
                } else if self.max.summarizing_session_id.as_deref() == Some(session_id.as_str()) {
                    self.preview.push_str("\n\n⏳ AI summarizing...");
                }
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

    pub fn runtime_dir(&self) -> &std::path::Path {
        &self.runtime_dir
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

    /// Whether this session has a cached AI summary.
    #[cfg(feature = "max")]
    pub fn has_ai_summary(&self, session_id: &str) -> bool {
        self.max.summary_results.contains_key(session_id)
    }

    /// Whether this session is currently being summarized.
    #[cfg(feature = "max")]
    pub fn is_summarizing(&self, session_id: &str) -> bool {
        self.max.summarizing_session_id.as_deref() == Some(session_id)
    }

    /// Whether the AI summary overlay popup is visible.
    #[cfg(feature = "max")]
    pub fn show_ai_summary_overlay(&self) -> bool {
        self.max.show_ai_summary_overlay
    }

    /// Get the AI summary text for the overlay, if any.
    #[cfg(feature = "max")]
    pub fn ai_summary_overlay_text(&self) -> Option<(&str, &str)> {
        let id = self.max.ai_summary_overlay_id.as_deref()?;
        let summary = self.max.summary_results.get(id)?;
        let title = self.session_by_id(id).map(|s| s.title.as_str()).unwrap_or("Unknown");
        Some((title, summary.as_str()))
    }

    /// Scroll offset for the AI summary overlay.
    #[cfg(feature = "max")]
    pub fn summary_overlay_scroll(&self) -> u16 {
        self.max.summary_overlay_scroll
    }

    /// Whether the AI diagram overlay popup is visible.
    #[cfg(feature = "max")]
    pub fn show_ai_diagram_overlay(&self) -> bool {
        self.max.show_ai_diagram_overlay
    }

    /// Get the AI diagram text for the overlay, if any.
    #[cfg(feature = "max")]
    pub fn ai_diagram_overlay_text(&self) -> Option<(&str, &str)> {
        let id = self.max.ai_diagram_overlay_id.as_deref()?;
        let result = self.max.diagram_results.get(id)?;
        let title = self.session_by_id(id).map(|s| s.title.as_str()).unwrap_or("Unknown");
        Some((title, result.diagram.as_str()))
    }

    /// Current scroll offset for the diagram overlay.
    #[cfg(feature = "max")]
    pub fn diagram_overlay_scroll(&self) -> u16 {
        self.max.diagram_overlay_scroll
    }

    /// Whether this session has a cached AI diagram.
    #[cfg(feature = "max")]
    pub fn has_ai_diagram(&self, session_id: &str) -> bool {
        self.max.diagram_results.contains_key(session_id)
    }

    /// Whether this session is currently being diagrammed.
    #[cfg(feature = "max")]
    pub fn is_diagramming(&self, session_id: &str) -> bool {
        self.max.diagramming_session_id.as_deref() == Some(session_id)
    }

    // --- Prompt collection (behavior analysis) ---

    /// Whether prompt collection is enabled in config.
    #[cfg(feature = "pro")]
    fn prompt_collection_enabled(&self) -> bool {
        self.config.claude_user_prompt_logging()
    }

    /// Number of collected prompts for a given session.
    #[cfg(feature = "pro")]
    pub fn collected_prompts_count(&self, session_id: &str) -> usize {
        self.pro.collected_prompts
            .get(session_id)
            .map(|dq| dq.len())
            .unwrap_or(0)
    }

    /// Get collected prompts for a session (timestamp, text).
    #[cfg(feature = "pro")]
    pub fn collected_prompts_for(&self, session_id: &str) -> Vec<(f64, String)> {
        self.pro.collected_prompts
            .get(session_id)
            .map(|dq| dq.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the AI analysis dialog, if open.
    #[cfg(feature = "max")]
    pub fn ai_analysis_dialog(&self) -> Option<&AiAnalysisDialog> {
        match &self.dialog {
            Some(Dialog::AiAnalysis(d)) => Some(d),
            _ => None,
        }
    }

    // --- Behavior analysis overlay ---

    /// Whether the behavior analysis overlay is visible.
    #[cfg(feature = "max")]
    pub fn show_behavior_overlay(&self) -> bool {
        self.max.show_behavior_overlay
    }

    /// Get the behavior analysis overlay text (title, analysis).
    #[cfg(feature = "max")]
    pub fn behavior_overlay_text(&self) -> Option<(&str, &str)> {
        let id = self.max.behavior_overlay_id.as_deref()?;
        let analysis = self.max.behavior_analysis_results.get(id)?;
        let title = self.session_by_id(id).map(|s| s.title.as_str()).unwrap_or("Unknown");
        Some((title, analysis.as_str()))
    }

    /// Scroll offset for the behavior overlay.
    #[cfg(feature = "max")]
    pub fn behavior_overlay_scroll(&self) -> u16 {
        self.max.behavior_overlay_scroll
    }

    /// Get the behavior analysis dialog, if open.
    #[cfg(feature = "max")]
    pub fn behavior_analysis_dialog(&self) -> Option<&BehaviorAnalysisDialog> {
        match &self.dialog {
            Some(Dialog::BehaviorAnalysis(d)) => Some(d),
            _ => None,
        }
    }

    pub fn canvas_state(&self) -> &crate::ui::canvas::CanvasState {
        &self.canvas_state
    }

    pub fn canvas_state_mut(&mut self) -> &mut crate::ui::canvas::CanvasState {
        &mut self.canvas_state
    }

    pub fn canvas_focused(&self) -> bool {
        self.canvas_focused
    }

    pub fn chat_visible(&self) -> bool {
        self.chat_visible
    }

    pub fn chat_input(&self) -> &str {
        &self.chat_input
    }

    pub fn chat_scroll(&self) -> u16 {
        self.chat_scroll
    }

    pub fn chat_messages(&self) -> Vec<&crate::chat::ChatMessage> {
        self.chat_conversation_id.as_ref()
            .and_then(|id| self.chat_service.as_ref()?.get_conversation(id))
            .map(|conv| conv.messages.iter().collect())
            .unwrap_or_default()
    }

    /// Get the current group whose canvas is loaded.
    #[cfg(feature = "pro")]
    pub fn canvas_group(&self) -> &str {
        &self.pro.canvas_group
    }

    /// Switch canvas to a different group. Saves current, loads new.
    /// No-op if the group hasn't changed.
    #[cfg(feature = "pro")]
    fn switch_canvas_to_group(&mut self, new_group: &str) {
        if self.pro.canvas_group == new_group {
            return;
        }
        self.transition_engine.request_transition();
        if let Some(ref dir) = self.pro.canvas_dir {
            if let Err(e) = self.canvas_state.save_for_group(dir, &self.pro.canvas_group) {
                tracing::warn!("Failed to save canvas for group '{}': {}", self.pro.canvas_group, e);
            }
            self.canvas_state = crate::ui::canvas::CanvasState::load_for_group(dir, new_group);
        }
        self.pro.canvas_group = new_group.to_string();
    }

    /// Switch the canvas between User (persistent) and Agent (generated) modes.
    /// Saves user canvas before leaving it, restores when returning.
    #[cfg(feature = "pro")]
    pub(super) fn switch_canvas_view(&mut self, view: crate::ui::canvas::CanvasView) {
        use crate::ui::canvas::CanvasView;
        let prev = self.canvas_state.current_view;
        if prev == view {
            return;
        }

        // Save user canvas before leaving
        if prev == CanvasView::User {
            if let Some(ref dir) = self.pro.canvas_dir {
                if let Err(e) = self.canvas_state.save_for_group(dir, &self.pro.canvas_group) {
                    tracing::warn!("Failed to save canvas for group '{}': {}", self.pro.canvas_group, e);
                }
            }
        }

        self.canvas_state.current_view = view;
        match view {
            CanvasView::User => self.restore_user_canvas(),
            CanvasView::Agent => self.load_agent_canvas_view(),
        }
    }

    /// Load agent-generated canvas ops from runtime snapshot.
    fn load_agent_canvas_view(&mut self) {
        self.canvas_state.clear_projection_nodes();

        let ops_path = self.runtime_dir.join("wasm_canvas_ops.json");

        let content = match std::fs::read_to_string(&ops_path) {
            Ok(c) => c,
            Err(_) => {
                tracing::debug!("No wasm_canvas_ops.json found for Agent view");
                return;
            }
        };

        let ops: Vec<serde_json::Value> = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Failed to parse wasm_canvas_ops.json: {}", e);
                return;
            }
        };

        for op_value in &ops {
            match serde_json::from_value::<crate::ui::canvas::CanvasOp>(op_value.clone()) {
                Ok(op) => {
                    let _ = self.canvas_state.handle_op(op);
                }
                Err(e) => {
                    tracing::debug!("Skipping invalid canvas op: {}", e);
                }
            }
        }

        tracing::debug!("Agent view loaded {} canvas ops", ops.len());
    }

    /// Dispatch a WASM canvas event (e.g. node click) to the ActionExecutor.
    #[cfg(feature = "wasm")]
    pub(super) fn dispatch_wasm_canvas_event(
        &self,
        event_type: &str,
        node_id: Option<String>,
    ) {
        if let Some(ref tx) = self.action_tx {
            let nc = self.canvas_state.node_count();
            let vc = self.canvas_state.panel_cols;
            let vr = self.canvas_state.panel_rows;
            let summary = crate::agent::wasm_canvas::CanvasSummary {
                node_count: nc,
                edge_count: self.canvas_state.edge_count(),
                node_ids: self.canvas_state.all_node_ids(),
                viewport_cols: vc,
                viewport_rows: vr,
                viewport_x: self.canvas_state.viewport.x,
                viewport_y: self.canvas_state.viewport.y,
                suggested_lod: crate::agent::wasm_canvas::compute_lod(nc, vc, vr).to_string(),
            };
            let action = crate::agent::Action::WasmCanvasEvent {
                event_type: event_type.to_string(),
                node_id,
                canvas_summary: Some(summary),
            };
            let _ = tx.send(action);
        }
    }

    /// Reload agent canvas view (re-read wasm_canvas_ops.json).
    /// Used after state changes that should trigger a visual refresh.
    #[cfg(feature = "pro")]
    fn reload_agent_canvas_view(&mut self) {
        if self.canvas_state.current_view == crate::ui::canvas::CanvasView::Agent {
            self.load_agent_canvas_view();
        }
    }

    // Scheduler/Evidence/Workflow view loaders removed in Phase C.2.
    // Agent/WASM plugins now generate canvas ops via canvas-render skill.



    /// Restore user canvas from disk.
    #[cfg(feature = "pro")]
    fn restore_user_canvas(&mut self) {
        self.canvas_state.clear_projection_nodes();
        if let Some(ref dir) = self.pro.canvas_dir {
            self.canvas_state = crate::ui::canvas::CanvasState::load_for_group(dir, &self.pro.canvas_group);
        }
        // Re-sync relationship edges for Max users
        if self.auth_token.as_ref().is_some_and(|t| t.is_max()) {
            let canvas_group = self.pro.canvas_group.clone();
            let group_relationships: Vec<_> = self.relationships.iter()
                .filter(|r| {
                    let src_in = self.sessions.iter().any(|s| s.id == r.session_a_id && s.group_path == canvas_group);
                    let tgt_in = self.sessions.iter().any(|s| s.id == r.session_b_id && s.group_path == canvas_group);
                    src_in && tgt_in
                })
                .cloned()
                .collect();
            self.canvas_state.sync_relationship_edges(&group_relationships);
        }
    }

    /// Approve a human review record: move from review_queue to proposed_followups.
    #[cfg(feature = "pro")]
    pub(super) fn approve_human_review(&mut self, record_id: &str) {
        use crate::agent::scheduler::SchedulerState;
        let path = self.runtime_dir.join("scheduler_state.json");
        let mut state: SchedulerState = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        if let Some(pos) = state.review_queue.iter().position(|r| r.id == record_id) {
            let rec = state.review_queue.remove(pos);
            state.proposed_followups.push(rec);
            if let Ok(json) = serde_json::to_string_pretty(&state) {
                let _ = std::fs::write(&path, json);
            }
        }

        // Refresh the scheduler view
        self.reload_agent_canvas_view();

        #[cfg(feature = "pro")]
        self.pro.toast_notifications.push(ToastNotification {
            message: "Approved review record".to_string(),
            created_at: Instant::now(),
            color: ratatui::style::Color::Green,
        });
    }

    /// Dismiss a human review record: remove from review_queue.
    #[cfg(feature = "pro")]
    pub(super) fn dismiss_human_review(&mut self, record_id: &str) {
        use crate::agent::scheduler::SchedulerState;
        let path = self.runtime_dir.join("scheduler_state.json");
        let mut state: SchedulerState = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        state.review_queue.retain(|r| r.id != record_id);
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = std::fs::write(&path, json);
        }

        // Refresh the scheduler view
        self.reload_agent_canvas_view();

        #[cfg(feature = "pro")]
        self.pro.toast_notifications.push(ToastNotification {
            message: "Dismissed review record".to_string(),
            created_at: Instant::now(),
            color: ratatui::style::Color::Yellow,
        });
    }

    /// Find a SchedulerRecord in the review queue by its short ID prefix.
    #[cfg(feature = "pro")]
    pub(super) fn find_review_record(&self, node_id: &str) -> Option<crate::agent::scheduler::SchedulerRecord> {
        use crate::agent::scheduler::SchedulerState;
        let state: SchedulerState = std::fs::read_to_string(self.runtime_dir.join("scheduler_state.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // node_id format: sched_review_{short_id}
        let short_id = node_id.strip_prefix("sched_review_")?;
        state.review_queue.into_iter().find(|r| r.id.starts_with(short_id))
    }

    /// Find a proposal record and its current status by node ID prefix.
    /// Returns the SchedulerRecord from proposed_followups + a status string.
    #[cfg(feature = "pro")]
    pub(super) fn find_proposal_record(&self, node_id: &str) -> Option<(crate::agent::scheduler::SchedulerRecord, String)> {
        use crate::agent::scheduler::{SchedulerState, load_proposals, ProposalStatus};
        let short_id = node_id.strip_prefix("sched_followup_")?;

        let state: SchedulerState = std::fs::read_to_string(self.runtime_dir.join("scheduler_state.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let rec = state.proposed_followups.into_iter().find(|r| r.id.starts_with(short_id))?;

        // Check persisted proposal status
        let proposals = load_proposals(&self.runtime_dir.join("proposals.json"));
        let status = proposals.iter()
            .find(|p| p.id == rec.id)
            .map(|p| match p.status {
                ProposalStatus::Accepted => "Accepted".to_string(),
                ProposalStatus::Rejected => "Rejected".to_string(),
                ProposalStatus::Pending => "Pending".to_string(),
            })
            .unwrap_or_else(|| "Pending".to_string());

        Some((rec, status))
    }

    /// Accept a followup proposal with urgency-gated execution.
    ///
    /// High/Critical urgency → auto-inject context into all target sessions.
    /// Medium/Low urgency → open ConfirmInjectionDialog for user to pick targets.
    #[cfg(feature = "pro")]
    pub(super) fn accept_followup_proposal(&mut self, proposal_id: &str) {
        use crate::agent::scheduler::{load_proposals, save_proposals, accept_proposal,
            build_followup_proposals, SchedulerState};

        let proposals_path = self.runtime_dir.join("proposals.json");
        let mut proposals = load_proposals(&proposals_path);

        // If proposals.json is empty, generate proposals from current scheduler state
        if proposals.is_empty() {
            let state: SchedulerState = std::fs::read_to_string(self.runtime_dir.join("scheduler_state.json"))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            proposals = build_followup_proposals(&state, 100);
        }

        // Find the proposal to get urgency + targets before accepting
        let proposal_data = proposals.iter()
            .find(|p| p.id == proposal_id)
            .map(|p| (p.urgency_level.clone(), p.target_session_ids.clone(), p.reason.clone()));

        if accept_proposal(&mut proposals, proposal_id) {
            let _ = save_proposals(&proposals_path, &proposals);
            self.reload_agent_canvas_view();

            if let Some((urgency, target_ids, reason)) = proposal_data {
                let targets = self.resolve_injection_targets(&target_ids);

                match urgency {
                    crate::agent::guard::RiskLevel::High | crate::agent::guard::RiskLevel::Critical => {
                        // Auto-inject into all targets
                        let count = self.execute_proposal_injection(&targets);
                        self.pro.toast_notifications.push(ToastNotification {
                            message: format!("Proposal accepted — injected into {} session(s)", count),
                            created_at: Instant::now(),
                            color: ratatui::style::Color::Green,
                        });
                    }
                    _ => {
                        if targets.is_empty() {
                            self.pro.toast_notifications.push(ToastNotification {
                                message: "Proposal accepted (no reachable targets)".to_string(),
                                created_at: Instant::now(),
                                color: ratatui::style::Color::Yellow,
                            });
                        } else {
                            // Open confirmation dialog for user to pick targets
                            let urgency_str = format!("{:?}", urgency);
                            self.open_injection_confirmation(proposal_id, &reason, &urgency_str, targets);
                        }
                    }
                }
            } else {
                self.pro.toast_notifications.push(ToastNotification {
                    message: "Proposal accepted".to_string(),
                    created_at: Instant::now(),
                    color: ratatui::style::Color::Green,
                });
            }
        }
    }

    /// Resolve target session IDs to InjectionTarget structs with project paths.
    #[cfg(feature = "pro")]
    fn resolve_injection_targets(&self, target_ids: &[String]) -> Vec<super::InjectionTarget> {
        target_ids.iter().filter_map(|tid| {
            self.sessions.iter()
                .find(|s| s.id == *tid)
                .map(|s| super::InjectionTarget {
                    session_key: s.tmux_session_name.clone().unwrap_or_else(|| format!("agentdeck_rs_{}", s.id)),
                    project_path: s.project_path.clone(),
                    selected: true,
                })
        }).collect()
    }

    /// Execute context injection for selected targets. Returns count of successful injections.
    #[cfg(feature = "pro")]
    fn execute_proposal_injection(&self, targets: &[super::InjectionTarget]) -> usize {
        let mut count = 0;
        for target in targets.iter().filter(|t| t.selected) {
            if self.inject_context_for_session(&target.session_key, &target.project_path) {
                count += 1;
            }
        }
        count
    }

    /// Inject context into a single session (sync file I/O).
    ///
    /// Reads progress, cold memory, and sidecar hint, then writes .agent-hand-context.md.
    /// Returns true on success.
    #[cfg(feature = "pro")]
    fn inject_context_for_session(&self, session_key: &str, project_path: &std::path::Path) -> bool {
        let progress_dir = self.progress_dir();
        let progress_file = progress_dir.join(format!("{}.md", session_key));
        let progress = match std::fs::read_to_string(&progress_file) {
            Ok(content) if !content.trim().is_empty() => content,
            _ => return false,
        };

        // Take last 50 lines of progress
        let recent: String = progress
            .lines()
            .rev()
            .take(50)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");

        // Cold memory section (sync)
        let memory_section = self.build_memory_section_sync();

        // Sidecar hint
        let sidecar_path = self.runtime_dir.join("sidecar").join(format!("{}.json", session_key));
        let sidecar_hint = format!(
            "\n## Sidecar Feedback\n\n\
             To provide structured feedback, write JSON to:\n\
             `{}`\n\n\
             Schema: `{{\"goal\": \"...\", \"now\": \"...\", \"blockers\": [...], \"decisions\": [...], \
             \"findings\": [...], \"next_steps\": [...], \"affected_targets\": [...], \
             \"urgency\": \"low|medium|high|critical\"}}`\n\
             All fields optional. File is read on each pipeline event.\n",
            sidecar_path.display()
        );

        let context = format!(
            "<!-- Auto-generated by agent-hand. Do not edit manually. -->\n\
             # Agent Progress: {}\n\n\
             ## Recent Activity\n\n\
             {}\n\
             {}{}\n",
            session_key, recent, memory_section, sidecar_hint
        );

        let context_path = project_path.join(".agent-hand-context.md");
        std::fs::write(&context_path, context).is_ok()
    }

    /// Build cold memory section (sync version for App context).
    #[cfg(feature = "pro")]
    fn build_memory_section_sync(&self) -> String {
        let path = self.runtime_dir.join("cold_memory_snapshot.json");
        let records: Vec<crate::agent::memory::ColdMemoryRecord> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if records.is_empty() {
            return String::new();
        }
        let mut section = String::from("\n## Memory\n\n");
        for r in records.iter().rev().take(5) {
            section.push_str(&format!("- **{:?}**: {}\n", r.kind, r.summary));
        }
        section
    }

    /// Get progress directory path.
    #[cfg(feature = "pro")]
    fn progress_dir(&self) -> std::path::PathBuf {
        // Progress dir is a sibling of runtime_dir: same parent, named "progress"
        self.runtime_dir.parent()
            .map(|p| p.join("progress"))
            .unwrap_or_else(|| self.runtime_dir.join("progress"))
    }

    /// Open the injection confirmation dialog for Medium/Low urgency proposals.
    #[cfg(feature = "pro")]
    fn open_injection_confirmation(&mut self, proposal_id: &str, reason: &str, urgency: &str, targets: Vec<super::InjectionTarget>) {
        self.dialog = Some(Dialog::ConfirmInjection(super::ConfirmInjectionDialog {
            proposal_id: proposal_id.to_string(),
            reason: reason.to_string(),
            urgency: urgency.to_string(),
            targets,
            cursor: 0,
        }));
        self.state = AppState::Dialog;
    }

    /// Reject a followup proposal: update status in proposals.json and refresh view.
    #[cfg(feature = "pro")]
    pub(super) fn reject_followup_proposal(&mut self, proposal_id: &str, reason: &str) {
        use crate::agent::scheduler::{load_proposals, save_proposals, reject_proposal,
            SchedulerState};

        let proposals_path = self.runtime_dir.join("proposals.json");
        let mut proposals = load_proposals(&proposals_path);

        // If proposals.json is empty, generate proposals from current scheduler state
        if proposals.is_empty() {
            let state: SchedulerState = std::fs::read_to_string(self.runtime_dir.join("scheduler_state.json"))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            proposals = crate::agent::scheduler::build_followup_proposals(&state, 100);
        }

        if reject_proposal(&mut proposals, proposal_id, reason) {
            let _ = save_proposals(&proposals_path, &proposals);
            self.reload_agent_canvas_view();
            self.pro.toast_notifications.push(ToastNotification {
                message: "Proposal rejected".to_string(),
                created_at: Instant::now(),
                color: ratatui::style::Color::Yellow,
            });
        }
    }

    pub fn set_canvas_focused(&mut self, focused: bool) {
        self.canvas_focused = focused;
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
    pub fn disconnect_viewer_dialog(&self) -> Option<&crate::ui::dialogs::DisconnectViewerDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::DisconnectViewer(d)) => Some(d),
            _ => None,
        }
    }

    pub fn pack_browser_dialog(&self) -> Option<&crate::ui::dialogs::PackBrowserDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::PackBrowser(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn control_request_dialog(&self) -> Option<&crate::ui::ControlRequestDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::ControlRequest(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn orphaned_rooms_dialog(&self) -> Option<&crate::ui::dialogs::OrphanedRoomsDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::OrphanedRooms(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn skills_manager_dialog(&self) -> Option<&crate::ui::dialogs::SkillsManagerDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::SkillsManager(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn human_review_dialog(&self) -> Option<&crate::ui::HumanReviewDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::HumanReview(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn proposal_action_dialog(&self) -> Option<&crate::ui::ProposalActionDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::ProposalAction(d)) => Some(d),
            _ => None,
        }
    }

    #[cfg(feature = "pro")]
    pub fn confirm_injection_dialog(&self) -> Option<&crate::ui::ConfirmInjectionDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::ConfirmInjection(d)) => Some(d),
            _ => None,
        }
    }

    pub fn language(&self) -> crate::i18n::Language {
        self.language
    }

    pub fn set_language(&mut self, lang: crate::i18n::Language) {
        self.language = lang;
    }

    pub fn show_onboarding(&self) -> bool {
        self.show_onboarding
    }

    pub fn info_bar_message(&self) -> Option<(&String, &ratatui::style::Color)> {
        self.info_bar_message.as_ref().map(|(msg, color, _)| (msg, color))
    }

    fn set_info_bar(&mut self, message: String, color: ratatui::style::Color) {
        self.info_bar_message = Some((message, color, Instant::now()));
    }

    pub fn dismiss_onboarding(&mut self) {
        self.show_onboarding = false;
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

    pub fn startup_phase(&self) -> crate::ui::transition::StartupPhase {
        self.startup_phase
    }

    pub fn startup_elapsed_ms(&self) -> u64 {
        self.startup_started_at
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn activity(&self) -> &activity::ActivityTracker {
        &self.activity
    }

    pub fn scroll_padding(&self) -> usize {
        self.scroll_padding
    }

    pub fn mouse_captured(&self) -> bool {
        self.mouse_captured
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

    #[cfg(feature = "pro")]
    pub fn viewer_panel_focused(&self) -> bool {
        self.pro.viewer_panel_focused
    }

    #[cfg(feature = "pro")]
    pub fn viewer_panel_selected(&self) -> usize {
        self.pro.viewer_panel_selected
    }

    /// Sessions that deserve attention: actively working OR recently finished (✓ ready).
    pub fn active_sessions(&self) -> Vec<&Instance> {
        self.ordered_session_indices_by_group_baseline()
            .into_iter()
            .filter_map(|idx| {
                let session = &self.sessions[idx];
                (!matches!(session.status, Status::Idle) || self.is_attention_active(&session.id))
                    .then_some(session)
            })
            .collect()
    }

    #[cfg(feature = "pro")]
    pub fn viewer_sessions(&self) -> &HashMap<String, ViewerSessionInfo> {
        &self.pro.viewer_sessions
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

    /// Get active toast notifications (for rendering).
    #[cfg(feature = "pro")]
    pub fn toast_notifications(&self) -> &[ToastNotification] {
        &self.pro.toast_notifications
    }

    /// Load persisted AI analysis results (summaries + diagrams) from disk.
    #[cfg(feature = "max")]
    fn load_ai_results(
        dir: &std::path::Path,
    ) -> (
        HashMap<String, String>,
        HashMap<String, crate::ai::DiagramResult>,
    ) {
        let summaries: HashMap<String, String> = std::fs::read_to_string(dir.join("ai_summaries.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let diagrams: HashMap<String, crate::ai::DiagramResult> =
            std::fs::read_to_string(dir.join("ai_diagrams.json"))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

        (summaries, diagrams)
    }

    /// Persist AI analysis results (summaries + diagrams) to disk.
    #[cfg(feature = "max")]
    fn save_ai_results(
        dir: &std::path::Path,
        summaries: &HashMap<String, String>,
        diagrams: &HashMap<String, crate::ai::DiagramResult>,
    ) {
        if let Ok(json) = serde_json::to_string_pretty(summaries) {
            if let Err(e) = std::fs::write(dir.join("ai_summaries.json"), json) {
                tracing::warn!("Failed to save AI summaries: {}", e);
            }
        }
        if let Ok(json) = serde_json::to_string_pretty(diagrams) {
            if let Err(e) = std::fs::write(dir.join("ai_diagrams.json"), json) {
                tracing::warn!("Failed to save AI diagrams: {}", e);
            }
        }
    }
}
