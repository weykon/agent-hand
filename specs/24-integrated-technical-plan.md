# Integrated Technical Plan - Unified Architecture Summary

## 1. Purpose

This document is the integrated technical plan for the current design work.

It is the convergence document that gathers the major architecture decisions into one place.

Use this document when you need:

- a high-level architecture summary
- a stable handoff for future implementation
- a compressed reference for the whole system direction

## 2. Product Direction

Agent Hand is evolving from:

```text
a tmux session manager with useful side effects
```

into:

```text
a guarded, packet-driven, multi-session coordination runtime
with multiple user-facing projections over one shared world
```

## 3. Core Layers

ASCII:

```text
+-----------------------------------------------------------+
| Layer 6: UI / Projections                                 |
| tree / canvas / scheduler view / evidence view / workflow |
+-----------------------------------------------------------+
| Layer 5: Cold Memory                                      |
| durable reusable memory                                   |
+-----------------------------------------------------------+
| Layer 4: Coordination Runtime                             |
| feedback packets / hot brain / consumers                  |
+-----------------------------------------------------------+
| Layer 3: Guarded Live Runtime                             |
| proposal / evidence / guard / commit                      |
+-----------------------------------------------------------+
| Layer 2: Domain World                                     |
| sessions / groups / relations / trust / state             |
+-----------------------------------------------------------+
| Layer 1: Transport Adapters                               |
| tmux/hooks today, ACPX/ACP tomorrow                       |
+-----------------------------------------------------------+
```

## 4. Layer Summaries

### Layer 1 - Transport Adapters

Purpose:

- normalize incoming runtime events
- execute control operations
- deliver runtime projections

Examples:

- tmux + hook adapter
- future ACPX/ACP adapter

### Layer 2 - Domain World

Purpose:

- hold sessions, groups, relations, trust state, and runtime-visible entities

### Layer 3 - Guarded Live Runtime

Purpose:

- turn raw runtime events into guarded execution outcomes

Main path:

```text
HookEvent
  -> Proposal
  -> Evidence
  -> Guard
  -> GuardedCommit
  -> FeedbackPacket
```

### Layer 4 - Coordination Runtime

Purpose:

- reason over recent packets and bounded world slices
- produce candidates
- deterministically normalize them

Main path:

```text
FeedbackPacket
  -> WorldSlice
  -> Hot Brain
  -> CandidateSet
  -> Consumers
```

### Layer 5 - Cold Memory

Purpose:

- keep durable, reusable, queryable knowledge

### Layer 6 - UI / Projections

Purpose:

- project the shared world into human-facing views

## 5. The Main Runtime Loop

ASCII:

```text
external runtime event
   |
   v
Transport Adapter
   |
   v
HookEvent / RuntimeEvent
   |
   v
Guarded Live Runtime
   |
   v
FeedbackPacket
   |
   v
Coordination Runtime
   |
   v
Consumers / normalized outputs
   |
   v
future bounded proposals / memory promotion / views
```

## 6. Guarded Runtime Summary

This is the first stable vertical slice.

```text
Proposal
  = what the system wants to do

Evidence
  = what supports that proposal

Guard
  = deterministic approval or block

GuardedCommit
  = the formal result of that guarded turn

FeedbackPacket
  = the smallest coordination-facing outcome of that turn
```

## 7. FeedbackPacket Summary

`FeedbackPacket` is:

- derived
- compact
- transport-neutral
- coordination-facing

It is not:

- raw transcript
- world snapshot
- command payload
- long-term memory

ASCII:

```text
Guarded turn
   |
   v
FeedbackPacket
   |
   +--> scheduler input
   +--> injection source
   +--> hot brain input
   \--> human handoff projection
```

## 8. Hot Brain Summary

Hot Brain is:

```text
a bounded, packet-driven, read-only coordination analyzer
```

It:

- reads bounded `WorldSlice`
- emits `CandidateSet`
- does not directly mutate the world

