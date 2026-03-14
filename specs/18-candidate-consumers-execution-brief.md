# Candidate Consumers - Execution Brief

## 1. Purpose

This is the execution brief for the next design-aligned implementation step after `WorldSlice V1`.

Primary design source:

- `specs/17-candidate-consumers.md`

Related docs:

- `specs/12-hot-brain-runtime.md`
- `specs/13-hot-brain-execution-brief.md`
- `specs/15-world-slice-v1.md`

## 2. Goal

Implement the smallest deterministic consumer layer for Hot Brain outputs.

That means:

- do not add more AI
- do not add more transport logic
- do not add direct world mutation

Instead:

- define normalized output types
- define deterministic consumption paths
- keep everything bounded and auditable

## 3. Scope

In scope:

- consumer-side normalized types for:
  - scheduler hints
  - memory candidates
- deterministic normalization functions
- bounded, testable behavior

Out of scope:

- direct scheduling actions
- direct memory DB writes
- direct world mutation
- UI
- transport adapters

## 4. Recommended Work

### 4.1 Define normalized consumer output types

Examples:

- `SchedulerDecision`
- `MemoryIngestEntry`

### 4.2 Implement deterministic normalization

Examples:

- dedupe scheduler hints
- reject low-value or redundant hints
- preserve traceability
- normalize memory candidate summaries and source refs

### 4.3 Add tests

Recommended coverage:

1. scheduler hints normalize deterministically
2. memory candidates normalize deterministically
3. source refs are preserved
4. duplicate candidates collapse safely
5. no world mutation side effects occur

## 5. Constraints

Do not:

- trigger session control
- emit new guarded commits
- rewrite packets
- add full scheduler behavior
- add full memory ingestion behavior

This is still a bounded intermediate layer.

## 6. Short Task Prompt

```text
Use specs/17-candidate-consumers.md as the primary design source.

Implement a narrow deterministic consumer layer for Hot Brain outputs:
- define normalized output types for SchedulerHint and MemoryCandidate
- add deterministic normalization functions
- preserve traceability
- keep the behavior bounded and auditable

Do not add direct world mutation, direct scheduling, direct memory writes, UI, or transport logic.
```
