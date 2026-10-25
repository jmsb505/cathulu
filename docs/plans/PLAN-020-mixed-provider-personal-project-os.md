# PLAN-020 — Mixed-provider personal project OS

Status: M47 is delivered. The Owner accepted M48 checkpoints `5fc5459` and `d7dff34`, and M49 `0e6718a`; Claude independently accepted the stable reviews and their core suites pass. The Owner accepted the F1 contract freeze at `09f192b`; amendments require a written review. F7 is confirmed: project priority ranks before task priority across projects. F8 is accepted only for local-only use; revisit before remote access. P-Q2 has no approved personal categories; it remains unresolved, and no personal facts are collected or disclosed. P-Q1 is partially advanced: Claude Code 2.1.302 is installed and its empty Cathulu config passed an unauthenticated `claude doctor`, but Owner login and isolation/permission verification remain open, so Claude is not available for live work. CX-04/M51b now routes queued Worker/Project Manager dispatch and Project Manager inbox turns through CodexAdapter with typed provider events and exact M52 admission (40a7ee0); restart recovery is recorded at e85c07d. Offline integration is validated, but the path remains non-activating and no live provider protocol acceptance is claimed. CX-05 adds a tested, non-activating M52 workspace/lease, credential-source and recovery foundation on schema v22. Claude accepted admission amendments A–D at `3444829134e7630c60b2e97b836cf5dbd7367300`, R1 at `6153cdc155a1ac3b10fc62b3e8b904e2b76e5bef`, and B1/D1 at `67266ab`. Claude accepted the recovery transaction and its R-A/B/C corrections at `ecf7d11`; the documented Windows spawn-to-assignment gap, lack of non-Windows containment, and operator-attested provider-stop check remain limits. Codex added focused renewal-failure, transport-loss, direct-revoke, simultaneous-claim and restart-quarantine coverage; the full offline all-target suite now passes 158/158. The runtime gate `650ad6b` holds product dispatch off and changes the previous default-on behavior. M52 source safety and admission work is complete; the conservative project-wide writer lease satisfies the current semantic-overlap safety requirement, and authorized M42/M44 live/recovery evidence remains open. The gate stays false; review does not itself activate product dispatch. CX-06 adds a provider-free pathless non-code project foundation under schema v23; M50 remains open for personal categories/preferences, correction/export/retention, provider disclosure grants, and Owner decisions. CX-07 registers pure Claude adapter and routing-policy cores with offline fixtures but does not wire dispatch. M53-M55 remain open. Local source/test evidence does not establish runtime or remote integration.
Created: 2026-10-25
Last updated: 2026-10-25
Direction: [ADR-0022](../decisions/ADR-0022-mixed-provider-personal-project-os.md)
Design and source evidence: [Pivot migration assessment](../pivot-migration.md)

Manual development assignment: [PLAN-021](PLAN-021-manual-claude-codex-development.md) maps these milestones to Claude Code and Codex working in the same directory under [ADR-0023](../decisions/ADR-0023-manual-shared-directory-development.md). This plan retains capability acceptance; PLAN-021 owns file/agent assignments and stable-tree integration. Its scheduling split allows offline M53 adapter preparation after the actual M51a common-interface freeze while M51b/M52 progress. M53 activation/completion still requires all original prerequisites; M54 policy preparation similarly does not complete the integrated milestone early.

## Objective

Pivot the existing Cathulu authority into a useful local personal project manager with provider-neutral coding execution. Preserve SQLite identity, curated memory, evidence, deterministic QA, role hierarchy and existing data. Claude Code and Codex become replaceable execution/judgment resources; provider availability must not affect basic project-management queries.

## Motivation, current and desired state

The Owner requested the mixed-provider pivot using the supplied plan. The M47 assessment began with a mature Rust/SQLite/MCP substrate, Codex app-server dispatch, structured plans/reports, source-backed memory and a paused desktop track. M48 now extends the existing task records for deterministic project planning while project registration and execution remain repository/Codex-bound. Provider-free usefulness remains the first migration step; M49-M55 are still future work.

## Scope and exclusions

In scope: migration definition; deterministic planning state; checkpoints/resume; controlled personal/non-code projects; typed artifacts; provider/session abstraction; workspace leases; actual-provider validation; explicit task-shape routing; reviewed handoffs and bounded hierarchy extension.

Excluded: blind rewrite, hosted backend/database, mandatory model API billing, separate vector database, broad transcript ingestion, permanent agent swarm, automatic policy promotion, production deployment and desktop implementation while paused. Planning provider roles does not authorize sending other projects or personal memory to a new provider. No install/account/config changes, live dispatch, runtime reload or Git publication is part of M47.

## Source-of-truth impact