V1 candidate types:

- `SchedulerHint`
- `MemoryCandidate`

## 9. WorldSlice Summary

`WorldSlice` is the bounded read lens for Hot Brain.

V1 slice taxonomy:

- `SessionTurnSlice`
- `NeighborhoodSlice`
- `CoordinationSlice`

V1 primary analyzed slice:

- `CoordinationSlice`

## 10. Consumer Summary

Consumers exist so that:

```text
Hot Brain suggestions do not directly become actions
```

Current conceptual consumers:

- `SchedulerConsumer`
- `MemoryConsumer`

Their job:

```text
Candidate
  -> deterministic interpretation
  -> normalized output
  -> later bounded consequence
```

## 11. Memory Summary

Memory is a ladder, not one thing.

ASCII:

```text
Audit
 -> Evidence
 -> FeedbackPacket
 -> MemoryCandidate
 -> ColdMemory
```

Each level serves a different purpose and must stay semantically distinct.

## 12. View / Canvas Summary

Canvas is not one semantic view.

It is a rendering surface for multiple projections:

- relationship view
- scheduler view
- evidence view
- workflow view

All of these must project from the same shared world.

## 13. A Concrete User Scenario

Scenario:

```text
Session A is active
User creates/forks Session B for a new task
Session C already produced an artifact/context that matters
User links A and C
Session B should begin with the right derived context from A and C
```

This flows through the architecture as:

ASCII:

```text
[A] completes guarded turn
   |
   v
FeedbackPacket(A)
   |
   +--> Hot Brain sees relation to C
   |
   v
SchedulerHint / MemoryCandidate / later proposal candidate
   |
   v
deterministic consumers
   |
   v
future bounded follow-up proposal for B
```

This is the kind of workflow the system is being designed to support.

## 14. Current Implementation State

The design is intentionally ahead of implementation in some layers.

Roughly:

### Already implemented or actively in progress

- guarded self-context path
- feedback packet V1
- hot brain V1 pure analyzer
- world slice V1 definitions

### Designed but not yet fully realized

- deterministic consumers with normalized outputs
- cold memory promotion path
- multi-view canvas projection
- adapter abstraction for ACPX/ACP

## 15. Architectural Guardrails

These are the main guardrails that keep the design coherent:

1. transport adapters do not define upper runtime semantics
2. guarded runtime stays separate from live REPL control
3. feedback packets stay transport-neutral
4. hot brain reads slices, not full world
5. hot brain emits candidates, not direct mutations
6. consumers normalize suggestions before consequences
7. cold memory is promoted, not inferred ad hoc
8. views project shared world state, not private UI logic

## 16. Recommended Implementation Order

High-level implementation order should remain:

### 1. Guarded runtime stability

- finish MVP path
- add runtime E2E

### 2. Packet stability

- keep `FeedbackPacket V1` frozen until runtime observations justify change

### 3. Hot Brain / slice / consumer stabilization

- keep bounded
- keep pure/deterministic where possible

### 4. Memory boundary and normalized outputs

- normalize before promoting or acting

### 5. Projection/view work

- relationship view first
- scheduler/evidence/workflow views after

## 17. What This Plan Is Good For

This integrated plan is good for:

- implementation sequencing
- onboarding future coding agents
- checking whether a new idea belongs to transport, runtime, memory, or view layer

## 18. What This Plan Intentionally Avoids

This plan does not assume:

- one giant autonomous planner
- direct AI mutation of world state
- transport-specific architecture
- immediate global graph intelligence

It prefers:

- bounded steps
- explicit layers
- deterministic consumers
- progressive expansion

## 19. Final Statement

The converged architecture is:

```text
transport adapters feed a guarded live runtime,
which emits feedback packets,
which feed a bounded coordination runtime,
which emits candidates,
which are normalized by deterministic consumers,
which later support memory, scheduling, and multi-view projections
over one shared world.
```

This is the current unified technical plan.
