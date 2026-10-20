# PLAN-001 — Phase 1 first vertical slice

Status: M1-M4 IMPLEMENTED, VALIDATED, AND PUBLISHED; M4 checkpoint `185741d` is on `main`.
Created: 2026-10-20
Last updated: 2026-10-21

## Objective

Create the first usable Cathulu vertical slice on Windows: register a local repository through Codex-facing MCP, persist it in SQLite, reconcile its Git state, and show that state in a read-only Tauri desktop view.

## Motivation

The project definition identifies durable project state as the smallest end-to-end proof of Cathulu's role around Codex. This slice exercises the core path without introducing run orchestration, Sub-Overseers, Graphify, vector retrieval, or cloud services.

## Current state

- At kickoff, the local checkout and personal GitHub repository `jmsb505/cathulu` were empty. M1 source was checkpointed on `codex/phase1-foundation`; the validated progress and managed-memory plan are now published on the remote default branch `main` at `5f43dcb`.
- Before M2 implementation, the owner refined Cathulu's memory lifecycle requirements. Architecture and milestone scope now distinguish memory class from temperature, preserve provenance and temporal history, bound HOT context, track known-by state and transfers, and defer automated gardening/indexing/collection.
- M2 implementation includes an additive SQLite v2 migration, workflow and memory domain operations, semantic MCP tools/resources, a shared Tauri dashboard snapshot, and expanded tests. Rust formatting, all seven workspace tests, all three React tests, the frontend build, the Windows release build, and an isolated five-second desktop startup check pass. The implementation checkpoint was pushed to `main` as `a3ca960`; its documentation follow-up is `2c05089`.
- M3 adds the SQLite v3 idempotent event key and hash-only Codex turn/run mapping, a sanitized MCP hook tool, and best-effort Git refresh on desktop/MCP startup. Synthetic integration tests cover privacy, unsupported and unregistered signals, rollback, restart deduplication, and fresh Git state. The validated checkpoint `1d445f6` is published to `main`. The user-controlled Codex hook trust/configuration path remains unmodified and unverified.
- The local dashboard preview was visually inspected. Its read-only label, page layout, local-state error, and visible keyboard focus render correctly; React tests cover empty/error states, active and blocked objectives, verified/blocked/failed runs, failed/inconclusive checks, and memory-health counters.
- The local toolchain reports Rust/Cargo 1.91.1, Node.js 24.11.1, npm 10.8.3, and Git available.
- Current official documentation supports Tauri 2 projects with React, React TypeScript components, the official `rmcp` Rust MCP SDK with stdio transport, and rusqlite transactions.
- Context7 resolution calls failed because the current monthly quota is exhausted. The source links listed below are the official vendor/project docs fallback. Retry Context7 before future library-specific implementation or API changes.
- At implementation start, the remote `main` was unborn. On 2026-10-21, after local validation and confirmation that the repository had no branch rulesets, the owner requested publication; `5f43dcb` was pushed without force and GitHub now recognizes `main` as the default branch.
- The owner approved the MCP round-trip, Tauri snapshot/dashboard, and SQLite restart persistence test seams; implementation and test coverage for those boundaries have been authored.
- Rust and npm dependencies were fetched after the package registries became available. The frontend uses React 19.3, TypeScript 5.9, Vite 8.3, and Vitest 5; the lockfiles pin resolved versions.
- The official Visual Studio Build Tools 2022 v17.14.42, MSVC x64/x86 14.44.35207, Windows SDK 10.0.26100.0, and `link.exe` are installed. Use an x64 Visual Studio developer shell for native Rust builds.
- M1 validation passed four Rust tests. M2 validation passes seven Rust tests: one core unit test, five MCP/domain integration tests, and one Tauri IPC command test. The React dashboard suite passes all three tests.
- `cargo tauri build --no-bundle -- --offline` builds `target/release/cathulu-desktop.exe`; the app was started with isolated temporary data directories, remained running for five seconds, and created its SQLite database.
- Tauri test binaries on Windows need the Common Controls v6 application manifest. Without it, the unit-test executable exits before the harness with `STATUS_ENTRYPOINT_NOT_FOUND`; `apps/desktop/src-tauri/build.rs` embeds the manifest for Windows/MSVC targets.

## Desired state

