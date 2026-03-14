---
name: settings-config
description: Read and modify agent-hand configuration via CLI or config file
---

# Settings & Config Management

Manage agent-hand settings programmatically via the `agent-hand config` CLI
or by editing `~/.agent-hand/config.toml` directly.

## CLI Commands

### List all settings

```bash
# As TOML (human-readable)
agent-hand config list

# As JSON (machine-parseable)
agent-hand config list --json
```

### Get a specific value

```bash
agent-hand config get notification.volume
agent-hand config get ai.provider
agent-hand config get context_bridge.enabled
```

### Set a value

```bash
agent-hand config set notification.volume 0.5
agent-hand config set ai.provider openai
agent-hand config set notification.enabled true
agent-hand config set context_bridge.cooldown_secs 30
```

Type inference is automatic — booleans, integers, floats, and strings are
detected from the existing field type. For arrays, use JSON syntax:

```bash
agent-hand config set context_bridge.trigger_events '["status_change","task_complete"]'
```

### Show config file path

```bash
agent-hand config path
# Output: /Users/<user>/.agent-hand/config.toml
```

### Reset to defaults

```bash
agent-hand config reset          # interactive confirmation
agent-hand config reset --force  # skip confirmation
```

---

## Configuration Reference

All settings use dot-notation paths. Default values shown after `=`.

### Notification (`notification.*`)

| Key                              | Type  | Default  | Description                         |
|----------------------------------|-------|----------|-------------------------------------|
| `notification.enabled`           | bool  | `true`   | Master notification toggle          |
| `notification.volume`            | float | `0.7`    | Volume 0.0–1.0                      |
| `notification.sound_pack`        | str   | `"default"` | Sound pack name                  |
| `notification.on_task_complete`  | bool  | `true`   | Notify when task finishes           |
| `notification.on_input_required` | bool  | `true`   | Notify when agent needs input       |
| `notification.on_error`          | bool  | `true`   | Notify on errors                    |
| `notification.on_session_start`  | bool  | `true`   | Notify on session start             |
| `notification.on_task_acknowledge` | bool | `false` | Notify on task acknowledgement      |
| `notification.on_resource_limit` | bool  | `false`  | Notify on resource limits           |
| `notification.on_user_spam`      | bool  | `false`  | Notify on repeated user messages    |
| `notification.quiet_when_focused`| bool  | `true`   | Mute when session is focused        |

### AI (`ai.*`)

| Key              | Type | Default     | Description                    |
|------------------|------|-------------|--------------------------------|
| `ai.provider`    | str  | `"openai"`  | AI provider (openai, anthropic, custom) |
| `ai.model`       | str  | `"gpt-4"`  | Model name                     |
| `ai.api_key`     | str  | `""`        | API key (keep secret!)         |
| `ai.base_url`    | str  | (none)      | Custom API endpoint URL        |
| `ai.summary_lines` | int | `10`      | Lines to include in summaries  |

### Context Bridge (`context_bridge.*`)

| Key                              | Type    | Default | Description                    |
|----------------------------------|---------|---------|--------------------------------|
| `context_bridge.enabled`         | bool    | `false` | Enable cross-session context   |
| `context_bridge.scope`           | str     | `"self_only"` | Scope: self_only, group, global |
| `context_bridge.trigger_events`  | array   | `["status_change"]` | Events that trigger sync |
| `context_bridge.cooldown_secs`   | int     | `60`    | Minimum seconds between syncs  |
| `context_bridge.max_lines`       | int     | `50`    | Max lines per context chunk    |
| `context_bridge.max_total_chars` | int     | `5000`  | Max total characters           |
| `context_bridge.write_debug_log` | bool    | `false` | Write debug log for bridge     |

### Sharing (`sharing.*`) — Pro

| Key                          | Type | Default                    | Description                  |
|------------------------------|------|----------------------------|------------------------------|
| `sharing.tmate_server_host`  | str  | `"ssh.tmate.io"`           | tmate server hostname        |
| `sharing.tmate_server_port`  | int  | `22`                       | tmate server port            |
| `sharing.default_permission` | str  | `"readonly"`               | Default share permission     |
| `sharing.auto_expire_minutes`| int  | (none)                     | Auto-expire time in minutes  |
| `sharing.relay_server_url`   | str  | (none)                     | Custom relay server URL      |
| `sharing.relay_discovery_url`| str  | `"https://auth.asymptai.com/api/relay/discover"` | Relay discovery endpoint |

### Hooks (`hooks.*`)

| Key                    | Type | Default | Description                       |
|------------------------|------|---------|-----------------------------------|
| `hooks.auto_register`  | bool | `true`  | Auto-register event bridge hooks  |

