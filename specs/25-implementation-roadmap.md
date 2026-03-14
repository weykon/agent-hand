# Implementation Roadmap - Ordered Delivery Plan

## 1. Purpose

This document is the closing roadmap for the current architecture phase.

It answers:

```text
Given all current design documents, what is the correct implementation order?
```

This roadmap exists to:

- prevent random implementation order
- preserve architectural layering
- keep the MVP narrow
- make future development agent work sequential and understandable

## 2. Roadmap Principle

The project should be built:

```text
from lower-risk deterministic foundations
toward higher-level coordination and projection layers
```

This means:

- stabilize the guarded runtime first
- stabilize packet semantics second
- stabilize Hot Brain boundaries third
- only then expand memory, scheduler, and views

## 3. The Overall Sequence

ASCII:

```text
Stage 1  Guarded Live Runtime
Stage 2  Packet and Runtime E2E
Stage 3  Hot Brain Foundations
Stage 4  Candidate Consumers
Stage 5  Memory Boundary
Stage 6  Scheduling Formalization
Stage 7  View / Canvas Projections
Stage 8  Transport Adapter Expansion
```

## 4. Stage 1 - Guarded Live Runtime

Primary docs:

- `specs/09-mvp-phase-a-brief.md`
- `specs/10-boundary-and-e2e-plan.md`
- `specs/11-feedback-packet-v1.md`

Goal:

- complete and stabilize the first vertical slice

Core path:

```text
HookEvent
 -> Proposal
 -> Evidence
 -> Guard
 -> GuardedCommit
 -> FeedbackPacket
```

Must be true before moving on:

1. guarded context path is stable
2. `FeedbackPacket V1` is emitted consistently
3. delivery semantics are still narrow

## 5. Stage 2 - Runtime End-to-End Proof

Primary docs:

- `specs/10-boundary-and-e2e-plan.md`

Goal:

- prove that the guarded runtime works beyond unit tests

Required E2E coverage:

1. approve path
2. block path
3. cooldown / dedup block path
4. ignored non-trigger path

Must be true before moving on:

```text
the first runtime loop is real, not just theoretical
```

## 6. Stage 3 - Hot Brain Foundations

Primary docs:

- `specs/12-hot-brain-runtime.md`
- `specs/13-hot-brain-execution-brief.md`
- `specs/15-world-slice-v1.md`
- `specs/16-world-slice-execution-brief.md`

Goal:

- define and stabilize the second coordination runtime as a bounded analysis layer

Scope:

- `WorldSlice`
- `CandidateSet`
- deterministic packet-driven Hot Brain pass

Must remain true:

- Hot Brain is read-only
- Hot Brain emits candidates only
- no direct world mutation

## 7. Stage 4 - Candidate Consumers

Primary docs:

- `specs/17-candidate-consumers.md`
- `specs/18-candidate-consumers-execution-brief.md`
- `specs/21-scheduler-normalized-outputs.md`
- `specs/22-scheduler-normalized-outputs-execution-brief.md`

Goal:

- normalize Hot Brain outputs before any consequence is allowed

Scope:

- `SchedulerConsumer`
- `MemoryConsumer`
- `SchedulerNormalizedOutput`
- bounded deterministic interpretation

Must be true before moving on:

```text
raw suggestions no longer directly imply runtime actions
```

## 8. Stage 5 - Memory Boundary Stabilization

Primary docs:

- `specs/19-memory-boundary.md`
- `specs/20-memory-boundary-execution-brief.md`

Goal:

- keep audit, evidence, packets, memory candidates, and cold memory semantically distinct

Scope:

- naming alignment
- future cold-memory record planning
- memory promotion boundaries

Must be true before moving on:

```text
the system knows what is trace, what is packet, what is candidate, and what is durable memory
```

## 9. Stage 6 - Scheduling Formalization

Primary docs:

- `specs/17-candidate-consumers.md`
- `specs/21-scheduler-normalized-outputs.md`

Goal:

- move from normalized scheduler-side outputs toward bounded real scheduling consequences

Scope:

- scheduler-side backlog records
- follow-up proposal creation rules
- review queue rules

Do not jump directly to:

- automatic large fan-out scheduling
- whole-graph autonomous coordination

## 10. Stage 7 - Canvas / Workflow Projections

Primary docs:

- `specs/23-canvas-workflow-views.md`

Goal:

- expose the same shared world through multiple human-facing views

Recommended projection order:

1. relationship view
2. scheduler view
3. evidence overlays
4. workflow view

Must remain true:

```text
canvas is a rendering surface, not a separate semantic system
```

## 11. Stage 8 - Transport Adapter Expansion

Primary docs:

- `specs/14-transport-adapter-boundary.md`

Goal:

- preserve upper runtime semantics while supporting more lower-layer transports

Examples:

- current tmux/hooks adapter
- future ACPX/ACP adapter

Important:

This stage is intentionally late, not because it is unimportant, but because:

```text
transport should adapt to the runtime semantics,
not redefine them before they are stable
```

## 12. What Should Not Be Done Too Early

Avoid these too early:

### 12.1 Cross-session semantic automation explosion

Do not immediately build:

- full graph reasoning
- unrestricted relation discovery loops
- autonomous scheduler chains

### 12.2 Delivery-channel coupling

Do not let:

- PTY send
- hook `additionalContext`
- filesystem artifact

redefine the packet model or guarded runtime semantics.

### 12.3 Memory collapse

Do not collapse:

- audit
- evidence
- packet
- candidate
- cold memory

into one storage concept.

## 13. Recommended Near-Term Agent Tasks

The next tasks for implementation agents should usually come from these buckets:

### Bucket A - Finish and harden MVP

- guarded context path
- runtime E2E
- packet consistency

### Bucket B - Harden Hot Brain V1

- slice invariants
- candidate invariants
- deterministic consumers

### Bucket C - Keep boundaries explicit

- memory boundary
- transport boundary
- scheduling normalization

## 14. Suggested Execution Order for Development Agents

If assigning implementation work to execution agents, prefer this order:

```text
1. Finish MVP E2E
2. Harden FeedbackPacket output
3. Harden WorldSlice/Hot Brain invariants
4. Implement candidate normalization
5. Align memory boundary
6. Add bounded scheduler-side formalization
7. Begin view-model projection work
8. Revisit transport adapters and ACPX integration
```

## 15. What Counts as “Architecturally Safe Progress”

A change is architecturally safe if it:

1. preserves lower/upper layer boundaries
2. does not bypass deterministic consumers
3. keeps packet semantics transport-neutral
4. does not turn Hot Brain into a free-form mutator
5. does not collapse memory layers together

## 16. What This Roadmap Is Optimizing For

This roadmap optimizes for:

- coherence
- auditability
- bounded automation
- progressive expansion

It intentionally does **not** optimize for:

- maximum short-term feature count
- immediate full autonomy
- protocol novelty before runtime stability

## 17. Final Statement

The project should be implemented in this order:

```text
stabilize the guarded runtime
 -> stabilize packet outputs
 -> stabilize Hot Brain as bounded analysis
 -> stabilize deterministic consumers
 -> stabilize memory boundary
 -> formalize scheduling
 -> project into views
 -> expand transport adapters
```

That is the current recommended implementation roadmap.
