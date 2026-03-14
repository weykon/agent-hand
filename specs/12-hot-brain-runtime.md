# Hot Brain Runtime - Design Spec

## 1. Purpose

This document defines the **Hot Brain** runtime layer.

It exists to answer a new architectural need:

- the guarded live runtime can produce `FeedbackPacket`
- but the system still needs a bounded way to do short-horizon semantic reasoning
- that reasoning should help coordination without becoming a free-form world mutator

This document defines:

- what Hot Brain is
- what it reads
- what it produces
- what limits constrain it
- how it fits with the existing MVP and `FeedbackPacket V1`

## 2. Motivating Scenario

A simple but important workflow:

```text
Session A
  -> user is working in current context
  -> forks / creates Session B for a new task

Session C
  -> already produced an artifact or useful API/package context

User
  -> links A and C in the canvas / relationship graph

Session B
  -> should start with the right context from both A and C
```

This is one of the practical reasons the system needs a second coordination layer.

ASCII:

```text
        [Session C]
            |
            | relation / artifact relevance
            v
[Session A] ---> [Session B]
    |
    +-- user forks / creates B
    +-- user links A <-> C in canvas

Goal:
B should begin with the right derived context from A and C,
without manually reconstructing everything each time.
```

This cannot be solved well by:

- raw event logs alone
- live REPL control alone
- static memory alone

It needs a bounded coordination runtime.

## 3. What Hot Brain Is

Hot Brain is:

```text
a bounded, short-horizon, read-only coordination analyzer
```

More concretely:

- it reads a bounded slice of world state
- it performs local reasoning over recent, relevant state
- it produces **candidates**
- it does not directly mutate the world

### 3.1 One-sentence definition

```text
Hot Brain is a packet-driven coordination analyzer over bounded world slices.
```

### 3.2 What it is not

Hot Brain is not:

- the ECS world
- the memory store
- the scheduler itself
- the guard
- a delivery channel
- a general-purpose autonomous agent with write authority

## 4. Architectural Position

### 4.1 Full vertical position

ASCII:

```text
Layer 1: Live Runtime
HookEvent
  -> guarded execution
  -> FeedbackPacket

Layer 2: Hot Brain Runtime
FeedbackPacket + bounded world slices
  -> semantic/local coordination analysis
  -> candidate outputs

Layer 3: Deterministic Consumers
  -> scheduler
  -> memory ingest
  -> relation update
  -> future packet enrichers
```

### 4.2 Why this layer exists

Without Hot Brain:

- the system can record and guard
- but it cannot do bounded semantic selection over multiple sessions

With Hot Brain:

- the system can notice relevant nearby results
- the system can propose useful coordination
- the system still keeps deterministic ownership of world mutation

## 5. Relationship to Existing Concepts

### 5.1 Relationship to FeedbackPacket

`FeedbackPacket` is not Hot Brain.

Instead:

```text
Live runtime
  -> emits FeedbackPacket

Hot Brain
  -> consumes FeedbackPacket
  -> consumes bounded world slices
  -> emits candidates
```

Short version:

```text
FeedbackPacket is the main input currency of Hot Brain,
not the same thing as Hot Brain itself.
```

### 5.2 Relationship to Memory

Cold Memory:

- long-lived
- archival
- searchable
- replayable

Hot Brain:

- short-lived
- near-current
- bounded
- coordination-focused

ASCII:

```text
            +----------------------+
            |     Cold Memory      |
            | long-term storage    |
            +----------------------+

            +----------------------+
            |      Hot Brain       |
            | short-term analysis  |
            +----------------------+
```

### 5.3 Relationship to pi-mono

In `pi-mono`, a useful mental reference is:

```text
transformContext
  = reshape internal context before model input

convertToLlm
  = convert reshaped context into provider-compatible format
```

Hot Brain is more analogous to a **coordination-side transform layer** than to provider translation.

That is:

```text
Hot Brain
  = decide what coordination-relevant derived state should exist

Delivery / projection
  = decide how that derived state is delivered or rendered
```

So the analogy is useful, but the layers must remain separate.

## 6. Core Rule

This is the most important rule in the whole document:

```text
Hot Brain may read bounded world slices and emit candidates.
Hot Brain may not directly mutate core world state.
```

That means:

- no direct component mutation
- no direct relation trust promotion
- no direct scheduler queue changes
- no direct guarded commit publication

Everything Hot Brain produces must go through deterministic consumers.

