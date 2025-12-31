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

    /// Check if terminal content shows a prompt waiting for input
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
        let recent_lower = recent.to_lowercase();

        // BUSY indicators - if present, Claude is NOT waiting
        let busy_indicators = [
            "esc to interrupt",
            "(esc to interrupt)",
            "· esc to interrupt",
        ];

        for indicator in &busy_indicators {
            if recent_lower.contains(indicator) {
                return false; // Actively working
            }
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
                    return false; // Spinner = actively processing
                }
            }
        }

        // Check for thinking/connecting indicators
        if recent_lower.contains("thinking") && recent_lower.contains("tokens") {
            return false;
        }
        if recent_lower.contains("connecting") && recent_lower.contains("tokens") {
            return false;
        }

        // WAITING indicators - Permission prompts
        let permission_prompts = [
            "No, and tell Claude what to do differently",
            "Yes, allow once",
            "Yes, allow always",
            "Allow once",
            "Allow always",
            "│ Do you want",
            "│ Would you like",
            "│ Allow",
            "❯ Yes",
            "❯ No",
            "❯ Allow",
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

        // WAITING - Input prompt (skip-permissions mode)
        if let Some(last_line) = lines.last() {
            let cleaned = strip_ansi(last_line);
            let clean = cleaned.trim();
            if clean == ">" || clean == "> " {
                return true;
            }

            // Prompt with partial user input
            if clean.starts_with("> ") && !clean.contains("esc") && clean.len() < 100 {
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

        // Completion indicators + prompt
        let completion_indicators = [
            "Task completed",
            "Done!",
            "Finished",
            "What would you like",
            "What else",
            "Anything else",
            "Let me know if",
        ];

        for indicator in &completion_indicators {
            if recent_lower.contains(&indicator.to_lowercase()) {
                // Check if ">" prompt is nearby
                for line in last_3 {
                    let cleaned = strip_ansi(line);
                    let clean = cleaned.trim();
                    if clean == ">" || clean == "> " {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn has_gemini_prompt(&self, content: &str) -> bool {
        content.contains("Yes, allow once")
            || content.contains("gemini>")
            || has_line_ending_with(content, ">")
    }

    fn has_opencode_prompt(&self, content: &str) -> bool {
        content.contains("Ask anything")
            || content.contains("┃")
            || content.contains("open code")
            || content.contains("Build")
            || content.contains("Plan")
            || has_line_ending_with(content, ">")
    }

    fn has_codex_prompt(&self, content: &str) -> bool {
        content.contains("codex>")
            || content.contains("Continue?")
            || has_line_ending_with(content, ">")
    }

    fn has_shell_prompt(&self, content: &str) -> bool {
        let lines = get_last_lines(content, 5);
        if lines.is_empty() {
            return false;
        }

        // Get last non-empty line
        let last_line = lines
            .iter()
            .rev()
            .find(|l| !l.trim().is_empty())
            .map(|s| s.as_str())
            .unwrap_or("");

        // Common shell prompt endings
        let shell_prompts = ["$ ", "# ", "% ", "❯ ", "➜ ", "> "];
        for prompt in &shell_prompts {
            if last_line.trim_end().ends_with(prompt.trim()) {
                return true;
            }
        }

        // Confirmation prompts
        let confirm_patterns = [
            "(Y/n)",
            "[Y/n]",
            "(y/N)",
            "[y/N]",
            "(yes/no)",
            "[yes/no]",
            "Continue?",
            "Proceed?",
        ];

        let recent = lines.join("\n");
        for pattern in &confirm_patterns {
            if recent.contains(pattern) {
                return true;
            }
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

/// Check if any recent line ends with suffix
fn has_line_ending_with(content: &str, suffix: &str) -> bool {
    let lines = get_last_lines(content, 5);
    for line in lines {
        let trimmed = line.trim();
        if trimmed == suffix || trimmed.ends_with(&format!("{} ", suffix)) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_busy_detection() {
        let detector = PromptDetector::new(Tool::Claude);
        assert!(!detector.has_prompt("Thinking… (45s · 1234 tokens · esc to interrupt)"));
        assert!(!detector.has_prompt("⠋ Processing..."));
    }

    #[test]
    fn test_claude_waiting_detection() {
        let detector = PromptDetector::new(Tool::Claude);
        assert!(detector.has_prompt("Yes, allow once\nNo, and tell Claude what to do differently"));
        assert!(detector.has_prompt(">"));
        assert!(detector.has_prompt("> "));
    }

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[32mGreen text\x1b[0m";
        assert_eq!(strip_ansi(input), "Green text");
    }
}
