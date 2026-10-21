# PLAN-002 — Phase 3 Codex-native delegation

Status: READY TO PUBLISH; M5 integration and its documentation are published to `main` at `744a428`; M6 implementation and full Windows validation pass locally on `codex/phase3-delegation-contract`.
Created: 2026-10-21
Last updated: 2026-10-21

## Objective

Let the Main Overseer use Cathulu to prepare and track auditable delegation to Codex-native subagents, preserving structured task packets, worker evidence reports, worktree isolation facts, and measured worker metadata without making Cathulu an agent runtime.

## Motivation

The first two phases now provide local project/workflow state, quality gates, memory lifecycle, and a combined MCP-to-Tauri persistence check. Phase 3 adds the next capability from the roadmap: bounded Codex-native worker delegation with structured evidence flowing back into the existing run, task, artifact, and memory authorities.

## Current state

- M1-M5 are published to `main`; M5 adds a repeatable disposable Windows integration smoke and is published on `codex/phase2-integration-validation` and `main` at `744a428` (implementation checkpoint `e29c90f`).
- Cathulu already records objectives, tasks, runs, immutable events, QA checks, evidence artifacts, and context transfers through SQLite-backed semantic MCP tools.
- Codex remains the owner of agent execution, worktrees, approvals, and the user-facing conversation.
- Current official Codex documentation confirms main-session model/reasoning settings, global subagent defaults, explicit spawn overrides, and custom-agent files with per-role `model` and `model_reasoning_effort`. It documents no API for Cathulu to change the currently running Codex turn. See [ADR-0002](../decisions/ADR-0002-cathulu-hosted-agent-selectors.md).
- Current hooks record allowlisted lifecycle metadata, but do not establish worker-specific completion or expose effective model/effort values.

## Desired state

- A delegation has immutable, provenance-linked identity and packet content attached to an existing project/task/run, plus validated lifecycle transitions recorded in the append-only event journal.
- A worker report contains structured summary, changed paths, check results, artifact references, commit/worktree facts, and blockers; a report is evidence, not verified success.
- Worktree isolation is represented using Git-derived base/head/branch/path data and a clear relationship to the delegated task. Cathulu records and verifies isolation; Codex continues to create and manage worktrees.
- Requested model/effort values, when present, are distinguishable from effective values. Effective values remain unknown unless a trusted source reports them.
- A later, separate Cathulu settings surface can prepare and preview supported Codex configuration changes and requires an explicit owner apply action. It does not silently edit global config or claim changes apply to an already running session.

## Scope

- M6: delegation identity, lifecycle, typed task packets, provenance, MCP operations/resources, and migration tests.
- M7: structured worker reports, idempotency, evidence/artifact references, explicit report review, dashboard presentation, and parent-run quality rules.
- M8: worktree isolation facts and reconciliation, native worker correlation where a supported identifier exists, and privacy-safe worker telemetry.
- M9: model/effort selector settings design and application path, based on the official configuration capabilities and an owner-approved preview/apply behavior.

## Out of scope

- A Cathulu-managed model loop, agent runtime, approval surface, shell executor, or tool router.
- Automatic spawning, worktree creation, worktree deletion, merging, or acceptance of worker changes.
- Treating a worker's self-report, hook event, or successful process exit as `verified_success`.
- Persisting raw prompts, full worker transcripts, unbounded tool outputs, or secrets.
- Guessing effective model/effort values when Codex does not report them.
- Persistent Sub-Overseer ownership or cross-project context replication; these remain Phase 4 and require measured reconstruction cost.

## Source-of-truth impact

- `src/migrations.rs`, `src/delegation.rs`, and `src/gardener.rs`: delegation schema, deterministic operations, and evidence-reference protection.
- `src/mcp.rs` and MCP resources: explicit task packet, lifecycle, report, and isolation contracts.
- `apps/desktop/src-tauri/` and `apps/desktop/src/`: read-only delegation status/evidence presentation; no orchestration controls.
- `docs/project-definition.md`, `docs/architecture.md`, `docs/local-mcp.md`, ADR-0002, `docs/roadmap.md`, and this plan.
- `docs/readiness/definition-gate.md`: update only if product boundaries or privacy assumptions change.

