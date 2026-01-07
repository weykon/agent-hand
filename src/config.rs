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
}

impl ConfigFile {
    pub async fn load() -> Result<Option<Self>> {
        let dir = Storage::get_agent_hand_dir()?;
        let path = dir.join("config.json");
        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        let cfg = serde_json::from_str::<Self>(&content)?;
        Ok(Some(cfg))
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
