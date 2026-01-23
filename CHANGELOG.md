# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.7] - 2026-01-21

### Fixed
- CI/release: update Cargo.lock so `--locked` builds succeed.

## [0.2.6] - 2026-01-21

### Added
- Windows: PowerShell installer (`install.ps1`) and clearer tmux installation guidance (recommend WSL).

## [0.2.10] - 2026-01-23

### Fixed
- tmux statusline: prevent overlapping runs from piling up shells/PTYs.

## [0.2.9] - 2026-01-21

### Added
- tmux statusline: daily cached update check; shows a short hint to run `agent-hand upgrade`.

## [0.2.5] - 2026-01-20

### Fixed
- CI/release: Windows build now succeeds (Windows no longer relies on Unix-only components).

## [0.2.4] - 2026-01-20

### Added
- CI/release: Windows build artifact (x86_64-pc-windows-msvc).

## [0.2.3] - 2026-01-20

### Changed
- Stop inferring tool type from command strings; rely on tags/labels.

## [0.2.2] - 2026-01-20

### Fixed
- Busy detection: avoid treating Copilot CLI footer hints like `ctrl+c ...` as RUNNING.

## [0.2.1] - 2026-01-20

### Fixed
- Busy detection: recognize Claude CLI variant `ctrl+c to interrupt`.

### Added
- Config: `status_detection` to customize WAITING/RUNNING detection via substrings or regex.
- Optional Claude Code hook to log user prompts (`claude.user_prompt_logging`).

## [0.1.16] - 2026-01-15

### Added
- Config: `ready_ttl_minutes` (default 40) to control how long a session stays Ready (✓) after Running.

## [0.1.15] - 2026-01-15

### Fixed
- Ctrl+N jump now uses dedicated `agent-hand jump` CLI command (fixes tmux env var issue).
- Jump excludes current session from targets, so pressing Ctrl+N in a ✓ session jumps to the next one.

## [0.1.14] - 2026-01-14

### Fixed
- tmux: Ctrl+N jump now refreshes priority target on-demand (works even if tmux status bar is off).

### Added
- tmux: dedicated server defaults to `mode-keys vi` for copy-mode (v/Space select, y/Enter copy).

## [0.1.13] - 2026-01-09

### Fixed
- CI: update Cargo.lock to match v0.1.13 for `--locked` builds.

## [0.1.12] - 2026-01-09

### Added
- TextInput component with cursor support for all dialog inputs.
- Left/Right arrow keys to move cursor within input fields.
- Home/End keys to jump to start/end of input.
- Delete key to remove character at cursor position.
- Visual cursor indicator (highlighted current character).

## [0.1.11] - 2026-01-09

### Fixed
- Config: Support multiple config paths (`~/.agent-hand/config.{json,toml}` and `~/.config/agent-hand/config.{toml,json}`).

### Removed
- Input-logging feature removed (script command cannot capture user input only).

## [0.1.10] - 2026-01-09

### Added
- Install script: Auto-detect and install tmux if missing (supports Homebrew, apt, dnf, yum, pacman, apk).
- Install script: `--skip-tmux` flag to bypass tmux installation check.
- TUI startup: Friendly error message with install instructions when tmux is not available.
- README: Prerequisites section with tmux installation commands.
- README: Session persistence documentation (tmux-resurrect/continuum).

### Fixed
- Config: Support multiple config paths (`~/.agent-hand/config.{json,toml}` and `~/.config/agent-hand/config.{toml,json}`).

## [0.1.9] - 2026-01-07

### Added
- Switcher tree view: Shows grouped sessions (like dashboard) when not searching, flat fuzzy results when typing.
- Activity analytics: Track session enter/exit/switch events. Enable via `config.json`: `{ "analytics": { "enabled": true } }`. Logs stored as JSONL in `~/.agent-hand/profiles/<profile>/analytics/`.

### Fixed
- New sessions now start with login shell (`$SHELL -l`) to ensure fresh shell config (`.zshrc`/`.bash_profile`) is loaded.

### Docs
- Added shell environment section explaining how config changes affect sessions and how to apply them.

## [0.1.8] - 2026-01-07

### Added
- Tag picker: Press `t` to apply an existing label+color combo to the selected session.
- Multi-instance safety: File locking for sessions.json prevents data corruption when running multiple instances.

### Fixed
- TUI no longer crashes when tmux operations fail (e.g., fork errors) - errors are now displayed in the preview pane.
- Binding deduplication: Skip redundant tmux key bindings if already correctly configured.

