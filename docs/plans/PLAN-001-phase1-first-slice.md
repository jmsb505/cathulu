# PLAN-001 — Phase 1 first vertical slice

Status: M1 IMPLEMENTED AND VALIDATED on Windows; see the evidence and remaining manual checks below
Created: 2026-10-20
Last updated: 2026-10-20

## Objective

Create the first usable Cathulu vertical slice on Windows: register a local repository through Codex-facing MCP, persist it in SQLite, reconcile its Git state, and show that state in a read-only Tauri desktop view.

## Motivation

The project definition identifies durable project state as the smallest end-to-end proof of Cathulu's role around Codex. This slice exercises the core path without introducing run orchestration, Sub-Overseers, Graphify, vector retrieval, or cloud services.

## Current state

- At kickoff, the local checkout and personal GitHub repository `jmsb505/cathulu` were empty. M1 source is now present locally on `codex/phase1-foundation`.
- The local toolchain reports Rust/Cargo 1.91.1, Node.js 24.11.1, npm 10.8.3, and Git available.
- Current official documentation supports Tauri 2 projects with React, React TypeScript components, the official `rmcp` Rust MCP SDK with stdio transport, and rusqlite transactions.
- Context7 resolution calls failed because the current monthly quota is exhausted. The source links listed below are the official vendor/project docs fallback. Retry Context7 before future library-specific implementation or API changes.
- The repo's original `main` is unborn. A local orphan milestone branch `codex/phase1-foundation` was created before implementation.
- The owner approved the MCP round-trip, Tauri snapshot/dashboard, and SQLite restart persistence test seams; implementation and test coverage for those boundaries have been authored.
- Rust and npm dependencies were fetched after the package registries became available. The frontend uses React 19.3, TypeScript 5.9, Vite 8.3, and Vitest 5; the lockfiles pin resolved versions.
- The official Visual Studio Build Tools 2022 v17.14.42, MSVC x64/x86 14.44.35207, Windows SDK 10.0.26100.0, and `link.exe` are installed. Use an x64 Visual Studio developer shell for native Rust builds.
- `cargo test --offline --workspace` passes all four tests: one core Git parser test, two MCP vertical-slice tests, and one Tauri IPC command test. The React dashboard suite passes all three tests.
- `cargo tauri build --no-bundle -- --offline` builds `target/release/cathulu-desktop.exe`; the app was started with isolated temporary data directories, remained running for five seconds, and created its SQLite database.
- Tauri test binaries on Windows need the Common Controls v6 application manifest. Without it, the unit-test executable exits before the harness with `STATUS_ENTRYPOINT_NOT_FOUND`; `apps/desktop/src-tauri/build.rs` embeds the manifest for Windows/MSVC targets.

## Desired state

- A Windows-first Tauri 2 + React/TypeScript desktop shell backed by Rust.
- A Rust core library owning project validation, SQLite persistence, and Git reconciliation.
- A local stdio MCP executable reusing that core and exposing semantic project registration/reconciliation plus compact read resources.
- A read-only desktop view that reads the same durable project snapshot through a narrow Tauri command boundary.
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

## Existing decisions and constraints

- [ADR-0001](../decisions/ADR-0001-cathulu-is-the-local-evidence-layer.md): Codex owns interaction and agent execution; Cathulu owns local state/evidence; dashboard is read-only.
- [ADR-0002](../decisions/ADR-0002-cathulu-hosted-agent-selectors.md): Cathulu will host future agent model/effort selectors in a separate settings surface; the Codex integration remains unverified.
- The owner selected Windows first, cross-platform later.
- Adopt the brief's preferred Tauri 2 / React / TypeScript / Rust / SQLite stack because no existing implementation exists.
- SQLite is the system of record; MCP never exposes generic SQL.
- Registered repository contents and telemetry are untrusted; no secrets, raw prompts, or full transcripts are retained by default.
- No known commit exists on `main`; do not advance it. Work on `codex/phase1-foundation`.

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

### M2 — Durable workflow and quality state

Goal: add the minimum durable records and quality rules required for Phase 1 run tracking.

Subtasks:

- [ ] Add dependencies, objectives, tasks, immutable decisions, and immutable workflow versions.
- [ ] Add append-only run events and materialized run state with transactional transitions.
- [ ] Add quality checks and terminal states; enforce that `verified_success` requires all mandatory checks.
- [ ] Add restart, migration, transaction atomicity, and FTS tests as retrieval is introduced.
- [ ] Extend MCP semantic resources/tools and read-only Overview/Activity/Project views.

