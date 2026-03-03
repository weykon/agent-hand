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
    General,
}

impl SettingsTab {
    pub fn available_tabs() -> Vec<SettingsTab> {
        let mut tabs = Vec::new();
        #[cfg(feature = "max")]
        tabs.push(SettingsTab::AI);
        #[cfg(feature = "pro")]
        tabs.push(SettingsTab::Sharing);
        tabs.push(SettingsTab::General);
        tabs
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AI => "AI",
            Self::Sharing => "Sharing",
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
    // General tab
    AnalyticsEnabled,
    JumpLines,
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
                Self::TmateHost,
                Self::TmatePort,
                Self::DefaultPermission,
                Self::AutoExpire,
            ],
            SettingsTab::General => vec![
                Self::AnalyticsEnabled,
                Self::JumpLines,
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
            Self::AnalyticsEnabled => "Analytics",
            Self::JumpLines => "Jump Lines",
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
                | Self::ReadyTtl
        )
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
    // General
    pub analytics_enabled: bool,
    pub jump_lines: TextInput,
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
            analytics_enabled: cfg.analytics_enabled(),
            jump_lines: TextInput::with_text(cfg.jump_lines().to_string()),
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
            SettingsField::JumpLines => Some(&mut self.jump_lines),
            SettingsField::ReadyTtl => Some(&mut self.ready_ttl),
            _ => None,
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
