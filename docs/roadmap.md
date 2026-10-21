# Cathulu roadmap

This roadmap preserves the phase order from the project brief and distinguishes planned work from delivered evidence.

**Current status (2026-10-21):** M1-M8 are validated and published to `main`; M5's implementation checkpoint is `e29c90f` and documentation closeout `744a428`, M6 is `1814ccd`, M7 is `6251515`, and M8 is `aecac57`. M8 adds existing-worktree Git verification and explicit, privacy-minimized worker lifecycle telemetry. GitHub reported no status checks or workflow runs for M8; local workspace tests and the Windows MCP/Tauri/restart smoke passed. Semantic indexing, Graphify invalidation, and raw telemetry compaction remain deferred because their source signals or measured need are not present. Official Codex docs confirm per-role model/effort configuration; the owner-approved apply flow and account-specific catalog remain open for M9. See [ADR-0002](decisions/ADR-0002-cathulu-hosted-agent-selectors.md), [ADR-0003](decisions/ADR-0003-managed-local-memory-lifecycle.md), [ADR-0005](decisions/ADR-0005-codex-worker-telemetry-and-worktree-verification.md), [PLAN-001](plans/PLAN-001-phase1-first-slice.md), and [PLAN-002](plans/PLAN-002-phase3-codex-delegation.md).

## Phase 1 — Instrumentation and state foundation (M1-M3)

Start with the vertical slice in the project definition: local project registration, SQLite persistence, deterministic Git reconciliation, local MCP state reads, read-only dashboard, and automated tests. Extend it with objectives/tasks, decisions, immutable workflow versions, transactional event/state updates, runs, QA evidence, available local Codex telemetry, blockers, recent activity, and restart recovery. Add the memory substrate in this phase: independent class and temperature, stable identity and hashes, provenance, temporal validity and supersession, freshness markers, known-by state, context-transfer records, artifact references, and bounded HOT-memory scopes. This is a schema and deterministic-policy foundation, not a full automatic memory agent.

No worker orchestration or model/effort selectors are included in M1-M3. Codex's supported configuration pathway and role-specific model/effort fields are verified; Cathulu's selector surface remains Phase 3 M9 after the owner-approved preview/apply behavior and option catalog are defined.

## Phase 2 — Memory lifecycle and context experiment (M4-M5)

Use the Phase 1 transfer ledger and memory metadata to measure context reintroduction, amplification, reuse, retrieval usefulness, and storage growth. M4 delivers selective SQL/FTS retrieval, a manual deterministic Memory Gardener for duplicate detection and reviewable promotion/demotion proposals, explicit provenance-linked consolidation candidates, extraction receipts, conservative reference-aware archiving, policy experiments, and storage/health metrics. Unique provenance and protected evidence remain traceable; retention stays configurable and disabled until an owner sets a minimum age. Later work may add summary/aggregate maintenance, telemetry compaction, or semantic indexing if usage and measured retrieval gaps justify them; any semantic index must return candidate IDs while SQLite remains authoritative. Graphify invalidation waits for a real graph snapshot source.

## Phase 3 — Codex-native worker delegation (M6-M9)

Reuse native Codex subagents and worktrees. M6 adds bounded typed task packets, hashed idempotency/correlation metadata, scope-checked memory and artifact links, and append-only delegation lifecycle events; it records owner handoff state without spawning agents. M7 adds structured worker reports and read-only dashboard presentation; M8 adds verifiable worktree isolation and supported worker telemetry. Official documentation confirms that Codex supports main-session model/effort configuration and distinct custom-agent role values; M9 will define the owner's preview/apply flow and supported option catalog before adding selectors. Detailed milestones are in [PLAN-002](plans/PLAN-002-phase3-codex-delegation.md).

## Phase 4 — Durable scoped context (M10)

Add scopes and Sub-Overseers only after telemetry shows repeated cross-project reconstruction whose measured cost exceeds context maintenance and coordination overhead.

## Phase 5 — Graphify (M11)

Integrate structural retrieval read-only, record graph snapshot identities/deltas, invalidate only memories tied to changed graph bases, and benchmark selected neighborhoods against simpler file/FTS retrieval. Graphify is a retrieval/freshness source, not a second memory authority.

## Phase 6 — Convergence (M12)

Add quality-distance vectors, state signatures, loop/stall/oscillation/regression detection, and escalation/replanning after enough run evidence exists.

## Phase 7 — Workflow experiments (M13)

Aggregate comparable runs by task class and workflow version; propose, review, and version workflow changes; compare future outcomes.

## Phase 8 — Adaptive policy research (M14)

Consider learned routing or automatic scope/policy changes only after enough trustworthy repeated data exists. Reinforcement learning and automatic policy changes are outside the MVP.
