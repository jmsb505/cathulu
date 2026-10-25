# Cathulu project definition

Status: NORMATIVE for product boundaries. Initial definition recorded 2026-10-20 from `CATHULU_PROJECT_KICKOFF.md` and refined through 2026-10-21 with the owner's platform, selector-surface, managed-memory, and optional specialist-model decisions.

Rebaseline (2026-10-25): [ADR-0022](decisions/ADR-0022-mixed-provider-personal-project-os.md) changes the product direction. Purpose and the pivot scope below own current target intent. The existing actors, coding workflows and first-increment descriptions later in this document describe the compatibility baseline; their Codex-only wording does not constrain the provider-neutral target. Existing safety, evidence, memory and role-cap invariants remain in force. Detailed migration proposals and observed gaps are in the [migration assessment](pivot-migration.md).

## Purpose

Cathulu is a persistent, local-first personal project operating system for one primary user. It owns canonical project/task/priority/decision state, curated memory, policy, evidence and continuity. Claude Code and Codex are replaceable cognition and execution resources beneath that authority. Projects, open work, blockers, priorities and checkpoint/resume queries should remain useful without either provider. Cathulu reuses provider tooling for model/tool loops and preserves local SQLite authority; no required hosted backend or model API billing is introduced.

The product question is whether Cathulu helps the Owner resume and advance useful accepted work with less context reconstruction, user attention and provider-capacity pressure while preserving verified quality. Comparative provider/workflow benefit requires measured evidence; routing defaults alone do not establish it.

## Pivot scope and invariants

The Owner remains the final authority for consequential decisions. Cathulu validates state transitions and disclosure; providers propose, implement or review inside granted scope. Logical roles persist independently of provider sessions. Current Main/PM/Worker responsibilities, one active PM per project and two Workers total per PM assignment remain unchanged until explicitly revised and validated. No new live provider or additional project scope is activated by planning this pivot.

The first pivot increment is provider-free structured task/priority/dependency state followed by immutable project checkpoints and deterministic resume. [PLAN-020 M48-M49](plans/PLAN-020-mixed-provider-personal-project-os.md) defines its proofs. Later increments add controlled personal/non-code project state, typed artifacts, provider adapters, exclusive writer workspaces, availability routing and reviewed handoffs. Claude's cognitive/UX and Codex's bounded-execution preferences are configurable priors. Data privacy, deterministic QA, parent reviews, no-wait event routing, headless MCP and the desktop pause remain in force.

No required cloud database, vector database, permanent swarm, transcript dump, automatic production deployment or silent API overflow belongs to V1. Personal/provider disclosure, actual subscription tooling, additional role types, paused UI resumption and calibrated routing claims have dependent gates in the [assessment](pivot-migration.md), [definition gate](readiness/definition-gate.md) and [kickoff state](kickoff-state.json).

## Development and supervised use

Under [ADR-0021](decisions/ADR-0021-continuous-supervised-development.md), exercising Cathulu's complete core workflow through already authorized work in other registered projects is part of Cathulu development. Development agents arrange those runs, capture source-backed results, and fix the integration defects they reveal. Cathulu itself is developed and repaired outside its dispatcher, so dispatch cannot prevent its own repair. The Owner's manual same-directory Claude Code / Codex workflow is governed by [ADR-0023](decisions/ADR-0023-manual-shared-directory-development.md) and [PLAN-021](plans/PLAN-021-manual-claude-codex-development.md), which preserve file ownership and stable-tree integration.

Bounded, supervised, reversible local core implementation and use may proceed after applicable safety, scope, correctness, and QA checks, before comparative benefit is proven. Implementation, built artifact, loaded process, live acceptance, and measured benefit are distinct evidence states. Milestones are validation checkpoints within the approved outcome; accepted work continues without a new Owner prompt solely because a checkpoint passed. Runtime bootstrap and recovery use the existing M42/M44 plans. Optional-provider consent, active-treatment approval, autonomous policy promotion, and production/data/access boundaries retain their own gates.

## Actors and ownership

- **Project owner / maintainer:** the local developer who works through Codex and owns decisions, approvals, and release choices.
- **Main Overseer:** one logical, owner-facing role. A Codex chat can temporarily represent it; the chat is not the identity. It coordinates project work, uses Cathulu state and memory, assigns one Project Manager per active project, resolves bounded cross-project decisions, and escalates owner-authority actions. It does not implement project work. Cathulu's current integration cannot guarantee one exclusive live chat; PLAN-015 proposes managed persistent role threads.
- **Project Managers:** Codex-native agents, with at most one assigned to a project per orchestration request. They plan and supervise one project, use Cathulu MCP and scoped memory, and may dispatch no more than two worker agents total per assignment. Project Managers do not implement repository changes themselves.
- **Workers:** Codex-native agents that perform bounded implementation, investigation, or verification work inside one Project Manager's project/task scope. They use Cathulu memory and evidence tools and return structured reports upward.
- **Cathulu local services:** own durable state, Git reconciliation, telemetry normalization, workflow/quality state, compact context resources, and the implemented asynchronous Codex role-thread dispatcher and event router. The default-on dispatcher becomes live after the matched MCP build is reloaded; end-to-end use remains subject to M42 verification.
- **Read-only dashboard:** presents current local state and evidence; it does not issue commands or mutate project state.
- **Selector surface:** Cathulu hosts model and effort selectors for the Main Overseer, default subagents, and named custom-agent roles in a settings surface separate from the read-only dashboard. Each preview offers project config, user base config, a named user profile, or export-only. Every file write requires an exact diff review and explicit Apply; stored or written values do not imply account availability or live-session effectiveness.

