# PLAN-008 — Persistent logical agent roles

Status: Role logic, one-graph dashboard, automated checks, and the versioned Windows release are validated and published behind the stable Codex junction; the already-running MCP process has not reconnected. Native visual/runtime smoke remains pending.
Created: 2026-10-22
Last updated: 2026-10-22

## Objective

Make Cathulu's Main Overseer and each registered project's Project Manager persistent logical roles whose identity, memory scope, and tool profile survive Codex chat changes. Treat chats and task delegations as temporary occupants or assignments, never as the role identity. Show that model in one project-spanning dashboard graph without run tabs.

## Motivation

The current graph has one Main Overseer root, but Project Manager nodes appear only when a recent run contains a recorded delegation. The database's `project_manager_assignments` table represents a task/run assignment and is released at completion; it does not represent the long-lived project role. Memory activation receipts accept a receiver scope but do not yet require all chats occupying a Project Manager seat to use the same stable scope. This makes the dashboard and the operating model disagree about what persists.

## Current state

- SQLite schema 11 in `target/m26-current-source` contains hierarchy parentage, temporary Project Manager assignments, and activation receipts.
- The tracked checkout is still schema 9. Its recent dashboard projection changes are not reconciled with that schema-11 source snapshot.
- Project and global memory scopes, `memory_known_by`, context transfers, and activation receipts already exist and remain authoritative.
- The dashboard shows one Main Overseer and every project, but derives the visible agents from each project's latest run. It does not show an idle persistent Project Manager seat.
- Main/PM/Worker spawning is owned by Codex. Cathulu can record assignment and successful memory activation, but cannot inspect the native chat identity or tool list.

## Desired state

- Exactly one durable `main-overseer` logical role exists.
- Every registered project has exactly one durable Project Manager role keyed by the stable Cathulu project ID.
- A role owns a stable context/known-by scope and a versioned tool-profile key. Main uses the global memory source scope; a Project Manager uses its project memory source scope.
- A task-scoped PM assignment occupies the persistent Project Manager role. A different Codex chat may resume that role by using the same stable scope; it does not create a new role or resend unchanged memory.
- Workers remain task-scoped delegations and are shown only when recorded with explicit parentage or hook evidence.
- The read-only graph always shows the Main role and one PM role per registered project. It shows activity only from persisted assignments, runs, activation receipts, and observed hooks. It has no run-selection pages or run tabs.

## Scope

- Reconcile the validated schema-11 M22/M23 source snapshot into the tracked source while preserving the UX-M3 project-wide snapshot changes.
- Add an additive SQLite migration for stable logical role records, tool-profile keys, memory-scope bindings, automatic PM role creation for new projects, and a link from temporary PM assignments to their persistent role.
- Enforce the role's stable receiver scope during PM memory retrieval and activation. Preserve existing activation receipts unchanged as historical evidence.
- Expose role definitions through local MCP and the dashboard snapshot.
- Update the graph to show persistent role seats and context/tool indicators; display active occupants only when actual assignment/activation evidence exists.
- Update the architecture, decisions, agent operating contract, dashboard plan, roadmap, and README.

## Out of scope

- Cathulu spawning, steering, or terminating Codex chats.
- Guessing a native chat ID from run/delegation IDs or hook names.
- Persisting Worker seats beyond their task/PM assignment.
- Replacing SQLite memory, run, assignment, or event systems.
- Cross-repository project links or synthetic handoffs.
- Showing run history as graph pages or introducing another activity tab.
- Enforcing native Codex tool availability beyond the existing successful memory-activation evidence.

## Source-of-truth impact

- SQLite gains one logical-role table and a foreign-key binding on temporary Project Manager assignments. Existing project, memory, delegation, run, and activation tables remain authoritative for their current domains.
- `cathulu://roles` and `dashboard_snapshot` expose read-only role projections.
- [Architecture](../architecture.md), [ADR-0011](../decisions/ADR-0011-persistent-logical-agent-roles.md), [ADR-0008](../decisions/ADR-0008-compact-dashboard-and-floating-monitor.md), [ADR-0009](../decisions/ADR-0009-bounded-project-agent-hierarchy.md), [agent operating contract](../agent-operating-contract.md), [PLAN-005](PLAN-005-desktop-support-dashboard.md), and [roadmap](../roadmap.md) describe the intended model.

