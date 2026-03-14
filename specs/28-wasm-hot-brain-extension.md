# WASM Hot Brain Extension - Pluggable Analyzer Design

## 1. Purpose

This document defines the WASM extension model for Hot Brain.

It answers:

```text
How can Hot Brain be extended dynamically
without turning the system into an uncontrolled execution platform?
```

This design exists because the system may benefit from:

- project-specific analyzers
- domain-specific coordination logic
- hot-swappable analysis modules
- user-installed reasoning strategies

But that power must remain bounded.

## 2. One-Sentence Definition

```text
WASM Hot Brain extensions are pluggable, bounded, read-only analyzers
that consume world slices and emit candidate outputs.
```

## 3. Core Decision

This is the most important rule:

```text
WASM extensions are analysis extensions, not execution authorities.
```

That means:

- they may read bounded slices
- they may emit candidates
- they may not directly mutate world state
- they may not bypass deterministic consumers

## 4. Why This Feature Exists

The built-in Hot Brain can only encode a fixed set of coordination heuristics.

WASM extensions allow:

- temporary or project-specific analysis logic
- richer relation-aware reasoning
- custom blocker clustering
- custom artifact relevance scoring
- custom memory ranking strategies

This gives the system a path to evolve without:

- recompiling the entire host
- hardcoding every new reasoning mode in Rust

## 5. Position in the Architecture

ASCII:

```text
                +-----------------------------+
                |        ECS World            |
                +--------------+--------------+
                               |
                               v
                +-----------------------------+
                |      WorldSlice Builder     |
                +--------------+--------------+
                               |
                               v
                +-----------------------------+
                |       HotBrainHost          |
                |-----------------------------|
                | built-in analyzers          |
                | wasm analyzers              |
                +--------------+--------------+
                               |
                               v
                +-----------------------------+
                |      CandidateSet merge     |
                +--------------+--------------+
                               |
                               v
                +-----------------------------+
                | Deterministic Consumers     |
                +-----------------------------+
```

Short version:

```text
WASM lives inside HotBrainHost.
It does not live inside the guarded runtime.
```

## 6. What WASM Extensions May Do

Allowed:

- inspect a bounded `WorldSlice`
- run local heuristics
- rank or classify nearby context
- emit:
  - `SchedulerHint`
  - `MemoryCandidate`
  - future bounded candidate types

Examples:

- API package relevance analyzer
- dependency drift analyzer
- repeated blocker pattern analyzer
- artifact handoff prioritizer

## 7. What WASM Extensions Must Not Do

Forbidden:

- mutate ECS world directly
- create guarded commits
- directly promote memory
- directly trigger transport/session control
- directly rewrite `FeedbackPacket`
- read full world without slicing

ASCII:

```text
Allowed:
WorldSlice -> WASM -> CandidateSet

Forbidden:
WASM -> modify relations / scheduler / memory / tmux
```

## 8. Host vs Extension Responsibilities

### 8.1 HotBrainHost responsibilities

The host must:

1. choose which analyzers are enabled
2. build bounded `WorldSlice`
3. enforce input limits
4. invoke analyzers safely
5. merge analyzer outputs
6. attach analyzer identity/metadata
7. send merged candidates into deterministic consumers

### 8.2 WASM extension responsibilities

The extension may:

1. read provided slice input
2. compute bounded candidate outputs
3. return explanation / metadata

The extension may not assume:

- access to filesystem
- access to network
- access to tmux
- access to full runtime internals

## 9. Input Model

WASM analyzers must not consume the full world.
They consume a bounded serialized slice.

ASCII:

```text
ECS World
   |
   v
WorldSliceBuilder
   |
   v
Serialized Slice Payload
   |
   v
WASM Analyzer
```

### 9.1 Recommended V1 input

WASM analyzers should initially consume:

```text
CoordinationSlice only
```

Not yet:

- full neighborhood graph
- full session turn details
- all packets in workspace

This keeps the first version small and safe.

## 10. Output Model

The extension returns a bounded candidate set.

