---
name: canvas-render
description: Render agent-driven canvas visualizations from runtime coordination artifacts
---

# Canvas Render — Agent Projection Skill

Teach any LLM agent to read coordination artifacts from the agent-hand runtime
directory and produce canvas visualizations via CanvasOps. This replaces hardcoded
Rust projection logic with agent-driven judgment about layout, LOD, and emphasis.

**Architecture**: Code writes JSON artifacts → Agent reads and reasons → Agent emits
CanvasOps → Canvas engine validates and renders. See `canvas-ops` skill for the
transport and op vocabulary.

## Data Sources

All runtime artifacts live at:
```
~/.agent-hand/profiles/default/agent-runtime/
```

### Files You Can Read

| File | Format | Contents |
|------|--------|----------|
| `scheduler_state.json` | JSON | Pending coordination, review queue, proposed followups |
| `feedback_packets.jsonl` | JSONL | Per-turn structured feedback from each session |
| `evidence.jsonl` | JSONL | Evidence records (session state snapshots) |
| `candidate_sets.jsonl` | JSONL | Memory promotion candidates with scheduler hints |
| `cold_memory_snapshot.json` | JSON | Promoted cold memory entries |
| `followup_proposals_snapshot.json` | JSON | Accepted/rejected/pending followup proposals |
| `commits.jsonl` | JSONL | Guarded commit audit trail |
| `proposals.jsonl` | JSONL | Context injection proposals |
| `sidecar/{session_key}.json` | JSON | External agent-written structured feedback |

### How to Read Data

**Via bridge CLI** (preferred — handles path resolution):
```bash
# Read a runtime file
cat ~/.agent-hand/profiles/default/agent-runtime/scheduler_state.json

# Read the latest feedback packets (last 5)
tail -5 ~/.agent-hand/profiles/default/agent-runtime/feedback_packets.jsonl

# Read sidecar feedback for a specific session
cat ~/.agent-hand/profiles/default/agent-runtime/sidecar/my_session_key.json
```

**Via bridge query** (for canvas state):
```bash
agent-hand-bridge query state      # Full canvas state (nodes, edges, cursor, viewport)
agent-hand-bridge query nodes      # All nodes
agent-hand-bridge query viewport   # Viewport dimensions and position
```

### Data Schemas

#### scheduler_state.json
```json
{
  "pending_coordination": [
    {
      "id": "abc-123",
      "trace_id": "trace-456",
      "source_session_id": "session-a",
      "target_session_ids": ["session-b"],
      "disposition": "PendingCoordination",
      "reason": "Session A produced output relevant to B",
      "urgency_level": "High",
      "created_at_ms": 1700000000000
    }
  ],
  "review_queue": [],
  "proposed_followups": []
}
```

#### feedback_packets.jsonl (one object per line)
```json
{
  "packet_id": "38103963-b64",
  "trace_id": "7cf6f7e4-ddb",
  "source_session_id": "7277028b-...",
  "created_at_ms": 1773323467081,
  "goal": "Implement auth module",
  "now": "Writing JWT middleware",
  "done_this_turn": ["Created token validator"],
  "blockers": [],
  "decisions": ["Using RS256 algorithm"],
  "findings": ["Existing session store is compatible"],
  "next_steps": ["Add refresh token support"],
  "affected_targets": ["src/auth/"],
  "source_refs": [],
  "urgency_level": "Low",
  "recommended_response_level": "L2SelfInject"
}
```

#### sidecar/{session_key}.json
```json
{
  "goal": "Build API endpoints",
  "now": "Writing handler tests",
  "blockers": ["DB schema not finalized"],
  "decisions": ["Using axum over actix"],
  "findings": ["Found existing middleware"],
  "next_steps": ["Wire up routes"],
  "affected_targets": ["src/api/"],
  "urgency": "medium"
}
```

## Querying Canvas State

Before rendering, always query current state to understand what exists:

