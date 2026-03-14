# Canvas / Workflow Views - Multi-View Projection Design

## 1. Purpose

This document defines how the system should expose multiple user-facing views over the same underlying runtime world.

It exists because the project should not treat Canvas as:

- only a relationship graph
- only a visual toy
- a separate data model

Instead, Canvas and adjacent views should be treated as:

```text
multiple projections over one shared runtime world
```

## 2. Core Idea

Users need different cognitive views for the same ongoing work.

Examples:

- "Who is related to whom?"
- "What should happen next?"
- "Where are blockers?"
- "What evidence supports this?"
- "What is the workflow state across sessions?"

These are not different systems.
They are different projections.

## 3. One-Sentence Definition

```text
Canvas / Workflow Views are bounded visual projections over the same world state,
optimized for different human reasoning tasks.
```

## 4. Shared World, Multiple Views

ASCII:

```text
                +--------------------------------+
                |            ECS World           |
                |--------------------------------|
                | sessions / groups / relations  |
                | packets / hints / decisions    |
                | evidence / memory / status     |
                +---------------+----------------+
                                |
        +-----------------------+-----------------------+
        |                       |                       |
        v                       v                       v
 [Relationship View]    [Scheduler View]       [Evidence View]
        |                       |                       |
        +-----------------------+-----------------------+
                                |
                                v
                      [Workflow / Task View]
```

## 5. Why This Matters

Without this model, the product tends to drift into:

- one graph with too much overloaded meaning
- separate UI views with separate hidden logic
- repeated business rules in rendering code

The correct model is:

```text
one world
many projections
```

## 6. Projection Layers

There are three layers involved.

### 6.1 World layer

Source of truth:

- sessions
- groups
- relations
- packets
- hints
- evidence
- normalized outputs

### 6.2 View model layer

Projection-specific shaping:

- filtering
- grouping
- coloring / status categorization
- edge selection
- prioritization

### 6.3 Rendering layer

Actual TUI / canvas rendering:

- node layout
- edge routing
- badges
- overlays
- panels

Hard rule:

```text
rendering must not define business semantics
```

## 7. Recommended Views

### 7.1 Relationship View

Question:

```text
How are sessions and groups connected?
```

Primary objects:

- sessions
- groups
- relation edges
- relation trust state

Main use:

- establish and inspect explicit coordination links

ASCII:

```text
 [A] ---peer--- [B]
  |              |
 dep           collab
  |              |
 [C] ----------- [D]
```

### 7.2 Scheduler View

Question:

```text
Which sessions are blocked, urgent, pending, or likely to be scheduled next?
```

Primary objects:

- recent packets
- scheduler hints
- normalized scheduler outputs
- blocked states

Main use:

- coordination priority
- next-step reasoning

ASCII:

```text
 [A] blocker: auth token
 [B] pending coordination
 [C] urgent follow-up
 [D] idle / no action
```

### 7.3 Evidence View

Question:

```text
Why did the system block, approve, or suggest something?
```

Primary objects:

- evidence records
- guard checks
- guarded commits
- source refs

Main use:

- human trust
- auditability
- debugging system decisions

ASCII:

```text
 [Proposal]
    |
    v
 [Evidence]
    |
    v
 [GuardChecks]
    |
    v
 [Commit: Approve / Block]
```

### 7.4 Workflow View

Question:

```text
What is the current end-to-end task state across sessions?
```

Primary objects:

- packets
- next steps
- blockers
- created follow-up items
- human handoff projections

Main use:

- understand whole-task progression
- see how one session leads to another

ASCII:

```text
 [A: gather info]
         |
         v
 [B: implement]
         |
         v
 [C: verify]
         |
         v
 [done / blocked / review]
```

## 8. Why Canvas Is Not Just Relationship View

Canvas is best understood as:

```text
a spatial rendering surface
```

not as:

```text
the relationship graph itself
```

That means:

- Relationship View can use canvas
- Scheduler View can use canvas
- Evidence overlays can use canvas
- Workflow View can use canvas

The canvas is a rendering medium, not a single semantic mode.

## 9. Projection Inputs

Different views consume different upper-layer artifacts.

### Relationship View inputs

- session/group graph
- relation trust
- explicit user-created links

### Scheduler View inputs

- `SchedulerHint`
- `SchedulerNormalizedOutput`
- packet urgency / blockers

### Evidence View inputs

- `EvidenceRecord`
- `GuardedCommit`
- `Attestation`

### Workflow View inputs

- `FeedbackPacket`
- `next_steps`
- blocked / pending normalized outputs

## 10. View-Specific Semantics

Each view should define a small semantic vocabulary.

### Relationship View

- edge type
- trust state
- group membership

### Scheduler View

- pending
- ignored
- review
- propose-followup

### Evidence View

- approved
- blocked
- weak evidence
- missing evidence

### Workflow View

- active
- blocked
- handoff-ready
- done

## 11. UI Tabs

Recommended top-level tabs:

```text
Tree
Canvas: Relationships
Canvas: Scheduler
Canvas: Evidence
Canvas: Workflow
```

This preserves one stable navigation model while exposing multiple views.

## 12. Projection Pipeline

ASCII:

```text
ECS World
   |
   v
Projection Builder
   |
   +--> RelationshipViewModel
   +--> SchedulerViewModel
   +--> EvidenceViewModel
   \--> WorkflowViewModel
            |
            v
        Canvas/TUI Renderer
```

This means view logic should preferably live in:

- projection builders

and not directly inside:

- raw renderer code

## 13. What Must Stay Out of Rendering

The renderer should not be the place where we decide:

- relation trust rules
- scheduling semantics
- memory promotion logic
- guard meaning

Those decisions belong to runtime and projection layers.

## 14. MVP / Near-Term Strategy

Near-term recommendation:

### Phase V0

Keep the existing tree view as the primary stable view.

### Phase V1

Add basic projection-ready structures for:

- relation state
- scheduler state
- evidence state

### Phase V2

Canvas first becomes:

```text
Relationship View
```

### Phase V3

Add:

- Scheduler View
- Evidence overlays

### Phase V4

Add:

- Workflow View

## 15. Why This Supports Your Original Goal

Your earlier scenario:

```text
A forks/creates B
C has useful context/artifact
User links A and C
B should continue with useful context from A and C
```

maps across the views like this:

### Relationship View

Shows:

- A linked to C
- B descended from A

### Scheduler View

Shows:

- B has pending coordination need
- C may be source-relevant

### Evidence View

Shows:

- why B was selected as a follow-up target
- why C was considered relevant

### Workflow View

Shows:

- the larger task chain:
  - A produced
  - C contributed
  - B continues

## 16. Execution Plan

### Phase VW0 - Freeze conceptual model

Done when:

- this document is accepted

### Phase VW1 - Add projection-oriented view models

Implementation target:

- define minimal projection structs for the four view types

### Phase VW2 - Relationship-first canvas

Implementation target:

- implement relationship projection into canvas

### Phase VW3 - Scheduler/evidence overlays

Implementation target:

- add scheduling and evidence projection fields into rendering

### Phase VW4 - Workflow projection

Implementation target:

- map packet chain / follow-up flow into workflow view

## 17. Final Statement

Canvas is not one view.

It is a rendering surface for several views:

```text
relationship
scheduler
evidence
workflow
```

All of them must project from the same shared world.
