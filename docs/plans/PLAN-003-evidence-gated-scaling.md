# PLAN-003 — Evidence-gated scaling: scopes, structural retrieval, and policy learning

Status: Planned after Phase 3. M8 is published; M9 remains in progress under PLAN-002. M10-M14 are conditional on the evidence gates below.
Created: 2026-10-21
Last updated: 2026-10-21

## Objective

Complete the roadmap's remaining Phase 4-8 capabilities only when Cathulu's local, verified run data demonstrates their need and expected benefit. Add durable scopes, structural retrieval, convergence signals, workflow experiments, and bounded policy research without duplicating memory authority, exposing private evidence, or automating unsupported decisions.

## Motivation

The roadmap deliberately conditions M10 on repeated cross-project context reconstruction, M11 on a useful Graphify snapshot source, and M12-M14 on enough comparable run evidence. The repo has no default Cathulu database or Graphify snapshot today, so those product-value conditions have not been met. This plan defines how to evaluate them and what to build after they are met; synthetic fixtures validate mechanics but do not satisfy product-evidence gates.

## Current state

- M1-M8 are validated and published to `main`; M8 implementation is `aecac57777fdf31c021553bce8a30164a9444e07` and its documentation closeout is on `main`.
- M9 is specified in [PLAN-002](PLAN-002-phase3-codex-delegation.md) and awaits the owner's selector target-layer choice.
- M4 already supplies stable scopes, HOT budgets, known-by state, transfer records, SQL/FTS retrieval, policy comparisons, storage snapshots, and health metrics. Later milestones must extend these authorities rather than create parallel ones.
- The current process/user `CATHULU_DATA_DIR` and default `LOCALAPPDATA\Cathulu\cathulu.sqlite3` locations had no database on 2026-10-21. `codex mcp list` also showed no Cathulu server in the current local Codex profile. No repository `graphify-out/graph.json` snapshot exists, and the Graphify CLI is not installed.
- No live Cathulu run corpus is available in the current local Codex profile to support M10, M12, M13, or M14 entry decisions. Other profiles or separately configured MCP launch environments were not inspected.

## Desired state

- Scoped Overseer state, if justified, resumes from compact durable state and source deltas, with strict project isolation and bounded HOT activation.
- Structural retrieval, if justified, consumes local, identified Graphify snapshots as retrieval input; SQLite remains the authority for memory and decisions.
- Convergence and workflow evaluation distinguish observed evidence from inferred labels and compare only like-for-like runs.
- Policy changes remain versioned proposals with owner review. Learned routing is offline and advisory; no autonomous policy changes or reinforcement learning are in the MVP.
- Each phase has a documented entry decision, validation evidence, and a recorded outcome. A gate that lacks evidence remains open; fixtures do not substitute for live evidence.

## Scope

- Define measurable entry gates for M10-M14 using the existing M4 telemetry and policy-evaluation records.
- Add only the smallest schema, domain, MCP, and dashboard extensions needed after each gate passes.
- Benchmark each optional subsystem against the simplest existing retrieval or workflow baseline.
- Preserve local-only operation, SQLite authority, provenance, stable identities, and existing scope/transfer records.

## Out of scope

- Building Sub-Overseer coordination before the M10 evidence gate passes.
- Embedding or uploading Cathulu repositories, transcripts, or evidence to an external service.
- Adding Graphify invalidation without an identified, locally available snapshot source and reproducible freshness identity.
- Calling synthetic test results proof of user benefit or sufficient run volume.
- Autonomous agent spawning, merging, approval changes, or Codex configuration writes.
- Reinforcement learning or automatic policy/routing changes; M14 remains bounded research and owner-reviewed recommendations.

## Source-of-truth impact

- `docs/roadmap.md`: link these phase gates and report whether each is eligible, active, validated, or still waiting on evidence.
- `docs/architecture.md`: update only when a gate passes and implementation changes actual data authority or retrieval behavior.
- `docs/decisions/`: add an ADR for material Graphify, scope ownership, evaluation, or automation-boundary decisions.
- `docs/plans/PLAN-003-evidence-gated-scaling.md`: record gate evidence, baselines, implementation, and validation per milestone.
- `docs/project-definition.md`: change only if the product boundary or MVP changes.

## Existing decisions and constraints