- A Windows-first Tauri 2 + React/TypeScript desktop shell backed by Rust.
- A Rust core library owning project validation, SQLite persistence, and Git reconciliation.
- A local stdio MCP executable reusing that core and exposing semantic project registration/reconciliation plus compact read resources.
- A read-only desktop view that reads the same durable project snapshot through a narrow Tauri command boundary.
- A shared SQLite memory substrate that references existing domain records instead of duplicating their authority, and can later support bounded scope activation and selective retrieval.
- A local setup note for connecting the MCP executable to Codex. The implementation must not edit the owner's global Codex configuration automatically.

## Scope

- Establish the minimal buildable application structure and Windows development commands.
- Add the first SQLite migration and local database initialization.
- Register a local Git repository idempotently by canonical path; reject invalid/non-repository paths without partial state.
- Reconcile branch, HEAD, dirty state, and changed paths using Git as the source of truth.
- Expose project registration/reconciliation and compact portfolio/project reads over local MCP.
- Render project name/path and current Git state in a read-only desktop view.
- Document local MCP setup and the Phase 1 implementation boundary.
- Add automated integration coverage at the owner-approved public seams listed below.

## Out of scope

- Objectives, tasks, project dependency relationships, decisions, workflow versions, runs, event journal, QA promotion, telemetry ingestion, or evidence-history views; these are later Phase 1 milestones.
- Background filesystem watchers; the first slice reconciles on registration and an explicit Codex-facing reconcile operation.
- Model/effort selectors, workers, Sub-Overseers, Graphify, embeddings, adaptive workflows, cloud services, and distributed infrastructure. Cathulu-hosted selectors are accepted for a later milestone; their Codex configuration pathway and per-role support remain to be verified.
- Automatically writing user-level Codex configuration or launching Codex on the user's behalf.

## Source-of-truth impact

- `docs/project-definition.md`: first slice scope and success criteria.
- `docs/architecture.md`: initial component/data boundaries and verified integration constraints.
- `docs/roadmap.md`: progress status for the first vertical slice only.
- `docs/readiness/definition-gate.md`: retain the definition PASS; milestone acceptance is tracked here separately.
- Root `README.md`: app overview and local setup entry point.
- `docs/local-mcp.md`: user-controlled Codex MCP setup and runtime limitations.
- `docs/decisions/ADR-0003-managed-local-memory-lifecycle.md`: accepted memory lifecycle, authority, retention, and staged implementation constraints.

## Existing decisions and constraints

- [ADR-0001](../decisions/ADR-0001-cathulu-is-the-local-evidence-layer.md): Codex owns interaction and agent execution; Cathulu owns local state/evidence; dashboard is read-only.
- [ADR-0002](../decisions/ADR-0002-cathulu-hosted-agent-selectors.md): Cathulu will host future agent model/effort selectors in a separate settings surface; the Codex integration remains unverified.
- [ADR-0003](../decisions/ADR-0003-managed-local-memory-lifecycle.md): memory is a provenance-preserving lifecycle over the existing SQLite domain and evidence records, with independent class/temperature dimensions and bounded active context.
- The owner selected Windows first, cross-platform later.
- Adopt the brief's preferred Tauri 2 / React / TypeScript / Rust / SQLite stack because no existing implementation exists.
- SQLite is the system of record; MCP never exposes generic SQL.
- Registered repository contents and telemetry are untrusted; no secrets, raw prompts, or full transcripts are retained by default.
- The validated M1 and accepted memory-plan refinement were first published to `origin/main` in `5f43dcb`. The M2 implementation branch was `codex/phase1-workflow-state`; its validated checkpoint is published to `main` in `a3ca960`.

## Supporting skills / tools

