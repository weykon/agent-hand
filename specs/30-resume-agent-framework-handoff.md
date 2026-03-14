# Resume Handoff - Positioning Agent Hand and Related Agent Systems Work

## 1. Purpose

This document is the handoff for another agent that will update the resume materials under:

- `/Users/weykon/Desktop/p/new-resume/resume/zh.md`
- `/Users/weykon/Desktop/p/new-resume/resume/en.md`
- optionally:
  - `/Users/weykon/Desktop/p/new-resume/summaries/project-highlights.md`
  - `/Users/weykon/Desktop/p/new-resume/summaries/skills-matrix.md`

The goal is to position the current work in a way that matches the market for:

- AI agent infrastructure
- agentic orchestration
- LLM systems / runtime engineering
- context / memory / guardrails / observability
- client/runtime surfaces for agent workflows

This handoff is intentionally written to be resume-edit ready.

## 2. Market Framing - What The Market Is Asking For

Based on current job postings and role summaries, the market is repeatedly asking for these capability clusters:

### Cluster A - Agent orchestration and runtime design

Common language in current roles:

- design and build orchestration infrastructure for LLM-based agents
- multi-agent workflows
- long-horizon reasoning / planning / tool routing
- structured agent lifecycle and stateful execution

### Cluster B - Context, memory, and retrieval

Common language:

- context engineering
- dynamic context assembly
- memory systems across sessions
- RAG / knowledge grounding
- graph / vector / stateful memory representation

### Cluster C - Safety, reliability, and observability

Common language:

- guardrails
- evaluation / hallucination detection
- logging / monitoring / observability
- production reliability
- approval and policy gates

### Cluster D - Systems + infrastructure capability

Common language:

- production-grade backend systems
- APIs / databases / services integration
- asynchronous / distributed systems
- cloud or infra-aware deployment paths

### Cluster E - UX / real-time interaction surfaces

More advanced roles increasingly mention:

- real-time user workflows
- interaction patterns
- toolchain infrastructure
- context handoff across sessions and surfaces

## 3. External Market Evidence

These examples are useful as positioning references, not as text to copy verbatim:

1. **GE Vernova - AI Agent Engineer**
   - asks for AI agents, multi-agent architectures, planning, autonomy, backend/API/database integration
   - source: <https://careers.gevernova.com/ai-agent-engineer/job/R5021726>

2. **Irvine Company - Principal AI Engineer**
   - asks for agentic systems, multi-agent workflows, context engineering, monitoring/logging/observability, persistence and schema design
   - source: <https://careers.irvinecompany.com/job/Irvine-Principal-AI-Engineer-%28%24211%2C200-%24263%2C800%29-CA-92617/1363975700/>

3. **Inizio Partners - AI Engineer, Agentic & RAG Systems**
   - asks for multi-agent orchestration, guardrails, RAG, evaluation frameworks, end-to-end production systems
   - source: <https://www.careers-page.com/inizio-partners-corp/job/W335XW96>

4. **HP IQ / CosmOS - Senior AI Engineer, Agentic Orchestration**
   - asks for orchestration infrastructure, memory systems, context graphs, toolchain infrastructure, interaction patterns, real-world user workflows
   - source: <https://www.sonara.ai/job-details/10b8f3ef-5ddc-5094-939c-ae2dde4c27c9>

5. **HumanBit - AI Engineer, Multi-Agent Systems & Orchestration**
   - asks for stateful execution, resource scheduling, process isolation, memory architectures, orchestration layer ownership
   - source: <https://jobs.humanbit.ai/jobs/2d1f4312-5652-4f1a-b500-c37fdf409f69>

6. **Builders + Backers - Full Stack AI Engineer**
   - asks for orchestration across models, context management, and systems that improve through user interaction
   - source: <https://wellfound.com/jobs/3655820-full-stack-ai-engineer>

## 4. The Positioning We Should Use

The strongest positioning is **not**:

```text
I built a chatbot app.
```

The strongest positioning is:

```text
I design and build the runtime, coordination, safety, and interaction substrate
that makes multi-agent systems usable in real work.
```

For this project, the right market-facing identity is:

```text
AI Agent Systems / Runtime / Orchestration Engineer
with strong Rust systems depth and operator-facing workflow design
```

## 5. What This Project Really Is

`Agent Hand` should be positioned as:

```text
a tmux-backed multi-agent session control plane and coordination runtime
for AI coding agents
```

Not just:

```text
a terminal session manager
```

The stronger and more accurate framing is:

```text
an operator-facing runtime for managing, observing, prioritizing, and coordinating many live coding-agent sessions
```

## 6. What Is Safe To Claim

This section is critical.

The resume must distinguish between:

- implemented and running capabilities
- architecture that exists in code but is still maturing
- designed future direction that should not be overclaimed

### 6.1 Safe to claim as implemented

These are supported by code and/or active implementation work:

- tmux-backed multi-session management for AI coding agents
- real-time session status detection (`running / waiting / idle / ready`)
- hook-driven runtime event ingestion
- guarded context injection path
- append-only runtime/audit streams
- packet-driven coordination layer foundation
- bounded Hot Brain analyzer over recent coordination packets
- deterministic normalization of scheduler and memory outputs
- scheduler-side state and follow-up proposal records
- cold-memory promotion path foundation
- projection/view-model layer for relationship/scheduler/evidence/workflow views

### 6.2 Safe to claim as "designed and partially implemented"

Use softer language here:

- second-layer coordination runtime
- packet-driven cross-session reasoning
- workflow and evidence projections over a shared world model
- transport-adapter path toward structured protocols

Recommended language:

```text
designed and began implementing
architected and wired the first runtime path for
built the foundation for
```

### 6.3 Do NOT claim as already delivered

Avoid claiming these as complete product capabilities:

- full autonomous scheduler
- full ACPX/ACP integration
- complete multi-view canvas workflow UI
- unrestricted cross-session semantic automation

Those should appear only as:

```text
architecture direction / system design / internal roadmap
```

if mentioned at all.

## 7. Recommended Resume Narrative

The best narrative is:

### Layer 1

```text
Built an operator-facing multi-agent session runtime
```

### Layer 2

```text
Added guarded context, packetized coordination, and bounded semantic analysis
```

### Layer 3

```text
Moved toward a shared world model that can project relationships, workflow, evidence, and scheduling state
```

This tells the market:

- systems thinking
- agent orchestration depth
- safety and observability awareness
- workflow product sense

## 8. Recommended Chinese Resume Wording

### Option A - concise version

可作为项目 bullet：

```text
- 设计并实现基于 Rust + Tokio 的 AI coding agents 多会话运行控制层，支持 tmux-backed session orchestration、实时状态检测与优先级切换。
- 构建 hook-driven guarded runtime：将 HookEvent 经过 Proposal / Evidence / Guard / Commit 流程转化为可审计的上下文注入与运行时事件记录。
- 设计并落地 FeedbackPacket + Hot Brain 协调层，对多会话结果进行 bounded packet analysis，产出 scheduler hints、memory candidates 与 follow-up proposal records。
- 建立 append-only runtime audit、scheduler state、cold-memory promotion 与 workflow/evidence view-model 投影，为后续多视图工作流协同和安全治理打下基础。
```

### Option B - stronger architecture version

如果想更偏架构岗位：

```text
- 主导构建 Agent Hand：面向 AI coding agents 的多会话控制平面与协调运行时，而非单纯终端管理工具。
- 以 Rust 实现 packet-driven coordination runtime，分层拆解为 Guarded Live Runtime、Hot Brain、Deterministic Consumers、Memory Boundary 与多视图 Projection。
- 将实时 Hook 事件、上下文注入、安全门控、调度建议、冷记忆提升与工作流投影统一到共享 world model 之上，形成可审计、可扩展的 agent runtime substrate。
```

## 9. Recommended English Resume Wording

### Option A - concise version

```text
- Designed and implemented a Rust/Tokio-based multi-session runtime for AI coding agents, including tmux-backed session orchestration, real-time status detection, and operator-first session control.
- Built a hook-driven guarded runtime where HookEvents flow through Proposal / Evidence / Guard / Commit stages before context injection or runtime-side effects are allowed.
- Implemented the foundation of a packet-driven coordination layer using FeedbackPacket, bounded Hot Brain analysis, deterministic consumers, scheduler-side state, and cold-memory promotion artifacts.
- Added append-only audit streams and shared projection/view-model layers for relationship, scheduler, evidence, and workflow views over a common runtime world model.
```

