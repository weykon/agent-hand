#!/usr/bin/env bash
# Bridge: Claude Code hook events → agent-hand JSONL event file.
#
# This script is registered as a hook for multiple Claude Code events
# (Stop, Notification, UserPromptSubmit, SubagentStart, PreCompact).
# It reads the event JSON from stdin, determines the tmux session name,
# maps the event to agent-hand's format, and appends to the events file.

set -euo pipefail

EVENTS_DIR="${HOME}/.agent-hand/events"
EVENTS_FILE="${EVENTS_DIR}/hook-events.jsonl"
mkdir -p "$EVENTS_DIR"

# Read stdin (Claude Code sends hook event as JSON)
INPUT="$(cat)"

# Determine the tmux session name.
# When running inside a tmux pane managed by agent-hand, TMUX is set.
TMUX_SESSION=""
if [ -n "${TMUX:-}" ]; then
  TMUX_SESSION="$(tmux display-message -p '#{session_name}' 2>/dev/null || true)"
fi

# If not in tmux, we can't map to an agent-hand session — skip silently
if [ -z "$TMUX_SESSION" ]; then
  exit 0
fi

# Use python3 to parse input and produce our event JSON line.
# Pass data via env vars to avoid shell quoting issues.
export AGENTHAND_HOOK_INPUT="$INPUT"
export AGENTHAND_HOOK_TMUX_SESSION="$TMUX_SESSION"

python3 -c '
import json, sys, time, os

try:
    data = json.loads(os.environ.get("AGENTHAND_HOOK_INPUT", "{}"))
except Exception:
    sys.exit(0)

tmux_session = os.environ.get("AGENTHAND_HOOK_TMUX_SESSION", "")
raw_event = data.get("hook_event_name", "")
session_id = data.get("session_id", "") or data.get("conversation_id", "")
cwd = data.get("cwd", "")
ts = time.time()

# Map Claude Code event names to our event kinds
event_map = {
    "Stop": {"type": "stop"},
    "UserPromptSubmit": {"type": "user_prompt_submit"},
    "Notification": {
        "type": "notification",
        "notification_type": data.get("notification_type", ""),
    },
    "PermissionRequest": {
        "type": "permission_request",
        "tool_name": data.get("tool_name", ""),
    },
    "PostToolUseFailure": {
        "type": "tool_failure",
        "tool_name": data.get("tool_name", ""),
        "error": data.get("error", ""),
    },
    "SubagentStart": {"type": "subagent_start"},
    "PreCompact": {"type": "pre_compact"},
    # Cursor compatibility
    "stop": {"type": "stop"},
    "preToolUse": {"type": "user_prompt_submit"},
    "postToolUse": {"type": "stop"},
    "subagentStop": {"type": "stop"},
    "subagentStart": {"type": "subagent_start"},
    "beforeSubmitPrompt": {"type": "user_prompt_submit"},
    "beforeShellExecution": {"type": "user_prompt_submit"},
    # Codex CLI
    "userPromptSubmitted": {"type": "user_prompt_submit"},
    "errorOccurred": {
        "type": "tool_failure",
        "tool_name": data.get("tool_name", ""),
        "error": data.get("error", ""),
    },
    # Windsurf
    "post_cascade_response": {"type": "stop"},
    "pre_user_prompt": {"type": "user_prompt_submit"},
    # Kiro
    "agentSpawn": {"type": "subagent_start"},
    "userPromptSubmit": {"type": "user_prompt_submit"},
    # Gemini CLI
    "turn_complete": {"type": "stop"},
    "user_prompt_submit": {"type": "user_prompt_submit"},
}

kind = event_map.get(raw_event)
if kind is None:
    sys.exit(0)

event = {
    "tmux_session": tmux_session,
    "kind": kind,
    "session_id": session_id,
    "cwd": cwd,
    "ts": ts,
}

print(json.dumps(event, separators=(",", ":")))
' >> "$EVENTS_FILE" 2>/dev/null || true