## Core workflows

1. The owner gives the Main Overseer a task and the registered project or projects it concerns; projects remain independent unless the owner explicitly declares a cross-project task.
2. Main reads a compact portfolio snapshot, selects the project task/run, loads relevant scoped HOT memory, searches WARM knowledge selectively, and prepares an auditable context transfer.
3. Main assigns one Project Manager to each active project and does no implementation work. In the target event-driven flow, independent project dispatches proceed in parallel; a project that needs owner input pauses on its own while unrelated projects continue. Each PM uses Cathulu MCP, plans work within one project, and may dispatch at most two workers total for the assignment.
4. Project Managers transfer only relevant Cathulu memory to workers using the existing known-by and context-transfer ledger. Workers perform scoped project work and submit evidence-backed reports; managers resolve routine in-scope choices and return only material decisions, blockers, owner actions, and results up the chain.
5. The implemented event-driven flow queues independent PM work and ends Main's dispatch turn after queue acceptance; routine updates stay in Cathulu. Main resumes in a fresh short turn only when it must decide, route an escalation, handle owner input, or review a material result. The default-on dispatcher becomes live after reloading the published MCP build. Actual multi-project behavior, project tool access, Codex thread ownership, and recovery still require PLAN-015 M42 live verification. The read-only dashboard presents persisted runs, delegations, events, and quality evidence.
6. A run becomes `verified_success` only after its acceptance criteria and all mandatory deterministic gates pass. A timeout or missing evidence is `blocked` or `inconclusive`, never success.

See the normative [agent operating contract](agent-operating-contract.md), [ADR-0009](decisions/ADR-0009-bounded-project-agent-hierarchy.md), and [ADR-0016](decisions/ADR-0016-event-driven-role-orchestration.md). M21 observed one native PM-to-Worker chain; M36 verified the isolated app-server protocol and M40/M41 implemented the local dispatcher and event routing. Use a bounded, authorized real assignment to gather M42 evidence with the normal gates and honest unverified status. Claim operational readiness only for live behavior actually demonstrated; full M42 completion requires its acceptance criteria to pass.

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

- A custom agent/worker reasoning runtime or permanent model processes. The planned local dispatcher may manage Codex role-thread lifecycle, but Codex remains the model and project execution harness.
- Durable Sub-Overseers before measured repeated cross-project reconstruction.
- Hosted Cathulu control-plane/storage services, embeddings/vector databases, graph databases, distributed/event-sourcing infrastructure, and reinforcement learning. Explicit, minimized calls to optional remote specialist providers are a separate Phase 9 experiment governed by [ADR-0007](decisions/ADR-0007-optional-specialist-model-optimization.md).
- Automatic memory consolidation/promotion, semantic indexing, evidence compaction/garbage collection, and adaptive temperature or memory-policy automation until Phase 2 evidence justifies them.
- Calibrated convergence claims and autonomous adaptive routing before trustworthy run evidence exists. Supervised local analysis and evaluation mechanics may be developed and exercised while insufficient evidence is reported explicitly.
- Model/reasoning selectors in the first increment. M9 implements the later settings surface outside the project graph. Project/user/profile writes require a per-preview target choice and explicit apply; export-only remains available. The graph has one separate Owner-action exception: a two-step local approve/decline control records a decision for one exact request and never executes the requested operation. Cathulu uses only the local CLI's whitelisted model/effort options, does not infer account availability, and does not claim runtime effectiveness.
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

1. Codex remains the primary reasoning, coding, planning, debugging, and conversation system. Cathulu may coordinate and manage Codex role-thread lifecycle, but it does not replace Codex's reasoning or perform repository implementation. Optional specialists may only support bounded decisions or transform existing information.
2. Events are append-only. Decisions and workflow versions are immutable; supersession creates a new record. Mutable current state is a transactional projection of events and domain operations.
3. No agent report, timeout, or budget limit can bypass mandatory quality gates.
4. The portfolio dashboard has no write actions. Project and run state changes come from Codex-facing semantic operations; selector settings are a separate Cathulu configuration boundary and can write only after a reviewed diff and explicit user apply.
5. Cathulu's database, MCP server, memory, evidence references, and telemetry remain local. Remote specialist use is off by default, explicitly configured, visible in run attribution, and limited to minimum sanitized input. Full repositories, secrets, raw Cathulu memory, complete transcripts, and entire artifact stores are not sent by default.
6. Context moves progressively from portfolio to project/scope to task and finally to exact evidence; raw transcripts are not propagated upward.
7. Additional layers must earn their cost through measured improvement in verified outcomes before benefit is claimed or autonomous promotion is justified. Supervised, reversible local core development and evaluation may generate that evidence under ADR-0021.
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