```bash
# Get full state including viewport
agent-hand-bridge canvas '{"op":"query","what":"state"}'
# Returns: {"type":"state","json":{"nodes":[...],"edges":[...],"cursor":[x,y],"viewport":{"x":0,"y":0}}}

# Get viewport dimensions (for LOD decisions)
agent-hand-bridge canvas '{"op":"query","what":"viewport"}'
# Returns: {"type":"viewport","panel_cols":80,"panel_rows":24,"viewport_x":0,"viewport_y":0}

# Check what projection nodes already exist
agent-hand-bridge query nodes --label "ap_"
```

## Prefix Conventions

Agent-generated nodes MUST use ID prefixes. The canvas engine enforces these:

| Prefix | Owner | Use Case |
|--------|-------|----------|
| `ap_` | Agent Projection | Nodes created by this skill (Claude Code, external LLM) |
| `wasm_` | WASM Plugin | Nodes created by WASM plugins |
| `session:` | User/System | Session-linked nodes (do NOT create these) |

You may only create, update, or remove nodes with `ap_` prefix. Attempts to
modify other prefixes will be rejected by the canvas engine.

## The ClearPrefix Pattern

**Always clear your prefix before re-rendering.** This prevents stale projection
nodes from accumulating.

```bash
# Step 1: Clear all existing projection nodes atomically
agent-hand-bridge canvas '{"op":"clear_prefix","prefix":"ap_"}'

# Step 2: Render fresh projection
agent-hand-bridge canvas --batch /dev/stdin <<'EOF'
[
  {"op":"add_node","id":"ap_sched_overview","label":"Scheduler","kind":"Process","pos":[2,1]},
  {"op":"add_node","id":"ap_pending_0","label":"Coord: session-a → session-b","kind":"Decision","pos":[2,5]},
  {"op":"add_edge","from":"ap_sched_overview","to":"ap_pending_0","label":"pending"},
  {"op":"layout","direction":"TopDown"}
]
EOF
```

## LOD (Level of Detail) Guidance

LOD is a **judgment call** — you decide what detail level is appropriate based on:

1. **Node count**: How many items exist in the data?
2. **Viewport size**: How much screen real estate is available?
3. **User context**: What is the user likely interested in?

### LOD Decision Matrix

| Data Items | Viewport | LOD Level | Strategy |
|------------|----------|-----------|----------|
| 1-5 | Any | **Detail** | Show all fields, use content blocks |
| 6-15 | ≥80 cols | **Standard** | Show labels + key status indicators |
| 6-15 | <80 cols | **Compact** | Abbreviate labels, skip content |
| 16-30 | Any | **Overview** | Group by category, show counts |
| 30+ | Any | **Summary** | Single summary node with aggregated stats |

### Applying LOD

**Detail level** — expand individual items:
```json
{
  "op": "add_node",
  "id": "ap_session_abc",
  "label": "Frontend (running)",
  "kind": "Process",
  "pos": [2, 1],
  "content": "Goal: Implement auth\nNow: Writing JWT\nBlockers: none\nNext: refresh tokens"
}
```

**Standard level** — one-line labels:
```json
{"op": "add_node", "id": "ap_session_abc", "label": "Frontend: JWT middleware", "kind": "Process", "pos": [2, 1]}
```

**Overview level** — grouped summaries:
```json
{"op": "add_node", "id": "ap_group_running", "label": "Running (3)", "kind": "Start", "pos": [2, 1]}
{"op": "add_node", "id": "ap_group_blocked", "label": "Blocked (1)", "kind": "End", "pos": [2, 5]}
```

**Summary level** — single aggregate node:
```json
{
  "op": "add_node",
  "id": "ap_summary",
  "label": "Workspace Status",
  "kind": "Note",
  "pos": [2, 1],
  "content": "Sessions: 4 running, 1 blocked\nCoordination: 2 pending\nReview: 0 items\nFollowups: 1 proposed"
}
```

## Example Scenarios

### Scenario 1: Scheduler Overview (few items)

**Input**: `scheduler_state.json` has 2 pending coordinations, 1 review item.

