#!/usr/bin/env bash
set -euo pipefail

# Log Claude Code user prompts with cleanup, truncation, and optional compression.

LOG_DIR="${CLAUDE_PROMPT_LOG_DIR:-$HOME/.agent-hand/logs/claude-prompts}"
LOG_FILE="${CLAUDE_PROMPT_LOG_FILE:-$LOG_DIR/user-prompts.log}"
MAX_CHARS="${CLAUDE_PROMPT_LOG_MAX_CHARS:-4000}"
MAX_BYTES="${CLAUDE_PROMPT_LOG_MAX_BYTES:-1048576}"
ENABLE_COMPRESS="${CLAUDE_PROMPT_LOG_COMPRESS:-1}"

mkdir -p "$LOG_DIR"

payload="$(cat)"
prompt="$(printf '%s' "$payload" | jq -r '
  .prompt //
  .user_prompt //
  .input.prompt //
  .tool_input.prompt //
  ""'
)"

if [[ -z "$prompt" ]]; then
  exit 0
fi

prompt_clean="$(printf '%s' "$prompt" | python3 - "$MAX_CHARS" <<'PY'
import sys
max_chars = int(sys.argv[1])
text = sys.stdin.read()
lines = [line.rstrip() for line in text.splitlines() if line.strip()]
text = "\n".join(lines).strip()
if max_chars > 0 and len(text) > max_chars:
    text = text[:max_chars] + "...[truncated]"
sys.stdout.write(text)
PY
)"

if [[ -z "$prompt_clean" ]]; then
  exit 0
fi

ts="$(date -u +%Y%m%d-%H%M%SZ)"
{
  printf -- "---\n"
  printf "ts=%s\n" "$ts"
  printf "len=%s\n" "${#prompt_clean}"
  printf "%s\n" "$prompt_clean"
} >> "$LOG_FILE"

if [[ "$ENABLE_COMPRESS" == "1" ]] && [[ -f "$LOG_FILE" ]]; then
  size="$(wc -c < "$LOG_FILE" | tr -d ' ')"
  if [[ "$size" -ge "$MAX_BYTES" ]]; then
    archive_ts="$(date -u +%Y%m%d-%H%M%SZ)"
    if command -v zip >/dev/null 2>&1; then
      zip -j "$LOG_DIR/user-prompts-$archive_ts.zip" "$LOG_FILE" >/dev/null
    else
      tar -czf "$LOG_DIR/user-prompts-$archive_ts.tgz" -C "$LOG_DIR" "$(basename "$LOG_FILE")"
    fi
    : > "$LOG_FILE"
  fi
fi
