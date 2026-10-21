# Cathulu architecture and development profile

Status: product boundaries are NORMATIVE. M1-M6 are published on Windows; M7 is implemented and locally validated on its milestone branch, with publication pending. The live owner Codex MCP connection remains an owner-controlled follow-up.

## System boundary

```text
Owner
  ↕ conversations, commands, approvals
Codex app / Main Overseer
  ↕ local MCP (domain reads and state transitions)
Cathulu Core ───── local SQLite database
  ├─ project, objective, task, and run state
  ├─ structured delegation packets and lifecycle evidence
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

The M1 implementation uses Tauri 2, React/TypeScript, `rusqlite` with bundled SQLite, RMCP over stdio, and Git CLI reconciliation. Exact resolved dependency versions are recorded in the Rust and npm lockfiles. The desktop host and MCP process share one SQLite database path when configured with the same data-directory environment; the dashboard reads snapshots through a Tauri command and polls every five seconds. The dashboard remains read-only. Windows/MSVC builds embed the Common Controls v6 manifest in app and test binaries so the Tauri mock IPC test harness can load. This follows the [Tauri Windows test-manifest example](https://github.com/tauri-apps/tauri/blob/dev/examples/api/src-tauri/build.rs). A later, separate settings surface will own model/effort selection.

M2 adds an additive SQLite v2 foundation alongside the original M1 project table. The shared schema covers project dependencies, objectives/tasks, immutable decisions and workflow versions, runs and quality checks, evidence artifact references, append-only domain events, memory metadata/provenance, known-by state, context transfers, and per-scope HOT budgets. M4 adds selective SQL/FTS retrieval, the manual proposal-first Gardener, explicit consolidation, and configurable reference-aware artifact-reference collection. M5 validates the shared MCP/Tauri store and restart behavior through a disposable Windows integration smoke.

M1-M5 implementation and Windows release-build validation is recorded in [PLAN-001](plans/PLAN-001-phase1-first-slice.md). Git commit freshness currently invalidates only memories tied to the changed project; Graphify, contract, and dependency freshness inputs remain stored metadata without automatic invalidation until their snapshot sources are implemented.

## Data and transaction model

SQLite is the source of truth. Use relational columns for projects, dependencies, objectives, tasks, decisions, workflow identities/versions, runs, quality checks, delegation identity/lifecycle, worker reports and review history, memory lifecycle metadata, and state needed for filtering. Use bounded JSON for typed delegation packets, worker reports, and heterogeneous event payloads. Each important transition appends its domain event and updates the corresponding current-state projection in one transaction. Decisions, workflow versions, delegation identity/packet content, report bodies, and delegation reference links are immutable; report reviews are append-only. Only a delegation's validated lifecycle status may change. Artifacts are stored as local references and hashes where possible, not copied into packets.

The initial data path is an OS-local application data directory. SQLite is configured for transactional writes and WAL. Startup runs additive database migrations and rejects databases newer than the app. MCP and desktop startup both attempt to reconcile every registered repository; the explicit `reconcile_project` tool refreshes one project. Git HEAD changes deterministically invalidate only memory records whose freshness basis names that project. The memory layer stores distilled knowledge and source references; it does not duplicate current project, decision, workflow, run, or artifact state.

### Codex-native delegation records

M6 adds an additive v5 delegation schema that links each record to one project, task, and matching running or blocked run. The typed v1 task packet is limited to 16 KiB and contains an objective, acceptance criteria, constraints, repository-relative allowed paths, current memory/artifact IDs, and requested report fields. Packet content is normalized before hashing. Memory references must be accepted, current, and visible to the same project/task scope. Artifact references must be available and associated with the parent run or unscoped; foreign-key links keep those artifacts from being collected as orphaned evidence.

The stable delegation ID and packet identity persist in SQLite. Idempotency keys and optional external correlation values are SHA-256 hashed; their raw values are not stored. Requested model and effort are stored separately, and effective values remain absent until a trusted source exists. `prepared`, `dispatched`, and `cancelled` are audit states; dispatch records an owner action through native Codex and does not prove that a worker started or completed. Status transitions append events in the existing journal, and the parent run's quality state is unchanged. MCP exposes semantic prepare/transition tools and `cathulu://delegations/{id}` only; it does not spawn agents, run tools, manage worktrees, or write Codex configuration.

