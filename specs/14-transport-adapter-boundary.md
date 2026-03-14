# Transport Adapter Boundary - tmux/hooks and ACPX/ACP

## 1. Purpose

This document defines the transport boundary for Agent Hand.

Its purpose is to prevent protocol and control details from leaking upward into:

- guarded context runtime
- feedback packet semantics
- Hot Brain runtime
- scheduler logic

This document is the design anchor for supporting both:

- today's tmux/hooks/control-socket integration
- future ACPX/ACP-style structured agent integration

## 2. Why This Boundary Matters

Right now, Agent Hand primarily interacts with external coding agents through:

- tmux sessions
- hook events
- pane inspection
- control socket operations

That is practical, but it is also transport-specific.

At the same time, newer structured protocols such as ACP/ACPX aim to replace:

```text
PTY scraping
```

with:

```text
typed event exchange
```

If the system is designed correctly, this should **not** force a rewrite of:

- packet structure
- guard behavior
- scheduler semantics
- Hot Brain analysis

That only works if transport stays in its own layer.

## 3. One-Sentence Definition

```text
Transport adapters are the only layer allowed to know how Agent Hand talks to external agent runtimes.
```

## 4. The Current Situation

Today's effective stack is:

```text
external coding agent CLI
   ^
   |
tmux pane / hooks / control socket
   ^
   |
agent-hand sidecar runtime
```

This means:

- session control is tmux-based
- event ingress is hook-based
- state visibility partly comes from pane output and status heuristics

That is acceptable for the current product stage, but it must not become the permanent semantic model of the system.

## 5. Future Situation

Structured protocols such as ACP/ACPX introduce a different kind of lower layer:

```text
external coding agent
   ^
   |
ACP/ACPX protocol
   ^
   |
agent-hand adapter
```

This means:

- typed events instead of inferred pane state
- typed control operations instead of tmux keystrokes
- direct protocol responses instead of output scraping

This is a major improvement at the transport layer.
It is **not** supposed to redefine the upper runtime layers.

## 6. Core Rule

The transport layer must be replaceable.

Hard rule:

```text
Upper runtime layers must depend on transport-independent semantics,
not tmux details or ACP details.
```

## 7. Layered Architecture

ASCII:

```text
                   +------------------------------+
                   | Guarded Runtime              |
                   | proposal / evidence / guard  |
                   | feedback packet / audit      |
                   +--------------+---------------+
                                  |
                                  v
                   +------------------------------+
                   | Hot Brain / Coordination     |
                   | world slices / candidates    |
                   | scheduler / memory ingest    |
                   +--------------+---------------+
                                  |
                                  v
                   +------------------------------+
                   | Transport Adapter Boundary   |
                   | event ingress / control      |
                   | artifact projection          |
                   +--------+---------------+-----+
                            |               |
                            v               v
                      tmux/hooks       ACPX / ACP
```

Short version:

```text
Upper layers reason in domain terms.
Only adapters reason in protocol terms.
```

## 8. What Belongs Above the Boundary

These concerns must remain transport-independent:

### 8.1 Guarded runtime

- proposal creation
- evidence shape
- guard checks
- guarded commit semantics
- feedback packet semantics

### 8.2 Hot Brain runtime

- world slices
- candidate generation
- scheduler hints
- memory candidates

### 8.3 Scheduler and memory

- consuming packets
- relation-aware routing
- long-term memory ingest

### 8.4 Canvas and other views

- relation topology
- scheduling view
- evidence view
- workflow view

All of the above should remain meaningful even if tmux disappears and ACP becomes the primary transport.

## 9. What Belongs Below the Boundary

These concerns are adapter-specific:

### 9.1 Event ingress

Questions:

```text
How do we learn what happened?
```

Examples:

- hook JSON line
- socket-delivered hook event
- ACP/ACPX event message
- pane-derived fallback signal

### 9.2 Live control

Questions:

```text
How do we interrupt / resume / send input?
```

Examples:

- tmux `send-keys`
- tmux Escape interrupt
- ACP control message
- protocol-native resume operation

### 9.3 Artifact projection

Questions:

```text
How do we deliver allowed context to the agent runtime?
```

Examples:

- filesystem artifact
- hook `additionalContext`
- PTY send
- ACP context payload

### 9.4 Capability detection

Questions:

```text
What can this transport/runtime actually support?
```

Examples:

- can inject pre-prompt context?
- can resume conversation?
- can interrupt safely?
- can emit structured tool events?

## 10. Domain Semantics vs Transport Semantics

This is the most important distinction.

### 10.1 Domain semantics

These are stable concepts:

- session
- relationship
- proposal
- evidence
- guarded commit
- feedback packet
- scheduler hint
- memory candidate

### 10.2 Transport semantics

These are implementation details:

- tmux session name
- hook event JSON shape
- ACP message envelope
- PTY input injection
- control socket payload shape

Hard rule:

```text
Transport semantics may carry domain semantics.
Transport semantics must not define domain semantics.
```

## 11. The Three Adapter Responsibilities

Each transport adapter has exactly three jobs.

### 11.1 Normalize incoming events

ASCII:

