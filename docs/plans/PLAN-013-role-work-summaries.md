# PLAN-013 — Selectable roles and concise work details

Status: Implementation and build complete; native visual review pending

Created: 2026-10-22

Last updated: 2026-10-22

## Objective

Make every persistent role in Cathulu's single activity graph selectable, show the work associated with that role immediately, and reduce the first view to a short, readable status summary.

## Motivation

The current graph only opens details for assigned Project Managers and Workers. The Main Overseer and idle Project Manager nodes do not respond. The Worker detail view expands progress, a full report, checks, history, Git state, and QA evidence together, obscuring the answer to “what is happening?”. The W/C/T marks are unexplained and mostly describe configuration rather than observable work.

## Current state

- The dashboard has one project-spanning graph and stores persistent Main and Project Manager roles.
- Selection is keyed only by delegation ID; a Project Manager node without an active assignment is not operable, and the Main node is not a control.
- Worker nodes display only “W”. Main and Project Manager nodes display “C” and “T” indicators.
- Selecting a Worker renders all report, checkpoint, worktree, and QA fields at once.
- The existing read-only snapshot contains role assignments, run summaries, checkpoints, reports, pending owner actions, and reconciled Git/QA evidence.

## Desired state

- Main, each Project Manager, and every recorded Worker is a clearly labeled, keyboard-operable graph control.
- Selecting any node opens a matching detail panel and scrolls it into view. Main shows coordination and attention; a Project Manager shows its current assignment or latest project activity; a Worker shows its task.
- The first view answers who, what, current state, latest useful progress, and whether the owner needs to act.
- Raw report, check list, checkpoint history, and Git/QA facts stay available behind one clear “Full evidence” disclosure.
- Remove unexplained W/C/T marks. Preserve the existing visual status language, factual boundaries, and one-graph layout.

## Scope

- Update `apps/desktop/src/App.tsx` and `apps/desktop/src/style.css`.
- Add the product decision and milestone documentation.
- Reuse the existing `DashboardSnapshot`; do not change database schema, MCP, runtime orchestration, or telemetry.

## Out of scope

- New tabs, windows, dashboard navigation, memory views, model controls, or agent actions.
- Opening, restarting, or focusing the dashboard or MCP server.
- Changing what counts as verified success or treating self-reported work as QA evidence.

## Source-of-truth impact

- SQLite-backed snapshot data remains authoritative.
- Detail summaries are projections of existing assignment, checkpoint, run, report, attention, Git, and QA records.
- Reports remain labeled as reported evidence; Git/worktree and Cathulu QA remain separate.
- No schema migration, write path, dependency, or service behavior changes.

## Existing decisions and constraints

- [ADR-0011](../decisions/ADR-0011-persistent-logical-agent-roles.md): one logical Main Overseer and one persistent Project Manager role per project; chats and assignments are replaceable.
- [ADR-0012](../decisions/ADR-0012-live-agent-progress-and-attention.md): show only recorded signals and expose owner attention.
- [ADR-0013](../decisions/ADR-0013-worker-and-memory-home-observability.md): preserve a single graph, report provenance, and distinguish report from Git/QA evidence.
- Main Overseer coordinates work; project agents execute repository tasks.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`, `ui-ux-research`, `ui-ux-pro-max`, and `impeccable`.
- Existing React and CSS components, native HTML buttons and `<details>`, the dashboard snapshot, and `scripts/build-current.ps1`.
- No design or runtime dependency is needed.

## Dependencies

- Current logical-role graph and SQLite-derived dashboard snapshot.
- Frontend build and matched Windows app/MCP build script.

## Risks and unknowns

- A role can be present without a current task; its detail must clearly say there is no active assignment rather than implying work.
- A report is self-reported and may differ from recorded verification; preserve those labels and separate evidence.
- The app will not be opened during this change. Native visual review remains a separate step to avoid interrupting the user's Codex session.

## Milestones

### M33 — Selectable roles and concise work details

**Goal:** Make graph selection reliably reveal the corresponding role's current recorded work in a concise first view.

**Subtasks**

- [x] Make Main, Project Manager, and Worker nodes selectable and expose the expanded state accessibly.
- [x] Replace delegation-only selection with role-aware selection and matching Main/project/worker summaries.
- [x] Replace W/C/T shorthand with clear Worker labels and remove non-actionable scope/profile marks.
- [x] Move detailed reports, checks, checkpoint history, and Git/QA evidence behind one disclosure.
- [x] Preserve keyboard focus, reduced-motion behavior, factual status labels, and narrow layouts.
- [x] Update this plan, ADR-0015, and the roadmap with actual completion evidence.

**Affected areas:** `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, `docs/decisions/ADR-0015-role-work-summaries.md`, `docs/roadmap.md`, and this plan.