- SQLite is authoritative for Cathulu memory, scopes, provenance, run outcomes, and policy experiments.
- Memory class and temperature remain separate; HOT context is bounded, WARM is selective, and COLD evidence is not automatically injected.
- Scopes reuse stable existing identities and must not create a second project, task, or memory authority.
- Git, contracts, dependencies, and graph snapshots invalidate only knowledge that declares a matching freshness basis.
- Graphify is retrieval infrastructure and a freshness source, not a second memory authority.
- Human review remains required for policy promotion and any change that can affect Codex configuration or execution.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`, `ponytail-balanced`, and `tdd`.
- `graphify` when building or evaluating a local Graphify snapshot integration; verify its local-data behavior and license before use.
- `ui-ux-research` and `impeccable` for any new substantial dashboard or scoped-context UI.
- Context7 for library/API/CLI documentation where available; if quota blocks it, use current primary vendor documentation and record the fallback.

## Dependencies

- M9 validated before M10 starts, so delegation settings and requested/effective values are represented accurately.
- M4 memory and measurement substrate; M6-M8 delegation, worker report, and worktree evidence.
- A real local Cathulu run history to evaluate M10/M12/M13/M14. The owner must generate representative runs through normal work; test fixtures alone do not pass these gates.
- For M11, a reproducible local Graphify output format/version and a local snapshot source. Any model-backed extraction or network access requires a separate product/privacy decision.

## Risks and unknowns

- Current synthetic tests demonstrate implementation mechanics, not repeated reconstruction costs or retrieval precision in the owner's work.
- Small or unmatched task populations can make apparent success and failure rates unreliable. Evaluation strata and decision thresholds must be declared before comparing variants, based on the observed baseline and test design rather than invented in this plan.
- Graphify may require a model-backed extraction path or expose a changing export format. Local-only behavior, schema stability, licensing, and maintenance must be checked before integration.
- Convergence labels can overstate evidence. Every signal must be explainable from immutable run/check/event records and remain advisory until independently validated.
- Automatic adaptation can amplify noisy outcomes. M14 must not change live policy without explicit approval and a reversible, versioned proposal.

## Milestones

### M10 — Durable scoped context and Sub-Overseers

Goal: reduce measured cross-project context reconstruction with resumable, bounded logical scopes, but only when evidence shows that scope maintenance costs less than repeated reconstruction.

Entry gate:

- Collect a real, local baseline showing repeated cross-project context reconstruction for the same recurring work pattern.
- Measure context bytes/tokens when observable, reintroduction, delay/rework, task outcomes, and the expected maintenance/coordination cost.
- Agree on the comparison method and decision threshold before implementation. Do not use synthetic fixtures or an arbitrary run count as the product-value proof.

Subtasks:

- Reuse existing scope IDs, memory budgets, known-by state, transfer ledger, and freshness metadata.
- Add Sub-Overseer ownership/snapshot state only if the existing `scope` records cannot represent the observed need.
- Resume from compact snapshot plus Git/dependency/contract deltas; invalidate stale knowledge and send only changed, relevant context.
- Keep project boundaries, parent ownership, worker reports, and the parent QA gate explicit.

Affected areas: memory scope domain, delegation contracts, migrations if needed, local MCP, Tauri snapshot, read-only dashboard, integration tests, docs.

Dependencies: M9; measured M4 transfer and run data; accepted entry-gate result.

Acceptance criteria:

- Repeated real work demonstrates a measurable reconstruction cost that exceeds measured scope maintenance/coordination cost under the predeclared comparison.
- A suspended scope resumes from state plus source deltas; tests show unchanged known context is omitted and changed/stale context is replaced.
- Memory, artifacts, and worker context remain visible only within the authorized scope; HOT memory remains bounded.
- Sub-Overseer conclusions do not override owner decisions or parent-run quality checks.

Validation:

- Public MCP/Tauri/restart tests for scope creation, resume, delta, freshness, isolation, and bounded activation.
- Paired local tasks with a declared baseline and held-out confirmation; preserve the evidence and limitations in the plan.
- If the entry gate fails, validate the gate computation and record M10 as not eligible; do not implement the coordination subsystem.

Documentation updates: architecture, roadmap, PLAN-003, and an ADR if ownership or authority changes.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `ui-ux-research`, `impeccable`.

Expected Git checkpoint: one validated checkpoint for the gate/implementation result; a failed eligibility gate may be a documentation-only checkpoint.

### M11 — Read-only Graphify retrieval and freshness

Goal: improve code-structure retrieval and targeted memory freshness using local Graphify snapshots only when a measured benchmark shows value over SQL/FTS and file/Git inspection.

Entry gate:

- Identify a reproducible Graphify export and version, and demonstrate it can be generated/consumed locally without sending private repository content to an unapproved service.
- Select structurally complex tasks and compare Graphify neighborhoods with current SQL/FTS plus file/Git retrieval under the same context budget.
- Adopt only if the predeclared relevance/usefulness and maintenance criteria are met.

Subtasks:

- Define an import/read-only adapter and a stable snapshot ID/content hash tied to repo identity and source Git revision.
- Retrieve candidate node/edge IDs and source locations; resolve any memory candidates back through SQLite.
- Invalidate only memories that explicitly depend on a changed graph snapshot; preserve historical knowledge and provenance.
- Keep stale, missing, malformed, or incompatible snapshots unavailable rather than silently treating them as fresh.

Affected areas: local Graphify adapter, typed graph snapshot metadata, retrieval MCP resources, memory freshness, tests, setup and architecture docs.

Dependencies: a verified local Graphify source/export; M4 retrieval baseline; M10 scope behavior if M10 passes.

Acceptance criteria:

- SQLite remains authoritative; Graphify contains no duplicate Cathulu memory authority.
- Graph results cite source locations and resolve to current repository/snapshot identity.
- Stale or removed graph candidates cannot override current exact state or inject COLD evidence.
- A benchmark demonstrates improvement for selected structural tasks at equal context budget; otherwise Graphify remains optional/deferred.

Validation:

- Fixtures for valid, stale, malformed, empty, changed, and removed graph snapshots; candidate filtering and source-location integrity.
- Local end-to-end retrieval against the M4 SQL/FTS baseline and documented dataset/build method.
- Reproducibility, licensing, privacy, and offline behavior review before adding a dependency or build step.

Documentation updates: architecture, local setup, roadmap, PLAN-003, and an ADR recording the Graphify source contract.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `graphify`, `openai-docs` only if a Codex/OpenAI service integration is proposed.

Expected Git checkpoint: validated local snapshot/retrieval checkpoint or documented non-adoption after the benchmark gate.

### M12 — Convergence signals and escalation evidence

Goal: identify stalled, looping, oscillating, or regressing work from explainable state differences and verified run evidence.

Entry gate:

- Collect enough comparable real runs and repeated failure/stall examples to define and hold out a validation set.
- Declare signal definitions and acceptable false-positive/false-negative tradeoffs before fitting thresholds.

Subtasks:

- Define state signatures and quality-distance components from existing objective/task/run/mandatory-check records.
- Compute deterministic stall, loop, oscillation, and regression candidates with evidence links and confidence/coverage.
- Present escalation/replan suggestions as reviewable evidence; do not autonomously interrupt or spawn agents.

Affected areas: run analysis, event projections, dashboard evidence, experiments, tests, docs.

Dependencies: M10-M11 only where their evidence changes the state signature; sufficiently varied real runs.

Acceptance criteria:

- Each signal can be reproduced from retained evidence and inspected by a human.
- Held-out evaluation meets the predeclared accuracy and regression constraints for each task class where the signal is enabled.
- Missing evidence yields unknown/inconclusive, not a false convergence claim.

Validation:

- Deterministic fixtures for known stall/loop/oscillation/regression and negative cases.
- Held-out real-run evaluation, false-positive review, and restart/replay equivalence.
- If data is insufficient, record the gap and defer thresholds or feature enablement.

Documentation updates: architecture, dashboard evidence contract, roadmap, PLAN-003, and an ADR if quality semantics change.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `release-readiness` only for a rollout.

Expected Git checkpoint: validated explainable signal checkpoint; no live automation is part of M12.

### M13 — Workflow experiments and reviewable proposals

Goal: compare workflow versions on similar task classes and recommend evidence-backed changes without automatic promotion.

Entry gate:

- Retain enough completed, quality-verified, comparable runs across workflow versions to estimate the chosen outcomes.
- Pre-register task classes, baseline version, measurements, regression guards, and stopping/decision rules.

Subtasks:

- Aggregate only runs with compatible task-class and quality evidence; report sample coverage and uncertainty.
- Create immutable experiment definitions and append-only evaluations linked to source runs.
- Generate a reviewable workflow-change proposal with expected benefit, measured regressions, and supporting evidence.
- Require explicit human acceptance before a new workflow version becomes active.

Affected areas: existing workflow/policy evaluation tables, aggregation, MCP read/review operation, dashboard, tests, docs.

Dependencies: M12 quality evidence where useful; enough comparable real runs; no new experiment database if existing SQLite structures suffice.

Acceptance criteria:

- Comparisons do not mix incompatible task classes or unverified outcomes without explicit labeling.
- Aggregates are traceable to immutable source runs and show coverage/uncertainty.
- Proposed changes cannot mutate active workflows or rewrite previous evaluations.
- A controlled follow-up confirms any claimed improvement against the unchanged baseline criteria.

Validation:

- Fixtures for compatible/incompatible cohorts, missing outcomes, regression guards, append-only proposals, and idempotency.
- Offline replay plus a prospectively collected comparison when data exists; report insufficient evidence honestly.

Documentation updates: experiment protocol, workflow authority, roadmap, PLAN-003, and an ADR for promotion rules.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`, `implementation-plan` for any experiment protocol change.

