# TUI Frontend Audit Report

**Date**: March 12, 2026
**Scope**: Complete user-accessible feature surface of agent-deck-rs TUI
**Status**: Comprehensive mapping complete

---

## 1. KEYBINDINGS - Default Actions

All keybindings are defined in `src/config.rs` and can be customized via `~/.agent-hand/config.toml`.

### Navigation & Core
| Key | Action | State | Tier |
|-----|--------|-------|------|
| `q` / `Q` / `Ctrl+C` | Quit (with confirmation) | Normal | Free |
| `↑` / `k` | Move selection up | Normal | Free |
| `↓` / `j` | Move selection down | Normal | Free |
| `Tab` | Cycle panel focus (tree → active → viewer → tree) | Normal | Pro |
| `←` (Left) | Collapse group/item | Normal | Free |
| `→` (Right) | Expand group/item | Normal | Free |
| `Space` | Toggle group expansion | Normal | Free |
| `Ctrl+D` | Page down (configurable lines, default 10) | Normal | Free |
| `Ctrl+U` | Page up (configurable lines, default 10) | Normal | Free |
| `?` | Show help modal | Normal | Free |

### Session Operations
| Key | Action | State | Tier |
|-----|--------|-------|------|
| `s` | Start session | Normal | Free |
| `x` | Stop session | Normal | Free |
| `R` (shift) | Restart session | Normal | Free |
| `u` | Resume session | Normal | Free |
| `n` | New session (dialog) | Normal | Free |
| `d` | Delete session (with confirm) | Normal | Free |
| `f` | Fork session (duplicate) | Normal | Free |
| `r` | Rename session (edit dialog) | Normal | Free |
| `m` | Move session to group (dialog) | Normal | Free |
| `t` | Tag picker dialog | Normal | Free |
| `Enter` | Select/attach to session | Normal | Free |

### Group Operations
| Key | Action | State | Tier |
|-----|--------|-------|------|
| `g` | Create group (dialog) | Normal | Free |

### Canvas & Visualization
| Key | Action | State | Tier |
|-----|--------|-------|------|
| `p` | Toggle canvas view (Pro) / Refresh preview (Free) | Normal | Pro/Free |
| `a` | Add selected session to canvas | Normal | Pro |

### AI Features (Max Tier)
| Key | Action | State | Tier | Requirements |
|-----|--------|-------|------|--------------|
| `A` (shift) | AI summarization (text overlay) | Normal | Max | AI provider configured |
| `B` (shift) | Behavior analysis on prompts | Normal | Max | AI provider configured |

### Other
| Key | Action | State | Tier |
|-----|--------|-------|------|
| `Ctrl+R` | Refresh | Normal | Free |
| `Ctrl+N` | Jump to next priority session | Normal | Free |
| `,` | Settings dialog | Normal | Free |
| `b` | Boost session (speed) | Normal | Free |
| `/` | Search/filter | Normal | Free |
| `,` | Open settings | Normal | Free |

### Canvas-Specific Keys (when canvas focused)
| Key | Action | Mode | Tier |
|-----|--------|------|------|
| `p` | Toggle back to tree | Canvas | Pro |
| `Enter` | Jump to session linked to node | Canvas | Pro |
| `j` / `k` | Navigate nodes | Canvas | Pro |
| `c` | Capture relationship context | Canvas | Pro |
| `a` | Annotate relationship | Canvas | Pro |
| `d` | Delete relationship | Canvas | Pro |
| `Ctrl+N` | New session from relationship context | Canvas | Pro |
| `Esc` | Exit canvas focus | Canvas | Pro |

### Overlay Navigation (Max Tier)
**AI Summary Overlay** (activated by `A` key, dismissed with `Esc`):
- `j` / `↓` / `d` / `PageDown` — Scroll down
- `k` / `↑` / `u` / `PageUp` — Scroll up
- `c` / `C` — Add summary to canvas
- `A` (reopen picker, falls through)

