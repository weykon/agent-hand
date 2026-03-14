# Agent-Hand Architecture Review - Current State

**Date**: 2026-03-13
**Scope**: Actual codebase implementation (not specs)
**Focus**: Module dependencies, data flows, file contracts, feature gates, entry points

---

## 1. Module Dependency Graph

### Core Hierarchy

```
src/lib.rs (root exports)
├── agent/ (ECS event processing framework)
│   ├── mod.rs (System trait, World, Action, ProgressEntry, event_to_status)
│   ├── runner.rs (SystemRunner + ActionExecutor — event loop)
│   ├── systems/ (built-in reactive Systems)
│   │   ├── context.rs (ContextGuardSystem — guard pipeline)
│   │   ├── progress.rs (ProgressSystem — write progress logs)
│   │   └── sound.rs (SoundSystem — play notifications)
│   ├── guard.rs (Proposal, Evidence, GuardedCommit, guard decision logic)
│   ├── hot_brain.rs (WorldSlice types, CandidateSet, analysis types)
│   ├── consumers.rs (SchedulerNormalizedOutput, MemoryIngestEntry normalization)
│   ├── scheduler.rs (SchedulerRecord, SchedulerState, FollowupProposalRecord)
│   ├── memory.rs (5-layer memory model: Audit→Evidence→Packet→Candidate→ColdMemory)
│   ├── projections.rs (view models: RelationshipViewModel, SchedulerViewModel)
│   └── io.rs (load_jsonl helper)
├── ui/ (TUI application)
│   ├── app/mod.rs (App struct, main event loop, dialog handling)
│   ├── canvas/ (workflow editor)
│   │   ├── mod.rs (NodeKind, NodeData, EdgeData, DiGraph)
│   │   ├── socket.rs (CanvasSocketServer — Unix socket for external canvas control)
│   │   ├── projection.rs (canvas view model builders)
│   │   ├── render/ (ratatui rendering)
│   │   └── input/ (canvas keyboard/mouse handling)
│   ├── render/ (rendering layer)
│   └── dialogs.rs (all dialog types)
├── session/ (data model)
│   ├── mod.rs (re-exports)
│   ├── storage.rs (Storage, StorageData, sessions.json persistence)
│   ├── instance.rs (Instance, Status, LabelColor)
│   ├── groups.rs (GroupData, GroupTree)
│   ├── relationships.rs (Relationship, RelationType)
│   └── context.rs (ContextBridgeConfig, injection scope)
├── control/ (external control interface)
│   ├── mod.rs (ControlOp enum — session/group/rel CRUD)
│   └── socket.rs (ControlSocketServer — Unix socket)
├── tmux/ (session lifecycle)
│   └── manager.rs (TmuxManager — start/stop/attach sessions)
├── hooks/ (event definitions)
├── config/ (NotificationConfig, ContextBridgeConfig, KeyBindings)
├── notification/ (sound playback)
├── cli/ (CLI commands)
└── bin/bridge.rs (agent-hand-bridge — lightweight IPC binary)
```

### Key Integration Points

1. **SystemRunner ← World ← HookEvent**
   - HookEvent comes from broadcast channel or stdin (bridge)
   - SystemRunner.update_from_event() updates World
   - Systems read World (immutable), emit Actions

2. **ActionExecutor → File I/O**
   - Writes progress files: `~/.agent-hand/profiles/{profile}/progress/{session}.md`
   - Writes audit trail: `~/.agent-hand/profiles/{profile}/agent-runtime/*.jsonl`
   - Calls run_coordination_pipeline() on FeedbackPacket

3. **UI ← Storage → sessions.json**
   - App loads Storage at startup
   - UI dialogs (create/delete/rename) → Storage.update_data()
   - Stores: instances, groups, relationships, updated_at

4. **Canvas ← Socket ← External Tools**
   - CanvasSocketServer listens on `~/.agent-hand/canvas.sock`
   - External tools send CanvasOp JSON, receive CanvasResponse
   - App processes and persists canvas state

