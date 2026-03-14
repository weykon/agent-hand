# WorldSlice V1 - Execution Brief

## 1. Purpose

This is the execution brief for `WorldSlice V1`.

Primary design source:

- `specs/15-world-slice-v1.md`

Related docs:

- `specs/11-feedback-packet-v1.md`
- `specs/12-hot-brain-runtime.md`
- `specs/13-hot-brain-execution-brief.md`
- `specs/14-transport-adapter-boundary.md`

## 2. Goal

Stabilize the bounded read model for Hot Brain.

This does **not** mean implementing more runtime behavior.
It means making the slice shapes and builder semantics explicit and correct.

## 3. Scope

In scope:

- `SessionTurnSlice`
- `NeighborhoodSlice`
- `CoordinationSlice`
- clear builder semantics for ordering, trimming, and provenance

Out of scope:

- direct runtime integration
- scheduler mutation
- memory writes
- UI
- transport-specific logic

## 4. Recommended Work

### 4.1 Review current `hot_brain.rs`

Check that:

- `CoordinationSlice` remains the only active V1 analyzer input
- `SessionTurnSlice` and `NeighborhoodSlice` remain defined but not overused

### 4.2 Tighten builder invariants

Ensure builder logic is explicit about:

- timestamp sorting
- bounded trimming
- dedup behavior
- provenance retention

### 4.3 Add missing tests if needed

Focus on:

1. deterministic sort
2. bounded trim
3. dedup behavior
4. provenance retention when aggregating blockers/targets

## 5. Constraints

Do not:

- add direct world mutation
- add new candidate classes
- add packet rewriting
- expand slices to full graph reads
- add raw transcript material

## 6. Short Task Prompt

```text
Use specs/15-world-slice-v1.md as the primary design source.

Do a narrow implementation/review pass on the Hot Brain slice model:
- keep CoordinationSlice as the primary V1 analyzed slice
- keep SessionTurnSlice and NeighborhoodSlice defined for future phases
- tighten builder invariants around sorting, trimming, dedup, and provenance
- add tests only if needed to prove these invariants

Do not expand runtime behavior.
Do not add direct world mutation.
Do not add UI or transport logic.
```
