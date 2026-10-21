# PLAN-004 — Optional specialist-model optimization

Status: M15-M18a and M18b MERGED; public MCP missing-key fallback coverage locally validated; one owner-approved Jev workflow-routing shadow integration run recorded after MCP reload; comparative scoring remains ineligible; M19 not started. Jev remains shadow-only. Active treatments still require separate explicit owner approval.
Created: 2026-10-21
Last updated: 2026-10-21

## Objective

Measure whether optional DecisionModel and ContextModel capabilities reduce Codex reasoning turns, unnecessary agent activations, context size and reintroduction, retrieval and tool-result interpretation overhead, workflow latency, and repeated processing while maintaining or improving verified task success, QA, convergence, regressions, rework, and human intervention.

Codex remains the primary reasoning, coding, planning, debugging, synthesis, and user-interaction system. Deterministic Cathulu code answers exact questions where possible. A specialist model is an optional optimization, never a required dependency. The same workflow must remain usable with every specialist disabled. Deterministic evidence and QA remain authoritative.

## Motivation

Cathulu already has a local SQLite authority, run and workflow records, structured events, selective SQL/FTS and Graphify retrieval, managed memory, known-by state, context-transfer ledger, and memory-policy/storage evaluation. PLAN-004 uses those foundations to test narrow models without creating a parallel memory system, experiment ledger, model selector, or agent runtime.

Shadow predictions can establish schema safety, privacy behavior, fallback, latency, and prediction quality, but cannot prove that an active treatment saves context or time. A reversible active pilot therefore requires a separate pre-registered comparison and explicit owner approval after shadow checks. No fixed sample size or numerical success threshold is invented; choose them from the observed baseline, task variation, and acceptable quality guard before scoring.

## Current state

- M1-M9 and M11 are implemented and validated; see the source plans and roadmap for checkpoints.
- M4 already records run-linked memory-policy evaluations, transfer measurements, retrieval usefulness, rework, latency, human intervention, and storage snapshots. M16-M19 must reuse these where they represent the measure.
- SQLite is authoritative. Existing runs, workflow_versions, events, memory_policy_evaluations, and evidence_artifacts are the first sources to audit for treatment attribution and invocation telemetry.
- M9's Codex model/effort preferences configure Codex and remain separate from specialist-provider settings.
- M10 and M12-M14 retain their own evidence gates. The deferred cross-repository handoff check is not a prerequisite for this phase.
- Jev/TypeSafe and ScaleDown remain optional evaluation providers, not dependencies. Current primary TypeSafe documentation supports minimized bounded decisions; ScaleDown official Extract API documents selected-text extraction and response offsets. The reviewed ScaleDown API pages do not state a request-retention period. Local alternatives and real provider quality remain to be evaluated.
- The initial readiness audit below found no runs or objectives. The current dashboard now records one verified Jev workflow-routing shadow integration run, with no blockers or failed checks; memory and transfer counters remain zero. There is still no comparable baseline cohort from which to set sample or quality thresholds, so M18 cannot score product impact yet.

## Scope

- Introduce minimal provider-neutral DecisionModel and ContextModel contracts, each with disabled behavior.
- Add independently configurable capability modes and per-workflow treatment attribution, defaulting every capability to disabled.
- Record privacy-minimized, append-only specialist invocation metadata locally, reusing current event/run/evaluation authorities before proposing a schema migration.
- Begin with shadow-only bounded workflow/delegation decisions and selected tool-result extraction.
- Compare deterministic/local alternatives where practical and verify current provider APIs, terms, availability, privacy, and maintenance requirements before writing an adapter.
- Use same-workflow ablation and controlled, reversible active trials only after shadow validation and explicit approval.
- Provide owner-reviewed evidence-backed enable/disable suggestions; do not change core policy automatically.

## Out of scope

- Replacing Codex, changing its active model/effort, or making any specialist provider mandatory.
- A new agent runtime, continuous model process, hosted Cathulu control plane, automatic model training, fine-tuning, or reinforcement learning.
- Sending full repositories, secrets, full transcripts, raw Cathulu memory, or entire artifact stores to a provider by default.
- Letting a specialist mark success, bypass deterministic QA/approval, establish a security finding as true, assert architecture or contract truth, or delete/promote durable memory autonomously.
- A large dashboard or model-management console. Configuration stays in local Cathulu/workflow configuration; any impact display is read-only and evidence-gated.
- Implementing memory consolidation, semantic indexing, garbage collection, or adaptive memory policy in this plan. Specialist-assisted memory relevance is only considered after deterministic retrieval has narrowed the candidate set.

## Architecture contract

### Intelligence hierarchy

1. Deterministic Cathulu software handles exact facts, state, rules, validation, freshness, and tests.
2. DecisionModel handles a bounded judgment over explicitly enumerated outputs.
3. ContextModel classifies, extracts, compresses, or summarizes selected existing information.
4. Codex handles open-ended reasoning, coding, architecture, planning, debugging, synthesis, and ambiguous investigation.
5. Deterministic QA and evidence decide whether work is verified.

Do not invoke both specialist roles for the same question by default. Choose the narrowest capable mechanism and measure whether it reduces total work, including provider latency and review/recovery.

### Provider contracts and configuration

Keep the contracts small and provider-neutral:

- DecisionModel operations may classify, choose, score, or return probabilities for bounded schemas.
- ContextModel operations may classify, extract, compress, or summarize existing selected information.
- A Disabled implementation provides a no-provider baseline. Provider errors, timeouts, invalid output, or missing availability are treated as a skipped optimization and continue the existing Cathulu/Codex path. Cathulu does not make a second Codex API call as a hidden fallback.
- Use typed adapters and a small static capability registry listing provider locality, supported capabilities, privacy/egress class, configured/enabled state, and availability. Derive latency and outcome history from invocation telemetry instead of duplicating it in a registry. Do not introduce a general plugin runtime or a vendor-specific dependency into the core domain.
- Independently configure each capability with an enabled flag and a shadow/active mode. The effective state is disabled when enabled is false, shadow when enabled is true and mode is shadow, or active when enabled is true and mode is active. Default every enabled flag to false. Per-workflow overrides are versioned. Active is available only for an approved, reversible treatment after the M18 gate.
- Record the exact provider, model/version when available, capability, schema version, workflow version, mode, and configuration hash used for every treatment/run. Configuration should identify local versus remote processing and an explicit remote-allow decision.
- Provider credentials must not be stored in SQLite, run records, events, prompts, or artifacts. Before any remote adapter is implemented, select a supported local credential mechanism and fail closed when it is missing.