5. **Control ← Socket ← Bridge/Scripts**
   - ControlSocketServer listens on `~/.agent-hand/control.sock`
   - Bridge sends ControlOp (session CRUD, group ops, rel ops)
   - Syncs with Storage and refreshes UI

---

## 2. Data Flow Architecture

### Flow 1: Tmux Event → Agent Brain → File Audit

```
┌─────────────────────────────────────────────────────────────┐
│ Tmux Hook (status change, prompt submit, stop, pre-compact) │
└────────────────────┬────────────────────────────────────────┘
                     │ executes
                     ▼
┌────────────────────────────────────────────────────────────────┐
│ agent-hand-bridge (lightweight sync binary)                    │
│ - Normalizes env to HookEvent                                  │
│ - Sends via broadcast channel (if running) or JSONL fallback   │
│ - Exit code 0 (never fail loudly in hook mode)                 │
└────────────────────┬───────────────────────────────────────────┘
                     │ broadcast
                     ▼
┌────────────────────────────────────────────────────────────────┐
│ SystemRunner (tokio task)                                      │
│ - recv_broadcast(HookEvent)                                    │
│ - world.update_from_event() — updates per-session state       │
│ - for system in systems: system.on_event(event, &world)       │
└────────────┬──────────┬──────────┬────────────────────────────┘
             │          │          │
    ┌────────▼──┐  ┌────▼──────┐  ┌─────▼────────────┐
    │ Progress  │  │ ContextG. │  │ Sound            │
    │ System    │  │ System    │  │ System           │
    └────────┬──┘  └────┬──────┘  └─────┬────────────┘
             │          │              │
    ┌────────▼──┐  ┌────▼──────┐  ┌─────▼────────────┐
    │WriteProgress
    │ Action    │  │GuardedCx  │  │ PlaySound        │
    │           │  │Injection  │  │ Action           │
    └────────┬──┘  └────┬──────┘  └─────┬────────────┘
             │          │              │
             └──────────┬──────────────┘
                        │ send via mpsc
                        ▼
┌────────────────────────────────────────────────────────────────┐
│ ActionExecutor (tokio task)                                    │
│ - recv(Action)                                                 │
│ - match action:                                                │
│   - PlaySound → notification_manager.play_category()           │
│   - WriteProgress → append to progress/{session}.md            │
│   - GuardedContextInjection →                                  │
│       append_audit(proposals.jsonl, evidence.jsonl, etc.)     │
│       if Approve: run_coordination_pipeline()                  │
└────────────────────────────────────────────────────────────────┘
```

**Files written:**
- `~/.agent-hand/profiles/{profile}/progress/{session}.md` (markdown log)
- `~/.agent-hand/profiles/{profile}/agent-runtime/proposals.jsonl`
- `~/.agent-hand/profiles/{profile}/agent-runtime/evidence.jsonl`
- `~/.agent-hand/profiles/{profile}/agent-runtime/commits.jsonl`
- `~/.agent-hand/profiles/{profile}/agent-runtime/feedback_packets.jsonl` (if approved)

---

### Flow 2: Guard Pipeline → Proposal → Audit Trail