M7 adds an additive v6 schema with one immutable, schema-v1 worker report (maximum 16 KiB) per dispatched delegation. Reports contain a summary, one of `completed`/`partial`/`blocked`/`failed`, changed repository-relative paths, commit IDs, worker check claims, blockers, and parent-run artifact/quality-check references. Changed paths must match the packet's allowed scope. Artifact references must be available and attached to the parent run; quality-check references must belong to that run and retain an immutable status/details-hash snapshot. Report and review submissions are idempotent, and only hashes of idempotency keys are stored.

Review decisions are append-only and produce `reported`, `accepted`, `rejected`, or `needs_follow_up` as the current review projection. Worker claims and review status do not change parent-run state or substitute for its mandatory quality checks. Report artifact links protect evidence from orphan collection. The MCP resource includes report and review history; the read-only dashboard shows the ten most recently active delegation records with the parent-run status and explicitly labels worker outcomes as claims.

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

Memory records need stable IDs and content hashes; class, temperature, kind, and scope; created/updated times; optional validity bounds; supersession links; importance, evidence confidence, reuse count, and last-use time; freshness bases such as repository commit, contract hash, dependency snapshot, or graph snapshot; and source-event and artifact references. Global scopes and project/objective/task scopes use stable domain IDs. A future logical agent scope can use a stable opaque scope ID without creating another project/decision/run authority. Confidence represents strength/quality of evidence, not a model's confidence score. Provenance may point to source files and commits, test results, accepted decisions, contracts, Graphify snapshots, runs, artifacts, command results, and human instructions. Canonical memory requires provenance. Changing a fact creates a successor or explicit new version; old facts remain queryable historically and are never silently overwritten. Current retrieval favors valid, non-superseded, fresh facts. A freshness mismatch invalidates only records depending on that basis. An episodic record may move WARM → HOT when strongly relevant and later return to WARM or COLD without changing its class or identity.

Raw experiences and evidence may have temporary retention. Evidence can be protected when it explains a major decision, security incident, serious regression, rare failure, human correction, or benchmark. Collection is reference-aware and deterministic: evidence must be past its configured minimum age, have a valid extraction receipt when required, and have no direct memory, QA, investigation-hold, protection, or legacy event reference. Retention is disabled until an owner configures an interval. The local maintenance CLI requires confirmation of the exact previewed artifact IDs and archives only Cathulu's URI metadata; it preserves stable IDs and hashes and never deletes external files. Existing immutable event rows that contain raw artifact URIs block collection.

### Bounded activation and provenance-preserving transfer

Each logical context scope has a configurable HOT-memory budget. Main Overseer, project/scope, and worker/task packets receive only their relevant state. When a budget is exceeded, deterministic policy first demotes inactive knowledge from HOT to WARM, consolidates or replaces detail with references when justified, and may route transient detail to evidence retention. Overflow never triggers arbitrary deletion of durable knowledge.

Known-by state is keyed by logical receiver scope and the memory content hash. If the same still-valid version is already known, omit it; if a reminder is useful, send a compact reference; if it changed, send a delta; if invalidated, replace it. Every Cathulu-controlled transfer records its run, sender and receiver scope, memory ID/hash, class and temperature, byte size, observable token count, retrieval method, whether it is new or reintroduced, timestamp, and downstream use when observable. Do not store the transferred prompt body merely to measure context.

Resumption starts from a compact scope snapshot, then checks Git, Graphify, dependency, and contract deltas, invalidates stale knowledge, promotes relevant WARM memories, and sends only changed/relevant context. Transcript replay is not the resumption mechanism. Context-transfer records and memory metadata are the shared foundation; telemetry must populate that ledger rather than introduce a second transfer system.

### Retrieval and maintenance boundaries

