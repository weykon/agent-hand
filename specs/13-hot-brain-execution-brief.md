# Hot Brain V1 - Execution Brief

## 1. Purpose

This is the execution brief for the first implementation phase of Hot Brain.

Primary design source:

- `specs/12-hot-brain-runtime.md`

Related docs:

- `specs/10-boundary-and-e2e-plan.md`
- `specs/11-feedback-packet-v1.md`

## 2. Implementation Prerequisite

Do not start this work until:

1. `FeedbackPacket V1` is accepted as frozen
2. Phase A guarded context runtime E2E is stable

Hot Brain must build on a stable packet-producing runtime.

## 3. V1 Goal

Implement only the smallest safe slice of Hot Brain:

```text
FeedbackPacket
  -> bounded CoordinationSlice
  -> Hot Brain analysis
  -> CandidateSet
     - SchedulerHint[]
     - MemoryCandidate[]
```

Do **not** implement direct world mutation.

## 4. In Scope

### 4.1 Types only

Add minimal types for:

- `WorldSlice`
- `CoordinationSlice`
- `CandidateSet`
- `SchedulerHint`
- `MemoryCandidate`

### 4.2 Input source

V1 Hot Brain should consume:

- recent `FeedbackPacket`
- bounded recent session/coordination metadata

Prefer:

```text
packet-driven input
```

Do not make V1 depend on:

- full graph walks
- full logs
- all sessions
- raw transcript blobs

### 4.3 Outputs

V1 should emit only:

- scheduler hints
- memory candidates

## 5. Out of Scope

Do **not** implement:

- packet candidate generation
- direct packet rewrites
- direct relation trust mutation
- direct scheduler queue mutation
- direct memory writes
- global workspace reasoning
- full semantic AI analyzer
- WASM runtime
- UI / canvas overlays

## 6. Limits

V1 must enforce these limits:

### 6.1 Step limit

One trigger may:

1. build one slice
2. run one analyzer pass
3. emit one candidate set
4. stop

### 6.2 Scope limit

V1 should operate over:

```text
CoordinationSlice only
```

No full graph traversal.

### 6.3 Material limit

Recommended defaults:

- max 3 recent feedback packets
- max 4 affected targets total
- max 20 recent event references

### 6.4 Output limit

- max 3 scheduler hints
- max 3 memory candidates

## 7. Suggested File Targets

Potential implementation locations:

- `src/agent/hot_brain.rs`
- `src/agent/mod.rs`
- `src/agent/runner.rs`

If a system is added:

- `src/agent/systems/hot_brain.rs`
- `src/agent/systems/mod.rs`

Use judgment, but keep the implementation narrow.

## 8. Suggested Runtime Shape

ASCII:

```text
FeedbackPacket (approved path only)
      |
      v
CoordinationSliceBuilder
      |
      v
HotBrainV1
      |
      v
CandidateSet
      |
      +--> scheduler hints log
      \--> memory candidates log
```

V1 may stop at audit/log emission if no deterministic consumer exists yet.

That is acceptable.

## 9. Acceptance Criteria

The implementation is complete when:

1. a bounded slice type exists
2. a Hot Brain analysis pass exists
3. a candidate set type exists
4. candidate counts are bounded
5. no direct world mutation occurs
6. outputs are traceable to input packet(s)
7. tests prove limits and deterministic behavior

## 10. Recommended Tests

Add tests for:

1. one input packet -> emits bounded scheduler hints
2. one input packet -> emits bounded memory candidates
3. too many inputs -> slice builder trims to limit
4. output count never exceeds configured cap
5. no mutation side effects occur in analysis pass

## 11. Short Task Prompt

```text
Implement Hot Brain V1 from specs/12-hot-brain-runtime.md and specs/13-hot-brain-execution-brief.md.

Scope:
- define bounded world slice types
- define candidate output types
- implement a small deterministic Hot Brain analysis pass
- consume recent FeedbackPacket inputs only
- emit SchedulerHint[] and MemoryCandidate[] only
- enforce step/material/output limits

Do not:
- mutate world state
- emit guarded commits
- rewrite FeedbackPacket
- add UI
- add WASM
- add cross-workspace reasoning
```
