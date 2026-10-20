# Cathulu architecture and development profile

Status: product boundaries are NORMATIVE. M1 is implemented; the managed-memory architecture in ADR-0003 is accepted for M2 and later milestones, with implementation details gated by their validation evidence.

## System boundary

```text
Owner
  ↕ conversations, commands, approvals
Codex app / Main Overseer
  ↕ local MCP (domain reads and state transitions)
Cathulu Core ───── local SQLite database
  ├─ project, objective, task, and run state
  ├─ append-only event, decision, QA, and workflow history
  ├─ managed memory lifecycle over domain records and evidence
  ├─ Git reconciliation
  └─ local telemetry normalization and context-transfer ledger
       ↕ read-only query boundary (M1 dashboard)
Tauri desktop app / React UI
```

Codex owns model/tool execution, shell, Git edits, worktrees, approvals, and native subagents. Cathulu owns portfolio persistence and evidence. The M1 dashboard reads a materialized state boundary and cannot mutate project state. A future Cathulu selector surface is separate from this dashboard and is defined in ADR-0002. The local MCP server exposes domain resources and semantic operations; it never exposes SQL or table CRUD.

## Phase 1 starting stack

The repository was empty at kickoff, so the user's preferred stack applies. M1 implements this baseline; the Windows build, automated tests, and isolated startup check passed on 2026-10-20. See [PLAN-001](plans/PLAN-001-phase1-first-slice.md) for exact commands and limits.

- Tauri 2 desktop shell.
- React and TypeScript for the dashboard.
- Rust for the domain/core and local MCP process.
- SQLite with migrations, WAL, transactions, and FTS5 where textual search is needed.
- Git as the source of truth for repository state; filesystem watchers may signal dirtiness but do not replace Git reconciliation.
- Local MCP over stdio using the official RMCP Rust SDK.

Keep the Rust workspace small: a core library for project persistence and Git reconciliation; a Tauri desktop host; and an MCP executable that reuses the core library. Avoid extra services and framework layers until multiple callers or an operational need justify them.

