# PLAN-016 — Project context capture and memory closeout

Status: M37 implementation and M38 initial onboarding complete; matched current pair published; active MCP reload pending
Created: 2026-10-22
Last updated: 2026-10-22

## Objective

Make Cathulu memory a working aid for project context by ensuring each analyzed project has a concise, source-backed canonical profile and every project run records whether durable knowledge was captured or reviewed with no update.

## Motivation

The memory schema, scoped retrieval, provenance, and candidate review already existed, but knowledge capture was only suggested to agents. Project registration, Git reconciliation, reports, and run completion did not require a project profile or a memory review, allowing useful work to accumulate while scopes remained empty. Memory must be curated from project sources and verified evidence; chat transcripts, private reasoning, and raw tool histories are excluded.

## Current state

- SQLite is authoritative for memory records, class/temperature, provenance, validity, supersession, and retrieval.
- `create_memory` writes a candidate, and `review_memory_candidate` accepts or rejects it.
- The agent operating contract only asks the Project Manager to review proposed durable knowledge; run completion has no memory capture check.
- The local dashboard reports zero stored records when no explicit `create_memory` calls were made.

## Desired state

- First project work establishes a compact accepted canonical `summary` memory from inspected repository documentation and verified project facts.
- Later runs either link accepted, current project memories that were captured/reinforced, or explicitly record that no durable knowledge changed.
- Cathulu refuses to mark a run complete until its memory review is recorded. A project with no current canonical summary cannot report `no_durable_change`.
- Review receipts retain only a disposition, fixed reason code, and memory IDs. They do not copy free-text rationale, transcripts, private reasoning, prompts, or raw tool output.
- All canonical memories remain source-linked and scoped to their project. Repository, Git, tests, decisions, and QA remain authoritative for facts they own.

## Scope

- Add an append-only project-memory review event using the existing SQLite event ledger; avoid a duplicate memory table or schema migration.
- Add an MCP operation to record `knowledge_saved` or `no_durable_change` for an open project run.
- Validate that referenced memories are accepted, current, provenance-bearing, and in the exact run's project scope.
- Gate run finalization on a recorded memory review and require a current accepted canonical project summary before accepting `no_durable_change`.
- Make project onboarding and run-closeout capture mandatory in the agent operating contract and MCP tool descriptions.
- Update architecture, ADR, roadmap, and this plan.

## Out of scope

- Copying or summarizing chat transcripts, private reasoning, or raw tool histories.
- Automatically generating memories from every event or worker report without project-role review.
- Bulk backfilling every registered repository without inspecting its actual source-of-truth files.
- Semantic/vector indexing, automatic evidence deletion, or LLM-driven Memory Gardener behavior.
- Dashboard redesign or changing the existing read-only memory-health surface.

## Source-of-truth impact

- `src/memory.rs`: validated memory-review receipt operation.
- `src/workflow.rs`: finalization gate.
- `src/mcp.rs`: public MCP tool and updated memory tool guidance.
- `docs/agent-operating-contract.md`: mandatory onboarding and closeout behavior.
- `docs/architecture.md`, `docs/decisions/ADR-0017-project-memory-capture.md`, and `docs/roadmap.md`: intended product behavior.
- `docs/plans/PLAN-016-project-context-capture.md`: milestone status and evidence.

## Existing decisions and constraints