```
┌──────────────────────────────────────────────────────────────┐
│ ContextGuardSystem.on_event()                                │
│ - Triggered on: user_prompt_submit, stop, pre_compact, etc   │
│ - Builds Proposal { kind: InjectContext, ... }               │
│ - Builds Evidence[] { kind: RiskAnalysis, Attestation, ... } │
└────┬────────────────────────────────────────────────────────┘
     │
     ▼
┌──────────────────────────────────────────────────────────────┐
│ guard::eval_decisions()                                       │
│ - Pure deterministic function (no I/O, no side effects)      │
│ - 8 checks: scope, cooldown, risk level, dedup, etc.        │
│ - Decision: Approve or Block                                 │
│ - Returns GuardedCommit { decision, checked_at, ... }       │
└────┬────────────────────────────────────────────────────────┘
     │
     ▼
┌──────────────────────────────────────────────────────────────┐
│ Action::GuardedContextInjection emitted                       │
│ - Contains: proposal, evidence[], commit, feedback_packet    │
└────┬────────────────────────────────────────────────────────┘
     │
     ▼
┌──────────────────────────────────────────────────────────────┐
│ ActionExecutor.execute()                                     │
│ - Always: append_audit(proposals.jsonl, proposal)           │
│ - Always: append_audit(evidence.jsonl, evidence[])          │
│ - Always: append_audit(commits.jsonl, commit)               │
│ - If Approve:                                                │
│     - inject_context(&session_key, &project_path)           │
│     - if feedback_packet: append_audit(feedback_packets)    │
│     - run_coordination_pipeline(packet)                      │
│ - If Block: only audit (no injection)                        │
└──────────────────────────────────────────────────────────────┘
```

---

### Flow 3: User Dialog → Storage Update → sessions.json

```
┌──────────────────────────────────────┐
│ User action (create/delete/rename)   │
└────────┬─────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────┐
│ UI Dialog Handler (e.g., CreateSessionDialog)   │
│ - Collect user input (title, path, group, etc.) │
│ - app.handle_dialog_result()                    │
└────────┬─────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────┐
│ Storage.update_data() or Storage.remove_instance │
│ - Modifies StorageData in memory                │
│ - Creates backup (max 3 generations)            │
│ - Writes to sessions.json atomically            │
└────────┬─────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────┐
│ ~/.agent-hand/profiles/default/sessions.json    │
│ {                                                │
│   "instances": [...],                           │
│   "groups": [...],                              │
│   "relationships": [...],                       │
│   "updated_at": "2026-03-13T..."                │
│ }                                                │
└──────────────────────────────────────────────────┘
```

---

### Flow 4: Canvas Operations → Persistent State

```
┌─────────────────────────────────────────────────┐
│ External tool: agent-hand-bridge canvas '{json}'│
│ or direct socket connection to canvas.sock      │
└────────┬────────────────────────────────────────┘
         │ JSON CanvasOp
         ▼
┌──────────────────────────────────────────────────┐
│ CanvasSocketServer.spawn_listener()              │
│ (Unix domain socket: ~/.agent-hand/canvas.sock) │
└────────┬───────────────────────────────────────┘
         │ (op, reply_channel)
         ▼
┌──────────────────────────────────────────────────┐
│ App event loop: canvas_rx.recv()               │
│ - process_canvas_op(op)                         │
│ - canvas_state.apply_op()  [updates DiGraph]   │
│ - send(CanvasResponse) back via reply_channel  │
└────────┬───────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────┐
│ [Optional] Periodic save or on_exit:            │
│ ~/.agent-hand/profiles/{profile}/canvas/       │
│   {group_id}.json                              │
│ [NodeData[], EdgeData[]]                       │
└──────────────────────────────────────────────────┘
```

---

## 3. Runtime File Contracts

### `~/.agent-hand/profiles/{profile}/sessions.json`

**Reader**: Storage::load_data(), App::new()
**Writer**: Storage::update_data() (on dialog actions)
**Format**: JSON

```json
{
  "instances": [
    {
      "id": "uuid",
      "title": "My Session",
      "group_path": "/group/name",
      "command": "cargo build",
      "working_dir": "/home/user/project",
      "status": "Idle" | "Running" | "Waiting",
      "labels": [{ "name": "tag", "color": "Red" }],
      "created_at": "ISO8601",
      "last_started": "ISO8601"
    }
  ],
  "groups": [
    {
      "path": "/group/name",
      "label": { "name": "tag", "color": "Blue" }
    }
  ],
  "relationships": [
    {
      "id": "uuid",
      "session_a_id": "uuid",
      "session_b_id": "uuid",
      "relation_type": "Blocks" | "Depends" | "Related",
      "label": "optional description",
      "confirmed": true | false
    }
  ],
  "updated_at": "2026-03-13T14:30:45.123Z"
}
```