## Existing decisions and constraints

- ADR-0001 keeps Cathulu local-first and SQLite authoritative.
- ADR-0008 keeps the dashboard read-only and forbids fabricated topology/activity.
- ADR-0009 keeps Main as coordinator, one PM per project assignment, and a maximum of two Workers per assignment.
- M22/M23 provide enforced temporary assignments and memory activation receipts; they must be reconciled into the tracked schema-11 source before a new migration is added.
- Memory source scopes and receiver known-by scopes are separate: PMs retrieve from the project scope while sharing a stable Project Manager receiver key across chats and assignments.
- Native chat identity and tool-list availability are not observable through current Cathulu MCP. Unknown occupancy/tool reachability stays unknown.
- SQLite enforces one Main role record and one PM role record per project. Codex does not expose the current chat ID to Cathulu, so Cathulu cannot identify or lock an exclusive chat occupant; any replacement Main chat must reuse the one stable receiver scope.
- User instruction: stack changes and produce one final desktop build after the implementation settles.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`, `ponytail-balanced`
- Existing React/TypeScript, Rust, Tauri, SQLite, and MCP infrastructure; no dependency additions.
- Existing Windows build scripts and local M26 release path documentation.

## Dependencies

- Schema-11 M22/M23 source and migration history.
- Stable project IDs and existing memory scope/known-by/transfer ledgers.
- Current native dashboard snapshot and local MCP resources.

## Risks and unknowns

| Risk or unknown | Mitigation |
| --- | --- |
| Schema-11 source snapshot differs from the tracked checkout | Merge only the validated M22/M23 source and test coverage; preserve later dashboard changes; compare final migration version and docs. |
| Existing activation receipts used chat-specific receiver keys | Keep old receipts historical; require stable keys only for new PM activation; do not rewrite provenance. |
| A ready receipt does not prove the native caller identity or full tool list | Label it as observed memory retrieval for the declared logical role; do not infer a live chat identity. |
| A project may have no PM assignment for a long period | Keep its logical PM role visible as idle with its stable project context/tool profile. |
| Codex does not expose native chat identity or an exclusive occupant token | Keep one durable logical role in SQLite and reuse its memory receiver scope. Label actual chat occupancy unknown; do not claim Cathulu can prevent two separately opened chats from invoking the same logical role concurrently. |
| Current/active assignments might fall outside recent-run limits | Snapshot active PM assignments and their children independently of historical run caps. |
| Building schema 12 against the user's existing schema-11 database | Use additive migration only; validate upgrade and reopen on a disposable DB; build against the reconciled source. |

## Milestones

### M27.1 — Reconcile the current schema-11 source

**Goal:** Bring validated M22/M23 hierarchy and activation code/tests into the tracked checkout before extending their schema.

**Subtasks**

- [x] Merge migrations 10/11, hierarchy/activation operations, MCP exposure, and relevant test coverage from the current M26 source snapshot.
- [x] Preserve UX-M3 per-project run/delegation/hook snapshot behavior and current desktop changes.
- [x] Update the tracked hierarchy plan/architecture status to match the reconciled source.

**Affected areas:** `src/migrations.rs`, `src/lib.rs`, `src/delegation.rs`, `src/mcp.rs`, `src/memory.rs`, `tests/delegation_lifecycle.rs`, hierarchy and source-reconciliation documentation.

**Dependencies:** Validated local M26 schema-11 source snapshot.

**Acceptance criteria**

- [x] Tracked migrations advance through schema 11 and preserve old records without inferred parentage or activation receipts.
- [x] Existing M22/M23 assignment caps and activation gate remain intact.
- [x] UX-M3 dashboard projection still includes each project's latest run and active assignments, without replacing the one-graph UX.

**Validation**

- [x] Focused hierarchy and activation checks on disposable SQLite data.
- [x] Full Rust test suite and `git diff --check`.

**Documentation updates:** PLAN-006, architecture, roadmap, and PLAN-007 reconciliation note.

**Applicable specialized skills:** `milestone-delivery`, `ponytail-balanced`.

**Expected Git checkpoint:** Reconciled local source checkpoint; no direct-to-main advancement until all stacked work passes.

### M27.2 — Persist role identity and bind activation context

**Goal:** Persist stable Main/Project Manager roles and ensure their chats share the same logical context and tool contract.

**Subtasks**

- [x] Add additive schema 12 for one global Main Overseer and one Project Manager role per project, with stable memory-scope keys and versioned tool-profile keys.
- [x] Seed existing projects and create the Project Manager role transactionally for each new registered project.
- [x] Bind every temporary Project Manager assignment to its persistent role; preserve role identity after the assignment closes.
- [x] Require new PM activation retrieval and receipts to use the role's stable receiver scope. Keep Worker receiver scopes task/delegation-specific.
- [x] Expose the role catalog through a read-only MCP resource and dashboard snapshot.
- [x] Update role operating instructions so replacement chats load the same role profile and memory scope before acting.

**Affected areas:** additive migrations, role/domain projection, delegation activation validation, MCP resources, `dashboard_snapshot`, agent operating contract, architecture, ADR-0011.

**Dependencies:** M27.1; existing project, memory, transfer, known-by, and activation records.

**Acceptance criteria**

- [x] Exactly one Main role and exactly one PM role per registered project exist after migration and project registration.
- [x] Repeated PM assignments for one project retain one role key and one receiver scope; active assignment limits remain unchanged.
- [x] A PM activation with a chat-specific or foreign role scope cannot become ready or be dispatched.
- [x] Existing schema-11 receipts remain intact and queryable.
- [x] No second memory store or duplicate run/task authority is introduced.
- [x] Role profile metadata describes intended tools; actual per-chat availability remains unknown until observed.

**Validation**

- [x] Migration/upgrade/reopen check on disposable legacy SQLite databases, including schema-11 hierarchy state.
- [x] Focused checks cover role seeding, automatic project role creation, repeated assignment identity, Main/PM known-by reuse across chat changes, and rejection of mismatched PM scope.
- [x] Full Rust tests, `cargo fmt --all --check`, and `git diff --check`.

**Documentation updates:** ADR-0011, ADR-0009, architecture, PLAN-006, local MCP guide, and this plan.

**Applicable specialized skills:** `milestone-delivery`, `ponytail-balanced`.

**Expected Git checkpoint:** Validated role identity and activation checkpoint on the current work branch.

### M27.3 — Project-spanning persistent-role graph

**Goal:** Make the main dashboard show the stable logical hierarchy and current project interaction state in one graph.

**Subtasks**

- [x] Always render one Main Overseer and exactly one Project Manager node for each registered project, including idle projects.
- [x] Add small context/tool indicators using the role's persisted scope/profile and observed activation status.
- [x] Link active assignment and explicitly parented Worker nodes to the correct persistent PM role.
- [x] Keep run IDs/history behind the evidence model; remove run title/page language from the main graph.
- [x] Remove dashboard navigation tabs; the main dashboard presents the one role graph, with Codex settings kept separate.
- [x] Keep activity animation limited to currently recorded running work and respect reduced-motion settings.

**Affected areas:** `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, dashboard snapshot query, ADR-0008, PLAN-005, README.

