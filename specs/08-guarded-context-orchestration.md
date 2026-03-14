# Guarded Context Orchestration - Integrated Design

## 1. Overview

This document consolidates the current architecture discussion into one execution-oriented design.
It is intended as the handoff document for the implementation agent.

It unifies four threads:

1. Spec 05: ECS runtime and event-driven systems
2. Spec 06: evidence-based security and risk gating
3. Spec 07: memory, relationship discovery, and context bridging
4. The current lightweight agent runtime already present in `src/agent/*`

The core change in this design is simple:

- Context injection is no longer treated as a plain side effect.
- Scheduling is no longer treated as an informal next step.
- Cross-session coordination is no longer based on raw prompt/context concatenation.

Instead, these become guarded effects:

```text
proposal -> evidence -> deterministic guard -> guarded commit -> execute
```

This design also formalizes the loop-back point:

```text
the end of one session turn is not "context file written"
it is "a structured feedback packet is now available for the next guarded turn"
```

## 2. Why This Document Exists

The project already has several good upper-layer specs, but they are split by concern:

- Spec 04 explains the Canvas as the visual layer
- Spec 05 explains the ECS runtime as the execution substrate
- Spec 06 explains evidence-based risk and security checkpoints
- Spec 07 explains memory, semantics, and relationship-aware context

What was missing was the integrated path between them:

```text
session runtime
  -> memory extraction
  -> relationship-aware context bridge
  -> security gate
  -> scheduler decision
  -> next turn execution
```

This document fills that gap and should be preferred as the direct implementation handoff for the next phase.

## 3. Current State Snapshot

### 3.1 What already exists

The current repository already contains the seed of the runtime:

- Hook event ingress
  - `src/bin/bridge.rs`
  - `src/hooks/event.rs`
  - `src/hooks/socket.rs`
  - `src/hooks/receiver.rs`
- Lightweight reactive runtime
  - `src/agent/mod.rs`
  - `src/agent/runner.rs`
  - `src/agent/systems/*`
- Existing domain objects
  - `src/session/instance.rs`
  - `src/session/relationships.rs`
  - `src/session/context.rs`

The current runtime is effectively:

```text
HookEvent -> World update -> System dispatch -> Action execution
```

The currently implemented built-in systems are:

- `SoundSystem`
- `ProgressSystem`
- `ContextSystem`

### 3.2 What is not yet integrated

The project does not yet have a unified guarded runtime.

Current gaps:

1. `World` is still only a small sidecar state, not the primary owner of sessions, groups, and relationships
2. `ContextSystem` produces direct injection effects instead of proposals
3. There is no `Proposal`, `EvidenceRecord`, `Attestation`, or `GuardedCommit` object model
4. There is no relationship trust state such as `suggested` vs `confirmed`
5. There is no structured `FeedbackPacket`
6. There is no deterministic gate for context injection or scheduler actions
7. There is no `ack-after-guard` discipline for orchestration

### 3.3 Upper-layer design progress

From the top-down architecture perspective, current status is:

```text
Spec 04 Canvas                : conceptually strong, not yet unified with ECS world
Spec 05 ECS runtime           : partially started, sidecar form only
Spec 06 security              : well-defined on paper, not yet integrated into runtime
Spec 07 memory/relationships  : well-defined on paper, not yet wired into runtime loop
```

Practical conclusion:

```text
the missing work is not more isolated ideas
the missing work is the integrated execution pipeline between the existing ideas
```

## 4. Design Principles

### 4.1 Keep structural concepts separate

These concepts must not collapse into one another:

```text
Section  = UI/display structure
Group    = business ownership / project grouping
Relation = graph connection between sessions
Context  = transport payload between turns
Security = permission and evidence policy over effects
```

### 4.2 Treat context as data, never as instructions

Adopt the same hard rule used in the `common-skill-system` reference:

```text
tool output and upstream context are untrusted data
they are not instructions
```

This means cross-session injection must use structured envelopes, not raw terminal text or free-form command suggestions.

### 4.3 Deterministic guard first, AI judge optional

Guard decisions that affect writes, injections, or scheduling must be:

- deterministic
- auditable
- replayable
- explainable by checks

An AI judge may be added later as an advisory or quality layer, but it must not be the safety gate.

