# ADR-0009 — Bounded per-project agent hierarchy

- Status: Accepted
- Date: 2026-10-21
- Decision owner: Cathulu project owner

## Context

The current Cathulu delegation layer records packets, reports, worktree evidence, and optional Codex hooks. It intentionally does not spawn agents. The desktop graph can therefore be empty when an Overseer works without starting a Cathulu run or recording delegations and hook events. The intended operating model is now explicit: the Main Overseer coordinates projects, one Project Manager owns each active project, and that manager assigns actual repository work to a small bounded worker group. Every role must use Cathulu's existing memory and evidence tools.

The current Codex documentation says spawned agents inherit the parent turn's MCP servers/tools, depending on the client configuration. It does not establish a universal nested-spawn guarantee for every Codex surface. Native manager-to-worker spawning and MCP availability must be demonstrated in the target client before claiming the full hierarchy is live. See the [Codex subagents guide](https://learn.chatgpt.com/docs/agent-configuration/subagents) and [Codex hooks guide](https://learn.chatgpt.com/docs/hooks).

## Decision

Adopt a strict, project-scoped hierarchy:

```text
Owner
  └─ Main Overseer — portfolio coordination only
       ├─ one Project Manager for each active project
       │    └─ up to two implementation workers per manager assignment
       └─ independent projects remain independent unless the owner declares a cross-project task
```

- Main Overseer owns the conversation, priorities, task/run setup, dispatch, monitoring, evidence review, QA gate tracking, and escalation. It does not inspect repositories to implement, edit code, run project commands/tests, or commit project work.
- At most one Project Manager is assigned to a project within a single orchestration request. A Project Manager owns one project's planning and coordination and does not perform the implementation itself.
- Each Project Manager assignment may dispatch at most two worker agents total, including replacement workers. Workers perform the repository work within the recorded project/task/path scope.
- All three roles must use the existing local Cathulu MCP server. Before reasoning from project history or assigning work, each role loads relevant accepted HOT project memory, selectively searches WARM candidates, transfers only selected knowledge through `prepare_context_transfer`, and includes provenance-bearing memory/artifact references in handoffs. Empty retrieval is recorded explicitly. COLD evidence is not automatically injected.
- Reuse existing memory, delegation, event, context-transfer, quality, and artifact authorities. Do not create a parallel memory store or infer success from agent reports.
- Codex remains responsible for agent execution, repository tools, approvals, Git, and worktrees. Cathulu remains the local evidence/control plane and read-only dashboard. This decision does not authorize changing user-level Codex configuration.
- The PULSO, Mecanografica proposals, and Cathulu repositories are separate current project scopes. Do not create cross-project dependencies or handoffs without an explicit owner task. The previously deferred natural cross-repository handoff evaluation remains backlog and is not a prerequisite.

## Runtime evidence — 2026-10-21

The M21 repository-free probe completed as Cathulu run 4 with `verified_success`. Main spawned one Project Manager (`/root/m21_project_manager`), which spawned one Worker (`/root/m21_project_manager/m21_probe_worker`). Cathulu delegations 1 and 2 were marked dispatched, and the Worker report was recorded. No project repository was accessed. This proves nested spawning for the tested client/session and one child Worker; it does not prove deterministic enforcement of the two-worker cap.

Main read the dashboard and HOT/WARM memory; the PM read `cathulu://dashboard`; the Worker successfully called `read_memories`. The PM did not have `read_memories` in its tool context, so role-specific memory-tool parity is unresolved. HOT/WARM results were empty. “No relevant memory” was passed to the PM and recorded in the run, but `prepare_context_transfer` cannot represent an empty transfer because it requires a memory ID. No hook events or hook metadata surfaced, so no hierarchy association was inferred. The `sub_overseer` role was accepted for the PM. These are runtime observations, not guarantees for every Codex client or future configuration; M23-M24 address memory activation and truthful lifecycle evidence.

## M22 implementation note — 2026-10-21

