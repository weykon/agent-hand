# Boundary and E2E Plan - Live REPL Control vs Guarded Context

## 1. Purpose

This document defines the boundary between:

- live REPL/session control
- guarded context runtime

It also defines the next execution plan after the Phase A MVP implementation:

1. lock the boundary
2. add end-to-end coverage
3. avoid architectural pollution between transport/control and policy/coordination

This document is intentionally narrow and execution-focused.

## 2. Current Implementation Status

As of the current branch, the following is now true:

### 2.1 Guarded context path exists

```text
HookEvent(UserPromptSubmit)
  -> ContextGuardSystem
  -> Proposal
  -> Evidence
  -> run_guard()
  -> GuardedCommit
  -> ActionExecutor
  -> .agent-hand-context.md
  -> JSONL audit streams
```

This is the first correct vertical slice for the MVP.

### 2.2 Live REPL control path also exists

The repository now also contains:

- `interrupt`
- `resume`
- `send prompt`

through tmux/control/bridge session commands.

This is useful, but it belongs to a different architectural layer.

## 3. The Core Boundary

The system must keep these two concerns separate.

### 3.1 Live REPL Control

This layer answers:

```text
Can we talk to the running agent CLI right now?
How do we interrupt it?
How do we resume it?
How do we send input into it?
```

ASCII:

```text
User / automation
      |
      v
[session control]
  - interrupt
  - resume
  - send prompt
      |
      v
[tmux pane / live REPL]
```

This is a transport/control-plane problem.

### 3.2 Guarded Context Runtime

This layer answers:

```text
Should any context be injected?
What context is allowed?
Why was it approved or blocked?
What evidence supports that decision?
What packet should exist after the turn?
```

ASCII:

```text
HookEvent
   |
   v
[proposal]
   |
   v
[evidence]
   |
   v
[guard]
   |
   +--> block
   |
   \--> commit
         |
         v
  context artifact / feedback packet / audit
```

This is a policy/coordination-plane problem.

## 4. Why Pollution Happens

These layers are easy to mix because both affect what the next agent turn sees.

Typical pollution patterns:

### 4.1 Transport controls start deciding semantics

Bad pattern:

```text
send_keys() directly decides what "context" means
```

Result:

- context becomes prompt-shaped
- future hook/file/provider injection becomes harder
- audit loses semantic meaning

### 4.2 Guarded context starts depending on a single delivery channel

Bad pattern:

```text
proposal/evidence/guard assumes PTY delivery
```

Result:

- cannot swap to hook stdout or file artifact cleanly
- packet shape becomes tied to one provider behavior

### 4.3 Resume is mistaken for system recovery

Bad pattern:

```text
provider conversation resume == full system resume
```

Result:

- conversation state and orchestration state collapse into one concept
- feedback packet / audit / relation state become secondary or missing

## 5. Correct Layering

The correct architecture is:

```text
                  +-----------------------------+
                  | Guarded Context Runtime     |
                  |-----------------------------|
HookEvent ------> | proposal / evidence / guard |
                  | feedback packet / audit     |
                  +-------------+---------------+
                                |
                                | approved payload
                                v
                  +-----------------------------+
                  | Delivery Layer              |
                  |-----------------------------|
                  | filesystem artifact         |
                  | hook additionalContext      |
                  | PTY send_keys               |
                  +-------------+---------------+
                                |
                                v
                  +-----------------------------+
                  | Live REPL Control           |
                  |-----------------------------|
                  | interrupt / resume / send   |
                  | session-state checks        |
                  +-------------+---------------+
                                |
                                v
                         external agent CLI
```

Short version:

```text
Guarded Context Runtime decides "what may be said"
Delivery Layer decides "through which channel it is delivered"
Live REPL Control decides "whether we can interact with the live session now"
```

## 6. Architectural Rule

Use this rule going forward:

```text
Live REPL control must never define context semantics.
Guarded context must never depend on one specific delivery channel.
```

## 7. Evaluation of Current Implementation

### 7.1 What is good

The current Phase A implementation proves the right vertical slice:

- proposal/evidence/guard/commit exists
- context writes are now gated
- audit streams exist
- duplicate injection prevention exists

This is enough to justify moving to integration testing.

### 7.2 What is still unresolved

The current branch also introduces a second line of work:

- tmux session interrupt/resume/send
- hook stdout `additionalContext`

These are valuable, but they are not yet clearly integrated into the layered model above.

Unresolved design questions:

1. Which delivery channel is primary for MVP?
2. Is `.agent-hand-context.md` still the source of truth, or just one projection?
3. Is hook stdout context enabled simultaneously with CLAUDE.md-based discovery?
4. Does `resume` mean:
   - provider conversation resume
   - session control resume
   - orchestration/state resume
   ?

These must be answered before expanding beyond the MVP.

## 8. Why End-to-End Tests Are Now Needed

The guard unit tests prove only the pure policy function.
They do **not** prove the runtime path.

What remains unproven without E2E:

- system registration in `ui/app/mod.rs`
- action emission from `ContextGuardSystem`
- audit file creation
- actual `.agent-hand-context.md` writes
- cooldown + dedup in live dispatch
- interaction between hook event ingress and runtime execution

ASCII:

```text
Unit tests prove:
  [guard()] in isolation

E2E tests must prove:
  HookEvent -> SystemRunner -> ActionExecutor -> Files/Audit
```

## 9. Recommended E2E Scope

Do not jump to full tmux/provider E2E yet.
Start with process-local integration E2E for the guarded path.

### 9.1 E2E Level 1 - Runtime path

Goal:

```text
prove the MVP guarded path works end-to-end inside the Rust runtime
```

Scenarios:

1. `UserPromptSubmit` with valid project path and progress file
   - expect approved commit
   - expect context file written
   - expect proposal/evidence/commit/feedback JSONL written

2. `UserPromptSubmit` when bridge disabled
   - expect blocked commit
   - expect no context artifact
   - expect proposal/evidence/commit logs still written

3. duplicate prompt in same cooldown window
   - first event approved
   - second event blocked
   - audit shows reason

4. event not in trigger list
   - no proposal
   - no context artifact

### 9.2 E2E Level 2 - Hook ingress to runtime

Goal:

```text
prove normalized HookEvent can actually flow into the runtime path
```

Scenarios:

1. emit `HookEventKind::UserPromptSubmit` through broadcast channel
2. let `SystemRunner` and `ActionExecutor` run
3. assert runtime side effects

This should still avoid real tmux and real Claude CLI.

### 9.3 E2E Level 3 - Delivery boundary

Only after the above is stable:

- filesystem projection test
- hook stdout projection test
- PTY delivery test

But these should be tested as delivery adapters, not merged into one giant scenario.

## 10. What Not To Do Yet

Do not write one huge E2E test that tries to prove:

- tmux
- provider resume
- hook stdout
- guarded context
- session control
- full UI

all at once.

That would blur the very boundary this document is trying to preserve.

## 11. Execution Plan

### Step 1. Freeze MVP semantics

Do this first:

- treat guarded self-context as the current primary runtime path
- do not add more semantics to `resume`, `interrupt`, or `send`
- do not expand to cross-session injection yet

### Step 2. Add runtime E2E coverage

Implementation target:

- add integration tests for:
  - approve
  - block
  - cooldown block
  - non-trigger event

Suggested focus areas:

- `src/agent/runner.rs`
- `src/agent/systems/context.rs`
- runtime/audit side effects

### Step 3. Add delivery-channel boundary note

Before more development:

- document that `.agent-hand-context.md`, hook stdout, and PTY send are separate delivery mechanisms
- pick one primary channel for MVP validation

### Step 4. Revisit session control as a separate plan

After MVP E2E is stable:

- evaluate `resume`
- evaluate `interrupt`
- evaluate `send prompt`

as a separate "agent session interaction" track

not as part of the guarded-context semantic core

## 12. Suggested Task For The Development Agent

Use this task prompt:

```text
Add end-to-end coverage for the guarded self-context MVP from specs/09-mvp-phase-a-brief.md and specs/10-boundary-and-e2e-plan.md.

Focus only on the guarded runtime path:
HookEvent -> ContextGuardSystem -> Proposal/Evidence/Guard -> GuardedCommit -> ActionExecutor -> context artifact + audit logs

Add tests for:
1. approved user_prompt_submit path
2. blocked path when context bridge disabled
3. cooldown/dedup block path
4. ignored non-trigger event

Do not expand session control semantics.
Do not add cross-session logic.
Do not add UI tests.
Do not merge PTY/hook/file delivery concerns into one test.
```

## 13. What To Discuss Next

After the E2E path is added, the next design discussion should be:

```text
What is the primary delivery channel for guarded context in Phase B?
```

That discussion must explicitly compare:

- filesystem artifact
- hook `additionalContext`
- PTY send

as delivery mechanisms only, not as semantic/context-definition layers.