### Changed
- Session refresh is now non-fatal - tmux server issues no longer crash the dashboard.

## [0.1.7] - 2026-01-07

### Fixed
- TUI: Session tree list now scrolls to keep the selection visible in small terminals.

### Docs
- tmux hotkeys: Added notes about common key conflicts/equivalences and recommended sticking with defaults.
- Added a short tmux basics cheat sheet (search/copy/paste) to reduce onboarding friction.

## [0.1.6] - 2026-01-06

### Fixed
- Status: On `Ctrl+Q` detach, dashboard now forces an immediate probe for the detached session to reduce stale/incorrect status.

### Added
- tmux: `Ctrl+Q` now records `AGENTHAND_LAST_DETACH_AT` for detach-triggered UX.
- tmux: Added `get_environment_global()` helper to read tmux server env.

## [0.1.5] - 2026-01-06

### Added
- Status: `last_running_at` field persisted to storage - Ready (`✓`) now survives dashboard restarts.
- Status: Added `(Esc to cancel)` as busy indicator for Copilot/OpenCode.

### Changed
- Status: Ready (`✓`) is now based on persisted `last_running_at` timestamp, not just in-memory state transitions.
- Status: Fallback probe interval reduced from 60s to 10s for faster status updates on non-selected sessions.
- Status: First observation now immediately triggers a probe (fixes delayed status on dashboard startup).

### Fixed
- TUI: Switcher popup no longer shows all sessions as Running on initial load.
- Status: Sessions that were Running before dashboard restart now correctly show Ready (`✓`) if within 20 minutes.

## [0.1.4] - 2026-01-06

### Added
- TUI: Switcher popup list now shows live status icons (`!` WAITING / `●` RUNNING / `○` IDLE).

### Changed
- Status detection: Unified prompt/busy detection across tools (Claude/Copilot/OpenCode) based on terminal output patterns.
- Status: Waiting detection tightened to reduce false positives (e.g. OpenCode idle prompts).
- Status: Activity changes (attach/detach) no longer directly set Running; always rely on capture-pane content matching.
- Status: Ready (`✓`) now triggers when Running→Waiting or Running→Idle (not just Running→Idle).

### Fixed
- Status: Correctly detect Claude numbered confirmation prompts (`❯ 1. Yes` etc) as WAITING.
- Status: Correctly detect Copilot/Codex confirmation prompts as WAITING.
- TUI: Avoid stale/false READY (`✓`) on startup by not treating the initial activity baseline as Running.
- TUI: Fix false Running on attach/detach (activity change no longer means Running).

## [0.1.3] - 2026-01-06

### Added
- TUI: Create empty groups (`g`) via a filterable list + Enter to create.
- TUI: Group rename (`r` on a group) and move session to group (`m` on a session).
- TUI: Session edit (`r` on a session): title + label + label color.
- TUI: Group delete (`d` on a group):
  - empty group deletes immediately
  - non-empty shows 3 choices (delete group only / cancel / delete group + sessions)
- tmux: Dedicated tmux server (`tmux -L agentdeck_rs`) with:
  - `Ctrl+Q` detach
  - `Ctrl+G` popup session switcher (`agent-hand switch`, shows sessions immediately; type to filter)
- CLI: `agent-hand upgrade` to download and install the latest (or specified) GitHub Release.
- Docs: Preview image in README.

### Changed
- TUI: New Session flow simplified (default shell; group selection is a filterable list; default group comes from current selection).
- TUI: New Session title defaults to empty (falls back to directory name on create).
- TUI: Contextual key hints updated (e.g. `g` is consistently “create group”, `m` is “move” only for sessions).
- TUI: Title bar updated to match project name.
- TUI: Waiting indicator blinks (~1s on / ~0.3s off).
- Status: Waiting only triggers for blocked prompts (e.g. confirmations), not a plain `>` input prompt.
- TUI: After Running ends, show a temporary `✓` READY reminder (~20m) to help you notice and read the agent output.

### Fixed
- TUI: Unicode-safe fuzzy filtering/scoring (avoid UTF-8 slice panics).
- TUI: Edit session dialog no longer blocks typing `l` in the label field.
- tmux: `Ctrl+G` switcher shows sessions by default (no more empty view on open).
- TUI: Multiple dialog UX improvements (shorter footers, selection-aware actions).

## [0.1.0]

- Initial public release.
