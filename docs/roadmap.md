# Cathulu roadmap

This roadmap preserves the phase order from the project brief and distinguishes planned work from delivered evidence.

**Current status (2026-10-21):** M1 and the validated M2 workflow/managed-memory foundation are published on `main` (`a3ca960`). M2 passes Rust, React, production-build, Windows release-build, isolated startup/database, and dashboard preview/focus checks. The live Codex MCP connection remains an owner-controlled follow-up. The accepted managed-memory architecture and staged scope are recorded in [ADR-0003](decisions/ADR-0003-managed-local-memory-lifecycle.md) and [PLAN-001](plans/PLAN-001-phase1-first-slice.md).

## Phase 1 — Instrumentation and state foundation

Start with the vertical slice in the project definition: local project registration, SQLite persistence, deterministic Git reconciliation, local MCP state reads, read-only dashboard, and automated tests. Extend it with objectives/tasks, decisions, immutable workflow versions, transactional event/state updates, runs, QA evidence, available local Codex telemetry, blockers, recent activity, and restart recovery. Add the memory substrate in this phase: independent class and temperature, stable identity and hashes, provenance, temporal validity and supersession, freshness markers, known-by state, context-transfer records, artifact references, and bounded HOT-memory scopes. This is a schema and deterministic-policy foundation, not a full automatic memory agent.

No worker orchestration or model/effort selectors are included. Cathulu will host the selectors in a later milestone, after the Codex configuration pathway and role-specific support are verified.

## Phase 2 — Memory lifecycle and context experiment

Use the Phase 1 transfer ledger and memory metadata to measure context reintroduction, amplification, reuse, retrieval usefulness, and storage growth. Add selective SQL/FTS retrieval and a local deterministic Memory Gardener for duplicate detection, promotion/demotion proposals, consolidation, evidence compaction, provenance checks, and reference-aware garbage collection. Preserve unique provenance and protected evidence; retention remains configurable and does not use arbitrary age cutoffs. Version memory policies and compare them against matched ordinary-Codex runs using verified outcomes, rework, context volume, latency, and human intervention. Add semantic indexing only if measured retrieval gaps justify it; any index returns candidate IDs and SQLite remains authoritative.

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