**Backups**: Keep max 3 generations of sessions.json.bak{1,2}

---

### `~/.agent-hand/profiles/{profile}/agent-runtime/*.jsonl`

**Readers**: Projections (for view models), explicit JSON parsing
**Writers**: ActionExecutor.append_audit()
**Format**: JSONL (one JSON object per line)

#### `proposals.jsonl`
```json
{ "id": "uuid", "kind": "InjectContext", "source_session": "session-key", "scope": "SelfOnly", "created_at_ms": 1710417045000 }
```
**Consumed by**: Audit layer (spec 19)

#### `evidence.jsonl`
```json
{ "id": "uuid", "kind": "RiskAnalysis", "risk_level": "Low" | "Medium" | "High" | "Critical", "reason": "...", "created_at_ms": 1710417045000 }
```
**Consumed by**: Evidence layer (spec 19)

#### `commits.jsonl`
```json
{ "id": "uuid", "proposal_id": "uuid", "decision": "Approve" | "Block", "checked_at_ms": 1710417045000, "reason": "..." }
```
**Consumed by**: Decision audit trail

#### `feedback_packets.jsonl`
```json
{ "id": "uuid", "trace_id": "uuid", "source_session_id": "session-key", "target_sessions": ["uuid"], "kind": "...", "summary": "...", "created_at_ms": 1710417045000 }
```
**Consumed by**: Hot Brain input, scheduler state building

---

### `~/.agent-hand/profiles/{profile}/progress/{session}.md`

**Reader**: None (for manual review)
**Writer**: ActionExecutor.write_progress()
**Format**: Markdown with timeline

```markdown
- [04:29:06] **task.complete**
  ```
  last tmux pane output...
  ```
- [04:30:44] **pre_compact** — context window compacting
- [04:32:11] **error** — tool `Bash`: command not found
```

**Purpose**: Durable memory across context-window compactions (matches Anthropic harness pattern)

---

### `~/.agent-hand/profiles/{profile}/canvas/{group_id}.json` (Pro only)

**Reader**: App::load_canvas()
**Writer**: App::save_canvas() (on exit or periodic)
**Format**: Serialized DiGraph + NodeData + EdgeData

```json
{
  "nodes": [
    { "id": "n1", "label": "Start", "kind": "Start", "session_id": null, "content": null, "ai_source_session": null },
    { "id": "n2", "label": "Session A", "kind": "Process", "session_id": "uuid-a", ... }
  ],
  "edges": [
    { "source": "n1", "target": "n2", "rel_type": "flow", "label": "on success" }
  ]
}
```

**Persistence**: One JSON file per group (Pro feature)

---

### `~/.agent-hand/profiles/{profile}/agent-runtime/scheduler_state.json` (Designed but not yet fully integrated)

**Schema**:
```json
{
  "pending_coordination": [
    { "id": "uuid", "trace_id": "uuid", "source_session_id": "...", "target_session_ids": [...], "disposition": "PendingCoordination", "reason": "...", "urgency_level": "Medium", "created_at_ms": 1710417045000 }
  ],
  "review_queue": [...],
  "proposed_followups": [...]
}
```

**Status**: Structure defined in `scheduler.rs`, consumption path unclear (see "Loose Ends")

---

## 4. Feature Gate Structure

### Free Tier (default, no flags)

**Included:**
- Session CRUD (create, delete, rename, move groups)
- Session lifecycle (start, stop, restart, attach, interrupt)
- Tmux integration + session tracking
- Progress logging (`progress/{session}.md`)
- Sound notifications (CESP sound packs)
- UI: Navigation, tree panel, preview, help, search
- Dialogs: NewSession, RenameSession, DeleteConfirm, CreateGroup, etc.
- Settings: Notification config, language, keybindings

