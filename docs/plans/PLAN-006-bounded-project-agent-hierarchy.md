# PLAN-006 — Bounded per-project agent hierarchy

Status: M20-M23 are implemented and validated on Windows. M24's truthful activity graph, portfolio-wide Main view, project-focus filter, per-project monitor, and automated checks are implemented; native Windows visual smoke and review are still required. M25 remains gated on an owner-selected real task. M21 proved native PM-to-Worker spawning and Cathulu MCP reachability, with role-specific memory-tool availability still open and no hook events observed in that probe.
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
- Delegations use the existing `worker` or `sub_overseer` roles. Migration 10 adds an optional parent link and a separate Project Manager assignment table, leaving legacy records unparented. SQLite admission now reserves one active PM slot per project and limits each PM assignment to two worker delegations total.
- Memory records, search, known-by filtering, HOT budgets, explicit context transfers, and artifact references already exist. Current delegation packets can reference memory, but memory activation is not yet an enforced prerequisite for every agent.
- The desktop surface shows actual runs, delegations, and observed subagent hooks; it does not infer missing hierarchy or display live Codex internals.
- The M21 probe demonstrated nested PM-to-Worker spawning in the current client and Cathulu MCP reachability from Main, PM, and Worker. Memory tool availability differed by role, and no hook events surfaced; see the M21 evidence below.

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
- Make agent activation require a successful bounded memory search plus transfer references or an explicit, recorded `no relevant memory` outcome, reusing existing memory and transfer authorities.
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

### M21 — Native hierarchy runtime capability proof — complete with recorded limitations

Goal: establish which manager-to-worker spawning, MCP inheritance, and lifecycle correlation paths the current Codex client actually supports.

Subtasks:

- [x] Verify a Project Manager can spawn child Workers in the current target Codex surface. The probe started one Worker; the two-worker ceiling itself remains to be enforced and tested in M22.
- [x] Verify Main, Project Manager, and Worker can reach Cathulu MCP. The probe exposed role-specific differences in available memory tools; consistent memory-operation availability remains for M23.
- [x] Check for hook events/correlation in the probe without storing transcripts or prompt/tool bodies. No hook metadata surfaced, so no hierarchy association was inferred; actual hook correlation remains unverified for M24.
- [x] Record hook configuration as an open question; no global config was changed.

Affected areas: Codex runtime probe, `docs/local-mcp.md`, PLAN-006 evidence.

Dependencies: M20; existing M6-M9 tools; configured local Cathulu MCP server.

Acceptance criteria:

- [x] Main performs no repository implementation work in the probe.
- [x] One Project Manager spawned one Worker, within the two-worker maximum. This proves one nested child is available; enforcement and a second-child boundary test belong to M22.
- [x] Each role reached Cathulu MCP. Main read dashboard/HOT/WARM; PM read `cathulu://dashboard`; Worker successfully called `read_memories`. HOT and WARM were empty. Main passed “no relevant memory” to PM and recorded it in the run. `prepare_context_transfer` could not encode this empty result because it requires a memory ID.
- [x] Results distinguish observed support from gaps. No global settings were changed; no hook event or hierarchy edge was fabricated.

Validation:

- [x] Manual, non-destructive Codex client probe with no repository edits; Cathulu run 4 is `verified_success`.
- [x] Record Cathulu delegation IDs 1 (Main → PM, role accepted as `sub_overseer`) and 2 (PM → Worker), including the limitations below; no synthetic cross-project handoffs were used.

#### M21 evidence and remaining gaps

- Main spawned exactly one Project Manager (`/root/m21_project_manager`); that PM spawned exactly one Worker (`/root/m21_project_manager/m21_probe_worker`). Cathulu delegations 1 and 2 were marked dispatched, and the Worker report was recorded.
- Main successfully read the dashboard and scoped HOT/WARM memory. The PM verified `cathulu://dashboard`, but `read_memories` was unavailable in its tool context. The Worker successfully called `mcp__cathulu__read_memories`.
- HOT and WARM results were empty. “No relevant memory” was passed to the PM and recorded in run 4. `prepare_context_transfer` requires a memory ID, so it cannot represent an empty transfer; M23 must provide a structured no-relevant-memory activation receipt.
- No hook events or actual hook metadata surfaced. No worker association or Main → PM → Worker edge was inferred from agent names alone. Hook setup/correlation remains an M24 evidence requirement, not a blocker for M22's schema and capacity work.
- The current delegation role accepted `sub_overseer` for the PM; M22 must preserve this legacy role while deciding how to expose the Project Manager concept in hierarchy-aware records.
- The run's `verified_success` classifies the Cathulu probe run only; it is not evidence of project repository work or QA.

