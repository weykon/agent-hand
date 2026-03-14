# WASM Hot Brain Extension - Execution Brief

## 1. Purpose

This is the execution brief for the first implementation stage of WASM-based Hot Brain extensions.

Primary design source:

- `specs/28-wasm-hot-brain-extension.md`

Related docs:

- `specs/12-hot-brain-runtime.md`
- `specs/15-world-slice-v1.md`
- `specs/17-candidate-consumers.md`

## 2. Goal

Implement the first safe boundary for WASM-based Hot Brain analyzers.

Not a full dynamic plugin platform.

Just the first bounded layer:

```text
CoordinationSlice
 -> WASM analyzer
 -> bounded candidate output
 -> merge into normal Hot Brain flow
```

## 3. In Scope

In scope:

- analyzer identity metadata
- analyzer input/output contract
- host-side invocation boundary
- one bounded analyzer execution path

Out of scope:

- world mutation from WASM
- transport/session control from WASM
- full hot reload UX
- arbitrary filesystem/network access

## 4. Recommended First Deliverables

### 4.1 Define analyzer contract

Conceptual structures:

- `AnalyzerIdentity`
- `WasmAnalyzerInput`
- `WasmAnalyzerOutput`

### 4.2 Define host interface

Add a small host-side abstraction for analyzer invocation.

### 4.3 Keep input bounded

Only allow:

```text
CoordinationSlice
```

as input for the first version.

### 4.4 Keep output bounded

Only allow:

- `SchedulerHint[]`
- `MemoryCandidate[]`

within strict caps.

## 5. Constraints

Do not:

- let WASM touch ECS world directly
- let WASM emit commits
- let WASM bypass deterministic consumers
- let WASM define new transport behavior

## 6. Short Task Prompt

```text
Use specs/28-wasm-hot-brain-extension.md as the primary design source.

Implement the first safe WASM Hot Brain extension boundary:
- define analyzer identity and IO contract
- define host-side analyzer invocation boundary
- keep input limited to CoordinationSlice
- keep output limited to bounded SchedulerHint and MemoryCandidate lists

Do not allow world mutation, transport control, or consumer bypass.
```
