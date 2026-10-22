# ADR-0011 — Persistent logical agent roles

- Status: Accepted
- Date: 2026-10-22
- Decision owner: Cathulu project owner

## Context

Cathulu currently stores Main Overseer activity in runs and temporary Project Manager assignments in `project_manager_assignments`. Those records describe work lifecycles. They do not by themselves define the durable logical roles whose scoped context and tools should remain stable when a Codex chat ends or is replaced. Treating a chat or run as the agent identity loses the continuity that Cathulu's memory and known-by ledger are intended to preserve.

The role hierarchy must also remain truthful about runtime limits: Cathulu does not spawn Codex chats and cannot inspect their native identity or full tool list. It can persist role definitions, bind assignments to a role, require stable memory activation scopes, and record tool-use evidence when a successful MCP operation is observed.

## Decision

1. Persist one `main-overseer` logical role and one `project-manager:<project_id>` role for every registered project in SQLite. The project ID is the role's durable identity; a temporary assignment or Codex chat is not.
2. Store each role's memory source scope, stable receiver/known-by scope, and versioned tool-profile key. Main's source memory scope is global; a Project Manager's is its existing project scope. Both use a role-specific receiver scope so unchanged knowledge remains known across chat replacement and later assignments.
3. Keep `project_manager_assignments` as the bounded, temporary holder of a PM seat for one task/run. Link it to the persistent Project Manager role. Completing/cancelling an assignment releases the seat but does not remove or recreate the role.
4. Keep Workers task-scoped and transient. Their delegation IDs and explicit parent links remain the source for Worker activity; do not create permanent Worker roles without evidence that they need durable identities.
5. Require new hierarchy-aware PM memory retrieval and activation receipts to use the stable receiver scope for the project's logical PM role. Keep old receipts unchanged as historical evidence. The activation gate still requires a successful bounded memory search and valid memory references or an explicit no-relevant-memory result.
6. Expose persistent roles read-only through local MCP and the dashboard snapshot. In the dashboard, show one Main role and one PM role per registered project at all times. Show assignment/chat activity only when recorded. Context/tool indicators describe the persistent scope/profile and observed activation state; they do not claim an unobserved Codex chat is connected.
7. Keep runs, delegations, events, and activation receipts as audit/evidence records. They do not become tabs, graph pages, or role identities.
8. Do not add a separate memory store, transcript store, or Cathulu-managed agent runtime. Codex remains the reasoning and execution system; SQLite remains authoritative.

## Consequences

- Schema 12 adds the logical role registry and links Project Manager assignments to it. Existing schema-11 assignments are linked by their project ID; old activation receipts are retained without rewriting.
- Role key, project binding, memory source scope, receiver scope, and creation identity stay immutable. The versioned tool-profile key may advance to a new role-profile version without changing the logical role or its memory identity.
- Project registration creates its logical PM role transactionally, so an idle role still exists and has a stable memory/tool contract.
- Any chat continuing a PM assignment must use that role's stable receiver scope. `memory_known_by` and `context_transfers` then suppress redundant context independently of chat identity.
- The dashboard can display the complete logical hierarchy even when no assignment is active, while unknown current chat/tool presence remains visibly unknown.
- Codex-native identity and complete tool inheritance remain outside Cathulu's observability. A ready activation receipt proves the declared logical scope completed the required bounded search, not the identity of the native caller.

## Alternatives considered

- **Use each run or delegation ID as the role identity:** rejected because both are temporary and would fragment memory across chats/tasks.
- **Derive PM roles only in the dashboard from project rows:** rejected because the role-to-memory/tool contract would remain implicit and could not be validated by activation logic.
- **Create a second memory database per role:** rejected because existing project/global memory scopes, known-by state, and transfers already provide the needed authority.
- **Persist Workers indefinitely:** deferred because current evidence supports task-scoped Workers and no durable Worker context requirement has been established.
- **Record every Codex chat as active without native identity evidence:** rejected because Cathulu cannot observe or authenticate that identity.

## Reconsider if

- A project can legitimately require multiple concurrent PM role identities.
- Project IDs cease to be stable across database backup/restore or project re-registration.
- Runtime integration provides trustworthy native chat identity and tool-list evidence.
- Workers repeatedly resume across tasks and evidence supports persistent Worker seats.
