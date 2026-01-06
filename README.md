# 🦀 Agent Deck (Rust) Agent Hand

A fast tmux-backed terminal session manager for AI coding agents.

> Agent Hand is a Rust rewrite inspired by the original Go open-source project
> [agent-deck](https://github.com/asheshgoplani/agent-deck).

Chinese README: [README.zh-CN.md](README.zh-CN.md)

![Preview](docs/preview.jpg)

## Highlights

- **TUI-first workflow**: run `agent-hand` and manage everything from the dashboard.
- **Groups**: create (`g`), rename (`r`), move session (`m`), delete (`d`, with safe options).
- **New Session UX**: path suggestions + group picker (filter + list selection).
- **Jump between running sessions**: inside tmux, `Ctrl+G` opens a popup switcher.
- **tmux QoL**: `Ctrl+Q` detaches back to the dashboard.
- **CLI + profiles** + self-upgrade (`agent-hand upgrade`).

## Install

### One-liner (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/weykon/agent-hand/master/install.sh | bash
```

By default it installs to `/usr/local/bin` (if writable), otherwise `~/.local/bin`.

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
- Session selected: `Enter` attach, `s` start, `x` stop, `r` rename, `R` restart, `m` move, `f` fork, `d` delete
- Group selected: `Enter` toggle, `g` create, `r` rename, `d` delete (empty = delete immediately; non-empty = confirm options)
- Global: `/` search, `p` capture preview snapshot, `?` help

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
- tmux preview capture is intentionally **cached by default**; press `p` to refresh the snapshot when needed.
- Global config lives under `~/.agent-hand/` (legacy `~/.agent-deck-rs/` is still accepted).

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## License

MIT
