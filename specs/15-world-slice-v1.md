# WorldSlice V1 - Bounded Read Model for Hot Brain

## 1. Purpose

This document defines `WorldSlice V1`.

`WorldSlice` is the bounded read model that Hot Brain is allowed to analyze.

Its purpose is to:

- prevent Hot Brain from reading the full world
- keep analysis stable and testable
- keep token/material usage bounded
- make future analyzers pluggable without giving them arbitrary world access

This is the next design step after:

- `FeedbackPacket V1`
- transport adapter boundary
- Hot Brain V1 concept

## 2. One-Sentence Definition

```text
WorldSlice is a bounded, serializable, task-relevant subset of ECS world state exposed to coordination analyzers.
```

## 3. Why WorldSlice Exists

If Hot Brain reads the full ECS world, it will become:

- too broad
- too expensive
- too hard to test
- too hard to reason about

ASCII:

```text
Bad:
Hot Brain
  -> full world
  -> too much context
  -> unstable reasoning

Good:
Hot Brain
  -> bounded slice
  -> narrow context
  -> predictable reasoning
```

## 4. Position in the System

ASCII:

```text
                +-----------------------------+
                |          ECS World          |
                | sessions / groups / edges   |
                | packets / commits / logs    |
                +--------------+--------------+
                               |
                               v
                +-----------------------------+
                |      WorldSlice Builder     |
                +--------------+--------------+
                               |
                               v
                +-----------------------------+
                |       WorldSlice V1         |
                +--------------+--------------+
                               |
                               v
                +-----------------------------+
                |          Hot Brain          |
                +--------------+--------------+
                               |
                               v
                +-----------------------------+
                |        Candidate Set        |
                +-----------------------------+
```

## 5. Core Rules

### Rule 1: Slice is read-only

Hard rule:

```text
WorldSlice may be read by analyzers.
WorldSlice may not be mutated by analyzers to affect ECS state.
```

### Rule 2: Slice must be bounded

Every slice must have explicit material limits.

### Rule 3: Slice must be transport-independent

It must not encode:

- tmux-specific controls
- ACP-specific envelopes
- PTY-oriented prompt payloads

### Rule 4: Slice is coordination-facing

It is not for low-level rendering, raw protocol handling, or full archival replay.

## 6. Slice Taxonomy

V1 defines three slice classes.

ASCII:

```text
WorldSlice
  ├─ SessionTurnSlice
  ├─ NeighborhoodSlice
  └─ CoordinationSlice
```

## 7. SessionTurnSlice

### 7.1 Purpose

`SessionTurnSlice` focuses on one session's recent execution state.

Use cases:

- self-follow-up reasoning
- local packet enrichment
- local blocker interpretation

### 7.2 Suggested structure

```rust
pub struct SessionTurnSlice {
    pub session_key: String,
    pub session_id: Option<String>,
    pub current_status: String,

    pub recent_packets: Vec<FeedbackPacket>,
    pub recent_commits: Vec<GuardedCommit>,
    pub recent_progress_refs: Vec<String>,
}
```

### 7.3 Allowed material

Allowed:

- same-session recent packets
- same-session recent guarded commits
- same-session recent progress references
- same-session current status

Disallowed:

- unrelated sessions
- full graph traversals
- full raw pane dumps
- arbitrary external artifacts

### 7.4 V1 status

Defined in concept, but not the primary analyzed slice in V1.

## 8. NeighborhoodSlice

### 8.1 Purpose

`NeighborhoodSlice` adds graph-local context around one focal session.

Use cases:

- relation-aware coordination
- selecting cross-session context sources
- direct dependency awareness

### 8.2 Suggested structure

```rust
pub struct NeighborhoodSlice {
    pub focal_session: SessionTurnSlice,
    pub neighbor_sessions: Vec<SessionTurnSlice>,
    pub shared_blockers: Vec<String>,
    pub confirmed_relation_ids: Vec<String>,
}
```

### 8.3 Allowed material

Allowed:

- focal session
- same-group neighbors (bounded)
- direct confirmed relation neighbors
- packets from direct neighbors

Disallowed:

- second-degree graph expansion in V1
- full workspace group scan
- global memory scan

### 8.4 Scope limit

V1 recommendation:

```text
max direct neighbors <= 4
relation depth <= 1
```

ASCII:

```text
      [B]
       |
[A] -- [C]
       |
      [D]

NeighborhoodSlice(A)
= A + direct confirmed neighbors only
```