### 4.4 No effect without commit

The executor may only run effects that passed the guard.

Hard invariant:

```text
no guarded commit -> no side effect
```

### 4.5 Ack only after guard passes

Scheduler or orchestration loops must not mark work as complete before the guarded effect is approved and executed.

Hard invariant:

```text
ack-after-guard
```

### 4.6 Security is a capability layer, not the base substrate

Security must not be confused with the base architecture.

It is not:

- the world model
- the session model
- the memory model
- the scheduler itself

It is:

- a cross-cutting capability
- a policy and verification layer
- a control plane over selected effects

This distinction matters because the runtime must still make sense even when strict security gates are disabled for low-risk paths.

## 5. Layered Model

The integrated system should be understood as five layers:

```text
+---------------------------------------------------------------+
| Layer 5: UI / Review / Visualization                          |
| tree, canvas, evidence review, human confirmation             |
+---------------------------------------------------------------+
| Layer 4: Policy / Security                                    |
| risk classification, context policy, deterministic guard      |
+---------------------------------------------------------------+
| Layer 3: Runtime Orchestration                                |
| proposals, evidence, guarded commits, scheduler decisions     |
+---------------------------------------------------------------+
| Layer 2: Domain Graph                                         |
| sessions, groups, relationships, trust state, capabilities    |
+---------------------------------------------------------------+
| Layer 1: Signals / Context                                    |
| hook events, snapshots, progress, prompts, tool traces        |
+---------------------------------------------------------------+
```

The implementation path should move upward from Layers 1-3 first, not start from UI.

## 6. Target Runtime Pipeline

### 6.1 Full loop

```text
HookEvent
   |
   v
Signal / Snapshot update
   |
   v
ProposalSystem
   |
   v
EvidenceSystem
   |
   v
GuardSystem
   |
   +--> BlockedAttestation
   |        |
   |        v
   |   audit / review / retry
   |
   \--> GuardedCommit
            |
            v
       CommitExecutor
            |
            v
      FeedbackPacket
            |
            v
 next-turn injection / scheduler / relation refinement
```

### 6.2 Runtime system responsibilities

```text
1. ProposalSystem
   - reads events and world state
   - proposes possible next effects
   - does not decide permission

2. EvidenceSystem
   - gathers proof and context relevant to the proposal
   - does not decide permission

3. GuardSystem
   - runs deterministic checks
   - emits approve / downgrade / block / needs_human / needs_evidence

4. CommitExecutor
   - executes only approved commits
   - writes output artifacts and feedback packets

5. SchedulerSystem
   - consumes guarded outputs and pending work
   - creates future proposals
```

### 6.3 Execution cadence: event-driven first, tick-driven later if needed

The current runtime should remain event-driven.

Why:

- the user and the LLM primarily interact through sessions
- session state already surfaces through hooks
- hooks already provide a natural runtime unit
- event-driven behavior is easier to reason about and audit in the current phase

Current recommendation:

```text
runtime coordination      -> event-driven
context injection         -> event-driven
scheduler decisions       -> event-driven
security gating           -> event-driven
```

Tick-driven behavior should only be introduced later for clearly scoped needs such as:

- layout animation or convergence
- cooldown expiry and delayed retries
- passive anomaly scans
- lease/timeout expiry
- scheduled wake-ups without a fresh hook event

Until these needs become concrete, the architecture should not be reoriented around ticks.

## 7. Core Domain Model

### 7.1 Session-centric entities

The ECS world should eventually own these concepts directly:

```text
Session
|- Identity
|- RuntimeState
|- GroupRef
|- RelationRefs
|- CapabilityScope
|- ContextPolicy
|- SecurityProfile
|- FeedbackState

Group
|- GroupPolicy
|- GroupMetadata

Relationship
|- RelationshipData
|- RelationTrust
|- RelationMetadata
|- ScheduleConstraint
```

### 7.2 Relationship trust

Relationship state must be explicit.

```rust
pub enum RelationTrust {
    Suggested,
    Confirmed,
    Suppressed,
}
```

Meaning:

- `Suggested`
  - discovered by heuristics or AI
  - may inform UI hints
  - may not authorize cross-session injection
