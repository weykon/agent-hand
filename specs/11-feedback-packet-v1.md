# FeedbackPacket V1 - Finalized Definition and Execution Plan

## 1. Decision

`FeedbackPacket` is now finalized for **V1**.

This is not a forever schema. It is the first stable schema that is good enough to:

- unblock the current MVP
- stabilize packet semantics before scheduler work
- prevent transport/control concerns from redefining the packet
- give later systems a shared coordination object

This document is the source of truth for `FeedbackPacket V1`.

## 2. One-Sentence Definition

```text
FeedbackPacket is the smallest structured, transport-neutral, coordination-facing artifact
produced after a guarded turn completes.
```

## 3. What It Is and What It Is Not

### 3.1 What it is

`FeedbackPacket` is:

- a **derived artifact**
- a **turn outcome capsule**
- a **coordination object**
- a **runtime-to-runtime handoff unit**

### 3.2 What it is not

`FeedbackPacket` is not:

- the ECS world itself
- a full world snapshot
- a raw transcript dump
- an execution command
- an authorization object
- a free-form query language

## 4. System Position

ASCII:

```text
Layer 1: Live Runtime
HookEvent
  -> proposal/evidence/guard
  -> guarded commit
  -> FeedbackPacket

Layer 2: Coordination Runtime
FeedbackPacket
  -> scheduler input
  -> memory seed
  -> injection source
  -> human handoff projection
```

Short version:

```text
For execution runtime: FeedbackPacket is an endpoint.
For coordination runtime: FeedbackPacket is an entrypoint.
```

## 5. Core Design Rules

### Rule 1: Derived, never authoritative

`FeedbackPacket` is always derived from guarded runtime state.

Hard rule:

```text
packet is derived from world state and execution outcome
packet does not own world state
```

### Rule 2: Transport-neutral

The packet must not assume:

- PTY prompt delivery
- hook `additionalContext`
- filesystem-only projection
- provider-native resume semantics

Hard rule:

```text
packet shape must not depend on delivery channel
```

### Rule 3: Coordination-facing

The packet exists primarily for:

- scheduler
- injection selection
- memory ingest
- human-readable projection

Not for low-level runtime transport.

### Rule 4: Compact by default

The packet should contain concise conclusions and references, not large bodies of raw data.

### Rule 5: Traceable

Every packet must be traceable back to:

- the producing turn
- the producing session
- the evidence and commit path that led to it

## 6. Relationships to Other Runtime Objects

ASCII:

```text
Proposal
  = what the system wants to do

Evidence
  = what supports the proposal

Attestation
  = how the guard judged it

GuardedCommit
  = the approved or blocked execution result

FeedbackPacket
  = the structured coordination outcome after that turn
```

Flow:

```text
Proposal -> Evidence -> Guard -> GuardedCommit -> FeedbackPacket
```

Important:

```text
FeedbackPacket is downstream of GuardedCommit.
It must never replace Proposal, Evidence, Attestation, or Commit.
```

## 7. FeedbackPacket V1 Schema

```rust
pub struct FeedbackPacket {
    pub packet_id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub created_at_ms: u64,

    pub goal: Option<String>,
    pub now: Option<String>,

    pub done_this_turn: Vec<String>,
    pub blockers: Vec<String>,
    pub decisions: Vec<String>,
    pub findings: Vec<String>,
    pub next_steps: Vec<String>,

    pub affected_targets: Vec<String>,
    pub source_refs: Vec<String>,

    pub urgency_level: RiskLevel,
    pub recommended_response_level: ResponseLevel,
}
```

## 8. Field Table

| Field | Required | Purpose | Main Consumers |
|-------|----------|---------|----------------|
| `packet_id` | yes | Stable packet identity | audit, storage |
| `trace_id` | yes | Link to proposal/evidence/commit chain | audit, debugging |
| `source_session_id` | yes | Session that produced the packet | scheduler, memory |
| `created_at_ms` | yes | Ordering, freshness, replay control | scheduler, memory |
| `goal` | optional | Concise statement of what this turn aimed to accomplish | human handoff, scheduler |
| `now` | optional | Concise statement of what should happen next | scheduler, handoff |
| `done_this_turn` | yes | Completed work items from this turn | scheduler, handoff, memory |
| `blockers` | yes | Outstanding blockers | scheduler, injection, human review |
| `decisions` | yes | Important choices that may affect later work | memory, handoff |
| `findings` | yes | Things learned that are worth reusing | memory, injection |
| `next_steps` | yes | Recommended follow-up tasks | scheduler, handoff |
| `affected_targets` | yes | Sessions/groups/paths likely affected | scheduler |
| `source_refs` | yes | References to evidence, commits, snapshots, progress entries | audit, memory |
| `urgency_level` | yes | Coordination/risk urgency | scheduler, review |
| `recommended_response_level` | yes | Hint for how strongly downstream systems should react | scheduler, injection |

## 9. Required vs Optional vs Forbidden

### 9.1 Required fields

These must always be present:

