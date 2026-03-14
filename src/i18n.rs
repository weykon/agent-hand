// Internationalization support for Agent Hand

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    English,
    Chinese,
}

impl Default for Language {
    fn default() -> Self {
        Language::English
    }
}

impl Language {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "zh" | "chinese" | "中文" => Language::Chinese,
            _ => Language::English,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Chinese => "中文",
        }
    }

    /// Auto-detect language from system locale environment variables.
    /// Returns Chinese for `zh*` locales, English for everything else.
    pub fn detect() -> Self {
        for var in &["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(val) = std::env::var(var) {
                let lower = val.to_lowercase();
                if lower.starts_with("zh") {
                    return Language::Chinese;
                }
                // If the var is set to a non-zh value, use English
                if !lower.is_empty() {
                    return Language::English;
                }
            }
        }
        Language::English
    }

    pub fn is_zh(&self) -> bool {
        matches!(self, Language::Chinese)
    }
}

/// Resolve CLI language: config preference → system locale → English.
///
/// Uses sync I/O (config files are small, CLI runs before TUI).
pub fn cli_lang() -> Language {
    // 1. Check config file for explicit language preference
    if let Some(lang_str) = read_config_language() {
        return Language::from_str(&lang_str);
    }
    // 2. Fall back to system locale detection
    Language::detect()
}

/// Read the `language` field from config.toml/config.json (sync, lightweight).
fn read_config_language() -> Option<String> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join(".agent-hand").join("config.toml"),
        home.join(".config").join("agent-hand").join("config.toml"),
        home.join(".agent-hand").join("config.json"),
        home.join(".config").join("agent-hand").join("config.json"),
    ];

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            // Extract just the language field — avoid pulling in full ConfigFile
            if ext == "toml" {
                if let Ok(table) = content.parse::<toml::Table>() {
                    if let Some(toml::Value::String(s)) = table.get("language") {
                        return Some(s.clone());
                    }
                }
            } else {
                if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content) {
                    if let Some(serde_json::Value::String(s)) = map.get("language") {
                        return Some(s.clone());
                    }
                }
            }
        }
    }
    None
}

/// Ergonomic inline translation macro for CLI strings.
///
/// Usage: `t!(lang, "English text", "中文文本")`
#[macro_export]
macro_rules! t {
    ($lang:expr, $en:expr, $zh:expr) => {
        if $lang.is_zh() { $zh } else { $en }
    };
}

// Translation keys and functions
pub trait Translate {
    fn t(&self, lang: Language) -> &'static str;
}

// Common UI strings
pub mod ui {
    use super::*;

    pub struct Title;
    impl Translate for Title {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "🦀 Agent Hand",
                Language::Chinese => "🦀 Agent Hand 智能助手",
            }
        }
    }

    pub struct HelpHint;
    impl Translate for HelpHint {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Press ? for Help",
                Language::Chinese => "按 ? 查看帮助",
            }
        }
    }

    pub struct Welcome;
    impl Translate for Welcome {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Welcome to Agent Hand!",
                Language::Chinese => "欢迎使用 Agent Hand！",
            }
        }
    }

    pub struct Settings;
    impl Translate for Settings {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Settings",
                Language::Chinese => "设置",
            }
        }
    }

    pub struct LanguageSetting;
    impl Translate for LanguageSetting {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Language",
                Language::Chinese => "语言",
            }
        }
    }
}

// Help modal strings
pub mod help {
    use super::*;

    pub struct Navigation;
    impl Translate for Navigation {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Navigation",
                Language::Chinese => "导航",
            }
        }
    }

    pub struct SessionActions;
    impl Translate for SessionActions {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Session Actions",
                Language::Chinese => "会话操作",
            }
        }
    }

    pub struct GroupActions;
    impl Translate for GroupActions {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Group Actions",
                Language::Chinese => "分组操作",
            }
        }
    }

    pub struct MoveUp;
    impl Translate for MoveUp {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Move up",
                Language::Chinese => "向上移动",
            }
        }
    }

    pub struct MoveDown;
    impl Translate for MoveDown {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Move down",
                Language::Chinese => "向下移动",
            }
        }
    }

    pub struct ToggleGroup;
    impl Translate for ToggleGroup {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Toggle group",
                Language::Chinese => "展开/折叠分组",
            }
        }
    }

    pub struct Search;
    impl Translate for Search {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Search",
                Language::Chinese => "搜索",
            }
        }
    }

    pub struct Attach;
    impl Translate for Attach {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Attach to session",
                Language::Chinese => "连接到会话",
            }
        }
    }

    pub struct Start;
    impl Translate for Start {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Start session",
                Language::Chinese => "启动会话",
            }
        }
    }

    pub struct Stop;
    impl Translate for Stop {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Stop session",
                Language::Chinese => "停止会话",
            }
        }
    }

    pub struct Edit;
    impl Translate for Edit {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Edit session",
                Language::Chinese => "编辑会话",
            }
        }
    }

    pub struct Restart;
    impl Translate for Restart {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Restart session",
                Language::Chinese => "重启会话",
            }
        }
    }

    pub struct MoveToGroup;
    impl Translate for MoveToGroup {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Move to group",
                Language::Chinese => "移动到分组",
            }
        }
    }

    pub struct Fork;
    impl Translate for Fork {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Fork session",
                Language::Chinese => "复制会话",
            }
        }
    }

    pub struct Delete;
    impl Translate for Delete {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Delete session",
                Language::Chinese => "删除会话",
            }
        }
    }
}

// First-time onboarding strings
pub mod onboarding {
    use super::*;

    pub struct WelcomeTitle;
    impl Translate for WelcomeTitle {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Welcome to Agent Hand!",
                Language::Chinese => "欢迎使用 Agent Hand！",
            }
        }
    }

    pub struct WelcomeMessage;
    impl Translate for WelcomeMessage {
        fn t(&self, lang: Language) -> &'static str {
            match lang {
                Language::English => "Agent Hand helps you manage multiple AI agent sessions efficiently.\n\nKey Features:\n• Organize sessions in groups\n• Start, stop, and attach to sessions\n• Real-time collaboration (Pro)\n• Session sharing and viewer mode (Pro)\n\nQuick Start:\n• Press 's' to start a new session\n• Use ↑/↓ or j/k to navigate\n• Press Enter to attach to a session\n• Press '?' anytime for help\n\nPress Enter to continue...",
                Language::Chinese => "Agent Hand 帮助您高效管理多个 AI 智能体会话。\n\n主要功能：\n• 分组管理会话\n• 启动、停止和连接会话\n• 实时协作（Pro）\n• 会话分享和观察者模式（Pro）\n\n快速开始：\n• 按 's' 创建新会话\n• 使用 ↑/↓ 或 j/k 导航\n• 按 Enter 连接到会话\n• 随时按 '?' 查看帮助\n\n按 Enter 继续...",
            }
        }
    }
}

