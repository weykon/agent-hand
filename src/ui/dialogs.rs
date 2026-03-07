use std::path::PathBuf;

use crate::error::Result;

#[cfg(feature = "pro")]
use crate::session::RelationType;

use super::input::TextInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewSessionField {
    Path,
    Title,
    Group,
}

#[derive(Debug, Clone)]
pub struct NewSessionDialog {
    pub path: TextInput,
    pub title: TextInput,
    pub group_path: TextInput,
    pub field: NewSessionField,

    pub group_all_groups: Vec<String>,
    pub group_matches: Vec<String>,
    pub group_selected: usize,

    pub path_suggestions: Vec<String>,
    pub path_suggestions_idx: usize,
    pub path_suggestions_visible: bool,

    // Debounced auto-suggest for the Path field.
    pub path_dirty: bool,
    pub path_last_edit: std::time::Instant,
}

impl NewSessionDialog {
    fn fuzzy_match(query: &str, text: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        let t = text.to_lowercase();
        let mut pos = 0usize;
        for ch in q.chars() {
            if let Some(found) = t[pos..].find(ch) {
                pos += found + ch.len_utf8();
            } else {
                return false;
            }
        }
        true
    }

    pub fn update_group_matches(&mut self) {
        let q = self.group_path.text().trim();
        let mut out: Vec<String> = self
            .group_all_groups
            .iter()
            .filter(|g| Self::fuzzy_match(q, g))
            .cloned()
            .collect();
        out.sort();
        self.group_matches = out;
        if self.group_selected >= self.group_matches.len() {
            self.group_selected = 0;
        }
    }

