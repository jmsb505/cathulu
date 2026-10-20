# Cathulu project definition

Status: NORMATIVE for product boundaries. Initial definition recorded 2026-10-20 from `CATHULU_PROJECT_KICKOFF.md` and updated with the owner's platform and selector-surface decisions.

## Purpose

Cathulu is a fully local desktop application that adds durable portfolio state, memory, workflow history, quality evidence, and observability around the Codex app. The owner continues to work through Codex. Cathulu is not a second chat surface or a general-purpose agent runtime.

The product question is whether durable, selectively disclosed state improves verified software work, or achieves equivalent verified quality with less context reconstruction and resource use, compared with ordinary Codex use.

## Actors and ownership

- **Project owner / maintainer:** the local developer who works through Codex and owns decisions, approvals, and release choices.
- **Main Overseer:** the only user-facing agent, running inside Codex. It reads and updates Cathulu through domain-level MCP operations and remains responsible for task direction.
- **Sub-Overseers and workers:** future Codex-native agents for scoped context ownership or narrow execution. They report structured evidence upward; Cathulu does not run their model/tool loops.
- **Cathulu local services:** own durable state, Git reconciliation, telemetry normalization, workflow/quality state, and compact context resources.
- **Read-only dashboard:** presents current local state and evidence; it does not issue commands or mutate project state.
- **Future selector surface:** Cathulu will host model and effort selectors for the Main Overseer and sub-agents in a later milestone. How those settings reach Codex and whether separate role values are supported remain to be verified.

## Core workflows

1. The owner asks the Main Overseer in Codex to work across registered projects.
2. The Main Overseer reads a compact portfolio or project snapshot from Cathulu, then asks for only the scoped detail required for the task.
3. Codex performs coding and delegation with its native tools. The Overseer records objectives, decisions, run events, artifacts, and quality results through semantic MCP operations.
4. Cathulu reconciles registered repositories against Git and presents resulting project state, blockers, runs, and quality evidence in the desktop dashboard.
5. A run becomes `verified_success` only after its acceptance criteria and all mandatory deterministic gates pass. A timeout or missing evidence is `blocked` or `inconclusive`, never success.

The dashboard's first views are Overview, Activity, Project/Scope, and Evidence. The first increment needs only the project registry and Git state path, with the other views introduced as their supporting state exists.

## Scope and non-goals

### First coherent increment

Prove this end-to-end path on Windows:

```text
register a local repository
→ persist project state in SQLite
→ reconcile branch, HEAD, and dirty/changed paths through Git
→ expose portfolio/project state through local MCP
→ show the state in a read-only desktop view
→ verify the flow with automated tests
```

The wider Phase 1 foundation then adds objectives, tasks, immutable decisions and workflow versions, append-only run events, quality evidence, available Codex telemetry, and recovery behavior.

### Explicitly deferred

- A custom agent/worker runtime or permanent model processes.
- Durable Sub-Overseers before measured repeated cross-project reconstruction.
- Graphify integration, embeddings/vector databases, graph databases, distributed/event-sourcing infrastructure, cloud services, and reinforcement learning.
- Advanced convergence and adaptive routing before trustworthy run evidence exists.
- Model/reasoning selectors in the first increment. Cathulu will host selectors in a later milestone; the Codex configuration pathway and per-role support must be verified before implementation. The portfolio dashboard remains read-only; selector controls belong in a separate settings surface.

## Domain terms and invariants

- **Project:** a registered local repository plus its current deterministic Git snapshot and durable project-level state.
- **Scope:** a context and ownership boundary spanning one or more projects; optional until evidence justifies it.
- **Run:** one Overseer-controlled workflow execution, tied permanently to a workflow ID and immutable version.
- **Evidence:** a traceable artifact or deterministic check result supporting a finding or outcome.
- **Candidate success:** an agent's report that work appears complete; it is not a terminal verified outcome.
- **Verified success:** a terminal result with acceptance criteria and mandatory quality gates recorded as passing.

Invariants:

1. Codex remains the execution and conversation surface; Cathulu stores state and evidence and does not run agents.
2. Events are append-only. Decisions and workflow versions are immutable; supersession creates a new record. Mutable current state is a transactional projection of events and domain operations.
3. No agent report, timeout, or budget limit can bypass mandatory quality gates.
4. The portfolio dashboard has no write actions. Project and run state changes come from Codex-facing semantic operations; future selector settings are a separate, explicitly controlled Cathulu configuration boundary.
5. Cathulu's database, MCP server, and local artifacts remain local. Raw transcripts, secrets, and prompt text are not retained by default.
6. Context moves progressively from portfolio to project/scope to task and finally to exact evidence; raw transcripts are not propagated upward.
7. Additional layers must earn their cost through measured improvement in verified outcomes.

## Constraints and quality drivers

| Priority | Driver | Architectural consequence | Intended evidence |
| --- | --- | --- | --- |
| 1 | Verified quality and correctness | Deterministic gates dominate agent self-report; retain provenance for each result. | Gate bypass tests, acceptance checks, and run evidence. |
| 2 | Local privacy and authority | No hosted Cathulu service; local IPC/stdio; avoid prompt/transcript storage; keep Codex approvals in force. | Configuration review, permission-boundary tests, and inspection of stored payloads. |
| 3 | Durability and recovery | SQLite transactions, migrations, append-only events plus current-state projections, Git reconciliation after restart. | Migration, atomicity, restart, and recovery tests. |
| 4 | Low operational and cognitive overhead | Reuse Codex-native agents, tools, MCP, hooks, Git and telemetry; defer speculative infrastructure. | Dependency and architecture review plus phased comparison against plain Codex. |

Windows is the Phase 1 platform; cross-platform support is a later option. No numeric performance or evaluation threshold is set without a measured baseline.

## Success and validation

The first increment succeeds when a registered local repository survives restart, its Git snapshot is current after reconciliation, Codex can read its compact state through local MCP, and the read-only dashboard renders the same state. Automated tests cover the project-registration path and the important persistence/Git/MCP invariants.

Phase 1 success further requires durable objectives/tasks/decisions/workflows/runs, event and state integrity, QA evidence that gates `verified_success`, supported local Codex telemetry ingestion, dashboard visibility for blockers and failed QA, and restart recovery. Later evaluation compares Cathulu-assisted runs with ordinary Codex runs using verified success, regressions, rework, context reconstruction, tool/resource use, stalls, and human intervention.

The exact first test seams and evaluation thresholds are selected in the implementation plan from the chosen public interfaces and measurable baseline. No vanity metric or model sentiment can substitute for deterministic evidence.