**NOT Included:**
- Canvas (workflow editor)
- Relationships (session linking)
- Sharing (collaboration relay)
- Skills (Anthropic harness)
- AI provider integration
- WebSocket data transport

---

### Pro Tier (`#[cfg(feature = "pro")]`)

**Unlock:**
- **Relationships**: Create, view, edit relationships between sessions
  - Files: `src/session/relationships.rs`, `src/ui/dialogs.rs` (CreateRelationshipDialog, RelationshipPanel)
  - Stored in: `sessions.json` → `relationships[]`
  - UI: Relationship tree panel, canvas edge rendering

- **Canvas Workflow Editor**: Full flowchart/diagram editor
  - Files: `src/ui/canvas/*`
  - Persistence: `canvas/{group_id}.json` per group
  - Sockets: Canvas control socket (`canvas.sock`)

- **Sharing & Collaboration**: Relay client integration
  - Files: `src/pro/collab/client.rs` (RelayClient)
  - UI: ShareDialog, OrphanedRoomsDialog
  - Feature: Real-time sync of sessions across users

- **Skills Module**: Anthropic harness integration
  - Files: `src/skills/` (skill definitions, Anthropic protocol)
  - Allows: CLI tools to load/execute skills

- **Viewer Panel**: Active sessions sidebar
  - UI: Second panel showing currently-running sessions
  - Feature: Attach/monitor other sessions

**Key Files**:
```
#[cfg(feature = "pro")]
- src/pro/ (pro-specific code)
- src/session/relationships.rs
- src/skills/
- src/ui/dialogs.rs (lines 1-300+)
- src/ui/app/mod.rs (viewer_panel_focused, canvas_group, canvas_dir)
- src/tmux/manager.rs (relationship tracking methods)
- src/cli/commands.rs (pro-only commands)
```

---

### Max Tier (`#[cfg(feature = "max")]`)

**Unlock:**
- **AI Provider Integration**: Claude, OpenAI, or custom LLM
  - Config: `src/config.rs::AiConfig`
  - Module: `src/ai/` (provider abstraction)
  - Usage: AI-powered analysis, summaries, diagram generation

- **WebSocket Transport**: Real-time data sync
  - Config: `src/config.rs::WsConfig`
  - Module: `src/ws/` (WebSocket server/client)
  - Purpose: Live coordination, agent-to-agent messaging

- **Advanced Dialogs**: AI analysis UI
  - `AiAnalysisDialog`: Run AI analysis on sessions
  - `BehaviorAnalysisDialog`: Analyze session behavior patterns

**Key Files**:
```
#[cfg(feature = "max")]
- src/ai/ (AI provider interface)
- src/ws/ (WebSocket integration)
- src/ui/dialogs.rs (lines 1618-1687)
- src/config.rs (AiConfig, WsConfig)
- src/lib.rs (ai, ws exports)
```

**Note**: Max requires Pro (they build on each other).

---

## 5. Entry Points & Triggers

### Application Startup

```
main.rs (not shown, but inferred):
  → cli::run_main()
  → ui::app::App::new()
    ├─ Storage::load_data() [sessions.json]
    ├─ SystemRunner::new() + register all Systems
    ├─ ActionExecutor::new() + spawn run() task
    ├─ CanvasSocketServer::start() → listen on canvas.sock
    ├─ ControlSocketServer::start() → listen on control.sock
    └─ App event loop (crossterm, ratatui)
```

### Coordination Pipeline Entry Points

#### 1. **Hook Event → SystemRunner**
- **Trigger**: Tmux status change (via hook)
- **Flow**:
  ```
  hook fires → agent-hand-bridge → broadcast channel
  → SystemRunner.run() → dispatch to Systems
  → Systems emit Actions → ActionExecutor
  ```
