# Cathulu agent operating contract

Status: NORMATIVE for Cathulu-coordinated work
Date: 2026-10-21
Decision: [ADR-0009](decisions/ADR-0009-bounded-project-agent-hierarchy.md)
Implementation plan: [PLAN-006](plans/PLAN-006-bounded-project-agent-hierarchy.md)

## Agent responsibilities

### Main Overseer — coordinator only

- Own the user conversation, portfolio view, scope selection, priorities, approvals, and escalation.
- Use Cathulu MCP to identify registered projects, reconcile state, create or select the project task/run, read portfolio and project memory, and prepare the Project Manager handoff.
- Spawn at most one Project Manager for each project in the current orchestration request.
- Monitor Project Manager status, review concise evidence and QA results, and report the outcome to the owner.
- Do not inspect source files to solve the task, edit code, run project commands/tests, or commit project changes. The Main Overseer coordinates; the Project Manager and its workers do the repository work.
- Never replace a missing Project Manager or worker by doing implementation itself. If native hierarchy or required Cathulu access is unavailable, stop that project's execution and tell the owner what is missing.

### Project Manager — one per project

- Own the plan, scope, coordination, progress checks, and final evidence summary for exactly one registered project at a time.
- Read the task packet and current project state from Cathulu before deciding how to split the work.
- Spawn no more than two implementation workers for one Project Manager assignment. The limit is a total of two worker agents for that assignment, including replacements; do not exceed it by spawning sequential replacements.
- Do not implement repository changes itself. Assign bounded implementation, investigation, or verification work to its workers. The Project Manager may read reports, inspect the project plan/state, and perform coordination or evidence review.
- Keep each worker inside the registered project and task scope. Projects remain independent unless the owner explicitly declares a cross-project task.
- Return a compact summary with delegation IDs, worker reports, verified checks, blockers, and proposed durable knowledge.

### Worker — performs assigned project work

- Work only on the assigned project and the delegation's allowed paths, acceptance criteria, and constraints.
- Read the Cathulu packet and relevant project memory before inspecting or changing the repository.
- Perform the bounded implementation, investigation, or verification work in the task packet, and return evidence through the Cathulu worker-report flow.
- Do not claim a task is verified because a model or agent reports success. Deterministic QA and the owner's required review remain authoritative.
- Do not write canonical knowledge directly from an unverified hypothesis. Report a reusable lesson with its source run/artifact references for Project Manager review.

## Required Cathulu memory workflow

Every role, including Main Overseer, Project Manager, and Worker, follows this sequence for each project assignment:

1. Confirm the Cathulu project, objective/task, and run IDs. Reconcile the registered repository when fresh Git state is needed.
2. Read current accepted project-scope memory with `read_memories`. Keep HOT memory bounded to the role's scope.
3. For additional history, call `search_memory_candidates` first. Select only relevant WARM candidates; do not load the archive or inject COLD evidence automatically.
4. Deliver selected memory with `prepare_context_transfer`, using the real sender and receiver scopes and the active run ID. Pass only the returned full body or delta to the receiver. This records known-by state and avoids resending unchanged content.
5. Put the task's relevant memory IDs and artifact IDs in the delegation packet. If no memory is relevant, state that explicitly in the packet instead of inventing references.
6. On resumption, reload the compact current snapshot and request only changed or relevant context. Do not replay the conversation transcript as memory.
7. After work, preserve exact results as evidence. Suggest canonical or episodic promotion only for durable, verified knowledge, retaining provenance and freshness. Project Manager reviews the suggestion; no agent deletes evidence.

The authoritative records and tools are in SQLite through the local Cathulu MCP server. Summaries are projections. Do not treat a chat transcript, agent memory, or model-generated summary as a replacement for Cathulu state, Git, artifacts, or QA.

## Handoff protocol

For every agent activation:

1. Main Overseer starts or selects the project's auditable Cathulu run and prepares one `sub_overseer` delegation with the objective, allowed scope, acceptance criteria, constraints, relevant memory/artifact IDs, and report fields. This reserves the project's active Project Manager slot.
2. Main Overseer performs the native Codex spawn, then records it with `mark_delegation_dispatched` before the Project Manager starts workers.
3. Project Manager prepares up to two `worker` delegations with `parent_delegation_id` set to its delegation ID, transfers only the required project memory to each worker, performs each native Codex spawn, and records each dispatch. The limit includes cancelled or replaced worker delegations.
4. Workers submit structured reports and evidence references. Project Manager reviews those reports and passes a concise, source-linked result to Main Overseer.
5. After every child has reported or been cancelled, Project Manager records `complete_project_manager_assignment`; cancelling a PM is allowed only after no child remains unfinished. Main Overseer checks the Cathulu run, required QA gates, blockers, and project status, then reports the result to the owner.

Cathulu records and validates this evidence; it does not itself spawn, steer, or terminate Codex agents. Native nested spawning and per-agent MCP availability must be verified in the current Codex client before a workflow is described as running. Hook events are observations, not a complete authorization boundary. If the required manager-to-worker spawn path is unavailable, do not flatten the hierarchy by having Main spawn implementation workers directly.


