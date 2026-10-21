# PLAN-002 — Phase 3 Codex-native delegation

Status: M1-M8 are validated and published to `main`; M8 is published at `aecac57`.
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
- ADR-0002: Cathulu hosts selectors in a separate settings surface. The user chooses a target for each preview: registered project config, user base config, a named user profile, or export-only. Account availability and effective runtime settings remain unverified.
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
- Context7 for library-specific API work; the 2026-10-21 `toml_edit` resolution request again returned monthly quota exhausted. The locally cached 0.25.17 source and official docs.rs reference were used; no new Tauri or React API was introduced.

## Dependencies

- M5 shared-store integration validation.
- Existing SQLite migration, event journal, run quality gates, artifact records, context transfer, and read-only dashboard.
- Current local Codex configuration remains owner-controlled; synthetic MCP tests do not require global settings changes.
- M6-M8 added no dependency. M9 adds `toml_edit` to preserve existing TOML comments and unrelated settings during targeted edits.

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

- [x] Add a versioned structured report with summary, outcome, changed files, commits, test/check results, blockers, and artifact references.
- [x] Validate reported files and evidence references against the registered project and existing artifact/check records; preserve raw report body limits.
- [x] Make report submission idempotent and distinguish `reported`, `accepted`, `rejected`, and `needs_follow_up` decisions.
- [x] Show delegation status and report evidence in the read-only dashboard without adding accept/dispatch controls.
- [x] Require parent-level mandatory checks to pass before any `verified_success` status.

Affected areas: delegation domain/MCP, event history, dashboard snapshot/types/UI/tests, `docs/`.

Dependencies: M6 delegation contract.

Acceptance criteria:

- [x] Malformed, oversized, duplicate, or cross-project artifact/path claims are rejected without partial state.
- [x] A valid report and later review decision remain separately auditable after restart.
- [x] A positive worker report cannot finish a parent run as verified without its mandatory checks.
- [x] The dashboard remains read-only and clearly distinguishes reported work from accepted/verified outcomes.

Validation:

- [x] MCP contract tests cover validation, idempotency, report review, reference scope, restart persistence, and quality-gate non-bypass.
- [x] React tests cover no delegations, active delegation, blocked report, reported-but-unverified run state, semantic headings/link, and the absence of write controls.
- [x] Responsive rules are present for 900px and 620px widths; semantic/read-only behavior is covered in React tests. Direct visual browser viewport inspection was unavailable because the browser tool could not connect to the local Windows loopback server; this limitation is retained in completion evidence.
- [x] Workspace tests, formatting, frontend build, and Windows MCP/Tauri desktop/restart smoke pass.

Documentation updates: project definition, architecture, local MCP, roadmap, plan.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `ui-ux-research`, `impeccable`.

Expected Git checkpoint: separate validated milestone branch and coherent commit.

### M8 — Worktree isolation and worker telemetry

Goal: connect reported worker changes to verifiable Git worktree facts without making Cathulu own worktree lifecycle.

Subtasks:

- [x] Record optional worktree reference, branch, base commit, observed head, cleanliness, changed paths, and last reconciliation time on a delegation.
- [x] Reconcile only worktrees that resolve under a registered repository/worktree boundary and confirm their Git common-dir relationship.
- [x] Associate available allowlisted Codex lifecycle events with delegations only when a supported correlation key exists; retain hashes/coarse metadata only.
- [x] Record requested and effective model/effort separately; leave effective values unknown without a supported source.
- [x] Document manual native Codex worker handoff and recovery of missing/unavailable worktrees.

Affected areas: Git/domain operations, M3 telemetry mapping, migration if required, MCP, dashboard evidence, tests, docs.

Dependencies: M6, M7; current official Codex hook/configuration contract.

Acceptance criteria:

- [x] Matching worktrees reconcile deterministically; unrelated, escaped, missing, or wrong-repository paths do not mutate delegation state.
- [x] A reported worker commit and changed paths can be checked against observed Git state.
- [x] Unsupported worker identity/model telemetry is reported as unavailable rather than inferred.
- [x] No tool spawns an agent, creates/removes a worktree, or merges changes.

Validation:

