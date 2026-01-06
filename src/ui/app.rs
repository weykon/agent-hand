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
use tokio::sync::Mutex;

use crate::error::Result;
use crate::mcp::{pooled_mcp_config, MCPManager, MCPPool};
use crate::session::{GroupTree, Instance, Status, Storage};
use crate::tmux::{TmuxManager, SESSION_PREFIX};

use super::{
    AppState, CreateGroupDialog, DeleteConfirmDialog, DeleteGroupChoice, DeleteGroupDialog, Dialog,
    ForkDialog, ForkField, MCPColumn, MCPDialog, MoveGroupDialog, NewSessionDialog,
    NewSessionField, RenameGroupDialog, RenameSessionDialog, TreeItem,
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

    // Navigation/perf
    last_navigation_time: Instant,
    is_navigating: bool,
    pending_preview_id: Option<String>,
    last_status_refresh: Instant,
    last_cache_refresh: Instant,

    // Status/probing
    last_tmux_activity: HashMap<String, i64>,
    last_tmux_activity_change: HashMap<String, Instant>,
    last_status_probe: HashMap<String, Instant>,

    // Backend
    storage: Arc<Mutex<Storage>>,
    tmux: Arc<TmuxManager>,
}

impl App {
    const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(150);
    const NAVIGATION_SETTLE: Duration = Duration::from_millis(300);
    const STATUS_REFRESH: Duration = Duration::from_secs(1);
    const CACHE_REFRESH: Duration = Duration::from_secs(2);

    const STATUS_COOLDOWN: Duration = Duration::from_secs(2);
    const STATUS_FALLBACK: Duration = Duration::from_secs(60);

    /// Create new application
    pub async fn new(profile: &str) -> Result<Self> {
        let storage = Storage::new(profile).await?;
        let (sessions, groups) = storage.load().await?;

        let tmux = TmuxManager::new();

        let mut app = Self {
            width: 0,
            height: 0,
            state: AppState::Normal,
            should_quit: false,
            sessions,
            sessions_by_id: HashMap::new(),
            groups,
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
            last_navigation_time: Instant::now(),
            is_navigating: false,
            pending_preview_id: None,
            last_status_refresh: Instant::now(),
            last_cache_refresh: Instant::now(),
            last_tmux_activity: HashMap::new(),
            last_tmux_activity_change: HashMap::new(),
            last_status_probe: HashMap::new(),
            storage: Arc::new(Mutex::new(storage)),
            tmux: Arc::new(tmux),
        };

        app.ensure_groups_exist();
        app.rebuild_tree();
        app.rebuild_sessions_index();

        // Prime tmux cache/status so initial render isn't stale
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
                self.refresh_statuses().await?;
                self.last_status_refresh = Instant::now();
            }
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

            // Cheap gating: if activity moved forward, assume running and skip capture-pane.
            if prev_activity.is_none() || prev_activity.is_some_and(|a| activity > a) {
                self.last_tmux_activity.insert(session.id.clone(), activity);
                self.last_tmux_activity_change
                    .insert(session.id.clone(), now);
                session.status = Status::Running;
                continue;
            }

            let need_fallback_probe = self
                .last_status_probe
                .get(&session.id)
                .is_none_or(|t| now.duration_since(*t) >= Self::STATUS_FALLBACK);

            let activity_settled = self
                .last_tmux_activity_change
                .get(&session.id)
                .is_some_and(|t| now.duration_since(*t) >= Self::STATUS_COOLDOWN);

            // Only probe when running has settled (to detect prompt/idle), or on infrequent fallback.
            if !(need_fallback_probe || (activity_settled && session.status == Status::Running)) {
                continue;
            }

