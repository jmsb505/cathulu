# PLAN-006 — Bounded per-project agent hierarchy

Status: M20 role contract complete; runtime and implementation milestones pending
Created: 2026-10-21
Last updated: 2026-10-21

## Objective

Make Cathulu-coordinated work follow an observable hierarchy where Main Overseer coordinates only, one Project Manager owns each active project, and each Project Manager dispatches no more than two implementation workers per assignment. Require every role to use the existing Cathulu memory lifecycle, MCP tools, and evidence records.

## Motivation

The owner reports that an orchestrator appears to be doing the repository work itself and that the open Cathulu app shows no corresponding activity. The current system intentionally records native Codex handoffs but does not spawn agents or enforce a hierarchy. The current snapshot has no running run, delegation, or agent activity for the expected workflow, so the dashboard has no evidence from which to show active agents. Fixing the graph alone would not fix the operating model.

The new hierarchy applies to independent project work. PULSO, Mecanografica proposals, and Cathulu are the current focus. The user has no task that crosses those repositories; cross-project handoff testing remains deferred and does not block this plan.

## Current state

- M6-M9 in [PLAN-002](PLAN-002-phase3-codex-delegation.md) provide typed delegation packets, explicit dispatch records, structured reports, worktree verification, and optional subagent hook correlation.
- `prepare_delegation` records a request only; `mark_delegation_dispatched` records a native Codex handoff. Cathulu has no agent-spawn API.
- Delegations have a `worker` or `sub_overseer` role but do not yet record a parent delegation. No deterministic one-manager-per-project or two-worker-per-assignment admission limit exists.
- Memory records, search, known-by filtering, HOT budgets, explicit context transfers, and artifact references already exist. Current delegation packets can reference memory, but memory activation is not yet an enforced prerequisite for every agent.
- The desktop surface shows actual runs, delegations, and observed subagent hooks; it does not infer missing hierarchy or display live Codex internals.
- Official Codex docs say custom agents inherit parent MCP servers/tools, but nested spawn availability is not guaranteed for every client. The runtime capability remains to be tested.

## Desired state

- Main Overseer creates/monitors project runs and delegates each active project to exactly one Project Manager; it does no implementation work.
- Each Project Manager is scoped to one registered project and may dispatch at most two worker agents total per manager assignment.
- All agent roles verify Cathulu MCP availability and make scoped memory retrieval/context transfer a required part of their activation and handoff.
- SQLite remains authoritative for project/run/delegation/memory/transfer/QA state. Hierarchy relationships and caps are deterministic and auditable.
- The dashboard shows Main → Project Manager → Worker relationships and animates only actual recorded active agents. Missing or unsupported telemetry is labeled unknown.
- Every comparison records the exact hierarchy policy/workflow and project independently; no cross-project dependency is assumed.

## Scope

- Define role boundaries and the required memory workflow.
- Verify nested native Codex agent creation and Cathulu MCP availability in the actual desktop client before relying on them.
- Extend current delegation and hook correlation only where required for parent-child hierarchy and caps.
- Make agent activation require a transfer receipt or an explicit, recorded `no relevant memory` outcome, reusing existing memory and transfer authorities.
- Project only actual hierarchy and lifecycle evidence into the read-only dashboard.
- Validate with an independent real project task when one is available; this does not require cross-repository work.

## Out of scope

- Replacing Codex with a Cathulu model/tool runtime.
- Having Cathulu spawn, steer, or terminate Codex agents.
- Main Overseer implementing project changes when agents or telemetry are unavailable.
- Cross-repository handoffs, synthetic cross-project dependencies, or changes to PULSO/Mecanografica source repositories in this milestone.
- Rebuilding memory tables, adding a second knowledge store, transcript replay, or broad semantic indexing.
- Enabling Jev/ScaleDown as active workflow controllers or changing model/effort preferences.
- Changing the owner's global Codex config without an explicit, separate approval.

## Source-of-truth impact