- `implementation-plan` for this plan; `milestone-delivery` for M1 implementation and validation.
- `ponytail-balanced` for dependency and abstraction choices.
- `ui-ux-research` plus `impeccable` for the read-only Operate dashboard and accessibility states.
- `tdd` because the brief requires integration tests. The owner approved the public seams below before test implementation.
- Context7 is the preferred docs tool, but its monthly quota was exhausted in this turn. Current primary fallback sources: [Tauri create-project](https://v2.tauri.app/start/create-project/), [React TypeScript](https://react.dev/learn/typescript), [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk), and [rusqlite](https://docs.rs/rusqlite/latest/rusqlite/).
- OpenAI product docs were checked for [local MCP](https://learn.chatgpt.com/docs/extend/mcp) and [Codex hooks](https://learn.chatgpt.com/docs/hooks). Recheck these before configuring runtime behavior.

## Dependencies

- Rust toolchain, Node/npm, Git, Windows WebView2, and Tauri's Windows build prerequisites.
- Rust/Cargo and npm dependencies are locked. Registry access was approved for dependency retrieval; future dependency changes should update the lockfiles.
- Codex's local MCP configuration path for the owner; M1 has a documented/manual fallback if in-app launch cannot be exercised in the sandbox.
- A disposable local Git repository for integration validation; it must not be a user repository.

## Risks and unknowns

- Tauri native build prerequisites or package downloads may be unavailable in the current environment. Check exact prerequisite state before changing the app architecture.
- The `rmcp` SDK and current Codex desktop runtime must interoperate over stdio on Windows. Prove this with a narrow integration check.
- Git paths may be invalid, detached, dirty, or unsafe. Canonicalize and validate paths, return explicit errors, and avoid partial database writes.
- The dashboard must stay read-only even though the MCP server has semantic write operations.
- Codex hook/OTel events and model/effort selection remain unverified runtime details; neither blocks the project-registry slice.
- The memory taxonomy spans several existing/future domain records. M2 must map those records to memory semantics without creating parallel copies of project state, decisions, workflow versions, runs, or evidence. Candidate status is a lifecycle state, not another memory class or temperature.
- A semantic-index interface or vector dependency may be premature before retrieval gaps are measured. SQLite remains authoritative; add an index only in a later milestone if evidence justifies it.

## Owner-approved public test seams

1. **MCP contract seam:** call the local stdio MCP server's `register_project` and `reconcile_project` tools plus `cathulu://projects` / `cathulu://projects/{id}` resources. Use real temporary SQLite and Git fixtures; verify invalid requests do not mutate state.
2. **Desktop read seam:** invoke the actual `project_snapshot` command through Tauri's mock IPC runtime against the shared store; separately test the React dashboard's `invoke` response rendering, including empty and error states. The command remains read-only. A native Windows WebView smoke check is still required.
3. **Persistence/restart seam:** close the MCP process, reopen it against the same temporary database, and read through an MCP resource, confirming durable project state and correct reconciliation.

The owner confirmed these seams. The integration tests observe MCP and dashboard boundaries rather than inspect SQLite tables as their only oracle.

## Milestones

### M1 — Project registry vertical slice

Goal: demonstrate the end-to-end path from a Codex-facing semantic MCP call to durable Git-aware project state and a read-only desktop view.

Subtasks:

- [x] Confirm the public test seams with the owner.
- [x] Create the minimal Tauri 2 React/TypeScript desktop structure.
- [x] Add a Rust core, first schema migration, local SQLite initialization, and project domain/read models.
- [x] Implement canonical repository validation and idempotent project registration.
- [x] Implement Git branch/HEAD/dirty/changed-path snapshots and explicit reconciliation.
- [x] Implement local stdio MCP tools/resources through the official RMCP SDK.
- [x] Implement the read-only dashboard through the `project_snapshot` Tauri command.
- [x] Add automated tests at the approved seams and document manual Windows desktop/MCP smoke coverage.
- [x] Add README and local MCP setup instructions; update source-of-truth docs and plan with current evidence.

Affected areas: `apps/desktop/`, `src/`, `tests/`, `docs/`, root project files.

Dependencies: test seam confirmation; Windows Tauri prerequisites; dependency availability.

Acceptance criteria:

- Windows development build launches the desktop app.
- Registration accepts a valid local Git repository and survives reopening the database.
- Reconciliation reports branch, HEAD, dirty state, and changed paths from Git; duplicate registration of the same canonical path is idempotent.
- A non-repository or inaccessible path returns a clear error and leaves project state unchanged.
- The local MCP server exposes typed semantic operations and compact portfolio/project resources; no generic SQL is available.
- The dashboard shows current projects and Git status and exposes no write control.
- Automated tests cover registration, reconciliation, invalid input, idempotency, MCP read state, and restart persistence at the agreed seams.
- Codex connection setup is documented and user-controlled.

Validation:

- `cargo fmt --all -- --check` (passed in the x64 Visual Studio developer environment)
- `cargo test --offline --workspace` (passed: four tests across core, MCP, and Tauri IPC)
- `npm test` (passed: three dashboard tests)
- `npm run build` (passed through the Tauri build's `beforeBuildCommand`)
- `cargo tauri build --no-bundle -- --offline` (passed; Windows release executable built)
- Isolated app startup smoke (passed: executable stayed running for five seconds and created a SQLite database under `target/m1-smoke`)
- MCP tests cover registration, reconciliation, invalid input, idempotency, resource reads, and persistence after reopening the store. The Tauri mock IPC test and React tests cover the command and dashboard rendering seams.
- A visible native UI inspection and live registration through the owner's Codex app were not performed in this session. Codex configuration remains user-controlled and unchanged; the manual connection instructions are documented.
- Review the diff for secret/prompt logging, unsafe path handling, dashboard writes, and exposed SQL.

Documentation updates:

- Update project definition, architecture, roadmap, README, local MCP setup, and this plan with the final command names, limitations, and validation evidence.
- Record any material architecture change in a new ADR.
- Do not add a changelog for the definition documents; add/update one only if the product's existing release convention is introduced and behavior is shipped.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `ui-ux-research`, `impeccable`, `tdd`, Context7 (retry), `openai-docs` for Codex integration.

Expected Git checkpoint: validated local commit `2ec0f0b` on `codex/phase1-foundation`. Push/PR progression remains a separate GitHub action and must use repository controls; the remote currently has no commit or `main` branch.

### M2 — Durable workflow, quality, and memory foundation

Goal: add the minimum durable records and quality rules for Phase 1 run tracking, plus the memory metadata and deterministic boundaries needed to preserve knowledge without unbounded active context. This is a storage/contract foundation, not automatic memory management.

Subtasks:

- [x] Add an additive SQLite migration for dependencies, objectives, tasks, immutable decisions, immutable workflow versions, runs, checks, and append-only events; retain existing M1 project rows and read behavior.
- [x] Update run state and append its event in one transaction. Enforce that only passing mandatory checks can produce `verified_success`; missing or failed evidence remains blocked, failed, or inconclusive.
- [x] Add stable memory IDs, content hashes, kind/scope, created/updated timestamps, and independent `memory_class` (`working`, `canonical`, `episodic`, `evidence`, `aggregate`) and `temperature` (`hot`, `warm`, `cold`) fields. Do not use temperature as a class or class as a retrieval priority.
- [x] Map memory semantics onto existing domain/evidence records where possible. Add a distilled memory record only for a distinct knowledge claim; link it to its domain sources instead of copying their mutable current state. Candidate/review state is separate from memory class and temperature.
- [x] Add scope identity, bounded HOT-budget metadata, importance, evidence confidence, reuse and last-used metadata. Global, project, objective, and task IDs resolve to existing records; opaque stable `scope` IDs support future Main/Sub-Overseer logical scopes without adding a parallel scope authority.
- [x] Add validity times, explicit supersession links, and freshness bases for repository/commit and other supported snapshots. Preserve superseded facts for history; current reads select valid, fresh, non-superseded knowledge. Start with the freshness inputs actually available and extend the typed basis as dependency/Graphify/contract snapshots are implemented.
- [x] Add provenance links to source events/domain records and artifact references. Canonical memory must have at least one traceable source. Add explicit protected-evidence state for important incidents and decisions.
- [x] Add known-by state keyed by receiver scope and content hash. An unchanged valid version is omitted, a changed version produces a delta/reference, and an invalidated version is replaced; temperature changes do not create a second memory identity.
- [x] Add a context-transfer ledger for Cathulu-controlled transfers, including run, sender/receiver scope, memory ID/hash, class, temperature, size, observable token count, first-time/reintroduced flag, retrieval method, timestamp, and downstream use when observable. Never retain raw prompt bodies just to populate this ledger.
- [x] Add deterministic HOT-budget enforcement with stable ordering by importance and last use; demote eligible inactive items before overflow and never delete durable knowledge to fit a packet. WARM remains selective and COLD is not included in ordinary activation.
- [x] Keep memory state, transfer records, and all evidence references local in the existing SQLite database. Do not add a hosted service, second database, vector package, continuously running LLM agent, or table-CRUD MCP surface.
- [x] Extend semantic MCP reads/writes and the read-only dashboard only where needed to inspect current workflow/quality state and compact scope memory health. Keep model/effort controls in their later separate settings surface.
- [x] Add migrations, transaction, restart, MCP, Tauri read, and memory-invariant tests at the existing public seams.

Affected areas: core schema/domain, MCP contract, dashboard query surface, migrations, tests, and source-of-truth documentation.

Dependencies: M1 validated; public MCP and Tauri read seams remain stable.

Acceptance criteria:

- Existing M1 projects survive migration/reopen unchanged, and all new events/projections use the existing SQLite authority.
- Events and current state update atomically; event rows cannot be edited through model-facing tools.
- Decision and workflow-version history is immutable and superseded by new records.
- No agent claim or timeout can set verified success with a failed/missing mandatory gate.
- A memory's class and temperature can vary independently; HOT/WARM/COLD transitions update one stable identity without duplicating or deleting the record.
- Content-identical durable knowledge is idempotent within the same scope and kind; content hashes change when the distilled body changes.
- Canonical memory has provenance; validity and supersession keep old truth historically queryable while current retrieval favors fresh, valid records.
- A changed Git/freshness basis invalidates only dependent memories; an unrelated project's memories remain valid.
- Known-by filtering does not resend unchanged valid content to a receiver; changed content produces a delta, with the transfer ledger recording the operation.
- HOT records remain within configured per-scope budgets after deterministic demotion. Durable knowledge and its provenance remain available in WARM/COLD state.
- COLD evidence is absent from ordinary activation; protection and reference metadata are queryable. M2 does not implement evidence deletion or garbage collection.
- Context-transfer accounting contains metadata/hashes and does not store raw prompt/transcript content.
- Dashboard shows objectives, blockers, recent runs, failed QA, and compact memory health without write actions.

Validation:

- Rust: additive migration with existing-data preservation; transaction rollback/atomicity; restart/recovery; state transitions and mandatory gates; class/temperature independence; stable identity/hash idempotence; supersession/history/current reads; selective freshness invalidation; known-by delta behavior; transfer-ledger provenance; budget overflow/demotion preserving durable records; COLD exclusion and protected/reference metadata.
- MCP: typed schema/input and permission-boundary tests for semantic domain operations and reads; no generic SQL/table CRUD.
- Tauri mock IPC and React tests for compact memory/workflow health rendering and empty/error states.
- Manual UI preview confirms layout, read-only presentation, local-state error, and visible keyboard focus. React tests verify empty/error states and the active/blocked/failed/inconclusive workflow and memory-health content; stale-memory count and protected-artifact indicators are included.
- Native Windows release executable stayed running for five seconds and created an isolated SQLite database under `target/m2-smoke`.

Documentation updates: update domain/architecture, API/MCP contract notes, roadmap, plan, and decision records with actual schema/API choices and evidence.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `impeccable`; Context7 for current crate/API details.

Expected Git checkpoint: separate validated milestone commit/branch following repository policy.

### M3 — Local telemetry and Phase 1 integration

Goal: ingest available Codex/tool signals locally and make Phase 1 state useful for measuring quality and context reuse.

Subtasks:

- [x] Review current official Codex hook and OpenTelemetry event documentation; the installed Windows CLI reports `codex-cli 0.146.1`. Keep the actual hook trust/configuration owner-controlled; it was not edited or live-tested.
- [x] Normalize only the allowlisted `UserPromptSubmit`, `PostToolUse`, `Stop`, and `Interrupt` metadata. Prompts, transcripts, tool inputs, and tool outputs are omitted; OpenTelemetry export remains disabled.
- [x] Keep the existing M2 context-transfer ledger and explicit `prepare_context_transfer` operation. Hooks do not infer transfer bodies, token counts, or downstream use.
- [x] Keep the existing read-only dashboard's run history, blockers, and failed QA; add coverage for observed inconclusive runs.
- [x] Reconcile registered Git snapshots at desktop/MCP process startup and make hook ingestion idempotent across process restart.

Affected areas: telemetry adapter, event model, database migration, MCP evidence resources, Activity/Evidence views.

Dependencies: M2 run/evidence and memory metadata/transfer model; local Codex runtime experiment.

Acceptance criteria:

- Supported local signals are associated with runs and retained as sanitized evidence.
- Unsupported or unavailable signals do not break MCP, persistence, or the dashboard.
- No raw prompt/transcript is stored by default and no secrets are retained in event payloads.
- Phase 1 Definition of Done is demonstrably met from recorded evidence.

Validation:

- Synthetic local MCP-hook integration tests for allowlisted schemas, raw-content omission, unsupported/unregistered events, event-journal failure rollback, idempotency, and restart recovery.
- Rust workspace and React dashboard tests; Windows release build and isolated desktop startup/database smoke.
- Manual owner-controlled Codex MCP hook trust/configuration smoke remains optional and unverified; no user Codex configuration is changed by Cathulu.

Documentation updates: update telemetry/privacy/security boundaries, roadmap, plan, and readiness evidence.

Applicable specialized skills: `milestone-delivery`, `openai-docs`, `ponytail-balanced`, `tdd`, `impeccable`; Context7 for implementation libraries.

Expected Git checkpoint: validated milestone commit; release-readiness remains separate.

### M4 — Memory lifecycle and context-efficiency experiments (implemented, validated, and published)

Goal: make Cathulu better informed over time while keeping active context and redundant evidence bounded. This milestone builds on measured M1-M3 state; it does not block the Phase 1 memory substrate.

Implementation constraints: keep the deterministic Gardener manually invoked and proposal-first; keep evidence collection out of agent-callable MCP tools and require an explicit local owner confirmation; disable collection by default until a retention interval is configured. Use SQLite FTS only if the bundled Windows SQLite build supports it. Do not add a semantic index without measured retrieval gaps or Graphify invalidation without a real graph snapshot source. Evidence collection may remove only Cathulu's local artifact reference metadata; it must never delete files referenced by artifact URIs.

Subtasks:

- [x] Add selective SQL/FTS retrieval for named canonical and episodic knowledge, with explicit freshness, supersession, known-by, and HOT-budget filtering.
- [x] Introduce the local deterministic Memory Gardener as a manual, proposal-first maintenance job, not a continuously running LLM agent.
- [x] Detect repeated accepted episodic/evidence knowledge; provide explicit consolidation into a reviewable canonical candidate while preserving source memories, provenance, and artifact links. Propose supported promotion and safe demotion without changing stable IDs.
- [x] Add configurable reference-aware evidence retention and collection. Preserve protected evidence and sources needed by canonical memory, decisions, active QA, or holds; require durable extraction receipts by default. Collection archives only Cathulu's URI metadata and never deletes external files.
- [x] Track actionable memory health and storage growth, including class/temperature sizes, duplicate/stale/orphan counts, HOT scope count and largest scope use, reuse, retrieval usefulness, context reintroduction/amplification, and logical URI bytes archived.
- [x] Version memory policies and compare outcomes including verified success, missing-context failures, regressions, rework, context volume, latency, retrieval usefulness, and human intervention.
- [x] Evaluate whether SQL/FTS retrieval justifies a semantic index. No measured gap exists yet, so no semantic index was added; SQLite remains authoritative.
- [ ] Add an index only if future measurements justify it. If added, index primarily WARM canonical/episodic memories, return candidate IDs, revalidate through SQLite, and support rebuild/removal without changing SQLite authority.
- [x] Keep invalidation limited to available repository Git freshness. Defer Graphify, contract, and dependency invalidation until those snapshot sources exist.
- [ ] Defer stale-summary detection, aggregate-memory refresh, and raw telemetry compaction until Cathulu has enough retained run data to validate the policies.

Acceptance criteria:

- Repeated supported observations can consolidate into a provenance-linked canonical review candidate while original episodes and their source references remain queryable.
- Current exact state and current canonical knowledge outrank historical/semantic matches; WARM results are selective and COLD evidence is never automatically injected.
- Superseded and stale knowledge cannot override current facts. Promotion/demotion changes temperature or lifecycle state without duplicating stable identities.
- Evidence collection is deterministic and reference-aware, protects protected/referenced evidence, and archives eligible local URI metadata only after useful knowledge and provenance survive; external files remain untouched.
- No semantic candidates are produced unless measured SQL/FTS retrieval gaps justify an index. If that later condition is met, candidates must resolve to authoritative SQLite records and support index rebuild/removal.
- Memory-health metrics expose excess growth or low knowledge density, including per-scope HOT use, without dumping the evidence archive into active context.
- A matched experiment can compare versioned policies against verified work and context/resource measures.

Validation:

- Tests for repeated-evidence consolidation and duplicate detection, unique-source retention, provenance validation, policy versioning, configurable retention, protected/reference-aware collection, and storage accounting.
- Retrieval tests for HOT preference, exact state precedence, WARM selectivity, COLD exclusion, stale/superseded suppression, and budget fit.
- If a semantic index is implemented later: candidate-ID resolution, stale/deleted candidate filtering, rebuild/remove correctness, and proof SQLite remains authoritative. The condition is not met in M4.
- Manual checks for reviewable Gardener proposals, explainable actions, and no model-controlled evidence deletion.

Dependencies: M2 memory foundation; M3 run/telemetry evidence; measured retrieval and storage need before optional semantic indexing.

Expected Git checkpoint: separate validated milestone commit/branch following repository policy.

## Final integration validation

After M1-M4, install/start the Windows desktop build from a clean local state, register a disposable repository, record and retrieve project/run/QA and memory provenance/transfer state through MCP, verify bounded current context and that the dashboard remains read-only, restart Cathulu, and confirm durable state and Git/freshness reconciliation. Run workspace checks and review privacy, security, and integration limits. Milestone-local M4 validation is complete; this combined Phase 1 integration pass remains outstanding and is not public release approval.

## Rollback / recovery

M1 stores only the project registry and latest Git snapshot. Keep database migrations forward-compatible; before changing schema, create and verify a backup in the test fixture. For local recovery, stop the app/MCP process, preserve the SQLite file, and report corrupt DB state explicitly rather than silently deleting/recreating it. A bad app version can be rolled back while retaining the database; the next version must reconcile the registry from Git.

## Progress

- [x] Definition and readiness gate completed; see [definition gate](../readiness/definition-gate.md).
- [x] Local milestone branch created: `codex/phase1-foundation`.
- [x] Current official fallback docs checked after Context7 quota exhaustion.
- [x] Public test seams confirmed.
- [x] M1 implementation started and implementation/test sources authored.
- [x] M1 automated validation and isolated Windows app-startup smoke complete.
- [x] M1 committed locally as `2ec0f0b` on `codex/phase1-foundation`.
- [x] Managed-memory principles and staged M2/M4 boundaries recorded in architecture, definition, roadmap, plan, and ADR-0003 before M2 implementation.
- [x] On 2026-10-21, re-ran `cargo fmt --all -- --check`, `cargo test --offline --workspace` (4 passed), `npm test` (3 passed), and `cargo tauri build --no-bundle -- --offline`; all passed.
- [x] Published `5f43dcb` to the previously unborn `main` with a normal push; verified the remote SHA and default branch. No GitHub check runs or Actions runs were configured for the commit.
- [x] On 2026-10-21, M2 validation passed: `cargo fmt --all -- --check`, `cargo test --offline --workspace` (7 passed), `npm test` (3 passed), `npm run build`, `cargo tauri build --no-bundle -- --offline`, and isolated Windows startup/database smoke.
- [x] Visually inspected the local dashboard preview and confirmed visible keyboard focus; React tests cover the remaining populated and empty/error states.
- [x] Published M2 commit `a3ca960` to `origin/main` with a normal fast-forward push; verified remote `refs/heads/main` resolves to `a3ca960b9f03efbe25e6824d4d7dd4e0ed85b599`.
- [x] On 2026-10-21, M3 validation passed: `cargo fmt --all -- --check`; 11 Rust tests; 3 React tests; `npm run build`; `cargo tauri build --no-bundle -- --offline`; `git diff --check`; and an isolated five-second Windows desktop startup/database check.
- [x] Published M3 commit `1d445f691bf23b957661d298aa60c0decbf7e20f` to `origin/main` with a normal fast-forward push and verified the remote SHA.
- [x] GitHub reports no commit status checks for the documentation follow-up and zero Actions workflow runs for the repository; local validation is the available gate.
- [x] On 2026-10-21, M4 validation passed: `cargo fmt --all -- --check`; all 17 Rust workspace tests (1 core, 11 MCP/domain, 4 memory lifecycle, 1 Tauri command); all 3 React tests; `npm run build`; `cargo build --offline --release --bin cathulu-maintain`; `cargo tauri build --no-bundle -- --offline`; and `git diff --check`.
- [x] M4 manual local checks passed: the maintenance CLI Gardener and evidence-GC preview ran against a disposable SQLite directory; the Windows desktop remained running for five seconds and created a SQLite database in a unique isolated data directory.
- [x] The M3 base commit has no GitHub status checks or associated workflow runs. The repository owner account has push/admin permission; local M4 checks are the available validation gate.
- [x] Published M4 checkpoint `185741d9cb2d3a223cf0ba5603e1961042a1d608` to `origin/main` with a normal fast-forward push; verified the resulting remote SHA.
- [ ] Owner-controlled Codex MCP/hook trust smoke remains unverified; Cathulu has not changed user Codex configuration.
- [ ] Combined M1-M4 end-to-end integration validation remains outstanding; it is separate from M4's acceptance checks.

## Implementation discoveries / decisions

- 2026-10-20: The repo is truly empty locally and remotely; there is no existing stack to preserve.
- 2026-10-20: The owner selected Windows-first support.
- 2026-10-20: The owner approved MCP registration/reconciliation and resource reads, the Tauri snapshot/dashboard boundary, and SQLite persistence across an MCP restart as M1 test seams.
- 2026-10-20: Local Codex MCP configuration is documented; hooks and OTel also exist, but exact Windows runtime behavior is still a plan risk.
- 2026-10-20: Context7 monthly quota was exhausted. Official primary docs were used as a fallback; no API calls should be copied from memory.
- 2026-10-20: Windows Tauri unit-test executables lacked the Common Controls v6 manifest and exited with `STATUS_ENTRYPOINT_NOT_FOUND`; the Windows/MSVC build script now embeds the application manifest so the approved IPC test can run.
- 2026-10-20: The Tauri application built and started with isolated local data. The verified acceptance seams pass; visible UI inspection and connecting the owner's Codex app remain manual follow-ups.
- 2026-10-20: Before starting M2, the owner specified a local managed-memory lifecycle. ADR-0003 records class/temperature separation, provenance/temporal history, bounded HOT activation, known-by/delta transfers, conservative forgetting, and the staged plan. M2 implements the substrate; automated gardening, compaction, collection, semantic indexing, and adaptive policy experiments are deferred.
- 2026-10-21: M2 adds the v2 SQLite foundation without a duplicate scope authority. Only repository Git freshness is currently produced and automatically invalidated; Graphify, contract, and dependency freshness remain inactive until those snapshot sources exist. The dashboard command stays read-only, and context-transfer records keep metadata and hashes rather than bodies.
- 2026-10-21: M3 uses the existing MCP connection with an allowlisted Codex `mcp_tool` hook for `UserPromptSubmit`, `PostToolUse`, `Stop`, and `Interrupt`. It persists event names, coarse tool categories, and hashes—not prompt/tool bodies, transcript paths, raw IDs, or the supplied working directory. Observed turns are never considered verified success; stop/interruption is inconclusive unless Cathulu QA already established a terminal result.
- 2026-10-21: OTel is not enabled. The official catalog includes tool-result output snippets, while the first M3 metrics need only lifecycle identity and tool category. Startup Git reconciliation is best-effort per project; a missing repository does not block other refreshes or app startup.
- 2026-10-21: M4 keeps SQLite authoritative and adds FTS5 candidate retrieval, manual Gardener proposals, explicit source-linked consolidation candidates, reference-aware URI metadata collection, immutable policy experiments, run-linked storage snapshots, and aggregate HOT scope health. No semantic index is added without measured retrieval gaps; Graphify/contract/dependency invalidation waits for usable snapshots. Raw telemetry compaction and automatic summary/aggregate maintenance remain later work until real run data can test those policies.
- 2026-10-21: M4 collection is opt-in through an owner-configured age policy, confirms the exact previewed artifact ID set, revalidates eligibility transactionally, and only tombstones Cathulu's local URI metadata. External files and stable artifact records remain intact. Legacy URI-bearing immutable events, active QA, holds, protection, references, or missing extraction receipts prevent collection.

## Completion evidence

M1's build, automated test seams, and isolated Windows startup checks passed before publishing `5f43dcb` to `main`. M2's formatting, seven Rust tests, three React tests, frontend build, Windows release build, isolated runtime/database smoke, and dashboard preview/focus check passed; the implementation and documentation follow-up are published at `a3ca960` and `2c05089`. M3's formatting, 11 Rust tests, three React tests, frontend build, Windows release build, failure-rollback/idempotency/privacy/restart tests, and isolated runtime/database smoke passed before the validated checkpoint `1d445f6` was fast-forwarded to `main`. M4's formatting, 17 Rust tests, three React tests, frontend build, Rust CLI and Tauri Windows release builds, local Gardener/GC preview, and isolated desktop/database smoke passed before checkpoint `185741d` was fast-forwarded to `main`. GitHub reported no status checks or workflow runs for M4; local validation was the available gate. Semantic indexing, Graphify invalidation, raw telemetry compaction, automatic aggregate refresh, and adaptive policy changes remain deferred until measured retrieval gaps, snapshot sources, or sufficient comparable run data exist. The live Codex MCP/hook trust smoke and combined M1-M4 integration validation remain outstanding.
