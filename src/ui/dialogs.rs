use std::path::PathBuf;

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewSessionField {
    Path,
    Title,
    Tool,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewSessionTool {
    Claude,
    Gemini,
    OpenCode,
    Codex,
    Shell,
    Custom,
}

impl NewSessionTool {
    pub const ALL: [NewSessionTool; 6] = [
        NewSessionTool::Claude,
        NewSessionTool::Gemini,
        NewSessionTool::OpenCode,
        NewSessionTool::Codex,
        NewSessionTool::Shell,
        NewSessionTool::Custom,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            NewSessionTool::Claude => "claude",
            NewSessionTool::Gemini => "gemini",
            NewSessionTool::OpenCode => "opencode",
            NewSessionTool::Codex => "codex",
            NewSessionTool::Shell => "shell",
            NewSessionTool::Custom => "custom",
        }
    }

    pub fn default_command(&self) -> Option<&'static str> {
        match self {
            NewSessionTool::Claude => Some("claude"),
            NewSessionTool::Gemini => Some("gemini"),
            NewSessionTool::OpenCode => Some("opencode"),
            NewSessionTool::Codex => Some("codex"),
            NewSessionTool::Shell => None,
            NewSessionTool::Custom => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewSessionDialog {
    pub path: String,
    pub title: String,
    pub tool: NewSessionTool,
    pub command: String,
    pub field: NewSessionField,

    pub path_suggestions: Vec<String>,
    pub path_suggestions_idx: usize,
    pub path_suggestions_visible: bool,
}

#[derive(Debug, Clone)]
pub struct DeleteConfirmDialog {
    pub session_id: String,
    pub title: String,
    pub kill_tmux: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MCPColumn {
    Attached,
    Available,
}

#[derive(Debug, Clone)]
pub struct MCPDialog {
    pub session_id: String,
    pub project_path: PathBuf,
    pub attached: Vec<String>,
    pub available: Vec<String>,
    pub column: MCPColumn,
    pub attached_idx: usize,
    pub available_idx: usize,
    pub dirty: bool,
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
    pub title: String,
    pub group_path: String,
    pub field: ForkField,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    NewSession(NewSessionDialog),
    DeleteConfirm(DeleteConfirmDialog),
    MCP(MCPDialog),
    Fork(ForkDialog),
}

impl NewSessionDialog {
    pub fn new(default_path: PathBuf) -> Self {
        let title = default_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();

        Self {
            path: default_path.to_string_lossy().to_string(),
            title,
            tool: NewSessionTool::Claude,
            command: "claude".to_string(),
            field: NewSessionField::Path,
            path_suggestions: Vec::new(),
            path_suggestions_idx: 0,
            path_suggestions_visible: false,
        }
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

        // Compute suggestions once
        self.clear_path_suggestions();

        let expanded = Self::expand_home(&self.path);
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

        let mut matches: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if !prefix.is_empty() && !name.starts_with(&prefix) {
                    return None;
                }
                let mut full = dir.join(&name).to_string_lossy().to_string();
                if e.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
                    full.push('/');
                }
                Some(full)
            })
            .collect();

        matches.sort();
        if matches.is_empty() {
            return;
        }

        if matches.len() == 1 {
            self.path = matches[0].clone();
            return;
        }

        // Show suggestion list (do not auto-apply arbitrary choice)
        self.path_suggestions = matches;
        self.path_suggestions_visible = true;
        self.path_suggestions_idx = 0;

        // If the user didn't type a slash and is completing in CWD, keep relative feeling.
        if !base_has_slash && self.path.starts_with("~") {
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
            self.path = sel;
        }
        self.clear_path_suggestions();
    }

    pub fn validate(&self) -> Result<PathBuf> {
        let project_path = Self::expand_home(&self.path);
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
