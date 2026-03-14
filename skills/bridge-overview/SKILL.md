---
name: bridge-overview
description: Overview of the agent-hand-bridge binary and when to use each mode
---

# agent-hand-bridge Overview

`agent-hand-bridge` is a lightweight, sync-only companion binary for agent-hand.
It provides fast IPC (~2ms startup, ~400K binary) for external tools and AI agents
to interact with a running agent-hand instance.

**Key design**: pure sync (no tokio runtime), never fails loudly in hook mode.

## Installation

The bridge binary is installed alongside `agent-hand`. Both are built from the
same Cargo workspace:

```bash
# Both binaries are installed together
cargo install --path .
# Produces: agent-hand (main TUI + CLI) and agent-hand-bridge (fast IPC)
```

## Modes

### 1. Hook Event Mode (default, stdin)

Reads a CLI tool's hook event from stdin, normalizes it, and delivers it to
agent-hand via Unix socket or JSONL file fallback.

```bash
# Typically called by Claude Code, Cursor, Codex, etc. hook systems:
echo '{"hook_event_name":"UserPromptSubmit","prompt":"fix the bug"}' | agent-hand-bridge
```

**When to use**: You don't call this directly. It's registered as a hook handler
for AI CLI tools (Claude Code, Cursor, Codex, Windsurf, Kiro, Gemini CLI).
agent-hand auto-registers this during startup.

**Socket**: `~/.agent-hand/events/hook.sock`

**Behavior**:
- Reads JSON from stdin
- Detects current tmux session (skips if not in tmux)
- Normalizes the event kind across different CLI tools
- Delivers via socket, falls back to `~/.agent-hand/events/hook-events.jsonl`
- **Never exits with error** -- silently drops on failure to avoid breaking the host CLI

**Supported CLI tools and their event mappings**:

| CLI Tool    | Start/Prompt Events              | Stop Events               |
|-------------|----------------------------------|---------------------------|
| Claude Code | `UserPromptSubmit`               | `Stop`                    |
| Cursor      | `beforeSubmitPrompt`, `preToolUse` | `stop`, `postToolUse`   |
| Codex CLI   | `userPromptSubmitted`            | (via error handling)      |
| Windsurf    | `pre_user_prompt`                | `post_cascade_response`   |
| Kiro        | `userPromptSubmit`               | (via agentSpawn)          |
| Gemini CLI  | `user_prompt_submit`             | `turn_complete`           |

### 2. Canvas Mode (`canvas <json>`)

Send a CanvasOp to the running TUI's canvas and receive a response.

```bash
# Inline JSON
agent-hand-bridge canvas '{"op":"add_node","id":"n1","label":"Task A"}'

# From stdin
echo '{"op":"query","what":"nodes"}' | agent-hand-bridge canvas -

# Batch from file
agent-hand-bridge canvas --batch ops.json
```

**When to use**: When an AI agent or script needs to programmatically create,
modify, or query the canvas workflow graph.

**Socket**: `~/.agent-hand/canvas.sock`

**Behavior**:
- Connects to canvas socket
- Sends JSON line, reads JSON response
- Reports errors to stderr (unlike hook mode)
- Exit code: 0 on success, 1 on error

See the `canvas-ops` skill for the full CanvasOp JSON reference.

### 3. Query Mode (`query <what>`)

Shortcut for canvas query operations. Equivalent to
`canvas '{"op":"query","what":"<what>"}'`.

```bash
agent-hand-bridge query nodes     # List all nodes
agent-hand-bridge query edges     # List all edges
agent-hand-bridge query state     # Full state dump
agent-hand-bridge query selected  # Currently selected nodes
```

**When to use**: Quick inspection of canvas state. Simpler than constructing
the full canvas JSON.

### 4. Session Management (`session <cmd>`)

Manage sessions via the control socket. Full CRUD + lifecycle + metadata.

```bash
# List all sessions
agent-hand-bridge session list
agent-hand-bridge session list --group "backend" --tag "experiment"

# Add a session
agent-hand-bridge session add /path/to/project --title "API Server" --group "backend"

# Lifecycle
agent-hand-bridge session start <id>
agent-hand-bridge session stop <id>
agent-hand-bridge session restart <id>

# Metadata
agent-hand-bridge session info <id>
agent-hand-bridge session rename <id> "New Title"
agent-hand-bridge session label <id> "priority" --color red
agent-hand-bridge session move <id> "frontend"
agent-hand-bridge session tag <id> "experiment"
agent-hand-bridge session untag <id> "experiment"

# Remove
agent-hand-bridge session remove <id>
```

**When to use**: AI agents or scripts that need to manage sessions programmatically
with fast startup (~2ms). Preferred over `agent-hand` CLI for automation.

**Socket**: `~/.agent-hand/control.sock`

### 5. Group Management (`group <cmd>`)

Manage session groups via the control socket.

```bash
agent-hand-bridge group list
agent-hand-bridge group create "work/frontend"
agent-hand-bridge group delete "work/frontend"
agent-hand-bridge group rename "work/frontend" "web/ui"
```

**Socket**: `~/.agent-hand/control.sock`

### 6. Relationship Management (`rel <cmd>`)