## 7. The WorldSlice Concept

### 7.1 Why slices exist

If Hot Brain reads the full world, several things break:

- token budgets
- reasoning focus
- testability
- reproducibility
- safety

So Hot Brain must only read a bounded subset of the world.

That subset is called a `WorldSlice`.

### 7.2 Definition

```text
WorldSlice
= a bounded, task-relevant, serializable subset of ECS world state
```

ASCII:

```text
+--------------------------------------+
|              ECS World               |
|--------------------------------------|
| sessions, groups, relations, logs,   |
| packets, commits, evidence, memory   |
+------------------+-------------------+
                   |
                   v
         +----------------------+
         |      WorldSlice      |
         | only what matters    |
         +----------------------+
```

### 7.3 Slice types

For V1, define three slice types.

#### A. SessionTurnSlice

Reads one session's recent local state.

```text
- session identity
- current status
- recent hook events
- recent progress tail
- latest guarded commits
- latest feedback packets
```

Use:

- self-follow-up reasoning
- local packet refinement

#### B. NeighborhoodSlice

Reads one session plus nearby related state.

```text
- session
- same-group sessions (bounded)
- confirmed direct relations
- latest packets from direct neighbors
- recent shared blockers/artifacts
```

Use:

- cross-session relevance reasoning
- context source selection
- dependency hints

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

#### C. CoordinationSlice

Reads the current coordination frontier.

```text
- recent feedback packets
- pending blocked items
- unresolved blockers
- recent urgency changes
- current scheduler-visible targets
```

Use:

- scheduler hint generation
- escalation reasoning
- memory-worthy candidate selection

## 8. Candidate Outputs

Hot Brain does not produce final actions.
It produces candidate outputs.

### 8.1 Why candidates

If Hot Brain could directly act, it would collapse:

- reasoning
- mutation
- authorization

into one layer.

That is explicitly not allowed.

### 8.2 Candidate types

V1 supports three conceptual candidate classes:

```text
PacketCandidate
SchedulerHint
MemoryCandidate
```

### 8.3 V1 scope decision

To keep V1 bounded, the recommended implementation target is:

```text
Hot Brain V1 emits:
- SchedulerHint
- MemoryCandidate

Hot Brain V1 does NOT emit:
- direct world mutations
- direct guarded commits
- free-form packet rewrites
```

`PacketCandidate` remains a future extension if needed.

Reason:

- `FeedbackPacket V1` already exists downstream of the guarded runtime
- we do not need Hot Brain to redefine the packet before we prove coordination value

## 9. Candidate Definitions

### 9.1 SchedulerHint

Definition:

```text
a non-authoritative recommendation about what coordination action may be useful next
```

Examples:

- wake session B
- keep session C paused
- route blocker from A to C
- ask human for relation confirmation

### 9.2 MemoryCandidate

Definition:

```text
a candidate statement that this turn produced something worth promoting into longer-lived memory
```

Examples:

- stable decision
- reusable finding
- repeated blocker pattern
- artifact relation worth indexing

### 9.3 PacketCandidate (future)

Definition:

```text
a candidate refinement or augmentation of a turn summary before packet finalization
```

This is intentionally deferred.

## 10. Limits

Hot Brain must be limited in four independent ways.

### 10.1 Step limit

Each invocation may only do a small number of reasoning steps.

Initial recommendation:

```text
one trigger
  -> build one slice
  -> run one analysis pass
  -> emit one candidate set
  -> stop
```

No recursive self-triggering.

ASCII:

```text
Trigger
  -> Step 1: build slice
  -> Step 2: analyze
  -> Step 3: emit candidates
STOP
```

### 10.2 Scope limit

Hot Brain may only see a bounded graph neighborhood.

Initial recommendation:

```text
Level 0 = self only
Level 1 = self + direct confirmed relations
Level 2 = self + bounded same-group window
```

V1 should default to:

```text
Level 0 or Level 1 only
```

### 10.3 Material limit

Each analysis must cap how much raw material it can consume.

Initial recommendation:

```text
- max 3 recent packets
- max 20 recent events
- max 50 progress lines
- max 4 neighbor sessions
- max 1 group neighborhood
```

### 10.4 Output limit

Each analysis may only emit a bounded candidate set.

Initial recommendation:

```text
- max 3 scheduler hints
- max 3 memory candidates
- max 0 packet candidates in V1
```

## 11. Trigger Model