**AI Diagram Overlay** (activated by `A` key, dismissed with `Esc`):
- Same scroll controls as summary
- `c` / `C` — Add diagram to canvas

**Behavior Analysis Overlay**:
- `j` / `k` — Scroll
- `d` / `u` — Page scroll
- `Esc` — Close

---

## 2. APP MODES & STATES

Defined in `src/ui/app/mod.rs` as `enum AppState`:

| State | Description | Access | Tier |
|-------|-------------|--------|------|
| **Normal** | Main TUI dashboard (session list + canvas/preview) | Default | Free |
| **Search** | Interactive search/filter overlay | `/` key | Free |
| **Dialog** | Modal dialog for various operations | Various | Free |
| **Help** | Help modal overlay | `?` key | Free |
| **Relationships** | Relationship view/editor (Pro only) | Dedicated mode | Pro |
| **ViewerMode** | Full-screen remote PTY viewer | Enter on viewer session | Pro |

### Main Dashboard Panels

**Free Tier:**
1. **Session Tree** (Left 45%) — Hierarchical view of sessions and groups
2. **Preview Panel** (Right 55%) — Session details, logs, etc.

**Pro Tier:**
1. **Session Tree** (Left 45%)
2. **Canvas** (Right 55%) — Visual workflow editor/diagram
3. **Active Sessions Panel** (Overlaid, toggleable via navigation)
4. **Viewer Sessions Panel** (Overlaid, for remote PTY viewers)

---

## 3. DIALOG TYPES - User Entry Points

All dialog types defined in `src/ui/dialogs.rs`:

### Free Tier Dialogs
| Dialog | Trigger | Purpose | Fields |
|--------|---------|---------|--------|
| **NewSession** | `n` key | Create new session | path, title, group (with autocomplete) |
| **DeleteConfirm** | `d` key → Confirm | Confirm session deletion | session_id, title, kill_tmux option |
| **DeleteGroup** | Group context → Delete | Delete group & children | choice: keep sessions / delete all |
| **Fork** | `f` key | Create session fork | parent_id, title, group |
| **CreateGroup** | `g` key | Create new group | group_path (fuzzy matched) |
| **MoveGroup** | `m` key | Move session to group | session_id, group_path (fuzzy) |
| **RenameGroup** | Group context → Rename | Rename group | old_path, new_path |
| **RenameSession** | `r` key | Edit session | title, label, label_color |
| **TagPicker** | `t` key | Add/edit tags | tag selection from list |
| **Settings** | `,` key | Global configuration | Various settings (see below) |
| **QuitConfirm** | `q` key → Confirm | Confirm application exit | (confirmation only) |
| **PackBrowser** | Sounds/skills context | Browse sound packs | pack selection (Free: sound packs only) |

### Pro Tier Dialogs
| Dialog | Trigger | Purpose | Fields |
|--------|---------|---------|--------|
| **Share** | Session context / `agent-hand share` CLI | Configure session sharing | permission (ro/rw), expire_minutes, URL generation |
| **CreateRelationship** | Canvas context | Create session relationship | session_a, session_b, type, label |
| **Annotate** | Canvas → selected edge | Add note to relationship | relationship_id, note text |
| **NewFromContext** | Relationship context (`Ctrl+N`) | Create session from relationship | relationship_id, title, context preview, injection method |
| **JoinSession** | `agent-hand join <URL>` CLI | Join remote shared session | relay validation, room_id, token |
| **DisconnectViewer** | Viewer panel → `d` key | Disconnect from viewer | room_id confirmation |
| **ControlRequest** | External control socket | Handle incoming control ops | Operation processing (async) |
| **OrphanedRooms** | Session startup (from relay) | Handle sessions without local record | room listing, selection for cleanup |
| **SkillsManager** | `K` key / "Skills Browser" | Browse & manage skills library | GitHub repo browser, sync controls |

