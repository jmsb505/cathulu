# Cathulu project definition

Status: NORMATIVE for product boundaries. Initial definition recorded 2026-10-20 from `CATHULU_PROJECT_KICKOFF.md` and refined through 2026-10-21 with the owner's platform, selector-surface, managed-memory, and optional specialist-model decisions.

## Purpose

Cathulu is a local-first desktop application and evidence layer around the Codex app. Its control plane, SQLite authority, memory, and evidence references remain local. Optional specialist calls may use a remote provider only when explicitly configured and only with the minimum sanitized input required; all specialist capabilities default to disabled. Cathulu adds durable portfolio state, managed memory, workflow history, quality evidence, and observability around Codex. It should become better informed over time without proportional growth in active context. The owner continues to work through Codex. Cathulu is not a second chat surface or a general-purpose agent runtime.

The product question is whether durable, selectively disclosed state improves verified software work, or achieves equivalent verified quality with less context reconstruction and resource use, compared with ordinary Codex use.

## Actors and ownership

- **Project owner / maintainer:** the local developer who works through Codex and owns decisions, approvals, and release choices.
- **Main Overseer:** the only user-facing agent, running inside Codex. It reads and updates Cathulu through domain-level MCP operations and remains responsible for task direction.
- **Sub-Overseers and workers:** future Codex-native agents for scoped context ownership or narrow execution. They report structured evidence upward; Cathulu does not run their model/tool loops.
- **Cathulu local services:** own durable state, Git reconciliation, telemetry normalization, workflow/quality state, and compact context resources.
- **Read-only dashboard:** presents current local state and evidence; it does not issue commands or mutate project state.
- **Selector surface:** Cathulu hosts model and effort selectors for the Main Overseer, default subagents, and named custom-agent roles in a settings surface separate from the read-only dashboard. Each preview offers project config, user base config, a named user profile, or export-only. Every file write requires an exact diff review and explicit Apply; stored or written values do not imply account availability or live-session effectiveness.

## Core workflows

1. The owner asks the Main Overseer in Codex to work across registered projects.
2. The Main Overseer reads a compact portfolio or project snapshot from Cathulu, then asks for only the scoped detail required for the task.
3. Codex performs coding and delegation with its native tools. The Overseer records objectives, decisions, run events, artifacts, and quality results through semantic MCP operations, and prepares a bounded task packet before recording a native delegation lifecycle action.
4. Cathulu reconciles registered repositories against Git and can read the facts from an already-existing native worker worktree. It presents project state, blockers, runs, quality evidence, worker report claims, and the separate Git verification result in the read-only dashboard. Explicit report review remains separate from parent-run quality verification.
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

The wider Phase 1 foundation then adds objectives, tasks, immutable decisions and workflow versions, append-only run events, quality evidence, available Codex telemetry, recovery behavior, and the provenance-preserving memory substrate with bounded scope activation.

### Explicitly deferred

- A custom agent/worker runtime or permanent model processes.
- Cathulu spawning, steering, cancelling, or accepting changes from native Codex workers, or creating/moving/removing/merging their worktrees; Cathulu records structured packets, reconciles existing worktree evidence, and records lifecycle telemetry while Codex remains the execution owner.
- Durable Sub-Overseers before measured repeated cross-project reconstruction.
- Hosted Cathulu control-plane/storage services, embeddings/vector databases, graph databases, distributed/event-sourcing infrastructure, and reinforcement learning. Explicit, minimized calls to optional remote specialist providers are a separate Phase 9 experiment governed by [ADR-0007](decisions/ADR-0007-optional-specialist-model-optimization.md).
- Automatic memory consolidation/promotion, semantic indexing, evidence compaction/garbage collection, and adaptive temperature or memory-policy automation until Phase 2 evidence justifies them.
- Advanced convergence and adaptive routing before trustworthy run evidence exists.
- Model/reasoning selectors in the first increment. M9 implements the later settings surface outside the read-only dashboard. Project/user/profile writes require a per-preview target choice and explicit apply; export-only remains available. Cathulu uses only the local CLI's whitelisted model/effort options, does not infer account availability, and does not claim runtime effectiveness.
- Optional specialist DecisionModel and ContextModel providers are a Phase 9 optimization layer. Jev and ScaleDown adapters are available for explicitly consented shadow trials, while all capabilities remain disabled by default. They do not replace Codex or become required dependencies. Any active workflow treatment requires separate evidence and explicit, reversible approval. Provider settings remain separate from the M9 Codex model/effort selectors.

