use regex::Regex;
use std::fmt;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Claude,
    Gemini,
    OpenCode,
    Codex,
    Shell,
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tool::Claude => write!(f, "claude"),
            Tool::Gemini => write!(f, "gemini"),
            Tool::OpenCode => write!(f, "opencode"),
            Tool::Codex => write!(f, "codex"),
            Tool::Shell => write!(f, "shell"),
        }
    }
}

impl Default for Tool {
    fn default() -> Self {
        Self::Shell
    }
}

impl Tool {
    pub fn from_command(cmd: &str) -> Self {
        let cmd_lower = cmd.to_lowercase();
        if cmd_lower.contains("claude") {
            Self::Claude
        } else if cmd_lower.contains("gemini") {
            Self::Gemini
        } else if cmd_lower.contains("opencode") || cmd_lower.contains("open-code") {
            Self::OpenCode
        } else if cmd_lower.contains("codex") || cmd_lower.contains("copilot") {
            Self::Codex
        } else {
            Self::Shell
        }
    }
}

/// Prompt detector - identifies when AI agents are waiting for input
/// Uses unified pattern matching across all tools (Claude, Copilot, OpenCode, etc.)
pub struct PromptDetector;

impl PromptDetector {
    pub fn new(_tool: Tool) -> Self {
        // Tool parameter kept for API compatibility but no longer used for dispatch
        Self
    }

    /// Check if terminal content shows the agent is currently busy (running/thinking).
    pub fn is_busy(&self, content: &str) -> bool {
        let lines = get_last_lines(content, 15);
        let recent = strip_ansi(&lines.join("\n")).to_lowercase();

        // Busy indicators across all tools
        let busy_indicators = [
            "esc to interrupt",
            "(esc to interrupt)",
            "· esc to interrupt",
        ];
        if busy_indicators.iter().any(|m| recent.contains(m)) {
            return true;
        }

        // Spinner characters (Claude braille dots)
        let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let last_3 = if lines.len() > 3 {
            &lines[lines.len() - 3..]
        } else {
            &lines[..]
        };
        for line in last_3 {
            for c in &spinner_chars {
                if line.contains(*c) {
                    return true;
                }
            }
        }

        // OpenCode/Copilot progress dots
        let dots = recent.chars().filter(|&c| c == '⬝').count();
        if dots >= 3 {
            return true;
        }

        // Thinking/connecting indicators
        if (recent.contains("thinking") && recent.contains("tokens"))
            || (recent.contains("connecting") && recent.contains("tokens"))
        {
            return true;
        }

        false
    }

    /// Check if terminal content shows a prompt waiting for user input.
    pub fn has_prompt(&self, content: &str) -> bool {
        let lines = get_last_lines(content, 15);
        let recent = strip_ansi(&lines.join("\n"));
        let recent_lower = recent.to_lowercase();

        // Blocking confirmation prompts (all tools)
        let blocking_prompts = [
            // Claude permission dialogs
            "no, and tell claude what to do differently",
            "yes, allow once",
            "yes, allow always",
            "allow once",
            "allow always",
            "do you want to create",
            "do you want to run this command",
            "do you trust the files in this folder",
            "allow this mcp server",
            "run this command?",
            "execute this?",
            // Copilot/Codex
            "confirm with number keys",
            // Generic y/n prompts
            "continue?",
            "proceed?",
            "(y/n)",
            "[y/n]",
            "(yes/no)",
            "[yes/no]",
            "approve this plan?",
            "execute plan?",
            "enter to continue",
        ];

        if blocking_prompts.iter().any(|p| recent_lower.contains(p)) {
            return true;
        }

        // Selection prompts with arrow indicator (Claude/Copilot numbered options)
        let selection_indicators = ["❯ yes", "❯ no", "❯ allow", "❯ 1.", "❯ 2.", "❯ 3."];
        if selection_indicators
            .iter()
            .any(|p| recent_lower.contains(p))
        {
            return true;
        }

        // Box-drawing prompts (Claude dialog boxes)
        let box_prompts = ["│ do you want", "│ would you like", "│ allow"];
        if box_prompts.iter().any(|p| recent_lower.contains(p)) {
            return true;
        }

        false
    }
}

/// Strip ANSI escape codes from content
pub fn strip_ansi(content: &str) -> String {
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    let re =
        ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07").unwrap());
    re.replace_all(content, "").to_string()
}

/// Get last N non-empty lines from content
fn get_last_lines(content: &str, n: usize) -> Vec<String> {
    content
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(n)
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_busy_detection() {
        let detector = PromptDetector::new(Tool::Shell);
        // Spinner and "esc to interrupt" = busy
        assert!(detector.is_busy("Thinking… (45s · 1234 tokens · esc to interrupt)"));
        assert!(detector.is_busy("⠋ Processing..."));
        // Progress dots = busy
        assert!(detector.is_busy("⬝⬝⬝⬝⬝⬝⬝⬝"));
        // Prompts should not be busy
        assert!(!detector.has_prompt("Thinking… (45s · 1234 tokens · esc to interrupt)"));
        assert!(!detector.has_prompt("⠋ Processing..."));
    }

    #[test]
    fn test_waiting_detection() {
        let detector = PromptDetector::new(Tool::Shell);
        // Claude permission dialogs
        assert!(detector.has_prompt("Yes, allow once\nNo, and tell Claude what to do differently"));
        assert!(
            detector.has_prompt("Do you want to create explore_db.py?\n❯ 1. Yes\nEsc to cancel")
        );
        // Copilot confirmation
        assert!(detector.has_prompt("Confirm with number keys or ↑↓ keys and Enter"));
        assert!(detector.has_prompt("Do you want to run this command?\n❯ 1. Yes"));
        // y/n prompts
        assert!(detector.has_prompt("Continue? (y/n)"));
        // Plain prompts should NOT be waiting
        assert!(!detector.has_prompt(">"));
        assert!(!detector.has_prompt("> "));
    }

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[32mGreen text\x1b[0m";
        assert_eq!(strip_ansi(input), "Green text");
    }
}
