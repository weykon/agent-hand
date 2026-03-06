use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::error::Result;
use crate::session::Storage;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s],
            OneOrMany::Many(v) => v,
        }
    }
}


/// Mouse capture mode for the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseCaptureMode {
    /// Detect environment: disable in nested tmux / weak terminals, enable otherwise.
    Auto,
    /// Always capture mouse events.
    On,
    /// Never capture mouse events (terminal-native selection).
    Off,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    keybindings: HashMap<String, OneOrMany>,

    #[serde(default)]
    tmux: TmuxKeys,

    #[serde(default)]
    pub analytics: AnalyticsConfig,

    #[serde(default)]
    claude: ClaudeHooksConfig,

    #[serde(default)]
    status_detection: StatusDetectionConfig,

    /// Sharing configuration (Premium)
    #[serde(default)]
    pub sharing: SharingConfig,

    /// Sound notification configuration (Pro)
    #[serde(default)]
    pub notification: NotificationConfig,

    /// Hook integration configuration
    #[serde(default)]
    pub hooks: HooksConfig,

    /// How long a session stays in "Ready (✓)" after leaving Running.
    /// Unit: minutes. Default: 40.
    #[serde(default)]
    pub ready_ttl_minutes: Option<u64>,

    /// Lines to jump with Ctrl+D / Ctrl+U. Default: 10.
    #[serde(default)]
    pub jump_lines: Option<usize>,

    /// Scroll padding: keep cursor N lines from top/bottom edge. Default: 5.
    #[serde(default)]
    pub scroll_padding: Option<usize>,

    /// Mouse capture mode: "auto" (default), "on", "off"
    #[serde(default)]
    pub mouse_capture: Option<String>,

    /// AI configuration (Max tier)
    #[cfg(feature = "max")]
    #[serde(default)]
    pub ai: AiConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct TmuxKeys {
    #[serde(default)]
    switcher: Option<String>,
    #[serde(default)]
    detach: Option<String>,
    #[serde(default)]
    jump: Option<String>,
    #[serde(default)]
    copy_mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnalyticsConfig {
    #[serde(default = "default_analytics_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ClaudeHooksConfig {
    #[serde(default)]
    user_prompt_logging: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct StatusDetectionConfig {
    #[serde(default)]
    pub prompt_contains: Vec<String>,
    #[serde(default)]
    pub prompt_regex: Vec<String>,
    #[serde(default)]
    pub busy_contains: Vec<String>,
    #[serde(default)]
    pub busy_regex: Vec<String>,
}

/// Configuration for remote session sharing (Premium)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SharingConfig {
    /// tmate relay host (default: tmate.io, or self-hosted)
    #[serde(default = "default_tmate_host")]
    pub tmate_server_host: String,
    /// tmate relay SSH port
    #[serde(default = "default_tmate_port")]
    pub tmate_server_port: u16,
    /// Default permission for new shares
    #[serde(default = "default_share_permission")]
    pub default_permission: String,
    /// Default auto-expire in minutes (None = no expiry)
    #[serde(default)]
    pub auto_expire_minutes: Option<u64>,
    /// WebSocket relay server URL (e.g. "http://localhost:9090").
    /// When set, overrides relay discovery and uses this URL directly.
    #[serde(default)]
    pub relay_server_url: Option<String>,
    /// URL for relay discovery (default: auth server's /api/relay-discover).
    /// The client calls this to get the best relay URL automatically.
    #[serde(default = "default_relay_discovery_url")]
    pub relay_discovery_url: String,
}

fn default_tmate_host() -> String {
    "tmate.io".to_string()
}

fn default_tmate_port() -> u16 {
    22
}

fn default_share_permission() -> String {
    "ro".to_string()
}

fn default_relay_discovery_url() -> String {
    "https://auth.asymptai.com/api/relay-discover".to_string()
}

impl Default for SharingConfig {
    fn default() -> Self {
        Self {
            tmate_server_host: default_tmate_host(),
            tmate_server_port: default_tmate_port(),
            default_permission: default_share_permission(),
            auto_expire_minutes: None,
            relay_server_url: None,
            relay_discovery_url: default_relay_discovery_url(),
        }
    }
}

/// Sound notification configuration (Pro tier).
/// Supports CESP (Coding Event Sound Pack) format — compatible with peon-ping packs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotificationConfig {
    /// Enable sound notifications (default: true when Pro)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Volume 0.0-1.0 (default: 0.5)
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Sound pack name — looked up in ~/.openpeon/packs/ or ~/.agent-hand/packs/
    #[serde(default = "default_pack")]
    pub sound_pack: String,
    /// Play sound on task completion (Running→Idle)
    #[serde(default = "default_true")]
    pub on_task_complete: bool,
    /// Play sound when agent needs input (→Waiting)
    #[serde(default = "default_true")]
    pub on_input_required: bool,
    /// Play sound on tool failure
    #[serde(default = "default_true")]
    pub on_error: bool,
    /// Play sound on session start (non-Running → Running)
    #[serde(default = "default_true")]
    pub on_session_start: bool,
    /// Play sound when prompt received while already running
    #[serde(default = "default_true")]
    pub on_task_acknowledge: bool,
    /// Play sound when context window is about to compact
    #[serde(default = "default_true")]
    pub on_resource_limit: bool,
    /// Play sound on rapid-fire prompt spam
    #[serde(default = "default_true")]
    pub on_user_spam: bool,
    /// Suppress sound when the session is currently attached (focused)
    #[serde(default = "default_true")]
    pub quiet_when_focused: bool,
}