### Max Tier Dialogs
| Dialog | Trigger | Purpose | Fields |
|--------|---------|---------|--------|
| **AiAnalysis** | `A` key with provider configured | Run summarization (async) | shows overlay result |
| **BehaviorAnalysis** | `B` key with provider configured | Analyze behavior patterns | shows overlay analysis |

---

## 4. SESSION OPERATIONS

Implemented in `src/ui/app/sessions.rs` and `src/ui/app/control.rs`:

### Lifecycle Operations
| Operation | Trigger | Backend Call | Tier | Notes |
|-----------|---------|--------------|------|-------|
| **Create** | `n` dialog → submit | `Instance::new()` + storage.save() | Free | Creates tmux session |
| **Delete** | `d` key → confirm | Kill tmux + remove from storage | Free | Irreversible |
| **Start** | `s` key | tmux new-session | Free | Creates PTY |
| **Stop** | `x` key | tmux kill-session | Free | Stops running session |
| **Restart** | `R` key | Kill + re-create | Free | Atomic operation |
| **Resume** | `u` key | Attach to session (PTY viewer) | Free | Restores terminal state |
| **Interrupt** | Control socket op | Send Ctrl+C to tmux | Free | Stop running command |
| **Fork** | `f` dialog → submit | Clone with new ID, same cmd/tool | Free | Parent link tracked |

### Metadata Operations
| Operation | Trigger | Backend Call | Tier | Notes |
|-----------|---------|--------------|------|-------|
| **Rename** | `r` key → edit | Update title + tmux session name | Free | Auto-renames tmux |
| **Move** | `m` dialog → select | Update group_path | Free | Affects tree display |
| **Label** | `r` key → label field | Set label + color | Free | Visual tagging |
| **Tag** | `t` key → picker | Add/remove tags from session.tags | Free | Multiple tags per session |

### Remote Sharing (Pro)
| Operation | Trigger | Backend Call | Tier | Notes |
|-----------|---------|--------------|------|-------|
| **Share** | Session context or CLI | Start tmate/relay server | Pro | Generates share URL |
| **Unshare** | Share context | Stop relay connection | Pro | Revokes access |
| **Capture Context** | Canvas edge (`c` key) | Collect session progress for relationship | Pro | Boundary capture |

### AI Operations (Max)
| Operation | Trigger | Backend Call | Tier | Requirements |
|-----------|---------|--------------|------|--------------|
| **Summarize** | `A` key | AI provider (deepseek/claude/etc) | Max | API key configured |
| **Behavior Analysis** | `B` key | Collect prompts → AI analysis | Max | Prompts logged in session |
| **Generate Diagram** | Summarize context → diagram gen | Canvas projection | Max | AI provider |

---

## 5. CANVAS OPERATIONS (Pro Only)

Canvas system in `src/ui/canvas/mod.rs`. All operations Pro-only (Free: read-only preview).

### Node Operations
| Operation | Trigger | Canvas Effect | Data Stored |
|-----------|---------|---------------|-------------|
| **AddNode** | Manual placement or AI generation | Creates node on canvas | id, label, kind, position, content |
| **RemoveNode** | Delete key on selected node | Remove from graph | (deletion) |
| **UpdateNode** | Edit mode | Modify node properties | label, kind, position, content |

### Edge Operations
| Operation | Trigger | Canvas Effect | Data Stored |
|-----------|---------|---------------|-------------|
| **AddEdge** | Relationship created or manual draw | Draw edge between nodes | from, to, label, relationship_id |
| **UpdateEdge** | Edit label/type | Modify edge properties | label |
| **RemoveEdge** | Delete on edge (`d` key) | Remove edge | (deletion) |

### Global Canvas Operations
| Operation | Trigger | Effect | Tier |
|-----------|---------|--------|------|
| **Layout** | Layout menu or auto-applied | Arrange nodes (TopDown/LeftRight) | Pro |
| **Undo** | Standard undo | Revert last operation | Pro |
| **Redo** | Standard redo | Reapply operation | Pro |
| **Query** | Search/filter operations | Find nodes/edges by criteria | Pro |
| **Batch** | Multiple ops in sequence | Atomic batch apply | Pro |
| **Save/Load** | TUI exit/startup | Persist to JSON file | Pro |