Documentation updates: update local MCP instructions and this plan with observed evidence.

Applicable specialized skills: `milestone-delivery`, `openai-docs`.

Expected Git checkpoint: runtime-capability checkpoint on a dedicated milestone branch.

### M22 — Hierarchy-aware delegation and deterministic limits

Goal: represent project-manager parentage and enforce one PM plus at most two workers per PM assignment in Cathulu's existing SQLite authority.

Subtasks:

- [x] Add migration 10 with an optional self-referencing parent link and a Project Manager assignment table; legacy delegation rows remain unparented and are not backfilled as assignments.
- [x] Validate that a worker parent is a hierarchy-aware `sub_overseer` PM with the same project/task/run and an active assignment. A worker may be prepared before its PM dispatch, but its dispatch is rejected until the PM is dispatched.
- [x] Preserve `sub_overseer` as the stored PM role. The explicit assignment row represents the hierarchy-facing Project Manager concept; existing records are not silently reinterpreted.
- [x] Enforce one active PM assignment per project and at most two child worker delegation records per PM assignment, including prepared, cancelled, and replaced workers.
- [x] Make checks transactional under concurrent MCP requests. PM completion releases the project slot only after each child has reported or been cancelled; cancellation also releases it after the same unfinished-child guard. The worker lifetime count remains attached to the closed assignment.
- [x] Keep existing role/status records and resources readable; write `delegation_admission_rejected` run events for cap and parent-dispatch rejections without inserting partial delegation state.

Affected areas: `src/migrations.rs`, `src/delegation.rs`, `src/mcp.rs`, delegation resources, and schema migration tests.

Dependencies: M21 nested-spawn capability proof; schema compatibility review. Hook correlation is not required for deterministic hierarchy admission, but remains required before M24 can claim observed live lifecycle telemetry.

Acceptance criteria:

- [x] Old delegation records migrate without inferred parents or Project Manager assignments.
- [x] A second PM for the same occupied project is rejected deterministically and the rejection is audited.
- [x] A third worker under one PM assignment is rejected, including when earlier workers are cancelled/replaced; concurrent final-slot requests admit only one worker.
- [x] Cross-project parent links, a worker supplied as the parent, and worker dispatch before parent dispatch are rejected transactionally. Completed/cancelled PM assignments cannot accept workers.
- [x] Existing M6-M9 prepare/report/review/worktree flows remain compatible; optional null parentage preserves legacy Worker behavior.

Validation:

- [x] Migration/reopen validation on disposable SQLite databases confirms version-ten upgrade and legacy parentless records.
- [x] Public MCP checks cover project slot reservation/release, concurrent PM and Worker admission, cancelled replacement accounting, parent dispatch order, completion guards, backward compatibility, and parent integrity.
- [x] `cargo fmt --all -- --check`, focused Rust/MCP tests, workspace checks, and `git diff --check` pass. The final offline workspace run passed 71 tests across the core, context extraction, delegation, MCP, memory, optimization, and desktop suites; one existing desktop integration test is ignored by design.

Documentation updates: architecture, local MCP, roadmap, ADR-0009, this plan, and migration/compatibility notes.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `openai-docs`.

Expected Git checkpoint: validated additive-schema milestone branch and coherent commit; publish only after required checks pass.

#### M22 validation evidence

- `cargo fmt --all -- --check` — passed.
- `cargo test --offline --test delegation_lifecycle` — 9 passed, including two concurrent cap-admission tests.
- `cargo test --offline --workspace` — 71 passed, 1 existing desktop integration test ignored by design; no failures.
- `git diff --check` — passed.
- The initial full-workspace attempt hit Windows `Access is denied` in the `aws-lc-sys` build script under the default sandbox. The same offline command passed when rerun with the build permission required by that script.
- Test fixtures were isolated; no PULSO, Mecanografica proposals, Cathulu, or other registered project repository was accessed.

### M23 — Memory-gated agent activation and handoff — complete with runtime limitations recorded

Goal: require every Project Manager and Worker activation to be traceable to scoped Cathulu memory retrieval and transfer.

Subtasks:

