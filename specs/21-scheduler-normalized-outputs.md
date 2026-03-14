# Scheduler Normalized Outputs - From Hint to Formal Scheduling State

## 1. Purpose

This document defines the layer after `SchedulerHint`.

It answers:

```text
Once Hot Brain suggests something scheduling-related,
what is the first formal scheduling object the system should produce?
```

This document exists because:

- `SchedulerHint` is only a suggestion
- the system should not execute raw hints directly
- the scheduler needs a deterministic internal language

## 2. One-Sentence Definition

```text
Scheduler normalized outputs are bounded, deterministic scheduling-side records
derived from scheduler hints before any real runtime consequence is allowed.
```

## 3. Why This Layer Exists

Without this layer:

```text
SchedulerHint
  -> immediate action
```

That would be too eager and too unstable.

Problems:

- hints are advisory, not authoritative
- hints may conflict
- hints may be weak, redundant, or stale
- hints may need human review

So the system needs:

```text
Hint
  -> deterministic adjudication
  -> formal scheduling record
  -> only then possible downstream action
```

## 4. Position in the Stack

ASCII:

```text
FeedbackPacket
   |
   v
Hot Brain
   |
   v
SchedulerHint
   |
   v
SchedulerConsumer
   |
   v
SchedulerNormalizedOutput
   |
   v
Later scheduler proposal / pending coordination / review
```

Short version:

```text
Hint = suggestion language
Normalized output = scheduler language
```

## 5. What SchedulerHint Is

`SchedulerHint` means:

```text
something may be worth coordinating
```

Examples:

- resolve blocker
- escalate urgency
- coordinate next steps

But `SchedulerHint` is not yet:

- a scheduling task
- a queue entry
- a runtime command

## 6. What SchedulerNormalizedOutput Is

`SchedulerNormalizedOutput` means:

```text
the scheduler has formally classified how it will treat the hint
```

This is still not necessarily execution.
It is formal internal scheduling state.

## 7. Core Rule

Hard rule:

```text
No direct scheduling action may be taken from raw SchedulerHint.
All scheduling consequences must first pass through normalized outputs.
```

## 8. Layered Interpretation

ASCII:

```text
Layer A: Semantic Suggestion
  SchedulerHint

Layer B: Deterministic Scheduling Interpretation
  SchedulerNormalizedOutput

Layer C: Runtime Consequence
  proposal / queue item / review request / ignore
```

## 9. Minimal V1 Normalized Output

Suggested V1 shape:

```rust
pub struct SchedulerNormalizedOutput {
    pub id: String,
    pub trace_id: String,
    pub source_session_id: String,
    pub target_session_ids: Vec<String>,
    pub disposition: SchedulerDisposition,
    pub reason: String,
    pub urgency_level: RiskLevel,
}
```

## 10. SchedulerDisposition

This is the most important field.

It answers:

```text
How is the scheduler treating this hint?
```

Suggested V1 values:

```rust
pub enum SchedulerDisposition {
    Ignore,
    RecordOnly,
    PendingCoordination,
    NeedsHumanReview,
    ProposeFollowup,
}
```

## 11. Meaning of Each Disposition

### 11.1 Ignore

Meaning:

```text
The hint is not useful enough to retain as scheduling state.
```

Use when:

- duplicate
- stale
- too weak
- unsupported by current policy

### 11.2 RecordOnly

Meaning:

```text
The hint is worth preserving for observability,
but not worth taking scheduling action on.
```

Use when:

- it may matter later
- it provides useful auditability
- immediate follow-up is not justified

### 11.3 PendingCoordination

Meaning:

```text
The hint has enough value to become a formal coordination item,
but not enough to directly generate a proposal yet.
```

Use when:

- blocker exists
- cross-session dependency is plausible
- more confirmation or additional inputs may still be needed

### 11.4 NeedsHumanReview

Meaning:

```text
The scheduler should not decide alone.
```

Use when:

- urgency is high
- impact is broad
- relation certainty is insufficient
- the potential effect is too risky

### 11.5 ProposeFollowup

Meaning:

```text
The hint is strong enough to justify creation of a bounded next-step proposal.
```

Use when:

- next step is clear
- impact is bounded
- required targets are known
- policy allows automated follow-up

## 12. Why This Is Better Than Direct Action

ASCII:

```text
Bad:
Hint -> wake session B

Good:
Hint -> normalized output -> maybe follow-up proposal later
```

This gives:

- clearer audit
- easier testing
- less accidental fan-out
- safer gradual automation

## 13. Consumer Responsibilities

`SchedulerConsumer` must do three things:

### 13.1 Deduplicate

Examples:

- multiple similar blocker hints collapse into one scheduling-side record
- repeated urgency hints do not create duplicates

### 13.2 Classify

Examples:

- strong hint -> `ProposeFollowup`
- weak hint -> `RecordOnly`
- risky hint -> `NeedsHumanReview`

### 13.3 Preserve traceability

Every normalized output should preserve:

- source session
- trace ID
- reason
- target set

## 14. What This Layer Must Not Do

This layer must not:

1. call tmux/session control
2. inject prompt text
3. directly mutate relation trust
4. directly rewrite packets
5. bypass guard semantics

It is scheduling interpretation only.

## 15. Interaction with Memory

Scheduling normalized outputs are not memory records.

But they may later be useful as:

- auditable coordination history
- source material for later pattern detection

Hard rule:

```text
Do not collapse scheduling normalized outputs into cold memory by default.
```

## 16. Interaction with Future Scheduler

Later, the scheduler may read normalized outputs and decide:

- no action
- enqueue a bounded coordination item
- create a formal new proposal
- wait for human confirmation

ASCII:

```text
SchedulerNormalizedOutput
   |
   +--> no-op
   +--> coordination backlog
   +--> formal proposal
   \--> human review queue
```

## 17. V1 Strategy

Recommended V1 strategy:

### Step 1

Define normalized output types and deterministic normalization logic.

### Step 2

Persist or log normalized outputs.

### Step 3

Do **not** yet hook them into direct runtime behavior.

That means V1 is still safe and bounded:

```text
suggest
 -> normalize
 -> persist
```

not yet:

```text
suggest
 -> normalize
 -> act
```

## 18. Acceptance Criteria

The scheduler normalized output layer is complete when:

1. raw `SchedulerHint` is no longer treated as execution-ready
2. normalized output shape exists
3. `SchedulerDisposition` is explicit
4. deterministic normalization exists
5. duplicate hints collapse cleanly
6. outputs remain traceable

## 19. Recommended Next Step

After this layer is stable, the next evolution can be:

```text
ProposeFollowup
  -> formal bounded proposal generation
```

But only later.

## 20. Final Statement

The scheduler should not act on hints.

It should first create formal scheduling-side records:

```text
SchedulerHint
  -> SchedulerNormalizedOutput
  -> later runtime consequence
```

That is the stable path to bounded automation.
