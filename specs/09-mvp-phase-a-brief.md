# MVP Phase A Brief - Guarded Self-Context Foundation

## 1. Purpose

This document is the short execution brief for the first MVP.

Use this document when implementing the first vertical slice.
For broader architecture context, refer to:

- `specs/08-guarded-context-orchestration.md`
- `specs/05-ecs-runtime-event-system.md`
- `specs/06-security-mechanism-evidence-confirmation.md`
- `specs/07-memory-relationship-system.md`

This MVP intentionally does **not** attempt to deliver the full ECS migration.
It only proves the first guarded runtime path.

## 2. MVP Goal

Build the smallest complete guarded pipeline for self-context injection:

```text
HookEvent(UserPromptSubmit)
  -> World update
  -> ContextProposal
  -> Self-only Evidence
  -> Deterministic Guard
  -> GuardedCommit
  -> Execute self-context write
  -> Persist FeedbackPacket V1 + audit log
```

The goal is to prove:

1. direct context injection can be replaced by guarded execution
2. the runtime can explain why an injection happened or was blocked
3. the path is stable enough to become the foundation for later group/relationship/scheduler work

## 3. In Scope

### 3.1 Runtime objects

Add the minimum object model required for guarded execution:

```rust
Proposal
EvidenceRecord
GuardCheck
Attestation
GuardDecision
GuardedCommit
FeedbackPacket
```

### 3.2 Runtime flow

Implement this exact behavior:

1. `HookEventKind::UserPromptSubmit` triggers context proposal generation
2. proposal is self-only
3. evidence is gathered from same-session state only
4. deterministic guard runs
5. only approved commits may write `.agent-hand-context.md`
6. all decisions are recorded to append-only logs

### 3.3 Config

Add minimal global config for guarded context behavior.

Suggested fields:

```text
context_bridge.enabled
context_bridge.scope                # default: self_only
context_bridge.trigger_events       # default: [user_prompt_submit]
context_bridge.cooldown_secs
context_bridge.max_lines
context_bridge.max_total_chars
context_bridge.write_debug_log
```

### 3.4 Persistence

For MVP, persistence should be simple:

- append-only JSONL logs for runtime decisions
- existing `.agent-hand-context.md` artifact remains the output target
- no database required

## 4. Out of Scope

Do **not** implement these in MVP:

- cross-session injection
- group-aware injection
- relationship-aware injection
- relation trust runtime flow
- scheduler automation
- CLI resume / restart improvements
- PTY prompt injection
- hook stdout `additionalContext` injection
- Canvas or UI changes
- semantic relationship discovery
- DB-backed ECS storage
- AI judge in guard path

## 5. Required File Targets

Primary implementation files:

- `src/agent/mod.rs`
- `src/agent/runner.rs`
- `src/agent/systems/context.rs`
- `src/config.rs`
- `src/session/context.rs`

Optional support files if needed:

- `src/agent/systems/mod.rs`
- `src/session/mod.rs`

## 6. Runtime Design For MVP

### 6.1 Proposal-only context system

Current behavior:

```text
UserPromptSubmit -> ContextSystem -> Action::InjectContext
```

MVP behavior:

```text
UserPromptSubmit -> ContextProposalSystem -> Proposal::InjectContext(self_only)
```

`ContextSystem` must stop emitting direct injection actions.

### 6.2 Evidence system

MVP evidence is self-only and deterministic.

Allowed sources:

- current session state from `agent::World`
- session project path
- recent progress log entries for this session
- current event metadata

Disallowed sources for MVP:

- other sessions
- groups
- relationships
- AI-generated bridge material

### 6.3 Guard system

MVP guard should check only:

1. context bridge is enabled
2. proposal target exists
3. project path exists
4. evidence exists
5. scope is `self_only`
6. cooldown is satisfied
7. max line / char budget is satisfied
8. duplicate injection for same turn has not already happened

Outputs:

```text
Approve
NeedsEvidence
Block
```

For MVP, `Downgrade` and `NeedsHuman` may exist in types but do not need full behavior yet.

### 6.4 Commit executor

Executor behavior:

