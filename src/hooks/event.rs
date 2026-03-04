use serde::Deserialize;

/// A hook event received from a CLI tool (Claude Code, Codex, etc.) via JSONL file.
///
/// The hook script running inside the tmux pane receives the tool's event on stdin,
/// enriches it with the tmux session name, and appends a JSON line to the events file.
#[derive(Debug, Clone, Deserialize)]
pub struct HookEvent {
    /// The tmux session name (set by the hook script via tmux display-message)
    pub tmux_session: String,
    /// The parsed event kind
    pub kind: HookEventKind,
    /// Claude Code session_id (for sub-agent tracking)
    #[serde(default)]
    pub session_id: String,
    /// Working directory of the agent
    #[serde(default)]
    pub cwd: String,
    /// Unix timestamp when the event was emitted
    #[serde(default)]
    pub ts: f64,
}

/// Enumeration of hook event kinds we care about.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type")]
pub enum HookEventKind {
    /// Agent started working (user submitted a prompt)
    #[serde(alias = "user_prompt_submit")]
    UserPromptSubmit,

    /// Agent finished its task
    #[serde(alias = "stop")]
    Stop,

    /// Agent needs user input — permission, question, or idle prompt
    #[serde(alias = "notification")]
    Notification {
        #[serde(default)]
        notification_type: String,
    },

    /// Agent is requesting permission to use a tool
    #[serde(alias = "permission_request")]
    PermissionRequest {
        #[serde(default)]
        tool_name: String,
    },

    /// A tool call failed
    #[serde(alias = "tool_failure")]
    ToolFailure {
        #[serde(default)]
        tool_name: String,
        #[serde(default)]
        error: String,
    },

    /// Sub-agent started (can suppress notifications)
    #[serde(alias = "subagent_start")]
    SubagentStart,

    /// Context compaction about to happen (resource limit)
    #[serde(alias = "pre_compact")]
    PreCompact,
}
