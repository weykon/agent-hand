# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.1] - 2026-01-06

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
