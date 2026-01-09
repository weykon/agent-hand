use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyModifiers};
use serde::Deserialize;
use tokio::fs;

use crate::error::Result;
use crate::session::Storage;

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfigFile {
    #[serde(default)]
    keybindings: HashMap<String, OneOrMany>,

    #[serde(default)]
    tmux: TmuxKeys,

    #[serde(default)]
    analytics: AnalyticsConfig,

    #[serde(default)]
    input_logging: InputLoggingConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TmuxKeys {
    #[serde(default)]
    switcher: Option<String>,
    #[serde(default)]
    detach: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalyticsConfig {
    #[serde(default = "default_analytics_enabled")]
    pub enabled: bool,
}

fn default_analytics_enabled() -> bool {
    false
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

/// Input logging config (requires `input-logging` feature at compile time)
#[derive(Debug, Clone, Deserialize)]
pub struct InputLoggingConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Compress logs larger than this size (in MB). Default: 10MB
    #[serde(default = "default_compress_threshold_mb")]
    pub compress_threshold_mb: u64,
    /// Maximum number of zip archives to keep. Default: 100
    #[serde(default = "default_max_archives")]
    pub max_archives: usize,
}

fn default_compress_threshold_mb() -> u64 {
    10
}

fn default_max_archives() -> usize {
    100
}

impl Default for InputLoggingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            compress_threshold_mb: 10,
            max_archives: 100,
        }
    }
}

impl InputLoggingConfig {
    pub fn compress_threshold_bytes(&self) -> u64 {
        self.compress_threshold_mb * 1024 * 1024
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
        let xdg_dir = dirs::home_dir()
            .map(|h| h.join(".config").join("agent-hand"));

        let candidates: Vec<std::path::PathBuf> = [
            Some(agent_hand_dir.join("config.json")),
            Some(agent_hand_dir.join("config.toml")),
            xdg_dir.as_ref().map(|d| d.join("config.toml")),
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

    pub fn analytics_enabled(&self) -> bool {
        self.analytics.enabled
    }

    /// Check if input logging is enabled in config
    /// Note: Also requires `input-logging` feature at compile time
    pub fn input_logging_enabled(&self) -> bool {
        self.input_logging.enabled
    }

    /// Get input logging config
    pub fn input_logging(&self) -> &InputLoggingConfig {
        &self.input_logging
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
            "restart",
            vec![KeySpec {
                code: KeyCode::Char('R'),
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
        self.bindings
            .get(action)
            .is_some_and(|v| v.iter().any(|k| &k.code == code && k.modifiers == modifiers))
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