```bash
# 1. Clear old projection
agent-hand-bridge canvas '{"op":"clear_prefix","prefix":"ap_"}'

# 2. Render
agent-hand-bridge canvas '{"op":"batch","ops":[
  {"op":"add_node","id":"ap_sched","label":"Scheduler State","kind":"Note","pos":[10,1]},
  {"op":"add_node","id":"ap_pending_0","label":"A → B: shared dep","kind":"Decision","pos":[2,5],"content":"Source: session-a\nTarget: session-b\nReason: shared dependency conflict\nUrgency: High"},
  {"op":"add_node","id":"ap_pending_1","label":"C → A: API change","kind":"Decision","pos":[22,5],"content":"Source: session-c\nTarget: session-a\nReason: API contract changed\nUrgency: Medium"},
  {"op":"add_node","id":"ap_review_0","label":"Review: auth scope","kind":"End","pos":[12,10],"content":"Needs human review\nReason: cross-session injection\nUrgency: High"},
  {"op":"add_edge","from":"ap_sched","to":"ap_pending_0","label":"pending"},
  {"op":"add_edge","from":"ap_sched","to":"ap_pending_1","label":"pending"},
  {"op":"add_edge","from":"ap_sched","to":"ap_review_0","label":"review"},
  {"op":"add_edge","from":"ap_pending_0","to":"ap_pending_1","label":"related"}
]}'
```

### Scenario 2: Session Activity Map (many sessions)

**Input**: 8 sessions with recent feedback packets.

```bash
# 1. Read data
PACKETS=$(tail -8 ~/.agent-hand/profiles/default/agent-runtime/feedback_packets.jsonl)

# 2. Clear + render overview (LOD: Standard — 8 items fits well)
agent-hand-bridge canvas '{"op":"clear_prefix","prefix":"ap_"}'

agent-hand-bridge canvas '{"op":"batch","ops":[
  {"op":"add_node","id":"ap_s1","label":"Frontend: JWT","kind":"Process","pos":[2,1]},
  {"op":"add_node","id":"ap_s2","label":"Backend: routes","kind":"Process","pos":[22,1]},
  {"op":"add_node","id":"ap_s3","label":"Shared: types","kind":"Process","pos":[42,1]},
  {"op":"add_node","id":"ap_s4","label":"Tests: e2e","kind":"Process","pos":[2,5]},
  {"op":"add_node","id":"ap_s5","label":"DB: migrations","kind":"Start","pos":[22,5]},
  {"op":"add_node","id":"ap_s6","label":"Auth: blocked","kind":"End","pos":[42,5]},
  {"op":"add_node","id":"ap_s7","label":"Deploy: idle","kind":"Note","pos":[2,9]},
  {"op":"add_node","id":"ap_s8","label":"Docs: writing","kind":"Process","pos":[22,9]},
  {"op":"add_edge","from":"ap_s1","to":"ap_s2","label":"API calls"},
  {"op":"add_edge","from":"ap_s2","to":"ap_s3","label":"imports"},
  {"op":"add_edge","from":"ap_s4","to":"ap_s1","label":"tests"},
  {"op":"add_edge","from":"ap_s6","to":"ap_s5","label":"waiting on"}
]}'
```

### Scenario 3: Focused Detail (single session deep-dive)

**Input**: User wants detail on one session. Latest feedback packet has rich data.

```bash
agent-hand-bridge canvas '{"op":"clear_prefix","prefix":"ap_"}'

agent-hand-bridge canvas '{"op":"batch","ops":[
  {"op":"add_node","id":"ap_focus","label":"Frontend Session","kind":"Start","pos":[2,1]},
  {"op":"add_node","id":"ap_goal","label":"Goal","kind":"Note","pos":[2,5],"content":"Implement JWT auth with\nrefresh token rotation"},
  {"op":"add_node","id":"ap_now","label":"Current","kind":"Process","pos":[22,5],"content":"Writing middleware\nfor token validation"},
  {"op":"add_node","id":"ap_done","label":"Done This Turn","kind":"Process","pos":[2,10],"content":"1. Created token validator\n2. Added RS256 signing\n3. Set up test fixtures"},
  {"op":"add_node","id":"ap_next","label":"Next Steps","kind":"Decision","pos":[22,10],"content":"1. Add refresh token\n2. Wire up routes\n3. Integration tests"},
  {"op":"add_node","id":"ap_decisions","label":"Decisions","kind":"Note","pos":[42,5],"content":"- RS256 over HS256\n- axum over actix\n- 15min token expiry"},
  {"op":"add_edge","from":"ap_focus","to":"ap_goal"},
  {"op":"add_edge","from":"ap_focus","to":"ap_now","label":"doing"},
  {"op":"add_edge","from":"ap_now","to":"ap_done","label":"completed"},
  {"op":"add_edge","from":"ap_now","to":"ap_next","label":"upcoming"},
  {"op":"add_edge","from":"ap_focus","to":"ap_decisions"}
]}'
```

