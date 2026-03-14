---
name: session-manager
description: Manage agent-hand tmux sessions via the CLI
---

# Session Manager

Manage AI coding agent sessions through the `agent-hand` CLI. Each session runs
inside a tmux pane with automatic status tracking.

## Session CRUD

### Add a session

```bash
# Add from current directory
agent-hand add

# Add with a specific path
agent-hand add /path/to/project

# Add with title and group
agent-hand add /path/to/project --title "API Server" --group "backend"

# Add with a custom command
agent-hand add /path/to/project --title "Dev Server" --cmd "npm run dev"
```

| Flag        | Short | Description                          |
|-------------|-------|--------------------------------------|
| `--title`   | `-t`  | Human-readable session name          |
| `--group`   | `-g`  | Group path for organization          |
| `--cmd`     | `-c`  | Command to run (default: shell)      |

### List sessions

```bash
# Human-readable table
agent-hand list

# Machine-readable JSON (use this for parsing)
agent-hand list --json

# List from all profiles
agent-hand list --all
```

JSON output format:
```json
[
  {
    "id": "abc123",
    "title": "My Project",
    "path": "/home/user/project",
    "group": "backend",
    "status": "running"
  }
]
```

### Remove a session

```bash
# Remove by ID
agent-hand remove abc123

# Remove by title
agent-hand remove "API Server"
```

### Show status

```bash
# Summary status
agent-hand status

# Verbose with details
agent-hand status --verbose

# Quiet mode (just count)
agent-hand status --quiet

# JSON output (for parsing)
agent-hand status --json
```

## Session Control

### Start / Stop / Restart

```bash
agent-hand session start <id>
agent-hand session stop <id>
agent-hand session restart <id>
```

### Interrupt, Resume & Send Prompt

Interact with running agent CLI sessions programmatically:

```bash
# Safely interrupt the current agent turn (sends Escape)
agent-hand-bridge session interrupt <id>

# Resume a previous CLI conversation (requires captured session ID)
agent-hand-bridge session resume <id>

# Send a text prompt to an idle/waiting session
agent-hand-bridge session send <id> <text...>
```

**Interrupt** sends `Escape` to the tmux pane, which safely interrupts the
agent CLI's current turn without exiting the REPL (unlike Ctrl+C which can
exit the process entirely).

**Resume** reconstructs the resume command for the session's tool type:
- Claude: `claude --resume <session-id>` (with `--dangerously-skip-permissions` if configured)
- Codex: `codex --continue`
- Gemini: `gemini resume <session-id>`

If the tmux session is still alive, it sends Escape first, waits, then injects
the resume command. If the tmux session is gone, it creates a fresh tmux pane
with the resume command.

**Send Prompt** types text into the session's PTY and presses Enter. Only works
when the session status is `idle` or `waiting` — returns an error if the agent
is currently `running`, `error`, or `starting`.

**State Guards:**
- `resume` requires the session to have a stored `cli_session_id` (captured by hooks)
- `interrupt` requires the tmux session to exist
- `send` only works when session status is **Idle** or **Waiting** — returns error if Running/Error/Starting
- The session must have been started at least once for the hook to capture the ID

### Attach to a session

Opens the tmux pane for interactive use:

```bash
agent-hand session attach <id>
```

### Show session details

```bash
# Show specific session
agent-hand session show <id>

# Show current session (auto-detect from tmux)
agent-hand session show
```

## Profile Management

Profiles isolate sets of sessions. The default profile is `default`.

```bash
# List all profiles
agent-hand profile list

# Create a new profile
agent-hand profile create work

# Delete a profile
agent-hand profile delete work

# Use a specific profile (any command)
agent-hand --profile work list
agent-hand -p work list

# Or via environment variable
export AGENTHAND_PROFILE=work
agent-hand list
```

## Tmux Status Line

For tmux integration, get a compact one-line status:

```bash
agent-hand statusline
```

## Common AI Agent Workflows

### Discover running sessions

```bash
# Via full CLI (slower, human-friendly)
SESSIONS=$(agent-hand list --json)
echo "$SESSIONS" | jq '.[] | select(.status == "running") | .id'

# Via bridge (faster, ~2ms startup)
agent-hand-bridge session list | jq '.sessions[] | select(.status == "running") | .id'
```