Suggested conceptual shape:

```rust
pub struct WasmAnalyzerOutput {
    pub analyzer_id: String,
    pub analyzer_version: String,
    pub scheduler_hints: Vec<SchedulerHint>,
    pub memory_candidates: Vec<MemoryCandidate>,
}
```

### 10.1 Boundaries

Every analyzer output must obey host-enforced limits:

- max scheduler hints
- max memory candidates
- max output bytes

## 11. Step and Resource Limits

WASM analyzers must be constrained in four ways.

### 11.1 Step limit

Each packet-triggered Hot Brain pass should invoke analyzers once and stop.

```text
one slice
 -> one analyzer invocation
 -> one output
 -> stop
```

### 11.2 Material limit

Analyzer input must already be bounded by slice limits.

Examples:

- max recent packets = 3
- max targets = 4

### 11.3 Output limit

Host must truncate or reject oversized outputs.

Examples:

- max 3 scheduler hints
- max 3 memory candidates

### 11.4 Runtime limit

Each analyzer invocation should be bounded by:

- CPU/time limit
- memory limit
- panic/trap isolation

## 12. Identity and Traceability

Every analyzer invocation must be attributable.

Suggested metadata:

```rust
pub struct AnalyzerIdentity {
    pub analyzer_id: String,
    pub version: String,
    pub hash: String,
}
```

Why this matters:

- hot reload changes behavior over time
- different analyzers may disagree
- audit needs to explain where a hint came from

Hard rule:

```text
No candidate from WASM should enter the system without analyzer identity.
```

## 13. Hot Reload Model

Hot reload is allowed, but must be controlled.

### 13.1 Safe model

```text
1. Host loads analyzer set
2. One packet-triggered pass starts
3. Analyzer set is frozen for that pass
4. Pass completes
5. New analyzer versions may be loaded afterward
```

### 13.2 Unsafe model

```text
Analyzer changes halfway through one coordination pass
```

This must not happen.

### 13.3 Rule

```text
Analyzer sets are immutable per analysis pass.
```

## 14. Merge Strategy

Multiple analyzers may emit overlapping suggestions.

So the host must merge before consumers see results.

ASCII:

```text
Built-in analyzer ----\
WASM analyzer A -------> merge -> CandidateSet
WASM analyzer B ------/
```

### 14.1 V1 merge rules

Recommended first rules:

- concatenate by type
- preserve analyzer identity in metadata
- dedup later in deterministic consumers

In other words:

```text
merge first
dedup in consumers
```

This keeps the host simple.

## 15. Why This Is Better Than Scripting Directly Inside Runtime

Without this design:

```text
user script / plugin
 -> touches world directly
 -> scheduler drift
 -> audit confusion
 -> hard-to-debug behavior
```

With this design:

```text
WASM
 -> bounded analysis
 -> candidate output
 -> deterministic consumer
 -> bounded consequence
```

This preserves the architecture.

## 16. V1 Implementation Strategy

Recommended first implementation:

### Phase W0

Document and freeze the contract.

### Phase W1

Introduce `HotBrainHost` conceptually in code, even if only with built-in analyzers.

### Phase W2

Define analyzer input/output interfaces and identity metadata.

### Phase W3

Load and invoke one bounded WASM analyzer against `CoordinationSlice`.

### Phase W4

Merge its outputs with built-in analyzer outputs and send to deterministic consumers.

## 17. What Not To Do In V1

Do not:

- support arbitrary world mutation from WASM
- expose transport/session control APIs to WASM
- support unrestricted filesystem/network by default
- let WASM define its own packet schema
- let WASM bypass consumers

## 18. Relationship to Your Product Goal

This design supports your broader goal:

```text
users can extend the coordination intelligence of the system
without destabilizing the execution core
```

That is exactly the right place to let the system become more expressive.

## 19. Final Statement

WASM Hot Brain is not:

```text
dynamic runtime authority
```

It is:

```text
dynamic analysis authority
```

That distinction is what keeps the architecture safe and extensible.
