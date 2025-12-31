# Agent Hand

> Agent Hand is a Rust rewrite inspired by the original Go open-source project
> [agent-deck](https://github.com/asheshgoplani/agent-deck).

Chinese README: [README.zh-CN.md](README.zh-CN.md)

## What it is

Agent Hand is a terminal session manager (tmux-based) for AI coding agents, with a CLI and a TUI (work-in-progress but already usable).

## Quick start

```bash
git clone https://github.com/weykon/agent-hand.git agent-hand
cd agent-hand
cargo build --release

# optional
cargo install --path .
```

## Usage

```bash
# add a session for current project
agent-hand add . -t "My Project" -c claude

# list sessions
agent-hand list

# status overview
agent-hand status -v

# start / attach
agent-hand session start <id>
agent-hand session attach <id>
```

## Notes

- tmux preview capture is intentionally **cached by default** (high-cost/low-benefit to capture pane on every selection change); refresh snapshot manually when needed.
- Global config lives under `~/.agent-hand/` (legacy `~/.agent-deck-rs/` is still accepted).

## License

MIT