- [Project definition](../project-definition.md): product purpose, actors, scope, accepted invariants.
- [Architecture](../architecture.md): legacy implementation boundary and pivot design/profile pointer.
- [Migration assessment](../pivot-migration.md): current-state evidence and proposed target contracts.
- [Operating contract](../agent-operating-contract.md): existing enforceable role/approval/privacy boundaries; provider-neutral target overlay.
- [Roadmap](../roadmap.md): pivot order; historical milestones retain their evidence status.
- [Definition gate](../readiness/definition-gate.md), [kickoff state](../kickoff-state.json), [documentation map](../README.md): bounded readiness, claims/questions and authority.
- [ADR-0022](../decisions/ADR-0022-mixed-provider-personal-project-os.md): scoped supersession; retain ADR-0021 development cadence.

No changelog is present in the inspected project file inventory. M47 ships no executable behavior; do not invent a release entry. Reassess changelog usage at shipped milestones.

## Existing decisions, constraints and dependencies

Use ADR-0021 and PLAN-019 continuous supervised development. Keep one active PM per project, two Workers total per PM assignment, parent reviews, source-backed memory, current permission gates and headless MCP. Keep desktop work paused. [PLAN-015 M42](PLAN-015-event-driven-role-orchestration.md) and [PLAN-017 M44](PLAN-017-terminal-observability.md) still own current live acceptance and recovery evidence; neither is declared passed by this pivot.

HEAD at inspection: `757fd3d5c0325095e40b07963602eb6feccd5624` with extensive existing dirty changes, including untracked app-server/dispatcher source. Inspect those diffs before touching a milestone's scope. The next schema version is selected only after rechecking the initializer; source currently supports 18. A conflict blocks its affected scope, not independent safe work. The Owner's subsequent 2026-10-25 synchronization request authorizes branch/commit/push/merge of the pending checkpoint and supersedes PLAN-019's earlier local-only Git boundary for that work.

## Supporting skills / tools

Use the assessment's [Development Skills & Tooling Profile](../pivot-migration.md#development-skills--tooling-profile). project-kickoff defines the bounded slice, implementation-plan owns this sequence, milestone-delivery owns execution/validation, ponytail-balanced limits new concepts, and Graphify/source tracing supports repository claims. Context7 was attempted and quota-limited; official provider docs were fetched as fallback. Use ui-ux-research plus impeccable only for the later resumed product UI. Existing Rust/MCP/Vitest seams are preferred. Release-readiness remains separate from milestone acceptance.

## Risks and unknowns

Assessment sections 8, 17, 22, 23 and 29-30 record the risks and dependent gates. Claude is absent on inspected PATH; installed subscription/configuration safety remains unverified. Current dispatch uses a registered root rather than allocated writer worktrees. Logical known-by does not prove fresh-session delivery. Existing role CHECK constraints and project registration encode narrower semantics. None of these blocks provider-free task/checkpoint work; each blocks its dependent execution or data scope.

## Milestones

### M47 — Evidence-backed migration definition

Goal: establish the product direction and a source-backed incremental migration before major implementation.

Subtasks:

- [x] Read the supplied brief, project instructions, ADR-0021, PLAN-019, existing product/architecture/contract and affected source/tests.
- [x] Inspect the dirty tree and capture preservation hashes before writing.
- [x] Record the 31-section assessment, compatibility/conflict analysis, provider documentation caveats, first slice and deferred questions.
- [x] Rebaseline canonical direction, decision record, scoped definition gate, authority map and working checkpoint.
- [x] Validate documents, links, state structure, milestone coverage and preservation; record exact evidence.

Affected areas: documentation only, plus README description. Dependencies: Owner's pivot request and inspected current source.

Acceptance: every requested assessment section exists; significant recommendations cite current files; observed/target/provider-live states are distinct; first increment is defined without relying on unresolved provider/personal-data/UI questions; no unrelated work is overwritten.

Validation: relative Markdown links and anchors, fenced code blocks, sequential assessment headings, milestone IDs/dependencies, kickoff JSON parse and scoped gate aggregation, targeted whitespace check, before/after hashes for unaffected files and patch review for changed preexisting files. No app tests/build/reload needed for documentation-only work.

Documentation: all source-of-truth impacts above. Skills: project-kickoff, implementation-plan, ponytail-balanced, Graphify fallback, Context7/openai-docs, ui-ux-research, anti-slop; milestone-delivery for final verification/checkpoint.

Expected Git checkpoint: reviewable local documentation patch, unstaged under current boundary; do not mark integrated/merged.

### M48 — Deterministic tasks and portfolio state

Goal: list and manage actionable project work with no coding provider.

Subtasks:

- [x] Recheck dirty diffs, schema number, existing task callers and capability filters; capture targeted baseline.
- [x] Add explicit project status/priority and task parent, priority, due date, effort, next action, completion/revision metadata; retain existing IDs/status strings/objective ownership.
- [x] Add validated prerequisite edges and structured blocker reason/resolution; reject cycles and cross-project links without declared scope.
- [x] Add bounded portfolio/task queries with deterministic eligibility, blocked reasons and stable sorting; distinguish unknown effort/due dates.
- [x] Expose semantic Store/MCP commands and resources with scoped authorization; no generic SQL/CRUD endpoint.
- [x] Guard administrative completion separately from execution-linked acceptance and parent/dependency constraints.

Affected areas: `src/workflow.rs`, `src/migrations.rs`, `src/lib.rs`, `src/mcp.rs`, relevant dispatcher capability checks, existing Rust/MCP tests; optional focused new portfolio module if separation warrants it.

Dependencies: M47 scoped gate PASS; preservation/conflict review. No provider account, personal-data consent or UI activation is required.

Acceptance: B blocked by A is excluded with a reason; explicit priority changes are audited; only completed prerequisites unblock by default; unauthorized cross-project edits/cycles/stale revisions are rejected atomically; existing run QA cannot be bypassed; providers absent or disabled do not alter query results or cause provider processes.

Validation: `cargo fmt --all -- --check`; targeted `tasks_m48` tests (6 passed); full `cargo test -p cathulu-core --locked --offline` (95 passed, 0 failed); populated v18 and v1 upgrade fixtures, restart persistence, deterministic ordering, dependency cycles, wrong-project IDs, stale revisions, administrative-vs-run completion, and a public MCP flow with dispatch disabled and provider credentials absent. Full evidence and limitations are in the [M48 evidence record](../readiness/m48-task-portfolio-2026-10-25.md).

Documentation: update current model/command contracts, migration evidence, operating capability rules and this plan. Skills: milestone-delivery, ponytail-balanced; Context7 if dependency-specific APIs change; use existing integration test conventions.

Expected Git checkpoint: keep the Owner-selected shared branch `codex/manual-dual-m48`; commit only complete reviewed Codex paths and leave Claude's files untouched. Do not claim remote integration, release, reload, or live-store migration from the local checks.

### M49 — Immutable checkpoints and deterministic project resume

Goal: prove the first complete provider-free pivot workflow.

Subtasks:

- [x] Add immutable project checkpoint revisions with source state/observation versions, artifact/evidence references and latest-project lookup.
- [x] Implement bounded `resume_project` projection joining current tasks, blockers, objectives, recent accepted decisions and verification/repository freshness.
- [x] Support missing/stale checkpoint and unavailable repository without guessed facts; make Git refresh explicit.
- [x] Reuse objectives as goals; no separate milestone entity is required by this checkpoint slice.
- [x] Exercise create/update/checkpoint/restart/resume with providers absent.

Affected areas: workflow/checkpoint module, migrations, Store/MCP queries, artifacts/events and existing tests. Dependencies: M48.

Acceptance: after restart the Owner can retrieve what remains, what blocks it and the next eligible action; current tasks override stale checkpoint summaries and deltas are visible; provider calls remain zero; returned evidence and freshness are traceable; historical checkpoint revisions are immutable.

Validation: three focused M49 tests and the full core suite (107 tests) cover public MCP create/update/checkpoint/restart/resume, populated v19 upgrade, immutable revisions, bounded results/omitted counts, task changes after checkpoint, missing repo/checkpoint, changed mandatory checks and non-passing QA. Public scoped PM cross-project denial and Main reads pass. Formatting passes. See [M49 evidence](../readiness/m49-checkpoint-resume-2026-10-25.md) for exact commands, the repaired fixture ordering issue and remaining runtime limits.

Documentation: accepted checkpoint/resume contract, actual test evidence and first-slice status. Skills: milestone-delivery, ponytail-balanced. Git checkpoint: validated M49 capability patch under applicable authorization; no release-readiness claim.

### M50 — Controlled personal and non-code project state

Goal: extend useful local state beyond Git repositories without weakening disclosure boundaries.

Subtasks:

- [x] Revisit P-Q2 at M50 start: it remains unresolved. No personal categories are approved; do not collect or disclose personal facts until categories and per-provider grants are explicitly decided. Keep schema/proposal work independent.
- [x] Introduce optional repository association while preserving project IDs, existing registration semantics, logical PM keys and Git-only command failures for non-code projects. CX-06's provider-free foundation preserves these identities and rejects/skips pathless Git operations; it does not complete M50.
- [x] Apply Claude's CC-06 amendments A-1 through A-5 in the Codex-owned M50 paths: omit private IDs/metadata from Main's project list, select only repository-kind workspaces, preserve the prior SQLite foreign-key setting across migration, clarify the FTS claim, and test dispatch's pathless refusal. Commit `0d33295f6811d047b040a4fc9d3147c407e1617d` is pushed; the full offline all-target suite passes 164 tests and the focused M50 suite passes 4/4. M50 remains open for the personal-data decisions below.
- [ ] Add explicit reviewed personal preference scope and minimal deterministic profile settings; keep portfolio summaries separate.
- [ ] Define correction, export/deletion/retention and provider disclosure behavior for the collected categories before activation.