- `packet_id`
- `trace_id`
- `source_session_id`
- `created_at_ms`
- `done_this_turn`
- `blockers`
- `decisions`
- `findings`
- `next_steps`
- `affected_targets`
- `source_refs`
- `urgency_level`
- `recommended_response_level`

Even if the lists are empty, they must still exist.

### 9.2 Optional fields

These may be absent in V1:

- `goal`
- `now`

These are useful and should remain in the schema, but the MVP is allowed to populate them sparsely.

### 9.3 Forbidden fields

Do **not** add these to `FeedbackPacket Core`:

- raw terminal output
- full prompt text
- full tool result bodies
- full diffs
- unbounded markdown blobs
- free-form command payloads
- provider-specific transport details

Those belong in logs, evidence, artifacts, or projections.

## 10. Projections

The packet core is not the final shape consumed everywhere.
It gets projected.

ASCII:

```text
               +----------------------+
               | FeedbackPacket Core  |
               +----------+-----------+
                          |
       +------------------+------------------+
       |                  |                  |
       v                  v                  v
 [SchedulerInput]  [InjectionEnvelope]  [HumanHandoff]
       |
       v
 [MemorySeed]
```

### 10.1 Scheduler projection

Reads:

- `blockers`
- `next_steps`
- `affected_targets`
- `urgency_level`
- `recommended_response_level`

Purpose:

- decide if same session continues
- decide if another session should be proposed
- decide if human escalation is needed

### 10.2 Injection projection

Reads:

- `done_this_turn`
- `blockers`
- `decisions`
- `findings`
- `next_steps`
- `source_refs`

Purpose:

- build a provider-safe, compact `InjectionEnvelope`

Hard rule:

```text
FeedbackPacket is source material for injection,
not the final injected string.
```

### 10.3 Human handoff projection

Reads almost everything.

Purpose:

- generate YAML or Markdown handoff document
- keep packet core machine-oriented while still allowing rich human-readable output

### 10.4 Memory seed projection

Reads:

- `decisions`
- `findings`
- `blockers`
- `next_steps`
- `source_refs`

Purpose:

- feed memory enrichment or search indexing

## 11. Runtime Responsibilities

`FeedbackPacket` must support these five jobs:

### 11.1 Summarize turn outcome

Answer:

```text
What happened this turn?
```

### 11.2 Express what needs to happen next

Answer:

```text
What should the system consider doing next?
```

### 11.3 Define impact scope

Answer:

```text
Who or what is likely affected?
```

### 11.4 Provide a reaction hint

Answer:

```text
How strongly should downstream systems react?
```

### 11.5 Keep traceability intact

Answer:

```text
Where did this conclusion come from?
```

## 12. Non-Responsibilities

`FeedbackPacket` must not:

1. mutate ECS state directly
2. bypass guard decisions
3. become a general query DSL
4. replace evidence
5. replace audit logs
6. replace memory storage
7. hardcode one delivery channel

## 13. Packet Builder Boundary

To support future deterministic systems, plugins, WASM analyzers, or agent-assisted analysis, packet building should operate on a bounded slice.

Recommended concept:

```rust
pub struct WorldSlice {
    // Shape intentionally bounded and serializable
}
```

Flow:

```text
World
  -> WorldSlice
  -> PacketBuilder
  -> FeedbackPacket
```

This allows:

- deterministic packet builders
- future WASM analyzers
- future agent-assisted analyzers

while preserving:

- bounded inputs
- stable tests
- no arbitrary world mutation

Hard rule:

```text
packet builders may read slices
packet builders may not directly mutate core world state
```

## 14. V1 Execution Plan

### Step 1. Freeze V1 schema

Action:

- treat the schema in this document as final for current MVP work
- do not add new packet fields unless there is a demonstrated runtime need

### Step 2. Align current MVP implementation

Action:

- ensure `FeedbackPacket` written by current guarded context runtime matches this schema
- allow sparse `goal` and `now` for now
- make sure empty list fields are still written

### Step 3. Add projection boundaries in docs and code comments

Action:

- make it explicit in code that packet core is not prompt text
- make it explicit that scheduler/injection/handoff are projections

### Step 4. Add runtime E2E before packet expansion

Action:

- prove the packet appears end-to-end on:
  - approve path
  - block path
  - cooldown block path

### Step 5. Only after E2E, discuss Phase B consumers

Action:

- scheduler consumption
- memory seed projection
- human handoff projection

Do **not** expand packet semantics before E2E is stable.

## 15. Implementation Notes For The Development Agent

When working with `FeedbackPacket V1`:

- preserve transport neutrality
- prefer refs over raw content
- keep fields explicit, not overloaded
- do not add provider-specific or tmux-specific semantics
- do not collapse packet with evidence or commit

## 16. What To Discuss Next

With `FeedbackPacket V1` now frozen, the next architecture topic should be:

```text
Hot Brain / dynamic runtime layer
```

Specifically:

- what data belongs in "hot" dynamic state
- what step limits or query limits constrain it
- how it reads world slices safely
- whether it produces packet candidates, scheduler hints, or memory candidates

That discussion should start **after** accepting this packet schema as the stable handoff core.