### DecisionModel boundaries

Build compact structured decision state deterministically from current Cathulu records. Include only fields relevant to the bounded decision, such as task class, project count, changed-file count, contract-change flag, current QA failures, existing-scope state, and selected aggregate statistics. Do not send a full repository or transcript.

Use versioned output schemas with explicit escapes such as unknown, insufficient_context, needs_codex, needs_more_evidence, and not_applicable where relevant. Strictly validate enum values and response shape. Invalid, malformed, or merely high-confidence output cannot control a workflow. Model-reported confidence/probabilities are kept separate from evidence confidence and never populate a durable memory record's confidence field. Never ask the DecisionModel to assert that tests passed, code is correct, security is safe, a fact is true, or evidence can be deleted.

Initial shadow questions:

- Which bounded workflow class applies: bug fix, feature, dependency fix, architecture, research, verification, other, or needs Codex?
- Does the current work fit Main-only, Sub-Overseer, worker, sub-plus-worker, needs-Codex, or insufficient-context routing?
- After deterministic retrieval only, are selected memory candidates relevant, possibly relevant, irrelevant, or unsupported by the available context?

The last question is deferred until the baseline decision and attribution path is stable. DecisionModel is a candidate reranker, not primary memory retrieval. Deterministic retrieval, deduplication, scope checks, known-by filtering, freshness validation, and supersession checks narrow the candidates first; the model sees only a bounded set and returns candidate scores/IDs that resolve to authoritative SQLite records. The current scope's HOT budget remains controlling.

### ContextModel boundaries

Start with selected compiler, test, CI, Graphify, security-scan, dependency, command, and structured worker outputs. A deterministic preflight chooses bounded spans and removes or masks sensitive material before any remote call. If safe minimization is not possible, skip that call and keep the original evidence local.

Each extracted or compressed result retains its source artifact ID and content hash plus stable byte offsets/source spans or another verifiable locator. A provider's self-reported confidence is labeled as model confidence; evidence confidence is assigned only from source verification or review, never copied from model confidence. Keep the raw artifact retrievable as COLD evidence. Model output is a projection for a task packet, not canonical truth. Never discard or overwrite raw output because a transformation succeeded. Measure output size reduction, source coverage/precision, information-recovery requests, missing-context failures, and latency.

Prefer deterministic parsers for exact compiler/test structures. Compare a local extraction/classification option when practical. A remote provider such as ScaleDown remains optional and must pass current API, terms, privacy, and maintenance review first.

### Shadow, active treatment, and fallback

In disabled mode, do not invoke the provider. In shadow mode, the current baseline controls execution and the specialist prediction/result is recorded only; it cannot affect delegation, context delivery, memory temperature, convergence, approval, QA, or deletion. In active mode, only an approved workflow-specific pilot may use the result for its narrow declared purpose. Each active capability has a fast disable path that restores the same baseline.

Shadow must precede active. To move into an active data-gathering pilot, shadow evaluation must demonstrate valid schemas, tested escape handling, acceptable provider failure/fallback behavior, satisfied privacy constraints, and no quality-rule bypass. Then pre-register the eligible task class, workflow version, mandatory QA, completion criteria, treatment assignment, quality guard, sample/decision method, and rollback criteria. Human approval is required; Cathulu does not promote itself.

### Attribution and privacy

For every provider invocation, record locally when observable:

- provider, model/version, capability, decision-schema version, workflow version, task type, run ID, mode, and configuration/treatment hash;
- input/output bytes and token count if the provider or local runtime exposes them, latency, normalized choice or structured result, confidence/probabilities, and escape selection;
- success, timeout, malformed response, provider error, fallback used, and whether the response influenced execution;
- downstream verified result, QA outcome, human correction, rework, and recovery request when Cathulu has a reliable source;
- a post-run, evidence-linked reviewer assessment of whether a shadow result likely would have helped, hurt, or made no difference, clearly labeled as a counterfactual judgment rather than an observed treatment effect.

Do not retain full request bodies, prompts, complete raw responses, or secrets in telemetry by default. Context outputs used in an active task are bounded, source-linked candidates; persist only what is needed for audit and recovery. Do not claim observability of Codex turns, context tokens, tool actions, or downstream use if no trusted source exposes them.

All Cathulu state, treatment records, evidence references, and telemetry remain local. For an explicitly enabled remote provider, transmit only the selected minimal sanitized fields/spans and make provider use visible in local configuration and run evidence. No whole repositories, secrets, raw memory, complete transcripts, or entire evidence stores. A remote provider being unavailable must not block Cathulu.

## Source-of-truth impact

- [Architecture](../architecture.md): records the provider boundary, fallback, authority, privacy, and relation to M9 selectors.
- [Project definition](../project-definition.md): records local-first and optional-provider invariants and quality objective.
- [Roadmap](../roadmap.md): tracks M15-M19 separately from M10 and M12-M14.
- [ADR-0007](../decisions/ADR-0007-optional-specialist-model-optimization.md): records accepted architectural constraints.
- This plan records implementation, evaluations, evidence, and per-milestone status.
- Audit current SQLite migrations, run/evaluation APIs, events, artifact links, and workflow-version snapshots before adding schema. If existing records cannot safely encode per-run treatment and invocation attribution, make the smallest additive migration; never create a second experiment, memory, or evidence authority.

## Dependencies