- `Confirmed`
  - accepted by user or deterministic rule
  - may authorize cross-session injection and scheduling
- `Suppressed`
  - known relation, intentionally not used for runtime coordination

Hard rule:

```text
only confirmed relations may authorize cross-session injection
```

### 7.3 Groups

Groups are not just display folders.
They become first-class domain structure.

Groups should be used for:

- default context policy
- default security posture
- default scheduling scope
- default canvas clustering

But group membership must still be distinct from relationship edges.

## 8. Core Runtime Objects

### 8.1 Proposal

Every meaningful effect begins as a proposal.

```rust
pub enum ProposalKind {
    InjectContext,
    ScheduleSession,
    PromoteRelation,
    EmitSecurityAlert,
}

pub struct Proposal {
    pub id: String,
    pub trace_id: String,
    pub session_id: String,
    pub kind: ProposalKind,
    pub trigger: TriggerEvent,
    pub payload: ProposalPayload,
    pub risk: RiskLevel,
}
```

### 8.2 Evidence

Evidence is the auditable support for a proposal.

```rust
pub struct EvidenceRecord {
    pub id: String,
    pub session_id: String,
    pub trace_id: String,
    pub kind: EvidenceKind,
    pub source: EvidenceSource,
    pub captured_at_ms: u64,
    pub data: serde_json::Value,
}
```

Important sources in this repository:

- hook events
- context snapshots
- progress log entries
- confirmed relationship metadata
- group membership
- session capability scope
- prior guard decisions

### 8.2.1 Invocation provenance: evidence must outgrow MCP

The security mechanism in the sibling reference implementation was first expressed through MCP-oriented evidence.
That is useful, but the real principle is broader:

```text
the system should trust records of real invocation, not model self-description
```

This repository must therefore treat invocation provenance as a generic concept, not an MCP-only concept.

Possible invocation channels:

- MCP tool call
- function call / structured tool call
- CLI command invocation
- hook-observed command lifecycle
- git hook event
- terminal-observed fallback signal

### 8.2.2 Invocation record

Add a first-class invocation log object:

```rust
pub enum InvocationChannel {
    McpTool,
    FunctionCall,
    CliCommand,
    HookEvent,
    GitHook,
    PtyObserved,
}

pub struct InvocationRecord {
    pub id: String,
    pub trace_id: String,
    pub session_id: String,
    pub channel: InvocationChannel,
    pub name: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub args_digest: Option<String>,
    pub result_digest: Option<String>,
    pub hook_ref: Option<String>,
    pub evidence_refs: Vec<String>,
    pub verified: bool,
}
```

Purpose:

- prove a call really happened
- correlate proposals with real execution history
- support post-hoc auditing
- support future deterministic checks on command/tool usage

For CLI-driven agents, hook-based capture is especially important:

```text
command was actually invoked
  -> hook captured it
  -> invocation record was stored
  -> evidence can reference that real record
```

This is stronger than relying on a model saying it "used" a command or tool.

### 8.3 Attestation

Guard output must be explainable.

```rust
pub struct GuardCheck {
    pub name: String,
    pub ok: bool,
    pub detail: Option<String>,
}

pub struct Attestation {
    pub ok: bool,
    pub summary: String,
    pub checks: Vec<GuardCheck>,
}
```

### 8.4 Guard decision

```rust
pub enum GuardDecision {
    Approve,
    Downgrade {
        response_level: ResponseLevel,
        scope: InjectionScope,
    },
    NeedsEvidence,
    NeedsHuman,
    Block,
}
```

### 8.5 Guarded commit

```rust
pub struct GuardedCommit {
    pub commit_id: String,
    pub trace_id: String,
    pub session_id: String,
    pub proposal: Proposal,
    pub decision: GuardDecision,
    pub attestation: Attestation,
}
```

This is the only runtime object that may cross into execution.

## 9. Context Injection Model

### 9.1 Context injection is a guarded effect

The current direct path:

```text
UserPromptSubmit -> ContextSystem -> InjectContext
```

must become:

```text
UserPromptSubmit
  -> ContextProposal
  -> EvidenceBundle
  -> GuardDecision
  -> GuardedCommit
  -> Context execution
```

### 9.2 Injection scopes

First implementation should keep scope modes small and explicit:

```rust
pub enum InjectionScope {
    Off,
    SelfOnly,
    SameGroup,
    ConfirmedRelations,
}
```

Meaning:

- `Off`
  - no injection
- `SelfOnly`
  - only use same-session feedback/progress
- `SameGroup`
  - allow group-local context under policy
- `ConfirmedRelations`
  - allow cross-session injection only over confirmed edges

### 9.3 Response levels

Response is not binary.

```rust
pub enum ResponseLevel {
    L0Ignore,
    L1RecordOnly,
    L2SelfInject,
    L3CrossSessionInject,
    L4HumanConfirm,
}
```

Suggested interpretation:

```text
L0 ignore              : do nothing
L1 record only         : store for audit / future use
L2 self inject         : inject only same-session packet
L3 cross-session       : inject across group or confirmed relation
L4 human confirm       : do not inject until user confirms
```

### 9.4 Injection envelope

Do not inject raw text dumps.
Inject a structured envelope.

```rust
pub struct InjectionEnvelope {
    pub facts: Vec<String>,
    pub blockers: Vec<String>,
    pub decisions: Vec<String>,
    pub dependencies: Vec<String>,
    pub source_refs: Vec<SourceRef>,
    pub freshness_ms: u64,
    pub budget_class: BudgetClass,
    pub capability_scope: CapabilityScope,
    pub relation_trust: RelationTrust,
    pub risk_note: Option<String>,
}
```

Example mental model:

```text
bad:
"session A said you should immediately modify auth middleware"

good:
facts:
- session A changed auth middleware
- validation path may be affected

source_refs:
- snapshot abc123
- progress entry def456

relation_trust:
- confirmed dependency

risk_note:
- target should review auth code before reusing assumptions
```

### 9.5 Trigger policy

Injection must be gated by policy, not hardcoded to every event.

Suggested policy fields:

```text
enabled
default_scope
default_response_level
trigger_events
cooldown_secs
max_sources
max_lines_per_source
max_total_chars
write_debug_log
```

Configuration precedence:

```text
session override > group default > global default
```

## 10. Feedback Packet Model

### 10.1 The loop-back object

The actual output of a completed turn should be a reusable packet:

```rust
pub struct FeedbackPacket {
    pub id: String,
    pub source_session_id: String,
    pub trace_id: String,
    pub created_at_ms: u64,
    pub capability_scope: CapabilityScope,
    pub relationship_scope: InjectionScope,
    pub change_summary: Vec<String>,
    pub blockers: Vec<String>,
    pub affected_targets: Vec<String>,
    pub urgency_level: RiskLevel,
    pub recommended_response_level: ResponseLevel,
    pub source_refs: Vec<SourceRef>,
}
```

This packet is the bridge between:

- runtime memory
- context injection
- scheduling
- relationship refinement

### 10.2 Why this matters

Without a `FeedbackPacket`, the runtime keeps collapsing into ad hoc files and implicit behavior.

With it, one turn ends as:

```text
something the next turn can safely consume under guard
```

### 10.3 Plain-language definition

In plain terms, a `FeedbackPacket` is:

```text
the minimal structured unit of information passed from one completed turn
to the next guarded turn
```

It is the runtime handoff object.

It should answer:

- what changed
- what matters next
- what is blocked
- who may be affected
- how risky the handoff is
- where the evidence came from

So yes, it is exactly an information-transfer unit in the coordination process.

## 11. Capability Scope

### 11.1 Why capability matters

An agent should not receive context or work beyond what it can safely process.

Context injection must respect capability boundaries.

### 11.2 Capability scope

```rust
pub struct CapabilityScope {
    pub path_scope: Vec<String>,
    pub tool_scope: Vec<String>,
    pub authority_level: AuthorityLevel,
    pub dependency_depth: u8,
    pub cognitive_budget: u32,
}
```

Interpretation:

- `path_scope`
  - which projects or paths the agent may reason over
- `tool_scope`
  - which tools it may use or which domain it supports
- `authority_level`
  - whether it may only analyze, may edit, may schedule, or requires human gate
- `dependency_depth`
  - how far upstream/downstream it may act
- `cognitive_budget`
  - rough bound on how much cross-context can be injected without destabilizing the turn