Affected areas: Store registration, project schema/triggers/roles, memory scope/disclosure, MCP filters and portfolio queries. Dependencies: M49; P-Q2 before personal collection; P-Q3 only if scope exceeds zero-or-one repository.

Acceptance: a non-code project has tasks/checkpoint without Git calls or fake paths; existing repos/role keys still resolve; personal memory is not in ordinary global/project context and unscoped clients cannot read it; explicit edits retain provenance; no provider invocation.

Validation: mixed repository/non-code populated migration, trigger/FK/FTS checks, restart, scope leakage and consent-denial tests, correction/retention scenarios; full relevant core checks.

Documentation: [ADR-0025](../decisions/ADR-0025-optional-project-repository.md) records the repository-identity decision and the unresolved privacy boundary. CX-06 records the source model, pathless foundation and reviewed A-1–A-5 corrections; data/retention/disclosure contract and any personal categories remain open. Skills: milestone-delivery, ponytail-balanced; privacy/capability review using existing tests. Git checkpoint: separately validated provider-free M50 slice; no personal-data activation.

### M51 — Typed artifacts and Codex adapter parity

Goal: place existing execution behind a provider-neutral boundary without losing current protections.

M51a checkpoint: the [F1 candidate](../parallel/codex/CX-03-F1-contract.md) exports real provider/artifact/session/routing-input types and a deterministic standalone-consumable double. This does not check off M51's runtime/persistence/parity criteria below; those require M51b and its original acceptance.

CX-04/M51b source checkpoint: commit `40a7ee0` routes queued Worker/Project Manager dispatch and Project Manager inbox turns through `CodexAdapter`, persists typed provider events against lease-bound attempts, and refreshes exact M52 admission before session operations and submits. Its focused core checkpoint passed the offline all-target suite (155/155) with dispatch disabled. The implementation is non-activating; independent semantic review remains open, and no live provider protocol or account acceptance is claimed.