- **Systems** that produce Actions:
  - `ProgressSystem`: on Stop, PreCompact, ToolFailure
  - `ContextGuardSystem`: on UserPromptSubmit (if scope != Off)
  - `SoundSystem`: on status transitions

#### 2. **Guard Decision → Audit + Pipeline**
- **Trigger**: ContextGuardSystem.on_event() (user_prompt_submit by default)
- **Flow**:
  ```
  Guard evaluation (pure function)
  → if Approve: emit GuardedContextInjection
  → ActionExecutor writes audit trail
  → ActionExecutor calls run_coordination_pipeline()
  → [TBD] Updates scheduler_state or proposals.json
  ```

#### 3. **Canvas Operations → Socket**
- **Trigger**: External tool sends canvas op via socket
- **Entry**: `CanvasSocketServer::spawn_listener()`
- **Flow**:
  ```
  canvas.sock receive → CanvasRequest → App event loop
  → App::process_canvas_op() → canvas_state.apply_op()
  → send CanvasResponse back
  ```

#### 4. **Control Operations → Socket**
- **Trigger**: bridge sends session/group/rel command
- **Entry**: `ControlSocketServer::spawn_listener()`
- **Flow**:
  ```
  control.sock receive → ControlRequest → App event loop
  → App::process_control_op() → Storage.update_data()
  → sessions.json updated → UI refreshed
  ```

#### 5. **Dialog Actions → Storage**
- **Trigger**: User submits dialog (create session, rename, delete, etc.)
- **Entry**: `App::handle_dialog_result()`
- **Flow**:
  ```
  Dialog approved → Storage method (add, remove, rename)
  → StorageData modified → sessions.json written
  → UI state updated
  ```

### Feature-Gated Entry Points

#### Pro:
- **CreateRelationshipDialog**: Triggered by user action → Relationship added to sessions.json
- **ShareDialog**: Triggered by user action → RelayClient spawned, relay server contacted
- **Canvas Editor**: Accessed via `p` key in Pro, hidden in Free

#### Max:
- **AiAnalysisDialog**: Triggered by user action → AI provider called → results displayed

---

## 6. Loose Ends & Dead Code

### Partial/Incomplete Implementations

#### 1. **Scheduler Coordination Pipeline** (partially wired)
- **Defined**: `src/agent/scheduler.rs` (SchedulerRecord, SchedulerState, FollowupProposalRecord)
- **Used**: `src/agent/projections.rs::build_scheduler_view_model()` for rendering
- **Issue**: Unclear how SchedulerRecord is populated from consumers output
- **Missing**: Deterministic scheduler logic that consumes SchedulerRecord and executes coordination
- **Status**: Designed but not fully wired into ActionExecutor → run_coordination_pipeline()

#### 2. **Memory Promotion Ladder** (types defined, pipeline unclear)
- **Defined**: `src/agent/memory.rs` (5-layer model: Audit → Evidence → Packet → Candidate → ColdMemory)
- **Promotion Gate**: `promote_to_cold_memory()` function exists (pure function, no I/O)
- **Missing**: Active ingestion and promotion of MemoryIngestEntry → ColdMemoryRecord
- **Status**: Architectural pattern defined but no active runtime integration found

#### 3. **Hot Brain Candidate Filtering** (types defined, primary analyzer unclear)
- **Defined**: `src/agent/hot_brain.rs` (WorldSlice, CandidateSet, scheduler hints + memory candidates)
- **Input**: WorldSlice (SessionTurn, Neighborhood, Coordination)
- **Output**: CandidateSet { scheduler_hints, memory_candidates }
- **Missing**: Primary analyzer function that reads WorldSlice and generates candidates
- **Note**: Specs reference this but actual implementation may be deferred or external