Hard rule:

```text
injected context must fit inside the target capability scope
```

## 12. Security Integration

### 12.1 Reference model

Adopt the security kernel pattern proven in the sibling reference repository:

- `../common-skill-system/src/runtime/guard.ts`
- `../common-skill-system/src/contracts/attestation.ts`
- `../common-skill-system/docs/mcp-security.md`

Key ideas to carry over:

1. untrusted output is data, not instructions
2. deterministic guard before effect execution
3. evidence-bound decisions
4. explainable attestation checks
5. ack-after-guard orchestration discipline

### 12.1.1 Execution evidence is protocol-independent

The guard design must not be coupled only to MCP.

The system should work across:

- MCP-based execution
- function-call-based execution
- CLI-driven execution
- hook-only execution

The invariant is not:

```text
every sensitive action must have MCP evidence
```

The invariant is:

```text
every sensitive action must have verifiable execution evidence
```

That evidence may come from:

- structured tool responses
- command hook trails
- invocation records
- append-only call logs

This matters because the future execution agent in this project may increasingly operate through command-line and function-call style invocation rather than classic MCP-only tooling.

### 12.2 What counts as a high-risk effect in Agent Hand

The following should be treated as guarded operations:

- cross-session context injection
- cross-group context injection
- automatic scheduler actions that start or resume a session
- relation promotion from `suggested` to `confirmed`
- security alerts that may interrupt or reroute workflow
- future external writes such as git-integrated or shell-integrated actions

### 12.3 Risk classes

Suggested runtime interpretation:

```text
Low
- self-only context injection
- local audit recording

Medium
- same-group injection
- low-impact scheduling hint

High
- confirmed cross-session injection
- automatic resume/start of another session
- relation promotion affecting scheduling

Critical
- cross-group routing with elevated scope
- external write or destructive future automation
```

### 12.4 Minimum guard checks

The first deterministic guard implementation should check:

1. proposal shape is valid
2. source session exists
3. target session exists
4. policy is enabled for this scope
5. evidence is present
6. evidence freshness is within window
7. relation trust is sufficient for scope
8. target capability scope allows this injection
9. token budget is within threshold
10. cooldown is not violated

Possible results:

```text
approve
downgrade
needs_evidence
needs_human
block
```

## 13. Relationship Discovery vs Relationship Usage

These must be different runtime lanes.

```text
Discovery lane
  - slow
  - heuristic or AI-assisted
  - may produce suggestions

Usage lane
  - fast
  - deterministic
  - may consume only trusted edges
```

Flow:

```text
signals
  -> suggestion
  -> review / confirmation
  -> confirmed relation
  -> runtime injection / scheduler use
```

This prevents runtime from improvising on weak context.

## 14. Canvas, Memory, and Security Alignment

### 14.1 Canvas

Spec 04 remains valid.
Canvas should eventually become a view over the ECS world, not a separate ad hoc model.

Canvas should visualize:

- sessions
- groups
- relation trust
- pending proposals
- blocked or approved guarded commits

The project should also treat canvas and adjacent views as multiple tabs over the same world, not as competing interpretations.

Suggested view tabs:

- relationship view
- scheduling view
- group topology view
- evidence/risk view

These are different cognitive views over the same runtime state.
This is important because users need multiple mental angles to understand concurrent agent work.

### 14.2 Memory system

Spec 07 remains valid.
But the runtime must split it into:

```text
raw context
  -> extracted signals
  -> relationship memory
  -> bridge material
  -> guarded injection
```

The bridge material must be filtered through the guard before it reaches a target session.

### 14.3 Security system

Spec 06 remains valid.
The missing change is to move security from a parallel audit concept into the direct execution path.

In other words:

```text
security is not just observing context injection
security is deciding whether context injection may happen
```

## 15. Phased Implementation Plan

### Phase A: Guarded self-context foundation

Goal:

- keep behavior narrow
- prove the proposal/evidence/guard/commit path
- avoid cross-session complexity initially

Deliverables:

1. add global context bridge policy config
2. add `Proposal`, `EvidenceRecord`, `Attestation`, `GuardedCommit`
3. add `InvocationRecord` and append-only call log foundation
4. convert `ContextSystem` into `ContextProposalSystem`
5. implement `EvidenceSystem` for self-only context
6. implement deterministic `GuardSystem`
7. executor runs only guarded self-injection commits
8. write audit artifacts for decisions

