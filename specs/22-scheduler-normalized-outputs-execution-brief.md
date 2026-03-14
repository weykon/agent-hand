# Scheduler Normalized Outputs - Execution Brief

## 1. Purpose

This is the execution brief for the scheduler normalization layer.

Primary design source:

- `specs/21-scheduler-normalized-outputs.md`

Related docs:

- `specs/17-candidate-consumers.md`
- `specs/18-candidate-consumers-execution-brief.md`

## 2. Goal

Add the smallest deterministic layer after `SchedulerHint`.

That means:

- define a normalized scheduler-side output type
- define `SchedulerDisposition`
- add normalization rules

Do **not** add direct runtime action yet.

## 3. In Scope

In scope:

- `SchedulerNormalizedOutput`
- `SchedulerDisposition`
- deterministic normalization of scheduler hints
- dedup / traceability tests

Out of scope:

- actual scheduling actions
- session wakeups
- tmux / ACP control
- new proposals with side effects

## 4. Recommended Work

### 4.1 Define types

Add:

- `SchedulerNormalizedOutput`
- `SchedulerDisposition`

### 4.2 Implement deterministic normalization

Examples:

- weak or duplicate hint -> `Ignore` or `RecordOnly`
- risk-sensitive hint -> `NeedsHumanReview`
- bounded, clear next step -> `ProposeFollowup`

### 4.3 Preserve traceability

Ensure normalized outputs retain:

- source session
- trace ID
- reason
- targets

## 5. Constraints

Do not:

- trigger actual session control
- mutate world state
- inject prompts
- collapse outputs into memory

This layer is formal scheduling interpretation only.

## 6. Short Task Prompt

```text
Use specs/21-scheduler-normalized-outputs.md as the primary design source.

Implement a narrow deterministic normalization layer for SchedulerHint:
- define SchedulerNormalizedOutput
- define SchedulerDisposition
- normalize raw hints into bounded scheduler-side outputs
- preserve traceability
- add tests for dedup and deterministic classification

Do not add direct runtime scheduling behavior, UI, transport logic, or world mutation.
```