- [ADR-0003](../decisions/ADR-0003-managed-local-memory-lifecycle.md) remains authoritative for the memory lifecycle.
- Memory class and temperature are independent; HOT is bounded, WARM is selective, COLD is not injected automatically.
- Canonical knowledge requires provenance and is never silently overwritten.
- SQLite remains authoritative. Summaries are projections, not source-of-truth facts.
- No transcript or private-reasoning capture is permitted.
- Jev remains an optional DecisionModel for bounded candidate classification; the Context Model/CDM (including the optional ScaleDown provider) may draft extraction candidates. Neither is required for capture, authoritative for facts, or allowed to accept/delete memory. No specialist call is made by M37; any later provider use follows the existing per-run remote-consent and shadow-mode controls.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`, `ponytail-balanced`, and `graphify`.
- Existing SQLite event, memory, MCP, migration, and workflow layers.

## Dependencies

- Existing schema-12 `memories`, `memory_sources`, `events`, `runs`, and project-scope identities.
- Current canonical project memory APIs and run finalization flow.

## Risks and unknowns

- The gate adds one explicit closeout call to project runs. Older MCP clients must reload the built MCP before they can record it.
- A model could write an inaccurate summary; the PM must verify every claim against source files or deterministic evidence before accepting it.
- Git HEAD changes do not necessarily make a project profile stale. Only attach a freshness basis where the summarized fact actually depends on that snapshot.
- This milestone prevents future silent omissions; existing repositories need source-based onboarding before their scopes become useful.

## Milestones

### M37 — Required project-memory onboarding and run review

Goal: prevent a project run from ending with no project-context capture review, while keeping all memory content distilled and evidence-backed.

Subtasks:

- [x] Add a run-scoped `record_project_memory_review` API and MCP tool using the existing append-only event table.
- [x] Validate exact project scope, accepted/current memory IDs, provenance, one review per run, and first-project-summary requirements.
- [x] Require the review receipt before `finish_run` can report a terminal result.
- [x] Strengthen role and tool guidance for source-based onboarding, memory updates, and transcript exclusion.
- [x] Add focused lifecycle and MCP contract coverage.

Affected areas:

- `src/memory.rs`, `src/workflow.rs`, `src/mcp.rs`, `tests/memory_lifecycle.rs`, `tests/mcp_vertical_slice.rs`, relevant run fixtures.
- `docs/agent-operating-contract.md`, architecture, ADR, roadmap, and this plan.

Dependencies: Existing memory and event schema; none of M36's Codex dispatcher work.

Acceptance criteria:

- [x] A project run with no memory review cannot be finalized.
- [x] An empty project scope cannot record `no_durable_change`; it must first accept a provenance-backed canonical `summary`.
- [x] Saved memory references must be accepted, current, source-backed, and scoped to that run's project.
- [x] Exact duplicate review retries are idempotent, including after closeout; conflicting second reviews are rejected.
- [x] The event stores disposition, a fixed reason code, and IDs only; it does not store free text, memory bodies, prompts, transcripts, or tool output.
- [x] No database migration or parallel memory authority is introduced.

Validation:

- [x] Focused memory lifecycle tests and MCP JSON-RPC contract test; the full Rust workspace suite passes (one existing native integration test remains ignored by its explicit script).
- [x] Rust formatting check and full Rust workspace tests.
- [x] Build the matched current Windows desktop/MCP pair; do not launch or restart either process during validation.
- [x] `git diff --check`.

Documentation updates:

- [x] Update ADR-0003 consequences, architecture, role contract, roadmap, and this plan with implemented behavior and validation evidence.

Applicable specialized skills: `milestone-delivery`, `ponytail-balanced`, `graphify`.

Expected Git checkpoint: one coherent M37 change set. Git metadata is read-only in the current environment, so local commit/push may be unavailable.

### M38 — Source-based onboarding for previously worked projects

Status: Complete for the five registered project scopes; initial records are local SQLite data.

Goal: populate useful project context for existing registered projects using only their repositories, documented decisions, and verified Cathulu evidence.

Subtasks:

- [x] Onboard each project from its own README/project definition, architecture docs, manifests, registered Git snapshot, and accepted decisions where applicable.
- [x] Treat existing Cathulu reports only as evidence leads; no agent report or transcript was promoted into project memory.
- [x] Create compact canonical profiles and project-specific durable constraints with traceable provenance.
- [x] Read back and verify scoped memories. Historical deduplication and ongoing source-refresh maintenance remain future work.

Acceptance criteria:

- [x] Each registered project has at least one accepted HOT canonical summary based on the source contents reviewed during onboarding, with local source references.
- [x] No memory is based solely on a chat transcript or unverified model report.
- [x] MCP readback confirms project scopes, accepted state, class, temperature, and provenance are available without replaying prior chat context.

Validation:

- [x] Review each profile against its cited source files and registered project state.
- [x] Read it back via MCP and verify scope, provenance, class, and temperature. Git freshness basis is deferred because cited files are modified/uncommitted in four repositories and MARX has no registered HEAD; do not infer a clean source snapshot.

Documentation updates: Record onboarding coverage and any project-specific unknowns; never infer missing project facts.

Expected Git checkpoint: source-backed project context entries in the local SQLite store; these are user data, not repository commits.

## Final integration validation

M37 is complete only after the gate, memory review workflow, full Rust validation, matched Windows build, and documentation are all evidenced. M38 covers the initial source-based onboarding; future source refreshes must attach deterministic Git/Graphify freshness when the source snapshot is trustworthy.

## Rollback / recovery

The review receipt is append-only. A mistaken receipt cannot be overwritten; resolve it with a separate correction event only if required. The finalization gate can be reverted in source without deleting memory records, but accepting no-provenance canonical memory is never an acceptable rollback.

## Progress

- 2026-10-22: Plan created after confirming existing memory writes are manual and run closeout does not require a capture review.
- 2026-10-22: M38 seeded seven accepted HOT canonical memories across the global scope and five registered project scopes: Cathulu (ID 1), artificial_inc (ID 2), PULSO (ID 3), mecanografica-proposals (IDs 4–5, including the owner-directed RUC-history constraint), MARX (ID 6), and the global project-independence constraint (ID 7). Each record was read back through local MCP; all are active and source-linked. No transcript or remote model provider was used. Git freshness basis remains unset because cited files are modified/uncommitted in four repositories and MARX has no registered HEAD.
- 2026-10-22: M37 implementation, full Rust workspace validation, and the matched Windows build are complete. The latest source fingerprint is `559903471A6F790382FD64C62BA6928D009873B9D005F345268249A41EEFEED9`; the active MCP process has not been restarted.

## Implementation discoveries / decisions

- Existing `events` can store the compact run review receipt, avoiding a schema migration.
- The initial empty-scope case is handled by requiring a canonical project summary before a `no_durable_change` review is accepted.

## Completion evidence

- M37 source changes compile and the full Rust workspace suite passes: 19 unit tests, 8 extraction tests, 13 delegation tests, 17 MCP vertical-slice tests, 4 memory lifecycle tests, 5 M15 tests, 6 M16 tests, and 3 desktop unit tests passed. One existing Tauri integration test is intentionally ignored and runs only through its isolated script.
- All seven project/global memories were accepted and read back through the live local MCP with the expected scope, canonical class, HOT temperature, active status, and provenance counts.
- `cargo fmt --all -- --check` and `git diff --check` passed. `cargo test --locked --offline --workspace` passed 75 tests; one existing Tauri integration test is intentionally ignored and runs only through `scripts/phase1-m4-integration-smoke.ps1`.
- `scripts/build-current.ps1` published matched app/MCP fingerprint `559903471A6F790382FD64C62BA6928D009873B9D005F345268249A41EEFEED9` at `target/mcp-versions/559903471A6F790382FD64C6`, behind `target/mcp-current`. MCP SHA-256: `0D75BAE183CAF2C6DA0086971F5F47F6181A3366C0C8DDF67E4347430C7FA27A`; desktop SHA-256: `8C956CDFFBAF9571916CBE4735037DF9E3503FFC79698F4F7DF75E1D5C94D489`; desktop `custom-protocol` enabled. The MCP/dashboard were not launched, focused, stopped, or restarted. The currently connected MCP retains its loaded version until a controlled Cathulu-only restart.
