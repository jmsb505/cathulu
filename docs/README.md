# Cathulu project documentation

This map defines where project intent, architecture, and implementation evidence live. The attached kickoff brief supplied the initial product requirements; this repository's documents are the current canonical records.

The Owner adopted the mixed-provider personal project OS direction on 2026-10-25. [ADR-0022](decisions/ADR-0022-mixed-provider-personal-project-os.md) scopes supersession of the original Codex-only identity. Current implementation, proposed design, runtime acceptance and comparative benefit remain separate evidence states.

| Concern | Authority | Status |
| --- | --- | --- |
| Purpose, actors, scope, invariants, MVP, and success criteria | [Project definition](project-definition.md) | NORMATIVE |
| System boundaries, drivers, managed memory lifecycle, local data/security, and skill/tool profile | [Architecture](architecture.md) | WORKING for implementation details; NORMATIVE for stated product boundaries |
| Pivot current-state assessment, target contracts, migration risks and current development profile | [Migration assessment](pivot-migration.md) | WORKING design; source observations scoped to the inspected dirty tree |
| Pivot implementation sequence and acceptance evidence | [PLAN-020](plans/PLAN-020-mixed-provider-personal-project-os.md) | M48 locally implemented and validated; Claude review and integration pending; M49-M55 planned |
| M48 task/portfolio implementation evidence | [M48 evidence](readiness/m48-task-portfolio-2026-10-25.md) | Local schema, workflow, MCP and regression evidence; no runtime/release claim |
| Manual Claude Code / Codex assignments in the same directory | [PLAN-021](plans/PLAN-021-manual-claude-codex-development.md), [ADR-0023](decisions/ADR-0023-manual-shared-directory-development.md) | CX-00 baseline recorded; CX-01 implemented/validated on the shared branch; stable Claude review and commit pending |
| Pending-work synchronization and integration validation | [2026-10-25 repository integration](readiness/repository-integration-2026-10-25.md) | Regression/build evidence and PR delivery record; live gates remain separate |
| Phase order and deferred capabilities | [Roadmap](roadmap.md) | NORMATIVE |
| Durable decisions and supersession | [Decision records](decisions/) | NORMATIVE when ACCEPTED |
| Optional specialist-model optimization | [ADR-0007](decisions/ADR-0007-optional-specialist-model-optimization.md), [PLAN-004](plans/PLAN-004-optional-specialist-model-optimization.md) | M15-M18a merged; public MCP missing-key fallback covered; live trials and comparative scoring pending |
| Definition readiness and dependent scope | [Definition gate](readiness/definition-gate.md) | EVIDENCE |
| Resumable interview, claims, and unresolved questions | [Kickoff state](kickoff-state.json) | WORKING checkpoint |

Implementation plans belong in `docs/plans/`. Verification evidence belongs with its plan and, when relevant, in the source-of-truth document it validates. A plan proposes work; it does not change project intent by itself.