Retrieval specializes by source: current exact state from SQLite; named knowledge and errors from SQL/FTS; code structure from Graphify; historically similar experience from optional semantic retrieval; large evidence through artifact references. After identifying the task scope, load its bounded HOT packet, query exact current state and canonical constraints, then add relevant WARM knowledge from SQL/FTS, Graphify, or optional semantic candidates. Deduplicate, filter already-known content, validate freshness and supersession, fit the result to the scope budget, and record controlled transfers. COLD evidence is never injected automatically. Semantic search, if later justified, returns candidate IDs only; every result resolves to an authoritative SQLite record. Limit semantic candidates primarily to WARM canonical and episodic knowledge. Do not embed raw telemetry, transcripts, full logs, exact current state, dependency edges, or whole Graphify snapshots by default.

The local **Memory Gardener** is deterministic maintenance logic, not a continuously running LLM agent. M4 exposes an explicit manual run that records proposals for inactive HOT demotion, stale-memory demotion, supported episodic promotion, duplicate episodic/evidence memories, duplicate artifact groups, and orphan evidence. Proposals retain their reasons and source IDs; accepted promotion/demotion changes the existing memory row and appends an event. Repeated episodes can be consolidated through a separate explicit operation that creates a reviewable canonical candidate and retains source memory and artifact links. The Gardener never deletes evidence. It suppresses duplicate pending proposals, and safe class/temperature changes are revalidated when reviewed.

Telemetry compaction follows evidence utility rather than a fixed age rule: recent runs may retain bounded full events, older runs can retain a summary and anomalies, and long-term history can retain workflow aggregates, significant run summaries, and protected failures. Before removing raw evidence, verify that useful knowledge, aggregate updates, supporting provenance, and unusual anomalies have been preserved. Duplicate observations are easier to compact than unique explanations for important decisions.

M4 records immutable memory-policy versions and evaluation rows keyed by experiment, task type, and completed Cathulu run. Comparisons report verified outcomes, context bytes, reintroduced bytes, retrieval usefulness, latency, regressions, rework, and human intervention. Storage snapshots are captured after run completion and associate SQLite/WAL size, growth, memory temperature bytes, artifact-reference metadata, duplicate groups, orphan evidence, and logical URI bytes archived with the run/project/workflow. The dashboard shows only compact health signals: HOT/WARM/COLD bytes, HOT scope count and largest scope HOT size, stale and duplicate counts, reuse, transfer reintroduction and amplification, retrieval usefulness, pending proposals, SQLite size/growth, orphan evidence, and logical bytes collected. These sizes describe Cathulu's database and URI metadata, not the size of external files referenced by artifacts.

No semantic index or Graphify invalidation is implemented: the available SQL/FTS retrieval has not yet shown a measured gap, and Cathulu has no Graphify snapshot source. SQLite remains authoritative. M4 does not compact raw telemetry or adapt policy automatically; it supplies versioned measurements and deterministic review points for later experiments.

## Codex integration and feasibility evidence

