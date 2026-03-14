# Memory Boundary - From Audit to Cold Memory

## 1. Purpose

This document defines the memory boundary of the system.

It answers a critical architectural question:

```text
Which runtime artifacts are transient,
which are candidate memory,
and which become long-lived memory?
```

This boundary is necessary because the system now contains several memory-like objects:

- audit logs
- evidence records
- feedback packets
- memory candidates
- future long-term memory store

Without a boundary, these concepts will blur and the system will become hard to reason about.

## 2. One-Sentence Definition

```text
Memory Boundary is the layered distinction between trace artifacts, coordination artifacts,
candidate memory artifacts, and accepted long-lived memory.
```

## 3. Why This Matters

At this point in the architecture, multiple objects can look similar:

- they all persist data
- they all summarize something
- they all may be searched or reused later

But they have different jobs.

If they are collapsed together, the system will suffer from:

- semantic confusion
- duplicate storage
- poor query design
- unclear promotion rules
- unstable Hot Brain outputs

## 4. The Layers

The memory boundary should be understood as five layers.

ASCII:

```text
+------------------------------------------------------+
| Layer 5: Cold Memory                                 |
| accepted, durable, reusable knowledge                |
+------------------------------------------------------+
| Layer 4: Memory Candidates                           |
| not yet accepted, bounded suggestions for promotion  |
+------------------------------------------------------+
| Layer 3: Feedback Packets                            |
| coordination-ready turn outcomes                     |
+------------------------------------------------------+
| Layer 2: Evidence & Guard Artifacts                  |
| why something was approved/blocked                   |
+------------------------------------------------------+
| Layer 1: Audit / Raw Trace                           |
| append-only logs of what happened                    |
+------------------------------------------------------+
```

## 5. Layer 1 - Audit / Raw Trace

### 5.1 What belongs here

Examples:

- hook event logs
- invocation logs
- proposal logs
- evidence logs
- guarded commit logs
- packet logs

### 5.2 Purpose

Audit exists to answer:

```text
What happened?
When did it happen?
How can we reconstruct it?
```

### 5.3 Properties

- append-only
- high fidelity
- replay-friendly
- not optimized for semantic reuse

### 5.4 What it is not

Audit is not:

- compact memory
- scheduler input by itself
- curated project knowledge

## 6. Layer 2 - Evidence & Guard Artifacts

### 6.1 What belongs here

Examples:

- `EvidenceRecord`
- `GuardCheck`
- `Attestation`
- `GuardedCommit`

### 6.2 Purpose

This layer answers:

```text
Why was a proposal accepted or blocked?
What evidence supported that decision?
```

### 6.3 Properties

- structured
- traceable
- policy-facing
- tightly tied to guarded runtime semantics

### 6.4 What it is not

This layer is not:

- user-facing handoff by default
- long-lived memory by default
- semantic understanding by itself

## 7. Layer 3 - Feedback Packets

### 7.1 What belongs here

Examples:

- `FeedbackPacket V1`

### 7.2 Purpose

This layer answers:

```text
What is the smallest structured outcome of the turn
that later coordination systems should care about?
```

### 7.3 Properties

- compact
- coordination-facing
- transport-neutral
- derived
- traceable to Layer 2 and Layer 1

### 7.4 What it is not

FeedbackPacket is not:

- long-term accepted memory
- raw evidence
- raw transcript

## 8. Layer 4 - Memory Candidates

### 8.1 What belongs here

Examples:

- `MemoryCandidate`

### 8.2 Purpose

This layer answers:

```text
What from the recent coordination/runtime activity may deserve durable storage?
```

### 8.3 Properties

- bounded
- suggested, not accepted
- downstream of Hot Brain or other analyzers
- still subject to deterministic consumers

### 8.4 What it is not

MemoryCandidate is not:

- durable memory
- search index row
- guaranteed truth

## 9. Layer 5 - Cold Memory

### 9.1 What belongs here

Examples:

- accepted decisions
- reusable findings
- repeated blocker patterns
- stable relation-related knowledge
- archived summaries worth future recall

### 9.2 Purpose

This layer answers:

```text
What knowledge should survive well beyond the current turn or session?
```

### 9.3 Properties

