# ADR-0024 — Product workspace leases and uncertain execution recovery

- Status: Accepted implementation baseline for CX-05/M52; runtime activation remains gated.
- Scope: Durable workspace assignment, role/session/attempt binding, credential-source eligibility and recovery for Cathulu-managed product execution.
- Depends on: [PLAN-020](../plans/PLAN-020-mixed-provider-personal-project-os.md) M52 and [PLAN-021](../plans/PLAN-021-manual-claude-codex-development.md) CX-05.

## Context

PLAN-020 M52 requires product writers to use exclusive workspaces, coordinators to stay read-only, and uncertain provider submission to block duplicate work until reconciled. The repository already records provider sessions and attempts, but it does not allocate exclusive workspaces or bind those records to an assigned lease. Product dispatch still starts the existing Codex app-server path directly.

The Owner's same-directory Claude Code/Codex development arrangement is a separate, manual exception under ADR-0023. It is not a product workspace lease and does not establish provider isolation, authentication or runtime safety.

## Decision

1. Schema v22 records immutable lease identity and append-only lease events. A writer is a Worker with write access to an exact linked worktree of the registered repository, sharing its Git common directory. The worktree must be clean when the lease is acquired. Project Manager/coordinator access is read-only; writers cannot use the registered project root.
2. With no semantic-overlap admission model, permit at most one active or quarantined writer lease per project. This is conservative project-wide serialization; it does not claim that disjoint tasks conflict semantically.
3. Permit one active provider session for each logical `binding_key` across all providers. On v21 upgrade, conflicting active session rows are retained but marked `unknown`, so the system does not silently choose a winner. A new session can replace the role's session only after every other attempt for that role is terminal; replacement inactivates the old session and revokes capabilities tied to it.
4. Bind a provider attempt and role capability to the same exact lease. Provider execution is eligible only when the adapter reports a known credential source and the permission manifest explicitly allows it. Unknown or unapproved credential sources block.
5. On expiry, release or revocation with unresolved attempts, mark the attempt `needs_reconcile`, quarantine the lease and revoke the capability. Retain its role/path locks until explicit terminal reconciliation. Do not automatically retry the provider call or delete, move or clean a worktree.
6. Keep these APIs non-activating until the dispatcher acquires a lease and validates credentials, workspace identity and permissions before every provider start. Do not migrate the live store or alter a running process as part of CX-05.

## Consequences

- A dirty Owner worktree is never cleaned to satisfy a lease. A lost acknowledgement leaves the workspace unavailable until the attempt is reconciled.
- A v21 role with sessions active at both providers becomes temporarily unavailable (`unknown`) until one provider session is explicitly recorded.
- M52 is not complete until dispatch enforces leases, semantic overlap is handled at admission, and the remaining recovery plus authorized M42/M44 acceptance is recorded. Source and fixture evidence alone do not establish loaded or live behavior.
- Schema v22 requires the repository's normal backup and compatible-binary recovery rules. Existing v18 binaries cannot open this database.

## Evidence

The [CX-05 handoff](../parallel/codex/CX-05-m52.md) records the v22 migration, scoped tests, full offline suite result and open runtime limitations. No provider was invoked, no live store was migrated, and no dispatcher process was started or reloaded.