### Option B - systems/architecture version

```text
- Built Agent Hand as a multi-agent session control plane and coordination runtime for live AI coding workflows, rather than a simple terminal session manager.
- Architected and implemented layered runtime boundaries spanning guarded live execution, packetized coordination, bounded semantic analysis, deterministic scheduling/memory consumers, and projection-ready workflow views.
- Focused on agent runtime safety, observability, and coordination: audit trails, guard-gated context flow, scheduler-side formalization, and reusable memory-promotion pipelines.
```

## 10. Which Project Entry To Add Or Update

### Primary recommendation

Add or update a project entry for:

```text
Agent Hand (or keep "Agent Deck" if you must preserve naming continuity, but "Agent Hand" is better)
```

Recommended category:

- AI/Agent systems
- tools/runtime

### Why this entry matters

Because this project directly maps to current market demand for:

- agent orchestration
- context management
- guardrails
- observability
- workflow UX

## 11. How To Pair It With A “Paid / Commercial” Signal

If the resume should include one open-source + one more commercial-facing project, the best pair is:

### Pairing recommendation

1. **Agent Hand**
   - open-source / operator runtime / session coordination

2. **WA-Agent-Reply**
   - commercial / enterprise / Claude Agent SDK / business workflow automation

Why this pair works:

```text
Agent Hand shows runtime + orchestration substrate
WA-Agent-Reply shows production/business deployment capability
```

Alternative pair if you want stronger safety/research signal:

1. Agent Hand
2. Common Skill System

This pair emphasizes:

- safety
- guardrails
- protocol/runtime design

## 12. Recommended File Changes For The Other Agent

Suggested edit targets:

### Must update

- `/Users/weykon/Desktop/p/new-resume/resume/zh.md`
- `/Users/weykon/Desktop/p/new-resume/resume/en.md`

### Nice to update

- `/Users/weykon/Desktop/p/new-resume/summaries/project-highlights.md`
- `/Users/weykon/Desktop/p/new-resume/summaries/skills-matrix.md`

### Optional new project file

Could add a dedicated project note such as:

```text
/Users/weykon/Desktop/p/new-resume/projects/ai-ml/agent-hand.md
```

if the editing agent prefers a project-source-first workflow.

## 13. What The Other Agent Should Actually Do

The other resume-editing agent should:

1. read current `zh.md` and `en.md`
2. find the best place to insert or replace the `Agent Deck / Agent Hand` entry
3. update wording to reflect the new runtime / coordination / guard / packet / workflow framing
4. keep the wording honest about what is implemented vs still architectural
5. regenerate PDF via the existing resume tooling in `/Users/weykon/Desktop/p/new-resume/resume/`

## 14. Short Handoff Prompt For The Resume Agent

```text
Use specs/30-resume-agent-framework-handoff.md as the primary writing guide.

Update the resume materials under /Users/weykon/Desktop/p/new-resume/ to better position the current Agent Hand work for the AI agent systems / agent runtime / orchestration market.

Priorities:
1. Add or upgrade the Agent Hand project description in both Chinese and English resumes.
2. Use market-facing language around multi-agent session runtime, guarded context flow, packet-driven coordination, bounded semantic analysis, scheduler/memory artifacts, and workflow/evidence projections.
3. Keep the wording honest: emphasize implemented runtime foundations, and use softer wording for still-maturing architecture.
4. If one more commercial-facing project is needed to strengthen the profile, prefer pairing Agent Hand with WA-Agent-Reply.
5. Update summary/highlights files if needed, then regenerate the PDF outputs.
```

## 15. Final Positioning Statement

The single best concise positioning for this project is:

```text
Built an operator-facing multi-agent runtime and coordination substrate for AI coding agents, with guarded context flow, packet-driven coordination, bounded semantic analysis, and projection-ready workflow views.
```

That is the core message the resume should deliver.