fn default_true() -> bool {
    true
}
fn default_volume() -> f32 {
    0.5
}
fn default_pack() -> String {
    "peon".to_string()
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: default_volume(),
            sound_pack: default_pack(),
            on_task_complete: true,
            on_input_required: true,
            on_error: true,
            on_session_start: true,
            on_task_acknowledge: true,
            on_resource_limit: true,
            on_user_spam: true,
            quiet_when_focused: true,
        }
    }
}

/// Hook integration configuration.
/// Controls auto-registration of hooks across detected AI CLI tools.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HooksConfig {
    /// Automatically register hooks for newly detected tools on startup.
    #[serde(default = "default_true")]
    pub auto_register: bool,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            auto_register: true,
        }
    }
}

/// AI provider configuration (Max tier)
#[cfg(feature = "max")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiConfig {
    /// Provider name (e.g. "deepseek", "claude", "ollama"). Default: "deepseek"
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    /// Model override. If empty, uses provider's default.
    #[serde(default)]
    pub model: String,
    /// API key override. If empty, reads from env var.
    #[serde(default)]
    pub api_key: String,
    /// Custom base URL (for proxies or self-hosted).
    #[serde(default)]
    pub base_url: Option<String>,
    /// Lines to capture for summarization. Default: 200.
    #[serde(default = "default_summary_lines")]
    pub summary_lines: usize,
}

#[cfg(feature = "max")]
fn default_ai_provider() -> String {
    "deepseek".to_string()
}

#[cfg(feature = "max")]
fn default_summary_lines() -> usize {
    200
}

#[cfg(feature = "max")]
impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_ai_provider(),
            model: String::new(),
            api_key: String::new(),
            base_url: None,
            summary_lines: default_summary_lines(),
        }
    }
}

