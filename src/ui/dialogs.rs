use std::path::PathBuf;

use crate::error::Result;

use super::input::TextInput;

// Pro/Max dialog types re-exported from pro module
#[cfg(feature = "pro")]
pub use crate::pro::ui::dialogs::*;
#[cfg(feature = "pro")]
pub use crate::pro::ui::dialogs_max::*;

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
    SessionId,
}

#[derive(Debug, Clone)]
pub struct RenameSessionDialog {
    pub session_id: String,
    pub old_title: String,
    pub new_title: TextInput,
    pub label: TextInput,
    pub label_color: crate::session::LabelColor,
    /// Editable CLI session ID (for manual correction)
    pub cli_session_id: TextInput,
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
    PackBrowser(PackBrowserDialog),
    #[cfg(feature = "pro")]
    OrphanedRooms(OrphanedRoomsDialog),
    #[cfg(feature = "pro")]
    SkillsManager(SkillsManagerDialog),
    #[cfg(feature = "pro")]
    HumanReview(HumanReviewDialog),
    #[cfg(feature = "pro")]
    ProposalAction(ProposalActionDialog),
    #[cfg(feature = "pro")]
    ConfirmInjection(ConfirmInjectionDialog),
    #[cfg(feature = "pro")]
    AiAnalysis(AiAnalysisDialog),
    #[cfg(feature = "pro")]
    BehaviorAnalysis(BehaviorAnalysisDialog),
}

// ── Pro/Max dialog struct definitions in pro/src/ui/dialogs*.rs ──

// ── Settings Dialog ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    AI,
    Sharing,
    Notification,
    General,
    Keys,
}