CX-04/CX-07 integration follow-up (working candidate after Claude's fresh PAUSE WRITES / BUILDS acknowledgement): fixed the Codex adapter's late-start ordering defect so a delayed `turn/started` for an older turn cannot hide the active turn's terminal event; the regression passed 1/1. Registered policy tests assert `PreflightRequired` for unknown availability and cover the Owner's D-4 decision for transport, installation and authentication failures, including the sole independent reviewer case; `routing_policy` passed 31/31. `cargo fmt --all -- --check`, `cargo check --offline --all-targets`, and `cargo test --offline --all-targets` passed with dispatch disabled. This is fixture evidence only. M51b remains partial pending Claude's read-only semantic review of the stable integration commit and authorized M42/M44 live/recovery evidence; M53 remains unavailable for live routing, and product dispatch stays off.

Subtasks:

- [x] Add minimal typed envelope/payload validation around existing artifact IDs and packets/reports; preserve historical readers.
- [x] Separate logical role, provider session and run/attempt identity; namespace external IDs and add session activation/context delivery receipts.
- [x] Wrap AppServerProcess in CodexAdapter and translate events/errors; keep scheduling, policy, approval and reconciliation in Cathulu.
- [x] Publish a versioned capabilities contract; unsupported interrupt/status features must fail explicitly until implemented.
- [x] Prove fresh-session context rehydration and existing durable queue/role tool/review behavior with fixtures.

Affected areas: new focused agent provider/artifact modules, app_server, dispatcher, migrations, memory delivery, MCP and capability/event-routing tests. Dependencies: M49; M50 is independent. No new live execution activated before M52 safety gates.

Acceptance: Codex protocol details are confined to the adapter; existing role queues/reviews/QA/unknown states survive; replacement sessions get selected context even with stable known-by records; malformed/cross-project artifacts and duplicate events cannot mutate canonical state twice; provider completion never sets task success directly.

Validation: adapter double contract suite plus existing app-server fixtures, schema compatibility, session ID collision, duplicate/out-of-order events, fresh-session delivery, approval denial and existing role tests. Recheck current official docs through Context7/openai-docs before implementing API details; run full core checks. Fixtures establish mechanics, not live account acceptance.

Documentation: actual adapter contract/capabilities, artifact versions, compatibility and observed attribution. Skills: milestone-delivery, ponytail-balanced, Context7/openai-docs. Git checkpoint: parity-validated M51 patch; preserve compatibility rollback path.

### M52 — Exclusive writer workspaces and recoverable provider execution

Goal: make provider execution obey task/workspace ownership and survive uncertain acknowledgement.

Subtasks:

- [x] Add Cathulu-owned workspace allocation/lease records; retain existing worktree verification helpers.
- [x] Start writers only in their exact leased linked worktree; coordinators/reviewers are read-only; serialize semantic overlap at task admission using the conservative per-project single-writer lease. This denies all concurrent writers within a project, satisfying the current safety requirement without claiming disjoint task scopes are semantically conflicting; finer-grained writer concurrency remains optional and requires an Owner decision.
- [x] Bind dispatcher-created capabilities and provider attempts to the active lease; tie context receipts to the lease-bound attempt; require WorkResult lease identity to match its WorkOrder; revoke attempt access on terminal completion or replacement (`235c4b9`, accepted in Claude's CC-03 review).
- [x] Add idempotent attempt/reconciliation transitions and interrupt/lease-loss handling without automatic duplicate execution.
- [x] Persist each dispatcher provider attempt against its exact lease before `start_turn`, track turn outcomes and renew confirmed live-turn leases.
- [ ] Reuse M42/M44 operational scenarios for bounded authorized Codex acceptance and document actual gaps.

Affected areas: worktree allocation, dispatcher/provider sessions, capability checks, events and tests. Dependencies: M51; current runtime observation before reload/use. Real external-project tasks need their existing authorization; the assessment creates none.

Acceptance: two writers cannot share a canonical writable worktree or active role lease; aliases/symlinks and wrong Git common-dir are rejected; PM cannot implement changes; each provider turn has a durable attempt bound to its active lease before submission; live turns renew their lease; crash, transport loss or expiry never releases a workspace with an unresolved attempt or silently starts a duplicate run; cleanup preserves dirty Owner data; reports reconcile against the assigned base/path. Current PLAN-015/017 live evidence remains separately tracked.

Validation: disposable Git worktrees, simultaneous claims/lease expiry, active-attempt renewal followed by expiry quarantine, renewal failure, transport loss, direct revoke with an open attempt, wrong path/common-dir, semantic-overlap denial, read-only coordinator, crash before/after submit acknowledgement, failed interrupt, restart recovery, dirty cleanup denial and real scoped acceptance when authorized. Restart recovery now atomically quarantines `created`, `ack_unknown`, and running attempts with in-flight inbox/outbox rows, revokes affected capabilities, and keeps bound workspace leases unavailable. This provider-free checkpoint passed the focused dispatcher suite (12/12), `cargo check --offline --all-targets`, scoped Rust formatting, `git diff --check`, and the full offline all-targets suite (158/158); dispatch, allowed credentials and network were disabled. These fixtures do not establish live M42/M44 acceptance. Rust suites plus matched Windows build only when loading an execution candidate; observe source/build/loaded/live separately.

Documentation: workspace/security/recovery contract, source-backed live evidence and open M42/M44 statuses. Skills: milestone-delivery, ponytail-balanced, Context7/openai-docs. Git checkpoint: validated M52 safety patch before new execution activation.

### M53 — Claude Code feasibility and bounded adapter

Goal: support one bounded Claude task using locally authenticated tooling without API billing as a V1 requirement.

Subtasks:

- [x] Install Claude Code 2.1.302 under the Owner's approval, create the empty Cathulu-owned config, and run `claude doctor` without signing in; no model call was made.
- [x] Register the pure Claude invocation/stream-mapping core and its 27 adapter fixtures as non-activating core code. It does not spawn a process; login, isolation, preflight, and live-task acceptance remain open.
- [ ] Complete Owner login and verify subscription mode, supported Windows permissions, and subscription-compatible isolation from ambient hooks/plugins/MCP/memory/config, including explicit tool permissions and no secret/API fallback. Login remains the Owner's action in a fresh PowerShell session with `ANTHROPIC_BASE_URL` unset. The documented `--bare` subscription limitation must be resolved, not ignored.
- [ ] Implement ClaudeCodeAdapter only for observed supported start/resume/stream/result/interrupt/status behavior; normalize failures without fabricating parity.
- [ ] Validate typed artifacts and scoped MCP/capability access in that harness; require fresh-session context delivery.
- [ ] Exercise one already authorized bounded task with deterministic acceptance and checkpoint/memory reconciliation; if prerequisite fails, keep Claude unavailable and preserve provider-free/Codex use.

Affected areas: Claude adapter, provider preflight, permission envelope/configuration, event normalization, availability state and contract fixtures. Dependencies: M52 and resolved Claude-specific safety/auth prerequisites. Installation, credentials or new provider data disclosure are not automatically authorized by the attachment.

Acceptance: no API key or paid overflow required; provider identity is separate from Cathulu identity; unrelated tools/hooks/config cannot execute; quota/auth/unsupported-feature errors queue/block safely; typed output is reviewed and verified before acceptance; observed installation/build/runtime states are reported separately.

Validation: captured safe protocol fixtures, invalid/truncated output, auth/quota errors, interruption/process exit, permission prompts, ambient-config negative cases, installed-version smoke and one scoped actual-provider task. The current version/empty-config `doctor` result is preliminary installation evidence only; authentication and isolation remain unverified. Run shared adapter/core suites; live use must pass the same policy/workspace/QA checks as Codex. If safe subscription operation is unsupported, record FAIL for that activation and the smallest feasible alternative without silently switching billing.

Documentation: feasibility evidence, exact supported harness/version/features, known gaps, source contract and permissions. Skills: milestone-delivery, ponytail-balanced, Context7 and official Claude docs. Git checkpoint: validated adapter capability; activation requires real preflight success.

### M54 — Availability router and reviewed cross-provider handoff

Goal: choose a suitable available provider and exchange bounded artifacts without shared transcripts.

Subtasks:

- [ ] Persist availability observations, timestamps/source/retry conditions and unknown quota values.
- [ ] Implement policy-first task-shape routing with versioned configurable priors, explainable eligibility exclusions and no provider call for deterministic queries.
- [ ] Add PlanSpec/PlanReview and author/reviewer handoffs only where needed; bind each handoff to task revision, evidence and permissions.
- [ ] Reconcile accepted results atomically/idempotently into tasks/checkpoints/reviewed memory state; keep required review gates in force when a provider is unavailable.
- [x] Register the pure deterministic routing policy and its 31 fixtures as non-activating core code. Availability persistence, policy dispatch, handoff artifacts, and reconciliation remain open.

Affected areas: routing, availability, artifact types, dispatch/reconciliation, evaluation attribution and tests. Dependencies: M53 (both adapter paths validated), M52 isolation, relevant disclosure grants. Do not make M48/M49 wait for this milestone.

Acceptance: ambiguous/UX work initially prefers Claude, bounded executable work initially prefers Codex, with recorded policy reasons; suitable reroute or explicit queue on capacity loss; required cross-family review is never silently waived; Claude->artifact->Codex and Codex->artifact->Claude operate without transcript ingestion; ordinary list/priority/resume commands invoke zero providers.

Validation: deterministic task-shape/risk fixtures, unknown/exhausted/auth/busy states, strict review waiting, explicit Owner priority commands, duplicate handoff and stale-task/HEAD rejection, recovery and bounded live handoffs using already authorized work. Test budgets and evidence distinguish fixture correctness from benefit; full core checks.

Documentation: accepted routing policy/version, availability semantics, outcome attribution, live contract evidence and limitations. Skills: milestone-delivery, ponytail-balanced, applicable provider docs. Git checkpoint: validated explainable routing/handoff slice; no autonomous preference learning.

### M55 — Scoped hierarchy extension and empirical routing proposals

Goal: add only needed logical specialists/dependency scopes and evaluate routing from accepted outcomes.

Subtasks:

- [ ] Introduce a specialist role only for an actual project need; retain stable existing role keys, one active PM and the two-Worker assignment cap across providers.
- [ ] Add a cross-project dependency role only for Owner-declared related scope with source-backed shared contracts; reuse PLAN-003 M10 criteria where applicable.
- [ ] Extend M12 convergence mechanics and M13 comparable-cohort reporting with provider/harness/task-shape attribution, explicit insufficient-evidence states and bounded replan proposals.
- [ ] Evaluate accepted progress, rework, context pressure and user attention; keep promotion advisory/Owner-controlled under existing gates.

Affected areas: roles/constraints, capability scopes, review/event routing, scoped memory and optimization/evaluation projections. Dependencies: M50 for relevant personal/non-code use, M54 for mixed-provider execution, real bounded scope need; sufficient comparable evidence only for calibrated claims.

Acceptance: logical identity survives provider replacement; specialist scopes cannot widen project access; worker caps and parent reviews cannot be bypassed by changing provider/role; convergence candidates are reproducible or unknown; routing suggestions show cohort/task comparability and do not activate policy automatically. No permanent heavyweight sessions.

Validation: role table migration with preserved keys, cross-project resource/packet denial, cap/review/routing invariants, provider replacement and session disclosure, stall/oscillation/negative fixtures, restart/replay and held-out real evidence before calibration claims. Existing core and affected public tests. A source/test pass alone does not complete live or benefit criteria.

Documentation: role/capability ADR where needed, updated operating contract, scoped graph evidence, evaluation limitations and PLAN-003 links. Skills: milestone-delivery, ponytail-balanced, Graphify where current/suitable; release-readiness for any meaningful rollout. Git checkpoint: one capability per validated patch; no blanket hierarchy expansion or learned-policy promotion.

## Final integration validation

Prove two independently useful paths: (1) provider executables absent -> deterministic tasks/checkpoint/restart/resume; (2) authorized WorkOrder -> selected available provider -> exclusive workspace -> typed report -> deterministic QA and required review -> idempotent reconciliation -> updated checkpoint/memory receipt. Exercise provider replacement, exhausted allowance, uncertain submission, rejected review, missing context, stale source, conflicting workspace and unresolved Owner approval. Record actual loaded version and environment for live checks. Preserve and reconcile M42/M44 evidence rather than bypass it.

## Rollback / recovery

M47: remove only this turn's added sections/new documents using its patch, preserving preexisting changes. Later milestones use consistent verified SQLite backups and matched binaries, versioned artifact readers and narrowly disableable execution paths. New schema cannot be opened by the old version-18 binary. Restore only through a reviewed data recovery plan that accounts for post-backup writes. Never reset the dirty checkout, clean unknown worktrees or copy an active SQLite file without its WAL/backup semantics. Details: assessment sections 17, 18 and 22.

## Progress and implementation discoveries

- M47 source inspection/design and local documentation validation passed; executable pivot work has not started. The Owner-authorized source checkpoint was committed, pushed and submitted in [PR #33](https://github.com/jmsb505/cathulu/pull/33); GitHub's PR state records the final merge outcome.
- SQLite, FTS5, logical identity, report reviews and durable event routing already exist; the pivot should extend them.
- Existing optional provider adapters are specialist APIs, not agent providers.
- Current writer-workspace allocation and fresh-session known-by semantics are migration gaps.
- Claude and Graphify were not found on inspected PATH; no local graph is available. Context7 provider lookups returned monthly-quota errors; official documentation was used.
- Claude `--bare` is unsuitable as an assumed subscription-preserving isolation shortcut. Actual supported controls remain a dependent feasibility gate.
- No app tests/build/reload/provider invocation/dispatch/data migration or external write is claimed by M47. Git integration remains pending.

## M52 implementation progress (2026-10-25)

CX-05 establishes a non-activating schema v22 foundation: exclusive leased workspaces, append-only lease events, capability/attempt lease binding, explicit credential-source allow-lists, and one active provider session per logical role across providers. Writers must claim a clean linked worktree in the registered repository's Git common directory; Project Manager/coordinator access is read-only. Claude's design-only assessment in CC-03 §18 confirms that the per-project single-writer lease satisfies M52 semantic-overlap safety by rejecting every concurrent same-project writer at admission. This conservative rule provides no same-project parallelism; the packet's `allowed_scope` is not persisted on the lease or enforced path-by-path by the workspace sandbox. A v21 database with concurrent active provider sessions for one role preserves both history rows but marks the ambiguous sessions `unknown` until a later explicit provider session is recorded.

Every dispatcher turn now records its provider session and an attempt bound to the exact workspace lease and role capability before calling `start_turn`; the prompt is stored only as a hash. Turn events move the attempt through submitted/running to a terminal outcome. The dispatcher renews active workspace leases to a 60-second horizon every 10 seconds. Transport or renewal loss marks the attempt `needs_reconcile`, revokes the capability, quarantines the lease and preserves the role/path lock. If the start acknowledgement is uncertain, the open attempt remains bound to its renewed lease, which expires into quarantine rather than releasing the lock. No automatic retry, filesystem cleanup, real provider call or live-store migration is performed. Session replacement across providers is refused while any attempt for that logical role remains unresolved.

This is not M52 completion: commit `650ad6b` holds product dispatch off regardless of environment flags, correcting the prior default-on behavior. The dispatcher admission boundary validates exact Worker/Project Manager leases, capabilities, permissions and credential sources before provider starts; Main inbox starts remain blocked. Claude accepted the `d851035` boundary as fail-closed, accepted amendments A–D in `3444829`, R1 in `6153cdc`, and B1/D1 in `67266ab`: a revoked capability remains unusable, but its persisted lease binding permits reuse only with the same currently active lease and a newly issued capability. Claude accepted the recovery corrections at `ecf7d11` and the conservative project-wide overlap rule as sufficient for M52 safety in CC-03 §18. Codex added failure-path, direct-revoke and simultaneous-claim tests; the current offline all-targets suite passed 152/152, with all-target checking, formatting and `git diff --check` passing while dispatch and admission remained disabled. Authorized M42/M44 live evidence remains open. No interim exception is allowed; the gate stays false and only the Owner may decide future activation. See [ADR-0024](../decisions/ADR-0024-workspace-leases-and-recovery.md) and the CX-05 handoff for exact implementation and test evidence.

Claude's read-only review of `c4f0fd8` accepted the recovery transaction, exact-set validation, exact replay and role denial, then required R-A/B/C before live use. Commit `dd6bfc8` puts the Windows app-server in a kill-on-close Job Object and fails startup if containment cannot be established, and adds identity-mismatch lock retention, invalid-transition, MCP-handler denial and transaction-rollback coverage. Follow-up `ecf7d11329383fedae2b73dc6583100c5c9fafa8` reads project/worktree paths and stored identity hashes from the same immediate transaction snapshot used for recovery; Claude accepted this exact head and its documentation. The current CX-05 packet adds renewal-failure and transport-loss quarantine tests, direct revoke with an open attempt, and a simultaneous writer-claim test. With both dispatcher flags and admission policy disabled, `cargo fmt --all -- --check`, `git diff --check`, and `cargo check --offline --all-targets` pass; the full offline all-targets suite passes 152/152. Windows process containment still has a narrow spawn-to-assignment gap; non-Windows platforms have no equivalent here; the provider-stop confirmation remains an operator assertion. Dispatch remains false, and M52 is still open.

Claude's review of `a83ccf9` conditionally accepted the non-activating foundation and required an explicit distinct-session Claude-to-Codex replacement test. Follow-up commit `ac3a3b2` verifies that an unresolved attempt blocks replacement, reconciliation permits it, the prior Claude session becomes inactive, and its completed attempt remains attached to that session. Claude's audit of terminal resolution, release/revocation and claim arbitration is complete in CC-03 §17. The Owner directed dispatch to remain off without an interim exception; runtime gate commit `650ad6b` changes prior default-on dispatch to fail-closed. Claude accepted admission commit `d851035` with amendments A–D, R1 at `6153cdc`, B1/D1 at `67266ab`, the recovery fixes at `ecf7d11`, and the project-wide overlap rule for current M52 safety in CC-03 §18. Codex's current safety-test packet covers renewal-failure, transport-loss, direct-revoke and simultaneous-claim cases. Authorized M42/M44 live evidence remains open.

The current CX-05 follow-up keeps attempts `created` until `start_turn` succeeds, then advances the attempt and its selected context receipts together; terminal attempt transitions revoke only the capability attached to that exact attempt. `WorkResult` validation also requires its lease ID to match the parent `WorkOrder`. These are source/fixture guarantees; the broader provider artifact pipeline and live M42/M44 path remain incomplete, so the combined M52 subtask stays open.

Recovery retains the accepted conservative transition rule: a `created` attempt has no durable acknowledgement or turn identity, so operator reconciliation cannot label it `completed`. After confirming that provider execution stopped, the operator records `failed` or `blocked` and explains any known outcome and uncertainty in the reason. This avoids fabricating a success state when the durable record cannot prove which turn ran.

## Completion evidence

The evidence below records the initial documentation-only assessment. Subsequent repository synchronization, full regression/build checks and test-fixture repairs are recorded separately in the [integration record](../readiness/repository-integration-2026-10-25.md); they do not implement M48-M55 or establish live-provider readiness.

Local M47 acceptance evidence (2026-10-25):

- Checked 10 affected Markdown files and all 371 local links/anchors; no missing targets, unbalanced fences or missing final newlines.
- Verified exactly 31 sequential assessment sections and nine milestones M47-M55, with goal, work, dependencies, acceptance, validation, documentation, skills and Git checkpoints.
- Parsed the kickoff JSON with 16 unique claim IDs and 11 unique question IDs; required claim fields and Boolean blocking flags passed. Historical claim/question records remain present, with explicit supersession for changed intent.
- The active first-slice gate contains all 11 required criteria and aggregates to PASS for M48-M49. The checkpoint records whole-pivot live readiness as UNKNOWN; dependent provider/personal/UI questions remain open.
- Compared the pre-write SHA-256 baseline of 118 files with the final tree: all 110 files outside the eight intentionally edited preexisting documentation files stayed byte-identical. Exactly three new documents were added: this plan, the assessment and ADR-0022. Reviewed targeted patches preserve preexisting runtime/policy changes; no source/UI/test/config file changed in this turn.
- Targeted `git diff --check` and explicit whitespace/fence checks for new documents passed. The final documentation check was rerun after recording evidence.
- No Rust/frontend tests, build, live database upgrade, reload, install, provider call, external-project dispatch, stage/commit/push or release was performed. Those do not follow from documentation checks. M48-M55 have no implementation or acceptance evidence yet; M42/M44 operational status is unchanged by this work.