- M4 run, workflow, event, memory-policy, transfer, storage, and evidence substrate.
- M6-M9 delegation and available worker/run telemetry where relevant.
- M11's local Graphify retrieval when structurally relevant; Graphify remains optional.
- M15 can proceed without M10 or M12-M14 eligibility and does not require live specialist usage data.
- Before M18a adapters, current provider API, configuration, privacy, and fallback documentation was reviewed. Context7 lookup reached its monthly quota; official TypeSafe and ScaleDown API sources were used. ScaleDown request-retention terms remain unknown and are disclosed before any opt-in call.
- M18 active treatment evaluation requires representative real runs and comparable tasks. Synthetic fixtures test mechanics only.
- No UI dependency. A read-only impact view is considered only if actual data and user workflow justify it.

## Risks and unknowns

| Risk or unknown | Mitigation |
| --- | --- |
| High-confidence but wrong bounded decision changes routing | Escape outcomes, strict schema validation, shadow first, deterministic fallback, quality review, and explicit approval for active treatment. |
| Context extraction omits a needed error or source detail | Preserve raw artifact, retain source spans and hashes, measure recovery requests/missing-context failures, and support immediate retrieval of the source. |
| Remote egress reveals repository or user data | Default disabled, explicit per-capability remote opt-in, deterministic minimization and sanitization, visible run attribution, and skip the call if safe minimization fails. |
| Provider latency/cost offsets fewer Codex turns | Measure end-to-end completion time, retries, rework, and provider latency, not just model response size. |
| Small or unmatched samples look like improvement | Define task strata and decision criteria from baseline variation before scoring; report uncertainty and do not promote from synthetic tests. |
| Provider/API/model changes break reproducibility | Capture provider/model/schema/workflow/treatment identity and error telemetry; keep adapter isolated and the baseline available. |
| A second telemetry/model registry duplicates current systems | Audit and reuse current run, workflow, event, artifact, and M4 evaluation records; keep only a static capability registry unless measured needs prove otherwise. |
| Specialist output is treated as authority or bypasses QA | Typed result semantics, explicit authority boundaries, QA isolation tests, and deterministic final acceptance. |
| Credential handling is unclear | No remote adapter until a local credential mechanism is selected; never put credentials in SQLite or telemetry. |

## Milestones

### M15 — Provider-neutral contracts and run attribution

**Goal:** establish a disabled-by-default, measurable control boundary without changing baseline behavior or invoking a model.

**Subtasks**

- Audit current Rust domain boundaries and schema to identify the smallest contract location and whether current run/workflow/event/evaluation data already represents treatment.
- Define versioned DecisionModel and ContextModel request/result types and Disabled implementations. Keep result data bounded and do not expose generic SQL or arbitrary actions.
- Add independent disabled/shadow/active capability configuration, with disabled defaults and optional workflow-version overrides. Invalid configuration fails closed to disabled.
- Snapshot effective treatment per run and add append-only, privacy-minimized invocation telemetry by extending the existing event/run/evaluation authorities first. Add schema only where the audit proves it necessary.
- Keep credentials and full model input/output out of records. Record local/remote status and egress permission for attribution.

**Acceptance criteria**

- With all capabilities disabled, the existing workflow, run state, memory retrieval, delegation, approval, and QA behavior is unchanged.
- Every capability can be disabled independently; no provider call occurs when disabled.
- Each run has an immutable, inspectable treatment snapshot; provider invocations and failures can be related to that run and workflow version.
- Disabled or misconfigured provider configuration cannot prevent startup, MCP reads, persistence, Git reconciliation, or workflow completion.
- No duplicate memory/evidence/experiment authority is introduced, and no secret or raw request body is written to SQLite or telemetry.
- Existing data survives any additive migration without rewriting past treatment as known.

**Validation**

- Unit/integration tests for independent flags, invalid configuration, baseline-disabled equivalence, immutable attribution, deduplicated/versioned telemetry, and restart persistence.
- Inspect stored records from disposable data and demonstrate that raw request/response bodies and credentials are absent by default.
- Run the existing relevant workspace/frontend checks and Windows local MCP/Tauri restart smoke; capture commands and results in this plan.
- Run git diff validation and document migration/rollback behavior.

**Expected checkpoint:** one locally validated additive checkpoint. No provider adapters or model calls are needed to complete M15.

### M16 — DecisionModel shadow trials

**Goal:** measure bounded routing predictions without changing execution.

**Subtasks**

- Start with workflow class and Main/Sub-Overseer/worker routing questions. Use compact structured state produced deterministically; do not forward full repository content or transcripts.
- Add explicit enumerated outcomes and escape values per versioned schema. Strictly parse, validate, bound, and normalize provider output.
- Implement one adapter only after current provider API, terms, privacy, credentials, supported models, locality, and maintenance have been reviewed. Jev is a candidate, not a commitment.
- Use shadow mode only. Preserve the current Main Overseer/Codex choice as controlling. Record the prediction, escapes/errors, latency and available usage, later outcome, and whether a human corrected it.
- If no verified provider is available, validate the disabled/shadow contract with a test double and keep provider use off; this does not establish model value.

**Acceptance criteria**

- The specialist result never routes work or changes agent activation in shadow mode.
- Unknown, insufficient context, needs Codex/evidence, and not-applicable escapes are representable and tested.
- Malformed, out-of-schema, timed-out, unavailable, or high-confidence incorrect-looking responses cannot control execution or block the baseline.
- Predictions can be compared with final run evidence without treating agreement or confidence as correctness.
- No provider payload contains a whole repository, transcript, secret, or unneeded raw memory.

**Validation**

- Test all allowed choices and escape cases, malformed/oversized response, timeout/provider error, and no-shadow-influence behavior.
- Test privacy-minimized Decision State construction from authoritative records.
- Review a real local provider call only if current docs, explicit egress configuration, and the required credential path are available; otherwise document this part as unimplemented.

**Expected checkpoint:** local validation of bounded shadow behavior; no active routing.