## Domain terms and invariants

- **Project:** a registered local repository plus its current deterministic Git snapshot and durable project-level state.
- **Scope:** a logical context and ownership boundary spanning one or more projects; it may initially key off stable global/project/objective/task identities without a separate scope registry. Persistent Sub-Overseer scopes remain optional until evidence justifies them.
- **Run:** one recorded Overseer execution unit. A run may be an explicit Cathulu workflow execution tied to an immutable workflow version, or a sanitized observed Codex turn with no implied workflow, transcript, or success claim.
- **Evidence:** a traceable artifact or deterministic check result supporting a finding or outcome.
- **Delegation:** an auditable child task packet linked to a project, task, and run. Its `prepared`, `dispatched`, or `cancelled` state records Cathulu's view of the handoff; it does not prove a worker ran or completed work. A bounded worker report and append-only review may be attached as evidence, and a report may be compared with an existing linked worktree and explicitly associated SubagentStart/Stop metadata. Git verification, report review, and parent-run quality remain separate findings.
- **Memory class:** the kind of information (`working`, `canonical`, `episodic`, `evidence`, or `aggregate`).
- **Memory temperature:** retrieval readiness (`hot`, `warm`, or `cold`), independent of class. HOT is bounded per logical scope; WARM is selective; COLD is not automatically injected.
- **Canonical memory:** a distilled durable knowledge claim with a stable identity, content hash, temporal validity, provenance, and explicit supersession history. It supplements and links to authoritative domain records; it does not copy their mutable state.
- **DecisionModel:** an optional provider-neutral interface for bounded choices over compact structured state, with explicit escape outputs. It does not own execution or truth.
- **ContextModel:** an optional provider-neutral interface for classifying, extracting, compressing, or summarizing selected existing information while retaining source references. Its output is a projection, not authority.
- **Candidate success:** an agent's report that work appears complete; it is not a terminal verified outcome.
- **Verified success:** a terminal result with acceptance criteria and mandatory quality gates recorded as passing.

Invariants:

1. Codex remains the primary reasoning, coding, planning, debugging, and conversation system; Cathulu stores state and evidence and does not run agents. Optional specialists may only support bounded decisions or transform existing information.
2. Events are append-only. Decisions and workflow versions are immutable; supersession creates a new record. Mutable current state is a transactional projection of events and domain operations.
3. No agent report, timeout, or budget limit can bypass mandatory quality gates.
4. The portfolio dashboard has no write actions. Project and run state changes come from Codex-facing semantic operations; selector settings are a separate Cathulu configuration boundary and can write only after a reviewed diff and explicit user apply.
5. Cathulu's database, MCP server, memory, evidence references, and telemetry remain local. Remote specialist use is off by default, explicitly configured, visible in run attribution, and limited to minimum sanitized input. Full repositories, secrets, raw Cathulu memory, complete transcripts, and entire artifact stores are not sent by default.
6. Context moves progressively from portfolio to project/scope to task and finally to exact evidence; raw transcripts are not propagated upward.
7. Additional layers must earn their cost through measured improvement in verified outcomes.
8. Raw experience becomes durable agent memory only after useful knowledge is extracted and its provenance retained; raw evidence may have temporary retention.
9. Memory class and temperature remain separate. Temperature changes update one stable record and do not duplicate or delete knowledge.
10. Current and historical truth remain distinguishable through temporal validity and supersession; canonical knowledge is never silently overwritten.
11. Context is scope-specific and bounded. Unchanged, valid context already known by a receiver is not resent in full; transfers preserve a ledger of hashes and provenance without retaining raw prompt bodies.
12. SQLite remains authoritative. Search indexes return candidate IDs only, and COLD evidence is accessed only for explicit historical, audit, debugging, or evaluation needs.
13. Unique important provenance is protected conservatively. Redundant evidence may be compacted only after its useful knowledge has been preserved; deletion is deterministic and reference-aware.
14. Summaries are projections, not authority. Justified durable knowledge and unique provenance may remain indefinitely while active context stays bounded.
15. Specialist models are optional optimization layers: deterministic Cathulu code is preferred for exact work, Codex remains primary for open reasoning and creation, and deterministic QA remains final. A specialist cannot mark success, bypass quality gates, or autonomously delete durable memory.