## Existing decisions and constraints

- ADR-0001: Codex owns conversations and execution; Cathulu owns durable evidence and state.
- ADR-0002: Cathulu hosts later selectors in a separate settings surface. The supported config structure is now documented; owner apply behavior and available account-specific options remain open.
- ADR-0003: SQLite is authoritative; context stays bounded and provenance-preserving.
- ADR-0004: telemetry is allowlisted, normalized, and cannot prove work succeeded.
- User selected Windows first; cross-platform compatibility remains later work.
- The dashboard is read-only. State changes use approval-gated domain MCP operations.
- No selector implementation may silently mutate user-level Codex configuration.

## Supporting skills / tools

- `implementation-plan` and `milestone-delivery` govern this plan and each checkpoint.
- `ponytail-balanced` for schema and API scope.
- `tdd` for migration, MCP contract, lifecycle, and IPC tests.
- `openai-docs` for Codex configuration, subagents, hooks, and telemetry; official documentation was checked on 2026-10-21.
- `ui-ux-research` and `impeccable` before M7/M9 dashboard or selector UI changes.
- Context7 for library-specific API work; the last request for Tauri docs returned monthly quota exhausted, so retry before using new framework APIs.

## Dependencies

- M5 shared-store integration validation.
- Existing SQLite migration, event journal, run quality gates, artifact records, context transfer, and read-only dashboard.
- Current local Codex configuration remains owner-controlled; synthetic MCP tests do not require global settings changes.
- No new third-party dependency is expected for M6-M8.

## Risks and unknowns

- Codex's local UI does not expose a documented API for Cathulu to spawn or steer a worker; the product must remain a preparation/recording layer around native Codex actions.
- Hook events may not identify a specific worker or report effective settings. If no supported source exists, these fields stay optional/unknown and manual semantic MCP reporting remains authoritative.
- Custom-agent configuration and model availability can vary by project trust and signed-in account. M9 must not invent an option catalog.
- Worktree paths can be stale, missing, dirty, or point outside the registered project. Reconciliation must validate path and Git identity before recording state.
- Worker-supplied paths, report text, artifact references, and commit IDs are untrusted input.
- A delegation's accepted report cannot bypass the parent's declared quality checks.

## Milestones

### M6 — Delegation identity and task packet contract

Goal: create one durable, auditable representation for a Codex-native delegation using the existing project/task/run model.

Subtasks:

- [x] Add an additive SQLite v5 migration for stable delegation IDs, parent project/task/run links, role, lifecycle, packet schema/version/hash, requested model/effort, hashed idempotency keys, and opaque external correlation hashes.
- [x] Define a bounded typed task packet with objective, acceptance criteria, constraints, allowed scope, referenced memories/artifacts, and requested report shape.
- [x] Add deterministic create/read/transition operations and append-only lifecycle events; retrying the same idempotency key must not duplicate a delegation.
- [x] Add semantic MCP tools and a `cathulu://delegations/{id}` resource; expose no generic SQL and no agent spawning operation.
- [x] Preserve project/task/run referential integrity, protect linked artifacts from evidence collection, and ensure delegation states do not change parent run quality status.

Affected areas: `src/migrations.rs`, new/existing Rust domain module, `src/lib.rs`, `src/mcp.rs`, `tests/`, `docs/local-mcp.md`.

Dependencies: M5 combined integration pass; additive migration discipline.

Acceptance criteria:

- [x] A delegation can be prepared for a valid task/run and read back with stable ID and packet hash.
- [x] Invalid project/run relationships, oversized or escaped-scope packet fields, and illegal lifecycle transitions leave state unchanged.
- [x] Repeated idempotent creation returns the same record; lifecycle history is append-only and survives reopen.
- [x] Requested model/effort is stored separately from effective values and never implies that Codex applied it.
- [x] MCP exposes delegation-specific semantic operations/resources only; it cannot start an agent, run a tool, or write Codex configuration.

Validation:

- [x] Migration from schema v4 with existing project/run/memory rows preserved.
- [x] Domain and MCP integration tests for valid/invalid lifecycle, idempotency, provenance links, artifact retention protection, and restart persistence.
- [x] `cargo fmt --all -- --check`, `cargo test --offline --workspace`, React tests/build, Windows Tauri build, and `git diff --check`.