**Result (2026-10-21):** implemented `RoutingDecisionSchema` v1 for workflow class and delegation strategy with explicit escape outcomes; `RoutingDecisionState` is derived from the existing run, task-status, project, QA, and delegation rows and excludes free-form text and repository paths. The trial runner accepts only that exact minimized SQLite projection, requires a local shadow treatment, validates and normalizes bounded results, writes metadata to the existing append-only event ledger, and does not return a routing action. Provider errors, timeouts, skipped results, and invalid output are recorded as non-influencing fallback outcomes. A post-run human review event can link the prediction to final-run and QA evidence. No migration was needed.

Jev was not integrated or called. Context7 `resolve-library-id` returned “Monthly quota reached”; the current [Jev AI API documentation](https://thejevai.com/docs), [service terms](https://thejevai.com/terms-of-service), and [API repository README](https://github.com/jev-ai/jev-api) were reviewed directly. The API docs describe a remote bearer-key endpoint and typed request/response; the repository says thejevai.com is an independent app rather than the official product site for the underlying Jev model. The published terms do not clarify request retention/data handling or model-operator identity, and no supported local credential path has been selected. The M16 runner therefore records remote configuration as skipped without calling it. Tests use only a deterministic local double; this validates mechanics, not model quality or product value.

**Validation evidence:** `cargo test --offline --workspace --test optimization_m16` — 6 passed, 0 failed. Full-workspace validation is recorded under Completion evidence after the final gate.

### M17 — ContextModel shadow extraction

**Goal:** assess whether extraction can reduce large tool-result interpretation while keeping source evidence intact.

**Subtasks**

- Choose a narrow first corpus of test/compiler/CI/Graphify/tool results with stable artifact references.
- Implement a selected-span/minimized-input path and preserve the raw artifact, content hash, byte offsets/source spans, artifact ID, and model result provenance.
- Compare a deterministic parser and/or feasible local classifier/extractor with an optional ContextModel provider. ScaleDown remains a candidate pending current provider review; GLiNER-family or another local alternative also requires current docs, license, runtime, and maintenance review.
- Start with shadow output. Measure useful structured errors/files/locations, missed evidence, output bytes, extraction latency, recovery requests, and human correction.
- Do not embed raw logs, transcripts, whole Graphify snapshots, or exact current state as semantic memory.

**Acceptance criteria**

- The raw source remains retrievable byte-for-byte through its evidence reference after extraction or compression.
- Every candidate fact/error links to the source artifact and verifiable source span/offset or locator.
- Unsafe-to-minimize remote inputs are skipped without affecting local evidence or the baseline workflow.
- Shadow extraction is not used to omit source context from Codex and cannot affect QA or verified success.
- Extraction quality and limitations are evaluated against source-level review or deterministic expectations.

**Validation**

- Test provenance/span/hash preservation, structured output validation, raw-source retrieval, empty/malformed/provider-failure cases, and remote egress filtering.
- Use representative real or safely captured tool outputs; synthetic cases validate mechanics but do not prove product value.
- Document the deterministic/local comparison and information-recovery rate.

**Expected checkpoint:** local shadow extraction evidence; no active compression or source replacement.

**Result (2026-10-21):** implemented an M17-only selected-span loader for registered local `test_log`, `compiler_log`, `ci_log`, `tool_output`, and `tool_result` artifacts. It enforces file-size and UTF-8 bounds, verifies any registered hash, selects error-adjacent byte spans, and provides only the selected text plus a stable `cathulu://artifacts/{id}` reference to a local ContextModel. The filesystem path stays inside the loader. Model candidates require a closed kind, bounded summary, exact quote-to-source-byte match, selected-range membership, matching artifact IDs/content hashes, and a bounded response. The original file remains untouched and retrievable by its artifact reference.

The run-attributed shadow path records invocation metadata and a separate append-only extraction event in the existing event ledger. The event includes source IDs/hashes, selected-input hash and size, raw-source size, deterministic-baseline hash/count, exact candidate span overlap counts, model result hash, and the validated bounded result. Remote providers are recorded as skipped before invocation; empty selected inputs, invalid responses, timeouts, and provider errors also do not affect the run. There is no schema migration, active mode, provider adapter, frontend change, QA change, or context replacement.

Validation includes seven integration tests and a locally generated failing Rust test-harness output captured as a real tool artifact. The model used in those tests is a deterministic test double; exact span checks and baseline overlap validate mechanics, not model quality, information-recovery rate, Codex savings, or workflow value. At the M17 checkpoint, ScaleDown and local model candidates remained unreviewed/unintegrated; M18a later reviewed and integrated the optional ScaleDown provider adapter. M18 must not claim benefit from this sample.

**Validation evidence:** `cargo test --offline --test context_extraction_m17` — 7 passed, 0 failed. Full-workspace validation is recorded under Completion evidence after the final gate.

### M18a — Optional Jev and ScaleDown provider connectivity

**Goal:** make the first provider shadow calls testable after local key setup while preserving the M16/M17 no-influence, provenance, and egress boundaries.

**Subtasks**

- Add typed Jev and ScaleDown HTTP adapters behind the existing DecisionModel and ContextModel interfaces. Use the TypeSafe System One endpoint for only the fixed workflow/delegation choice schemas; use the ScaleDown Extract endpoint for selected failure excerpts.
- Read only the named provider keys from process environment or a simple .env.local file, with environment values taking precedence. Support CATHULU_ENV_FILE for an explicit local file. Do not evaluate shell syntax, log credentials, or store them in SQLite/events.
- Expose the immutable per-run treatment input through MCP and async shadow-trial tools. A provider call requires the matching capability to be enabled in shadow mode, provider locality set to remote, remoteDataAllowed set to true, and its local key available.
- Keep ScaleDown inputs to selected spans from registered local artifacts for the same run. Verify the raw artifact hash, omit task text/paths/artifact IDs from the provider request, best-effort mask credential labels and known token forms, and accept only exact returned quotes with validated source offsets. Preserve the original artifact.
- Keep both providers disabled by default. Missing credentials or provider failure must record a skipped/failure invocation and leave run state, QA, context delivery, routing, and success unchanged. Active mode remains gated by M18.

**Acceptance criteria**

- The all-disabled baseline makes no provider call, and an API key alone does not enable a capability.
- Each remote shadow call requires the immutable run treatment’s explicit remote opt-in and is attributed through the existing append-only run event ledger.
- Jev receives only the bounded SQLite routing projection and fixed outcome choices; ScaleDown receives only selected sanitized spans.
- Context artifacts from a different run are rejected before model invocation.
- Raw evidence remains retrievable; provider output is a validated projection, never authority for QA, correctness, or success.
- Provider failures and malformed responses do not block the underlying run. Credentials and raw provider request/response bodies are not persisted.
- The provider setup guide discloses that ScaleDown request retention was not found in reviewed public API docs and that credential masking is best-effort.

**Validation**

- Use loopback-only HTTP mock servers to validate Jev bearer authentication, request minimization, bounded response parsing, provider usage metadata, ScaleDown x-api-key authentication, selected-span minimization, credential masking, exact quote/span mapping, and token/byte metadata. These tests must not contact external providers.
- Run the context extraction suite, including same-run artifact ownership and unchanged raw evidence, plus decision-trial tests and the full Rust workspace suite.
- Check formatting, dependency lock consistency, and Git whitespace. Do not claim live provider quality, retention terms, or workflow value from local mocks.

**Expected checkpoint:** local connector readiness. Live shadow trials begin only after the owner adds the chosen keys locally and explicitly starts a consented remote shadow run. M18 comparative scoring and any active treatment remain separately gated.

**Public MCP contract follow-up (2026-10-21):** a real MCP subprocess test supplies an immutable run treatment for Jev workflow routing and ScaleDown extraction, removes both provider keys from the child process, and points `CATHULU_ENV_FILE` at a nonexistent file. Both public shadow tools return `skipped` and append attributed invocation events with `fallbackUsed=true` and `influencedExecution=false`. The run and pending QA check are byte-for-byte equivalent before and after the calls; the raw local test artifact remains unchanged. This closes the public tool/fallback test gap without making a live provider call. It validates missing-key behavior only, not provider response quality or product value.

**Validation evidence:** `cargo test --offline --test mcp_vertical_slice public_mcp_specialist_shadow_trials_skip_without_keys_and_preserve_run_state -- --exact --nocapture` — 1 passed, 0 failed. Full-workspace results are recorded in the milestone closeout below.

**ScaleDown response-schema correction (2026-10-21):** review of the merged adapter found its request schema described four top-level strings while its parser required an `entities` array of typed exact spans. The parser also defaulted a missing array to empty, making a malformed response indistinguishable from a valid no-match result. The request now asks for the exact array/field shape consumed by the parser, constrains entity kinds and offsets, and disallows unknown fields; a missing `entities` field now fails closed. A regression test checks the request schema against the deserialization contract. This corrects local protocol consistency; it does not establish live-provider compatibility or extraction quality.
### M18 — Controlled ablation and evidence review

**Goal:** determine actual efficiency and quality impact under comparable conditions.

**Entry gate**

- M15-M17 validation passes for the capabilities proposed for active pilot.
- A treatment protocol defines task inclusion, task class/complexity strata, same workflow version, mandatory QA, completion criteria, baseline, treatment assignment, acceptable quality guard, measures, sample/decision method, and stop/rollback conditions before scoring.
- Shadow evidence supports response validity and fallback safety. Active participation is a separate explicit owner decision.

**Subtasks**

- Compare BASELINE (all specialists disabled), decision-only, context-only, and both only where the specific treatment has passed the entry gate.
- Keep task classes/complexity, workflow versions, acceptance criteria, Codex model/effort configuration, and mandatory QA gates comparable. Record the exact treatment and provider version for each run.
- Measure verified success, mandatory QA, convergence where trustworthy, regressions, rework, human intervention, Codex turns/context when observable, context reintroduction/amplification, unnecessary agent activations, tool/reinspection count, retrieval overhead, elapsed time, retries, and storage/provider overhead.
- Analyze quality and efficiency together. Stratify by task complexity; report missing observations and uncertainty. Do not infer savings from model confidence, agreement, token estimates that were not observed, or easier task assignment.
- Produce owner-reviewed results; continue shadow/disabled status when evidence is inconclusive.

**Acceptance criteria**

- Each treatment is traceable to an immutable run/workflow configuration and uses the same required QA/completion gates.
- At least one predeclared, baseline-derived criterion supports any efficiency claim, and the predeclared quality guard is maintained. The threshold is chosen from actual observed variability, not invented in this plan.
- Provider latency, recovery, rework, regressions, QA, and human intervention are included; a reduction in one metric cannot mask material quality loss.
- An active capability remains per-workflow and reversible. No global default changes automatically.
- Insufficient, unmatched, or biased evidence is reported as inconclusive.

**Validation**

- Re-run deterministic QA and all applicable privacy, fallback, QA-isolation, attribution, and rollback tests for each active trial.
- Review the run cohort for selection/complexity imbalance and verify treatment records before comparing metrics.
- Capture raw counts/denominators, measurement availability, task strata, method, limitations, and owner decision in this plan.

**Expected checkpoint:** an evidence-backed keep-disabled, continue-shadow, controlled-active, or reject recommendation for each capability.

### M19 — Additional evidence-backed capability proposals

**Goal:** test other narrow capabilities only when M18 data and actual retrieval/workflow gaps justify them.

Possible one-at-a-time experiments:

- Post-retrieval memory relevance, promotion, or temperature suggestions after SQL/FTS/Graphify candidate narrowing. These remain suggestions for the deterministic Gardener and owner review; the specialist cannot change class/temperature or delete evidence itself.
- Query-conditioned compression with immutable source references and missing-information recovery checks.
- Scope/run summarization as a provenance-linked projection.
- Classification of large unstructured input where deterministic parsing is insufficient.
- Scope activation and secondary convergence/stagnation classification, only as advisory signals with existing deterministic state and M10/M12 gates preserved.

**Acceptance criteria**

- Each capability has its own independent flag, shadow stage, provider/failure record, run treatment, baseline, and rollback switch.
- Memory retrieval remains deterministic-first and results resolve to authoritative SQLite memory records. A specialist cannot promote/demote/delete memory without existing deterministic rules and owner review.
- Convergence, safety, approvals, and success semantics remain deterministic and unchanged.
- Suggestions include proposed workflow/capability change, comparable sample size, baseline/treatment, quality and efficiency deltas, uncertainty, and known risks.
- No orchestration policy is rewritten automatically. A compact dashboard impact view is added only if the data supports a clear, read-only use.

**Validation**

- Reuse the relevant M15-M18 contract, egress, artifact, QA, attribution, and comparison tests.
- Verify a single capability disabled at a time leaves other capability flags and the baseline intact.
- Require real comparable data for product-value claims; otherwise retain a future experiment proposal only.

**Expected checkpoint:** one evidence-backed capability evaluation at a time; no automatic policy promotion.

## Measurement dictionary

| Dimension | Measures | Source/limit |
| --- | --- | --- |
| Verified quality | Verified task success, mandatory QA results, regressions, convergence quality, rework, human corrections/intervention | Prefer existing run and quality-check sources; define convergence only where a validated source exists. |
| Decision efficiency | Codex reasoning turns, Sub-Overseer/worker activations, unnecessary activations, retries, repeated inspection, tool calls, elapsed time | Count only where Cathulu has an observable source; otherwise record unknown or a documented manual measurement. |
| Context efficiency | Input/output/context bytes or tokens, reintroduction, amplification, repeated processing, retrieval overhead, recovery requests | Reuse M4 transfer/retrieval data; token counts remain null when unavailable. |
| Decision-model quality | Decisions per call, choice/escape rate, latency, provider errors, schema failures, agreement with final outcome, high-confidence errors, corrections, counterfactual escalations avoided | Shadow agreement and counterfactual assessments are diagnostic only; deterministic run outcome remains the reference. |
| Context-model quality | Extraction precision/completeness against reviewed source, compression ratio, source coverage, missed evidence, recovery requests, latency | Keep raw artifact and reviewed source reference. |
| Operational cost | Provider/model usage when exposed, total latency, availability, retries, storage growth, maintenance/failure rate | Store only available provider metadata; no assumed pricing/API fields. |

## Required tests

- Providers/capabilities are independently enabled or disabled; all-disabled behavior matches the current baseline.
- Shadow results are persisted and attributable but never affect execution.
- Provider failure, timeout, malformed output, unavailable credentials, or invalid configuration falls back safely and does not block a run.
- Decision schemas support escape outcomes; invalid output cannot control routing.
- Context extraction/compression retains source artifact references and spans; the raw evidence remains retrievable.
- No specialist inference can bypass mandatory QA, approvals, safety policy, or deterministic success criteria.
- Every run records the effective immutable treatment configuration; every invocation links to its run/workflow and records whether it influenced execution.
- Remote requests pass an explicit allow check and egress minimization; secrets, whole repositories, transcripts, raw memory, and full artifact stores are excluded by default.
- Memory reranking returns candidates that resolve to authoritative SQLite rows and cannot make COLD evidence auto-injected.
- Active pilot can be disabled and returns the workflow to the same all-specialists-disabled baseline.

## Rollback / recovery

Set the affected capability to disabled and resume the existing Cathulu/Codex path. Preserve treatment history, provider errors, source artifacts, run outcomes, and QA evidence. Do not delete evidence or rewrite prior run treatment. A provider outage, privacy concern, schema change, quality regression, or unexpected recovery request is a reason to disable the affected capability while retaining the evidence for review. Rollback never changes M9 Codex settings.

## Progress

- [x] Inspect the existing M4 memory/run/telemetry substrate and M9/M11 boundaries.
- [x] Record the optional specialist architecture and M15-M19 plan. No implementation or provider call is included in this documentation milestone.
- [x] Validate documentation with git diff whitespace checks, local link resolution, and a trailing-whitespace scan.
- [x] M15 provider-neutral contracts, disabled-by-default configuration, immutable run attribution, and append-only telemetry; merged via [PR #6](https://github.com/jmsb505/cathulu/pull/6) at `bf9e9a9b5f78ef45da6f31b549fdd857c80201c8`.
- [x] M16 local shadow-trial mechanics, strict decision schemas, SQLite routing-state minimization, and evidence-linked human review; no provider or live call.
- [x] M17 local ContextModel shadow extraction boundary and selected-span provenance; no provider or live call.
- [x] Re-audit M18 provider documentation and current Cathulu run readiness; preserve the no-run/no-egress gate.
- [x] M18a optional Jev and ScaleDown provider connectivity; local mock protocol checks and egress/same-run safeguards validated. No live call was part of that implementation closeout; see the first live Jev trial below.
- [x] M18a public MCP follow-up verifies missing-key skips, immutable run/QA state, and raw-artifact preservation without provider calls.
- [x] M18b ScaleDown response-schema correction; merged via [PR #12](https://github.com/jmsb505/cathulu/pull/12) at `6b609450d2fe71925cc5b5d7c571398d2622cd0f`.
- [x] First owner-approved Jev workflow-routing shadow integration run after MCP reload; invocation telemetry and verified unchanged run/QA state recorded locally. This is not a model-quality or product-value result.
- [ ] M18 controlled ablation and evidence review.
- [ ] M19 additional evidence-backed capability proposals.

## Implementation discoveries / decisions

- 2026-10-21: This plan adds an optional provider layer around the current local Cathulu/Codex workflow; it does not change existing memory classes, temperatures, M9 Codex selectors, QA authority, or M10/M12-M14 gates.
- 2026-10-21: Shadow results cannot demonstrate realized reductions in Codex work because they do not affect execution. They establish safety and prediction/extraction quality only; a separate explicitly approved, reversible active pilot is needed to measure actual impact.
- 2026-10-21: Provider candidate names are not a commitment. Current primary documentation, API, privacy/terms, credentials, local alternatives, and maintenance need review before adapters are implemented.
- 2026-10-21: M15 uses the existing append-only `run_started` event for immutable treatment snapshots and `external_event_id` for idempotent invocation telemetry. The audit found no need for a schema migration or a parallel experiment/telemetry table. Invalid base or workflow override configuration is captured as an all-disabled fallback so it cannot prevent run creation. Active settings are represented in the contract but rejected until the M18 owner-approved pilot gate.
- 2026-10-21: `Store::start_run` continues to use the all-disabled defaults. A host may pass a typed `OptimizationConfig` to `start_run_with_optimization`; an immutable workflow version can override selected entries under its `optimization.capabilities` object. This milestone does not add a settings UI, provider registry, credential store, provider adapter, or model call.
- 2026-10-21: M16 uses only the existing event ledger for shadow invocations and post-run counterfactual review; no schema migration or duplicate experiment store was necessary. The SQLite-derived routing projection excludes user task text, repository paths, artifact contents, memory bodies, and summaries. The call boundary enforces this projection exactly and currently accepts only local providers.
- 2026-10-21: Primary TypeSafe OpenAPI documentation confirms `POST https://api.typesafe.ai/v1/systemone`, bearer-key authentication, and the bounded `model`/`state`/`questions` request shape ([OpenAPI](https://api.typesafe.ai/openapi.json)). TypeSafe's data-handling FAQ says requests are not used to train models but may be processed for service delivery, usage charges, abuse prevention, and telemetry; retention depends on the applicable agreement, with zero-data-retention offered for enterprise accounts ([data handling](https://help.typesafe.ai/articles/5974925716-how-is-submitted-data-used-or-retained)). This makes Jev a candidate only for the minimized SQLite-derived routing state. No remote egress approval, adapter, or live call is present.
- 2026-10-21: ScaleDown's official Python client documents an API-powered compression service and its `SCALEDOWN_API_KEY`/`SCALEDOWN_API_URL` settings ([official client](https://github.com/scaledown-team/scaledown)). The sources reviewed do not verify a selected-span extraction API or request-retention terms; do not send selected context to ScaleDown until its extraction contract and retention terms are verified and remote processing is explicitly enabled.
- 2026-10-21: M17 reuses the existing evidence-artifact registry and event ledger without a migration. The extraction request contains selected text and a stable Cathulu resource ID, never the local filesystem path; the output event stores hashes and validated spans while retaining the raw artifact by reference. Remote extraction remains unimplemented and is skipped regardless of configured remote opt-in.
- 2026-10-21: `.env.example` documents empty reserved decision/context API-key placeholders and `.gitignore` excludes local `.env` files. The runtime currently has no dotenv loader, provider adapter, or secure credential path; adding key values to `.env.local` does not activate a provider.

- 2026-10-21: M18a adds optional Jev and ScaleDown adapters without changing the all-disabled baseline. Jev calls TypeSafe’s official POST /v1/systemone endpoint with bearer auth and only the compact SQLite routing state. ScaleDown calls the official POST /v1/chat/completions Extract API with x-api-key auth and selected local excerpts only. The reviewed [TypeSafe OpenAPI](https://api.typesafe.ai/openapi.json), [TypeSafe data-handling FAQ](https://help.typesafe.ai/articles/5974925716-how-is-submitted-data-used-or-retained), [ScaleDown Extract API](https://docs.scaledown.ai/api-reference/extract), and [ScaleDown quickstart](https://docs.scaledown.ai/quickstart) establish request/response shape; the reviewed ScaleDown pages do not state request-retention terms.
- 2026-10-21: Provider credentials are read from only CATHULU_DECISION_PROVIDER_API_KEY and CATHULU_CONTEXT_PROVIDER_API_KEY, from process environment or a local .env.local/CATHULU_ENV_FILE. They are never added to database, events, or provider telemetry. The MCP tools require a matching immutable shadow treatment with providerLocality remote and remoteDataAllowed true; active mode remains rejected.
- 2026-10-21: ScaleDown extraction now checks that all selected artifacts belong to the trial run, masks common credential-labeled values/token formats, and discards candidate quotes overlapping masked source bytes. This is a best-effort filter, not a guarantee that every secret is recognized; the setup guide instructs the owner to review selected evidence before remote consent.
- 2026-10-21: Reqwest is configured for JSON, Rustls TLS, and system proxy support with default features disabled; unused HTTP/2 is not enabled. The HTTP tests use loopback mock servers and fake test keys only. No external provider call, active treatment, or product-value claim is included.
## Completion evidence

M15 passed local Windows validation on 2026-10-21. Commands and results:

- `cargo fmt --all -- --check` — passed.
- `cargo test --offline --workspace` — passed: core unit tests (10), delegation lifecycle (4), MCP vertical slice (16), memory lifecycle (4), M15 optimization tests (5), desktop library tests (3 passed, 1 existing integration test ignored by design), and doc tests.
- `npm ci` in `apps/desktop` — installed the locked dependencies; 0 vulnerabilities reported. No package manifest or lockfile change was made.
- `npm test -- --run` — passed (5 tests).
- `npm run build` — passed.
- `scripts/phase1-m4-integration-smoke.ps1` — passed, including the MCP/Tauri shared-state restart integration test and hidden Windows desktop startup.
- M15 tests confirm that invocation payloads contain metadata only, reuse of an invocation ID is idempotent, differing telemetry under the same ID is rejected, and existing schema version 9 is unchanged.
- Rollback is a code rollback only: no schema migration or new provider is present. Existing v9 databases and append-only treatment events remain readable; a prior binary ignores the additional `run_started` field and unknown invocation event type.

M16 passed local Windows validation on 2026-10-21. Commands and results:

- `cargo fmt --all -- --check` — passed.
- `cargo test --offline --workspace --test optimization_m16` — passed (6 tests).
- `cargo test --offline --workspace` — passed: core unit tests (10), delegation lifecycle (4), MCP vertical slice (16), memory lifecycle (4), M15 optimization tests (5), M16 optimization tests (6), desktop library tests (3 passed, 1 existing Windows integration test ignored by design), and doc tests.
- M16 tests cover fixed outcome/escape schemas, strict/oversized response parsing, probability normalization, SQLite-derived minimized state, rejected unminimized state, no shadow influence, disabled/remote/failure fallback, and append-only post-run review idempotency and persistence after reopening SQLite.
- No migration or frontend change was required. The M16 runner rejected remote providers before invoking a model. No provider call, active treatment, or product-value result is claimed.

M17 passed focused local Windows validation on 2026-10-21. Commands and results:

- `cargo fmt --all` — passed; final `--check` is recorded after the full gate.
- `cargo test --offline --test context_extraction_m17` — passed (7 tests).
- Tests cover selected byte spans and hashes, non-disclosure of local paths, raw artifact retrieval, a captured local Rust test-harness failure, deterministic candidate provenance, shadow run isolation, malformed quote rejection, provider timeout/unavailability, remote and empty-input skips, changed-hash rejection, and unsupported artifact kinds.
- `cargo fmt --all -- --check` — passed.
- `cargo test --offline --workspace` — passed (55 passed, 0 failed, 1 existing desktop integration test ignored by design; doc tests passed).
- `git diff --check` — passed. `.env.local` is ignored and `.env.example` remains trackable.
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — not clean because of 10 existing lints in `codex_config_files.rs`, `gardener.rs`, `graphify.rs`, `mcp.rs`, `memory.rs`, `workflow.rs`, and `lib.rs`. The strict run reported no remaining M17 lint after cleanup; unrelated lint debt was left unchanged.
- No migration, dependency, provider adapter, real model call, active treatment, UI, or product-value claim was added. Model effectiveness, information-recovery rate, Codex-turn savings, and latency impact remain unknown.

M18 pretrial readiness audit (2026-10-21): at that snapshot, the read-only `cathulu://dashboard` reported `recentRuns=[]`, `objectives=[]`, `blockers=[]`, and `failedChecks=[]`; memory, transfer, and storage counters were all zero. No comparable baseline cohort existed, so sample size, expected variation, and a quality guard could not be responsibly selected. The TypeSafe API and data-handling sources were reviewed; at that point no provider adapter, credential loader, live model call, remote transmission, active treatment, or product-value measurement had been implemented or claimed. M18 remains ineligible for comparative scoring until representative real Cathulu runs exist; any active pilot additionally requires explicit owner approval.

M18a provider-connectivity readiness passed local Windows validation on 2026-10-21. Commands and results:

- `cargo fmt --all -- --check` — passed.
- `cargo test --offline --workspace` — passed: 64 passed, 0 failed, 1 existing desktop integration test ignored by design; doc tests passed.
- `git diff --check` — passed after the final evidence-only plan update.
- `cargo check --locked --offline --workspace --all-targets` — passed during M18a implementation.
- Provider tests use loopback HTTP fixtures and fake keys only. They cover Jev bearer auth and bounded routing state, ScaleDown x-api-key auth and excerpt minimization, secret masking and exact source-span mapping, and credential parsing without shell evaluation. M17 tests also cover same-run artifact enforcement and non-blocking fallback.
- No real API key was read, no live provider call or external data transmission was made, and no active optimization mode was enabled. Provider quality, retention behavior beyond the reviewed documentation, and workflow impact remain unmeasured. ScaleDown retention is unspecified in the reviewed API pages; credential masking is best-effort.

M18a public MCP fallback follow-up passed local Windows validation on 2026-10-21:

- `cargo fmt --all -- --check` — passed.
- `cargo test --offline --test mcp_vertical_slice public_mcp_specialist_shadow_trials_skip_without_keys_and_preserve_run_state -- --exact --nocapture` — passed (1 test).
- `cargo test --offline --workspace` — passed (65 passed, 0 failed, 1 existing desktop integration test ignored by design; doc tests passed).
- `cargo check --locked --offline --workspace --all-targets` — passed.
- `git diff --check` — passed.
- The subprocess test explicitly removes both provider keys and sets `CATHULU_ENV_FILE` to a nonexistent path. No API key was read, and the test could not make a live provider call. Mocks and missing-key behavior validate contracts and fallback only; no quality or product-value result is claimed.
- The currently connected Cathulu MCP tool schema in this Codex session still exposes the older `start_run` shape and does not list the M18a shadow tools. Before owner-initiated manual trials, rebuild the configured MCP executable using [the local MCP setup guide](../local-mcp.md) and reload/restart the MCP server. No user Codex MCP configuration was changed during this milestone.

M18b ScaleDown response-schema correction was merged on 2026-10-21 via [PR #12](https://github.com/jmsb505/cathulu/pull/12), commit `6b609450d2fe71925cc5b5d7c571398d2622cd0f`. Local Windows validation:

- `cargo fmt --all -- --check` — passed.
- `cargo test --offline --workspace` — passed (66 passed, 0 failed, 1 existing desktop integration test ignored by design; doc tests passed).
- `cargo check --locked --offline --workspace --all-targets` — passed.
- `git diff --check` — passed.
- The new focused regression checks that the requested JSON schema exactly defines the parser's required `entities` shape and rejects a response missing `entities`. No provider key was read and no live request or external transmission occurred.
- GitHub reported no workflow runs or check contexts for the PR head; the `main` branch API reports protection disabled. No configured CI gate was available. No provider key was read and no live request or external transmission occurred. Provider contract compatibility and extraction quality remain unverified.

First owner-approved live Jev shadow integration run (2026-10-21): local Cathulu run 1 used immutable workflow `specialist_shadow_validation` v1 and enabled only `workflow_routing` in `shadow` mode. The refreshed MCP server read Cathulu at clean `main` HEAD `4d3e7b81541697052a484f46ab1fbc5c5bc3adb6`. Jev reported model version `jev-1.13.0`, prediction `insufficient_context`, confidence 0.35, and 326 ms latency; telemetry recorded 1,746 input bytes, 758 input tokens, and 125 output tokens. The invocation had `influencedExecution=false` and did not alter run status or QA checks; the integration run finished `verified_success`. Raw request/response payloads were not persisted. This is one integration observation, not evidence of routing accuracy, efficiency, or product value. ScaleDown was not invoked, active treatment remains disabled, and M18 comparative scoring remains pending a representative cohort.
