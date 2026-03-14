---
name: workspace-ops
description: Manage agent-hand workspace — sessions, groups, canvas, and progress tracking
---

# Workspace Operations

Unified reference for AI agents managing an agent-hand workspace. All commands use
the fast `agent-hand-bridge` binary (~2ms startup) over Unix domain sockets.

## Quick Reference

```bash
agent-hand-bridge ping                          # Check if agent-hand is running
agent-hand-bridge status                        # Overview of all sessions
agent-hand-bridge session list                  # List all sessions (JSON)
agent-hand-bridge session list --status running # Filter by status
agent-hand-bridge session pane <id>             # Read tmux pane output
agent-hand-bridge session progress <id>         # Read progress file
agent-hand-bridge query nodes                   # List canvas nodes
agent-hand-bridge query nodes --kind Decision   # Filter by node kind
```

### Session Interaction

```bash
# Interrupt a running agent (safe Escape, not Ctrl+C)
agent-hand-bridge session interrupt <id>

# Resume a previous conversation
agent-hand-bridge session resume <id>

# Send a prompt to an idle/waiting session
agent-hand-bridge session send <id> "implement the login feature"
```

## Common Workflows

### 1. Check What's Running

```bash
# Quick health check
agent-hand-bridge ping
# → agent-hand: running (hook=ok, canvas=ok, control=ok)

# List only running sessions
agent-hand-bridge session list --status running | jq '.sessions[]'

# Count by status
agent-hand-bridge status | jq '{running, waiting, idle, error}'
```

### 2. Read Session Output

Read the last N lines from a session's tmux pane to see what the agent is doing:

```bash
# Default: last 30 lines
agent-hand-bridge session pane <id>

# More context: last 100 lines
agent-hand-bridge session pane <id> --lines 100

# Extract just the content text
agent-hand-bridge session pane <id> | jq -r '.content'
```

### 3. Check Session Progress

Progress files track task completions, pre-compact saves, and context snapshots:

```bash
agent-hand-bridge session progress <id> | jq -r '.content'
```

Returns the markdown progress log at `~/.agent-hand/profiles/default/progress/{tmux_name}.md`.

### 4. Create a Project Workspace

```bash
# Create a group
agent-hand-bridge group create "my-project"

# Add sessions
agent-hand-bridge session add ~/code/frontend --title "Frontend" --group "my-project"
agent-hand-bridge session add ~/code/backend --title "Backend" --group "my-project"
agent-hand-bridge session add ~/code/shared --title "Shared" --group "my-project"

# Tag for filtering
FRONTEND_ID=$(agent-hand-bridge session list --group "my-project" | jq -r '.sessions[] | select(.title=="Frontend") | .id')
agent-hand-bridge session tag "$FRONTEND_ID" "react"

# Start all sessions
for id in $(agent-hand-bridge session list --group "my-project" | jq -r '.sessions[].id'); do
  agent-hand-bridge session start "$id"
done

# Build a canvas map
agent-hand-bridge canvas --batch /dev/stdin <<'EOF'
[
  {"op":"add_node","id":"session:frontend","label":"Frontend","kind":"Process"},
  {"op":"add_node","id":"session:backend","label":"Backend","kind":"Process"},
  {"op":"add_node","id":"session:shared","label":"Shared","kind":"Note"},
  {"op":"add_edge","from":"session:frontend","to":"session:backend","label":"API calls"},
  {"op":"add_edge","from":"session:backend","to":"session:shared","label":"imports"},
  {"op":"layout","direction":"LeftRight"}
]
EOF
```

### 5. Monitor and React

```bash
# Check status of all sessions
agent-hand-bridge session list --status error | jq '.sessions[].id'

# For each errored session, read what happened
for id in $(agent-hand-bridge session list --status error | jq -r '.sessions[].id'); do
  echo "=== Session $id ==="
  agent-hand-bridge session pane "$id" --lines 20 | jq -r '.content'
  echo ""
done

# Restart errored sessions
for id in $(agent-hand-bridge session list --status error | jq -r '.sessions[].id'); do
  agent-hand-bridge session restart "$id"
done
```

### 6. Interact with Sessions Programmatically

Use the bridge to control agent sessions without attaching to tmux:

```bash
# Check which sessions are running
agent-hand-bridge session list --status running

# Interrupt a busy agent to free it up
agent-hand-bridge session interrupt <id>

# Resume a crashed or restarted session (preserves conversation history)
agent-hand-bridge session resume <id>

# Send a new task to an idle agent
agent-hand-bridge session send <id> "refactor the auth module to use JWT"

# Monitor the agent's progress
agent-hand-bridge session pane <id> --lines 50
```

**Workflow: Graceful restart with conversation preservation**

```bash
# Instead of killing and restarting (loses conversation):
# 1. Interrupt the current turn
agent-hand-bridge session interrupt <id>

# 2. Wait a moment for the CLI to return to prompt
sleep 2

# 3. Resume with the stored session ID
agent-hand-bridge session resume <id>
```

This preserves the full conversation history and context, unlike a
traditional restart which creates a fresh session.

**Workflow: Check status before sending**

```bash
# Only send a prompt if the session is idle or waiting
status=$(agent-hand-bridge session list | jq -r ".sessions[] | select(.id==\"$ID\") | .status")
if [ "$status" = "idle" ] || [ "$status" = "waiting" ]; then
  agent-hand-bridge session send "$ID" "run tests"
fi
```

### 7. Canvas Query and Filtering

```bash
# All nodes
agent-hand-bridge query nodes

# Only Decision nodes
agent-hand-bridge query nodes --kind Decision

# Nodes containing "API" in the label
agent-hand-bridge query nodes --label "API"

# Specific node by ID
agent-hand-bridge query nodes --id "session:backend"

# Edges containing "calls" in the label
agent-hand-bridge query edges --label "calls"
```

## Status Values

Sessions can have the following statuses:

| Status     | Meaning                                    |
|------------|--------------------------------------------|
| `running`  | Agent is actively processing               |
| `waiting`  | Agent is waiting for user input            |
| `idle`     | Tmux session exists but agent is inactive  |
| `error`    | Agent encountered an error                 |
| `starting` | Session is initializing                    |

## Socket Paths

| Socket               | Path                          | Purpose          |
|----------------------|-------------------------------|------------------|
| Hook events          | `~/.agent-hand/events/hook.sock` | Hook delivery |
| Canvas operations    | `~/.agent-hand/canvas.sock`   | Canvas ops       |
| Control operations   | `~/.agent-hand/control.sock`  | Session/group ops|

## Tips

1. Always check `agent-hand-bridge ping` before sending commands.
2. Use `--status` filter to avoid scanning all sessions.
3. Pipe JSON output to `jq` for parsing — all bridge output is JSON.
4. Use `session pane` to inspect what an agent is currently doing.
5. Use `session progress` to see accumulated task history.
6. Batch canvas operations with `--batch` for atomicity.
7. Session IDs are stable UUIDs; titles may change.

## Related Skills

- **`canvas-ops`** — Full CanvasOp vocabulary, node kinds, response formats
- **`canvas-render`** — Agent-driven canvas visualizations from runtime artifacts (LOD, projection rendering)