**Dependencies:** Existing read-only role graph and snapshot fields.

**Acceptance criteria**

- [x] Main, any Project Manager (including idle), and a Worker share a role-aware selection path that opens the matching details and scrolls/focuses the panel.
- [x] The first view presents a plain-language state, task or role context, one current summary, and recorded owner attention.
- [x] Worker graph controls say “Worker” with an index; W/C/T marks are absent.
- [x] Full report, checkpoints, changed paths, and Git/QA data remain accessible through one disclosure and retain their provenance labels.
- [x] Missing data is explicit; no activity, progress, or verification is fabricated.

**Validation**

- [x] `npm run build` from `apps/desktop` (TypeScript and Vite production build passed).
- [x] `git diff --check` and source review of each node type, keyboard state, progressive disclosure, reduced motion, and narrow-width behavior.
- [x] Build and publish the matched Windows app/MCP pair with `scripts/build-current.ps1`, without launching either process.
- [ ] Native pointer/keyboard review of Main, idle and assigned Project Managers, Workers, disclosure, narrow widths, and reduced motion remains pending; no app was opened.

**Documentation updates:** Add ADR-0015; update the Phase 16 UX item in `docs/roadmap.md`; record actual validation and limitations here.

**Applicable specialized skills:** `ui-ux-research`, `ui-ux-pro-max`, `impeccable`, `implementation-plan`, and `milestone-delivery`.

**Expected Git checkpoint:** A validated M33 change. The current sandbox grants read-only access to `.git`, so branch/commit/push operations are unavailable; do not advance `main` without the required validation and Git access.

## Final integration validation

- Build the frontend and the matched Windows desktop/MCP pair.
- Confirm `git diff --check` and inspect the final source and documentation diff.
- Do not launch the app or MCP during delivery. Mark native visual/runtime smoke pending.

## Rollback / recovery

Revert the UI/documentation changes if the frontend or matched build fails. No persistent data, schema, or service behavior changes.

## Progress

- [x] Confirmed clean `main` at `757fd3d`; existing snapshot and graph already contain the data required for the refinement.
- [x] Identified that selection only targets delegation IDs and that the dossier renders all evidence by default.
- [x] Implement M33 and publish the matched Windows build.
- [ ] Native visual review remains pending.

## Implementation discoveries / decisions

- The graph needs a role-aware selection key so persistent role nodes remain operable even when no current delegation exists.
- Keep Main's view focused on coordination and attention; Main does not claim repository implementation work.
- Keep the full evidence in the same graph page, behind native progressive disclosure; no extra tab or window.

## Completion evidence

Frontend `npm run build` passed on 2026-10-22. The later stacked UI build, including this milestone, published source fingerprint `68FBA5F7D8C3B3DC45B7327033C9943692A79E057BBFFF2AFB54B54E51AFC45D` under `target/mcp-versions/68FBA5F7D8C3B3DC45B73270`; desktop SHA-256 is `030F484191C073063EA8ABA13BCFF04D9F5A51603C317FFEC5C16264223059C5`, MCP SHA-256 is `5997BDD0C7EDCE1A36AE2C7FD53F9933FFC440A2E07C88D0C10612F1C8A8C4EA`. `git diff --check` passed. The app and MCP were not launched or restarted. Automated tests and native interaction review were not run; native review remains pending. Changes remain uncommitted on the local `main` working tree because `.git` is read-only in this session.
