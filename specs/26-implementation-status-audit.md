# Implementation Status Audit - Code vs Design

## 1. Purpose

This document audits the current codebase against the recent architecture/design docs.

It answers:

```text
What is already real in code?
What is only designed?
What is partially implemented but not yet connected?
```

This document is intentionally practical and evidence-oriented.

## 2. Reading Guide

Status meanings:

- `Implemented`
  - exists in code and is actively wired into runtime or usable modules
- `Partial`
  - exists in code, but is not yet fully integrated into runtime/user flow
- `Designed Only`
  - exists in specs/docs, but no meaningful code implementation yet

## 3. Summary Table

| Area | Status | Notes |
|------|--------|-------|
| Guarded self-context runtime | Implemented | Main MVP path exists and is wired |
| FeedbackPacket V1 | Implemented | Type exists and is emitted by context path |
| Runtime E2E for guarded path | Implemented | Multiple runtime tests exist in `runner.rs` |
| Hot Brain V1 analyzer | Implemented | Pure bounded analyzer exists |
| WorldSlice V1 types | Implemented | Slice types exist in code |
| Candidate consumers | Implemented | Deterministic normalization exists |
| Memory boundary types | Partial | Cold memory record and promotion checks exist, but no full ingest pipeline |
| Scheduler normalized outputs | Implemented | Types + normalization functions exist |
| Hot Brain in active runtime loop | Designed Only | Not wired into packet-triggered runtime yet |
| Deterministic consumers in active runtime loop | Designed Only | Not wired into actual scheduler/memory runtime consequences |
| Transport adapter abstraction | Designed Only | Boundary documented, no generic adapter layer in code |
| Canvas / workflow multi-view projections | Designed Only | Mostly still doc/spec territory |

## 4. Guarded Self-Context Runtime

### Status

`Implemented`

### Evidence

The main guarded path exists in code:

- `src/agent/guard.rs`
- `src/agent/systems/context.rs`
- `src/agent/runner.rs`
- `src/ui/app/mod.rs`

### Actual path in code

```text
HookEvent(UserPromptSubmit)
  -> ContextGuardSystem
  -> Proposal + Evidence
  -> run_guard()
  -> GuardedCommit
  -> Action::GuardedContextInjection
  -> ActionExecutor
  -> .agent-hand-context.md + JSONL audit
```

### File evidence

- `run_guard()` exists in [src/agent/guard.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/guard.rs)
- `ContextGuardSystem` builds proposal/evidence and emits guarded action in [src/agent/systems/context.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/systems/context.rs)
- `ActionExecutor` writes audit streams and context artifact in [src/agent/runner.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/runner.rs)
- system registration happens in [src/ui/app/mod.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/ui/app/mod.rs)

## 5. FeedbackPacket V1

### Status

`Implemented`

### Evidence

The packet type exists in [src/agent/guard.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/guard.rs) and is instantiated in [src/agent/systems/context.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/systems/context.rs).

### Current limitation

The current packet payload is still minimal in practice:

- mostly empty lists by default
- `goal` / `now` sparse

So the schema exists and is real, but the semantic richness is still early.

## 6. Runtime E2E for Guarded Path

### Status

`Implemented`

### Evidence

There are runtime-oriented tests in [src/agent/runner.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/runner.rs) around:

- approved injection path
- blocked path
- cooldown behavior
- audit file writes

This means the guarded path is no longer only a pure-function design.

## 7. Hot Brain V1

### Status

`Implemented`

### Evidence

Hot Brain exists in:

- [src/agent/hot_brain.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/hot_brain.rs)

Implemented:

- `HotBrainConfig`
- `WorldSlice`
- `SessionTurnSlice`
- `NeighborhoodSlice`
- `CoordinationSlice`
- `SchedulerHint`
- `MemoryCandidate`
- `CandidateSet`
- `build_coordination_slice()`
- `analyze()`

### Important clarification

This is currently:

```text
a bounded pure analyzer module
```

It is **not yet** an actively triggered second runtime.

So Hot Brain exists in code, but only as an analysis layer, not yet as a live runtime participant.

## 8. WorldSlice V1

### Status

`Implemented`

### Evidence

`WorldSlice`, `SessionTurnSlice`, `NeighborhoodSlice`, and `CoordinationSlice` all exist in [src/agent/hot_brain.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/hot_brain.rs).

### Current practical status

Only `CoordinationSlice` is actively used by the analyzer logic.

So:

- taxonomy exists
- V1 active usage is intentionally narrow

This matches the design.

## 9. Candidate Consumers

### Status

`Implemented`

