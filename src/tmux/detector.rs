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
        } else if cmd_lower.contains("codex") {
            Self::Codex
        } else {
            Self::Shell
        }
    }
}

/// Prompt detector - identifies when AI agents are waiting for input
/// Based on Claude Squad's implementation with enhancements
pub struct PromptDetector {
    tool: Tool,
}

impl PromptDetector {
    pub fn new(tool: Tool) -> Self {
        Self { tool }
    }

    /// Check if terminal content shows the agent is currently busy.
    pub fn is_busy(&self, content: &str) -> bool {
        match self.tool {
            Tool::Claude => self.is_claude_busy(content),
            Tool::Gemini | Tool::OpenCode | Tool::Codex | Tool::Shell => {
                self.is_generic_busy(content)
            }
        }
    }

    /// Check if terminal content shows a prompt waiting for input.
    pub fn has_prompt(&self, content: &str) -> bool {
        match self.tool {
            Tool::Claude => self.has_claude_prompt(content),
            Tool::Gemini => self.has_gemini_prompt(content),
            Tool::OpenCode => self.has_opencode_prompt(content),
            Tool::Codex => self.has_codex_prompt(content),
            Tool::Shell => self.has_shell_prompt(content),
        }
    }

    /// Detect Claude Code prompt states
    ///
    /// States:
    /// - BUSY: "esc to interrupt" with spinner (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
    /// - WAITING (normal): Permission dialogs with Yes/No
    /// - WAITING (skip-permissions): Just ">" prompt
    /// - THINKING: Extended reasoning with "think" keywords
    fn has_claude_prompt(&self, content: &str) -> bool {
        let lines = get_last_lines(content, 15);
        let recent = lines.join("\n");

        // BUSY indicators - if present, Claude is NOT waiting
        if self.is_claude_busy(content) {
            return false;
        }

        // WAITING indicators - Permission prompts
        let permission_prompts = [
            "No, and tell Claude what to do differently",
            "Yes, allow once",
            "Yes, allow always",
            "Allow once",
            "Allow always",
            "Do you want to create",
            "│ Do you want",
            "│ Would you like",
            "│ Allow",
            "❯ Yes",
            "❯ No",
            "❯ Allow",
            "❯ 1.",
            "❯ 2.",
            "❯ 3.",
            "Do you trust the files in this folder?",
            "Allow this MCP server",
            "Run this command?",
            "Execute this?",
        ];

        for prompt in &permission_prompts {
            if content.contains(prompt) {
                return true;
            }
        }

        // Question prompts
        let question_prompts = [
            "Continue?",
            "Proceed?",
            "(Y/n)",
            "(y/N)",
            "[Y/n]",
            "[y/N]",
            "(yes/no)",
            "[yes/no]",
            "Approve this plan?",
            "Execute plan?",
        ];

        for prompt in &question_prompts {
            if recent.contains(prompt) {
                return true;
            }
        }

        false
    }

    fn is_generic_busy(&self, content: &str) -> bool {
        let lines = get_last_lines(content, 15);
        let recent = strip_ansi(&lines.join("\n")).to_lowercase();

        // Common "busy" markers across tools.
        let busy_indicators = [
            "(esc to cancel)",
            "esc to cancel",
            "esc to interrupt",
            "(esc to interrupt)",
            "esc to stop",
            "(esc to stop)",
            "esc interrupt",
        ];
        if busy_indicators.iter().any(|m| recent.contains(m)) {
            return true;
        }

        // OpenCode/Copilot progress indicator (e.g. ⬝⬝⬝⬝⬝⬝)
        let dots = recent.chars().filter(|&c| c == '⬝').count();
        dots >= 3
    }

