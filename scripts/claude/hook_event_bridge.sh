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

# Use python3 to parse input, write JSONL events (if in tmux), and
# output hookSpecificOutput context injection to stdout for Claude Code.
# Pass data via env vars to avoid shell quoting issues.
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

def extract_u64(obj, *keys):
    if not isinstance(obj, dict):
        return None
    for key in keys:
        val = obj.get(key)
        if isinstance(val, int) and val >= 0:
            return val
        if isinstance(val, str):
            try:
                parsed = int(val)
                if parsed >= 0:
                    return parsed
            except Exception:
                pass
    return None

def extract_usage(data):
    candidates = [
        data,
        data.get("usage"),
        data.get("token_usage"),
        data.get("metrics"),
        (data.get("result") or {}).get("usage") if isinstance(data.get("result"), dict) else None,
        (data.get("result") or {}).get("token_usage") if isinstance(data.get("result"), dict) else None,
        (data.get("message") or {}).get("usage") if isinstance(data.get("message"), dict) else None,
    ]
    usage = {}
    for candidate in candidates:
        usage.setdefault("input_tokens", extract_u64(candidate, "input_tokens", "prompt_tokens", "inputTokens", "promptTokens"))
        usage.setdefault("output_tokens", extract_u64(candidate, "output_tokens", "completion_tokens", "outputTokens", "completionTokens"))
        usage.setdefault("total_tokens", extract_u64(candidate, "total_tokens", "tokens", "totalTokens"))
        usage.setdefault("cache_creation_tokens", extract_u64(candidate, "cache_creation_tokens", "cacheCreationTokens"))
        usage.setdefault("cache_read_tokens", extract_u64(candidate, "cache_read_tokens", "cacheReadTokens"))

    if usage.get("total_tokens") is None and usage.get("input_tokens") is not None and usage.get("output_tokens") is not None:
        usage["total_tokens"] = usage["input_tokens"] + usage["output_tokens"]

    usage = {k: v for k, v in usage.items() if v is not None}
    return usage or None

# For prompt-submit events, extract the user prompt text (truncated).
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

usage = extract_usage(data)
if usage:
    event["usage"] = usage

# Write event to JSONL file using Python file I/O (stdout is reserved for Claude Code)
# Only write if we have a tmux session (agent-hand requirement)
events_file = os.environ.get("AGENTHAND_EVENTS_FILE", "")
if events_file and tmux_session:
    try:
        with open(events_file, "a", encoding="utf-8") as f:
            f.write(json.dumps(event, separators=(",", ":")) + "\n")
    except Exception:
        pass  # Never fail on event logging

# Hook stdout context injection:
# On UserPromptSubmit, read .agent-hand-context.md and output as
# hookSpecificOutput so Claude Code receives real-time context.
# Also injects AI summaries from related sessions via relationships.
MAX_CONTEXT_CHARS = 9000
event_type = kind.get("type", "") if isinstance(kind, dict) else ""
is_prompt_submit = event_type in ("user_prompt_submit", "UserPromptSubmit")
if is_prompt_submit and cwd:
    context_parts = []

    # Part 1: existing .agent-hand-context.md
    context_path = os.path.join(cwd, ".agent-hand-context.md")
    try:
        if os.path.isfile(context_path):
            with open(context_path, "r", encoding="utf-8", errors="replace") as f:
                file_ctx = f.read(MAX_CONTEXT_CHARS)
            if file_ctx.strip():
                context_parts.append(file_ctx)
    except Exception:
        pass

    # Part 2: cross-session context via relationships
    # Find current session by tmux_session name, look up relationships,
    # inject AI summaries from related sessions.
    try:
        home = os.path.expanduser("~")
        sessions_path = os.path.join(home, ".agent-hand", "profiles", "default", "sessions.json")
        summaries_path = os.path.join(home, ".agent-hand", "ai_summaries.json")

        if tmux_session and os.path.isfile(sessions_path):
            with open(sessions_path, "r", encoding="utf-8") as f:
                store = json.loads(f.read())

            instances = store.get("instances", [])
            relationships = store.get("relationships", [])

            # Map tmux_session name → session ID
            # tmux name format: "{sanitized_title}_{id_prefix}" or "agentdeck_rs_{id}"
            current_id = None
            for inst in instances:
                sid = inst.get("id", "")
                tmux_name = inst.get("tmux_session_name", "")
                # Direct match on stored tmux name
                if tmux_name and tmux_name == tmux_session:
                    current_id = sid
                    break
                # Legacy format: agentdeck_rs_{id}
                if tmux_session == "agentdeck_rs_" + sid:
                    current_id = sid
                    break
                # New format: {sanitized_title}_{id_first_8}
                if sid and tmux_session.endswith("_" + sid[:8]):
                    current_id = sid
                    break

            if current_id and relationships:
                # Find related session IDs
                related_ids = []
                for rel in relationships:
                    a = rel.get("session_a_id", "")
                    b = rel.get("session_b_id", "")
                    rtype = rel.get("relation_type", "")
                    label = rel.get("label", "") or ""
                    if a == current_id:
                        related_ids.append((b, rtype, label))
                    elif b == current_id:
                        related_ids.append((a, rtype, label))

                if related_ids:
                    # Load AI summaries
                    summaries = {}
                    if os.path.isfile(summaries_path):
                        try:
                            with open(summaries_path, "r", encoding="utf-8") as f:
                                summaries = json.loads(f.read())
                        except Exception:
                            pass

                    # Build session ID → title map
                    id_to_title = {inst.get("id", ""): inst.get("title", "") for inst in instances}

                    # Build related context section
                    related_lines = []
                    for rid, rtype, rlabel in related_ids:
                        rtitle = id_to_title.get(rid, rid[:12])
                        summary = summaries.get(rid, "")
                        if summary:
                            header = rtitle
                            if rlabel:
                                header += " (" + rlabel + ")"
                            related_lines.append("### " + header + " [" + rtype + "]")
                            related_lines.append(summary)
                            related_lines.append("")

                    if related_lines:
                        section = "## Related Sessions Context\n\n" + "\n".join(related_lines)
                        context_parts.append(section)
    except Exception:
        pass  # Never fail on relationship context

    # Output combined context
    if context_parts:
        combined = "\n\n---\n\n".join(context_parts)
        if len(combined) > MAX_CONTEXT_CHARS:
            combined = combined[:MAX_CONTEXT_CHARS]
        hook_output = {
            "hookSpecificOutput": {
                "additionalContext": combined
            }
        }
        print(json.dumps(hook_output, separators=(",", ":")))
' 2>/dev/null || true