- durable
- curated
- queryable
- reusable across sessions
- no longer tied to one transport or one immediate runtime event

### 9.4 What it is not

Cold Memory is not:

- raw event storage
- replay log
- arbitrary transcript archive

## 10. Promotion Flow

Memory should not be written directly from lower layers.

ASCII:

```text
Audit
  -> Evidence / Commit
  -> FeedbackPacket
  -> MemoryCandidate
  -> MemoryConsumer
  -> ColdMemory
```

This gives a clean promotion ladder.

### 10.1 Hard rule

```text
Only accepted memory consumers may promote data into Cold Memory.
```

Neither:

- audit logs
- feedback packets
- Hot Brain

may directly define cold memory truth.

## 11. Comparison Table

| Layer | Main Object | Main Question | Lifespan | Main Consumer |
|------|-------------|---------------|----------|---------------|
| Audit | raw log entries | what happened? | long | audit/replay |
| Evidence | evidence/attestation/commit | why was it allowed? | medium-long | guard/debug |
| Packet | feedback packet | what should coordination know? | medium | scheduler/hot brain |
| Candidate | memory candidate | what may be worth keeping? | short-medium | memory consumer |
| Cold Memory | accepted memory record | what should survive? | long | future recall/search |

## 12. What Should Be Searchable

Not every layer needs the same query semantics.

### 12.1 Audit search

Good for:

- exact trace lookup
- replay
- compliance

Bad for:

- semantic recall
- user-facing memory summaries

### 12.2 Packet search

Good for:

- recent coordination review
- short-range runtime debugging
- scheduler inspection

### 12.3 Cold memory search

Good for:

- semantic recall
- cross-session reasoning
- long-term project understanding

## 13. Storage Implications

This boundary strongly suggests a hybrid model.

ASCII:

```text
Audit / Evidence / Packets
  -> append-only logs first

Memory Candidates
  -> bounded intermediate records

Cold Memory
  -> curated store / later query layer
```

This means:

- not everything needs to go into a database immediately
- not everything should be indexed semantically
- lower layers can remain cheap and append-only

## 14. Hot Brain and Memory Boundary

Hot Brain should primarily operate across Layers 3 and 4.

That means:

- it consumes `FeedbackPacket`
- it emits `MemoryCandidate`

It should not directly operate as a cold-memory writer.

ASCII:

```text
FeedbackPacket
   |
   v
Hot Brain
   |
   v
MemoryCandidate
   |
   v
MemoryConsumer
   |
   v
ColdMemory
```

## 15. Human Handoff vs Cold Memory

These are related but not identical.

### Human handoff

- optimized for human comprehension
- may be verbose
- may include tactical next steps

### Cold memory

- optimized for reusable knowledge
- should be more stable and less conversational
- should avoid redundant tactical clutter

Hard rule:

```text
Do not assume every handoff line belongs in cold memory.
```

## 16. Acceptance Rules for Cold Memory

Before a memory item becomes cold memory, it should satisfy at least:

1. traceable source
2. bounded summary
3. stable enough for reuse
4. not merely accidental noise from one turn

Repeated blocker detection is a good example:

```text
one blocker once
  -> maybe just packet detail

same blocker across multiple packets
  -> good memory candidate
```

## 17. Execution Plan

### Phase M0 - Freeze memory layers

Done when:

- this document is accepted as the memory boundary

### Phase M1 - Keep current lower layers clean

Implementation target:

- keep audit, evidence, packet, candidate, and cold memory concepts separate in code and docs

### Phase M2 - Define cold memory record shape

Implementation target:

- add conceptual shape for accepted durable memory entry

### Phase M3 - Add deterministic MemoryConsumer outputs

Implementation target:

- normalized ingest entry
- bounded promotion path

## 18. Recommended Next Document

After this, the next document should be:

```text
Scheduler Normalized Outputs
```

Because once memory is bounded, the next remaining gap is:

```text
How scheduler-side accepted outputs are represented before they affect runtime behavior.
```

## 19. Final Statement

The memory system is not one thing.

It is a ladder:

```text
trace
 -> evidence
 -> packet
 -> candidate
 -> cold memory
```

That ladder is the boundary that keeps the architecture coherent.
