# Cathulu project documentation

This map defines where project intent, architecture, and implementation evidence live. The attached kickoff brief supplied the initial product requirements; this repository's documents are the current canonical records.

| Concern | Authority | Status |
| --- | --- | --- |
| Purpose, actors, scope, invariants, MVP, and success criteria | [Project definition](project-definition.md) | NORMATIVE |
| System boundaries, drivers, managed memory lifecycle, local data/security, and skill/tool profile | [Architecture](architecture.md) | WORKING for implementation details; NORMATIVE for stated product boundaries |
| Phase order and deferred capabilities | [Roadmap](roadmap.md) | NORMATIVE |
| Durable decisions and supersession | [Decision records](decisions/) | NORMATIVE when ACCEPTED |
| Optional specialist-model optimization | [ADR-0007](decisions/ADR-0007-optional-specialist-model-optimization.md), [PLAN-004](plans/PLAN-004-optional-specialist-model-optimization.md) | M15-M18a merged; public MCP missing-key fallback covered; live trials and comparative scoring pending |
| Definition readiness and dependent scope | [Definition gate](readiness/definition-gate.md) | EVIDENCE |
| Resumable interview, claims, and unresolved questions | [Kickoff state](kickoff-state.json) | WORKING checkpoint |

Implementation plans belong in `docs/plans/`. Verification evidence belongs with its plan and, when relevant, in the source-of-truth document it validates. A plan proposes work; it does not change project intent by itself.