Documentation updates: architecture, local MCP, roadmap, plan; ADR only if an accepted authority boundary changes.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `openai-docs`.

Expected Git checkpoint: a separate validated milestone branch and coherent commit; publish only after local validation and applicable remote checks.

### M7 — Structured worker reports and review

Goal: capture bounded worker claims and evidence in a form the Main Overseer can inspect and the parent run can evaluate.

Subtasks:

- [ ] Add a versioned structured report with summary, outcome, changed files, commits, test/check results, blockers, and artifact references.
- [ ] Validate reported files and evidence references against the registered project and existing artifact/check records; preserve raw report body limits.
- [ ] Make report submission idempotent and distinguish `reported`, `accepted`, `rejected`, and `needs_follow_up` decisions.
- [ ] Show delegation status and report evidence in the read-only dashboard without adding accept/dispatch controls.
- [ ] Require parent-level mandatory checks to pass before any `verified_success` status.

Affected areas: delegation domain/MCP, event history, dashboard snapshot/types/UI/tests, `docs/`.

Dependencies: M6 delegation contract.

Acceptance criteria:

- [ ] Malformed, oversized, duplicate, or cross-project artifact/path claims are rejected without partial state.
- [ ] A valid report and later review decision remain separately auditable after restart.
- [ ] A positive worker report cannot finish a parent run as verified without its mandatory checks.
- [ ] The dashboard remains read-only and clearly distinguishes reported work from accepted/verified outcomes.

Validation:

- [ ] Contract and property tests for validation, idempotency, report review, and quality-gate non-bypass.
- [ ] React tests for no delegations, active delegation, blocked report, and reported-but-unverified run states.
- [ ] Keyboard/accessibility and narrow-window dashboard checks; all workspace checks and Windows build.

Documentation updates: project definition, architecture, local MCP, roadmap, plan.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `ui-ux-research`, `impeccable`.

Expected Git checkpoint: separate validated milestone branch and coherent commit.

### M8 — Worktree isolation and worker telemetry

Goal: connect reported worker changes to verifiable Git worktree facts without making Cathulu own worktree lifecycle.

Subtasks:

- [ ] Record optional worktree reference, branch, base commit, observed head, cleanliness, changed paths, and last reconciliation time on a delegation.
- [ ] Reconcile only worktrees that resolve under a registered repository/worktree boundary and confirm their Git common-dir relationship.
- [ ] Associate available allowlisted Codex lifecycle events with delegations only when a supported correlation key exists; retain hashes/coarse metadata only.
- [ ] Record requested and effective model/effort separately; leave effective values unknown without a supported source.
- [ ] Document manual native Codex worker handoff and recovery of missing/unavailable worktrees.

Affected areas: Git/domain operations, M3 telemetry mapping, migration if required, MCP, dashboard evidence, tests, docs.

Dependencies: M6, M7; current official Codex hook/configuration contract.

Acceptance criteria:

- [ ] Matching worktrees reconcile deterministically; unrelated, escaped, missing, or wrong-repository paths do not mutate delegation state.
- [ ] A reported worker commit and changed paths can be checked against observed Git state.
- [ ] Unsupported worker identity/model telemetry is reported as unavailable rather than inferred.
- [ ] No tool spawns an agent, creates/removes a worktree, or merges changes.

Validation:

- [ ] Disposable Git worktree tests for clean, dirty, advanced, missing, escaped, and foreign-repository cases.
- [ ] Hook ingestion tests for correlation, privacy minimization, duplicate delivery, and unsupported fields.
- [ ] Restart and shared-store smoke plus all workspace checks and Windows build.

Documentation updates: architecture, ADR-0004 if telemetry behavior changes, local MCP, roadmap, plan.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `openai-docs`.

Expected Git checkpoint: separate validated milestone branch and coherent commit.

### M9 — Model and effort selector application path

Goal: provide the owner-approved Cathulu settings surface for model and effort preferences without claiming unsupported runtime control.

Subtasks:

- [ ] Confirm the owner-approved target layer: project-scoped, user/profile-scoped, or export-only.
- [ ] Discover and validate available model/effort options without hardcoding account access or unsupported combinations.
- [ ] Preview exact Codex configuration changes and require explicit owner application; preserve backups and unrelated config keys.
- [ ] Represent global Main Overseer choices and distinct named-agent choices using supported Codex config/custom-agent files.
- [ ] State when a selection is merely stored, written to config, loaded by a new session, or observed effective.

Affected areas: settings-only UI, typed config parser/serializer, local state, docs, acceptance tests; dashboard remains read-only.

Dependencies: M8; current OpenAI docs; explicit owner decision on configuration target/apply behavior.

Acceptance criteria:

- [ ] A user can review a precise diff before any config write and cancel without side effects.
- [ ] Existing unknown/unrelated config keys and comments are preserved or the UI offers safe export instead of destructive rewriting.
- [ ] Main and custom-role settings are represented independently and validated against the current Codex schema.
- [ ] No selection is labeled effective until the supported Codex runtime confirms it.

Validation:

- [ ] Parser/serializer round-trip fixtures for user/project/custom-role configuration and malformed files.
- [ ] Tests for cancel, apply, backup/restore, unsupported values, config precedence, and role overrides.
- [ ] Owner-controlled Codex smoke only after the preview/apply workflow is reviewable; no unattended global config edits.

Documentation updates: ADR-0002, architecture, project definition, setup docs, roadmap, plan.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `openai-docs`, `ui-ux-research`, `impeccable`.

Expected Git checkpoint: separate validated milestone branch and coherent commit.

## Final integration validation

After M6-M9, run the delegation workflow through local MCP using a disposable Git repository and native Codex subagent handoff: prepare a packet, record dispatch, submit a structured worker report, reconcile its worktree, review evidence, and verify parent QA gates. Confirm requested/effective model and effort distinction, read-only dashboard reporting, persistence across restarts, no agent-spawn/config-write operation in Cathulu MCP, and no raw transcript or secret persistence. The live owner-config selector smoke depends on the owner-approved M9 apply target.

## Rollback / recovery

Use additive SQLite migrations and preserve prior schema versions. Delegation/report records remain evidence if a later UI or integration is reverted. Do not delete worktrees or config files during rollback. For selector config, create and validate a backup before writing; restore only the exact file snapshot the owner approved.

## Progress

- [x] Current Codex documentation checked for model/effort and custom-agent per-role settings.
- [x] M5 combined M1-M4 integration gate passed locally on Windows.
- [x] Publish M5 integration closeout and this plan after docs and checks pass.
- [ ] Publish the validated M6 Git checkpoint to `main` and record its revision.
- [ ] Implement and validate M7.
- [ ] Implement and validate M8.
- [ ] Resolve the owner-controlled M9 apply target, then implement and validate M9.

## Implementation discoveries / decisions

- 2026-10-21: OpenAI documentation confirms `model`/`model_reasoning_effort` for custom agents, global `agents.default_subagent_model` and `agents.default_subagent_reasoning_effort`, and explicit spawn overrides. The current local app must still be checked before claiming effective values.
- 2026-10-21: Cathulu will prepare and record native delegation; it will not spawn agents, create/delete worktrees, run tools, accept code, or mutate Codex configuration through MCP.
- 2026-10-21: M5 uses a disposable repo/database and verifies shared MCP/Tauri SQLite state and restart Git reconciliation.
- 2026-10-21: M6 uses SQLite schema v5, structured 16 KiB task packets, hashed retry/correlation identifiers, scope-checked memory/artifact links, and lifecycle events in the existing append-only journal. Dispatch records a native Codex handoff action but does not start a worker.

## Completion evidence

M6 validation evidence (Windows, 2026-10-21): `cargo fmt --all -- --check` passed; `cargo test --offline --workspace` passed 19 tests with one dedicated integration test ignored by default; `scripts/phase1-m4-integration-smoke.ps1` passed and exercised that test plus desktop startup/restart; `npm test -- --run` passed 3 tests; `npm run build` passed; `cargo tauri build --no-bundle -- --offline` passed; `git diff --check` passed. The milestone branch commit and publication to `main` remain pending. Each later milestone must record exact validation output, source-of-truth documentation, and Git checkpoint.