#### 4. **Consumer Normalization** (implemented, consumption path unclear)
- **Defined**: `src/agent/consumers.rs` (SchedulerNormalizedOutput, MemoryIngestEntry)
- **Function**: `consume_and_normalize()` (pure function)
- **Issue**: Who calls this? Where are outputs routed?
- **Status**: Intermediate layer without clear upstream/downstream binding

#### 5. **Canvas Multi-Layer Vision** (Layer 0-1 implemented, Layer 2+ designed)
- **Working**:
  - Layer 0: Session Map (nodes = sessions)
  - Layer 1: Relationship Graph (edges between sessions)
- **Designed but not implemented**:
  - Layer 2: Derived/AI Graphs (AI-generated analysis nodes)
  - Layer 3: Semantic Zoom (LOD with AI-generated titles at different zoom levels)
- **Files**: `src/agent/projections.rs` has room for future layers
- **Status**: Architectural foundation laid, features deferred

### Unused/Potentially Dead Code

#### 1. **TODO Comment**
- **Location**: `src/cli/commands.rs:760`
- **Text**: "TODO: Access relay client state from statusline context"
- **Status**: Blocking feature (Pro tier, relay state display)

#### 2. **Orphaned Features**
- `canvas_dir` (Pro): Canvas directory path, may not be used everywhere
- `canvas_group` (Pro): Current group's canvas, logic partial

#### 3. **Partial Canvas Features**
- Canvas LOD system is designed but zoom/LOD rendering not wired
- AI node generation (ai_source_session, ai_source_type) fields exist but production flow unclear

#### 4. **WebSocket Layer** (Max, unclear if end-to-end)
- `src/ws/` module exists with WsConfig
- **Question**: Is it actually wired end-to-end for real-time coordination?
- **Status**: Module exists, integration unclear

### Architectural Gaps

1. **scheduler_state.json Consumption**: Where is this file read? No reader found.
2. **Hot Brain → Scheduler**: How does hot_brain output flow into scheduler input?
3. **Feedback Packet → Memory**: How do feedback_packets.jsonl entries promote through the ladder?
4. **Canvas Persistence**: When is canvas saved? On exit? Periodically? No explicit save logic found in event loop.
5. **Relay State Access**: Pro feature (sharing) has a TODO blocking status line display of relay state.

---

## 7. Summary Matrix

| Aspect | Status | Details |
|--------|--------|---------|
| **Module Graph** | ✅ Complete | Clear dependency tree, circular deps checked |
| **Data Flow** | ✅ Complete | Event → System → Action → Executor → Files |
| **File Contracts** | ✅ Complete | JSON/JSONL schemas for all runtime files |
| **Feature Gates** | ✅ Complete | Free/Pro/Max tiers well-defined |
| **Entry Points** | ✅ Complete | Hook events, socket ops, dialogs all mapped |
| **Coordination Pipeline** | ⚠️ Partial | Guard + audit complete, scheduler dispatch incomplete |
| **Memory System** | ⚠️ Partial | Types defined, promotion ladder not active |
| **Hot Brain** | ⚠️ Partial | Input/output types defined, analyzer unclear |
| **Canvas Multi-Layer** | ⚠️ Partial | Layer 0-1 done, Layer 2-3 designed but deferred |
| **WebSocket End-to-End** | ❓ Unclear | Module exists, integration status unknown |
| **Dead Code** | ✅ Minimal | One TODO found, rest is structural (not dead) |

---

## 8. Recommendations for Next Phase

1. **Clarify scheduler_state.json Consumption**: Find or implement reader (missing in current codebase)
2. **Wire Hot Brain → Scheduler**: Connect candidate filtering output to scheduler state building
3. **Complete Memory Promotion**: Activate the promotion ladder in runtime
4. **Verify WebSocket Integration**: Confirm Max tier WebSocket is end-to-end operational
5. **Canvas Save Logic**: Find or implement explicit persistence trigger (not just on exit)
6. **Resolve TODO**: Access relay client state from statusline context (Pro feature blocker)
