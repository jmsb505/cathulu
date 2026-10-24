# ADR-0022 — Mixed-provider personal project operating system

- Status: Accepted product direction from the Owner's 2026-10-25 request to use the supplied pivot plan. Detailed migration contracts remain proposed until validated in their milestones.
- Scope: Cathulu product identity, provider neutrality and incremental migration order. This decision implements no new runtime behavior.
- Partially supersedes: ADR-0001's Codex-only product identity and ADR-0007's assumption that Codex must remain the sole primary reasoning system. Their local authority, evidence, optional-specialist and consent protections remain in force.
- Preserves: ADR-0021 supervised development, ADR-0020 headless startup, ADR-0019 desktop pause, ADR-0009/0011 hierarchy and identity, current role caps, review gates and access/production permissions.
- Design: [Migration assessment](../pivot-migration.md); delivery: [PLAN-020](../plans/PLAN-020-mixed-provider-personal-project-os.md).

## Context

The Owner supplied `Cathulu_Pivot_Mixed_Provider_Personal_Project_OS.md` and asked to use it to pivot. Its immediate deliverable is a repository-grounded migration assessment and staged plan. Current Rust/SQLite/MCP source already owns useful workflow, role, memory and evidence state, but execution and the original product definition are centered on Codex. A rewrite would discard established identities, constraints and retained evidence.

## Decision

Cathulu becomes the persistent local personal project operating system: canonical project/task/priority/decision/memory/policy/evidence state belongs to Cathulu, and Claude Code/Codex are replaceable cognition/execution providers. Core project-management queries and explicit administrative commands must work without either provider. Git/code remains authoritative for repository contents; objective verification and required human decisions remain authoritative for acceptance.

Preserve and extend SQLite, FTS5, memory lifecycle, stable logical role keys, event queues, work plans, reviewed reports and QA. Begin with provider-free tasks, priorities, dependencies and checkpoint/resume. Add provider-neutral session/artifact boundaries and verified workspace ownership before mixed-provider execution. Use Claude for ambiguity/product judgment and Codex for bounded implementation/verification as configurable initial priors; no comparative quality claim or autonomous routing promotion follows.

No required model API billing, hosted backend, vector database, new swarm or transcript ingestion is introduced. Subscription-compatible safe harness behavior, tool availability, authentication and capacity require actual evidence. New provider use retains explicit data-disclosure and access controls; the attachment does not authorize credentials, installs, spending, other-project tasks or publication.

The current role hierarchy/caps remain enforceable during migration. Broader specialist/dependency roles are deferred to an actual scoped need. Desktop implementation stays paused; its future home is planned around work and project resume while current observation surfaces remain available. This decision does not resolve M42/M44 runtime acceptance or waive their gates.

## Consequences and verification

Current docs distinguish accepted target intent from the legacy implementation and historical evidence. The migration assessment owns proposed contracts and specific source-backed gaps; PLAN-020 owns milestone acceptance. Provider sessions can disappear without deleting Cathulu identity/state, but new sessions must receive selected context even when logical known-by history exists. Independent writers require exclusive leased workspaces; the existing root-based dispatch path must not be claimed as satisfying that target.

Validate the first slice with both providers absent, a populated schema upgrade, task dependency/permission cases and checkpoint/restart/resume. Validate adapters, permissions, workspaces, actual-provider acceptance and comparative benefit separately. No such executable pivot validation is claimed by this documentation decision.

Recovery is incremental: preserve existing IDs/readers/history; apply reversible local patches and copy-tested migrations with consistent backups/matched binaries. Reconsider specific contracts if installed subscription tooling cannot meet safety boundaries or observed workflow evidence favors a smaller solution. The product direction does not justify bypassing those boundaries.
