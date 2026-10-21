# ADR-0007: Optional specialist-model optimization

Date: 2026-10-21
Status: ACCEPTED
Scope: Optional provider-neutral decision and context optimization around the local Cathulu/Codex workflow
Supersedes / superseded by: None; supplements ADR-0001 and ADR-0002 without changing Codex execution ownership or M9 selector behavior

## Context

Cathulu may benefit from narrow models that make bounded choices or reduce already-existing information before Codex consumes it. Potential gains include fewer Codex reasoning turns, unnecessary agent activations, repeated context processing, and tool-result interpretation overhead. Those gains are useful only if verified task success, QA, convergence, regression rate, rework, and human intervention are maintained or improved.

The existing system already has deterministic state and QA, local SQLite authority, run and workflow history, memory provenance and temperature, selective retrieval, context-transfer records, and evidence artifacts. A new optimization layer must extend those systems, not create parallel state, memory, experiment, or agent authorities.

The user requested that Codex remain the primary reasoning/coding system, that no specialist be mandatory, and that shadow evaluation precede active use. Provider examples such as Jev, ScaleDown, and GLiNER-family models are candidates; their current API, terms, availability, locality, and capabilities are not established by this decision.

## Decision

- Prefer deterministic Cathulu code for exact facts, rules, state, validation, freshness, and tests.
- Define two small provider-neutral optional interfaces: DecisionModel for bounded choices over compact structured inputs, and ContextModel for classifying, extracting, compressing, or summarizing selected existing information.
- Keep Codex as the primary system for open-ended reasoning, coding, architecture, planning, debugging, synthesis, and interaction. Specialist models do not replace or silently reconfigure Codex.
- Keep deterministic evidence and QA authoritative. A specialist cannot declare correctness, test success, security safety, architectural truth, or verified run success; it cannot bypass approval or autonomously delete or promote durable memory.
- Make every capability independently configurable per workflow, with an enabled flag and shadow/active mode. The effective state is disabled when the flag is false; all flags default to false. Disabled operation must preserve the existing baseline workflow.
- Require shadow mode before any active treatment. Shadow predictions never influence execution. Active treatment requires valid shadow behavior, a pre-registered comparison and quality guard, and explicit owner approval of a reversible pilot. Cathulu never promotes its own policy.
- Include explicit escape outcomes in bounded decision schemas, such as unknown, insufficient_context, needs_codex, needs_more_evidence, and not_applicable where appropriate. A valid-looking or high-confidence result is not itself evidence of correctness.
- Use compact, deterministically constructed decision state. Do not send whole repositories or full transcripts. Context outputs remain source-linked projections; the raw evidence stays retrievable.
- Keep Cathulu's database, memory authority, artifacts/evidence references, treatment records, and telemetry local. A remote specialist is an explicit egress exception requiring owner configuration, minimized/sanitized selected input, and local run attribution. Whole repositories, secrets, raw Cathulu memory, full transcripts, and entire artifact stores are excluded by default.
- Reuse existing runs, immutable workflow versions, append-only events, memory-policy evaluations, context transfers, and evidence artifact references. Audit before adding schema; make only the smallest additive change needed for immutable treatment and invocation attribution.
- A provider failure, timeout, invalid response, unavailable credential, or unsafe-to-minimize input skips the optimization and returns to the existing Cathulu/Codex path. Cathulu does not make a hidden Codex API call as fallback.
- Keep M9's Codex model/effort selector settings separate. They configure Codex; specialist provider settings are a distinct optional capability configuration and do not change the model or effort of a live Codex turn.
- Compare providers with deterministic and local alternatives where practical. Use a small static capability registry and typed adapters, not a general plugin runtime.
- Keep the dashboard read-only. A compact impact view may be proposed only after real data justifies it.

## Alternatives considered

- **Use a specialist model as a required step:** rejected because it would make Cathulu unavailable on provider outage, add needless latency/cost, and violate the requirement that baseline workflows work with all specialists disabled.
- **Replace Codex with a smaller model for general reasoning or coding:** rejected because Codex remains the owner's primary reasoning and implementation system.
- **Call a vendor directly from workflow logic:** rejected because vendor-specific code would couple policy to one provider and make disabled behavior and local comparison harder to validate.
- **Let a model decide correctness, QA, or retention:** rejected because model inference cannot replace deterministic checks, provenance, or reference-aware memory safety.
- **Enable providers by default after a shadow agreement score:** rejected because shadow cannot show realized downstream efficiency, and agreement alone cannot establish benefit. A controlled, explicitly approved treatment is needed to measure actual effect.
- **Build a general plugin or model-management console now:** rejected because no evidence justifies a runtime/plugin system or large UI. Typed interfaces and a small static registry suffice initially.

## Consequences

- [PLAN-004](../plans/PLAN-004-optional-specialist-model-optimization.md) defines M15-M19: contracts and attribution, DecisionModel shadow, ContextModel extraction shadow, controlled ablation, and later evidence-backed proposals.
- M15 can proceed independently of the M10 and M12-M14 real-work gates. M16-M19 must preserve those gates and use real comparable runs for product-value claims.
- Shadow experiments establish schema, privacy, fallback, latency, and candidate quality; they do not prove reduced Codex context or workflow time.
- Every invocation needs treatment attribution and bounded telemetry. Unobservable Codex turns, token use, downstream use, and provider usage remain unknown rather than guessed.
- Context transformations must retain artifact identity/hash and verifiable source spans or locators. Compressed/extracted content cannot replace its source.
- A remote adapter requires current primary API/privacy review, secure local credentials, minimized input, and explicit per-run egress consent. M18a implements only the Jev and ScaleDown shadow adapters under those controls; ScaleDown request-retention terms remain unknown and are disclosed.
- Jev, ScaleDown, and GLiNER-family models remain evaluation candidates, not required providers or assumed supported APIs.
- Any active pilot remains per-workflow and reversible. Human approval is required before active execution; no policy adaptation happens automatically.
- Existing memory authority, provenance, temperature classes, Gardener constraints, and evidence-retention decisions remain unchanged. Candidate memory reranking occurs only after deterministic retrieval and never grants deletion authority.

