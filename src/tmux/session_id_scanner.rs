use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio::sync::RwLock;

use super::detector::Tool;

/// A session that needs scanning for CLI session ID.
#[derive(Debug, Clone)]
pub struct ScanTarget {
    /// The tmux session name (e.g. "agentdeck_rs_mychat_abc123")
    pub tmux_session_name: String,
    /// Current detected tool type
    pub tool: Tool,
    /// Project path for this session
    pub project_path: PathBuf,
    /// Whether we already have a session ID (skip scanning)
    pub has_session_id: bool,
}

/// Result of scanning a single session.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// The tmux session name this result is for
    pub tmux_session_name: String,
    /// Detected tool type (may upgrade Shell → Claude/Codex/etc.)
    pub detected_tool: Option<Tool>,
    /// Detected CLI session ID
    pub detected_session_id: Option<String>,
}

/// Shared scanner state, updated by background task, read by UI thread.
#[derive(Debug, Default)]
pub struct ScanState {
    /// Targets to scan on next iteration (written by UI, read by scanner)
    pub targets: Vec<ScanTarget>,
    /// Results from last scan (written by scanner, read by UI)
    pub results: Vec<ScanResult>,
    /// Whether a scan is currently running
    pub is_scanning: bool,
    /// Last completed scan timestamp
    pub last_scan: Option<Instant>,
}

pub type SharedScanState = Arc<RwLock<ScanState>>;

/// Spawn a background task that periodically scans sessions for CLI session IDs.
///
/// Runs every 7 seconds, checks process trees and config files.
pub fn spawn_session_id_scanner(
    state: SharedScanState,
    server_name: String,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Initial delay: let the app start up before first scan
        tokio::time::sleep(Duration::from_secs(3)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(7));

        loop {
            interval.tick().await;
            perform_scan(&state, &server_name).await;
        }
    })
}

/// Perform a single scan iteration.
async fn perform_scan(state: &SharedScanState, server_name: &str) {
    // Read targets (set by UI thread)
    let targets = {
        let mut guard = state.write().await;
        guard.is_scanning = true;
        std::mem::take(&mut guard.targets)
    };

    if targets.is_empty() {
        let mut guard = state.write().await;
        guard.is_scanning = false;
        guard.last_scan = Some(Instant::now());
        return;
    }

    // Get pane PIDs for all tmux sessions on our server
    let pane_pids = super::ptmx::get_tmux_pane_pids(server_name).await;
    let pane_pid_map: HashMap<String, u32> = pane_pids.into_iter().collect();

    let mut results = Vec::new();

    for target in &targets {
        let pane_pid = match pane_pid_map.get(&target.tmux_session_name) {
            Some(pid) => *pid,
            None => continue, // Session not running in tmux
        };

        // Strategy 1 & 3: Walk process tree, detect tool + session ID from args
        let (detected_tool, detected_id) =
            detect_from_process_tree(pane_pid, target.tool).await;

        // Strategy 2: If Claude detected but no ID from args, try project files
        let final_id = if detected_id.is_some() {
            detected_id
        } else {
            let tool = detected_tool.unwrap_or(target.tool);
            if tool == Tool::Claude {
                detect_from_claude_project_files(&target.project_path).await
            } else {
                None
            }
        };

        if detected_tool.is_some() || final_id.is_some() {
            results.push(ScanResult {
                tmux_session_name: target.tmux_session_name.clone(),
                detected_tool,
                detected_session_id: final_id,
            });
        }
    }

    // Write results
    {
        let mut guard = state.write().await;
        guard.results = results;
        guard.is_scanning = false;
        guard.last_scan = Some(Instant::now());
    }
}

/// Walk the process tree from a pane PID, detect tool type and session ID.
async fn detect_from_process_tree(pane_pid: u32, current_tool: Tool) -> (Option<Tool>, Option<String>) {
    let tree = super::ptmx::collect_process_tree(pane_pid).await;

    let mut detected_tool: Option<Tool> = None;
    let mut detected_id: Option<String> = None;

    for pid in &tree {
        let args = match get_process_args(*pid).await {
            Some(a) => a,
            None => continue,
        };

        // Tool type detection (upgrade Shell → actual tool)
        if current_tool == Tool::Shell && detected_tool.is_none() {
            if let Some(tool) = detect_tool_from_args(&args) {
                detected_tool = Some(tool);
            }
        }

        // Session ID detection
        let tool_for_parse = detected_tool.unwrap_or(current_tool);
        if detected_id.is_none() {
            if let Some(id) = parse_session_id_from_args(&args, tool_for_parse) {
                detected_id = Some(id);
            }
        }

        if detected_tool.is_some() && detected_id.is_some() {
            break;
        }
    }

    (detected_tool, detected_id)
}