- [x] Reuse current memories, known-by state, freshness, scope budgets, artifact links, and transfer ledger; add no duplicate memory authority.
- [x] Add append-only, revisioned activation receipts referencing a successful bounded memory search and transferred memory IDs/hashes, already-known refs, or a structured no-relevant-memory disposition.
- [x] Require the latest ready receipt before dispatching hierarchy-aware PMs and child Workers; reject missing/incomplete receipts and append a run admission event.
- [x] Record a successful `search_memory_candidates` event tied to delegation run and exact logical receiver. A boolean claim or generic Cathulu resource read alone cannot satisfy readiness.
- [x] Keep the limitation explicit: Cathulu does not receive native Codex caller identity or tool-list visibility. The receipt records a declared receiver scope and a successful scoped search; runtime role-specific tool availability remains a separate probe requirement.
- [x] Record each receiver's logical scope so unchanged known content is omitted and changed knowledge is a delta.
- [x] Reuse the existing provenance-bearing memory-candidate and review flow for durable worker discoveries; do not promote report text or model summaries directly to canonical authority.
- [x] Keep summaries non-authoritative and specialist output unable to delete or overwrite durable memory.

Affected areas: `src/memory.rs`, `src/delegation.rs`, migrations only if required, `src/mcp.rs`, context-transfer resources, and `docs/local-mcp.md`.

Dependencies: M20; M21 runtime capability; M22 parent/child identity; existing Phase 1 memory schema. M23 closes the empty-transfer evidence gap without inventing memory IDs. The observed PM memory-tool availability gap remains a runtime limitation: a hierarchy-aware PM stays prepared until it can perform the required bounded search.

Acceptance criteria:

- [x] Each dispatched hierarchy-aware PM/Worker has a ready receipt tied to a successful bounded memory search, plus valid transfer/known-by refs or an explicit no-relevant-memory disposition. Missing/incomplete receipts block Cathulu's dispatch record.
- [x] A ready receipt requires a successful bounded search event for its exact delegation run and receiver scope; no search event means dispatch remains blocked.
- [x] Documentation distinguishes scoped search evidence from proof of per-agent MCP tool availability; the M21 PM `read_memories` gap remains open pending a runtime probe that confirms the required bounded-search operation is available to that role.
- [x] Stale, COLD, superseded, foreign-scope, or over-budget memory cannot be injected as current context.
- [x] Unchanged known content is not resent; changed content records a delta/replacement.
- [x] Existing canonical promotion continues to preserve source evidence; M23 adds no alternate promotion or deletion authority.

Validation:

- [x] MCP and persistence checks exercise missing/incomplete/readiness gate, first delivery, idempotent receipt retry, already-known omission, delta delivery, no-memory outcome, foreign/superseded/COLD/over-budget memory, and migration preservation.
- [x] Run existing memory lifecycle and delegation checks; build/check the Rust workspace and verify `git diff --check`.

#### M23 validation evidence

- `cargo fmt --all -- --check` — passed.
- `cargo test --offline --test delegation_lifecycle` — 12 passed.
- `cargo test --offline --workspace` — 74 passed, 1 existing desktop integration test ignored by design; no failures.
- `git diff --check` — passed.
- The initial default-sandbox workspace run was blocked by Windows `Access is denied` in the Tauri build script; the same offline workspace run passed when rerun with the build permission required by that script.
- No project repository was accessed. These tests validate Cathulu's persistence, MCP, and dispatch-record behavior; they do not prove which native agent identity called an MCP tool or that the PM's client tool inventory now includes memory search.

Documentation updates: local MCP role sequence, architecture, ADR-0009 consequences, agent contract, roadmap, and this plan.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`.

Expected Git checkpoint: validated memory-activation milestone branch and coherent commit.

### M24 — Project-scoped live activity graph

Goal: show truthful, role-labeled Main → Project Manager → Worker activity for independent project runs.

Design direction: make the recorded activity graph the first view and keep detail progressive. The default is a portfolio overview with exactly one stable Main Overseer root and a separate project lane for each registered project; lanes show only that project's latest running or recent run and explicit delegation relationships. Project chips are visualization filters that focus one lane and its run history; they do not select an orchestrator or alter project assignment. The owner can return to `All projects` at any time. Illustrative cross-project edges, agent names, metrics, and percentages from the rough canvas are not Cathulu evidence and must not appear. Use the existing React/CSS stack for this fixed topology rather than adding a graph library. Chats are session representations of the same Main role, not separate Main agents. Only a linked `SubagentStart` without a later `SubagentStop`, while its run remains `running`, may pulse as observed active. A recorded dispatch without linked hook activity is `unobserved`, never active. M21 surfaced no hook events, so unobserved states are expected and the dashboard must communicate that state without implying a proven idle agent. The application surface must fit its native window without document-level scrollbars; content should be compacted or contained in its own view where necessary. Keep the detached monitor as a separate compact per-project board; do not shrink the main dashboard into it. Native window and document corners must remain transparent, with a low-opacity clipped CSS surface rather than a native full-window material.

Subtasks:

- [x] Extend the existing read-only snapshot with stable project IDs, explicit hierarchy relationships, and coarse hook activity.
- [x] Correlate observed start/stop events only through stable hashes and explicit delegation references; keep unbound hooks separate and retain privacy minimization.
- [x] Default to one portfolio graph with one Main root and project-scoped lanes; allow explicit project focus and per-project run history without implying multiple Main identities.
- [x] Add a clear unobserved state that distinguishes missing hook data from a proven idle agent.
- [x] Keep the compact floating monitor focused on each project's state; show short project rows when expanded.

Affected areas: `src/telemetry.rs`, `src/dashboard.rs`, `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, Tauri snapshot types, and UI documentation.