Migration 10 adds an optional `parent_delegation_id` and a separate `project_manager_assignments` record. New `sub_overseer` delegations reserve the one active PM slot for their project; SQLite transactions and a unique partial index prevent concurrent duplicate assignments. The public MCP accepts worker parentage only when the parent is an active hierarchy-aware PM for the same project, task, and run. A child can be prepared before its PM dispatch, but dispatch is rejected until the PM is dispatched.

Each PM assignment can have at most two worker delegation records over its lifetime. Prepared, cancelled, and replacement workers count toward the cap; the count remains after the assignment closes. A PM assignment can complete through `complete_project_manager_assignment` only after each worker has reported or been cancelled. Cancelling a PM releases its project slot only when no child remains unfinished. Over-cap and parent-dispatch rejections append run events without creating a partial delegation. Existing `sub_overseer` role values remain unchanged, and old rows receive neither inferred parent links nor inferred assignment records.

Tests cover version-ten migration/reopen, concurrent PM and Worker admissions, slot release, cancellation/replacement counting, parent scope and role validation, dispatch order, and PM completion guards. These deterministic checks do not spawn Codex agents or prove live agent hierarchy telemetry; M23 and M24 retain the memory-activation and hook/activity-graph work.

## M23 implementation note — 2026-10-21

Schema migration 11 adds append-only, revisioned activation receipts and memory-reference rows. A ready receipt must reference a successful `search_memory_candidates` event associated with the delegation run and declared logical receiver. Selected memories must retain their current content hash and provenance, remain accepted/current/fresh and non-COLD, fit the project/task scope and HOT budget, and be evidenced by a same-run transfer or exact known-by hash. An explicit `no_relevant_memory` receipt is valid only after that bounded search; incomplete receipts remain in history but prevent Cathulu from recording dispatch. Old delegation and memory rows remain intact, and the implementation reuses SQLite's existing memory, known-by, transfer, artifact, and event authorities.

The receipt does not authenticate which native Codex agent called MCP: Cathulu receives no per-agent caller identity or tool inventory. Its receiver scope is a declared workflow identity, and the search event proves a successful scoped search was recorded. The role contract requires the receiving PM/Worker to perform the activation call; M21's PM `read_memories` gap remains a runtime capability issue until a fresh nested-agent probe observes the bounded search operation in that role. The database gate constrains Cathulu's dispatch record but cannot stop a native agent from acting before that record is written. Tests cover missing/incomplete/ready dispatch gates, idempotent and revised receipts, first/repeated/delta transfer behavior, no-relevant-memory search, stale/superseded/COLD/foreign/over-budget rejection, and legacy/hierarchy migration preservation. Local tests validate MCP and SQLite behavior, not native role identity or live spawning.

## Consequences

- Existing M6-M9 delegation records remain valid and readable, with null parentage and no inferred PM assignment. New hierarchy-aware delegations have deterministic parent links and SQLite-enforced admission limits; legacy rows remain outside those new assignments.
- Activity in the dashboard must be projected from actual runs, delegation relationships, and observed lifecycle events. Missing events stay visibly unknown; no progress or agent nodes are fabricated.
- Memory transfer ledger entries and ready activation receipts are the auditable evidence that relevant Cathulu memory was selected and supplied. Missing or incomplete receipts block Cathulu's dispatch transition. Native agent behavior remains governed by the Codex runtime and role contract; the receipt does not prove the native caller identity or prevent work before dispatch is recorded.
- The tested client/session supports nested spawning and Cathulu MCP reachability, but memory-tool parity and lifecycle correlation remain incomplete. If a future target client/configuration lacks the required capabilities, expose that gap and preserve the baseline workflow. Never let Main silently take over implementation or flatten the hierarchy.

## Reconsider if

- Runtime evidence shows the configured Codex client cannot support nested spawning or preserve Cathulu MCP access for child workers.
- Measured coordination overhead or verified project outcomes show the hierarchy consistently harms quality or efficiency.
- The owner changes the project count, manager concurrency, or per-assignment worker limit.

## Affected canonical documents

- [Project definition](../project-definition.md)
- [Architecture](../architecture.md)
- [Agent operating contract](../agent-operating-contract.md)
- [Roadmap](../roadmap.md)
- [PLAN-006](../plans/PLAN-006-bounded-project-agent-hierarchy.md)

