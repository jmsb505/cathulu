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
- For two or more independent eligible projects, Main prepares each available project packet and dispatches all eligible Project Managers concurrently. Main must not use a per-project `spawn → wait → spawn` loop or wait for first completion. M35 initially described this as Codex fan-out followed by monitoring; that was insufficient and is superseded by the no-wait dispatcher target in PLAN-015. A setup or dispatch failure is isolated to that project; other eligible PMs still dispatch.
- Each Project Manager assignment may dispatch at most two worker agents total, including replacement workers. Workers perform the repository work within the recorded project/task/path scope.
- All three roles must use the existing local Cathulu MCP server. Before reasoning from project history or assigning work, each role loads relevant accepted HOT project memory, selectively searches WARM candidates, transfers only selected knowledge through `prepare_context_transfer`, and includes provenance-bearing memory/artifact references in handoffs. Empty retrieval is recorded explicitly. COLD evidence is not automatically injected.
- Reuse existing memory, delegation, event, context-transfer, quality, and artifact authorities. Do not create a parallel memory store or infer success from agent reports.
- Codex remains responsible for agent execution, repository tools, approvals, Git, and worktrees. Cathulu remains the local evidence/control plane and read-only dashboard. This decision does not authorize changing user-level Codex configuration.
- The PULSO, Mecanografica proposals, and Cathulu repositories are separate current project scopes. Do not create cross-project dependencies or handoffs without an explicit owner task. The previously deferred natural cross-repository handoff evaluation remains backlog and is not a prerequisite.

## Runtime evidence — 2026-10-21

The M21 repository-free probe completed as Cathulu run 4 with `verified_success`. Main spawned one Project Manager (`/root/m21_project_manager`), which spawned one Worker (`/root/m21_project_manager/m21_probe_worker`). Cathulu delegations 1 and 2 were marked dispatched, and the Worker report was recorded. No project repository was accessed. This proves nested spawning for the tested client/session and one child Worker; it does not prove deterministic enforcement of the two-worker cap.

Main read the dashboard and HOT/WARM memory; the PM read `cathulu://dashboard`; the Worker successfully called `read_memories`. The PM did not have `read_memories` in its tool context, so role-specific memory-tool parity is unresolved. HOT/WARM results were empty. “No relevant memory” was passed to the PM and recorded in the run, but `prepare_context_transfer` cannot represent an empty transfer because it requires a memory ID. No hook events or hook metadata surfaced, so no hierarchy association was inferred. The `sub_overseer` role was accepted for the PM. These are runtime observations, not guarantees for every Codex client or future configuration; M22-M24 address deterministic hierarchy, memory activation, and truthful lifecycle evidence.

## Consequences

- Existing M6-M9 delegation records remain valid but cannot yet express every parent/child relationship or enforce the new caps. Additive hierarchy state and deterministic admission checks are required before Cathulu can claim it enforces the limits.
- Activity in the dashboard must be projected from actual runs, delegation relationships, and observed lifecycle events. Missing events stay visibly unknown; no progress or agent nodes are fabricated.
- Memory transfer ledger entries are the auditable evidence that relevant Cathulu memory was supplied. Agent instructions alone are not a hard enforcement boundary; the implementation plan adds deterministic checks and reports missing receipts.
- The tested client/session supports nested spawning and Cathulu MCP reachability, but tool parity and lifecycle correlation remain incomplete. If a future target client/configuration lacks the required capabilities, expose that gap and preserve the baseline workflow. Never let Main silently take over implementation or flatten the hierarchy.

## Amendment — 2026-10-22: parallel project fan-out

The owner observed Main coordinating multiple independent projects serially. The existing first-completion monitoring rule applied only after PM chats were active, leaving the initial dispatch order ambiguous. The Main Overseer must now prepare all eligible project handoffs and fan out their PMs before waiting on any result. Per-project setup failures do not delay other projects. M21 proved a single PM → Worker path only; it did not verify multi-PM concurrency. See [PLAN-014](../plans/PLAN-014-parallel-main-overseer-fanout.md).

## Amendment — 2026-10-22: no-wait dispatch and authority chain

The owner clarified that first-result monitoring still leaves Main's generation active and delays work that should continue independently. After dispatch acknowledgements, Main must end its turn; routine progress updates Cathulu without waking Main. A material decision, escalation, owner action, or project result starts a fresh short Main turn. Workers report to their PM; PMs resolve routine in-scope decisions; Main handles bounded portfolio decisions; the owner is the final authority for sensitive, irreversible, external, or out-of-scope actions. Only the project requiring owner input pauses. Current MCP cannot create/resume Codex chats or wake an idle role, so this is a target behavior, not a live capability. PLAN-015 tests local app-server support before implementation; PLAN-014's wait/monitor sequence is insufficient and is superseded for runtime behavior.

## Amendment — 2026-10-22: project workflow plans and hierarchical review

Before assigning a PM, Main creates a concise outcome-focused workflow brief for each requested project, with phases, boundaries, acceptance evidence, and approval gates. Each PM turns its brief into bounded Worker task packets and reviews Worker evidence before accepting or escalating it. Main reviews each PM's plan/results against the brief and evidence. Escalations move Worker → PM → Main → Owner, and Main surfaces Owner-required actions as soon as they arrive; only that project's dependent work pauses. These reviews are separate from deterministic QA. This strengthens role accountability without changing the one-Main, one-PM-per-project, two-Worker-per-assignment limits. M39 adds SQLite-enforced sequencing for briefs, plans, matching Worker packets, parent report reviews, unresolved action gates, and explicit owner-approval records. The MCP still cannot authenticate which Codex chat invoked a role-declared review, and M36 still blocks the no-wait dispatcher. See [PLAN-015](../plans/PLAN-015-event-driven-role-orchestration.md).

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