- [x] Disposable Git worktree tests for clean, dirty, advanced, missing, escaped, and foreign-repository cases.
- [x] Hook ingestion tests for correlation, privacy minimization, duplicate delivery, and unsupported fields.
- [x] Restart and shared-store smoke plus all workspace checks and Windows build.

Documentation updates: architecture, ADR-0005, local MCP, project definition, roadmap, plan.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `openai-docs`.

Expected Git checkpoint: separate validated milestone branch and coherent commit.

### M9 — Model and effort selector application path

Goal: provide a user-controlled Cathulu settings surface for model and effort preferences without claiming unsupported runtime control.

Subtasks:

- [x] Offer explicit project-scoped, user base, named profile, and export-only target choices per preview; no target is changed automatically.
- [x] Discover model/effort candidates from the bundled local Codex CLI catalog, whitelist the fields Cathulu exposes, and validate each effort against its model. The catalog is not treated as proof of account availability.
- [x] Preview exact Codex configuration changes and require a separate Apply action; create backups, preserve comments and unrelated keys, and reject stale previews.
- [x] Represent Main Overseer, default subagent, and named-agent choices independently using supported Codex config/custom-agent files. Missing or malformed named-agent files are shown as skipped and are never created as partial definitions.
- [x] Distinguish Cathulu-stored preferences from settings written to a selected config target; runtime effectiveness remains explicitly unverified.

Affected areas: settings-only UI, typed config parser/serializer, local state, docs, acceptance tests; dashboard remains read-only.

Dependencies: M8; current OpenAI docs; explicit per-preview target selection and a separate Apply action, with no implicit default target.

Acceptance criteria:

- [x] A user can review a precise field-level diff before any config write and discard it; preparing or discarding a preview does not write config files.
- [x] Existing unknown/unrelated config keys and comments are preserved by a TOML-aware patch; export-only returns targeted snippets without reading config contents.
- [x] Main, default subagent, and existing custom-role settings are represented independently and validated against the current local CLI catalog; account availability is not inferred.
- [x] No selection is labeled effective; every settings surface reports runtime effectiveness as unverified.

Validation:

- [x] TOML fixtures cover preserving comments and unrelated keys, malformed files, project-target preview/apply, user base/profile target resolution, export-only, and existing custom-role overrides.
- [x] Tests cover cancel/no-write preview, backup creation, stale-preview refusal, unsupported model/effort pairs, target isolation, and role-specific configuration.
- [x] The Tauri command test applies only to a disposable registered repository. No owner Codex config file was changed; no live runtime-effectiveness claim is made.

Documentation updates: ADR-0002, architecture, project definition, setup docs, roadmap, plan.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `openai-docs`, `ui-ux-research`, `impeccable`.

Expected Git checkpoint: separate validated milestone branch and coherent commit.

## Final integration validation

After M6-M9, run the delegation workflow through local MCP using a disposable Git repository and native Codex subagent handoff: prepare a packet, record dispatch, submit a structured worker report, reconcile its worktree, review evidence, and verify parent QA gates. Confirm requested/effective model and effort distinction, read-only dashboard reporting, persistence across restarts, no agent-spawn/config-write operation in Cathulu MCP, and no raw transcript or secret persistence. The settings surface is validated against disposable files; live runtime-effectiveness remains unverified because Codex exposes no supported control/verification API to Cathulu.

## Rollback / recovery

Use additive SQLite migrations and preserve prior schema versions. Delegation/report records remain evidence if a later UI or integration is reverted. Do not delete worktrees or config files during rollback. For selector config, create and validate a backup before writing; restore only the exact file snapshot the owner approved.

## Progress

- [x] Current Codex documentation checked for model/effort and custom-agent per-role settings.
- [x] M5 combined M1-M4 integration gate passed locally on Windows.
- [x] Publish M5 integration closeout and this plan after docs and checks pass.
- [x] Publish the validated M6 Git checkpoint to `main` and record its revision (`1814ccd`).
- [x] Implement, validate, and publish M7 to `main` at `6251515`.
- [x] Implement, validate, and publish M8 to `main` at `aecac57`.
- [x] Implement the M9 settings surface with explicit per-preview project, user-base, user-profile, and export-only choices.
- [x] Implement safe configuration preview/apply/export behavior and validate with local, disposable fixtures.
- [x] Publish the validated M9 checkpoint to `main` at `f2381ae` after local checks.

