# Cathulu roadmap

This roadmap preserves the phase order from the project brief and distinguishes planned work from delivered evidence.

**Current status (2026-10-21):** M1-M4 are validated and published to `main`; M4 code is at `185741d`. M4 adds selective local SQL/FTS retrieval, a manual proposal-first Gardener, provenance-linked consolidation candidates, conservative evidence-reference archiving, versioned memory-policy experiments, storage accounting, and scoped HOT-health metrics. It passes 17 Rust tests, three React tests, formatting, frontend and Windows release builds, CLI maintenance smoke checks, and an isolated desktop startup/database smoke. GitHub reports no status checks or workflow runs for M4. Semantic indexing, Graphify invalidation, and raw telemetry compaction remain deferred because their source signals or measured need are not present. The owner-controlled live Codex hook trust/configuration smoke and the combined M1-M4 integration pass remain outstanding. The memory architecture and staged lifecycle remain in [ADR-0003](decisions/ADR-0003-managed-local-memory-lifecycle.md) and [PLAN-001](plans/PLAN-001-phase1-first-slice.md).

## Phase 1 — Instrumentation and state foundation

Start with the vertical slice in the project definition: local project registration, SQLite persistence, deterministic Git reconciliation, local MCP state reads, read-only dashboard, and automated tests. Extend it with objectives/tasks, decisions, immutable workflow versions, transactional event/state updates, runs, QA evidence, available local Codex telemetry, blockers, recent activity, and restart recovery. Add the memory substrate in this phase: independent class and temperature, stable identity and hashes, provenance, temporal validity and supersession, freshness markers, known-by state, context-transfer records, artifact references, and bounded HOT-memory scopes. This is a schema and deterministic-policy foundation, not a full automatic memory agent.

No worker orchestration or model/effort selectors are included. Cathulu will host the selectors in a later milestone, after the Codex configuration pathway and role-specific support are verified.

## Phase 2 — Memory lifecycle and context experiment

Use the Phase 1 transfer ledger and memory metadata to measure context reintroduction, amplification, reuse, retrieval usefulness, and storage growth. M4 delivers selective SQL/FTS retrieval, a manual deterministic Memory Gardener for duplicate detection and reviewable promotion/demotion proposals, explicit provenance-linked consolidation candidates, extraction receipts, conservative reference-aware archiving, policy experiments, and storage/health metrics. Unique provenance and protected evidence remain traceable; retention stays configurable and disabled until an owner sets a minimum age. Later work may add summary/aggregate maintenance, telemetry compaction, or semantic indexing if usage and measured retrieval gaps justify them; any semantic index must return candidate IDs while SQLite remains authoritative. Graphify invalidation waits for a real graph snapshot source.

## Phase 3 — Codex-native worker delegation

Reuse native Codex subagents and worktrees. Add task packets, structured evidence reports, delegation metadata, worktree isolation records, and worker telemetry. Verify how Cathulu can apply its selected model/effort values to Codex, and whether separate values per role are supported, before adding the selector surface.

## Phase 4 — Durable scoped context

Add scopes and Sub-Overseers only after telemetry shows repeated cross-project reconstruction whose measured cost exceeds context maintenance and coordination overhead.

## Phase 5 — Graphify

Integrate structural retrieval read-only, record graph snapshot identities/deltas, invalidate only memories tied to changed graph bases, and benchmark selected neighborhoods against simpler file/FTS retrieval. Graphify is a retrieval/freshness source, not a second memory authority.

## Phase 6 — Convergence

Add quality-distance vectors, state signatures, loop/stall/oscillation/regression detection, and escalation/replanning after enough run evidence exists.

## Phase 7 — Workflow experiments

Aggregate comparable runs by task class and workflow version; propose, review, and version workflow changes; compare future outcomes.

## Phase 8 — Adaptive policy research

Consider learned routing or automatic scope/policy changes only after enough trustworthy repeated data exists. Reinforcement learning and automatic policy changes are outside the MVP.