### Evidence

Consumer logic exists in:

- [src/agent/consumers.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/consumers.rs)

Implemented:

- `ConsumerConfig`
- `SchedulerDisposition`
- `SchedulerNormalizedOutput`
- `MemoryIngestEntry`
- `normalize_scheduler_hints()`
- `normalize_memory_candidates()`

### Important clarification

These consumers currently normalize data.
They do **not yet** produce active runtime consequences.

So this layer is real, but still operating as:

```text
deterministic normalization without active downstream scheduling/memory runtime integration
```

## 10. Memory Boundary

### Status

`Partial`

### Evidence

Memory boundary code exists in:

- [src/agent/memory.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/memory.rs)

Implemented:

- `MemoryLayer`
- `ColdMemoryRecord`
- `check_promotion_eligibility()`

### Why only partial

What exists:

- memory-layer taxonomy
- promotion gate logic
- cold-memory record shape

What does not yet exist:

- actual memory promotion runtime
- accepted cold-memory store
- ingestion pipeline from `MemoryIngestEntry` to durable memory

So the boundary is partially encoded, but the full memory pipeline is not.

## 11. Scheduler Normalized Outputs

### Status

`Implemented`

### Evidence

Scheduler normalization types and dispositions exist in:

- [src/agent/consumers.rs](/Users/weykon/Desktop/p/agent-deck-rs/src/agent/consumers.rs)

### Why this counts as implemented

The key layer:

```text
SchedulerHint
 -> SchedulerConsumer
 -> SchedulerNormalizedOutput
```

exists concretely in code.

### Limitation

The outputs are not yet wired into actual scheduler-side state transitions or proposal generation.

## 12. Hot Brain as Active Runtime

### Status

`Designed Only`

### Why

There is no code path today that does:

```text
FeedbackPacket emitted
 -> packet-triggered Hot Brain pass
 -> candidate set persisted or routed
```

The analyzer exists, but the second runtime loop does not.

This is one of the biggest remaining gaps between design and implementation.

## 13. Deterministic Consumers as Active Runtime

### Status

`Designed Only`

### Why

Consumers normalize hints and memory candidates, but:

- are not called by a runtime loop
- do not feed active scheduler state
- do not feed active cold-memory ingestion

So the deterministic consumer stage exists as code modules, but not yet as active runtime architecture.

## 14. Transport Adapter Abstraction

### Status

`Designed Only`

### Why

There is still no generalized adapter interface in code for:

- ingress normalization
- control execution
- projection delivery
- capability reporting

Current transport is still direct implementation around:

- tmux
- hooks
- sockets

The design boundary exists in docs, not yet in code.

## 15. Canvas / Workflow Multi-Views

### Status

`Designed Only`

### Why

While there is existing canvas-related code under `src/ui/canvas/`, the newly designed semantic multi-view model:

- relationship view
- scheduler view
- evidence view
- workflow view

is not yet reflected as a unified projection architecture in the codebase.

So this remains mainly a design-level direction.

## 16. Current Gap Map

ASCII:

```text
Implemented:
HookEvent -> Guarded Runtime -> FeedbackPacket
FeedbackPacket -> Hot Brain (pure analyzer)
CandidateSet -> Consumers (pure normalization)

Missing:
FeedbackPacket -> active coordination runtime trigger
Consumers -> active scheduler consequences
Consumers -> active memory promotion
Shared world -> multi-view canvas/workflow projections
Transport abstraction layer
```

## 17. What Is Actually Safe To Claim

At this point, it is safe to claim:

### Real in code

1. Guarded self-context runtime exists
2. FeedbackPacket V1 exists
3. Hot Brain V1 exists as bounded analyzer
4. WorldSlice V1 exists
5. Candidate consumers exist
6. Memory boundary types partly exist

### Not yet real in code

1. Full second coordination runtime
2. Full memory ingestion pipeline
3. Full scheduler follow-up runtime
4. Multi-view canvas/workflow projection model
5. Protocol-neutral transport adapter interface

## 18. Recommended Next Practical Focus

If the goal is to keep implementation aligned with design, the next highest-value focus areas are:

1. active packet-driven coordination runtime wiring
2. deterministic consumer wiring
3. memory promotion path
4. view-model projection layer for canvas/workflow

## 19. Final Statement

The current codebase is no longer just a document-only design.

But it is also not yet the full architecture described in the later specs.

The most accurate statement is:

```text
the first runtime and most second-layer pure modules are implemented,
while the second runtime loop, memory promotion path, transport abstraction,
and multi-view projection layer are still ahead of the code
```