Manage relationships between sessions (Pro feature).

```bash
agent-hand-bridge rel list
agent-hand-bridge rel list --session <id>
agent-hand-bridge rel add <session-a-id> <session-b-id> --type peer --label "shared API"
agent-hand-bridge rel remove <rel-id>
```

**Socket**: `~/.agent-hand/control.sock`

### 7. Status (`status`)

Get an overall status report of all sessions.

```bash
agent-hand-bridge status
# Output: {"type":"status_report","total":5,"running":2,"waiting":1,"idle":2,"error":0}
```

**Socket**: `~/.agent-hand/control.sock`

### 8. Raw Control (`control <json>`)

Send raw ControlOp JSON directly. For advanced usage or batch operations.

```bash
# Any control operation as raw JSON
agent-hand-bridge control '{"op":"list_sessions"}'
agent-hand-bridge control '{"op":"batch","ops":[{"op":"list_sessions"},{"op":"status"}]}'
```

**Socket**: `~/.agent-hand/control.sock`

### 9. Ping Mode (`ping`)

Check if agent-hand is running and which sockets are alive.

```bash
agent-hand-bridge ping
# Output: agent-hand: running (hook=ok, canvas=ok, control=ok)
# Exit code: 0 if at least one socket is up, 1 if none
```

**When to use**: Health checks before sending operations. Fast liveness probe.

## Bridge vs Full CLI

| Aspect               | `agent-hand-bridge`          | `agent-hand`               |
|----------------------|------------------------------|----------------------------|
| Startup time         | ~2ms (sync, no runtime)      | ~50ms (tokio runtime)      |
| Binary size          | ~400K                        | Full application           |
| Canvas ops           | Raw JSON via socket          | Typed subcommands          |
| Session management   | Yes (via control socket)     | Yes (add, list, remove...) |
| TUI                  | No                           | Yes                        |
| Hook processing      | Yes (default mode)           | No (delegates to bridge)   |
| Error handling       | Silent in hook mode          | Full error reporting       |

**Decision guide**:

- **Use `agent-hand-bridge`** for:
  - Canvas operations from AI agents (speed matters)
  - Session/group/tag management from scripts and AI agents
  - Hook event delivery (registered automatically)
  - Quick canvas queries, status checks, and pings
  - Any programmatic/scripted interaction

- **Use `agent-hand`** for:
  - Profile management
  - Account and authentication
  - Interactive TUI usage
  - Canvas operations via typed CLI flags (more ergonomic for humans)

## Socket Paths

| Socket                            | Purpose                           |
|-----------------------------------|-----------------------------------|
| `~/.agent-hand/canvas.sock`       | Canvas operations and queries     |
| `~/.agent-hand/control.sock`      | Session/group/tag management      |
| `~/.agent-hand/events/hook.sock`  | Hook event delivery               |

Fallback file (when hook socket is unavailable):
`~/.agent-hand/events/hook-events.jsonl`

## Examples for AI Agents

### Check availability before operating

```bash
# Step 1: Is agent-hand running?
if agent-hand-bridge ping > /dev/null 2>&1; then
  # Step 2: Query current state
  agent-hand-bridge query nodes
  # Step 3: Add to canvas
  agent-hand-bridge canvas '{"op":"add_node","id":"task1","label":"Implement feature"}'
else
  echo "agent-hand is not running"
fi
```

### Session management workflow

```bash
# Create a project workspace
agent-hand-bridge session add ~/projects/frontend --title "Frontend" --group "web-app"
agent-hand-bridge session add ~/projects/backend --title "Backend API" --group "web-app"

# Tag sessions for filtering
agent-hand-bridge session tag <frontend-id> "react"
agent-hand-bridge session tag <backend-id> "rust"

# Start all sessions in a group
for id in $(agent-hand-bridge session list --group "web-app" | jq -r '.sessions[].id'); do
  agent-hand-bridge session start "$id"
done

# Check overall status
agent-hand-bridge status

# Create a relationship between sessions
agent-hand-bridge rel add <frontend-id> <backend-id> --type dependency --label "API consumer"
```

### Pipe JSON from a script

```bash
# Generate ops dynamically and pipe
python3 -c '
import json
ops = [
    {"op": "add_node", "id": f"step{i}", "label": f"Step {i}"}
    for i in range(5)
]
for i in range(4):
    ops.append({"op": "add_edge", "from": f"step{i}", "to": f"step{i+1}"})
print(json.dumps({"op": "batch", "ops": ops}))
' | agent-hand-bridge canvas -
```

### Parse query results

```bash
# Get nodes and extract IDs
agent-hand-bridge query nodes | jq -r '.nodes[].id'

# Count edges
agent-hand-bridge query edges | jq '.edges | length'

# Find a specific node
agent-hand-bridge query nodes | jq '.nodes[] | select(.label | test("API"))'

# List sessions filtered by tag
agent-hand-bridge session list --tag "experiment" | jq '.sessions[].title'
```

### Batch control operations

```bash
# Multiple operations in one call
agent-hand-bridge control '{"op":"batch","ops":[
  {"op":"list_sessions"},
  {"op":"list_groups"},
  {"op":"status"}
]}'
```
