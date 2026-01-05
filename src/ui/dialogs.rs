use std::path::PathBuf;

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewSessionField {
    Path,
    Title,
    Group,
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
    pub group_path: String,
    pub tool: NewSessionTool,
    pub command: String,
    pub field: NewSessionField,

    pub path_suggestions: Vec<String>,
    pub path_suggestions_idx: usize,
    pub path_suggestions_visible: bool,

    // Debounced auto-suggest for the Path field.
    pub path_dirty: bool,
    pub path_last_edit: std::time::Instant,
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
pub struct MoveGroupDialog {
    pub session_id: String,
    pub title: String,
    pub group_path: String,
}

#[derive(Debug, Clone)]
pub struct RenameGroupDialog {
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    NewSession(NewSessionDialog),
    DeleteConfirm(DeleteConfirmDialog),
    MCP(MCPDialog),
    Fork(ForkDialog),
    MoveGroup(MoveGroupDialog),
    RenameGroup(RenameGroupDialog),
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
            group_path: String::new(),
            tool: NewSessionTool::Claude,
            command: "claude".to_string(),
            field: NewSessionField::Path,
            path_suggestions: Vec::new(),
            path_suggestions_idx: 0,
            path_suggestions_visible: false,
            path_dirty: false,
            path_last_edit: std::time::Instant::now(),
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
                pos = idx + 1;
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
            self.path = self.path_suggestions[0].clone();
            self.clear_path_suggestions();
        }
    }

    pub fn update_path_suggestions(&mut self) {
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
        if !base_has_slash && self.path.starts_with('~') {
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