### Check if agent-hand is responsive

```bash
agent-hand-bridge ping
# Output: agent-hand: running (hook=ok, canvas=ok, control=ok)
# Exit code: 0 if running, 1 if not
```

### Create a project workspace (via bridge)

```bash
# Add sessions via the fast bridge binary
agent-hand-bridge session add ~/projects/frontend --title "Frontend" --group "web-app"
agent-hand-bridge session add ~/projects/backend --title "Backend API" --group "web-app"
agent-hand-bridge session add ~/projects/shared --title "Shared Libs" --group "web-app"

# Tag for filtering
agent-hand-bridge session tag <frontend-id> "react"
agent-hand-bridge session tag <backend-id> "rust"

# Start all sessions in the group
for id in $(agent-hand-bridge session list --group "web-app" | jq -r '.sessions[].id'); do
  agent-hand-bridge session start "$id"
done
```

### Monitor session status

```bash
# Via bridge (preferred for scripts)
agent-hand-bridge status | jq '.'

# Detailed per-session info
agent-hand-bridge session info <id> | jq '.'

# Filter by status (running, waiting, idle, error, starting)
agent-hand-bridge session list --status running
agent-hand-bridge session list --status error | jq '.sessions[].id'
```

### Read session output (pane)

Read the last N lines of a session's tmux pane to see what the agent is doing:

```bash
# Default: last 30 lines
agent-hand-bridge session pane <id>

# More context
agent-hand-bridge session pane <id> --lines 100

# Extract text content
agent-hand-bridge session pane <id> | jq -r '.content'
```

### Read session progress

Progress files track task completions and context snapshots:

```bash
agent-hand-bridge session progress <id>
agent-hand-bridge session progress <id> | jq -r '.content'
```

### Restart a failed session

```bash
agent-hand-bridge session restart <id>
```

### Manage tags

```bash
# Add tags for categorization
agent-hand-bridge session tag <id> "experiment"
agent-hand-bridge session tag <id> "high-priority"

# Filter by tag
agent-hand-bridge session list --tag "experiment"

# Remove a tag
agent-hand-bridge session untag <id> "experiment"
```

### Create relationships between sessions

```bash
# Link related sessions (Pro)
agent-hand-bridge rel add <frontend-id> <backend-id> --type dependency --label "API"
agent-hand-bridge rel add <backend-id> <shared-id> --type peer

# View relationships
agent-hand-bridge rel list --session <id>
```

## Version & Upgrade

```bash
# Show version
agent-hand version

# Upgrade to latest
agent-hand upgrade

# Upgrade to specific version
agent-hand upgrade --version v0.3.8

# Install to custom prefix
agent-hand upgrade --prefix ~/.local/bin
```

## Authentication (Pro/Max features)

```bash
# Login to unlock premium features
agent-hand login

# Check account status
agent-hand account
agent-hand account --refresh

# Logout
agent-hand logout

# Manage devices
agent-hand devices
agent-hand devices --remove <device-id-prefix>
```

## Session Sharing (Premium)

```bash
# Share a session (read-only by default)
agent-hand share <session-id>
agent-hand share <session-id> --permission rw
agent-hand share <session-id> --expire 30  # minutes

# Stop sharing
agent-hand unshare <session-id>

# Join a shared session
agent-hand join <share-url>
```

## Tips for AI Agents

1. **Prefer `agent-hand-bridge`** for scripted/automated session management (~2ms startup vs ~50ms).
2. Use `agent-hand-bridge ping` for fast liveness checks.
3. Session IDs are stable across restarts; titles may change.
4. The `--profile` flag or `AGENTHAND_PROFILE` env var isolates sessions.
5. Group paths (`--group`) enable logical organization without filesystem coupling.
6. Tags enable flexible filtering: `session list --tag "experiment"`.
7. All bridge output is JSON — pipe to `jq` for parsing.
8. Use `agent-hand-bridge control` for raw JSON operations and batch requests.
9. The full CLI (`agent-hand`) is still needed for profile management, auth, and TUI.
