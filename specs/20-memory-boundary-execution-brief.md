# Memory Boundary - Execution Brief

## 1. Purpose

This is the execution brief for the memory-boundary stage.

Primary design source:

- `specs/19-memory-boundary.md`

Related docs:

- `specs/11-feedback-packet-v1.md`
- `specs/17-candidate-consumers.md`

## 2. Goal

The immediate goal is not to implement full cold memory.

The goal is to make the codebase and next-stage design align with the layered memory model:

```text
Audit
 -> Evidence
 -> FeedbackPacket
 -> MemoryCandidate
 -> ColdMemory
```

## 3. In Scope

In scope:

- documentation alignment
- naming alignment
- minimal type additions if required for future cold-memory record shape
- keeping lower layers separate in code comments and docs

Out of scope:

- full memory DB
- semantic search layer
- large refactors of audit storage
- UI

## 4. Recommended Work

### 4.1 Review current runtime objects

Check:

- audit logs
- evidence records
- feedback packets
- memory candidates

Confirm they are not semantically collapsing.

### 4.2 Add conceptual cold-memory record type if useful

This may be added as a placeholder type only if it improves clarity.

### 4.3 Avoid premature merging

Do not:

- turn packets directly into memory
- turn candidates directly into storage rows without normalization

## 5. Constraints

Do not implement:

- full memory ingestion pipeline
- semantic recall
- DB-first cold memory

This stage is about boundary correctness, not feature expansion.

## 6. Short Task Prompt

```text
Use specs/19-memory-boundary.md as the primary design source.

Do a narrow pass to align the codebase and docs with the layered memory model:
Audit -> Evidence -> FeedbackPacket -> MemoryCandidate -> ColdMemory

Focus on keeping these concepts distinct.
Do not implement full cold memory or semantic search yet.
```
