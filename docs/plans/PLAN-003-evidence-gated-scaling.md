# PLAN-003 — Evidence-gated scaling: scopes, structural retrieval, and policy learning

Status: M9 is validated and published to `main`; M11 is implemented, validated, and published to `main` as `e18dc46` (PR #1). M10 is not currently eligible because no qualifying recurring cross-project workflow has been observed. M12-M14 remain conditional on their evidence gates below.
Created: 2026-10-21
Last updated: 2026-10-21

## Objective

Complete the roadmap's remaining Phase 4-8 capabilities only when Cathulu's local, verified run data demonstrates their need and expected benefit. Add durable scopes, structural retrieval, convergence signals, workflow experiments, and bounded policy research without duplicating memory authority, exposing private evidence, or automating unsupported decisions.

## Motivation

The roadmap deliberately conditions M10 on repeated cross-project context reconstruction, M11 on a useful Graphify snapshot source, and M12-M14 on enough comparable run evidence. M11's product-value gate has passed. The local Cathulu MCP server is now configured and the Cathulu repository is registered, but no qualifying recurring cross-project workflow is currently available to measure. This plan defines how to evaluate the remaining gates and what to build only after they are met; synthetic fixtures validate mechanics but do not satisfy product-evidence gates.

## Current state

- M1-M9 are validated and published to `main`; M9 implementation is `f2381ae`. M11 is validated and published as `e18dc46` (PR #1).
- M4 already supplies stable scopes, HOT budgets, known-by state, transfer records, SQL/FTS retrieval, policy comparisons, storage snapshots, and health metrics. Later milestones must extend these authorities rather than create parallel ones.
- The 2026-10-21 entry-gate audit was rechecked after M11 publication: `CATHULU_DATA_DIR` is unset, the default `LOCALAPPDATA\Cathulu\cathulu.sqlite3` database is absent, `codex mcp list` has no Cathulu server, and there is no project-root Graphify snapshot. Graphify 0.9.82 remains available through the existing user Python site package; the pilot snapshot is ignored and temporary.
- The local MCP registration and Cathulu project resource were verified during this continuation. No qualifying recurring cross-project workflow is currently available to evaluate.
- M10 is not eligible under the currently observed work pattern. Do not infer dependencies, combine unrelated repository tasks, or manufacture cross-project evidence. Reconsider M10 if a naturally recurring task crosses repositories.

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
- Current applicability: available evidence contains no qualifying recurring cross-project task. Do not start M10 evidence collection until one exists.
- Measure context bytes/tokens when observable, reintroduction, delay/rework, task outcomes, and the expected maintenance/coordination cost.
- Agree on the comparison method and decision threshold before implementation. Do not use synthetic fixtures or an arbitrary run count as the product-value proof.

Evidence collection protocol (complete before the first scored pair):

- Define a privacy-safe recurring-work label, task inclusion/exclusion rules, a common acceptance check, and which runs are exploratory versus held out.
- Declare the baseline and Cathulu-assisted procedure, how task order will be handled, the outcome/rework measures, the acceptable regression guard, and the decision rule. Choose the observation count from observed variation and coverage; do not set a run count here.
- For each run, retain the Cathulu run ID when one exists, the paired run reference, project/task class, start/end or elapsed time, verified outcome and QA results, context bytes/tokens when observable, context reintroduction, rework, and human interventions.
- Record scope setup, refresh/resume, curation, and coordination time separately as manual observations until those costs have a trustworthy automatic source. Include the measurement method and observer.
- Existing records can supply run status and timestamps (`runs`), QA outcomes (`quality_checks`), Cathulu-controlled transfer size/token estimate/reintroduction (`context_transfers`), and explicitly entered rework/intervention/latency measures (`memory_policy_evaluations`). Cathulu only observes transfers routed through its own boundary; unavailable token counts and external activity remain unknown rather than inferred.
- Keep prompt and transcript bodies out of the worksheet. Store local run IDs and coded labels, and record enough source references for an owner to verify the result locally.

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

Pre-registered local benchmark (before scored retrieval runs):

- Corpus: only code in the Cathulu repository at `92ee9d5`; Graphify `0.9.82`, schema version `1`, code-only extraction, no documentation/media extraction, no LLM backend, and query logging disabled.
- Tasks: trace worktree reconciliation to report verification; trace `finish_run` to mandatory-check validation; trace Tauri Codex preview to its TOML preview builder.
- Baseline: exact file search with `rg -n -C 2` around caller and callee, followed by source-line inspection. Graphify result: exact directed path plus the matching edge's relative file, source location, relation, and confidence.
- Context budget: cap both representations at 8 KiB per task. A task succeeds only when the expected relationship is present with a source location that matches the current source and is neither stale nor outside the registered repository.
- Adopt only if at least two of three tasks succeed, Graphify uses at most half the baseline bytes for those same tasks, and the adapter can fail closed on unsupported schema/version or stale source commits without adding a required runtime dependency. SQL-specific semantics remain an explicit FTS/manual fallback case.
- Earlier exploratory CLI calls are feasibility checks only; they are not benchmark scores. The scored run records raw byte counts and source-verified edges below.

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
- [x] Initial pre-onboarding audit on 2026-10-21: no database in the process/user/default Cathulu locations, no Cathulu MCP entry in the current Codex profile, and no project-root Graphify snapshot were found.
- [x] Validate M9 with explicit per-preview target selection and no implicit target.
- [x] Publish the validated M9 checkpoint `f2381ae` and record its revision.
- [x] Re-evaluate the M10-M14 entry gates after M9; the live local run-data gate remains unmet.
- [x] Reproduce Graphify 0.9.82 schema 1 locally with code-only extraction, no LLM backend, and query logging disabled; verify snapshot commit identity against the unchanged code corpus.
- [x] Review local privacy, offline, and distribution metadata: the installed Graphify distribution includes Apache-2.0 and MIT license texts; Cathulu adds no Graphify dependency, bundled code, or automatic extraction step.
- [x] Pre-register and score the three-task Graphify comparison under the same 8 KiB cap; all three results matched current source and each Graphify packet was at most half the paired `rg` output.
- [x] Implement and validate the read-only Graphify adapter, bounded structural retrieval, stale/malformed snapshot rejection, and current-source filtering.
- [x] Update architecture, roadmap, setup, and the Graphify source-contract ADR after the implementation passes validation.
- [x] Validate the additive SQLite v9 snapshot migration against version-four and version-five data fixtures.
- [x] Publish M11 after local Windows validation and recheck the M10-M14 entry gates.
- [x] Configure the local Cathulu MCP with prompt-gated tool approval and register the Cathulu checkout; verify the projects resource reports it.
- [x] Review available work-pattern evidence; no recurring cross-project task qualifies for the M10 gate.
- [ ] Collect comparable real local Cathulu runs for M12-M14. Reconsider M10 only if a naturally recurring cross-project task appears.

## Implementation discoveries / decisions

- 2026-10-21: Initial pre-onboarding review found no durable Cathulu MCP entry or database, so existing M4 measurements could not demonstrate repeated cross-project reconstruction. M9's disposable integration database was removed by the smoke script and is not product-use evidence.
- 2026-10-21: M11's evidence gate passed in a local-only pilot. Existing Graphify 0.9.82 was imported from the user Python site package; code-only extraction skipped all docs/media, used `GRAPHIFY_BACKEND=none`, and disabled query logging. The generated snapshot reported schema 1 and `built_at_commit=92ee9d571c3aa87278b6fd50a17d852e68216930`; the current source HEAD matched. No Graphify dependency or project-root artifact was added.
- 2026-10-21: The local `graphifyy` 0.9.82 distribution contains Apache-2.0 and MIT license texts. Cathulu consumes the export format only; no Graphify code or package is being copied or bundled. Upstream source and current licensing are linked from ADR-0006.
- 2026-10-21: The M11 benchmark was pre-registered before scored queries. Each Graphify packet contains the exact directed path plus compact edge metadata. Scored results: worktree reconciliation → report verification, 201/411 bytes (48.9%, EXTRACTED, `src/worktree.rs:L107`); `finish_run` → mandatory-check validation, 189/1040 bytes (18.2%, EXTRACTED, `src/workflow.rs:L691`); Tauri Codex preview → TOML preview builder, 222/2181 bytes (10.2%, INFERRED, `apps/desktop/src-tauri/src/lib.rs:L114`). The three edge locations were checked against current source. Earlier exploratory Graphify calls and Graphify's generic benchmark were excluded. The first task narrowly meets the 50% cutoff; benchmark byte accounting should be retained as defined, not rounded to a stricter threshold.
- 2026-10-21: Graphify is optional retrieval infrastructure, not an automatic dependency or memory authority. The adapter rejects unsupported schema/version and commit mismatches, omits results whose referenced source files changed or disappeared, and retains SQL/FTS fallback for exact state and memory.
- 2026-10-21: At the post-M11 pre-onboarding audit, M10 had no local Cathulu run history. M11 proceeded independently because its pilot and adapter rely on the existing registered-project identity and M4 SQLite/FTS substrate; it does not add M10 Sub-Overseer or durable-scope behavior.
- 2026-10-21: No qualifying recurring cross-project task has been observed. M10 is ineligible under current evidence; do not infer a relationship or run a synthetic cross-project pilot.
- 2026-10-21: M12-M14 cannot be evaluated from this checkout without real completed and comparable run evidence. Synthetic fixtures will validate mechanics only and will not satisfy these gates.
- 2026-10-21: Context7 resolution is currently unavailable because its monthly quota is exhausted; current official OpenAI docs were fetched for M9. Resolve the app-specific account catalog through a supported local Codex source and do not infer account entitlement from public docs.

## Completion evidence

M11 is complete: `cargo fmt --all -- --check` passed; `cargo test --offline --workspace` passed with 37 tests and one dedicated desktop/MCP smoke test ignored for its PowerShell harness; all three focused Graphify MCP tests passed; the offline Windows release MCP build passed; and `git diff --check` passed. The migration tests exercise version-four and version-five data through v9. SQLite remains authoritative and no Graphify dependency or bundled code was added.

M11 was merged to `main` at `e18dc46` through PR #1. The initial pre-onboarding check found no Cathulu MCP registration or database. Since then, the local MCP has been configured and Cathulu registered; no real run history exists yet.

M10 and M12-M14 are not claimed complete. M10 is not eligible under current evidence because no qualifying recurring cross-project workflow has been observed; its gate requires naturally recurring cross-project context reconstruction and must not be manufactured. M12-M14 need completed, comparable real runs. Synthetic fixture tests validate mechanics only and do not satisfy those product-evidence gates.
