# ADR-0006: Local Graphify snapshot contract

Date: 2026-10-21
Status: ACCEPTED
Scope: Optional code-structure retrieval and Graphify-based memory freshness
Supersedes / superseded by: None

## Context

M11's pre-registered local pilot found that exact Graphify paths were useful for three structurally linked code tasks and fit within half of the corresponding `rg` output. Cathulu already has SQLite/FTS retrieval, provenance, and per-memory freshness bases. It must not duplicate those authorities or turn a useful graph export into an unbounded runtime dependency.

The pilot used the existing Graphify 0.9.82 installation, schema version 1, code-only extraction, no LLM backend, and disabled query logging. The graph header's source commit matched the code corpus. The installed distribution carries Apache-2.0 and MIT license texts; Cathulu will not vendor or bundle the Graphify implementation.

## Decision

- Graphify is an optional, local retrieval source. Cathulu does not install or invoke Graphify automatically and does not require it at build or runtime.
- Read only the conventional project-local `graphify-out/graph.json` export. Accept only the schema and Graphify version exercised by fixtures and the scored pilot; initially this is schema 1 and Graphify 0.9.82. Unsupported or malformed exports fail closed.
- Accept a snapshot only when it identifies the registered repository and current Git commit. A deterministic snapshot ID includes the registered project identity, canonical repository path, source commit, and content hash. SQLite stores the current snapshot metadata and change events, not a second graph or code/memory authority.
- Structural retrieval returns bounded node/edge candidates with relative source paths, locations, relation, and confidence. Candidate files must remain inside the registered repository, exist in the current checkout, and not be changed or removed relative to the indexed source. Graphify candidates are retrieval hints; exact project/run/memory state and SQL/FTS remain authoritative.
- A new validated snapshot invalidates only memories whose existing freshness basis explicitly names the same project and a different graph snapshot. Historical memory, provenance, and source evidence remain stored. Missing, stale, or unsupported snapshots are unavailable and fall back to existing file/Git and SQL/FTS retrieval.
- Graphify output is not automatically embedded in prompts, copied into durable memory, or semantically indexed. The local export remains user-managed and may be regenerated with code-only extraction.

## Alternatives considered

- **Add Graphify as a required Rust/Python dependency or invoke it during Cathulu startup:** rejected because the measured benefit applies to selected structural tasks and does not justify a required toolchain or startup work.
- **Persist the full graph in SQLite:** rejected because Graphify remains replaceable retrieval infrastructure; the exported graph already exists as a local artifact, and SQLite needs only freshness metadata.
- **Treat the graph as authority for memory or current state:** rejected because this would duplicate SQLite and could let stale structural results override exact state.
- **Use broad natural-language Graphify search as automatic context:** rejected because pilot queries were noisy and sometimes truncated; bounded exact-anchor traversal is easier to validate and budget.

## Consequences and validation

- M11 adds an additive SQLite snapshot-metadata migration and a bounded local MCP retrieval operation. Reading or failing to read a graph never modifies project source or Graphify output; SQLite records validated snapshot identity and explicit freshness invalidations.
- Current Git commit, snapshot schema/version, repository containment, changed/deleted source paths, and source locations are checked before a candidate is returned.
- If validation fails, the graph contributes no result. Existing SQL/FTS and exact source inspection remain available.
- The measured pilot is recorded in [PLAN-003](../plans/PLAN-003-evidence-gated-scaling.md). It is evidence for this selected adapter, not a general claim that Graphify is better for all tasks.

## Sources

- Local Graphify 0.9.82 snapshot and license files at the time of the pilot.
- [Graphify upstream repository](https://github.com/Graphify-Labs/graphify), including its source and license files.
- Pre-registered M11 benchmark and source-verified output in [PLAN-003](../plans/PLAN-003-evidence-gated-scaling.md).
- Existing memory authority and freshness rules in [ADR-0003](ADR-0003-managed-local-memory-lifecycle.md).

## Reconsider if

Graphify changes its export contract, source freshness cannot be verified at acceptable cost, real use fails to reuse structural retrieval, or the local-only privacy and maintenance assumptions no longer hold.

## Affected canonical documents

- [Architecture](../architecture.md)
- [Roadmap](../roadmap.md)
- [Local MCP setup](../local-mcp.md)
- [PLAN-003](../plans/PLAN-003-evidence-gated-scaling.md)
