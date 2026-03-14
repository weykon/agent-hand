# Candidate Consumers - Deterministic Consumption of Hot Brain Outputs

## 1. Purpose

This document defines how Hot Brain candidate outputs are consumed.

It answers a simple but important question:

```text
Hot Brain can suggest things.
Who is allowed to act on those suggestions?
```

This is the next step after:

- `FeedbackPacket V1`
- `Hot Brain V1`
- `WorldSlice V1`

## 2. Why This Document Exists

Without a consumer boundary, candidate outputs become dangerous.

Example failure mode:

```text
Hot Brain emits a hint
  -> hint directly mutates world
  -> no deterministic gate
  -> no clear audit boundary
  -> no stable ownership
```

That is explicitly not allowed.

The purpose of this document is to define a safe path:

```text
Candidate
  -> deterministic consumer
  -> normalized decision
  -> world mutation or persistence (if allowed)
```

## 3. One-Sentence Definition

```text
Candidate consumers are deterministic systems that translate bounded Hot Brain suggestions into bounded, auditable runtime actions.
```

## 4. Core Principle

Hot Brain is allowed to suggest.

Consumers are allowed to decide.

Hard rule:

```text
Hot Brain never directly mutates core world state.
Only deterministic consumers may convert candidate outputs into system actions.
```

## 5. The Main Flow

ASCII:

```text
FeedbackPacket
   |
   v
WorldSlice
   |
   v
Hot Brain
   |
   v
CandidateSet
   |
   +--> SchedulerConsumer
   +--> MemoryConsumer
   \--> Future RelationConsumer
```

## 6. Candidate Types in V1

V1 currently recognizes:

```text
SchedulerHint
MemoryCandidate
```

Future possible types:

```text
RelationCandidate
PacketCandidate
ReviewCandidate
```

But those are not part of the current V1 implementation target.

## 7. SchedulerHint

### 7.1 What it means

`SchedulerHint` is:

```text
a bounded recommendation that some coordination action may be useful
```

Examples:

- resolve blocker
- escalate urgency
- coordinate next steps

### 7.2 What it is not

It is not:

- a new session proposal by itself
- a scheduling command
- an approved action

So this is wrong:

```text
SchedulerHint -> immediately wake another session
```

And this is right:

```text
SchedulerHint -> SchedulerConsumer -> deterministic decision -> possible proposal
```

## 8. MemoryCandidate

### 8.1 What it means

`MemoryCandidate` is:

```text
a bounded suggestion that something may be worth promoting into longer-lived memory
```

Examples:

- decision worth storing
- finding worth indexing
- repeated blocker pattern

### 8.2 What it is not

It is not:

- a fully accepted memory record
- a direct write into long-term storage
- a search index entry by itself

So this is wrong:

```text
MemoryCandidate -> write directly into permanent memory
```

And this is right:

```text
MemoryCandidate -> MemoryConsumer -> deterministic normalization -> accepted memory entry
```

## 9. Consumer Types

V1 should define two deterministic consumers.

### 9.1 SchedulerConsumer

Purpose:

```text
translate scheduler hints into bounded scheduling-side results
```

Allowed responsibilities:

- rank hints
- deduplicate hints
- discard weak or redundant hints
- convert accepted hints into scheduler-side proposals or queue markers

Disallowed responsibilities:

- directly execute session control actions
- bypass guarded runtime
- create unbounded fan-out

ASCII:

```text
SchedulerHint[]
    |
    v
[SchedulerConsumer]
    |
    +--> discard
    +--> normalize
    \--> bounded scheduler-side output
```

### 9.2 MemoryConsumer

Purpose:

```text
translate memory candidates into bounded memory-side ingestion results
```

Allowed responsibilities:

- deduplicate repeated items
- attach stable refs
- classify accepted memory entries
- write accepted normalized entries to memory ingest path

Disallowed responsibilities:

- free-form summarization of unrelated world state
- cross-workspace global scans in V1
- rewriting packet semantics

ASCII:

```text
MemoryCandidate[]
    |
    v
[MemoryConsumer]
    |
    +--> discard
    +--> normalize
    \--> bounded memory ingest output
```

## 10. Deterministic Consumer Boundary

Consumers must obey these rules:

### Rule 1: deterministic in / deterministic out