### Phase B: Group-aware context

Deliverables:

1. carry `group_path` into runtime world
2. add group-level context defaults
3. support `SameGroup` injection scope
4. add downgrade behavior for budget overflow

### Phase C: Relation-trust integration

Deliverables:

1. add `RelationTrust`
2. allow only `ConfirmedRelations` for cross-session injection
3. record why a suggestion was blocked
4. generate `FeedbackPacket`

### Phase D: Scheduler integration

Deliverables:

1. add `ScheduleSession` proposals
2. apply same guard kernel to scheduling
3. enforce `ack-after-guard`
4. store pending retry and needs-human states

### Phase E: UI and Canvas integration

Deliverables:

1. evidence/guard summary in UI
2. human review path for `NeedsHuman`
3. canvas overlays for relation trust and blocked commits

## 16. Storage Model

### 16.1 Storage direction

The system needs to persist:

- relationship data
- hook records
- invocation records
- evidence records
- guarded commits
- feedback packets
- selected memory artifacts

The recommended near-term path is:

```text
serialized ECS-adjacent state + append-only logs first
database later if query pressure justifies it
```

This is likely sufficient for the next phase because:

- entity count is still small
- auditability is more important than query sophistication
- append-only logs align well with evidence and replay
- serialized world snapshots are fast to implement

### 16.2 Suggested storage split

```text
1. Append-only logs
   - hook events
   - invocation records
   - evidence records
   - guarded commits

2. Snapshot state
   - sessions
   - groups
   - relationships
   - relation trust
   - scheduler state

3. Memory artifacts
   - context snapshots
   - semantic signals
   - bridges
   - feedback packets
```

### 16.3 Why not start with a database

A database may still be useful later, especially for:

- cross-session search
- audit dashboards
- analytics
- long-lived relationship mining

But it should not block the next phase.

First priority is to stabilize the data model and event flow.

### 16.4 Serialization target

The storage layer should be designed so that the world can be:

- serialized quickly
- reloaded deterministically
- replayed from logs
- migrated later into a database if necessary

## 17. Handoff Guidance For The Implementation Agent

### 17.1 Start here

Primary files to evolve first:

- `src/agent/mod.rs`
- `src/agent/runner.rs`
- `src/agent/systems/context.rs`
- `src/config.rs`
- `src/session/context.rs`
- `src/session/relationships.rs`

### 17.2 Do not do these first

Avoid starting with:

- full AI relation discovery
- canvas rendering changes
- human review UI
- full world migration of every existing app field

These are downstream of the core runtime path.

### 17.3 First proof target

The first success criterion should be:

```text
on a prompt event, the runtime builds:
proposal -> evidence -> guard -> guarded commit

and only then produces self-only context injection
```

If that path is not stable, nothing broader should be added yet.

## 18. Open Questions

These questions are still intentionally open and should be resolved during implementation or before Phase C:

1. Should `FeedbackPacket` be persisted as part of memory storage, evidence storage, or a dedicated packet store?
2. Should group policy inherit automatically into child sessions on creation, or resolve dynamically at runtime?
3. What is the exact freshness window for cross-session evidence?
4. Should prompt collection and feedback packet generation share the same retention and privacy policy?
5. Should `session security level` from Spec 06 be merged with `ContextPolicy`, or remain parallel?
6. Should relation confirmation live in the TUI first, or be derived from deterministic rules for parent-child and explicit dependency edges?
7. For non-Claude tools, how much trustworthy structured evidence can be extracted from hooks alone?
8. Which invocation channels are strong enough to set `verified=true` on `InvocationRecord`?
9. At what scale does the serialized ECS + append-only log approach need to transition to a database-backed query layer?

## 19. Final Architecture Statement

The integrated architecture can be summarized in one sentence:

```text
Agent Hand should evolve from "reactive session manager with some side effects"
into "a guarded multi-session runtime where context, scheduling, and coordination
flow through proposals, evidence, and audited commits"
```

If implementation follows this document, the next execution agent should have enough context to code without reconstructing the design discussion from scratch.