## Constraints and quality drivers

| Priority | Driver | Architectural consequence | Intended evidence |
| --- | --- | --- | --- |
| 1 | Verified quality and correctness | Deterministic gates dominate agent self-report; retain provenance for each result. | Gate bypass tests, acceptance checks, and run evidence. |
| 2 | Local privacy and authority | No hosted Cathulu control plane; local IPC/stdio and storage; remote specialist egress is explicit, minimized, and independently disableable; avoid prompt/transcript storage and keep Codex approvals in force. | Configuration review, permission-boundary tests, and inspection of stored payloads and egress inputs. |
| 3 | Knowledge retention with bounded active context | Keep durable knowledge justified by evidence, compact redundant representation, and budget HOT context per scope. | Supersession/provenance tests, HOT-budget tests, known-by and transfer-ledger checks, growth and context-reuse measures. |
| 4 | Durability and recovery | SQLite transactions, migrations, append-only events plus current-state projections, Git reconciliation after restart. | Migration, atomicity, restart, and recovery tests. |
| 5 | Low operational and cognitive overhead | Reuse Codex-native agents, tools, MCP, hooks, Git and telemetry; defer speculative infrastructure. | Dependency and architecture review plus phased comparison against plain Codex. |

Windows is the Phase 1 platform; cross-platform support is a later option. No numeric performance or evaluation threshold is set without a measured baseline.

## Success and validation

The first increment succeeds when a registered local repository survives restart, its Git snapshot is current after reconciliation, Codex can read its compact state through local MCP, and the read-only dashboard renders the same state. Automated tests cover the project-registration path and the important persistence/Git/MCP invariants.

Phase 1 success further requires durable objectives/tasks/decisions/workflows/runs, event and state integrity, QA evidence that gates `verified_success`, supported local Codex telemetry ingestion, dashboard visibility for blockers and failed QA, and restart recovery. Its memory foundation must also keep class separate from temperature, preserve stable identity/content hashes and provenance, support temporal validity/supersession and freshness, track known-by state and context transfers, and enforce bounded HOT scope budgets without deleting durable knowledge. Automatic consolidation, semantic indexing, adaptive policies, and evidence garbage collection are later capabilities, not Phase 1 entry conditions. Later evaluation compares Cathulu-assisted runs with ordinary Codex runs using verified success, regressions, rework, context reconstruction, tool/resource use, stalls, and human intervention.

The exact first test seams and evaluation thresholds are selected in the implementation plan from the chosen public interfaces and measurable baseline. No vanity metric or model sentiment can substitute for deterministic evidence.

The optional specialist-model layer succeeds only if comparable real runs show a useful reduction in context, repeated processing, agent activations, retrieval overhead, or latency while maintaining or improving verified task success, QA, convergence, regression rate, rework, and human intervention. The evaluation method and acceptable quality guard are declared from baseline evidence before a controlled active pilot. Unobservable measures remain unknown. Shadow agreement or a model's confidence alone is not proof of workflow value.