## Implementation discoveries / decisions

- 2026-10-21: OpenAI documentation confirms `model`/`model_reasoning_effort` for custom agents, global `agents.default_subagent_model` and `agents.default_subagent_reasoning_effort`, and explicit spawn overrides. The current local app must still be checked before claiming effective values.
- 2026-10-21: Cathulu will prepare and record native delegation; it will not spawn agents, create/delete worktrees, run tools, accept code, or mutate Codex configuration through MCP.
- 2026-10-21: M5 uses a disposable repo/database and verifies shared MCP/Tauri SQLite state and restart Git reconciliation.
- 2026-10-21: M6 uses SQLite schema v5, structured 16 KiB task packets, hashed retry/correlation identifiers, scope-checked memory/artifact links, and lifecycle events in the existing append-only journal. Dispatch records a native Codex handoff action but does not start a worker.
- 2026-10-21: M6 implementation `1814ccd` was pushed to `main` after local validation passed; GitHub returned no status checks and no open pull request for the branch.
- 2026-10-21: M7 uses additive SQLite schema v6. One immutable 16 KiB schema-v1 worker report may attach to a dispatched delegation. Changed paths are checked against packet scope; artifact and quality-check references are restricted to the parent run; parent quality-check state and details hashes are snapshotted. Report/review requests are idempotent, key values are hashed, reviews are append-only, and worker outcomes do not affect parent-run quality. Referenced report artifacts are protected from orphan collection.
- 2026-10-21: The dashboard adds up to ten recently active delegation/report summaries as a read-only snapshot. React checks cover semantic labels/read-only states and responsive breakpoints are defined at 900px/620px. Browser viewport inspection could not run because available browser contexts could not reach the local Windows loopback server; no visual pass is claimed.
- 2026-10-21: M7 commit `6251515156c033e15b276bacc73b54df92101643` was pushed as a fast-forward to `main` after local validation. GitHub's combined status response contained no status checks.
- 2026-10-21: Current official OpenAI Hooks documentation lists `agent_id`, `agent_type`, and the active `model` on `SubagentStart`/`SubagentStop`; subagent hooks use the parent `session_id`. It documents no reasoning-effort field. M8 correlates through an explicit stored SubagentStart event reference and parent-run match, hashes session/agent identifiers, stores only coarse role/model metadata, and leaves effective effort unknown. Transcript paths and last assistant messages remain excluded. See [ADR-0005](../decisions/ADR-0005-codex-worker-telemetry-and-worktree-verification.md) and the [Hooks guide](https://learn.chatgpt.com/docs/hooks).
- 2026-10-21: Git documentation confirms `git worktree list --porcelain` provides machine-readable worktree paths/HEAD/branch state, `rev-parse --show-toplevel` identifies the real worktree root, and `--git-common-dir` identifies shared repository metadata. M8 requires an exact canonical linked worktree, distinct from the registered root, and matching common directory before storing state; worker worktrees remain Codex-owned. New delegations capture the actual registered project HEAD at preparation; legacy rows remain baseline-unknown.
- 2026-10-21: Current official OpenAI docs direct users to the local `/model` picker and say available models depend on account/workspace. The installed `codex-cli 0.146.1` exposes five visible model entries and per-model effort values through `codex debug models`, but the debug output is not documented as a stable application API and includes non-catalog instructions. M9 must whitelist any consumed catalog fields, never expose raw output, and label a model as effective only after the runtime verifies it. No account entitlement is inferred from the bundled catalog.
- 2026-10-21: M9 reads only `codex debug models --bundled`, keeps `slug`, `display_name`, `supported_reasoning_levels[].effort`, and `default_reasoning_level`, and discards all other catalog fields including instruction text. SQLite schema v8 stores independently keyed Main Overseer, default subagent, and named-agent preferences with the CLI version, applied-target metadata, and append-only events. The catalog explicitly reports `accountAvailabilityVerified: false`.
- 2026-10-21: M9 adds a separate desktop settings page. The dashboard remains read-only. Each preview chooses registered project config, user base config, a named profile, or export-only. Project paths come only from the registered-project table; user paths come from `CODEX_HOME` or `%USERPROFILE%\\.codex`; caller-supplied filesystem paths are not accepted. Project config displays the Codex trust requirement.
- 2026-10-21: `toml_edit` patches only the supported model/effort keys and preserves comments and unrelated keys. Apply requires the in-memory reviewed preview ID, rechecks original content hashes and path boundaries, creates per-file backups, then records a `written_to_config` event. Named-agent files must already exist and match their declared name; incompatible agents are reported as skipped. Export-only emits only selected TOML snippets and does not read config contents.
- 2026-10-21: Cathulu reports preferences as `stored_in_cathulu` or `written_to_config`; runtime effectiveness always remains unverified. The local model bundle is not treated as account availability. Cathulu MCP has no configuration-write command, and the owner-controlled Codex config was not changed during implementation or tests.

## Completion evidence

M6 validation evidence (Windows, 2026-10-21): `cargo fmt --all -- --check` passed; `cargo test --offline --workspace` passed 19 tests with one dedicated integration test ignored by default; `scripts/phase1-m4-integration-smoke.ps1` passed and exercised that test plus desktop startup/restart; `npm test -- --run` passed 3 tests; `npm run build` passed; `cargo tauri build --no-bundle -- --offline` passed; `git diff --check` passed. Commit `1814ccd` is published to `main`. GitHub reported no status checks for the commit; all applicable local gates passed.

M7 validation evidence (Windows, 2026-10-21): `cargo fmt --all -- --check` passed; `cargo test --offline --workspace` passed 21 tests with one dedicated integration test ignored by default; `scripts/phase1-m4-integration-smoke.ps1` passed, including the dedicated report/review path through MCP, the shared Tauri dashboard snapshot, Windows desktop startup, and MCP restart persistence; `npm test -- --run` passed 3 tests; `npm run build` passed; `git diff --check` passed. Browser viewport inspection could not run because the available browser hosts timed out connecting to the local Vite loopback port; semantic React/read-only assertions and the 900px/620px responsive rules were checked instead. Commit `6251515` is published to `main`; GitHub returned no status checks. Each milestone must record source-of-truth documentation and its Git checkpoint.

M8 validation evidence (Windows, 2026-10-21): `cargo fmt --all -- --check` passed; `cargo test --offline --target-dir target\m8-integration-smoke --workspace` passed 23 tests with one dedicated Tauri integration test ignored by default; `scripts/phase1-m4-integration-smoke.ps1` passed and exercised that integration path with an actual linked worker worktree, report verification through MCP and Tauri, Windows desktop startup, and persisted MCP reads after restart; `npm test -- --run` passed 3 tests; `npm run build` passed; `git diff --check` passed. Commit `aecac57777fdf31c021553bce8a30164a9444e07` was pushed as a fast-forward to `main`. GitHub returned no status checks and no workflow runs. The live owner Codex hook trust/configuration path remains unmodified and unverified.

M9 validation evidence (Windows, 2026-10-21): `cargo fmt --all -- --check` passed; `cargo test --offline --workspace` passed 33 tests with one dedicated Tauri integration test ignored by default; `scripts/phase1-m4-integration-smoke.ps1` passed, including the ignored MCP/Tauri shared-state and restart test, the no-bundle Windows desktop build/startup, persisted MCP reads after restart, and a verified Git refresh; `npm test -- --run` passed 5 tests; `npm run build` passed; and `git diff --check` passed. Rust tests exercised bundled catalog allowlisting, preference persistence, separate project/user/profile/export target resolution, TOML preservation, backup creation, stale-preview refusal, malformed/missing named-agent handling, and explicit Tauri preview/apply. All config writes used disposable files; the owner's Codex config was not changed. The live browser could not reach the Windows loopback preview, so no viewport visual pass is claimed; React semantics, responsive rules, and the prior clean design-detector result were checked, and small settings text colors were darkened after a manual contrast calculation. Runtime effectiveness and account availability remain unverified.

M9 implementation commit `f2381aea6de57b6473245b4eddd051b1ef0010c2` was fast-forwarded to `main` after local validation. GitHub returned no status checks for the commit.