Given the same candidate set and same visible state, the same decision should result.

### Rule 2: bounded side effects

Consumers may not create unbounded chains of downstream actions.

### Rule 3: explicit normalization

Consumers should convert:

```text
candidate form
```

into:

```text
runtime-owned normalized form
```

before persistence or proposal emission.

### Rule 4: traceability preserved

Consumers must carry forward:

- source session
- trace ID
- source refs

## 11. Candidate -> Consumer -> Output Model

ASCII:

```text
CandidateSet
├─ SchedulerHint[]
│    |
│    v
│  SchedulerConsumer
│    |
│    v
│  SchedulerDecision[]
│
└─ MemoryCandidate[]
     |
     v
   MemoryConsumer
     |
     v
   MemoryIngestEntry[]
```

## 12. Normalized Consumer Outputs

This document does not require a final code shape yet, but conceptually the consumer outputs should be more concrete than candidates and less raw than world mutations.

### 12.1 SchedulerDecision

Conceptually:

```text
an accepted or rejected scheduling interpretation of a scheduler hint
```

Examples:

- ignore
- mark as pending coordination
- create bounded proposal
- escalate for human review

### 12.2 MemoryIngestEntry

Conceptually:

```text
a normalized memory-side record prepared for ingestion
```

Examples:

- accepted decision memory
- accepted finding memory
- accepted repeated blocker pattern

## 13. What Consumers Must Not Do

Consumers must not:

1. redefine `FeedbackPacket`
2. perform arbitrary world reads beyond allowed slice or narrow local metadata
3. directly control tmux / PTY / ACP transport
4. collapse into delivery logic
5. become hidden AI systems

The whole point is to keep them deterministic and bounded.

## 14. V1 Consumption Strategy

Recommended V1 strategy:

### SchedulerConsumer V1

Allowed outcomes:

- keep hint in log only
- emit normalized scheduler-side records
- no direct scheduling action yet

Reason:

- scheduler semantics are still emerging
- current priority is to preserve traceable, bounded outputs

### MemoryConsumer V1

Allowed outcomes:

- keep normalized memory entries in audit/log form
- no heavy semantic indexing yet

Reason:

- long-term memory semantics are still evolving
- current priority is to build reliable ingest boundaries

In short:

```text
V1 consumers may normalize and persist,
without yet driving large runtime consequences.
```

## 15. Staging Model

### Stage C0 - Candidate only

```text
Hot Brain emits candidates
Consumers do not yet act
```

### Stage C1 - Normalization

```text
Consumers normalize candidates into deterministic outputs
Outputs logged/persisted
```

### Stage C2 - Bounded proposals

```text
SchedulerConsumer may create bounded proposals
MemoryConsumer may create bounded ingest entries
```

### Stage C3 - Real downstream activation

```text
Deterministic outputs begin driving later runtime systems
```

This staged approach prevents a premature collapse into uncontrolled orchestration.

## 16. Relationship to Hot Brain

This document exists partly to preserve a clean role split:

```text
Hot Brain
  = suggestion engine

Consumers
  = deterministic adjudicators
```

ASCII:

```text
Hot Brain: "Maybe these things matter."
Consumer:  "Only these bounded things are accepted."
```

## 17. Relationship to Future Runtime

Later, the system may evolve toward:

```text
FeedbackPacket
   -> Hot Brain
   -> CandidateSet
   -> Consumers
   -> bounded new proposals / memory updates
```

But that future path should still preserve:

- deterministic consumption
- explicit audit boundary
- bounded propagation

## 18. Execution Plan

### Phase C0

Done when:

- this document is accepted

### Phase C1

Next implementation target:

- define normalized consumer output types
- no direct world mutation yet

### Phase C2

Later target:

- let consumers persist normalized outputs

### Phase C3

Future target:

- allow scheduler-side bounded proposal generation

## 19. Recommended Next Brief

The next implementation brief should focus on:

```text
define normalized consumer outputs
without connecting them to large runtime consequences yet
```

## 20. Final Statement

Hot Brain suggestions become safe only when they pass through deterministic consumers.

That means:

```text
Candidate
  -> deterministic consumer
  -> normalized output
  -> later bounded runtime consequence
```

That is the intended control architecture.