### 11.1 Packet-driven first

Hot Brain should not start as a global fixed-tick system.

Recommended first model:

```text
new packet
  -> evaluate whether a slice should be built
  -> run Hot Brain once
```

This is effectively:

```text
packet-driven coordination runtime
```

### 11.2 Future optional ticks

Tick-like behavior may be added later for:

- delayed retries
- cooldown expiry
- reminder wakeups
- long-running unresolved blocker review

But those should be added only after the packet-driven path proves useful.

## 12. Core Data Flow

ASCII:

```text
                ┌─────────────────────────────┐
                │         ECS World           │
                │-----------------------------│
                │ session/group/relation/logs │
                └──────────────┬──────────────┘
                               |
                               v
                ┌─────────────────────────────┐
                │      WorldSlice Builder     │
                └──────────────┬──────────────┘
                               |
                               v
                ┌─────────────────────────────┐
                │       Hot Brain V1          │
                │-----------------------------│
                │ bounded semantic analysis   │
                └──────────────┬──────────────┘
                               |
                               v
                ┌─────────────────────────────┐
                │       Candidate Set         │
                │-----------------------------│
                │ scheduler hints             │
                │ memory candidates           │
                └──────────────┬──────────────┘
                               |
                               v
                ┌─────────────────────────────┐
                │ Deterministic Consumers     │
                │-----------------------------│
                │ scheduler / memory ingest   │
                └─────────────────────────────┘
```

## 13. V1 Output Shape

Suggested V1 shape:

```rust
pub struct CandidateSet {
    pub scheduler_hints: Vec<SchedulerHint>,
    pub memory_candidates: Vec<MemoryCandidate>,
}
```

Where:

```rust
pub struct SchedulerHint {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub target_session_ids: Vec<String>,
    pub reason: String,
    pub urgency_level: RiskLevel,
}

pub struct MemoryCandidate {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub kind: String,
    pub summary: String,
    pub source_refs: Vec<String>,
}
```

This is illustrative. Exact naming can change, but the semantics should stay.

## 14. Relationship to Canvas

Canvas is one of the user-facing surfaces that helps establish:

- explicit relations
- visible graph topology
- human understanding of session neighborhoods

Hot Brain may later consume relation state influenced by canvas actions, but:

```text
canvas is not the hot brain
hot brain is not the canvas
```

Canvas helps create and visualize graph state.
Hot Brain helps reason over bounded graph state.

## 15. Safety and Control Model

### 15.1 Hard constraints

Hot Brain must obey these constraints:

1. read-only over bounded slices
2. bounded candidate count
3. no direct core world mutation
4. no direct guard bypass
5. no implicit cross-workspace global reasoning in V1

### 15.2 Why this matters

Without these constraints, Hot Brain becomes:

- opaque
- hard to test
- hard to reproduce
- prone to token explosion
- effectively a second uncontrolled agent

This document explicitly rejects that direction.

## 16. Implementation Phases

### Phase H0 - Documentation freeze

Done when:

- this Hot Brain model is accepted
- packet semantics remain frozen in `FeedbackPacket V1`

### Phase H1 - Data shape only

Goal:

- define `WorldSlice`
- define `CandidateSet`
- no semantic analysis yet

Deliverables:

- slice structs
- candidate structs
- no runtime integration required

### Phase H2 - Deterministic packet-driven analyzer

Goal:

- build one deterministic Hot Brain pass
- emit simple scheduler hints from bounded packets

Deliverables:

- on new packet, build `CoordinationSlice`
- emit bounded scheduler hints

### Phase H3 - Memory candidate emission

Goal:

- identify bounded memory-worthy outcomes

Deliverables:

- memory candidate production
- handoff to memory ingest path

### Phase H4 - Optional semantic analyzers

Goal:

- add pluggable analyzers, potentially agent-assisted or WASM-based

Deliverables:

- analyzer interface
- bounded input/output enforcement
- still no direct world mutation

## 17. Recommended Next Execution Brief

Do not implement Hot Brain immediately if:

- Phase A guarded context E2E is still missing
- FeedbackPacket V1 is not yet stable in runtime output

Only start Hot Brain implementation after the guarded context path is stable.

## 18. Final Statement

Hot Brain is not a second free-form agent.

It is:

```text
a bounded, packet-driven coordination analyzer over world slices
that emits candidates for deterministic systems to consume.
```

That is the stable direction for future work.