- [ADR-0009](../decisions/ADR-0009-bounded-project-agent-hierarchy.md) records accepted role and capacity boundaries.
- [Agent operating contract](../agent-operating-contract.md) contains the portable role instructions and Cathulu memory workflow.
- [Project definition](../project-definition.md), [architecture](../architecture.md), and [roadmap](../roadmap.md) describe intended and actual behavior separately.
- `src/migrations.rs`, `src/delegation.rs`, `src/telemetry.rs`, `src/mcp.rs`, and `src/dashboard.rs` may change only after runtime feasibility and current schema review.
- `docs/local-mcp.md` will document the exact per-role tool sequence and any owner-controlled hook requirements.
- Dashboard implementation remains read-only and uses the existing snapshot authority.

## Existing decisions and constraints

- ADR-0001 keeps Codex as the conversation and execution system and Cathulu as the local evidence layer.
- ADR-0003 keeps SQLite authoritative and HOT/WARM/COLD, provenance, supersession, known-by state, and transfer ledger semantics independent.
- ADR-0004/0005 make hooks optional privacy-minimized observations, not complete audit or success proof.
- ADR-0007 keeps specialists optional and does not change the agent hierarchy.
- ADR-0008 keeps the dashboard read-only and evidence-based.
- M10 in PLAN-003 remains gated on naturally recurring cross-project reconstruction evidence. This plan is per-project delegation, not cross-project scope activation.
- All current repository work remains independent unless the owner states otherwise.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`, `ponytail-balanced`
- `openai-docs` and official Codex subagent/hooks documentation for runtime capabilities
- Existing local Cathulu MCP tools, resources, SQLite migrations, and read-only dashboard
- No model/provider, remote service, or new UI library is required

## Dependencies

- Existing M6-M9 run/delegation/report/telemetry contracts.
- A connected local Cathulu MCP server in the Main Overseer session and inherited tool availability for nested agents.
- Runtime proof that a Project Manager can spawn child workers in the selected Codex client. If unsupported, implementation pauses at the capability boundary without falling back to Main implementation.
- Owner-controlled hook review/trust if hook-based lifecycle telemetry is used.

## Risks and unknowns

| Risk or unknown | Mitigation |
| --- | --- |
| Nested subagent spawning may be unavailable in a target client | Run a small, non-destructive capability probe before implementing hierarchy-dependent runtime assumptions. Keep the fallback explicit and do not flatten roles. |
| Codex hook metadata may not identify who spawned a nested worker | Require explicit delegation-to-hook association or supported event correlation; display unknown relationships when linkage is absent. |
| Hooks and role prompts cannot fully prevent root behavior | Combine role instructions, deterministic Cathulu admission checks, clear tool affordances, and actual telemetry. Do not describe prompt-only guidance as hard security enforcement. |
| One PM per project can create a queue for simultaneous requests | Keep separate task state and report queue/wait status; do not start a second PM while the project slot is occupied. |
| Memory steps add startup cost | Reuse HOT snapshots, known-by hashes, and transfer deltas; measure added context and latency. |
| Missing owner-level hook trust hides activity | Keep runs and domain state observable through explicit MCP updates; dashboard labels hook coverage as unavailable rather than idle when evidence is missing. |

## Milestones

### M20 — Role contract and memory protocol

Goal: codify the owner-approved role boundaries, capacity limits, and required Cathulu memory workflow.

Subtasks:

- [x] Define Main Overseer, Project Manager, and Worker responsibilities and capacity limits.
- [x] Define the mandatory memory retrieval, transfer, and provenance workflow for all roles.
- [x] Record that native hierarchy spawning and MCP availability require runtime verification; do not claim them as implemented.

Affected areas: `docs/agent-operating-contract.md`, ADR-0009, project definition, architecture, roadmap, local MCP guide.

Dependencies: Owner-approved hierarchy decision; existing M6-M9 and Phase 1 memory tools.

Acceptance criteria:

- [x] Main is coordinator-only; Project Managers plan and supervise one project; Workers perform scoped project work.
- [x] The limit is two total workers per manager assignment, including replacements.
- [x] Every role is required to use scoped Cathulu memory, context transfers, provenance, and deterministic QA.
- [x] No cross-project handoff is inferred; M10's cross-project evidence gate remains separate.
- [x] Documentation distinguishes intended behavior from verified runtime behavior.

Validation:

- [x] Reviewed current project definition, architecture, Phase 3 delegation plan, memory contract, and local MCP guide.
- [x] `git diff --check` and targeted review of internal document links and consistent role/limit wording.

Documentation updates: ADR-0009, agent operating contract, project definition, architecture, roadmap, local MCP guide, and this plan.

Applicable specialized skills: `implementation-plan`, `milestone-delivery`, `ponytail-balanced`.

Expected Git checkpoint: documentation checkpoint on a dedicated milestone branch.

### M21 — Native hierarchy runtime capability proof

Goal: establish which manager-to-worker spawning, MCP inheritance, and lifecycle correlation paths the current Codex client actually supports.

Subtasks:

- [ ] Verify a Project Manager can spawn up to two child workers in the current target Codex surface.
- [ ] Verify child workers inherit the local Cathulu MCP server and can read/transfer project memory.
- [ ] Verify the actual hook payloads/correlation available for Main → Project Manager → Worker without storing transcripts or prompt/tool bodies.
- [ ] Record whether owner-controlled hooks need configuration; do not edit global config in this milestone.

Affected areas: Codex runtime probe, `docs/local-mcp.md`, PLAN-006 evidence.

Dependencies: M20; existing M6-M9 tools; configured local Cathulu MCP server.

Acceptance criteria:

- [ ] Main performs no implementation tool work in the probe.
- [ ] One project has one Project Manager, and no more than two worker children are started.
- [ ] Each role proves Cathulu MCP visibility by reading the same project snapshot and completing a memory-transfer or explicit no-relevant-memory path.
- [ ] Results distinguish documented support from observed runtime support. No global settings are changed.

Validation:

- [ ] Manual, non-destructive Codex client probe with no repository edits.
- [ ] Record resulting Cathulu event IDs and limitations; do not use synthetic cross-project handoffs.

Documentation updates: update local MCP instructions and this plan with observed evidence.

Applicable specialized skills: `milestone-delivery`, `openai-docs`.

Expected Git checkpoint: runtime-capability checkpoint on a dedicated milestone branch.

### M22 — Hierarchy-aware delegation and deterministic limits

Goal: represent project-manager parentage and enforce one PM plus at most two workers per PM assignment in Cathulu's existing SQLite authority.

Subtasks:

- [ ] Add an additive migration linking child worker delegations to a parent Project Manager delegation without changing legacy rows.
- [ ] Validate the parent has `sub_overseer` role, same project/task/run, and is dispatched before a worker child can be dispatched.
- [ ] Enforce at most one active Project Manager assignment per project and at most two total worker delegations per manager assignment, including replacements.
- [ ] Make caps transactional and idempotent under concurrent MCP requests; cancelled/finished assignments release only the project slot, not the two-worker historical count for that assignment.
- [ ] Keep existing role/status records and old resources readable; append lifecycle evidence for rejected over-cap attempts without partial delegation state.

Affected areas: `src/migrations.rs`, `src/delegation.rs`, `src/mcp.rs`, delegation resources, and schema migration tests.

Dependencies: M21 runtime capability proof; schema compatibility review.

Acceptance criteria:

- [ ] Old delegation records migrate without inferred parents.
- [ ] A second PM for the same occupied project is rejected deterministically.
- [ ] A third worker under one PM assignment is rejected, whether workers are active or completed.
- [ ] Cross-project parent links, wrong role, stale parent, or mismatched run/task are rejected transactionally.
- [ ] Existing M6-M9 prepare/report/review/worktree flows remain compatible.

Validation:

- [ ] Migration/reopen validation on a disposable SQLite database.
- [ ] Domain and public MCP checks cover caps, races, lifecycle, backward compatibility, and parent integrity.
- [ ] `cargo fmt --all -- --check`, focused Rust/MCP tests, workspace checks, and `git diff --check`.

Documentation updates: architecture, local MCP, roadmap, this plan, and migration/compatibility notes.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `openai-docs`.

Expected Git checkpoint: validated additive-schema milestone branch and coherent commit.

### M23 — Memory-gated agent activation and handoff

Goal: require every Project Manager and Worker activation to be traceable to scoped Cathulu memory retrieval and transfer.

Subtasks:

- [ ] Reuse current memories, known-by state, freshness, scope budgets, artifact links, and transfer ledger; add no duplicate memory authority.
- [ ] Add a compact agent-activation receipt referencing transferred memory IDs/hashes or a structured no-relevant-memory disposition.
- [ ] Require the receipt in a delegation packet before recording dispatch; enforce project scope and active HOT budget.
- [ ] Record each receiver's logical scope so unchanged known content is omitted and changed knowledge is a delta.
- [ ] Require worker discoveries to be submitted as provenance-linked evidence/candidates and reviewed before canonical promotion.
- [ ] Reject attempts to use a model summary as authoritative state or to use specialist output to delete/overwrite memory.

Affected areas: `src/memory.rs`, `src/delegation.rs`, migrations only if required, `src/mcp.rs`, context-transfer resources, and `docs/local-mcp.md`.

Dependencies: M20; M21 runtime capability; M22 parent/child identity; existing Phase 1 memory schema.

Acceptance criteria:

- [ ] Each dispatched PM/worker has a transfer receipt or explicit no-relevant-memory record.
- [ ] Stale, COLD, superseded, foreign-scope, or over-budget memory cannot be injected as current context.
- [ ] Unchanged known content is not resent; changed content records a delta/replacement.
- [ ] Canonical promotion preserves source evidence and does not delete unique provenance.

Validation:

- [ ] MCP and persistence checks exercise first delivery, repeat delivery, delta delivery, no-memory outcome, stale memory, and worker candidate provenance.
- [ ] Run existing memory lifecycle and delegation checks; build/check the Rust workspace and verify `git diff --check`.

Documentation updates: local MCP role sequence, architecture, ADR-0009 consequences, and this plan.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`.

