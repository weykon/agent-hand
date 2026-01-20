#!/usr/bin/env bash
set -euo pipefail

action="${1:-}"
if [[ "$action" != "--enable" && "$action" != "--disable" ]]; then
  echo "Usage: $(basename "$0") --enable|--disable" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required. Please install jq first." >&2
  exit 1
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 is required. Please install python3 first." >&2
  exit 1
fi

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
src_hook="$repo_dir/scripts/claude/log_user_prompt.sh"
dst_hook="$HOME/.agent-hand/hooks/log_user_prompt.sh"
settings="$HOME/.claude/settings.json"

mkdir -p "$(dirname "$settings")"

if [[ ! -f "$settings" ]]; then
  echo "{}" > "$settings"
fi

cp -f "$settings" "$settings.bak"

hook_cmd="$dst_hook"

if [[ "$action" == "--enable" ]]; then
  mkdir -p "$(dirname "$dst_hook")"
  cp -f "$src_hook" "$dst_hook"
  chmod +x "$dst_hook"

  jq --arg cmd "$hook_cmd" '
    .hooks |= (. // {}) |
    .hooks.UserPromptSubmit |= (. // []) |
    if (.hooks.UserPromptSubmit | map(.hooks[]?.command == $cmd) | any) then
      .
    else
      .hooks.UserPromptSubmit += [{
        matcher: "",
        hooks: [{type: "command", command: $cmd}]
      }]
    end
  ' "$settings" > "$settings.tmp" && mv "$settings.tmp" "$settings"

  echo "Enabled UserPromptSubmit hook in $settings"
  exit 0
fi

jq --arg cmd "$hook_cmd" '
  if .hooks?.UserPromptSubmit then
    .hooks.UserPromptSubmit |= (
      map(.hooks = (.hooks | map(select(.command != $cmd))))
      | map(select(.hooks | length > 0))
    )
  else
    .
  end
  | if .hooks?.UserPromptSubmit == [] then .hooks |= del(.UserPromptSubmit) else . end
  | if .hooks == {} then del(.hooks) else . end
' "$settings" > "$settings.tmp" && mv "$settings.tmp" "$settings"

echo "Disabled UserPromptSubmit hook in $settings"