/// Get the command-line arguments for a PID.
async fn get_process_args(pid: u32) -> Option<String> {
    let out = Command::new("ps")
        .args(["-o", "args=", "-p", &pid.to_string()])
        .output()
        .await
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Detect which tool is running from process arguments.
fn detect_tool_from_args(args: &str) -> Option<Tool> {
    // Look for tool binary names in the command line
    let lower = args.to_lowercase();

    // Check for specific tool binaries (match basename, not paths containing the name)
    // e.g. "/usr/local/bin/claude --resume abc" or "claude code ..."
    if is_tool_binary(&lower, "claude") {
        return Some(Tool::Claude);
    }
    if is_tool_binary(&lower, "codex") {
        return Some(Tool::Codex);
    }
    if is_tool_binary(&lower, "gemini") {
        return Some(Tool::Gemini);
    }
    if is_tool_binary(&lower, "opencode") {
        return Some(Tool::OpenCode);
    }

    None
}

/// Check if the args string contains a tool binary name as a command.
fn is_tool_binary(lower_args: &str, tool_name: &str) -> bool {
    // Match: tool_name at start, or preceded by / or space
    // This avoids false positives like "/path/not-claude-thing"
    for part in lower_args.split_whitespace() {
        let basename = part.rsplit('/').next().unwrap_or(part);
        if basename == tool_name {
            return true;
        }
    }
    false
}

/// Parse a CLI session ID from command-line arguments.
/// Delegates to the unified resume_adapter for consistent build/parse rules.
fn parse_session_id_from_args(args: &str, _tool: Tool) -> Option<String> {
    let (_detected_tool, session_id) = super::resume_adapter::parse_resume_args(args);
    session_id
}

/// Delegates to the canonical implementation in resume_adapter.
fn looks_like_session_id(s: &str) -> bool {
    super::resume_adapter::looks_like_session_id(s)
}

/// Try to detect a Claude session ID from project files.
///
/// Scans `~/.claude/projects/<encoded-project-path>/` for `.jsonl` files,
/// whose filenames (without extension) are session IDs.
///
/// Returns `Some(id)` only when exactly one matching file exists (medium confidence).
/// When multiple sessions exist in the same directory, returns `None` to avoid
/// guessing the wrong session.
async fn detect_from_claude_project_files(project_path: &PathBuf) -> Option<String> {
    let project_dir = derive_claude_project_dir(project_path)?;

    if !project_dir.exists() {
        return None;
    }

    let mut entries = match tokio::fs::read_dir(&project_dir).await {
        Ok(e) => e,
        Err(_) => return None,
    };

    let mut candidates: Vec<String> = Vec::new();

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        if !super::resume_adapter::looks_like_session_id(&stem) {
            continue;
        }

        candidates.push(stem);
    }

    // Only use the file-based fallback when there's exactly one session file.
    // Multiple files → ambiguous, don't guess.
    if candidates.len() == 1 {
        candidates.into_iter().next()
    } else {
        None
    }
}

/// Derive the Claude project directory for a given project path.
///
/// Claude stores project data at `~/.claude/projects/<encoded-path>/`
/// where the path is the absolute project path with `/` replaced by `-`.
fn derive_claude_project_dir(project_path: &PathBuf) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let path_str = project_path.to_str()?;

    // Claude encodes the path: absolute path with slashes → hyphens
    // e.g. /Users/user/project → -Users-user-project
    let encoded = path_str.replace('/', "-");

    Some(home.join(".claude").join("projects").join(encoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_session_id() {
        assert!(looks_like_session_id("abc12345"));
        assert!(looks_like_session_id("550e8400-e29b-41d4-a716-446655440000"));
        assert!(looks_like_session_id("a1b2c3d4e5f6"));
        assert!(!looks_like_session_id(""));
        assert!(!looks_like_session_id("short"));
        assert!(!looks_like_session_id("--flag"));
        assert!(!looks_like_session_id("-verbose"));
    }

    #[test]
    fn test_detect_tool_from_args() {
        assert_eq!(
            detect_tool_from_args("/usr/local/bin/claude --resume abc"),
            Some(Tool::Claude)
        );
        assert_eq!(
            detect_tool_from_args("codex --help"),
            Some(Tool::Codex)
        );
        assert_eq!(
            detect_tool_from_args("node server.js"),
            None
        );
        // Should not match partial names
        assert_eq!(
            detect_tool_from_args("/path/to/not-claude-thing"),
            None
        );
    }

    #[test]
    fn test_parse_session_id_claude() {
        assert_eq!(
            parse_session_id_from_args("claude --resume 550e8400-e29b-41d4-a716-446655440000", Tool::Claude),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(
            parse_session_id_from_args("claude -r abc12345def", Tool::Claude),
            Some("abc12345def".to_string())
        );
        assert_eq!(
            parse_session_id_from_args("claude --continue mySessionId123", Tool::Claude),
            Some("mySessionId123".to_string())
        );
        assert_eq!(
            parse_session_id_from_args("claude --resume=abc12345def", Tool::Claude),
            Some("abc12345def".to_string())
        );
        // No session ID flag
        assert_eq!(
            parse_session_id_from_args("claude chat", Tool::Claude),
            None
        );
    }

    #[test]
    fn test_parse_session_id_codex() {
        assert_eq!(
            parse_session_id_from_args("codex --resume session123abc", Tool::Codex),
            Some("session123abc".to_string())
        );
    }

    #[test]
    fn test_is_tool_binary() {
        assert!(is_tool_binary("claude --resume abc", "claude"));
        assert!(is_tool_binary("/usr/bin/claude --help", "claude"));
        assert!(!is_tool_binary("not-claude-thing run", "claude"));
        assert!(!is_tool_binary("myclaudeapp start", "claude"));
    }

    #[test]
    fn test_derive_claude_project_dir() {
        let path = PathBuf::from("/Users/test/project");
        let result = derive_claude_project_dir(&path);
        assert!(result.is_some());
        let dir = result.unwrap();
        assert!(dir.to_str().unwrap().contains("-Users-test-project"));
    }
}