### 8.5 V1 status

Defined in concept, but not the primary analyzed slice in V1.

## 9. CoordinationSlice

### 9.1 Purpose

`CoordinationSlice` is the primary V1 input for Hot Brain.

It focuses on recent coordination frontier data rather than low-level execution detail.

Use cases:

- scheduler hint generation
- repeated blocker detection
- memory candidate selection

### 9.2 Suggested structure

```rust
pub struct CoordinationSlice {
    pub recent_packets: Vec<FeedbackPacket>,
    pub pending_blockers: Vec<String>,
    pub affected_targets: Vec<String>,
}
```

### 9.3 Why this is the V1 slice

Reasons:

1. It is already aligned with `FeedbackPacket V1`
2. It stays above transport concerns
3. It allows bounded reasoning without world-wide graph access
4. It avoids packet rewriting in V1

### 9.4 Material limits

Recommended defaults:

- `max_recent_packets = 3`
- `max_affected_targets = 4`

If more data is needed later, expand in V2, not implicitly in V1.

## 10. What WorldSlice V1 Must Not Contain

Do not put these directly into slices in V1:

- raw terminal transcript blobs
- full prompt history
- unbounded tool output
- provider-specific resume information
- PTY command payloads
- hook transport envelopes
- full graph snapshots

Those belong elsewhere:

- logs
- artifacts
- transport adapters
- future cold memory layers

## 11. Builder Responsibilities

`WorldSliceBuilder` is the only thing that should assemble slices from ECS state.

Its responsibilities:

1. select relevant world data
2. sort and bound it
3. deduplicate where appropriate
4. strip transport-specific detail
5. produce a stable input for analyzers

ASCII:

```text
ECS World
   |
   v
WorldSliceBuilder
   |
   +--> sort
   +--> trim
   +--> dedupe
   +--> normalize
   |
   v
WorldSlice
```

## 12. Builder Constraints

### 12.1 Deterministic ordering

Builder must sort by explicit timestamps or stable keys.

Never rely on caller ordering.

### 12.2 Dedup behavior

Builder may deduplicate:

- blockers
- targets
- packet refs

but should not lose provenance.

### 12.3 Provenance retention

If a builder collapses multiple inputs into one aggregated entry, it should still preserve:

- source refs
- source session IDs
- stable traceability

## 13. V1 Analyzer Permissions

Given a `WorldSlice V1`, analyzers may:

- classify
- rank
- aggregate
- emit candidates

They may not:

- mutate world state
- mutate slices and write them back
- publish commits
- bypass deterministic consumers

## 14. Interaction with FeedbackPacket

`WorldSlice V1` should be packet-centric.

ASCII:

```text
FeedbackPacket
   |
   +--> SessionTurnSlice
   +--> NeighborhoodSlice
   \--> CoordinationSlice
```

This means:

- packet remains the main coordination artifact
- slice remains the bounded analysis window

These are different roles and should not collapse.

## 15. Interaction with Canvas and Relations

Canvas and relation graph state may help determine what enters a slice, but:

```text
canvas does not define slices
relation state constrains slices
```

For example:

- confirmed relations may permit neighborhood inclusion
- same-group membership may permit bounded group-local inclusion

But V1 should still stay narrow.

## 16. Execution Plan

### Phase S0 - Freeze V1 shapes

Done when:

- this document is accepted
- `SessionTurnSlice`, `NeighborhoodSlice`, and `CoordinationSlice` are treated as the V1 slice taxonomy

### Phase S1 - Align implementation

Implementation target:

- make sure current `hot_brain.rs` uses `CoordinationSlice` as primary V1 slice
- keep `SessionTurnSlice` and `NeighborhoodSlice` defined but not overused

### Phase S2 - Stabilize builder semantics

Implementation target:

- timestamp ordering
- bounded trimming
- dedupe with provenance

### Phase S3 - Add explicit source metadata if needed

Only if candidate traceability or scheduler quality requires it.

## 17. Recommended Next Document

After `WorldSlice V1`, the next document should be:

```text
Candidate Consumers
```

Because the next architectural question becomes:

```text
Who is allowed to consume SchedulerHint and MemoryCandidate,
and how do those consumers stay deterministic?
```

## 18. Final Statement

`WorldSlice V1` is the bounded lens through which Hot Brain sees the world.

That means:

```text
World = everything
Slice = what analysis is allowed to see
Candidate = what analysis is allowed to suggest
```

That is the stable V1 boundary.