Expected Git checkpoint: one validated experiment engine/proposal checkpoint; workflow promotion remains owner-controlled.

### M14 — Bounded adaptive-policy research

Goal: determine whether learned routing or scope/memory policy proposals can improve verified outcomes without risking silent behavior changes.

Entry gate:

- M13 produces a trustworthy cohort with repeated verified outcomes and a prospective improvement over a stable baseline.
- Predeclare task classes, policy features, leakage controls, evaluation split, safety constraints, and a no-regression criterion.
- If these conditions are absent, M14 remains research-only and no adaptive model is introduced.

Subtasks:

- Compare candidate policies offline using a reproducible and auditable dataset derived from local records.
- Version policy definitions and evaluation artifacts; report the baseline, sample coverage, uncertainty, and regressions.
- Emit owner-reviewed suggestions only. Keep the active policy unchanged until an explicit, reversible application is approved.
- Exclude reinforcement learning and autonomous policy changes from the MVP.

Affected areas: local experiment analysis, immutable policy records, advisory UI, tests, docs.

Dependencies: M13 prospective comparison and sufficient trustworthy repeated data.

Acceptance criteria:

- Candidate policy has a predeclared, independently reproducible improvement on held-out data and meets the no-regression guard.
- No private data leaves the local machine; no live task is routed or memory policy changed automatically.
- The UI distinguishes observed, proposed, approved, and active policy states.
- If the evidence gate fails, publish a documented research conclusion and keep learned routing out of the product.

