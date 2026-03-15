#!/usr/bin/env bash
# Bridge: Claude Code hook events → agent-hand JSONL event file.
#
# Base version: event logging + basic context file injection.
# Pro/Max version (in private repo) adds relationship context injection.

set -euo pipefail

EVENTS_DIR="${HOME}/.agent-hand/events"
EVENTS_FILE="${EVENTS_DIR}/hook-events.jsonl"
mkdir -p "$EVENTS_DIR"

INPUT="$(cat)"

TMUX_SESSION=""
if [ -n "${TMUX:-}" ]; then
  TMUX_SESSION="$(tmux display-message -p '#{session_name}' 2>/dev/null || true)"
fi

export AGENTHAND_HOOK_INPUT="$INPUT"
export AGENTHAND_HOOK_TMUX_SESSION="$TMUX_SESSION"
export AGENTHAND_EVENTS_FILE="$EVENTS_FILE"

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
    "PreToolUse": {
        "type": "pre_tool_use",
        "tool_name": data.get("tool_name", ""),
        "tool_input": json.dumps(data.get("tool_input", {}))[:4000],
        "tool_use_id": data.get("tool_use_id", ""),
    },
    "PostToolUse": {
        "type": "post_tool_use",
        "tool_name": data.get("tool_name", ""),
        "tool_input": json.dumps(data.get("tool_input", {}))[:4000],
        "tool_response": (data.get("tool_response", "") or "")[:4000],
        "tool_use_id": data.get("tool_use_id", ""),
    },
    "stop": {"type": "stop"},
    "preToolUse": {
        "type": "pre_tool_use",
        "tool_name": data.get("tool_name", ""),
        "tool_input": json.dumps(data.get("tool_input", {}))[:4000],
        "tool_use_id": data.get("tool_use_id", ""),
    },
    "postToolUse": {
        "type": "post_tool_use",
        "tool_name": data.get("tool_name", ""),
        "tool_input": json.dumps(data.get("tool_input", {}))[:4000],
        "tool_response": (data.get("tool_response", "") or "")[:4000],
        "tool_use_id": data.get("tool_use_id", ""),
    },
    "subagentStop": {"type": "stop"},
    "subagentStart": {"type": "subagent_start"},
    "beforeSubmitPrompt": {"type": "user_prompt_submit"},
    "beforeShellExecution": {"type": "user_prompt_submit"},
    "userPromptSubmitted": {"type": "user_prompt_submit"},
    "errorOccurred": {
        "type": "tool_failure",
        "tool_name": data.get("tool_name", ""),
        "error": data.get("error", ""),
    },
    "post_cascade_response": {"type": "stop"},
    "pre_user_prompt": {"type": "user_prompt_submit"},
    "agentSpawn": {"type": "subagent_start"},
    "userPromptSubmit": {"type": "user_prompt_submit"},
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

MAX_PROMPT_CHARS = 2000
if kind.get("type") == "user_prompt_submit":
    prompt = (
        data.get("prompt")
        or data.get("user_prompt")
        or (data.get("input") or {}).get("prompt")
        or (data.get("tool_input") or {}).get("prompt")
        or ""
    )
    if isinstance(prompt, str) and prompt.strip():
        prompt = prompt.strip()
        if len(prompt) > MAX_PROMPT_CHARS:
            prompt = prompt[:MAX_PROMPT_CHARS]
        event["prompt"] = prompt

events_file = os.environ.get("AGENTHAND_EVENTS_FILE", "")
if events_file and tmux_session:
    try:
        with open(events_file, "a", encoding="utf-8") as f:
            f.write(json.dumps(event, separators=(",", ":")) + "\n")
    except Exception:
        pass

# Context injection: read .agent-hand-context.md on UserPromptSubmit
MAX_CONTEXT_CHARS = 9000
event_type = kind.get("type", "") if isinstance(kind, dict) else ""
is_prompt_submit = event_type in ("user_prompt_submit", "UserPromptSubmit")
if is_prompt_submit and cwd:
    context_path = os.path.join(cwd, ".agent-hand-context.md")
    try:
        if os.path.isfile(context_path):
            with open(context_path, "r", encoding="utf-8", errors="replace") as f:
                context_content = f.read(MAX_CONTEXT_CHARS)
            if context_content.strip():
                hook_output = {
                    "hookSpecificOutput": {
                        "additionalContext": context_content
                    }
                }
                print(json.dumps(hook_output, separators=(",", ":")))
    except Exception:
        pass
' 2>/dev/null || true