### Canvas Node Types
- **Start** — Green rounded border (▶ indicator)
- **End** — Red box border (■ indicator)
- **Process** — Cyan plain border (no indicator)
- **Decision** — Yellow double border (◇ indicator)
- **Note** — Gray plain border (# indicator, multi-line content)

### Special Node Linking
- **Session Nodes**: Linked to session ID → shows live session status (color updates)
- **AI Nodes**: Generated from summarization/behavior analysis → tracks source session
- **Relationship Edges**: Can link to session relationship records for context capture

---

## 6. SEARCH & FILTER

Implemented in `src/ui/app/navigation.rs`:

| Feature | Trigger | Behavior | Scope |
|---------|---------|----------|-------|
| **Text Search** | `/` key | Fuzzy match on session titles, groups | Session tree |
| **Tag Filter** | Search + tag input | Filter by applied tags | Sessions |
| **Status Filter** | Search + status filter | Running, Waiting, Idle, Error, Ready | Sessions |
| **Group Jump** | Autocomplete in dialogs | Fuzzy match group paths | All group fields |
| **Path Autocomplete** | NewSession dialog | Suggest ~/projects directories | Path field |

---

## 7. SETTINGS DIALOG

Triggered by `,` key. Configurable options:

### Display Settings
- **Language** — English / Chinese
- **Mouse Capture** — Auto / On / Off
- **Scroll Padding** — Lines to keep from edge (default: 5)
- **Jump Lines** — Ctrl+D/U distance (default: 10)
- **Ready TTL** — Minutes to keep session as "Ready" (default: 40)

### Pro Tier Settings
- **Skills Library** — Auto-sync, repo URL override
- **Sound Notifications** — Enable/volume/pack/event triggers
  - Task complete, input required, error, session start, task acknowledge, resource limit, user spam
  - Quiet when focused option
- **Relationship Features** — Enable/settings

### Max Tier Settings
- **AI Provider** — Select (deepseek/claude/ollama)
- **Model Override** — Custom model name
- **API Key** — Manual entry (or env var fallback)
- **Base URL** — Proxy/self-hosted support
- **Summary Lines** — Default: 200

### Features
- **Canvas Projection** — Enable/disable AI-generated node visualization
- **Hooks Auto-Register** — Auto-register with Claude/Cursor/Windsurf hooks
- **Analytics** — Enable usage tracking
- **Context Bridge** — Guarded context injection config

---

## 8. RELATIONSHIP MANAGEMENT (Pro Only)

Separate UI mode accessed via dedicated navigation or right-panel button.

### Relationship Types
- **ParentChild** — Hierarchical dependency
- **Peer** — Collaborative sessions
- **Dependency** — One session depends on another
- **Collaboration** — Working together on same task
- **Custom** — User-defined label

### Relationship Operations
| Operation | Trigger | Effect |
|-----------|---------|--------|
| **Create** | Dedicated dialog | Add relationship between 2 sessions |
| **Delete** | Canvas edge delete or relationship panel | Remove link |
| **Annotate** | Canvas edge + `a` key | Add notes to relationship |
| **Bidirectional** | Creation option | Make relationship two-way |
| **Capture Context** | Canvas edge + `c` key | Extract progress from both sessions for this relationship |

### Relationship Panel
- **Orphaned Rooms** — Shows sessions with relay connections but no local record
- **Remote Viewers** — List of active viewer connections (can disconnect with `d`)

---

## 9. REMOTE SHARING & COLLABORATION (Pro Only)

### Share Modes
| Mode | Transport | Tier | URL Format |
|------|-----------|------|-----------|
| **tmate** | SSH relay at tmate.io (default) | Pro | ssh://tmate.io/... |
| **Relay** | WebSocket relay (configured) | Pro | https://relay.../share/ROOM?token=... |

### Share Permissions
- **ro** — Read-only viewer access
- **rw** — Read-write (guest can type)

### Share Configuration
- **Auto-expire** — Set in config or per-share dialog
- **Default permission** — Config setting for new shares
- **Relay discovery** — Auto-detect best relay via config URL

### Viewer Mode (Pro)
- Full-screen PTY streaming from remote relay
- Real-time terminal output rendering
- Viewer list in active panel

---

## 10. SKILLS LIBRARY (Pro Only)

Browser accessed via `K` key or skills dialog.

| Feature | Trigger | Effect | Backend |
|---------|---------|--------|---------|
| **Browse** | Skills dialog | List available skills from registry | GitHub raw content |
| **Sync** | Auto on startup or manual | Update local skills cache | Git pull or API fetch |
| **Search** | Skills panel | Filter skills by name | Local cache search |
| **View Details** | Select skill | Show metadata and docs | Skill YAML parsing |

---

## 11. SOUND NOTIFICATIONS (Pro Only)

Configured in settings, uses CESP format (Coding Event Sound Pack).

### Trigger Events
- **Task complete** (Running → Idle)
- **Input required** (→ Waiting)
- **Tool failure** (Error status)
- **Session start** (non-Running → Running)
- **Task acknowledge** (new prompt while running)
- **Resource limit** (context window compact)
- **User spam** (rapid prompts)

### Configuration
- **Pack selection** — ~/.openpeon/packs/ or ~/.agent-hand/packs/
- **Volume** — 0.0–1.0 (default: 0.5)
- **Quiet when focused** — Suppress sound when session is actively attached

---

## 12. CLI COMMANDS

All commands defined in `src/cli/commands.rs`:

### Session Management
```
agent-hand add [PATH] [--title TITLE] [--group GROUP] [--cmd COMMAND]
agent-hand list [--json] [--all]
agent-hand remove <SESSION_ID>
agent-hand status [--verbose] [--quiet] [--json]
```

### Session Lifecycle (via socket or direct)
```
agent-hand session start <ID>
agent-hand session stop <ID>
agent-hand session restart <ID>
agent-hand session resume <ID>
agent-hand session interrupt <ID>
agent-hand session send <ID> <TEXT>
```

### Sharing (Pro)
```
agent-hand share <SESSION_ID> [--permission ro|rw] [--expire MINUTES]
agent-hand unshare <SESSION_ID>
agent-hand join <SHARE_URL>
```

### Skills (Pro)
```
agent-hand skills list
agent-hand skills sync
agent-hand skills view <SKILL_NAME>
```

### Canvas (Pro)
```
agent-hand canvas add-node <GROUP> <LABEL> [--kind START|END|PROCESS|DECISION|NOTE]
agent-hand canvas add-edge <FROM> <TO> [--label LABEL]
agent-hand canvas query <FILTER>
agent-hand canvas save <GROUP>
agent-hand canvas load <GROUP>
```

### Authentication
```
agent-hand login
agent-hand logout
agent-hand account [--refresh]
agent-hand devices [--remove ID]
```

### Profiles & Switcher
```
agent-hand switch                    # Interactive tmux switcher
agent-hand jump                      # Jump to session
agent-hand profile list
agent-hand profile create <NAME>
agent-hand profile delete <NAME>
```

### Version & Help
```
agent-hand --version
agent-hand statusline [--profile PROFILE]
agent-hand upgrade [--prefix PATH] [--version VERSION]
```

---

## 13. CONTROL SOCKET API

External tools can send operations via Unix socket at `~/.agent-hand/sockets/control.sock`.

### Control Operations (ControlOp enum)

**Session CRUD:**
- `AddSession` — Create session
- `RemoveSession` — Delete session
- `ListSessions` — Query sessions (filter by group/tag/status)
- `SessionInfo` — Get details of one session

**Lifecycle:**
- `StartSession` / `StopSession` / `RestartSession` / `ResumeSession` / `InterruptSession`
- `SendPrompt` — Send text input to session

**Metadata:**
- `RenameSession` — Change title
- `SetLabel` — Apply label + color
- `MoveSession` — Change group
- `AddTag` / `RemoveTag` — Manage tags

**Groups:**
- `ListGroups` — All groups
- `CreateGroup` / `DeleteGroup` / `RenameGroup`

**Relationships (Pro):**
- `AddRelationship` — Create session link
- `RemoveRelationship` — Delete link
- `ListRelationships` — Query by session

**Inspection:**
- `ReadPane` — Get last N lines of session output (default: 30)
- `ReadProgress` — Get progress file content

**Batch:**
- `Batch { ops: Vec<ControlOp> }` — Atomic multi-op

---

## 14. FEATURE TIER SUMMARY

### Free Tier
- Session CRUD (create, delete, rename, tag, label, move, fork)
- Group management (create, rename, delete)
- Session lifecycle (start, stop, restart, resume, interrupt)
- Search & filter
- Settings (basic: language, scroll, mouse)
- Sound notifications (basic pack browser only)
- CLI commands (session/profile/auth management)
- Control socket API (read/write operations)

### Pro Tier (adds)
- Canvas workflow editor with nodes/edges
- Session visualization on canvas
- Relationships between sessions (4 types)
- Remote sharing via tmate/relay
- Viewer mode (PTY streaming)
- Skills library browser & management
- Context capture & injection
- Sound notifications (custom packs, event selection)
- Relationship panel UI
- Active sessions & viewer panels

### Max Tier (adds)
- AI summarization (`A` key)
- Behavior analysis on prompts (`B` key)
- AI diagram generation
- AI-generated canvas nodes
- Configurable AI provider (DeepSeek/Claude/Ollama)
- Custom API keys & base URLs

---

## 15. RENDER & DISPLAY ARCHITECTURE

Implemented in `src/ui/render/mod.rs` and sub-modules:

### Layout Structure
```
┌─────────────────────────────────────────────┐
│          Title Bar (3 lines)                 │
├────────────────┬────────────────────────────┤
│                │  Info Bar (1 line, optional)│
├────────────────┼────────────────────────────┤
│                │                            │
│  Session Tree  │   Canvas / Preview Panel   │
│  (45%)         │   (55%)                    │
│                │                            │
│                │  (Pro: Canvas, Free: Info) │
│                │                            │
├────────────────┴────────────────────────────┤
│          Status Bar (3 lines)                │
└─────────────────────────────────────────────┘
```

### Session Tree Display (45% left panel)
- Hierarchical group/session structure
- Session status indicator (●Running, ◆Waiting, ✓Idle, ✗Error, ⊗Ready)
- Labels with colors
- Tag display
- Parent session indicator (fork link)

### Preview/Canvas Panel (55% right panel)
**Free:** Session information, command, recent output
**Pro:** Full canvas editor with visual nodes & edges

### Status Bar Information
- Current tier (Free/Pro/Max)
- Selected session name
- Session status
- Group path
- Active panel indicator
- Keyboard hint (press ? for help)

### Overlays & Modals
- **Help Modal** — Full keybindings reference
- **Dialogs** — Modal input forms with validation
- **Toast Notifications** (Pro) — Brief status notifications
- **AI Overlays** (Max) — Summary/diagram/behavior output in scrollable pane
- **Search Popup** — Interactive text input overlay
- **Onboarding Welcome** — First-launch guide

---

## 16. SPECIAL FEATURES

### Agent Brain / Projections (Max)
- **Canvas projection enabled** by default (configurable)
- AI-generated nodes for session summaries/diagrams
- Nodes track source session & AI analysis type
- Real-time projection updates as AI results arrive

### Event Bridge Integration
- Hooks for Claude Code, Cursor, Codex, Windsurf
- Auto-registration on startup (configurable)
- Status detection via configured regex patterns
- Event-driven status updates to session state

### Context Bridge (Guarded Injection)
- Injects session progress into external tools on hook events
- Configurable scope: self_only / same_group / confirmed_relations
- Cooldown to prevent spam (default: 5 seconds)
- Max lines & char limits to prevent bloat

### Mouse Support
- Click to select sessions/groups
- Drag to pan canvas (Pro)
- Scroll for pagination
- Mode: Auto (detect), On, Off

---

## 17. ACCESSIBILITY & INTERNATIONALIZATION

### Languages
- English
- 中文 (Simplified Chinese)

### UI Elements with i18n
- Title bar
- Help hints
- Dialog labels
- Status messages
- Error messages

### Accessibility Considerations
- Full keyboard navigation (vim keys: hjkl)
- Terminal-native mouse selection (when disabled)
- High contrast terminal theme support
- Screen reader compatibility (basic)

---

## 18. GAPS & OBSERVATIONS

### Features Exposed via Frontend
✅ Session CRUD (complete coverage)
✅ Group management (complete coverage)
✅ Canvas operations (Pro: visual, Free: blind via CLI only)
✅ Relationships (Pro: fully exposed, Free: N/A)
✅ Remote sharing (Pro: initiated via dialog, Free: N/A)
✅ AI analysis (Max: via overlays, Free/Pro: N/A)
✅ Skills library (Pro: browsable, Free: N/A)
✅ Sound notifications (Pro: configurable, Free: N/A)
✅ Settings (complete per tier)

### Features NOT Exposed via TUI
❌ Batch operations (only via control socket / CLI)
❌ Canvas import/export (persisted to JSON, not UI-exposed)
❌ Profile creation/deletion (CLI only)
❌ Advanced canvas queries (CLI only)
❌ WebSocket transport configuration (Settings not exposed)
❌ Advanced hook configuration (Not in settings dialog)
❌ Context bridge scope editing (Config file only)

### Potential UX Issues
- Canvas-to-tree context switching via `p` key (easy to miss)
- Relationship panel vs. main view (separate mode, not integrated)
- AI overlays (Max) scroll interface could be more discoverable
- No visual indicator of tier restrictions in dialogs
- Skills sync (Pro) requires manual trigger or startup config

---

## 19. EXTERNAL INTEGRATIONS

### Authentication
- Asymptai OAuth flow (login dialog)
- Token persistence (auth.json)
- Tier detection from token

### AI Providers (Max)
- DeepSeek (default)
- Claude (via Anthropic API)
- Ollama (self-hosted)
- Custom base URL support

### Hooks & Event Bridge
- Claude Code hooks
- Cursor CLI hooks
- Codex CLI hooks
- Windsurf CLI hooks
- Status detection regex

### Remote Sharing
- tmate.io relay (default)
- Custom relay server support
- Relay discovery protocol

### WebSocket (Max)
- Real-time agent brain updates
- Canvas projection streaming
- Configurable WS server URL

---

## 20. DATA FLOWS

### Session Creation
```
TUI (NewSession dialog) → validation → Instance::new() → storage.save() → tmux new-session
```

### Canvas Operation
```
Canvas input → canvas::input::handle_canvas_input() → CanvasOp → CanvasState (in-memory)
↓
Periodic sync → storage.save() (persist JSON)
```

### AI Summarization
```
TUI ('A' key) → AI provider API → text result → show_ai_summary_overlay = true
↓
User ('C' key) → add_summary_to_canvas() → CanvasOp::AddNode with ai_source_session
```

### Remote Sharing
```
TUI (Share dialog) → start_share_task() → tmate/relay spawn → generate URL → copy to clipboard
↓
External tool (agent-hand join URL) → validate → PTY viewer mode
```

---

## Report Generated By
Frontend Auditor on gap-audit team.
See memory files for implementation details:
- `canvas-architecture.md` — Canvas system design
- `feature-tier-roadmap.md` — Tier gating and roadmap
- `tmux-capabilities.md` — Session management capabilities
