# ADR-0003: Managed local memory lifecycle

Date: 2026-10-21
Status: ACCEPTED
Scope: Memory classification, retrieval, provenance, context activation, and retention
Supersedes / superseded by: None

## Context

Cathulu's purpose includes durable, selective memory around Codex. Keeping every event as active memory would inflate context; deleting old information aggressively would lose unique knowledge and the evidence behind important decisions. The project already has SQLite as its system of record and is adding project, workflow, run, event, and quality domains in Phase 1. A memory design must extend those records without duplicating them or introducing a separately operated memory service.

The owner requires memory to remain local, provenance-preserving, temporally aware, auditable, increasingly useful through experience, and bounded in active context. Raw evidence is not automatically permanent agent memory. Unique provenance must remain traceable, while redundant representation can be compacted after useful knowledge is preserved.

## Decision

Cathulu uses a managed lifecycle: experience → evidence → extraction → candidate → consolidation → durable knowledge. Candidate/review state is independent of memory class and retrieval temperature. Promotion is based on deterministic evidence and policy; optional model assistance may suggest classification or summaries but cannot authoritatively overwrite knowledge or delete evidence.

Memory class and temperature are separate dimensions. Classes are `working`, `canonical`, `episodic`, `evidence`, and `aggregate`; temperatures are `hot`, `warm`, and `cold`. HOT context is bounded per logical scope, WARM knowledge is selectively retrieved, and COLD evidence is excluded from automatic injection. Temperature changes update the same stable identity.

SQLite remains authoritative. Existing project, objective, task, decision, workflow, run, check, event, and artifact records remain their own source of truth. A distinct distilled memory claim may reference these records and their evidence but cannot mirror their mutable state. Canonical memory has a stable ID, content hash, provenance, temporal validity, freshness basis where applicable, and explicit supersession. Historical claims remain queryable; current retrieval filters stale or superseded items.

Cathulu tracks receiver known-by state by scope and content hash, and records every controlled context transfer. Unchanged valid context is not resent in full; changed context is represented as a delta/reference. Transfers retain metadata and hashes, not raw prompts solely for measurement. Scope activation resumes from a compact snapshot plus relevant Git/contract/dependency/Graphify deltas, not transcript replay.

The memory foundation is implemented before automatic lifecycle maintenance. A later local deterministic Memory Gardener may consolidate duplicates, promote/demote knowledge, compact evidence, and collect eligible artifacts under explicit reference-aware policy. Protected evidence and sources required by memory, decisions, QA, or investigations are not automatically removed. Retention has no arbitrary fixed age cutoff. Semantic retrieval is optional and justified by measured gaps; any index returns candidate IDs that resolve to SQLite authority.

## Alternatives considered

- **Store every run/event as permanent active memory:** rejected because active context and storage would grow with raw history, and evidence would be confused with distilled knowledge.
- **Delete stale or old records aggressively:** rejected because unique provenance and historical truth can explain current architecture and failures.
- **Make one generic memory table the authority for all domain state:** rejected because it duplicates existing/future domain models and weakens transaction, audit, and quality boundaries.
- **Add a vector database or continuously running LLM gardener now:** rejected because M1/M2 have no measured retrieval gap or operational need; this adds dependencies and autonomous behavior before evidence exists.
- **Replay prior transcripts to resume a scope:** rejected because it repeats large, stale, and potentially sensitive context instead of applying state and deltas.

## Consequences and staged implementation

- M2 adds stable identities, class and temperature, hashes, provenance, temporal validity/supersession, freshness markers, known-by tracking, transfer records, artifact references, protected evidence metadata, and bounded HOT-scope budgets alongside workflow/quality state.
- M2 tests the memory invariants, not automatic learning: class/temperature independence, stable identity and idempotence, historical/current retrieval, selective invalidation, transfer deltas, and budget demotion without knowledge loss.
- M3 populates the existing transfer/evidence model from supported sanitized telemetry; unavailable telemetry does not block semantic operations.
- A later milestone adds selective FTS retrieval and deterministic gardening, consolidation, retention/GC, actionable memory-health metrics, storage-growth accounting, and versioned policy experiments.
- A semantic index or Graphify integration is added only when evidence shows it improves retrieval. SQLite remains authoritative and local.
- The read-only dashboard may show compact actionable memory health but does not become a memory mutation surface.

## M2 implementation mapping

- An additive SQLite v2 migration extends the existing database. Projects, objectives, tasks, decisions, workflow versions, runs, checks, events, and artifact references remain their own authorities; memory rows hold distinct claims and references.
- Logical scopes use typed references for global/project/objective/task records plus opaque stable scope IDs for future overseer scopes; no parallel scope registry is introduced.
- The memory row stores stable ID, normalized content hash, class, temperature, review state, importance/confidence, reuse data, validity interval, supersession links, and a structured freshness basis. Class and temperature can change independently without replacing the ID.
- Canonical memories require a source reference. Artifact references and protection metadata are local; M2 does not copy or delete artifact contents. Event/projection changes are transactional, and event, decision, and workflow-version history is immutable through the exposed API.
- Known-by records key a receiver scope to a content hash. Context-transfer rows store size/token estimates, retrieval method, and reuse metadata, while explicit transfer calls return content only for a requested full or delta transfer.
- M2 currently produces Git commit freshness and selectively invalidates affected project memories. Graphify, contract, and dependency freshness inputs can be represented but do not become active invalidation sources until those snapshots exist.
- M2 validates the deterministic substrate only. Promotion, consolidation, Gardener maintenance, semantic indexing, storage-growth experiments, and reference-aware garbage collection remain deferred.

## Reconsider if

Measured retrieval shows that the metadata/provenance model is too costly or cannot preserve useful knowledge; supported Codex context-transfer signals make the transfer ledger inaccurate; SQLite cannot meet measured local scale; or safe reference-aware retention proves impractical. Any change must preserve local authority, temporal history, provenance, and bounded active context.

## Affected canonical documents

- [Project definition](../project-definition.md)
- [Architecture](../architecture.md)
- [Roadmap](../roadmap.md)
- [Phase 1 implementation plan](../plans/PLAN-001-phase1-first-slice.md)
