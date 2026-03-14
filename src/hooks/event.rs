use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HookUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
}

/// A hook event received from a CLI tool (Claude Code, Codex, etc.) via JSONL file
/// or Unix domain socket.
///
/// The hook binary (or legacy shell script) running inside the tmux pane receives the
/// tool's event on stdin, enriches it with the tmux session name, and either pushes it
/// via Unix socket (preferred) or appends a JSON line to the events file (fallback).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEvent {
    /// The tmux session name (set by the hook binary via tmux display-message)
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
    /// User prompt text (only present for UserPromptSubmit events).
    /// Truncated to ~2000 chars by the hook binary.
    #[serde(default)]
    pub prompt: Option<String>,
    /// Structured token usage if the upstream hook payload exposed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<HookUsage>,
}

/// Enumeration of hook event kinds we care about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HookEventKind {
    /// Agent started working (user submitted a prompt)
    #[serde(rename = "user_prompt_submit", alias = "user_prompt_submit")]
    UserPromptSubmit,

    /// Agent finished its task
    #[serde(rename = "stop", alias = "stop")]
    Stop,

    /// Agent needs user input — permission, question, or idle prompt
    #[serde(rename = "notification", alias = "notification")]
    Notification {
        #[serde(default)]
        notification_type: String,
    },

    /// Agent is requesting permission to use a tool
    #[serde(rename = "permission_request", alias = "permission_request")]
    PermissionRequest {
        #[serde(default)]
        tool_name: String,
    },

    /// A tool call failed
    #[serde(rename = "tool_failure", alias = "tool_failure")]
    ToolFailure {
        #[serde(default)]
        tool_name: String,
        #[serde(default)]
        error: String,
    },

    /// Sub-agent started (can suppress notifications)
    #[serde(rename = "subagent_start", alias = "subagent_start")]
    SubagentStart,

    /// Context compaction about to happen (resource limit)
    #[serde(rename = "pre_compact", alias = "pre_compact")]
    PreCompact,

    /// User-initiated chat message (sideband — does not affect session status)
    #[serde(rename = "user_chat", alias = "user_chat")]
    UserChat {
        /// The user's chat message text
        #[serde(default)]
        message: String,
        /// Optional target session context (tmux session name)
        #[serde(default)]
        target_session: Option<String>,
        /// Conversation ID for multi-turn tracking
        #[serde(default)]
        conversation_id: Option<String>,
    },
}
