# ADR-0024 — Product workspace leases and uncertain execution recovery

- Status: Accepted M52 safety baseline; dispatcher admission amendments A–D are committed at `3444829134e7630c60b2e97b836cf5dbd7367300` and await Claude's re-review. Product dispatch remains held off by explicit Owner direction; review does not activate it.
- Scope: Durable workspace assignment, role/session/attempt binding, credential-source eligibility and recovery for Cathulu-managed product execution.
- Depends on: [PLAN-020](../plans/PLAN-020-mixed-provider-personal-project-os.md) M52 and [PLAN-021](../plans/PLAN-021-manual-claude-codex-development.md) CX-05.

## Context

PLAN-020 M52 requires product writers to use exclusive workspaces, coordinators to stay read-only, and uncertain provider submission to block duplicate work until reconciled. The repository records provider sessions, attempts and workspace leases. The dispatcher still uses the Codex app-server transport, but its Worker and Project Manager paths now require exact lease, permission and credential admission before process, thread or turn starts. Main inbox starts remain blocked because Main has no lease association.

The Owner's same-directory Claude Code/Codex development arrangement is a separate, manual exception under ADR-0023. It is not a product workspace lease and does not establish provider isolation, authentication or runtime safety.

## Decision

1. Schema v22 records immutable lease identity and append-only lease events. A writer is a Worker with write access to an exact linked worktree of the registered repository, sharing its Git common directory. The worktree must be clean when the lease is acquired. Project Manager/coordinator access is read-only; writers cannot use the registered project root.
2. With no semantic-overlap admission model, permit at most one active or quarantined writer lease per project. This is conservative project-wide serialization; it does not claim that disjoint tasks conflict semantically.
3. Permit one active provider session for each logical `binding_key` across all providers. On v21 upgrade, conflicting active session rows are retained but marked `unknown`, so the system does not silently choose a winner. A new session can replace the role's session only after every other attempt for that role is terminal; replacement inactivates the old session and revokes capabilities tied to it.
4. Bind a provider attempt and role capability to the same exact lease. Provider execution is eligible only when the adapter reports a known credential source and the permission manifest explicitly allows it. Unknown or unapproved credential sources block.
5. On expiry, release or revocation with unresolved attempts, mark the attempt `needs_reconcile`, quarantine the lease and revoke the capability. Retain its role/path locks until explicit terminal reconciliation. Do not automatically retry the provider call or delete, move or clean a worktree.
6. Keep product dispatch behind the Owner's fail-closed runtime gate. Before every Worker or Project Manager provider start, resolve and validate the exact active workspace lease, workspace identity, role capability, permission manifest and known allowed credential source. Reject unbound capabilities and attempts. Block Main inbox starts until Main has an exact lease model. Review is a prerequisite; only the Owner may decide whether to enable dispatch later. Do not migrate the live store or alter a running process as part of CX-05.

## Consequences

- A dirty Owner worktree is never cleaned to satisfy a lease. A lost acknowledgement leaves the workspace unavailable until the attempt is reconciled.
- A v21 role with sessions active at both providers becomes temporarily unavailable (`unknown`) until one provider session is explicitly recorded.
- M52 is not complete until the admission candidate is independently reviewed, semantic overlap is handled at admission, long-turn lease renewal is scheduled, and authorized M42/M44 acceptance is recorded. Source and fixture evidence alone do not establish loaded or live behavior.
- The product dispatch gate remains false during review. This changes default behavior: an unset CATHULU_ROLE_DISPATCHER_ENABLED previously enabled dispatch, and now means off. CATHULU_ROLE_DISPATCHER_ALLOWED_CREDENTIAL_SOURCES and CATHULU_ROLE_DISPATCHER_NETWORK_ACCESS also default to deny. Current adapters report an unknown credential source, so real dispatch remains blocked before spawn.
- Main inbox events remain blocked because there is no exact Main workspace-lease association. A workspace/Git identity revalidation failure quarantines the lease and revokes its capability. The current Codex sandbox provides workspace-level access and does not enforce individual packet paths.
- The dispatcher does not yet schedule renew_workspace_lease during long turns; an expired active turn fails closed and requires reconciliation.
- Workspace verification errors and successful verification with changed workspace/Git identity both quarantine the lease, but use distinct audit reasons. Verification errors can be transient; they still require explicit reconciliation because the system cannot prove the workspace identity.
- The public in-process validators `validate_provider_workspace` and `validate_role_provider_start` are trusted admission APIs. Production use must remain behind the dispatcher admission boundary; the current adapters still report `CredentialSource::Unknown`, so no provider start is eligible.
- Schema v22 requires the repository's normal backup and compatible-binary recovery rules. Existing v18 binaries cannot open this database.

## Evidence

The [CX-05 handoff](../parallel/codex/CX-05-m52.md) records the v22 migration, admission implementation, amendment tests, full offline suite result and open runtime limitations. No real provider was invoked, no live store was migrated, and no dispatcher process was started or reloaded.
