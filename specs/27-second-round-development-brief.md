# Second-Round Development Brief - From Persisted Outputs to Real System Consequences

## 1. Purpose

This brief defines the next development stage after the current status audited in:

- `specs/26-implementation-status-audit.md`

It assumes the following are already true:

- guarded live runtime exists
- `FeedbackPacket V1` exists
- Hot Brain V1 exists as a bounded analyzer
- candidate consumers exist as deterministic normalizers
- the packet-driven coordination path now persists:
  - `candidate_sets.jsonl`
  - `scheduler_outputs.jsonl`
  - `memory_ingest_entries.jsonl`

What is still missing is the next step:

```text
persisted coordination outputs
  -> bounded runtime consequences
```

## 2. The Three Real Gaps

The next stage should focus only on these three gaps:

1. `SchedulerNormalizedOutput` must enter a bounded scheduler-side state model
2. `MemoryIngestEntry` must enter a bounded cold-memory promotion path
3. shared world state must begin projecting into canvas/workflow view models

These are the first real "vertical consequence" steps after the current packet/analyze/normalize/persist stage.

## 3. Scope

### In scope

- scheduler-side normalized state
- cold-memory promotion path
- projection/view-model layer for canvas/workflow

### Out of scope

- ACPX/ACP transport integration
- whole-graph autonomous scheduling
- unrestricted relation discovery
- PTY/hook/file delivery redesign
- major UI polish

## 4. Workstream A - Scheduler-Side State

### 4.1 Goal

Move from:

```text
SchedulerNormalizedOutput persisted to JSONL
```

to:

```text
SchedulerNormalizedOutput -> bounded scheduler-side state
```

### 4.2 What to add

Add a small scheduler-side state model, for example:

```rust
pub struct SchedulerState {
    pub pending_coordination: Vec<SchedulerRecord>,
    pub review_queue: Vec<SchedulerRecord>,
    pub proposed_followups: Vec<SchedulerRecord>,
}

pub struct SchedulerRecord {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub target_session_ids: Vec<String>,
    pub disposition: SchedulerDisposition,
    pub reason: String,
    pub urgency_level: RiskLevel,
    pub created_at_ms: u64,
}
```

### 4.3 Rules

- `Ignore` -> do not enter scheduler state
- `RecordOnly` -> may remain audit-only
- `PendingCoordination` -> enters `pending_coordination`
- `NeedsHumanReview` -> enters `review_queue`
- `ProposeFollowup` -> enters `proposed_followups`

### 4.4 Constraints

- no tmux/session control yet
- no automatic wake/resume/start yet
- no proposal execution yet

This stage is still state formalization, not active automation.

## 5. Workstream B - Cold Memory Promotion Path

### 5.1 Goal

Move from:

```text
MemoryIngestEntry persisted to JSONL
```

to:

```text
MemoryIngestEntry -> validated promotion -> ColdMemoryRecord
```

### 5.2 What to add

Add a bounded promotion function or module that:

1. reads accepted `MemoryIngestEntry`
2. checks promotion eligibility
3. emits `ColdMemoryRecord`
4. persists promoted records separately

Possible file target:

- `src/agent/memory.rs`

### 5.3 Suggested persistence

Use another append-only JSONL file first, for example:

```text
runtime_dir/cold_memory.jsonl
```

### 5.4 Constraints

- no semantic search yet
- no DB migration yet
- no heavy indexing yet

This stage is only about promotion correctness and durable shape.

## 6. Workstream C - Projection/View-Model Layer

### 6.1 Goal

Move from:

```text
world state + docs about views
```

to:

```text
explicit projection structs for relationship / scheduler / evidence / workflow views
```

### 6.2 What to add

Add lightweight projection/view-model types, for example:

```rust
pub struct RelationshipViewModel { ... }
pub struct SchedulerViewModel { ... }
pub struct EvidenceViewModel { ... }
pub struct WorkflowViewModel { ... }
```

These do not need full rendering yet.
They only need to prove that the shared world can be projected into stable view-oriented models.

### 6.3 Constraints

- no full canvas UI overhaul yet
- no renderer semantics in projection builders
- no duplicated business rules in UI

## 7. Recommended Order

Implement in this order:

### Step 1

Scheduler-side state

Reason:

- closest downstream consequence of `SchedulerNormalizedOutput`
- lowest risk

### Step 2

Cold memory promotion path

Reason:

- closest downstream consequence of `MemoryIngestEntry`
- preserves the memory ladder cleanly

### Step 3

Projection/view-model layer

Reason:

- depends on stable scheduler/memory-side semantics

## 8. Suggested Deliverables

At the end of this second round, the codebase should have:

1. a bounded scheduler-side state representation
2. a bounded cold-memory promotion representation
3. first-class projection/view-model structs for future canvas/workflow rendering

## 9. Acceptance Criteria

This stage is complete when:

### Scheduler

- normalized scheduler outputs are no longer only audit records
- they enter a bounded scheduler state

### Memory

- normalized memory entries can become cold memory records through a deterministic gate

### Views

- projection structs exist for relationship/scheduler/evidence/workflow views
- projections use shared world semantics, not renderer-specific semantics

## 10. Implementation Constraints

Do not:

- add direct live session automation
- add unrestricted autonomous scheduling
- merge all state into one giant memory structure
- jump straight into heavy canvas rendering

Keep this round structurally clean.

## 11. Short Task Prompt

```text
Use specs/26-implementation-status-audit.md and specs/27-second-round-development-brief.md as the primary guide for the next implementation stage.

Focus only on the three real missing layers:
1. SchedulerNormalizedOutput -> bounded scheduler-side state
2. MemoryIngestEntry -> deterministic ColdMemoryRecord promotion path
3. Shared world -> explicit projection/view-model structs for relationship, scheduler, evidence, and workflow views

Do not add direct session automation, ACPX integration, or heavy UI rendering yet.
Keep the changes bounded, deterministic, and aligned with the current architecture.
```