Validation:

- Reproducible offline evaluation, leakage and adversarial cases, held-out comparison, rollback simulation, and deterministic policy versioning.
- Owner review of the proposed change before any activation workflow is implemented or used.

Documentation updates: research protocol, architecture, roadmap, PLAN-003, and an ADR if a policy boundary changes.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `tdd`; `release-readiness` only before rollout.

Expected Git checkpoint: validated research/evaluation checkpoint or explicit evidence-based non-adoption.

## Final integration validation

Run the M10-M14 scenarios with local, disposable fixtures and then with prospectively collected, owner-approved real run evidence. Verify SQLite authority, privacy boundaries, provenance, restart behavior, freshness, scope isolation, bounded context, comparison validity, holdout integrity, owner-controlled proposals, and rollback. Never report a conditional milestone complete because its synthetic tests pass when its product-evidence gate remains unmet.

## Rollback / recovery

Prefer additive schema and deterministic projections. Keep prior snapshots, source run references, and workflow/policy versions immutable. Disable optional retrieval or suggestions by configuration without deleting source evidence. Preserve any imported Graphify file unchanged; on parser/version failure, mark the snapshot unavailable and fall back to current SQLite/SQL/FTS retrieval. Never activate an experimental policy as part of rollback or cleanup.

## Progress

- [x] Inspect the existing M10-M14 roadmap goals and M4 memory/telemetry substrate.
- [x] Check current local evidence for the M10-M14 entry gates without reading raw user memory: no database in the process/user/default Cathulu locations, no Cathulu MCP entry in the current Codex profile, no Graphify snapshot, and no Graphify CLI were found on 2026-10-21.
- [ ] Validate M9 and record the owner-approved selector target before beginning Phase 4.
- [ ] Collect real local Cathulu runs and evaluate M10-M14 gates in sequence.

## Implementation discoveries / decisions

- 2026-10-21: M10's gate is unmet in the current local Codex profile: no Cathulu MCP entry or database was found, so the existing M4 real-world measurements cannot currently demonstrate repeated cross-project reconstruction.
- 2026-10-21: M11's gate is unmet: there is no `graphify-out/graph.json` in this checkout and no installed Graphify CLI. No extraction was run because that tool may involve model-backed processing and no local Graphify/privacy contract is approved.
- 2026-10-21: M12-M14 cannot be evaluated from this checkout without real completed and comparable run evidence. Synthetic fixtures will validate mechanics only and will not satisfy these gates.
- 2026-10-21: Context7 resolution is currently unavailable because its monthly quota is exhausted; current official OpenAI docs were fetched for M9. Resolve the app-specific account catalog through a supported local Codex source and do not infer account entitlement from public docs.

## Completion evidence

No M10-M14 implementation is claimed. The entry-gate audit above is the current evidence. Continue after M9 with local Cathulu use and fresh gate evaluation; implement each capability only after its own preconditions pass.