## M16 implementation decision

M16 implements bounded workflow/delegation schemas, a compact SQLite-derived state projection, local-only shadow evaluation, and an append-only post-run review event. No remote provider adapter or model call is included. Context7's current-documentation lookup returned a monthly quota limit, so the current Jev AI API documentation, service terms, and API repository README were checked directly. The API page documents a remote endpoint, bearer-key requirement, model IDs, request schema, and response shape; the repository describes thejevai.com as an independent app rather than the official site for the underlying Jev model. The published terms do not establish request retention/data handling, model-operator identity, or Cathulu's secure credential path. The M16 runner records remote configurations as skipped without calling the service until those items are verified. This keeps Jev as a candidate and does not alter the all-disabled baseline.

## M17 implementation decision

At the M17 checkpoint, extraction read only registered local test/compiler/CI/tool-output artifacts through existing evidence references. It verified a stored content hash when present, selected bounded error-adjacent byte spans, and gave a local ContextModel a stable `cathulu://artifacts/{id}` reference rather than a filesystem path. Candidate output had to match its artifact IDs/hashes, schema, selected ranges, exact source quotes, and output bound. The immutable source remained available, and invocation/result evidence reused append-only run events without a migration. M18a later added opt-in remote adapters under the provider and consent constraints below. These tests establish the shadow boundary only; deterministic doubles and missing-key fallback do not establish provider quality or workflow benefit.

## M18a provider connectivity decision

The owner explicitly approved adding optional Jev and ScaleDown connections so local API keys can be configured and shadow testing can proceed. These adapters supplement M16/M17 and preserve the architecture decision above:

- Jev uses TypeSafe’s official POST https://api.typesafe.ai/v1/systemone endpoint with bearer authentication. The request contains only Cathulu’s bounded SQLite routing projection and the predefined workflow/delegation choices. It cannot receive task descriptions, source text, or caller-supplied state.
- ScaleDown uses its official POST https://api.scaledown.xyz/v1/chat/completions Extract API with x-api-key authentication. It receives only selected spans from registered local test/compiler/CI/tool-output artifacts. Cathulu omits task text, artifact IDs, source references, and local paths; it best-effort masks credential-labeled values and known token formats. Returned quotes must map to exact unmasked bytes in the registered source, and artifacts must belong to the same run.
- Both providers are selected only by an immutable per-run shadow treatment with remote locality and explicit remoteDataAllowed consent. All capabilities remain disabled by default. Active mode is rejected, and model output does not change routing, context delivery, QA, or verified success.
- Keys are read from process environment or a local .env.local file; CATHULU_ENV_FILE may name an explicit local file. Only the two documented Cathulu key names are read. Credentials and raw provider bodies are not logged or persisted.
- The [TypeSafe OpenAPI](https://api.typesafe.ai/openapi.json) and [data-handling FAQ](https://help.typesafe.ai/articles/5974925716-how-is-submitted-data-used-or-retained), and the [ScaleDown Extract API](https://docs.scaledown.ai/api-reference/extract) and [quickstart](https://docs.scaledown.ai/quickstart), were reviewed for request/response shape and egress behavior. The reviewed ScaleDown pages do not state a request-retention period; no retention claim is made.
- Local loopback HTTP tests use fake keys and do not contact either provider. They validate protocol mapping and safety boundaries, not provider quality or workflow benefit. M18 comparative scoring remains ineligible without representative Cathulu baseline runs; any future active trial needs a pre-registered comparison and separate explicit approval.

Reqwest is kept to JSON, Rustls TLS, and system-proxy support with default features disabled; HTTP/2 is not needed for these provider endpoints. Context7’s documentation lookup had reached its monthly quota, so the current official provider API pages above were used directly.
## Evidence

- User's specialist-model architecture request in the current task.
- Existing source-of-truth constraints in [ADR-0001](ADR-0001-cathulu-is-the-local-evidence-layer.md), [ADR-0002](ADR-0002-cathulu-hosted-agent-selectors.md), [ADR-0003](ADR-0003-managed-local-memory-lifecycle.md), [ADR-0005](ADR-0005-codex-worker-telemetry-and-worktree-verification.md), and [ADR-0006](ADR-0006-local-graphify-snapshot-contract.md).
- Existing measurement authorities described in [architecture](../architecture.md), [roadmap](../roadmap.md), and [PLAN-003](../plans/PLAN-003-evidence-gated-scaling.md).

This ADR records the accepted architecture boundary, the original local-only M16/M17 implementations, and the M18a optional provider adapters. M15-M18a implementation and evidence are recorded in [PLAN-004](../plans/PLAN-004-optional-specialist-model-optimization.md). No live model call, active workflow treatment, or measured optimization result is claimed.

## Reconsider if

No specialist capability produces measurable benefit for a workflow; comparable runs show worse quality, regressions, rework, human intervention, latency, privacy, or maintenance; no safe provider/credential path exists; or deterministic/local processing provides equivalent results with lower cost and risk. Keep that capability disabled in those cases.

## Affected canonical documents

- [Project definition](../project-definition.md)
- [Architecture](../architecture.md)
- [Roadmap](../roadmap.md)
- [PLAN-004](../plans/PLAN-004-optional-specialist-model-optimization.md)
- [Documentation map](../README.md)