Dependencies: M21 nested-spawn capability findings (including the absence of hook events in the probe), M22 hierarchy relationships, M23 activation evidence. Hook-backed activity remains conditional on observed/bound hook events and is not assumed by the graph.

Acceptance criteria:

- [x] A recorded run renders one Main root; manager and worker nodes follow explicit persisted assignment/parent relationships.
- [x] The default dashboard shows the same single Main Overseer connected to every registered project's latest recorded activity; project selection is clearly labeled as a focus filter and can return to All projects.
- [x] Main remains the same global coordinator identity across project/run views; the selected run does not create another Main agent.
- [x] A completed or failed child remains visible as history without pulsing as active.
- [x] Missing telemetry never creates an active node or percentage.
- [x] Independent projects have separate graphs, tasks, and memory scopes.
- [x] The dashboard remains read-only.

Validation:

- [x] Frontend build and Rust snapshot checks.
- [x] Main dashboard root/document sizing prevents body-margin overflow and page-level horizontal/vertical scrollbars.
- [ ] Windows native manual smoke with one recorded hierarchy, missing telemetry, stopped/blocked run, independent projects, transparent corners, dragging, always-on-top, and compact/expanded sizing.
- [x] Reduced-motion/reduced-transparency behavior and `git diff --check`.

Documentation updates: ADR-0008, architecture, roadmap, README, and this plan.

Applicable specialized skills: `milestone-delivery`, `ui-ux-research`, `redesign-existing-projects`, `impeccable`, `apple-design`, `emil-design-eng`.

Expected Git checkpoint: M24 branch and review request after automated validation; merge remains gated on Windows native visual smoke.

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
- [x] Confirmed official docs describe MCP inheritance for configured Codex subagents; M21 later verified nested spawning in the tested client/session.
- [x] M20 — role contract and memory protocol.
- [x] M21 — nested-spawn/MCP capability probe; role-specific memory tool parity and hook correlation remain open follow-ups.
- [x] M22 — hierarchy-aware delegation and caps.
- [x] M23 — memory-gated activation and handoff; role-specific tool availability and native hook identity remain runtime limitations.
- [ ] M24 — project activity graph.
- [ ] M25 — independent real-task verification when a task exists.

## Implementation discoveries / decisions

- 2026-10-21: Current dashboard state has no running Cathulu run, saved delegation, or agent activity for the workflow the owner expected to see. The UI cannot display activity absent stored evidence.
- 2026-10-21: Current M6-M9 deliberately prepare and record native Codex handoffs but do not spawn agents. Existing role fields do not encode parent-child delegation relationships.
- 2026-10-21: Official Codex subagent docs state that custom agents inherit MCP servers from the parent configuration; they do not promise nested spawn support uniformly across clients. M21 then demonstrated one PM-to-Worker spawn in the tested session, but PM/Worker memory tool availability differed and no hook metadata surfaced.
- 2026-10-21: The limit is interpreted strictly as two worker agents total for a Project Manager assignment, including replacements. Revisit only if the owner changes the rule.
- 2026-10-21: The current workflow uses independent project scopes; cross-project handoff testing remains deferred.
- 2026-10-21: M23 adds an append-only activation receipt and requires a receiver/run-scoped successful `search_memory_candidates` event before Cathulu can record a hierarchy-aware delegation as dispatched. The activation receipt validates current memory, exact transfer or known-by state, scope, temperature, freshness, and HOT budget. Cathulu still cannot authenticate the native agent caller or inspect its tool inventory; the M21 PM memory-tool gap therefore remains visible and a PM must stay prepared until its bounded-search call succeeds.