```text
transport event
   |
   v
normalized runtime event
```

Examples:

- hook JSON -> `HookEvent`
- ACP event -> normalized session/tool/runtime event

### 11.2 Execute control requests

ASCII:

```text
runtime control intent
   |
   v
transport-specific operation
```

Examples:

- interrupt session
- resume conversation
- send prompt

### 11.3 Deliver projections

ASCII:

```text
InjectionEnvelope / runtime projection
   |
   v
actual transport-specific delivery
```

Examples:

- write file
- emit hook context
- send PTY text
- send ACP context payload

## 12. Recommended Adapter Interface

The exact code shape can vary, but conceptually the adapter boundary should look like:

```rust
pub trait TransportAdapter {
    // Event ingress
    fn normalize_event(&self, raw: RawTransportEvent) -> Option<RuntimeEvent>;

    // Control plane
    fn execute_control(&self, intent: ControlIntent) -> Result<ControlResult>;

    // Projection delivery
    fn deliver_projection(&self, projection: DeliveryProjection) -> Result<DeliveryResult>;

    // Capability reporting
    fn capabilities(&self) -> TransportCapabilities;
}
```

This interface is conceptual.
The immediate goal is not to implement it fully, but to design toward it.

## 13. TransportCapabilities

Agent Hand should explicitly track what a transport supports.

Suggested conceptual shape:

```rust
pub struct TransportCapabilities {
    pub supports_structured_events: bool,
    pub supports_live_interrupt: bool,
    pub supports_resume: bool,
    pub supports_pre_prompt_context: bool,
    pub supports_post_turn_artifact_projection: bool,
    pub supports_user_visible_prompt_injection: bool,
}
```

Why this matters:

- not every runtime can do hook context injection
- not every runtime can safely resume
- not every runtime has a concept of PTY
- ACP-style transports and tmux-style transports differ radically

Capabilities make those differences explicit without polluting upper layers.

## 14. Mapping Current Transport to the Model

### 14.1 Current tmux/hooks path

Current adapter pieces:

```text
Ingress:
- HookSocketServer
- EventReceiver
- hook_event_bridge.sh

Control:
- control socket
- tmux manager
- send_keys / interrupt / resume logic

Projection:
- .agent-hand-context.md
- CLAUDE.md reference
- hook additionalContext (experimental/current)
- PTY send prompt
```

### 14.2 Current risks

The current lower layer is still semantically leaky in a few places:

1. `resume` behavior can easily drift into transport-specific assumptions
2. multiple projection channels may duplicate the same context
3. transport behavior may be mistaken for orchestration semantics

This is exactly why this boundary document exists.

## 15. Mapping ACPX/ACP to the Model

### 15.1 What ACPX changes

ACPX can potentially replace:

- PTY scraping
- fragile hook inference
- tmux-mediated control for compatible runtimes

with:

- structured event ingress
- structured control
- structured context delivery

### 15.2 What ACPX does not change

ACPX should **not** redefine:

- `FeedbackPacket`
- `Proposal`
- `EvidenceRecord`
- `GuardedCommit`
- `WorldSlice`
- `SchedulerHint`
- `MemoryCandidate`

Those remain upper-layer domain objects.

### 15.3 Correct adoption model

ASCII:

```text
Wrong:
ACPX protocol
  -> redefine scheduler / packet / memory semantics

Right:
ACPX adapter
  -> normalize to existing runtime semantics
```

## 16. Delivery Channels as Adapter Concerns

The following delivery modes all belong below the boundary:

- filesystem artifact
- hook `additionalContext`
- PTY send
- future ACP direct payload

This means the packet and guarded runtime do not need to know which one is used.

ASCII:

```text
FeedbackPacket / InjectionEnvelope
          |
          v
   [Delivery Adapter]
      |    |    |    |
      v    v    v    v
     file hook PTY  ACP
```

## 17. Execution Plan

### Phase T0 - Document and freeze the boundary

Done when:

- this document is accepted as the transport boundary source of truth

### Phase T1 - Keep tmux/hooks path working

Goal:

- continue current MVP and E2E on existing adapter path
- do not force ACP adoption yet

### Phase T2 - Introduce transport capability reporting

Goal:

- explicitly describe what current tmux/hooks transport can do
- make limitations visible in one place

### Phase T3 - Introduce internal adapter abstraction

Goal:

- create a narrow internal interface for:
  - ingress normalization
  - control execution
  - projection delivery

### Phase T4 - Evaluate ACPX adapter

Goal:

- map ACPX into the adapter boundary
- reuse upper runtime unchanged

## 18. What To Discuss Next

After this document, the most useful next design topic is:

```text
WorldSlice V1
```

Reason:

- transport boundary is now defined
- packet semantics are already frozen
- Hot Brain depends directly on slice boundaries

So the clean progression is:

```text
Transport Boundary
  -> WorldSlice V1
  -> Candidate Consumers
```

## 19. Final Statement

Agent Hand must be designed so that:

```text
tmux/hooks is one transport adapter
ACPX/ACP is another transport adapter
the guarded runtime, packet model, and Hot Brain remain above both
```

That is the key to keeping the architecture extensible without losing semantic coherence.
