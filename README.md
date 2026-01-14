# 🦀 Agent Hand

A fast tmux-backed terminal session manager for AI coding agents.

> Agent Hand is a Rust rewrite inspired by the original Go open-source project
> [agent-deck](https://github.com/asheshgoplani/agent-deck).

Chinese README: [README.zh.md](README.zh.md)

![Preview](docs/preview.jpg)

## Why Agent Hand?

When you run multiple AI agents (Claude, Copilot, OpenCode, etc.) at the same time:
- too many panes to track (who needs confirmation, who is still working, who just finished)
- constant context switching to find “that session from a minute ago”
- easy to miss a permission/confirmation prompt and waste time waiting

Agent Hand makes this manageable with clear status icons:

| Icon | Meaning | What you should do |
|------|---------|--------------------|
| `!` (blue, blinking) | **WAITING** – the agent is blocked on a Yes/No style prompt | go check it now |
| `●` (yellow, animated) | **RUNNING** – the agent is thinking/executing | you can do something else |
| `✓` (cyan) | **READY** – finished within the last ~20 minutes | read the output |
| `○` (gray) | **IDLE** – not started yet or already seen | continue anytime |

## Highlights

- **At-a-glance status list** for all sessions
- **Fast switching**: `Ctrl+G` popup → fuzzy search and jump to any session
- **TUI dashboard**: run `agent-hand`
- **Groups**: organize by project/use case
- **Labels**: custom title + colored labels
- **tmux-friendly**: `Ctrl+Q` detach back to the dashboard
- **Self-upgrade**: `agent-hand upgrade`

## Prerequisites

- **tmux** (required) - The install script will attempt to install it automatically

```bash
# macOS
brew install tmux

# Ubuntu/Debian
sudo apt install tmux

# Fedora
sudo dnf install tmux
```

## Install

### One-liner (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/weykon/agent-hand/master/install.sh | bash
```

The install script will:
1. Check if tmux is installed (and install it if possible)
2. Download the appropriate binary for your OS/arch
3. Install to `/usr/local/bin` (if writable) or `~/.local/bin`

### Build from source

```bash
git clone https://github.com/weykon/agent-hand.git agent-hand
cd agent-hand
cargo build --release

# optional
cargo install --path .
```

## Quickstart

```bash
# open the TUI dashboard
agent-hand
```

From the dashboard:
- `n` create a session
- `Enter` attach
- in tmux: `Ctrl+Q` detach back to the dashboard
- in tmux: `Ctrl+G` popup → search + switch to another session

## Keybindings (TUI)

- Navigation: `↑/↓` or `j/k`, `Space` toggle expand/collapse group
- Session selected: `Enter` attach, `s` start, `x` stop, `r` edit (title/label), `t` tag, `R` restart, `m` move, `f` fork, `d` delete
- Group selected: `Enter` toggle, `g` create, `r` rename, `d` delete (empty = delete immediately; non-empty = confirm options)
- Global: `/` search, `p` capture preview snapshot, `?` help

## Custom keybindings

On startup, agent-hand reads configuration from (in priority order):
1. `~/.agent-hand/config.json` (legacy)
2. `~/.agent-hand/config.toml`
3. `~/.config/agent-hand/config.toml` (XDG standard)
4. `~/.config/agent-hand/config.json`

Example:

```json
{
  "keybindings": {
    "quit": ["q", "Ctrl+c"],
    "up": ["Up", "k"],
    "down": ["Down", "j"],

    "select": "Enter",
    "toggle_group": "Space",
    "expand": "Right",
    "collapse": "Left",

    "new_session": "n",
    "refresh": "Ctrl+r",
    "search": "/",
    "help": "?",

    "start": "s",
    "stop": "x",
    "rename": "r",
    "restart": "R",
    "delete": "d",
    "fork": "f",
    "create_group": "g",
    "move": "m",
    "tag": "t",
    "preview_refresh": "p"
  }
}
```

Supported key names: `Enter`, `Esc`, `Tab`, `Backspace`, `Space`, `Up`, `Down`, `Left`, `Right`, plus single characters (e.g. `r`, `R`, `/`).
Modifiers: `Ctrl+`, `Alt+`, `Shift+`.

Note: currently this only affects the main dashboard (Normal mode); other dialogs still use fixed keys.

### tmux hotkeys (Ctrl+G / Ctrl+Q)

These are bound on agent-hand’s **dedicated tmux server** (`tmux -L agentdeck_rs`), so they won’t affect your default tmux server.

Add to `~/.agent-hand/config.json`:

```json
{
  "tmux": {
    "switcher": "Ctrl+g",
    "detach": "Ctrl+q"
  }
}
```

Changes take effect the next time you attach (agent-hand rebinds keys on attach).

Notes on conflicts:
- Some keys can be **effectively the same** in terminals (e.g. `Ctrl+i` ≈ `Tab`, `Ctrl+m` ≈ `Enter`, `Ctrl+[` ≈ `Esc`), so choosing them may appear “not working”.
- Keys may also be **already bound** by tmux or your terminal/app.
- If a key doesn’t work, pick a different one (the defaults `Ctrl+G` / `Ctrl+Q` are a solid, tested choice) and verify current bindings with:
  `tmux -L agentdeck_rs list-keys -T root`

If you previously used the legacy directory `~/.agent-deck-rs/`, agent-hand will automatically migrate existing profiles into `~/.agent-hand/` on startup when it detects the new directory has no sessions.

## CLI

```bash
# add a session (optional --cmd runs when starting the tmux session)
agent-hand add . -t "My Project" -g "work/demo" -c "claude"

# list sessions
agent-hand list

# status overview
agent-hand status -v

# start / attach
agent-hand session start <id>
agent-hand session attach <id>

# upgrade from GitHub Releases
agent-hand upgrade
```

## Notes

- Agent Hand uses a **dedicated tmux server** (`tmux -L agentdeck_rs`) so it won’t touch your default tmux.
- This dedicated tmux server defaults to `mode-keys vi` for copy-mode (config: `tmux.copy_mode = "emacs"|"off"`).
- tmux preview capture is intentionally **cached by default**; press `p` to refresh the snapshot when needed.
- Global config lives under `~/.agent-hand/` (legacy `~/.agent-deck-rs/` is still accepted).



### tmux basics (search/copy/paste)

Agent Hand is tmux-backed, so it helps to know a few tmux basics (defaults assume tmux prefix is `Ctrl+b`):

- Enter copy mode (scroll/search): `Ctrl+b` then `[`
- Search in copy mode: `/` then type, `Enter`; jump: `n` / `N`
- Copy selection (agent-hand default is `mode-keys vi`): `v`/`Space` to start selection, `y`/`Enter` to copy
  - If you prefer emacs keys, set `tmux.copy_mode = "emacs"`.
- Paste: `Ctrl+b` then `]`

Tip: agent-hand enables tmux mouse mode on its dedicated server, so you can often scroll with the mouse wheel.

### Activity Analytics (optional)

Track your session usage to understand your workflow patterns. When enabled, agent-hand records:
- Session enters (attach)
- Session exits (Ctrl+Q detach)
- Switcher usage

Enable in `~/.agent-hand/config.json`:

```json
{
  "analytics": {
    "enabled": true
  }
}
```

Logs are stored per-profile in `~/.agent-hand/profiles/<profile>/analytics/YYYY-MM-DD.jsonl` (JSONL format - one event per line for efficient append).

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## License

MIT