The M1 implementation uses Tauri 2, React/TypeScript, `rusqlite` with bundled SQLite, RMCP over stdio, and Git CLI reconciliation. Exact resolved dependency versions are recorded in the Rust and npm lockfiles. The desktop host and MCP process share one SQLite database path when configured with the same data-directory environment; the dashboard reads snapshots through a Tauri command and polls every five seconds. The M1 dashboard does not mutate project state. Windows/MSVC builds embed the Common Controls v6 manifest in app and test binaries so the Tauri mock IPC test harness can load. This follows the [Tauri Windows test-manifest example](https://github.com/tauri-apps/tauri/blob/dev/examples/api/src-tauri/build.rs). A later, separate settings surface will own model/effort selection.

## Data and transaction model

SQLite is the source of truth. Use relational columns for projects, dependencies, objectives, tasks, decisions, workflow identities/versions, runs, quality checks, memory lifecycle metadata, and state needed for filtering. Use JSON for bounded heterogeneous event payloads only. Each important transition appends its domain event and updates the corresponding current-state projection in one transaction. Decisions and workflow versions are immutable. Artifacts are stored as local references and hashes where possible, not copied into prompts.

The initial data path is an OS-local application data directory. SQLite is configured for transactional writes and WAL. Startup runs database migrations. In M1, registration captures a Git snapshot and the explicit `reconcile_project` MCP tool refreshes it; startup does not yet reconcile every registered repository. Later milestones add memory metadata and broader reconciliation checks without creating another database, replacing the event model, or copying current-state tables into a parallel memory store.

### Managed memory lifecycle

Cathulu manages knowledge through **experience → evidence → extraction → candidate → consolidation → durable knowledge**. Raw evidence is not permanent agent memory by default. Durable knowledge is a distilled, content-hashed record with provenance; no model-generated summary replaces its sources or the authoritative domain record it describes.

Memory class and temperature are independent dimensions:

| Class | Meaning | Typical use | Usual temperature |
| --- | --- | --- | --- |
| `working` | Current context needed by a logical scope | Objective, blockers, active constraints | HOT |
| `canonical` | Durable, accepted knowledge | Decisions, facts, contracts, procedures, lessons | HOT when active; otherwise WARM or COLD if superseded/historical |
| `episodic` | Reusable historical experience | Incidents, migrations, outcomes, failure patterns | WARM; HOT when strongly relevant; COLD after inactivity |
| `evidence` | What happened and its supporting artifacts | Run events, check results, diffs, bounded logs | COLD |
| `aggregate` | Compact statistics derived from multiple observations | Workflow and memory-policy outcomes | WARM |

Temperature is retrieval policy, not storage class. `hot` is in the small activation set for a scope; `warm` is selectively retrieved; `cold` is excluded from automatic context and opened for provenance, debugging, audit, or evaluation. A temperature transition updates the same stable record; it does not duplicate or erase it. Classes describe the information and are not inferred from temperature.

Existing domain entities remain authoritative. Memory metadata may point at an objective, decision, workflow version, run, check, or artifact, but must not mirror that entity's mutable state or create a competing copy of its meaning. A distilled canonical or episodic record is a separate knowledge claim and must link to the evidence and events that support it. Current project/Git state stays in its domain tables; exact state is read from SQLite, not semantic search.

Memory records need stable IDs and content hashes; class, temperature, kind, and scope; created/updated times; optional validity bounds; supersession links; importance, evidence confidence, reuse count, and last-use time; freshness bases such as repository commit, contract hash, dependency snapshot, or graph snapshot; and source-event and artifact references. Confidence represents strength/quality of evidence, not a model's confidence score. Provenance may point to source files and commits, test results, accepted decisions, contracts, Graphify snapshots, runs, artifacts, command results, and human instructions. Canonical memory requires provenance. Changing a fact creates a successor or explicit new version; old facts remain queryable historically and are never silently overwritten. Current retrieval favors valid, non-superseded, fresh facts. A freshness mismatch invalidates only records depending on that basis. An episodic record may move WARM → HOT when strongly relevant and later return to WARM or COLD without changing its class or identity.

Raw experiences and evidence may have temporary retention. Evidence can be protected when it explains a major decision, security incident, serious regression, rare failure, human correction, or benchmark. Deletion is reference-aware and deterministic: eligible evidence must have no live memory, decision, investigation, QA, or protection reference, and its useful extraction/aggregate must already be preserved. Retention is configurable and evidence-driven; do not hardcode arbitrary age cutoffs or let an LLM delete records.

### Bounded activation and provenance-preserving transfer

Each logical context scope has a configurable HOT-memory budget. Main Overseer, project/scope, and worker/task packets receive only their relevant state. When a budget is exceeded, deterministic policy first demotes inactive knowledge from HOT to WARM, consolidates or replaces detail with references when justified, and may route transient detail to evidence retention. Overflow never triggers arbitrary deletion of durable knowledge.

Known-by state is keyed by logical receiver scope and the memory content hash. If the same still-valid version is already known, omit it; if a reminder is useful, send a compact reference; if it changed, send a delta; if invalidated, replace it. Every Cathulu-controlled transfer records its run, sender and receiver scope, memory ID/hash, class and temperature, byte size, observable token count, retrieval method, whether it is new or reintroduced, timestamp, and downstream use when observable. Do not store the transferred prompt body merely to measure context.

Resumption starts from a compact scope snapshot, then checks Git, Graphify, dependency, and contract deltas, invalidates stale knowledge, promotes relevant WARM memories, and sends only changed/relevant context. Transcript replay is not the resumption mechanism. Context-transfer records and memory metadata are the shared foundation; telemetry must populate that ledger rather than introduce a second transfer system.

### Retrieval and maintenance boundaries

Retrieval specializes by source: current exact state from SQLite; named knowledge and errors from SQL/FTS; code structure from Graphify; historically similar experience from optional semantic retrieval; large evidence through artifact references. After identifying the task scope, load its bounded HOT packet, query exact current state and canonical constraints, then add relevant WARM knowledge from SQL/FTS, Graphify, or optional semantic candidates. Deduplicate, filter already-known content, validate freshness and supersession, fit the result to the scope budget, and record controlled transfers. COLD evidence is never injected automatically. Semantic search, if later justified, returns candidate IDs only; every result resolves to an authoritative SQLite record. Limit semantic candidates primarily to WARM canonical and episodic knowledge. Do not embed raw telemetry, transcripts, full logs, exact current state, dependency edges, or whole Graphify snapshots by default.

The future local **Memory Gardener** is deterministic maintenance logic, not a continuously running LLM agent. It may run manually, at startup, periodically, after a run threshold, or when storage thresholds are reached. It may identify duplicates, stale summaries, supersession, consolidation candidates, missing provenance, orphaned artifacts, index entries to remove, and memory-health issues. Any model assistance proposes classifications or summaries; validated evidence and explicit policy control promotion, demotion, protection, and deletion. The foundation milestone implements the records and invariants first; automatic consolidation, memory-assisted semantic indexing, compaction, adaptive temperature policy, and garbage collection remain later work.

Telemetry compaction follows evidence utility rather than a fixed age rule: recent runs may retain bounded full events, older runs can retain a summary and anomalies, and long-term history can retain workflow aggregates, significant run summaries, and protected failures. Before removing raw evidence, verify that useful knowledge, aggregate updates, supporting provenance, and unusual anomalies have been preserved. Duplicate observations are easier to compact than unique explanations for important decisions.

Memory-health and self-improvement evidence can include HOT/WARM/COLD size; class counts; duplicate, stale, and orphan rates; per-scope HOT size; reuse and promotion counts; consolidation and temperature-transition rates; retrieval precision/usefulness; context reintroduction/amplification; and bytes compacted/collected. Track SQLite, artifact, and optional index size and growth by run/project/workflow, plus raw-evidence-to-canonical ratio. Version memory policies and compare them on verified success, missing-context failures, regressions, rework, context volume, latency, and human intervention. These are local optimization measures; the dashboard need show only actionable health summaries.

## Codex integration and feasibility evidence

Current official Codex documentation describes local MCP server configuration in the desktop app and through `~/.codex/config.toml`, plus Codex hook events including session, tool, and subagent lifecycle events. Current configuration documentation also describes OpenTelemetry exporters. The Cathulu app launch and local MCP stdio contract have passed isolated checks; Codex app registration/restart and hook behavior still need a runtime check when the owner connects the server.

- [Codex MCP configuration](https://learn.chatgpt.com/docs/extend/mcp) documents local STDIO servers and desktop configuration.
- [Codex hooks](https://learn.chatgpt.com/docs/hooks) documents local lifecycle/tool hooks and their trust requirements. Hooks are telemetry/context mechanisms, not a complete security or audit boundary.
- [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) documents OpenTelemetry settings and that raw prompts require explicit opt-in. Cathulu leaves raw prompt export disabled and filters/sanitizes event payloads.
- [Local MCP in desktop plugins](https://developers.openai.com/plugins/deploy/desktop-only) is an alternative packaging path if settings-based launch is insufficient.

Telemetry adapters are optional and degrade to manual semantic MCP event recording if an emitter is unavailable. The app does not depend on an OpenAI API key or cloud service for Cathulu persistence. Codex's own model/provider behavior remains outside Cathulu's service boundary.

## Security and privacy baseline

### Protected assets and trust boundaries

- Local repository paths and their Git metadata.
- Durable decisions, objectives, memory, event history, run outcomes, provenance, and quality artifacts.
- Model/tool telemetry that may contain untrusted repository text or sensitive command arguments.
- Codex's approval, sandbox, and permission decisions, which remain owned by Codex.

Repository files and telemetry inputs are untrusted. MCP input is validated and mapped to domain operations. Registered paths are canonicalized and restricted to the explicit registry. No generic SQL operation is exposed. No unnecessary TCP listener is opened; use stdio/local IPC and loopback-only telemetry when a supported exporter requires it. Never treat hooks as an authorization boundary. Preserve Codex's sandbox and approval mechanisms for consequential operations.

Do not persist secrets, raw prompts, or full agent transcripts by default. Store provenance, event type, identifiers, bounded/sanitized metadata, content hashes, and artifact references. Treat any future prompt retention as a separate privacy decision. Keep all memory, artifacts, telemetry, indexes, and policy experiments local. Use OS-local storage permissions, database transactions, idempotent state transitions, and startup reconciliation for recovery. Protect unique provenance conservatively and compact redundant representations only after useful knowledge has been preserved.

## Dashboard interaction contract

The dashboard uses the Operate mode: scan current portfolio state, identify blockers, and inspect evidence. It has four planned areas: Overview, Activity, Project/Scope, and Evidence. Empty, stale, failed, and inconclusive states must be explicit. Keyboard navigation, visible focus, semantic structure, useful zoom, contrast, and narrow-window behavior are part of later UI validation. It has no chat input, approvals, orchestration controls, or model selector.

The owner decided that Cathulu will host future model and reasoning-effort selectors for the Main Overseer and delegated agents. The selector UI is deferred beyond M1 and stays separate from the read-only dashboard. The mechanism for applying selections to Codex, the available option catalog, and per-role configuration support remain unresolved. Verify those integration details before implementing selectors; record effective values when supported telemetry exposes them. See [ADR-0002](decisions/ADR-0002-cathulu-hosted-agent-selectors.md).

## Development Skills & Tooling Profile

Scope: local Windows-first desktop product, Rust core, React/TypeScript UI, local SQLite and Codex integration.
Last checked: 2026-10-20.
Inspected locations: session skill catalog; `C:\Users\sjuan\.codex\skills`; `C:\Users\sjuan\.codex\skills\.system`; official OpenAI Codex docs; empty local checkout and empty GitHub repository.

| Capability / project reason | Selected skill/tool and exact source | Availability / check | Role and trigger | Expected evidence | Fallback / gap |
| --- | --- | --- | --- | --- | --- |
| Project definition | `project-kickoff`, `C:\Users\sjuan\.codex\skills\project-kickoff\SKILL.md` | AVAILABLE; read and applied 2026-10-20 | Required before implementation; recheck on definition changes | Canonical docs, claims, readiness gate | None |
| Small maintainable architecture and implementation | `ponytail-balanced`, `C:\Users\sjuan\.codex\skills\ponytail-balanced\SKILL.md` | AVAILABLE; read and applied to initial boundary choices | Required for architecture and production implementation | Dependency/abstraction review and scoped maintainable changes | Follow current repo conventions if they emerge |
| Product UX research | `ui-ux-research`, `C:\Users\sjuan\.codex\skills\ui-ux-research\SKILL.md` | AVAILABLE; read and applied to user/surface boundaries | Required in planning for dashboard information architecture, states, and accessibility | Interaction contract and UX validation criteria | Use the brief's four-screen/read-only boundary if no external pattern improves it |
| Product UI craft | `impeccable` v4.5.0, `C:\Users\sjuan\.codex\skills\impeccable\SKILL.md` | AVAILABLE; full instructions read; not run during documentation-only definition | Primary UI craft for new Operate surface; load project context before UI planning/editing | PRODUCT/DESIGN or equivalent product UI contract; accessibility and responsive checks | No second visual foundation unless evidence requires it |
| Current library/framework/API/CLI docs | Context7 `resolve-library-id` then `query-docs` (`mcp__context7`) | BLOCKED for this turn: all four resolve requests returned “Monthly quota reached” on 2026-10-20 | Required before library-specific setup or API implementation | Current primary docs for the exact Tauri/React/Rust MCP/SQLite usage | For this plan, official Tauri, React, Model Context Protocol Rust SDK, and rusqlite docs were fetched directly; retry Context7 before future library-specific work |
| Codex product and local integration docs | `openai-docs`, `C:\Users\sjuan\.codex\skills\.system\openai-docs\SKILL.md` | AVAILABLE; read and applied; current docs reviewed 2026-10-20 | Required when implementing or changing Codex MCP, hooks, telemetry, model selectors, or settings | Official OpenAI documentation links and local runtime proof | Recheck official docs; treat unsupported signals as unavailable |
| Significant milestone planning | `implementation-plan`, `C:\Users\sjuan\.codex\skills\implementation-plan\SKILL.md` | AVAILABLE; read | Required after definition gate PASS and before the vertical slice | `docs/plans/PLAN-001-phase1-first-slice.md` with acceptance criteria and test seams | Do not start production implementation without a ready plan |
| Milestone implementation/validation | `milestone-delivery`, `C:\Users\sjuan\.codex\skills\milestone-delivery\SKILL.md` | AVAILABLE; read | Required for each planned milestone | Passing local checks, updated docs/plan, coherent Git checkpoint | Keep incomplete work safely branched and report any exact blocker |
| Release assessment | `release-readiness`, `C:\Users\sjuan\.codex\skills\release-readiness\SKILL.md` | AVAILABLE; read | Required before meaningful public release or user rollout | `docs/readiness/production-readiness.md` with evidence | No release claim from milestone validation alone |
| Graph-based codebase analysis | `graphify` catalog skill | AVAILABLE, but not selected for this kickoff | Reconsider only when a substantial codebase or cross-file relationships exist | Freshness/coverage and source-verified graph claims | Focused source tracing; current repo is empty |
| Architecture diagrams | `Archify` | NOT_FOUND in session catalog | Optional only if a diagram materially clarifies a later design | Source-backed diagram and validation | Text/Mermaid in docs if sufficient |
| Test-first workflow | `tdd`, `C:\Users\sjuan\.codex\skills\tdd\SKILL.md` | ACTIVE for M1; owner approved the public seams | The brief requires integration tests and the owner confirmed the M1 seams | Tests at MCP, Tauri snapshot/dashboard, and SQLite restart boundaries | Keep tests at those boundaries and record any environment-blocked validation |

The project brief's required tests are part of the project requirements. The exact seams and commands belong in the implementation plan and are reviewed before implementation.

## Feasibility risks and next checks

| Risk | Current evidence | Phase affected | Action |
| --- | --- | --- | --- |
| Current Codex MCP/hook/OTel behavior may vary by Windows desktop release and configuration. | Official docs show local MCP, hooks, and OpenTelemetry config. | First increment and Phase 1 telemetry | Exercise local launch/restart and a minimal event path before depending on telemetry. |
| Git reconciliation may encounter unavailable, unsafe, or invalid repository paths. | Git is selected as source of truth; no implementation exists. | First vertical slice | Validate canonical paths and repository identity; return explicit per-project failures without corrupting state. |
| Cathulu's future model/effort selectors may not be able to apply separate values to the Main Overseer and sub-agents. | Cathulu owns the future selector surface; the Codex configuration pathway and role-level support remain unresolved. | Delegation phase | Verify supported Codex configuration and per-role behavior before implementing selectors. |
| Evaluation thresholds lack a baseline. | The brief defines metrics and an experiment question but no measured baseline. | Context experiment | Collect comparable plain-Codex and Cathulu-assisted runs before setting numeric thresholds. |