Affected areas: core schema/domain, MCP contract, dashboard query surface, documentation.

Dependencies: M1 validated; public MCP and Tauri read seams remain stable.

Acceptance criteria:

- Events and current state update atomically; events cannot be edited through model-facing tools.
- Decision and workflow-version history is immutable and superseded by new records.
- No agent claim or timeout can set verified success with a failed/missing mandatory gate.
- Dashboard shows objectives, blockers, recent runs, and failed QA without write actions.

Validation:

- Migration, transaction rollback/atomicity, restart/recovery, state transition, and mandatory-gate tests.
- MCP schema/input and permission-boundary tests.
- Manual UI check for empty, active, blocked, failed, and inconclusive states.

Documentation updates: update domain/architecture, API/MCP contract notes, roadmap, plan, and decision records.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `impeccable`; Context7 for current crate/API details.

Expected Git checkpoint: separate validated milestone commit/branch following repository policy.

### M3 — Local telemetry and Phase 1 integration

Goal: ingest available Codex/tool signals locally and make Phase 1 state useful for measuring quality and context reuse.

Subtasks:

- [ ] Verify available Codex hooks and local OpenTelemetry fields on the target Windows app version.
- [ ] Normalize only supported non-sensitive signals; keep raw prompts and transcripts disabled.
- [ ] Add context transfer records and content hashes only if they can be captured without storing raw prompt bodies.
- [ ] Show run history, blockers, and failed QA in the dashboard.
- [ ] Reconcile Git state after app restart and make telemetry ingestion idempotent.

Affected areas: telemetry adapter, event model, database migration, MCP evidence resources, Activity/Evidence views.

Dependencies: M2 run/evidence model; local Codex runtime experiment.

Acceptance criteria:

- Supported local signals are associated with runs and retained as sanitized evidence.
- Unsupported or unavailable signals do not break MCP, persistence, or the dashboard.
- No raw prompt/transcript is stored by default and no secrets are retained in event payloads.
- Phase 1 Definition of Done is demonstrably met from recorded evidence.

Validation:

- Local hook/OTel integration tests using synthetic non-sensitive input.
- Idempotency, malformed event, partial failure, and restart recovery tests.
- Manual desktop review of project, objective, blocker, run, and QA states.

Documentation updates: update telemetry/privacy/security boundaries, roadmap, plan, and readiness evidence.

Applicable specialized skills: `milestone-delivery`, `openai-docs`, `ponytail-balanced`, `tdd`, `impeccable`; Context7 for implementation libraries.

Expected Git checkpoint: validated milestone commit; release-readiness remains separate.

## Final integration validation

After M1-M3, install/start the Windows desktop build from a clean local state, register a disposable repository, record and retrieve project/run/QA evidence through MCP, verify the dashboard remains read-only, restart Cathulu, and confirm durable state and Git reconciliation. Run the workspace test/build checks and review privacy, security, and integration limitations. This is Phase 1 milestone validation, not public release approval.

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
- [ ] Optional visible UI and owner-controlled Codex connection smoke.
- [ ] M2 and M3 complete.

## Implementation discoveries / decisions

- 2026-10-20: The repo is truly empty locally and remotely; there is no existing stack to preserve.
- 2026-10-20: The owner selected Windows-first support.
- 2026-10-20: The owner approved MCP registration/reconciliation and resource reads, the Tauri snapshot/dashboard boundary, and SQLite persistence across an MCP restart as M1 test seams.
- 2026-10-20: Local Codex MCP configuration is documented; hooks and OTel also exist, but exact Windows runtime behavior is still a plan risk.
- 2026-10-20: Context7 monthly quota was exhausted. Official primary docs were used as a fallback; no API calls should be copied from memory.
- 2026-10-20: Windows Tauri unit-test executables lacked the Common Controls v6 manifest and exited with `STATUS_ENTRYPOINT_NOT_FOUND`; the Windows/MSVC build script now embeds the application manifest so the approved IPC test can run.
- 2026-10-20: The Tauri application built and started with isolated local data. The verified acceptance seams pass; visible UI inspection and connecting the owner's Codex app remain manual follow-ups.

## Completion evidence

M1's build, automated test seams, and isolated Windows startup checks pass. A visible UI/Codex configuration smoke remains manual, and M2/M3 are still planned work.