    fn is_claude_busy(&self, content: &str) -> bool {
        let lines = get_last_lines(content, 15);
        let recent = lines.join("\n");
        let recent_lower = recent.to_lowercase();

        // BUSY indicators
        let busy_indicators = [
            "esc to interrupt",
            "(esc to interrupt)",
            "· esc to interrupt",
        ];
        if busy_indicators.iter().any(|m| recent_lower.contains(m)) {
            return true;
        }

        // Check for spinner characters (braille dots from cli-spinners)
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

        // thinking/connecting indicators
        (recent_lower.contains("thinking") && recent_lower.contains("tokens"))
            || (recent_lower.contains("connecting") && recent_lower.contains("tokens"))
    }

    fn has_gemini_prompt(&self, content: &str) -> bool {
        let recent = strip_ansi(&get_last_lines(content, 15).join("\n")).to_lowercase();
        let prompts = [
            "yes, allow once",
            "yes, allow always",
            "allow once",
            "allow always",
            "continue?",
            "proceed?",
            "(y/n)",
            "[y/n]",
            "(yes/no)",
            "[yes/no]",
            "enter to continue",
            "press enter",
        ];
        prompts.iter().any(|p| recent.contains(p))
    }

    fn has_opencode_prompt(&self, content: &str) -> bool {
        let recent = strip_ansi(&get_last_lines(content, 15).join("\n")).to_lowercase();
        if recent.contains("confirm with number keys") {
            return true;
        }

        // OpenCode often shows "press enter" even when it's just idle/ready for input.
        // We only treat explicit blocking prompts as WAITING.
        let prompts = [
            "continue?",
            "proceed?",
            "(y/n)",
            "[y/n]",
            "(yes/no)",
            "[yes/no]",
            "enter to continue",
        ];
        prompts.iter().any(|p| recent.contains(p))
    }

    fn has_codex_prompt(&self, content: &str) -> bool {
        let recent = strip_ansi(&get_last_lines(content, 15).join("\n")).to_lowercase();
        let prompts = [
            "continue?",
            "proceed?",
            "(y/n)",
            "[y/n]",
            "(yes/no)",
            "[yes/no]",
            "enter to continue",
            "press enter",
        ];
        prompts.iter().any(|p| recent.contains(p))
    }

    fn has_shell_prompt(&self, content: &str) -> bool {
        let recent = strip_ansi(&get_last_lines(content, 15).join("\n")).to_lowercase();
        let prompts = [
            "(y/n)",
            "[y/n]",
            "(y/n)",
            "[y/n]",
            "(yes/no)",
            "[yes/no]",
            "continue?",
            "proceed?",
            "enter to continue",
            "press enter",
        ];
        prompts.iter().any(|p| recent.contains(p))
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
    fn test_claude_busy_detection() {
        let detector = PromptDetector::new(Tool::Claude);
        assert!(detector.is_busy("Thinking… (45s · 1234 tokens · esc to interrupt)"));
        assert!(detector.is_busy("⠋ Processing..."));
        assert!(!detector.has_prompt("Thinking… (45s · 1234 tokens · esc to interrupt)"));
        assert!(!detector.has_prompt("⠋ Processing..."));
    }

    #[test]
    fn test_opencode_busy_detection() {
        let detector = PromptDetector::new(Tool::OpenCode);
        assert!(detector.is_busy("Running... (Esc to cancel)"));
        assert!(detector.is_busy("⬝⬝⬝⬝⬝⬝⬝⬝"));
    }

    #[test]
    fn test_claude_waiting_detection() {
        let detector = PromptDetector::new(Tool::Claude);
        assert!(detector.has_prompt("Yes, allow once\nNo, and tell Claude what to do differently"));
        assert!(
            detector.has_prompt("Do you want to create explore_db.py?\n❯ 1. Yes\nEsc to cancel")
        );
        assert!(!detector.has_prompt(">"));
        assert!(!detector.has_prompt("> "));
    }

    #[test]
    fn test_opencode_waiting_detection() {
        let detector = PromptDetector::new(Tool::OpenCode);
        assert!(detector.has_prompt("Confirm with number keys or ↑↓ keys and Enter"));
        assert!(!detector.has_prompt(">"));
    }

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[32mGreen text\x1b[0m";
        assert_eq!(strip_ansi(input), "Green text");
    }
}
