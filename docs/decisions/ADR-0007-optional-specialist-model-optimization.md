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
- A remote adapter cannot be implemented until current primary API/terms/privacy/availability and secure local credential handling have been reviewed.
- Jev, ScaleDown, and GLiNER-family models remain evaluation candidates, not required providers or assumed supported APIs.
- Any active pilot remains per-workflow and reversible. Human approval is required before active execution; no policy adaptation happens automatically.
- Existing memory authority, provenance, temperature classes, Gardener constraints, and evidence-retention decisions remain unchanged. Candidate memory reranking occurs only after deterministic retrieval and never grants deletion authority.

## Evidence

- User's specialist-model architecture request in the current task.
- Existing source-of-truth constraints in [ADR-0001](ADR-0001-cathulu-is-the-local-evidence-layer.md), [ADR-0002](ADR-0002-cathulu-hosted-agent-selectors.md), [ADR-0003](ADR-0003-managed-local-memory-lifecycle.md), [ADR-0005](ADR-0005-codex-worker-telemetry-and-worktree-verification.md), and [ADR-0006](ADR-0006-local-graphify-snapshot-contract.md).
- Existing measurement authorities described in [architecture](../architecture.md), [roadmap](../roadmap.md), and [PLAN-003](../plans/PLAN-003-evidence-gated-scaling.md).

This ADR accepts architecture boundaries only. It does not claim a provider/API validation, implementation, active workflow treatment, or measured optimization result.

## Reconsider if

No specialist capability produces measurable benefit for a workflow; comparable runs show worse quality, regressions, rework, human intervention, latency, privacy, or maintenance; no safe provider/credential path exists; or deterministic/local processing provides equivalent results with lower cost and risk. Keep that capability disabled in those cases.

## Affected canonical documents

- [Project definition](../project-definition.md)
- [Architecture](../architecture.md)
- [Roadmap](../roadmap.md)
- [PLAN-004](../plans/PLAN-004-optional-specialist-model-optimization.md)
- [Documentation map](../README.md)

