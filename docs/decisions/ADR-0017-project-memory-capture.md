# ADR-0017: Required project-memory capture review

Date: 2026-10-22
Status: ACCEPTED
Scope: Project-memory onboarding and run closeout
Supersedes / superseded by: Extends [ADR-0003](ADR-0003-managed-local-memory-lifecycle.md)

## Context

Cathulu already stores scoped memory with provenance, freshness, temporal validity, supersession, and selective retrieval. However, records are only created when an agent explicitly calls `create_memory`, and the operating contract only says to suggest durable knowledge. A project can be analyzed, worked on, and reported while its memory scope remains empty. The dashboard's zero count is therefore a capture-workflow gap, not evidence that the project has no useful knowledge.

Cathulu memory is a curated, durable operational knowledge system that helps future work resume with the right project context. It stores distilled, reusable knowledge—not chat transcripts, private reasoning, raw tool histories, or every execution event. Repositories, Git, tests, Graphify, decisions, and QA artifacts remain authoritative for the facts they own.

## Decision

Project Managers must establish a compact, source-backed canonical project summary during the first analyzed project run. At each project-run closeout, they must either record the accepted current memories that preserve durable knowledge or explicitly record that no durable update was warranted. A project with no current canonical summary cannot use the no-update disposition.

Cathulu records the review as a compact append-only event attached to the run. Run finalization requires that event. The review includes only a disposition, a fixed reason code, and memory IDs; it has no free-text rationale, and the knowledge body lives only in the existing memory table with provenance. The operation validates that referenced records are accepted, current, source-backed, and scoped to the exact project. Exact retries are idempotent; a conflicting second review is rejected.

Source-based onboarding may use project documentation, code structure, manifests, current Git facts, tests, accepted decisions, and verified reports/artifacts. It must not import prior chat transcripts or private model reasoning. Agent reports are claims until validated; durable claims must cite their actual sources.

## Alternatives considered

- **Copy past chats and transcripts into memory:** rejected because it duplicates execution history, expands context, and violates the memory boundary.
- **Rely only on optional agent judgment:** rejected because the existing workflow silently completed while leaving every project scope empty.
- **Automatically promote every run report:** rejected because reports may be unverified, transient, or duplicative.
- **Add a second project-profile database/table:** rejected because the existing scoped canonical memory model already represents this knowledge.

## Consequences

- First project work must establish a reviewed canonical `summary` memory with provenance.
- Every project run receives a memory-review receipt before finalization; a valid no-update receipt is allowed only after a current project summary exists.
- The event ledger provides auditability without storing memory bodies or copying transcripts.
- Jev (DecisionModel) may later classify bounded memory candidates, and the Context Model/CDM (including ScaleDown) may draft source-span extractions. They remain optional suggestions under existing shadow/consent controls; deterministic source review and existing Cathulu records remain authoritative.
- The existing memory dashboard becomes meaningful as projects are onboarded and records are reused.
- Existing project scopes require source-based onboarding; initial profiles for the five currently registered projects were added on 2026-10-22. Their Git freshness markers remain unset where source files are dirty or the registered HEAD is unknown. No transcript backfill is permitted.
- See [PLAN-016](../plans/PLAN-016-project-context-capture.md) for implementation and prior-project onboarding.

## Reconsider if

The closeout receipt materially blocks valid runs without improving memory coverage, the event ledger cannot provide reliable idempotency, or a future workflow replaces explicit run finalization with a verified transactional closeout.