- approved commit:
  - materialize context content
  - write `.agent-hand-context.md`
  - write `FeedbackPacket V1`
  - append audit entries
- blocked commit:
  - do not write context artifact
  - append blocked attestation to audit

Hard rule:

```text
no guarded commit -> no context write
```

## 7. FeedbackPacket V1

### 7.1 Definition

`FeedbackPacket` is the smallest structured handoff unit from one completed turn to the next runtime decision.

For MVP it is still self-focused, but it must already have the shape needed for future scheduling and cross-session flow.

### 7.2 Required fields

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

### 7.3 Field meaning

- `packet_id`
  - unique identifier for this packet
- `trace_id`
  - links packet to proposal / commit / audit
- `source_session_id`
  - session that produced the packet
- `created_at_ms`
  - ordering / freshness
- `goal`
  - optional concise statement of what the turn was trying to accomplish
- `now`
  - optional statement of what should happen next
- `done_this_turn`
  - short completed items
- `blockers`
  - unresolved obstacles
- `decisions`
  - important choices taken this turn
- `findings`
  - things learned that matter later
- `next_steps`
  - recommended follow-up actions
- `affected_targets`
  - currently self-only, but future-compatible with cross-session routing
- `source_refs`
  - references to evidence / progress / snapshots
- `urgency_level`
  - rough risk or coordination urgency
- `recommended_response_level`
  - future-compatible routing hint

### 7.4 What not to put in the packet

Do **not** put these into `FeedbackPacket V1`:

- raw terminal dumps
- full prompt text
- full tool response blobs
- full diffs
- large context strings duplicated from artifacts

Those should live in logs or artifacts and be referenced via `source_refs`.

## 8. Append-only Audit

For MVP, add append-only JSONL logs for:

- proposals
- evidence records
- guarded commits
- feedback packets

Suggested location:

```text
~/.agent-hand/profiles/{profile}/agent-runtime/
  proposals.jsonl
  evidence.jsonl
  commits.jsonl
  feedback_packets.jsonl
```

Exact path may be adjusted if there is already a more suitable profile-scoped runtime directory.

## 9. Acceptance Criteria

The MVP is complete when all of the following are true:

1. `UserPromptSubmit` no longer directly injects context
2. `UserPromptSubmit` creates a proposal for self-context injection
3. proposal produces evidence
4. deterministic guard decides approve or block
5. approved commit writes `.agent-hand-context.md`
6. blocked commit writes no context artifact
7. `FeedbackPacket V1` is persisted for approved path
8. all four JSONL audit streams are written
9. duplicate injection for the same turn is prevented
10. existing sound/progress/status behavior still works

## 10. Non-Goals For This Implementation

Even if the architecture suggests them, the implementation agent should avoid adding:

- speculative abstractions for future phases beyond what MVP needs
- extra UI
- semantic search
- DB schema
- cross-session context merge logic
- relation or group policy behavior

This MVP succeeds by being narrow and stable.

## 11. Short Execution Prompt

Use the following instruction for the implementation agent:

```text
Implement the Phase A MVP from specs/09-mvp-phase-a-brief.md.

Build only the guarded self-context foundation:
HookEvent(UserPromptSubmit)
-> ContextProposal
-> Self-only Evidence
-> Deterministic Guard
-> GuardedCommit
-> .agent-hand-context.md write
-> FeedbackPacket V1 + JSONL audit

Constraints:
- No cross-session injection
- No scheduler
- No relation trust behavior
- No PTY injection
- No hook additionalContext injection
- No database layer
- No UI work

Preserve existing sound/progress/status behavior.
Keep the implementation narrow and deterministic.
```

## 12. What To Discuss Next

After MVP implementation starts, the next design discussion should focus on one of:

1. `FeedbackPacket V1` refinement after seeing real runtime output
2. the boundary between filesystem injection, hook injection, and PTY injection
3. the future `runtime_session_id / cli_conversation_id / resume_id` model
4. when to add SQLite projection over append-only logs

Recommended next discussion:

```text
filesystem injection vs hook additionalContext vs PTY send_keys
```

That is the next highest-impact boundary after the MVP path exists.