### Claude Hooks (`claude.*`)

| Key                                  | Type | Default | Description                        |
|--------------------------------------|------|---------|------------------------------------|
| `claude.user_prompt_logging`         | bool | `false` | Log user prompts                   |
| `claude.dangerously_skip_permissions`| bool | `false` | Skip permission checks (dangerous) |

### Analytics (`analytics.*`)

| Key                 | Type | Default | Description            |
|---------------------|------|---------|------------------------|
| `analytics.enabled` | bool | `true`  | Enable usage analytics |

### Skills (`skills.*`) — Pro

| Key               | Type | Default | Description                 |
|-------------------|------|---------|-----------------------------|
| `skills.repo_url` | str  | (none)  | GitHub skills repo URL      |
| `skills.auto_sync`| bool | `true`  | Auto-sync on startup        |

### General (top-level)

| Key                          | Type | Default  | Description                           |
|------------------------------|------|----------|---------------------------------------|
| `ready_ttl_minutes`          | int  | `5`      | Minutes before "ready" status expires |
| `jump_lines`                 | int  | `20`     | Lines to jump with PgUp/PgDn         |
| `scroll_padding`             | int  | `3`      | Scroll padding at edges               |
| `mouse_capture`              | str  | `"auto"` | Mouse mode: auto, always, never       |
| `language`                   | str  | (none)   | UI language: en, zh, ja               |
| `first_launch`               | bool | `true`   | Show first-launch wizard              |
| `canvas_projection_enabled`  | bool | `false`  | Enable canvas projection              |

### Keybindings (`keybindings.*`)

Keybindings use the format `modifier+key` where modifiers are `ctrl`, `alt`,
`shift`. Multiple bindings for one action use comma separation.

```bash
agent-hand config set keybindings.quit "ctrl+c"
agent-hand config set keybindings.next_session "ctrl+n"
agent-hand config set keybindings.canvas_toggle "ctrl+k"
```

Available keybinding actions:

| Action               | Default          | Description                    |
|----------------------|------------------|--------------------------------|
| `quit`               | `q`, `ctrl+c`   | Quit application               |
| `help`               | `?`, `F1`       | Show help dialog               |
| `next_session`       | `j`, `Down`      | Move to next session           |
| `prev_session`       | `k`, `Up`        | Move to previous session       |
| `start`              | `s`              | Start selected session         |
| `stop`               | `x`              | Stop selected session          |
| `restart`            | `r`              | Restart selected session       |
| `attach`             | `Enter`, `a`     | Attach to session              |
| `canvas_toggle`      | `ctrl+k`         | Toggle canvas view             |
| `canvas_ai`          | `ctrl+a`         | Canvas AI assistant            |
| `canvas_auto_layout` | `ctrl+l`         | Auto-layout canvas             |
| `hot_brain_toggle`   | `ctrl+b`         | Toggle Hot Brain panel         |
| `context_inject`     | `ctrl+i`         | Inject context                 |
| `sound_toggle`       | `ctrl+m`         | Toggle sound notifications     |
| `theme_toggle`       | `ctrl+t`         | Toggle dark/light theme        |
| `group_next`         | `Tab`            | Next session group             |
| `group_prev`         | `shift+Tab`      | Previous session group         |
| `search`             | `/`              | Open search                    |

---

## Config File Location

The configuration file is at `~/.agent-hand/config.toml`. It is auto-created
on first launch with sensible defaults.

## Hot-Reload Behavior

| Setting category | Requires restart? |
|-----------------|-------------------|
| Notification    | No — applied immediately |
| Keybindings     | No — hot-reloaded when saved via Settings UI or CLI |
| AI              | No — next AI call uses new config |
| Context Bridge  | No — next sync cycle uses new config |
| Sharing         | Yes — relay connection uses startup config |
| Hooks           | Yes — hook registration runs at startup |

## Common Agent Workflows

### Mute notifications for a focused session

```bash
agent-hand config set notification.quiet_when_focused true
agent-hand config set notification.volume 0.3
```

### Enable context bridge between sessions

```bash
agent-hand config set context_bridge.enabled true
agent-hand config set context_bridge.scope group
agent-hand config set context_bridge.cooldown_secs 30
```

### Switch AI provider

```bash
agent-hand config set ai.provider anthropic
agent-hand config set ai.model claude-sonnet-4-20250514
agent-hand config set ai.api_key sk-ant-...
```

### Change UI language

```bash
agent-hand config set language zh   # Chinese
agent-hand config set language en   # English
agent-hand config set language ja   # Japanese
```

### Rebind a shortcut

```bash
agent-hand config set keybindings.quit "ctrl+q"
agent-hand config set keybindings.canvas_toggle "alt+k"
```