### Scenario 4: Evidence Trail Visualization

**Input**: Show the guard pipeline audit trail for recent injections.

```bash
agent-hand-bridge canvas '{"op":"clear_prefix","prefix":"ap_"}'

agent-hand-bridge canvas '{"op":"batch","ops":[
  {"op":"add_node","id":"ap_ev_title","label":"Evidence Trail","kind":"Note","pos":[10,1]},
  {"op":"add_node","id":"ap_proposal","label":"Proposal: inject ctx","kind":"Start","pos":[2,4],"content":"scope: SelfOnly\nrisk: Low\nsession: frontend"},
  {"op":"add_node","id":"ap_evidence","label":"Evidence","kind":"Process","pos":[22,4],"content":"SessionState: Running\nProgressLog: 3 tasks done"},
  {"op":"add_node","id":"ap_guard","label":"Guard: 8/8 passed","kind":"Decision","pos":[12,8],"content":"bridge_enabled: PASS\ntarget_exists: PASS\npath_exists: PASS\nevidence_exists: PASS\nscope_self_only: PASS\ncooldown: PASS\nbudget: PASS\nno_duplicate: PASS"},
  {"op":"add_node","id":"ap_commit","label":"Commit: approved","kind":"End","pos":[12,13]},
  {"op":"add_edge","from":"ap_proposal","to":"ap_evidence","label":"collected"},
  {"op":"add_edge","from":"ap_evidence","to":"ap_guard","label":"evaluated"},
  {"op":"add_edge","from":"ap_guard","to":"ap_commit","label":"approved"}
]}'
```

## Rendering Workflow

Follow this sequence every time you render a projection:

```
1. READ   — Load runtime artifacts (scheduler_state.json, feedback_packets.jsonl, etc.)
2. QUERY  — Query current canvas state and viewport dimensions
3. DECIDE — Apply LOD judgment based on data volume and viewport
4. CLEAR  — ClearPrefix to remove stale projection nodes
5. EMIT   — Send new CanvasOps as a batch
6. VERIFY — Optional: query nodes to confirm rendering succeeded
```

### Decision Checklist

Before emitting ops, ask yourself:

- [ ] How many data items am I visualizing? → Pick LOD level
- [ ] What's the viewport size? → Adjust layout density
- [ ] Are there relationships between items? → Add edges
- [ ] What's most important to the user? → Put it at the top/center
- [ ] Did I clear the old prefix first? → Always ClearPrefix before rendering

## Limits

| Constraint | Value | Enforced By |
|------------|-------|-------------|
| Max batch size | 200 ops | Canvas engine |
| Max projection nodes | 100 | Canvas engine |
| Allowed prefixes | `ap_`, `wasm_` | Prefix validation filter |
| Node ID uniqueness | Required | Canvas engine |

## NodeKind Reference

Use NodeKind to convey semantic meaning:

| Kind | Visual | Best For |
|------|--------|----------|
| `Start` | Green, rounded | Active/running items, entry points |
| `End` | Red, rounded | Blocked/error items, exit points |
| `Process` | Cyan, plain | Normal tasks, in-progress work |
| `Decision` | Yellow, double | Pending decisions, branching points |
| `Note` | Gray, plain | Summaries, annotations, content blocks |

## Cross-References

- **`canvas-ops`** — Full CanvasOp vocabulary, transport details, error handling
- **`workspace-ops`** — Session management, group operations, bridge commands
- **`bridge-overview`** — Binary overview, socket paths, when to use each mode