impl SettingsTab {
    pub fn available_tabs() -> Vec<SettingsTab> {
        let mut tabs = Vec::new();
        #[cfg(feature = "pro")]
        tabs.push(SettingsTab::AI);
        #[cfg(feature = "pro")]
        tabs.push(SettingsTab::Sharing);
        tabs.push(SettingsTab::Notification);
        tabs.push(SettingsTab::General);
        tabs.push(SettingsTab::Keys);
        tabs
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AI => "AI",
            Self::Sharing => "Sharing",
            Self::Notification => "Sound",
            Self::General => "General",
            Self::Keys => "Keys",
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
    // Notification tab — Hook Integration section
    NotifHookStatus,
    NotifAutoRegister,
    // Notification tab — Sound section
    NotifEnabled,
    NotifSoundPack,
    NotifOnComplete,
    NotifOnInput,
    NotifOnError,
    NotifVolume,
    NotifTestSound,
    NotifPackLink,
    // General tab
    AnimationsEnabled,
    PromptCollection,
    AnalyticsEnabled,
    MouseCapture,
    JumpLines,
    ScrollPadding,
    ReadyTtl,
    Language,
    // General tab — Auto-permission flags (per-tool resume)
    ClaudeSkipPerms,
    CodexFullAuto,
    GeminiYolo,
    // Keys tab
    KeyUp,
    KeyDown,
    KeyHalfPageDown,
    KeyHalfPageUp,
    KeySelect,
    KeyStart,
    KeyStop,
    KeyRestart,
    KeyDelete,
    KeyRename,
    KeyNewSession,
    KeyFork,
    KeyCanvasToggle,
    KeySummarize,
    KeyBehaviorAnalysis,
    KeySearch,
    KeySettings,
    KeyBoost,
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
                Self::AnimationsEnabled,
                Self::PromptCollection,
                Self::AnalyticsEnabled,
                Self::MouseCapture,
                Self::JumpLines,
                Self::ScrollPadding,
                Self::ReadyTtl,
                Self::Language,
                Self::ClaudeSkipPerms,
                Self::CodexFullAuto,
                Self::GeminiYolo,
            ],
            SettingsTab::Keys => vec![
                Self::KeyUp,
                Self::KeyDown,
                Self::KeyHalfPageDown,
                Self::KeyHalfPageUp,
                Self::KeySelect,
                Self::KeyStart,
                Self::KeyStop,
                Self::KeyRestart,
                Self::KeyDelete,
                Self::KeyRename,
                Self::KeyNewSession,
                Self::KeyFork,
                Self::KeyCanvasToggle,
                Self::KeySummarize,
                Self::KeyBehaviorAnalysis,
                Self::KeySearch,
                Self::KeySettings,
                Self::KeyBoost,
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
            Self::NotifHookStatus => "Hook Status",
            Self::NotifAutoRegister => "Auto-Register",
            Self::NotifEnabled => "Enabled",
            Self::NotifSoundPack => "Sound Pack",
            Self::NotifOnComplete => "On Complete",
            Self::NotifOnInput => "On Input",
            Self::NotifOnError => "On Error",
            Self::NotifVolume => "Volume",
            Self::NotifTestSound => "Test Sound",
            Self::NotifPackLink => "Install Packs",
            Self::AnimationsEnabled => "Animations",
            Self::PromptCollection => "Prompt Collection",
            Self::AnalyticsEnabled => "Analytics",
            Self::MouseCapture => "Mouse Capture",
            Self::JumpLines => "Jump Lines",
            Self::ScrollPadding => "Scroll Padding",
            Self::ReadyTtl => "Ready TTL (min)",
            Self::Language => "Language",
            Self::ClaudeSkipPerms => "Claude: Auto-Permit",
            Self::CodexFullAuto => "Codex: Full Auto",
            Self::GeminiYolo => "Gemini: Yolo Mode",
            Self::KeyUp => "Up",
            Self::KeyDown => "Down",
            Self::KeyHalfPageDown => "Half Page Down",
            Self::KeyHalfPageUp => "Half Page Up",
            Self::KeySelect => "Select",
            Self::KeyStart => "Start Session",
            Self::KeyStop => "Stop Session",
            Self::KeyRestart => "Restart",
            Self::KeyDelete => "Delete",
            Self::KeyRename => "Rename",
            Self::KeyNewSession => "New Session",
            Self::KeyFork => "Fork Session",
            Self::KeyCanvasToggle => "Canvas Toggle",
            Self::KeySummarize => "AI Summarize",
            Self::KeyBehaviorAnalysis => "Behavior Analysis",
            Self::KeySearch => "Search",
            Self::KeySettings => "Settings",
            Self::KeyBoost => "Boost",
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
            Self::AiProvider | Self::DefaultPermission | Self::AnimationsEnabled | Self::PromptCollection | Self::AnalyticsEnabled | Self::MouseCapture | Self::Language | Self::ClaudeSkipPerms | Self::CodexFullAuto | Self::GeminiYolo => true,
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

    pub fn is_key_binding(&self) -> bool {
        matches!(
            self,
            Self::KeyUp
                | Self::KeyDown
                | Self::KeyHalfPageDown
                | Self::KeyHalfPageUp
                | Self::KeySelect
                | Self::KeyStart
                | Self::KeyStop
                | Self::KeyRestart
                | Self::KeyDelete
                | Self::KeyRename
                | Self::KeyNewSession
                | Self::KeyFork
                | Self::KeyCanvasToggle
                | Self::KeySummarize
                | Self::KeyBehaviorAnalysis
                | Self::KeySearch
                | Self::KeySettings
                | Self::KeyBoost
        )
    }

    pub fn key_action(&self) -> Option<&'static str> {
        match self {
            Self::KeyUp => Some("up"),
            Self::KeyDown => Some("down"),
            Self::KeyHalfPageDown => Some("half_page_down"),
            Self::KeyHalfPageUp => Some("half_page_up"),
            Self::KeySelect => Some("select"),
            Self::KeyStart => Some("start"),
            Self::KeyStop => Some("stop"),
            Self::KeyRestart => Some("restart"),
            Self::KeyDelete => Some("delete"),
            Self::KeyRename => Some("rename"),
            Self::KeyNewSession => Some("new_session"),
            Self::KeyFork => Some("fork"),
            Self::KeyCanvasToggle => Some("canvas_toggle"),
            Self::KeySummarize => Some("summarize"),
            Self::KeyBehaviorAnalysis => Some("behavior_analysis"),
            Self::KeySearch => Some("search"),
            Self::KeySettings => Some("settings"),
            Self::KeyBoost => Some("boost"),
            _ => None,
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
    pub hook_tools: Vec<agent_hooks::ToolInfo>,
    pub hook_auto_register: bool,
    pub hook_selected_tool: usize,
    // Notification (Free tier)
    pub notif_enabled: bool,
    pub notif_pack_names: Vec<String>,
    pub notif_pack_idx: usize,
    pub notif_on_complete: bool,
    pub notif_on_input: bool,
    pub notif_on_error: bool,
    pub notif_volume: TextInput,
    pub notif_test_status: Option<String>,
    // General
    pub animations_enabled: bool,
    pub prompt_collection: bool,
    pub analytics_enabled: bool,
    /// 0=Auto, 1=On, 2=Off
    pub mouse_capture_mode: u8,
    pub jump_lines: TextInput,
    pub scroll_padding: TextInput,
    pub ready_ttl: TextInput,
    /// 0=English, 1=Chinese
    pub language_idx: usize,
    // Auto-permission flags
    pub claude_skip_perms: bool,
    pub codex_full_auto: bool,
    pub gemini_yolo: bool,
    // Keys tab
    pub key_bindings: std::collections::HashMap<&'static str, Vec<crate::config::KeySpec>>,
    /// When true, waiting for next keypress to capture as new binding
    pub key_capturing: bool,
    // State
    pub editing: bool,
    pub dirty: bool,
}

impl SettingsDialog {
    #[allow(unused_variables)]
    pub fn new(cfg: &crate::config::ConfigFile, kb: &crate::config::KeyBindings) -> Self {
        // Build provider list + AI fields (max-gated)
        #[cfg(feature = "pro")]
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

        // Hook integration fields
        let (hook_tools, hook_auto_register) = {
            (agent_hooks::detect_all(), cfg.hooks().auto_register)
        };

        // Notification fields
        let (notif_enabled, notif_pack_names, notif_pack_idx, notif_on_complete, notif_on_input, notif_on_error, notif_volume) = {
            let nc = cfg.notification();
            let mut pack_names = crate::notification::SoundPack::list_installed();
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
            hook_tools,
            hook_auto_register,
            hook_selected_tool: 0,
            notif_enabled,
            notif_pack_names,
            notif_pack_idx,
            notif_on_complete,
            notif_on_input,
            notif_on_error,
            notif_volume,
            notif_test_status: None,
            animations_enabled: cfg.animations_enabled(),
            prompt_collection: cfg.claude_user_prompt_logging(),
            analytics_enabled: cfg.analytics_enabled(),
            mouse_capture_mode: match cfg.mouse_capture() {
                crate::config::MouseCaptureMode::Auto => 0,
                crate::config::MouseCaptureMode::On => 1,
                crate::config::MouseCaptureMode::Off => 2,
            },
            jump_lines: TextInput::with_text(cfg.jump_lines().to_string()),
            scroll_padding: TextInput::with_text(cfg.scroll_padding().to_string()),
            ready_ttl: TextInput::with_text(cfg.ready_ttl_minutes().to_string()),
            language_idx: match cfg.language.as_ref().map(|s| crate::i18n::Language::from_str(s)) {
                Some(crate::i18n::Language::Chinese) => 1,
                _ => 0,
            },
            claude_skip_perms: cfg.claude.dangerously_skip_permissions,
            codex_full_auto: cfg.codex.full_auto,
            gemini_yolo: cfg.gemini.yolo,
            key_bindings: {
                let mut key_bindings = std::collections::HashMap::new();
                for action in ["up", "down", "half_page_down", "half_page_up", "select", "start", "stop", "restart", "delete", "rename", "new_session", "fork", "canvas_toggle", "summarize", "behavior_analysis", "search", "settings", "boost"] {
                    if let Some(specs) = kb.get_specs(action) {
                        key_bindings.insert(action, specs.to_vec());
                    }
                }
                key_bindings
            },
            key_capturing: false,
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
    pub fn pack_display(&self) -> &str {
        self.notif_pack_names
            .get(self.notif_pack_idx)
            .map(|s| s.as_str())
            .unwrap_or("(none)")
    }

    /// Refresh hook tool detection status.
    pub fn refresh_hook_status(&mut self) {
        self.hook_tools = agent_hooks::detect_all();
    }

    /// Toggle hook registration for the selected tool.
    /// Returns an action description if something happened.
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

    /// Cycle through tool list when on HookStatus field.
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
#[derive(Debug, Clone)]
pub struct PackBrowserDialog {
    /// Available packs from the registry.
    pub packs: Vec<crate::notification::registry::RegistryPack>,
    /// Currently selected index.
    pub selected: usize,
    /// Status message (loading, installing, error).
    pub status: String,
    /// Whether we're currently loading the pack list.
    pub loading: bool,
    /// Whether we're currently installing a pack.
    pub installing: bool,
}

/// Dialog for managing orphaned relay rooms detected at startup (Premium)


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

    pub fn selected_pack(&self) -> Option<&crate::notification::registry::RegistryPack> {
        self.packs.get(self.selected)
    }
}

// ── Skills Manager + AI dialogs — definitions in pro/src/ui/dialogs*.rs ──