**Dependencies:** M27.2; schema-12 role projection.

**Acceptance criteria**

- [x] One graph shows the whole logical Main → Project Manager → observed Worker picture; there are no run selectors or bottom run dots.
- [x] PM role nodes remain stable when assignments/chats end or change; status becomes idle/unknown rather than deleting the role.
- [x] No chat identity, Worker, or relationship is fabricated.
- [x] Context and tool indicators have accessible names/tooltips and distinguish configured role scope/profile from observed runtime activation.
- [x] The graph is the only dashboard view; no run, repository, or memory tabs are present.
- [ ] Layout fits the 500 × 580 side-by-side window without page-wide horizontal or vertical scrollbars at the supported project count; excess activity is bounded within the graph.

**Validation**

- [x] Production desktop and MCP release builds from the reconciled tracked source, with the Tauri `custom-protocol` feature, into an isolated target directory.
- [x] Migration and targeted role/activation checks from M27.2.
- [x] `npm run build`, frontend tests, the full Rust test suite, formatting, and `git diff --check`.
- [ ] Native Windows review of graph states and one final app build; do not restart the Codex MCP transport as part of this milestone.

**Documentation updates:** ADR-0008, PLAN-005, README, architecture, roadmap, and completion evidence here.

**Applicable specialized skills:** `milestone-delivery`, `ui-ux-pro-max`, `ui-ux-research`, `ponytail-balanced`.