    pub fn selected_group_value(&self) -> Option<&str> {
        self.group_matches
            .get(self.group_selected)
            .map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct DeleteConfirmDialog {
    pub session_id: String,
    pub title: String,
    pub kill_tmux: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteGroupChoice {
    DeleteGroupKeepSessions,
    Cancel,
    DeleteGroupAndSessions,
}

#[derive(Debug, Clone)]
pub struct DeleteGroupDialog {
    pub group_path: String,
    pub session_count: usize,
    pub choice: DeleteGroupChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkField {
    Title,
    Group,
}

#[derive(Debug, Clone)]
pub struct ForkDialog {
    pub parent_session_id: String,
    pub project_path: PathBuf,
    pub title: TextInput,
    pub group_path: TextInput,
    pub field: ForkField,
}

#[derive(Debug, Clone)]
pub struct CreateGroupDialog {
    pub input: TextInput,
    pub all_groups: Vec<String>,
    pub matches: Vec<String>,
    pub selected: usize,
}

impl CreateGroupDialog {
    fn fuzzy_match(query: &str, text: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        let t = text.to_lowercase();
        let mut pos = 0usize;
        for ch in q.chars() {
            if let Some(found) = t[pos..].find(ch) {
                pos += found + ch.len_utf8();
            } else {
                return false;
            }
        }
        true
    }

    pub fn update_matches(&mut self) {
        let q = self.input.text().trim();
        let mut out: Vec<String> = self
            .all_groups
            .iter()
            .filter(|g| Self::fuzzy_match(q, g))
            .cloned()
            .collect();
        out.sort();
        self.matches = out;
        if self.selected >= self.matches.len() {
            self.selected = 0;
        }
    }

    pub fn selected_value(&self) -> Option<&str> {
        self.matches.get(self.selected).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct MoveGroupDialog {
    pub session_id: String,
    pub title: String,
    pub input: TextInput,
    pub all_groups: Vec<String>,
    pub matches: Vec<String>,
    pub selected: usize,
}

impl MoveGroupDialog {
    fn fuzzy_match(query: &str, text: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        let t = text.to_lowercase();
        let mut pos = 0usize;
        for ch in q.chars() {
            if let Some(found) = t[pos..].find(ch) {
                pos += found + ch.len_utf8();
            } else {
                return false;
            }
        }
        true
    }

    pub fn update_matches(&mut self) {
        let q = self.input.text().trim();
        let mut out: Vec<String> = self
            .all_groups
            .iter()
            .filter(|g| Self::fuzzy_match(q, g))
            .cloned()
            .collect();
        out.sort();
        self.matches = out;
        if self.selected >= self.matches.len() {
            self.selected = 0;
        }
    }

    pub fn selected_value(&self) -> Option<&str> {
        self.matches.get(self.selected).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct RenameGroupDialog {
    pub old_path: String,
    pub new_path: TextInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEditField {
    Title,
    Label,
    Color,
}

#[derive(Debug, Clone)]
pub struct RenameSessionDialog {
    pub session_id: String,
    pub old_title: String,
    pub new_title: TextInput,
    pub label: TextInput,
    pub label_color: crate::session::LabelColor,
    pub field: SessionEditField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSpec {
    pub name: String,
    pub color: crate::session::LabelColor,
}

#[derive(Debug, Clone)]
pub struct TagPickerDialog {
    pub session_id: String,
    pub tags: Vec<TagSpec>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    NewSession(NewSessionDialog),
    DeleteConfirm(DeleteConfirmDialog),
    DeleteGroup(DeleteGroupDialog),
    Fork(ForkDialog),
    CreateGroup(CreateGroupDialog),
    MoveGroup(MoveGroupDialog),
    RenameGroup(RenameGroupDialog),
    RenameSession(RenameSessionDialog),
    TagPicker(TagPickerDialog),
    QuitConfirm,
    Settings(SettingsDialog),
    #[cfg(feature = "pro")]
    Share(ShareDialog),
    #[cfg(feature = "pro")]
    CreateRelationship(CreateRelationshipDialog),
    #[cfg(feature = "pro")]
    Annotate(AnnotateDialog),
    #[cfg(feature = "pro")]
    NewFromContext(NewFromContextDialog),
    #[cfg(feature = "pro")]
    JoinSession(JoinSessionDialog),
    #[cfg(feature = "pro")]
    DisconnectViewer(DisconnectViewerDialog),
    #[cfg(feature = "pro")]
    ControlRequest(ControlRequestDialog),
    #[cfg(feature = "pro")]
    PackBrowser(PackBrowserDialog),
}

/// Dialog for sharing a session remotely (Premium)
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct ShareDialog {
    pub session_id: String,
    pub session_title: String,
    pub permission: crate::sharing::SharePermission,
    pub expire_minutes: TextInput,
    pub ssh_url: Option<String>,
    pub web_url: Option<String>,
    pub already_sharing: bool,
    /// Relay share URL (replaces tmate SSH/web URLs when using relay).
    pub relay_share_url: Option<String>,
    /// Room ID on the relay server.
    pub relay_room_id: Option<String>,
    /// Inline "Copied!" feedback timestamp (shown for ~2s after pressing 'c').
    pub copy_feedback_at: Option<std::time::Instant>,
    /// Selected viewer index in the viewer list (for revoke/management actions).
    pub selected_viewer: Option<usize>,
    /// Connection status message (shown during connection process).
    pub status_message: Option<String>,
}

/// Dialog for joining a shared session via relay URL (Premium)
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct JoinSessionDialog {
    /// Share URL input (e.g. https://relay.asymptai.com/share/ROOM_ID?token=TOKEN)
    pub url_input: TextInput,
    /// Status message shown to user (connection errors, success, etc.)
    pub status: Option<String>,
    /// Live URL validation hint (shown while typing, separate from connection status)
    pub validation_hint: Option<String>,
    /// Whether currently connecting
    pub connecting: bool,
    /// Identity shown to the host (from auth token email, or None if anonymous)
    pub viewer_identity: Option<String>,
}

#[cfg(feature = "pro")]
impl JoinSessionDialog {
    pub fn new() -> Self {
        Self {
            url_input: TextInput::new(),
            status: None,
            validation_hint: None,
            connecting: false,
            viewer_identity: None,
        }
    }

    /// Parse a share URL into (relay_base_url, room_id, viewer_token).
    /// Accepts: https://relay.asymptai.com/share/ROOM_ID?token=TOKEN
    pub fn parse_share_url(url: &str) -> Option<(String, String, String)> {
        let url = url.trim();
        // Find /share/ in the URL
        let share_idx = url.find("/share/")?;
        let base_url = &url[..share_idx];
        let after_share = &url[share_idx + 7..]; // skip "/share/"

        // Split room_id from query string
        let (room_id, query) = if let Some(q_idx) = after_share.find('?') {
            (&after_share[..q_idx], &after_share[q_idx + 1..])
        } else {
            (after_share, "")
        };

        if room_id.is_empty() {
            return None;
        }

        // Extract token from query params — require non-empty token
        let token = query
            .split('&')
            .find_map(|pair| {
                let (k, v) = pair.split_once('=')?;
                if k == "token" && !v.is_empty() { Some(v.to_string()) } else { None }
            })?;

        Some((base_url.to_string(), room_id.to_string(), token))
    }
}

/// Dialog for disconnecting from a viewer session (Premium)
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct DisconnectViewerDialog {
    pub room_id: String,
    pub relay_url: String,
    pub selected_option: usize, // 0=disconnect only, 1=disconnect+delete, 2=cancel
}

#[cfg(feature = "pro")]
impl DisconnectViewerDialog {
    pub fn new(room_id: String, relay_url: String) -> Self {
        Self {
            room_id,
            relay_url,
            selected_option: 0,
        }
    }
}

/// Dialog for a viewer requesting read-write control of a session (Premium)
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct ControlRequestDialog {
    /// Session ID being shared.
    pub session_id: String,
    /// Session title for display.
    pub session_title: String,
    /// Viewer ID requesting control.
    pub viewer_id: String,
    /// Display name of the viewer.
    pub display_name: String,
    /// When the dialog was created (for auto-timeout).
    pub created_at: std::time::Instant,
}

#[cfg(feature = "pro")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateRelationshipField {
    Search,
    Label,
}

/// Dialog for creating a relationship between two sessions (Premium)
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct CreateRelationshipDialog {
    pub relation_type: RelationType,
    pub session_a_id: String,
    pub session_a_title: String,
    pub search_input: TextInput,
    pub all_sessions: Vec<(String, String)>, // (id, title)
    pub matches: Vec<(String, String)>,
    pub selected: usize,
    pub label: TextInput,
    pub field: CreateRelationshipField,
}

#[cfg(feature = "pro")]
impl CreateRelationshipDialog {
    fn fuzzy_match(query: &str, text: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        let t = text.to_lowercase();
        let mut pos = 0usize;
        for ch in q.chars() {
            if let Some(found) = t[pos..].find(ch) {
                pos += found + ch.len_utf8();
            } else {
                return false;
            }
        }
        true
    }

    pub fn update_matches(&mut self) {
        let q = self.search_input.text().trim();
        self.matches = self
            .all_sessions
            .iter()
            .filter(|(id, title)| {
                *id != self.session_a_id && Self::fuzzy_match(q, title)
            })
            .cloned()
            .collect();
        if self.selected >= self.matches.len() {
            self.selected = 0;
        }
    }

    pub fn selected_session(&self) -> Option<&(String, String)> {
        self.matches.get(self.selected)
    }

    pub fn cycle_relation_type(&mut self) {
        self.relation_type = match self.relation_type {
            RelationType::Peer => RelationType::Dependency,
            RelationType::Dependency => RelationType::Collaboration,
            RelationType::Collaboration => RelationType::ParentChild,
            RelationType::ParentChild => RelationType::Custom,
            RelationType::Custom => RelationType::Peer,
        };
    }
}

/// Dialog for adding an annotation to a relationship (Premium)
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct AnnotateDialog {
    pub relationship_id: String,
    pub note: TextInput,
}

/// Injection method for new-from-context
#[cfg(feature = "pro")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextInjectionMethod {
    InitialPrompt,
    ClaudeMd,
    EnvironmentVariable,
}

#[cfg(feature = "pro")]
impl ContextInjectionMethod {
    pub fn cycle(&self) -> Self {
        match self {
            Self::InitialPrompt => Self::ClaudeMd,
            Self::ClaudeMd => Self::EnvironmentVariable,
            Self::EnvironmentVariable => Self::InitialPrompt,
        }
    }
}

#[cfg(feature = "pro")]
impl std::fmt::Display for ContextInjectionMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitialPrompt => write!(f, "Initial Prompt"),
            Self::ClaudeMd => write!(f, "CLAUDE.md"),
            Self::EnvironmentVariable => write!(f, "Environment Variable"),
        }
    }
}

/// Dialog for creating a new session from relationship context (Premium)
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct NewFromContextDialog {
    pub relationship_id: String,
    pub context_preview: String,
    pub title: TextInput,
    pub injection_method: ContextInjectionMethod,
}

// ── Settings Dialog ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    AI,
    Sharing,
    #[cfg(feature = "pro")]
    Notification,
    General,
}

impl SettingsTab {
    pub fn available_tabs() -> Vec<SettingsTab> {
        let mut tabs = Vec::new();
        #[cfg(feature = "max")]
        tabs.push(SettingsTab::AI);
        #[cfg(feature = "pro")]
        tabs.push(SettingsTab::Sharing);
        #[cfg(feature = "pro")]
        tabs.push(SettingsTab::Notification);
        tabs.push(SettingsTab::General);
        tabs
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AI => "AI",
            Self::Sharing => "Sharing",
            #[cfg(feature = "pro")]
            Self::Notification => "Sound",
            Self::General => "General",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    // AI tab
    AiProvider,
    AiApiKey,
    AiModel,
    AiBaseUrl,
    AiSummaryLines,
    AiTest,
    // Sharing tab
    RelayServerUrl,
    TmateHost,
    TmatePort,
    DefaultPermission,
    AutoExpire,
    // Notification tab (Pro) — Hook Integration section
    #[cfg(feature = "pro")]
    NotifHookStatus,
    #[cfg(feature = "pro")]
    NotifAutoRegister,
    // Notification tab (Pro) — Sound section
    #[cfg(feature = "pro")]
    NotifEnabled,
    #[cfg(feature = "pro")]
    NotifSoundPack,
    #[cfg(feature = "pro")]
    NotifOnComplete,
    #[cfg(feature = "pro")]
    NotifOnInput,
    #[cfg(feature = "pro")]
    NotifOnError,
    #[cfg(feature = "pro")]
    NotifVolume,
    #[cfg(feature = "pro")]
    NotifTestSound,
    #[cfg(feature = "pro")]
    NotifPackLink,
    // General tab
    AnalyticsEnabled,
    MouseCapture,
    JumpLines,
    ScrollPadding,
    ReadyTtl,
}

impl SettingsField {
    pub fn fields_for_tab(tab: SettingsTab) -> Vec<SettingsField> {
        match tab {
            SettingsTab::AI => vec![
                Self::AiProvider,
                Self::AiApiKey,
                Self::AiModel,
                Self::AiBaseUrl,
                Self::AiSummaryLines,
                Self::AiTest,
            ],
            SettingsTab::Sharing => vec![
                Self::RelayServerUrl,
                Self::DefaultPermission,
                Self::AutoExpire,
            ],
            #[cfg(feature = "pro")]
            SettingsTab::Notification => vec![
                Self::NotifHookStatus,
                Self::NotifAutoRegister,
                Self::NotifEnabled,
                Self::NotifSoundPack,
                Self::NotifOnComplete,
                Self::NotifOnInput,
                Self::NotifOnError,
                Self::NotifVolume,
                Self::NotifTestSound,
                Self::NotifPackLink,
            ],
            SettingsTab::General => vec![
                Self::AnalyticsEnabled,
                Self::MouseCapture,
                Self::JumpLines,
                Self::ScrollPadding,
                Self::ReadyTtl,
            ],
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AiProvider => "Provider",
            Self::AiApiKey => "API Key",
            Self::AiModel => "Model",
            Self::AiBaseUrl => "Base URL",
            Self::AiSummaryLines => "Summary Lines",
            Self::AiTest => "Test Connection",
            Self::RelayServerUrl => "Relay Server",
            Self::TmateHost => "tmate Host",
            Self::TmatePort => "tmate Port",
            Self::DefaultPermission => "Default Permission",
            Self::AutoExpire => "Auto-Expire (min)",
            #[cfg(feature = "pro")]
            Self::NotifHookStatus => "Hook Status",
            #[cfg(feature = "pro")]
            Self::NotifAutoRegister => "Auto-Register",
            #[cfg(feature = "pro")]
            Self::NotifEnabled => "Enabled",
            #[cfg(feature = "pro")]
            Self::NotifSoundPack => "Sound Pack",
            #[cfg(feature = "pro")]
            Self::NotifOnComplete => "On Complete",
            #[cfg(feature = "pro")]
            Self::NotifOnInput => "On Input",
            #[cfg(feature = "pro")]
            Self::NotifOnError => "On Error",
            #[cfg(feature = "pro")]
            Self::NotifVolume => "Volume",
            #[cfg(feature = "pro")]
            Self::NotifTestSound => "Test Sound",
            #[cfg(feature = "pro")]
            Self::NotifPackLink => "Install Packs",
            Self::AnalyticsEnabled => "Analytics",
            Self::MouseCapture => "Mouse Capture",
            Self::JumpLines => "Jump Lines",
            Self::ScrollPadding => "Scroll Padding",
            Self::ReadyTtl => "Ready TTL (min)",
        }
    }

    pub fn is_text_input(&self) -> bool {
        matches!(
            self,
            Self::AiApiKey
                | Self::AiModel
                | Self::AiBaseUrl
                | Self::AiSummaryLines
                | Self::RelayServerUrl
                | Self::TmateHost
                | Self::TmatePort
                | Self::AutoExpire
                | Self::JumpLines
                | Self::ScrollPadding
                | Self::ReadyTtl
        )
    }

    /// Whether this field is a selector (toggle/cycle) type.
    pub fn is_selector(&self) -> bool {
        match self {
            Self::AiProvider | Self::DefaultPermission | Self::AnalyticsEnabled | Self::MouseCapture => true,
            #[cfg(feature = "pro")]
            Self::NotifAutoRegister
            | Self::NotifEnabled
            | Self::NotifSoundPack
            | Self::NotifOnComplete
            | Self::NotifOnInput
            | Self::NotifOnError => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsDialog {
    pub tab: SettingsTab,
    pub field: SettingsField,
    // AI
    pub ai_provider_idx: usize,
    pub ai_provider_names: Vec<String>,
    pub ai_api_key: TextInput,
    pub ai_model: TextInput,
    pub ai_base_url: TextInput,
    pub ai_summary_lines: TextInput,
    pub ai_test_status: Option<String>,
    // Sharing
    pub relay_url: TextInput,
    pub tmate_host: TextInput,
    pub tmate_port: TextInput,
    pub default_permission: String,
    pub auto_expire: TextInput,
    // Hook Integration (Pro)
    #[cfg(feature = "pro")]
    pub hook_tools: Vec<agent_hooks::ToolInfo>,
    #[cfg(feature = "pro")]
    pub hook_auto_register: bool,
    #[cfg(feature = "pro")]
    pub hook_selected_tool: usize,
    // Notification (Pro)
    #[cfg(feature = "pro")]
    pub notif_enabled: bool,
    #[cfg(feature = "pro")]
    pub notif_pack_names: Vec<String>,
    #[cfg(feature = "pro")]
    pub notif_pack_idx: usize,
    #[cfg(feature = "pro")]
    pub notif_on_complete: bool,
    #[cfg(feature = "pro")]
    pub notif_on_input: bool,
    #[cfg(feature = "pro")]
    pub notif_on_error: bool,
    #[cfg(feature = "pro")]
    pub notif_volume: TextInput,
    #[cfg(feature = "pro")]
    pub notif_test_status: Option<String>,
    // General
    pub analytics_enabled: bool,
    /// 0=Auto, 1=On, 2=Off
    pub mouse_capture_mode: u8,
    pub jump_lines: TextInput,
    pub scroll_padding: TextInput,
    pub ready_ttl: TextInput,
    // State
    pub editing: bool,
    pub dirty: bool,
}

impl SettingsDialog {
    #[allow(unused_variables)]
    pub fn new(cfg: &crate::config::ConfigFile) -> Self {
        // Build provider list + AI fields (max-gated)
        #[cfg(feature = "max")]
        let (ai_provider_names, ai_provider_idx, ai_api_key, ai_model, ai_base_url, ai_summary_lines) = {
            let names: Vec<String> = ai_api_provider::PROVIDERS
                .iter()
                .map(|p| p.name.to_string())
                .collect();
            let idx = names
                .iter()
                .position(|n| n == &cfg.ai.provider)
                .unwrap_or(0);
            let key = TextInput::with_text(&cfg.ai.api_key);
            let model = TextInput::with_text(&cfg.ai.model);
            let base = TextInput::with_text(cfg.ai.base_url.as_deref().unwrap_or(""));
            let lines = TextInput::with_text(cfg.ai.summary_lines.to_string());
            (names, idx, key, model, base, lines)
        };
        #[cfg(not(feature = "max"))]
        let (ai_provider_names, ai_provider_idx, ai_api_key, ai_model, ai_base_url, ai_summary_lines) = {
            (Vec::<String>::new(), 0usize, TextInput::new(), TextInput::new(), TextInput::new(), TextInput::with_text("200"))
        };

        let tabs = SettingsTab::available_tabs();
        let first_tab = tabs.first().copied().unwrap_or(SettingsTab::General);
        let first_field = SettingsField::fields_for_tab(first_tab)
            .first()
            .copied()
            .unwrap_or(SettingsField::AnalyticsEnabled);

        // Hook integration fields (Pro)
        #[cfg(feature = "pro")]
        let (hook_tools, hook_auto_register) = {
            (agent_hooks::detect_all(), cfg.hooks().auto_register)
        };

        // Notification fields (Pro)
        #[cfg(feature = "pro")]
        let (notif_enabled, notif_pack_names, notif_pack_idx, notif_on_complete, notif_on_input, notif_on_error, notif_volume) = {
            let nc = cfg.notification();
            let mut pack_names = crate::pro::notification::SoundPack::list_installed();
            // Ensure current config pack is in the list even if not found on disk
            if !pack_names.iter().any(|n| n == &nc.sound_pack) {
                pack_names.insert(0, nc.sound_pack.clone());
            }
            let idx = pack_names.iter().position(|n| n == &nc.sound_pack).unwrap_or(0);
            (
                nc.enabled,
                pack_names,
                idx,
                nc.on_task_complete,
                nc.on_input_required,
                nc.on_error,
                TextInput::with_text(format!("{:.0}", nc.volume * 100.0)),
            )
        };

        Self {
            tab: first_tab,
            field: first_field,
            ai_provider_idx,
            ai_provider_names,
            ai_api_key,
            ai_model,
            ai_base_url,
            ai_summary_lines,
            ai_test_status: None,
            relay_url: TextInput::with_text(
                cfg.sharing().relay_server_url.as_deref().unwrap_or(""),
            ),
            tmate_host: TextInput::with_text(&cfg.sharing().tmate_server_host),
            tmate_port: TextInput::with_text(cfg.sharing().tmate_server_port.to_string()),
            default_permission: cfg.sharing().default_permission.clone(),
            auto_expire: TextInput::with_text(
                cfg.sharing()
                    .auto_expire_minutes
                    .map(|m| m.to_string())
                    .unwrap_or_default(),
            ),
            #[cfg(feature = "pro")]
            hook_tools,
            #[cfg(feature = "pro")]
            hook_auto_register,
            #[cfg(feature = "pro")]
            hook_selected_tool: 0,
            #[cfg(feature = "pro")]
            notif_enabled,
            #[cfg(feature = "pro")]
            notif_pack_names,
            #[cfg(feature = "pro")]
            notif_pack_idx,
            #[cfg(feature = "pro")]
            notif_on_complete,
            #[cfg(feature = "pro")]
            notif_on_input,
            #[cfg(feature = "pro")]
            notif_on_error,
            #[cfg(feature = "pro")]
            notif_volume,
            #[cfg(feature = "pro")]
            notif_test_status: None,
            analytics_enabled: cfg.analytics_enabled(),
            mouse_capture_mode: match cfg.mouse_capture() {
                crate::config::MouseCaptureMode::Auto => 0,
                crate::config::MouseCaptureMode::On => 1,
                crate::config::MouseCaptureMode::Off => 2,
            },
            jump_lines: TextInput::with_text(cfg.jump_lines().to_string()),
            scroll_padding: TextInput::with_text(cfg.scroll_padding().to_string()),
            ready_ttl: TextInput::with_text(cfg.ready_ttl_minutes().to_string()),
            editing: false,
            dirty: false,
        }
    }

    pub fn current_fields(&self) -> Vec<SettingsField> {
        SettingsField::fields_for_tab(self.tab)
    }

    pub fn active_input(&mut self) -> Option<&mut TextInput> {
        match self.field {
            SettingsField::AiApiKey => Some(&mut self.ai_api_key),
            SettingsField::AiModel => Some(&mut self.ai_model),
            SettingsField::AiBaseUrl => Some(&mut self.ai_base_url),
            SettingsField::AiSummaryLines => Some(&mut self.ai_summary_lines),
            SettingsField::RelayServerUrl => Some(&mut self.relay_url),
            SettingsField::TmateHost => Some(&mut self.tmate_host),
            SettingsField::TmatePort => Some(&mut self.tmate_port),
            SettingsField::AutoExpire => Some(&mut self.auto_expire),
            #[cfg(feature = "pro")]
            SettingsField::NotifVolume => Some(&mut self.notif_volume),
            SettingsField::JumpLines => Some(&mut self.jump_lines),
            SettingsField::ScrollPadding => Some(&mut self.scroll_padding),
            SettingsField::ReadyTtl => Some(&mut self.ready_ttl),
            _ => None,
        }
    }

    /// Cycle sound pack selection (Pro)
    #[cfg(feature = "pro")]
    pub fn cycle_pack(&mut self, delta: i32) {
        let len = self.notif_pack_names.len();
        if len == 0 {
            return;
        }
        if delta > 0 {
            self.notif_pack_idx = (self.notif_pack_idx + 1) % len;
        } else if self.notif_pack_idx == 0 {
            self.notif_pack_idx = len - 1;
        } else {
            self.notif_pack_idx -= 1;
        }
        self.dirty = true;
    }

    /// Get the currently selected pack name (Pro)
    #[cfg(feature = "pro")]
    pub fn pack_display(&self) -> &str {
        self.notif_pack_names
            .get(self.notif_pack_idx)
            .map(|s| s.as_str())
            .unwrap_or("(none)")
    }

    /// Refresh hook tool detection status (Pro).
    #[cfg(feature = "pro")]
    pub fn refresh_hook_status(&mut self) {
        self.hook_tools = agent_hooks::detect_all();
    }

    /// Toggle hook registration for the selected tool (Pro).
    /// Returns an action description if something happened.
    #[cfg(feature = "pro")]
    pub fn toggle_selected_hook(&mut self) -> Option<String> {
        let Some(info) = self.hook_tools.get(self.hook_selected_tool) else {
            return None;
        };
        let bridge = crate::claude::bridge_script_path()?;
        let adapter = agent_hooks::get_adapter(&info.name)?;
        let result = match info.status {
            agent_hooks::ToolStatus::Detected => {
                adapter.register_hooks(&bridge).ok()?;
                Some(format!("Registered hooks for {}", info.display_name))
            }
            agent_hooks::ToolStatus::HooksRegistered => {
                adapter.unregister_hooks().ok()?;
                Some(format!("Unregistered hooks for {}", info.display_name))
            }
            agent_hooks::ToolStatus::NotInstalled => None,
        };
        self.refresh_hook_status();
        result
    }

    /// Cycle through tool list when on HookStatus field (Pro).
    #[cfg(feature = "pro")]
    pub fn cycle_hook_tool(&mut self, delta: i32) {
        let len = self.hook_tools.len();
        if len == 0 {
            return;
        }
        if delta > 0 {
            self.hook_selected_tool = (self.hook_selected_tool + 1) % len;
        } else if self.hook_selected_tool == 0 {
            self.hook_selected_tool = len - 1;
        } else {
            self.hook_selected_tool -= 1;
        }
    }

    pub fn move_field(&mut self, delta: i32) {
        let fields = self.current_fields();
        if fields.is_empty() {
            return;
        }
        let idx = fields.iter().position(|f| *f == self.field).unwrap_or(0);
        let new_idx = if delta > 0 {
            (idx + 1) % fields.len()
        } else if idx == 0 {
            fields.len() - 1
        } else {
            idx - 1
        };
        self.field = fields[new_idx];
    }

    pub fn switch_tab(&mut self, delta: i32) {
        let tabs = SettingsTab::available_tabs();
        if tabs.is_empty() {
            return;
        }
        let idx = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
        let new_idx = if delta > 0 {
            (idx + 1) % tabs.len()
        } else if idx == 0 {
            tabs.len() - 1
        } else {
            idx - 1
        };
        self.tab = tabs[new_idx];
        let fields = SettingsField::fields_for_tab(self.tab);
        self.field = fields.first().copied().unwrap_or(SettingsField::AnalyticsEnabled);
    }

    pub fn provider_display(&self) -> &str {
        self.ai_provider_names
            .get(self.ai_provider_idx)
            .map(|s| s.as_str())
            .unwrap_or("(none)")
    }

    pub fn cycle_provider(&mut self, delta: i32) {
        let len = self.ai_provider_names.len();
        if len == 0 {
            return;
        }
        if delta > 0 {
            self.ai_provider_idx = (self.ai_provider_idx + 1) % len;
        } else if self.ai_provider_idx == 0 {
            self.ai_provider_idx = len - 1;
        } else {
            self.ai_provider_idx -= 1;
        }
        self.dirty = true;
    }

    pub fn toggle_permission(&mut self) {
        self.default_permission = if self.default_permission == "ro" {
            "rw".to_string()
        } else {
            "ro".to_string()
        };
        self.dirty = true;
    }

    pub fn masked_api_key(&self) -> String {
        let key = self.ai_api_key.text();
        if key.len() <= 4 {
            "*".repeat(key.len())
        } else {
            format!("{}****{}", &key[..3], &key[key.len() - 4..])
        }
    }
}

impl NewSessionDialog {
    pub fn new(default_path: PathBuf, default_group: String, all_groups: Vec<String>) -> Self {
        let mut d = Self {
            path: TextInput::with_text(default_path.to_string_lossy().to_string()),
            title: TextInput::new(),
            group_path: TextInput::with_text(default_group),
            field: NewSessionField::Path,
            group_all_groups: all_groups,
            group_matches: Vec::new(),
            group_selected: 0,
            path_suggestions: Vec::new(),
            path_suggestions_idx: 0,
            path_suggestions_visible: false,
            path_dirty: false,
            path_last_edit: std::time::Instant::now(),
        };
        d.update_group_matches();
        d
    }

    pub fn clear_path_suggestions(&mut self) {
        self.path_suggestions.clear();
        self.path_suggestions_idx = 0;
        self.path_suggestions_visible = false;
    }

    fn expand_home(path: &str) -> PathBuf {
        let trimmed = path.trim();
        if trimmed == "~" {
            return dirs::home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
        }
        if let Some(rest) = trimmed.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(rest);
            }
        }
        PathBuf::from(trimmed)
    }

    pub fn expanded_path(&self) -> PathBuf {
        Self::expand_home(self.path.text())
    }

    pub fn path_will_be_created(&self) -> bool {
        let p = self.expanded_path();
        !p.as_os_str().is_empty() && !p.exists()
    }

    fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
        if query.is_empty() {
            return Some(0);
        }

        let mut score: i32 = 0;
        let mut last_match: Option<usize> = None;
        let mut pos = 0usize;

        for ch in query.chars() {
            if let Some(found) = text[pos..].find(ch) {
                let idx = pos + found;
                score += 10;
                if let Some(prev) = last_match {
                    if idx == prev + 1 {
                        score += 15;
                    } else {
                        score -= (idx.saturating_sub(prev) as i32).min(10);
                    }
                } else {
                    score -= idx.min(15) as i32;
                }
                last_match = Some(idx);
                pos = idx + ch.len_utf8();
            } else {
                return None;
            }
        }

        Some(score)
    }

    pub fn complete_path_or_cycle(&mut self, backwards: bool) {
        if self.path_suggestions_visible && !self.path_suggestions.is_empty() {
            if backwards {
                if self.path_suggestions_idx == 0 {
                    self.path_suggestions_idx = self.path_suggestions.len() - 1;
                } else {
                    self.path_suggestions_idx -= 1;
                }
            } else {
                self.path_suggestions_idx =
                    (self.path_suggestions_idx + 1) % self.path_suggestions.len();
            }
            return;
        }

        self.update_path_suggestions();

        // Keep the original behavior for manual completion: if there's exactly one match, apply it.
        if self.path_suggestions.len() == 1 {
            self.path.set_text(self.path_suggestions[0].clone());
            self.clear_path_suggestions();
        }
    }

    pub fn update_path_suggestions(&mut self) {
        // Compute suggestions once
        self.clear_path_suggestions();

        let expanded = Self::expand_home(self.path.text());
        let raw = expanded.to_string_lossy().to_string();
        let (dir, prefix, base_has_slash) = match raw.rfind('/') {
            Some(idx) => (
                PathBuf::from(&raw[..=idx]),
                raw[idx + 1..].to_string(),
                true,
            ),
            None => (PathBuf::from("./"), raw.clone(), false),
        };

        let Ok(rd) = std::fs::read_dir(&dir) else {
            return;
        };

        use std::time::UNIX_EPOCH;

        let q = prefix.to_lowercase();

        let mut matches: Vec<(i64, i32, String)> = Vec::new();
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let name_lc = name.to_lowercase();

            let score = if q.is_empty() {
                Some(0)
            } else {
                Self::fuzzy_score(&q, &name_lc)
            };
            if score.is_none() {
                continue;
            }

            let mtime = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let mut full = dir.join(&name).to_string_lossy().to_string();
            if e.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
                full.push('/');
            }

            matches.push((mtime, score.unwrap_or(0), full));
        }

        matches.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        if matches.is_empty() {
            return;
        }

        // Show suggestion list (do not auto-apply arbitrary choice)
        self.path_suggestions = matches.into_iter().map(|(_, _, p)| p).collect();
        self.path_suggestions_visible = true;
        self.path_suggestions_idx = 0;

        // If the user didn't type a slash and is completing in CWD, keep relative feeling.
        if !base_has_slash && self.path.text().starts_with('~') {
            // leave as-is
        }
    }

    pub fn apply_selected_path_suggestion(&mut self) {
        if !self.path_suggestions_visible {
            return;
        }
        if let Some(sel) = self
            .path_suggestions
            .get(self.path_suggestions_idx)
            .cloned()
        {
            self.path.set_text(sel);
        }
        self.clear_path_suggestions();
    }

    pub fn validate(&self) -> Result<PathBuf> {
        let project_path = Self::expand_home(self.path.text());
        if project_path.as_os_str().is_empty() {
            return Err(crate::Error::InvalidInput("Path is empty".to_string()));
        }

        if !project_path.exists() {
            std::fs::create_dir_all(&project_path)?;
        }

        let project_path = project_path.canonicalize()?;
        if !project_path.is_dir() {
            return Err(crate::Error::InvalidInput(format!(
                "Path is not a directory: {}",
                project_path.display()
            )));
        }
        Ok(project_path)
    }
}

/// Dialog for browsing and installing sound packs from the registry.
#[cfg(feature = "pro")]
#[derive(Debug, Clone)]
pub struct PackBrowserDialog {
    /// Available packs from the registry.
    pub packs: Vec<crate::pro::notification::registry::RegistryPack>,
    /// Currently selected index.
    pub selected: usize,
    /// Status message (loading, installing, error).
    pub status: String,
    /// Whether we're currently loading the pack list.
    pub loading: bool,
    /// Whether we're currently installing a pack.
    pub installing: bool,
}

#[cfg(feature = "pro")]
impl PackBrowserDialog {
    pub fn new() -> Self {
        Self {
            packs: Vec::new(),
            selected: 0,
            status: "Loading pack list...".to_string(),
            loading: true,
            installing: false,
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.packs.is_empty() {
            return;
        }
        let len = self.packs.len() as i32;
        let new_idx = (self.selected as i32 + delta).rem_euclid(len);
        self.selected = new_idx as usize;
    }

    pub fn selected_pack(&self) -> Option<&crate::pro::notification::registry::RegistryPack> {
        self.packs.get(self.selected)
    }
}