Expected Git checkpoint: validated memory-activation milestone branch and coherent commit.

### M24 — Project-scoped live activity graph

Goal: show truthful, role-labeled Main → Project Manager → Worker activity for independent project runs.

Subtasks:

- [ ] Extend the existing read-only snapshot with explicit hierarchy relationships and coarse lifecycle/tool activity supported by existing hooks.
- [ ] Correlate observed start/stop events only through stable hashes and explicit delegation references; retain privacy minimization.
- [ ] Show one project graph at a time, with accurate active/blocked/stopped/idle states and no inferred cross-project edges.
- [ ] Add a clear telemetry-unavailable state that distinguishes missing hook data from a proven idle agent.
- [ ] Keep the compact floating monitor focused on each project's state; show real hierarchy/activity only when expanded.

Affected areas: `src/telemetry.rs`, `src/dashboard.rs`, `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, Tauri snapshot types, and UI documentation.

Dependencies: M21 runtime events, M22 hierarchy relationships, M23 activation evidence.

Acceptance criteria:

- [ ] A recorded run renders one Main root and only real, linked Project Manager/Worker nodes.
- [ ] A completed or failed child remains visible as history without pulsing as active.
- [ ] Missing telemetry never creates an active node or percentage.
- [ ] Independent projects have separate graphs, tasks, and memory scopes.
- [ ] The dashboard remains read-only.

Validation:

- [ ] Frontend build and Rust snapshot checks.
- [ ] Windows manual smoke with one recorded hierarchy, missing telemetry, stopped/blocked run, and multiple independent projects.
- [ ] Reduced-motion/compact monitor behavior and `git diff --check`.

Documentation updates: ADR-0008 compatibility note if necessary, architecture, roadmap, README, and this plan.

Applicable specialized skills: `milestone-delivery`, `ui-ux-research`, `redesign-existing-projects`, `impeccable`, `apple-design`, `emil-design-eng`.

Expected Git checkpoint: validated dashboard hierarchy milestone branch after Windows native smoke.

### M25 — Independent project-run verification

Goal: validate the hierarchy on an actual independent task in one active project and use evidence to decide rollout.

Subtasks:

- [ ] Select a real owner-approved task in PULSO or Mecanografica proposals when available.
- [ ] Record Main, one PM, up to two workers, memory transfers, QA, rework, context size, latency, and human intervention.
- [ ] Compare the observed result with ordinary single-agent baseline evidence where task classes are comparable.
- [ ] Keep other repositories idle unless they have independent owner-approved work.

Affected areas: real project's existing task/run state, Cathulu run artifacts, PLAN-006 evidence.

Dependencies: M21-M24; an actual task and normal required QA checks.

Acceptance criteria:

- [ ] The agent hierarchy and all Cathulu memory operations are visible in real run evidence.
- [ ] No worker crosses repository/task scope.
- [ ] Quality is established only by mandatory deterministic checks and owner review.
- [ ] If no representative task exists, the product implementation may complete on mechanical evidence, while this evaluation remains explicitly pending.

Validation:

- [ ] Review Cathulu resources and dashboard snapshot after run completion.
- [ ] Preserve run/artifact provenance and record missing metrics as unknown.

Documentation updates: this plan and the evidence-backed roadmap status.

Applicable specialized skills: `milestone-delivery` and any task-specific repository skills.

Expected Git checkpoint: separate evaluation record; no cross-repository synthetic task.

## Final integration validation

- Confirm the target Codex client can perform the intended nested spawn and that each child can access the already configured Cathulu MCP server.
- Migrate and reopen a disposable copy of the SQLite store; verify old rows and current event history remain intact.
- Exercise the one-PM/two-worker caps and memory transfer receipt rules through public MCP operations.
- Build/check Rust, frontend, and Tauri surfaces affected by the selected milestone; run project-required deterministic QA on any actual task.
- Review the read-only dashboard with real evidence and confirm unsupported runtime activity remains visibly unknown.
- Run `git diff --check`; record exact command results and manual limitations.
- Never change global Codex configuration or send work to another repository as a side effect of validation.

## Rollback / recovery

Use additive migrations only. Preserve legacy delegation records without guessed parent relationships and preserve existing transfer/memory history. If new admission rules cause false blocks, disable the hierarchy gate using a documented local migration/config flag while retaining append-only evidence; never delete source delegation or memory records. Revert dashboard projections independently from the underlying records. Restore native behavior only through the owner's Codex controls; Cathulu does not spawn/terminate agents or modify global settings.

## Progress

- [x] Reviewed current delegation, memory, telemetry, dashboard, Phase 3 plan, and roadmap.
- [x] Recorded the user-approved Main/Project Manager/Worker role contract and memory workflow.
- [x] Confirmed official docs describe MCP inheritance for configured Codex subagents; nested spawning remains a client runtime acceptance gate.
- [x] M20 — role contract and memory protocol.
- [ ] M21 — runtime capability proof.
- [ ] M22 — hierarchy-aware delegation and caps.
- [ ] M23 — memory-gated activation and handoff.
- [ ] M24 — project activity graph.
- [ ] M25 — independent real-task verification when a task exists.

## Implementation discoveries / decisions

- 2026-10-21: Current dashboard state has no running Cathulu run, saved delegation, or agent activity for the workflow the owner expected to see. The UI cannot display activity absent stored evidence.
- 2026-10-21: Current M6-M9 deliberately prepare and record native Codex handoffs but do not spawn agents. Existing role fields do not encode parent-child delegation relationships.
- 2026-10-21: Official Codex subagent docs state that custom agents inherit MCP servers from the parent configuration; the reviewed docs do not promise nested spawn support uniformly across clients. Validate the current desktop client before relying on it.
- 2026-10-21: The limit is interpreted strictly as two worker agents total for a Project Manager assignment, including replacements. Revisit only if the owner changes the rule.
- 2026-10-21: The current workflow uses independent project scopes; cross-project handoff testing remains deferred.

## Completion evidence

M20 documentation delivery (2026-10-21): added the normative Main/Project Manager/Worker operating contract and Cathulu memory workflow; accepted ADR-0009; updated the project definition, architecture, roadmap, and local MCP guide. Documentation review confirms coordinator-only Main behavior, one manager per project assignment, a strict two-worker total per assignment, independent project scopes, and no invented runtime capability. Nested spawning, deterministic enforcement, memory receipts, dashboard hierarchy, and real-task evaluation remain pending under M21-M25.