**Expected Git checkpoint:** One locally validated implementation checkpoint after all three M27 milestones pass; publish only through the normal validated Git path.

## Final integration validation

- Verify the schema-11 → schema-12 migration and restart/reopen without touching the user's database.
- Build the full desktop app once after the stacked implementation is complete; also build the bundled frontend and MCP/core components through the production build.
- Confirm the built app uses the same local SQLite store and latest source revision; do not install or launch an older binary.
- Inspect the final diff, run the plan's focused checks, validate all docs/links, and record native visual limitations.
- Do not launch or focus the desktop window automatically during validation.

## Rollback / recovery

Schema 12 is additive. If UI or provider-role presentation must be rolled back, revert the dashboard projection while retaining the role table and existing assignments as valid domain state. Do not downgrade a database or remove role/context history. Restore the last schema-compatible app binary if the built app fails validation.

## Progress

- [x] Confirmed user intent: Main Overseer and each Project Manager are persistent logical seats; Codex chats are replaceable occupants; role nodes show context/tool indicators.
- [x] Reconcile schema 11 into the tracked checkout.
- [x] Implement schema-12 logical role registry and stable activation scopes.
- [x] Update the dashboard to render persistent roles and observed activity in one graph.
- [x] Build and validate the stacked local app in an isolated target directory.
- [ ] Review the native graph/window visually without interrupting Codex; do not claim actual chat identity or tool availability.

## Implementation discoveries / decisions

- The existing `project_manager_assignments` row is deliberately task/run-scoped and closes after the task; it is not the role identity.
- Stable role identity is separate from both task assignments and native Codex chat identity.
- The SQLite unique index guarantees one durable Main role record. The current Codex integration does not provide Cathulu with a native chat identity or exclusive occupant lease, so multiple chats can reference the same role concurrently; Cathulu cannot identify which one is currently acting as Main.
- PM source memory remains the existing `project:<id>` scope; its known-by/receiver identity is a stable logical scope shared across assignments.
- The graph's context/tool badges represent role configuration and recorded activation evidence. They must not imply that an unobserved chat is currently connected.

## Completion evidence

M27 checks: `cargo test --locked --offline` passed (full Rust suite); `npm test -- --reporter=dot` passed (5 UI tests); `npm run build` passed; `cargo fmt --all --check` and `git diff --check` passed. Matching Windows release binaries built with Tauri `custom-protocol` at `target/m27-build/release/cathulu-mcp.exe` and `target/m27-build/release/cathulu-desktop.exe`. The configured `target/mcp-refresh/release` pair was not replaced, and no application/MCP process was started or restarted. Native visual/window behavior remains unverified because opening it could interrupt the active Codex session.

Release-refresh follow-up: `scripts/build-current.ps1` now publishes the matched schema-12 pair under `target/mcp-versions/858B4E0C5836ED87EF3D5CFA`; Codex points once to the stable `target/mcp-current/cathulu-mcp.exe` junction. MCP and desktop SHA-256 values are recorded in PLAN-007. The connected MCP process was not restarted, so live role-resource and native graph verification remain pending.