## Completion evidence

M20 documentation delivery (2026-10-21): added the normative Main/Project Manager/Worker operating contract and Cathulu memory workflow; accepted ADR-0009; updated the project definition, architecture, roadmap, and local MCP guide. Documentation review confirms coordinator-only Main behavior, one manager per project assignment, a strict two-worker total per assignment, independent project scopes, and no invented runtime capability. M21 subsequently verified nested spawning in the tested session; deterministic enforcement, complete memory activation, dashboard hierarchy, and real-task evaluation remain pending under M22-M25.

M21 capability probe (2026-10-21): Cathulu run 4 is `verified_success`; the Main spawned one PM and the PM spawned one Worker, with delegations 1 and 2 recorded as dispatched and the Worker report submitted. No project repository was accessed. Main, PM, and Worker each reached Cathulu MCP, but the PM lacked `read_memories` while the Worker had it. HOT/WARM memory was empty; “no relevant memory” was recorded in the run, while `prepare_context_transfer` could not encode an empty transfer. No hooks surfaced, so no hierarchy edge was inferred. M21 closes the nested-spawn capability gate; M22 handles deterministic hierarchy, M23 addresses activation/tool parity, and M24 remains responsible for hook-backed graph evidence.

M23 implementation and validation (2026-10-21): migration 11 adds append-only revisioned activation receipts and references. A ready receipt is tied to a successful bounded memory search event for the exact delegation run and receiver; selected memories must pass existing acceptance, freshness, scope, temperature, HOT-budget, transfer, and known-by rules. A structured `no_relevant_memory` disposition supports empty candidate sets without fabricating memory IDs. The latest ready receipt is required before Cathulu records dispatch; missing/incomplete cases append a run admission rejection. Local Windows validation passed formatting, 12 delegation lifecycle tests, the full offline workspace suite (74 passed, 1 existing desktop test ignored), and `git diff --check`. The default-sandbox Tauri build initially failed with Windows access denied; the same suite passed with the build permission required by that script. No project repository was accessed. M23 does not resolve the M21 PM memory-tool availability difference or native caller attribution; those remain explicit runtime limitations.

M24 implementation and automated validation (2026-10-21): added a project-ID-scoped read-only graph for Main, explicitly assigned Project Managers, and parent-linked Workers; hook observations without a delegation binding stay in a separate unlinked strip. The dashboard marks dispatches without linked starts unobserved, and only a linked start on a running run may pulse as active. Added project IDs to dashboard run/blocker/delegation records, a bounded hook-activity projection, separate dashboard/monitor Tauri capabilities, and a 148×42 compact / 324×204 expanded always-on-top monitor with a transparent document and clipped low-opacity glass. Rust workspace tests passed (74 passed, 1 existing ignored), frontend tests passed (4), the frontend build passed, and formatting passed. Automated coverage includes real and missing hierarchy links, unlinked hooks, stop state, and project isolation. Native Windows visual smoke has not been performed; M24 is therefore not complete and must not merge until that smoke passes.

Logical identity clarification (2026-10-21): the owner clarified that one logical Main Overseer exists portfolio-wide while multiple Codex chats may represent or resume that role. Updated ADR-0009 and the normative agent contract. M24 now renders a stable coordinator node without run-specific identity or an active-work pulse; project/run context remains in the graph toolbar. This is a presentation and role-contract invariant, not runtime chat authentication or a cross-chat lock.

Portfolio observability clarification (2026-10-21): the owner could not tell what the project chips controlled and could not see Main's interactions with all projects at once. M24 therefore defaults to an `All projects` portfolio graph with one Main root and one latest-activity lane per project; choosing a project explicitly narrows the view. The visible global scrollbars came from the default body margin combined with a viewport-sized root; the desktop document now resets margin, uses border-box sizing, and clips the app viewport. Current frontend validation: `npm test -- --run` — 5 passed; `npm run build` — passed; `git diff --check` — passed. The native Windows viewport-fit and monitor smoke remains required before merge; no native visual pass is claimed.

Compact monitor regression correction (2026-10-21): the final monitor CSS reintroduced 4px root padding after the global border-box reset. Since the detached window is only 148×42 logical pixels, the glass surface contracted to 140×34 and its controls lost usable space. The monitor root now has zero padding so the glass uses the full native window bounds; the dashboard viewport and scroll fix are unchanged. Frontend validation: `npm test -- --run` — 5 passed; `npm run build` — passed; `git diff --check` — passed. Native Windows monitor interaction still needs visual smoke before M24 can merge.