Current official Codex documentation describes local MCP servers and lifecycle hooks. On 2026-10-21, the installed Windows CLI reported `codex-cli 0.146.1`; the live owner Codex hook trust/configuration path remains unmodified and has not been exercised. The hook contract was verified against current official documentation and synthetic MCP integration tests. See [ADR-0004](decisions/ADR-0004-local-codex-hook-telemetry.md) and the optional [hook setup](local-mcp.md#3-optional-local-codex-activity-hooks).

- [Codex MCP configuration](https://learn.chatgpt.com/docs/extend/mcp) documents local STDIO servers and desktop configuration.
- [Codex hooks](https://learn.chatgpt.com/docs/hooks) documents MCP tool hooks, their reviewed/trusted configuration, shared session/turn fields, and tool lifecycle payloads. Cathulu uses only `UserPromptSubmit`, `PostToolUse`, `Stop`, and `Interrupt`; the MCP schema and optional hook snippet omit prompt, transcript, tool-input, and tool-response fields.
- [Codex monitoring and telemetry](https://learn.chatgpt.com/docs/agent-approvals-security#monitoring-and-telemetry) documents opt-in OpenTelemetry and event categories. Cathulu does not enable OTel because tool-result events may contain output snippets; an external collector is not required for the local hook path.
- [Local MCP in desktop plugins](https://developers.openai.com/plugins/deploy/desktop-only) is an alternative packaging path if settings-based launch is insufficient.

The `record_codex_hook_event` operation validates event names, hashes session/turn/tool-use/source-event identifiers, maps `cwd` to an already registered Git root without persisting the directory string, and reduces tool names to a coarse category. A session/turn maps to one existing run; stop/interruption is recorded as inconclusive and cannot establish verified success. Repeated events are deduplicated in the append-only event journal. Missing MCP connections, unsupported hook events, invalid directories, and unregistered projects do not create partial state. Context-transfer records continue to come from the existing explicit `prepare_context_transfer` operation; Codex hooks do not expose transfer bodies or token counts to Cathulu.

Desktop and MCP process startup both attempt Git reconciliation for every registered project. A failure for one path is reported to stderr and does not block other project refreshes or startup. The app does not depend on an OpenAI API key or cloud service for Cathulu persistence. Codex's own model/provider behavior remains outside Cathulu's service boundary.

## Security and privacy baseline

### Protected assets and trust boundaries

- Local repository paths and their Git metadata.
- Durable decisions, objectives, memory, event history, run outcomes, provenance, and quality artifacts.
- Model/tool telemetry that may contain untrusted repository text or sensitive command arguments.
- Codex's approval, sandbox, and permission decisions, which remain owned by Codex.

Repository files and telemetry inputs are untrusted. MCP input is validated and mapped to domain operations. Registered paths are canonicalized and restricted to the explicit registry. No generic SQL operation is exposed. No unnecessary TCP listener is opened. Never treat hooks as an authorization boundary or as proof that a tool succeeded; preserve Codex's sandbox and approval mechanisms for consequential operations.

Do not persist secrets, raw prompts, or full agent transcripts by default. Store structured task fields, provenance links, event type, hashed identifiers, bounded metadata, content hashes, and artifact references. Treat any future prompt or transcript retention as a separate privacy decision. Keep all memory, artifacts, telemetry, indexes, and policy experiments local. Use OS-local storage permissions, database transactions, idempotent state transitions, and startup reconciliation for recovery. Protect unique provenance conservatively and compact redundant representations only after useful knowledge has been preserved.

## Dashboard interaction contract

The dashboard uses the Operate mode: scan current portfolio state, identify blockers, and inspect evidence. The current read-only surface shows registered repositories and Git state, active objectives and blocked tasks, recent workflow runs, failed/inconclusive checks, compact HOT/WARM/COLD memory sizes, HOT scope count, largest per-scope HOT size, other aggregate health counters, and delegation/report summaries. Delegation reports are clearly marked as worker claims, separate from review status and parent-run verification. Empty, stale, failed, and inconclusive states must be explicit. Keyboard navigation, visible focus, semantic structure, useful zoom, contrast, and narrow-window behavior remain part of UI validation. The dashboard has no chat input, approvals, orchestration controls, or model selector.

The owner decided that Cathulu will host future model and reasoning-effort selectors for the Main Overseer and delegated agents. Current official Codex documentation supports main-session model/effort settings, global sub-agent defaults, explicit spawn overrides, and custom agent files with separate per-role values. This confirms the configuration pathway and per-role capability, but not a live-session control API or an account-specific option catalog. The selector UI stays separate from the read-only dashboard, requires an owner-approved preview/apply flow, and must not silently edit user-level config or claim a stored preference is active. See [ADR-0002](decisions/ADR-0002-cathulu-hosted-agent-selectors.md) and [PLAN-002](plans/PLAN-002-phase3-codex-delegation.md).

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
| Cathulu's future model/effort selectors must match the owner's active Codex configuration layer and account-specific catalog. | Official docs confirm main-session settings, global subagent defaults, explicit spawn overrides, and per-role custom-agent files; current-turn control and an account-specific catalog are not established. | Phase 3 M9 | Agree on preview/apply target with the owner; label requested/configured/effective states accurately and never edit global config silently. |
| Evaluation thresholds lack a baseline. | The brief defines metrics and an experiment question but no measured baseline. | Context experiment | Collect comparable plain-Codex and Cathulu-assisted runs before setting numeric thresholds. |