            let content = self
                .tmux
                .capture_pane(&tmux_session, 30)
                .await
                .unwrap_or_default();
            let detector = crate::tmux::PromptDetector::new(session.tool);
            let has_prompt = detector.has_prompt(&content);
            session.status = if has_prompt {
                Status::Waiting
            } else {
                Status::Idle
            };
            self.last_status_probe.insert(session.id.clone(), now);
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
        }
    }

    /// Handle keys in normal mode
    async fn handle_normal_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<()> {
        match key {
            // Quit
            KeyCode::Char('q') | KeyCode::Char('Q')
                if !modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.should_quit = true;
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }

            // Navigation
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection_up();
                self.on_navigation();
                self.preview.clear();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection_down();
                self.on_navigation();
                self.preview.clear();
            }

            // Actions
            KeyCode::Enter => {
                if self.toggle_selected_group(None).await? {
                    self.preview.clear();
                } else {
                    self.queue_attach_selected().await?;
                }
            }
            KeyCode::Left => {
                let _ = self.toggle_selected_group(Some(false)).await?;
            }
            KeyCode::Right => {
                let _ = self.toggle_selected_group(Some(true)).await?;
            }
            KeyCode::Char(' ') => {
                let _ = self.toggle_selected_group(None).await?;
            }
            KeyCode::Char('s') => {
                self.start_selected().await?;
            }
            KeyCode::Char('x') => {
                self.stop_selected().await?;
            }
            KeyCode::Char('r') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.refresh_sessions().await?;
            }
            KeyCode::Char('r') => {
                if matches!(self.selected_tree_item(), Some(TreeItem::Group { .. })) {
                    self.open_rename_group_dialog();
                } else if self.selected_session().is_some() {
                    self.open_rename_session_dialog();
                }
            }

            // New session
            KeyCode::Char('n') => {
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
            }

            // Delete session / group
            KeyCode::Char('d') => {
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
            }

            // Fork
            KeyCode::Char('f') => {
                if self.selected_session().is_some() {
                    self.open_fork_dialog();
                }
            }

            // Create group
            KeyCode::Char('g') => {
                self.open_create_group_dialog();
            }

            // Move session to group
            KeyCode::Char('m') => {
                if self.selected_session().is_some() {
                    self.open_move_group_dialog();
                }
            }

            // Refresh preview (cached snapshot)
            KeyCode::Char('p') => {
                self.refresh_preview_cache_selected().await?;
            }

            // Search
            KeyCode::Char('/') => {
                self.state = AppState::Search;
                self.search_query.clear();
                self.search_results.clear();
                self.search_selected = 0;
                self.update_search_results();
            }

            // Help
            KeyCode::Char('?') => {
                self.help_visible = !self.help_visible;
                self.state = if self.help_visible {
                    AppState::Help
                } else {
                    AppState::Normal
                };
            }

            // Restart selected session
            KeyCode::Char('R') => {
                if self.selected_session().is_some() {
                    self.restart_selected().await?;
                }
            }

            _ => {}
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
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                    if d.field == NewSessionField::Group {
                        if d.group_matches.is_empty() {
                            return Ok(());
                        }
                        if matches!(key, KeyCode::Up | KeyCode::Left) {
                            if d.group_selected == 0 {
                                d.group_selected = d.group_matches.len() - 1;
                            } else {
                                d.group_selected -= 1;
                            }
                        } else {
                            d.group_selected = (d.group_selected + 1) % d.group_matches.len();
                        }
                    } else if d.field == NewSessionField::Path && d.path_suggestions_visible {
                        d.complete_path_or_cycle(matches!(key, KeyCode::Up | KeyCode::Left));
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
                            d.group_path = sel.to_string();
                            d.update_group_matches();
                        } else {
                            d.group_path = d.group_path.trim().to_string();
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
                            d.path.pop();
                            d.clear_path_suggestions();
                            d.path_dirty = true;
                            d.path_last_edit = Instant::now();
                        }
                        NewSessionField::Title => {
                            d.title.pop();
                        }
                        NewSessionField::Group => {
                            d.group_path.pop();
                            d.update_group_matches();
                        }
                    };
                }
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
                            d.path.push(ch);
                            d.clear_path_suggestions();
                            d.path_dirty = true;
                            d.path_last_edit = Instant::now();
                        }
                        NewSessionField::Title => d.title.push(ch),
                        NewSessionField::Group => {
                            d.group_path.push(ch);
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
            Dialog::MCP(d) => match key {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Tab => {
                    d.column = match d.column {
                        MCPColumn::Attached => MCPColumn::Available,
                        MCPColumn::Available => MCPColumn::Attached,
                    };
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    match d.column {
                        MCPColumn::Attached => {
                            if !d.attached.is_empty() {
                                d.attached_idx = d.attached_idx.saturating_sub(1);
                            }
                        }
                        MCPColumn::Available => {
                            if !d.available.is_empty() {
                                d.available_idx = d.available_idx.saturating_sub(1);
                            }
                        }
                    };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    match d.column {
                        MCPColumn::Attached => {
                            if !d.attached.is_empty() && d.attached_idx + 1 < d.attached.len() {
                                d.attached_idx += 1;
                            }
                        }
                        MCPColumn::Available => {
                            if !d.available.is_empty() && d.available_idx + 1 < d.available.len() {
                                d.available_idx += 1;
                            }
                        }
                    };
                }
                KeyCode::Enter => {
                    d.dirty = true;
                    match d.column {
                        MCPColumn::Attached => {
                            if d.attached.is_empty() {
                                return Ok(());
                            }
                            let name = d.attached.remove(d.attached_idx);
                            d.available.push(name);
                            d.available.sort();
                            if d.attached_idx >= d.attached.len() && !d.attached.is_empty() {
                                d.attached_idx = d.attached.len() - 1;
                            }
                        }
                        MCPColumn::Available => {
                            if d.available.is_empty() {
                                return Ok(());
                            }
                            let name = d.available.remove(d.available_idx);
                            d.attached.push(name);
                            d.attached.sort();
                            if d.available_idx >= d.available.len() && !d.available.is_empty() {
                                d.available_idx = d.available.len() - 1;
                            }
                        }
                    }
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    let session_id = d.session_id.clone();
                    let project_path = d.project_path.clone();
                    let attached = d.attached.clone();
                    self.apply_mcp_changes(&session_id, &project_path, &attached)
                        .await?;
                    self.dialog = None;
                    self.state = AppState::Normal;
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
                        let title = d.title.clone();
                        let group_path = d.group_path.clone();
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
                        d.title.pop();
                    }
                    ForkField::Group => {
                        d.group_path.pop();
                    }
                },
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        match d.field {
                            ForkField::Title => d.title.push(ch),
                            ForkField::Group => d.group_path.push(ch),
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
                    let new_path = d.new_path.clone();
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
                    d.new_path.pop();
                }
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        d.new_path.push(ch);
                    }
                }
                _ => {}
            },
            Dialog::RenameSession(d) => match key {
                KeyCode::Esc => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Enter => {
                    let session_id = d.session_id.clone();
                    let new_title = d.new_title.clone();
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.apply_rename_session(&session_id, &new_title).await?;
                    self.refresh_sessions().await?;
                    self.focus_session(&session_id).await?;
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.dialog = None;
                    self.state = AppState::Normal;
                }
                KeyCode::Backspace => {
                    d.new_title.pop();
                }
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        d.new_title.push(ch);
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
                        .unwrap_or_else(|| d.input.trim().to_string());
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
                    d.input.pop();
                    d.update_matches();
                }
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        d.input.push(ch);
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
                        .unwrap_or_else(|| d.input.trim().to_string());
                    self.dialog = None;
                    self.state = AppState::Normal;
                    self.apply_move_group(&session_id, &group_path).await?;
                    self.refresh_sessions().await?;
                    self.focus_session(&session_id).await?;
                }
                KeyCode::Backspace => {
                    d.input.pop();
                    d.update_matches();
                }
                KeyCode::Char(ch) => {
                    if !modifiers.contains(KeyModifiers::CONTROL) {
                        d.input.push(ch);
                        d.update_matches();
                    }
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
            title,
            group_path: parent.group_path.clone(),
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
            input: String::new(),
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
            input: s.group_path.clone(),
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
            new_title: s.title.clone(),
        }));
        self.state = AppState::Dialog;
    }

    fn open_rename_group_dialog(&mut self) {
        let Some(TreeItem::Group { path, .. }) = self.selected_tree_item() else {
            return;
        };

        self.dialog = Some(Dialog::RenameGroup(RenameGroupDialog {
            old_path: path.clone(),
            new_path: path.clone(),
        }));
        self.state = AppState::Dialog;
    }

    #[allow(dead_code)]
    async fn open_mcp_dialog(&mut self) -> Result<()> {
        let Some(session) = self.selected_session() else {
            return Ok(());
        };

        let pool = MCPManager::load_global_pool().await.unwrap_or_default();
        let mut available: Vec<String> = pool.keys().cloned().collect();
        available.sort();

        let project_mcp = MCPManager::load_project_mcp(&session.project_path)
            .await
            .unwrap_or_default();
        let mut attached: Vec<String> = project_mcp.keys().cloned().collect();
        attached.sort();

        // Remove attached from available
        available.retain(|n| !attached.contains(n));

        self.dialog = Some(Dialog::MCP(MCPDialog {
            session_id: session.id.clone(),
            project_path: session.project_path.clone(),
            attached,
            available,
            column: MCPColumn::Attached,
            attached_idx: 0,
            available_idx: 0,
            dirty: false,
        }));
        self.state = AppState::Dialog;

        Ok(())
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
        inst.loaded_mcp_names = parent.loaded_mcp_names.clone();
        inst.parent_session_id = Some(parent_session_id.to_string());

        let storage = self.storage.lock().await;
        let (mut instances, tree) = storage.load().await?;
        instances.push(inst.clone());
        storage.save(&instances, &tree).await?;

        Ok(inst.id)
    }

    async fn apply_create_group(&mut self, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();
        if group_path.is_empty() {
            return Ok(());
        }

        let storage = self.storage.lock().await;
        let (instances, mut tree) = storage.load().await?;

        tree.create_group(group_path.to_string());

        let parts: Vec<&str> = group_path.split('/').collect();
        for i in 1..=parts.len() {
            let p = parts[..i].join("/");
            tree.set_expanded(&p, true);
        }

        storage.save(&instances, &tree).await?;
        Ok(())
    }

    async fn apply_delete_group_prefix(&mut self, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();
        if group_path.is_empty() {
            return Ok(());
        }

        let storage = self.storage.lock().await;
        let (instances, mut tree) = storage.load().await?;

        tree.delete_group_prefix(group_path);

        storage.save(&instances, &tree).await?;
        Ok(())
    }

    async fn apply_delete_group_keep_sessions(&mut self, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();
        if group_path.is_empty() {
            return Ok(());
        }

        let prefix = format!("{}/", group_path);

        let storage = self.storage.lock().await;
        let (mut instances, mut tree) = storage.load().await?;

        for inst in instances.iter_mut() {
            if inst.group_path == group_path || inst.group_path.starts_with(&prefix) {
                inst.group_path.clear();
            }
        }

        tree.delete_group_prefix(group_path);
        storage.save(&instances, &tree).await?;
        Ok(())
    }

    async fn apply_delete_group_and_sessions(&mut self, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();
        if group_path.is_empty() {
            return Ok(());
        }

        let prefix = format!("{}/", group_path);

        let storage = self.storage.lock().await;
        let (mut instances, mut tree) = storage.load().await?;

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
        storage.save(&instances, &tree).await?;
        Ok(())
    }

    async fn apply_move_group(&mut self, session_id: &str, group_path: &str) -> Result<()> {
        let group_path = group_path.trim();

        let storage = self.storage.lock().await;
        let (mut instances, mut tree) = storage.load().await?;

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

        storage.save(&instances, &tree).await?;
        Ok(())
    }

    async fn apply_rename_session(&mut self, session_id: &str, new_title: &str) -> Result<()> {
        let new_title = new_title.trim();
        if new_title.is_empty() {
            return Ok(());
        }

        let storage = self.storage.lock().await;
        let (mut instances, tree) = storage.load().await?;

        if let Some(inst) = instances.iter_mut().find(|s| s.id == session_id) {
            inst.title = new_title.to_string();
        }

        storage.save(&instances, &tree).await?;
        Ok(())
    }

    async fn apply_rename_group(&mut self, old_path: &str, new_path: &str) -> Result<()> {
        let old_path = old_path.trim();
        let new_path = new_path.trim();
        if old_path.is_empty() || new_path.is_empty() || old_path == new_path {
            return Ok(());
        }

        let storage = self.storage.lock().await;
        let (mut instances, mut tree) = storage.load().await?;

        let old_slash = format!("{}/", old_path);
        for inst in instances.iter_mut() {
            if inst.group_path == old_path || inst.group_path.starts_with(&old_slash) {
                let suffix = &inst.group_path[old_path.len()..];
                inst.group_path = format!("{new_path}{suffix}");
            }
        }

        tree.rename_prefix(old_path, new_path);
        storage.save(&instances, &tree).await?;
        Ok(())
    }

    async fn apply_mcp_changes(
        &mut self,
        session_id: &str,
        project_path: &std::path::Path,
        attached: &[String],
    ) -> Result<()> {
        let pool = MCPManager::load_global_pool().await.unwrap_or_default();
        let existing = MCPManager::load_project_mcp(project_path)
            .await
            .unwrap_or_default();

        let mut next = std::collections::HashMap::new();
        for name in attached {
            if let Some(cfg) = pool.get(name) {
                if MCPPool::is_running(name).await {
                    if let Ok(sock) = MCPPool::socket_path(name) {
                        next.insert(name.clone(), pooled_mcp_config(name, &sock, cfg));
                        continue;
                    }
                }
                next.insert(name.clone(), cfg.clone());
            } else if let Some(cfg) = existing.get(name) {
                next.insert(name.clone(), cfg.clone());
            }
        }

        MCPManager::write_project_mcp(project_path, &next).await?;

        // Persist to sessions.json
        {
            let storage = self.storage.lock().await;
            let (mut instances, tree) = storage.load().await?;
            if let Some(inst) = instances.iter_mut().find(|s| s.id == session_id) {
                inst.loaded_mcp_names = attached.to_vec();
            }
            storage.save(&instances, &tree).await?;
        }

        // Restart if running
        let tmux_session = TmuxManager::session_name(session_id);
        if self.tmux.session_exists(&tmux_session).unwrap_or(false) {
            let _ = self.tmux.kill_session(&tmux_session).await;
            if let Some(inst) = self.session_by_id(session_id) {
                let _ = self
                    .tmux
                    .create_session(
                        &tmux_session,
                        &inst.project_path.to_string_lossy(),
                        if inst.command.trim().is_empty() {
                            None
                        } else {
                            Some(inst.command.as_str())
                        },
                    )
                    .await;
            }
        }

        Ok(())
    }

    async fn create_session_from_dialog(&mut self) -> Result<()> {
        let Some(Dialog::NewSession(d)) = self.dialog.as_ref() else {
            return Ok(());
        };

        let project_path = d.validate()?;
        let title = if d.title.trim().is_empty() {
            project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Untitled")
                .to_string()
        } else {
            d.title.trim().to_string()
        };

        let storage = self.storage.lock().await;
        let (mut instances, mut tree) = storage.load().await?;

        let mut instance = Instance::new(title.clone(), project_path.clone());
        let group_path = d.group_path.trim();
        if !group_path.is_empty() {
            instance.group_path = group_path.to_string();
            tree.create_group(instance.group_path.clone());
        }

        instance.command.clear();
        instance.tool = crate::tmux::Tool::Shell;

        instances.push(instance);
        storage.save(&instances, &tree).await?;

        Ok(())
    }

    async fn delete_session(&mut self, session_id: &str, kill_tmux: bool) -> Result<()> {
        let tmux_name = TmuxManager::session_name(session_id);

        if kill_tmux && self.tmux.session_exists(&tmux_name).unwrap_or(false) {
            let _ = self.tmux.kill_session(&tmux_name).await;
        }

        let storage = self.storage.lock().await;
        let (mut instances, tree) = storage.load().await?;
        let before = instances.len();
        instances.retain(|s| s.id != session_id);
        if instances.len() != before {
            storage.save(&instances, &tree).await?;
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
        storage.save(&self.sessions, &self.groups).await?;
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
            storage.save(&self.sessions, &self.groups).await?;
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
                self.tmux
                    .create_session(
                        &tmux_session,
                        &session.project_path.to_string_lossy(),
                        if session.command.trim().is_empty() {
                            None
                        } else {
                            Some(session.command.as_str())
                        },
                    )
                    .await?;

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
        let (sessions, groups) = storage.load().await?;
        drop(storage);

        self.sessions = sessions;
        self.groups = groups;

        self.ensure_groups_exist();
        self.rebuild_sessions_index();
        self.rebuild_tree();

        // Refresh tmux cache (rate-limited)
        if self.last_cache_refresh.elapsed() >= Self::CACHE_REFRESH {
            self.tmux.refresh_cache().await?;
            self.last_cache_refresh = Instant::now();
        }

        // Drop stale activity entries after reload
        self.last_tmux_activity
            .retain(|id, _| self.sessions_by_id.contains_key(id));

        // Update session statuses (rate-limited in refresh_statuses)
        self.refresh_statuses().await?;
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
                    self.preview = format!(
                        "{}\n\nPath: {}\nTool: {}\n\nPreview not cached. Press 'p' to capture a snapshot.",
                        session.title,
                        session.project_path.to_string_lossy(),
                        session.tool
                    );
                }
            } else {
                self.preview = format!(
                    "{}\n\nPath: {}\nTool: {}\n\nNot running. Press 's' to start, Enter to start+attach.",
                    session.title,
                    session.project_path.to_string_lossy(),
                    session.tool
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

    pub fn mcp_dialog(&self) -> Option<&MCPDialog> {
        match self.dialog.as_ref() {
            Some(Dialog::MCP(d)) => Some(d),
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

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }
}