fn default_analytics_enabled() -> bool {
    false
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl ConfigFile {
    pub async fn load() -> Result<Option<Self>> {
        // Check multiple config paths in order of priority:
        // 1. ~/.agent-hand/config.json (legacy)
        // 2. ~/.agent-hand/config.toml
        // 3. ~/.config/agent-hand/config.toml (XDG standard)
        // 4. ~/.config/agent-hand/config.json
        let agent_hand_dir = Storage::get_agent_hand_dir()?;
        let xdg_dir = dirs::home_dir().map(|h| h.join(".config").join("agent-hand"));

        let candidates: Vec<std::path::PathBuf> = [
            Some(agent_hand_dir.join("config.toml")),
            xdg_dir.as_ref().map(|d| d.join("config.toml")),
            Some(agent_hand_dir.join("config.json")),
            xdg_dir.as_ref().map(|d| d.join("config.json")),
        ]
        .into_iter()
        .flatten()
        .collect();

        for path in candidates {
            let content = match fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let cfg: Self = match ext {
                "toml" => toml::from_str(&content)?,
                _ => serde_json::from_str(&content)?,
            };
            return Ok(Some(cfg));
        }

        Ok(None)
    }

    pub fn tmux_switcher_key(&self) -> Option<&str> {
        self.tmux.switcher.as_deref()
    }

    pub fn tmux_detach_key(&self) -> Option<&str> {
        self.tmux.detach.as_deref()
    }

    pub fn tmux_jump_key(&self) -> Option<&str> {
        self.tmux.jump.as_deref()
    }

    pub fn tmux_copy_mode(&self) -> Option<&str> {
        self.tmux.copy_mode.as_deref()
    }

    pub fn analytics_enabled(&self) -> bool {
        self.analytics.enabled
    }

    pub fn claude_user_prompt_logging(&self) -> bool {
        self.claude.user_prompt_logging
    }

    pub fn status_detection(&self) -> &StatusDetectionConfig {
        &self.status_detection
    }

    pub fn ready_ttl_minutes(&self) -> u64 {
        self.ready_ttl_minutes.unwrap_or(40)
    }

    pub fn jump_lines(&self) -> usize {
        self.jump_lines.unwrap_or(10)
    }

    pub fn scroll_padding(&self) -> usize {
        self.scroll_padding.unwrap_or(5)
    }

    pub fn mouse_capture(&self) -> MouseCaptureMode {
        match self.mouse_capture.as_deref() {
            Some("on") => MouseCaptureMode::On,
            Some("off") => MouseCaptureMode::Off,
            _ => MouseCaptureMode::Auto,
        }
    }

    pub fn sharing(&self) -> &SharingConfig {
        &self.sharing
    }

    pub fn notification(&self) -> &NotificationConfig {
        &self.notification
    }

    pub fn hooks(&self) -> &HooksConfig {
        &self.hooks
    }

    #[cfg(feature = "max")]
    pub fn ai(&self) -> &AiConfig {
        &self.ai
    }

    /// Save configuration to `~/.agent-hand/config.toml`.
    pub fn save(&self) -> Result<()> {
        let dir = Storage::get_agent_hand_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let toml = toml::to_string_pretty(self)
            .map_err(|e| crate::Error::InvalidInput(format!("TOML serialize: {e}")))?;
        std::fs::write(&path, toml)?;
        // Remove legacy config.json so it doesn't shadow the TOML on next load
        let legacy = dir.join("config.json");
        if legacy.exists() {
            let _ = std::fs::remove_file(legacy);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySpec {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone)]
pub struct KeyBindings {
    bindings: HashMap<&'static str, Vec<KeySpec>>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut kb = Self {
            bindings: HashMap::new(),
        };

        kb.bindings.insert(
            "quit",
            vec![
                KeySpec {
                    code: KeyCode::Char('q'),
                    modifiers: KeyModifiers::NONE,
                },
                KeySpec {
                    code: KeyCode::Char('Q'),
                    modifiers: KeyModifiers::NONE,
                },
                KeySpec {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                },
            ],
        );
        kb.bindings.insert(
            "up",
            vec![
                KeySpec {
                    code: KeyCode::Up,
                    modifiers: KeyModifiers::NONE,
                },
                KeySpec {
                    code: KeyCode::Char('k'),
                    modifiers: KeyModifiers::NONE,
                },
            ],
        );
        kb.bindings.insert(
            "down",
            vec![
                KeySpec {
                    code: KeyCode::Down,
                    modifiers: KeyModifiers::NONE,
                },
                KeySpec {
                    code: KeyCode::Char('j'),
                    modifiers: KeyModifiers::NONE,
                },
            ],
        );

        kb.bindings.insert(
            "select",
            vec![KeySpec {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "collapse",
            vec![KeySpec {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "expand",
            vec![KeySpec {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "toggle_group",
            vec![KeySpec {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
            }],
        );

        kb.bindings.insert(
            "start",
            vec![KeySpec {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "stop",
            vec![KeySpec {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "refresh",
            vec![KeySpec {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
            }],
        );
        kb.bindings.insert(
            "rename",
            vec![KeySpec {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "new_session",
            vec![KeySpec {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "delete",
            vec![KeySpec {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "fork",
            vec![KeySpec {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "create_group",
            vec![KeySpec {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "move",
            vec![KeySpec {
                code: KeyCode::Char('m'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "tag",
            vec![KeySpec {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "preview_refresh",
            vec![KeySpec {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "search",
            vec![KeySpec {
                code: KeyCode::Char('/'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "help",
            vec![KeySpec {
                code: KeyCode::Char('?'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "jump_priority",
            vec![KeySpec {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
            }],
        );
        kb.bindings.insert(
            "restart",
            vec![KeySpec {
                code: KeyCode::Char('R'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "boost",
            vec![KeySpec {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "summarize",
            vec![KeySpec {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::NONE,
            }],
        );
        kb.bindings.insert(
            "half_page_down",
            vec![KeySpec {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
            }],
        );
        kb.bindings.insert(
            "half_page_up",
            vec![KeySpec {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
            }],
        );
        kb.bindings.insert(
            "settings",
            vec![KeySpec {
                code: KeyCode::Char(','),
                modifiers: KeyModifiers::NONE,
            }],
        );

        kb
    }
}

impl KeyBindings {
    pub async fn load_or_default() -> Self {
        let mut kb = Self::default();
        let Ok(Some(cfg)) = ConfigFile::load().await else {
            return kb;
        };

        for (action, spec) in cfg.keybindings {
            let mut parsed = Vec::new();
            for s in spec.into_vec() {
                if let Some(k) = parse_key_spec(&s) {
                    parsed.push(k);
                }
            }
            if !parsed.is_empty() {
                if let Some(slot) = kb.bindings.get_mut(action.as_str()) {
                    *slot = parsed;
                }
            }
        }

        kb
    }

    pub fn matches(&self, action: &'static str, code: &KeyCode, modifiers: KeyModifiers) -> bool {
        self.bindings.get(action).is_some_and(|v| {
            v.iter()
                .any(|k| &k.code == code && k.modifiers == modifiers)
        })
    }
}

fn parse_key_spec(s: &str) -> Option<KeySpec> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let mut modifiers = KeyModifiers::NONE;
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    let (mods, key_part) = if parts.len() >= 2 {
        (&parts[..parts.len() - 1], parts[parts.len() - 1])
    } else {
        (&[][..], parts[0])
    };

    for m in mods {
        match m.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => return None,
        }
    }

    let key_part_trim = key_part.trim();
    let lower = key_part_trim.to_lowercase();

    let code = match lower.as_str() {
        "enter" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "backspace" => KeyCode::Backspace,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        _ => {
            // Single-character fallback (keeps case for e.g. "R")
            if key_part_trim.chars().count() == 1 {
                KeyCode::Char(key_part_trim.chars().next()?)
            } else {
                return None;
            }
        }
    };

    Some(KeySpec { code, modifiers })
}

pub fn parse_tmux_key(s: &str) -> Option<String> {
    let raw = s.trim();
    if raw.is_empty() {
        return None;
    }

    // Accept native tmux notation directly.
    if raw.starts_with("C-") || raw.starts_with("M-") {
        return Some(escape_tmux_key(raw));
    }

    // Accept human-friendly notation: Ctrl+g / Alt+g
    let parts: Vec<&str> = raw.split('+').map(|p| p.trim()).collect();
    if parts.len() >= 2 {
        let key = parts[parts.len() - 1];
        let mods = &parts[..parts.len() - 1];
        if key.chars().count() != 1 {
            // Allow some named keys
            let lower = key.to_lowercase();
            return match lower.as_str() {
                "enter" => Some("Enter".to_string()),
                "esc" | "escape" => Some("Escape".to_string()),
                "tab" => Some("Tab".to_string()),
                _ => None,
            };
        }

        let ch = key.chars().next()?;
        let mut out_prefix: Option<&'static str> = None;
        for m in mods {
            match m.to_lowercase().as_str() {
                "ctrl" | "control" => out_prefix = Some("C-"),
                "alt" => out_prefix = Some("M-"),
                "shift" => {}
                _ => return None,
            }
        }
        if let Some(p) = out_prefix {
            return Some(escape_tmux_key(&format!("{p}{ch}")));
        }
    }

    // Single character
    if raw.chars().count() == 1 {
        return Some(escape_tmux_key(raw));
    }

    // Named keys (pass through; tmux will accept or ignore)
    let lower = raw.to_lowercase();
    match lower.as_str() {
        "enter" => Some("Enter".to_string()),
        "esc" | "escape" => Some("Escape".to_string()),
        "tab" => Some("Tab".to_string()),
        _ => None,
    }
}

fn escape_tmux_key(s: &str) -> String {
    // tmux treats `;` as a command separator in its command language, so it must be escaped.
    let mut out = String::with_capacity(s.len());
    let mut prev_backslash = false;
    for ch in s.chars() {
        if ch == ';' && !prev_backslash {
            out.push('\\');
        }
        out.push(ch);
        prev_backslash = ch == '\\';
    }
    out
}
