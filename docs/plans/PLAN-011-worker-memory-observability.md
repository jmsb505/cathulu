# PLAN-011 — Worker work dossiers and home observability

Status: M31 merged to `main` in PR #29; implementation and matched current build passed; native visual review remains pending
Created: 2026-10-22  
Last updated: 2026-10-22

## Objective

Make the single Cathulu home graph explain what a selected Worker has done and provide a compact, visual view of current process activity and memory usage.

## Motivation

The current graph lets an owner select a Worker but exposes only a one-line checkpoint. It also has agent-status counts, while memory health is present in the dashboard snapshot but hidden from the home view. This leaves work evidence and memory composition difficult to inspect.

## Current state

- The dashboard is a read-only projection of the local SQLite store and shows one project-spanning persistent-role graph.
- Worker nodes are selectable. Selection opens a small overlay with the latest checkpoint, state, and report review status.
- `DashboardSnapshot.memoryHealth` already contains HOT/WARM/COLD body-byte totals and some class counts; `working` and `aggregate` counts are not projected.
- Worker reports, progress updates, checks, changed paths, and reconciled worktree facts are already stored and projected through `DashboardSnapshot.delegations` and `agentUpdates`.

## Desired state

- Selecting a Worker expands an in-flow dossier with its objective, latest and recent checkpoints, worker-reported outcome, changed files, report checks/blockers, and reconciled worktree verification when present.
- Explicitly parented Workers remain visible for the active assignment or the latest completed assignment, so their work can still be inspected after the manager finishes.
- The dossier clearly distinguishes self-reported work from observed Git/worktree facts and QA evidence. Missing evidence is shown as missing, never inferred.
- The home graph includes a compact process pulse and memory usage visualizations: all five memory-class record counts and HOT/WARM/COLD stored-body bytes. Class and temperature remain independent dimensions.
- The dashboard stays read-only and responsive, with no new tabs or backend memory schema changes.

## Scope

- Extend the dashboard read projection with the missing working/aggregate class counts.
- Add accessible selection semantics and a responsive Worker work dossier.
- Add compact process and memory visualizations to the single home graph.
- Update the plan, this decision record, and the roadmap.

## Out of scope

- Agent control, memory mutation, memory lifecycle policy changes, new telemetry ingestion, new DB tables/migrations, dashboard navigation/tabs, and floating-monitor redesign.
- Claiming that an agent is active or that a report is verified solely from self-reported text.

## Source-of-truth impact

- SQLite remains authoritative for memory records, agent updates, reports, Git reconciliation, and QA checks.
- `DashboardSnapshot` is a read-only projection. Class counts include stored memory rows; temperature byte totals measure UTF-8 body bytes, not SQLite or artifact-store size.
- Worker dossiers present report content as self-reported unless the separate worktree/QA evidence confirms it.

## Existing decisions and constraints

- [ADR-0008](../decisions/ADR-0008-compact-dashboard-and-floating-monitor.md): support dashboard stays readable beside Codex; compact monitor remains separate.
- [ADR-0011](../decisions/ADR-0011-persistent-logical-agent-roles.md): one persistent-role graph, independent from temporary assignments.
- [ADR-0012](../decisions/ADR-0012-live-agent-progress-and-attention.md): updates are self-reported, evidence sources stay distinct, one graph, no run tabs.
- [PLAN-009](PLAN-009-live-agent-observability.md) and [PLAN-010](PLAN-010-responsive-support-dashboard.md) define current progress/attention and responsive behavior.
- No new memory schema or migrations are in scope.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`
- `ui-ux-research`, `ui-ux-pro-max`
- Existing React, TypeScript, CSS, Rust/SQLite, Vite, and Tauri stack; no new dependency.

## Dependencies

- Existing dashboard report/update/worktree projections and the current memory-health snapshot.
- Matched desktop/MCP build script for the Windows release pair.

## Risks and unknowns

- Some workers have no report, checkpoint, or reconciled worktree; the dossier needs clear empty states.
- Class counts include candidate, superseded, inactive, and historical records unless explicitly filtered. The UI must say “stored records” to avoid suggesting all records are current/retrievable.
- Stored body bytes do not include SQLite indexes, metadata, or artifact bytes. The chart must label them as memory-body bytes.
- A selected dossier increases vertical content; the page, not the graph canvas, must handle scrolling without clipping focused controls.

## Milestones

### M1 — Worker dossier and process/memory overview

Goal: Make active worker work and system memory composition legible from the one home graph.

Subtasks:

- [x] Add missing memory-class counts to the read-only dashboard snapshot.
- [x] Replace the one-line selection overlay with a responsive, in-flow work dossier.
- [x] Add process-pulse and memory-class/temperature visualizations.
- [x] Preserve keyboard interaction, visible focus, and reduced-motion behavior.
- [x] Update source-of-truth documentation; record the matched build evidence after compilation.

Affected areas:

- `src/dashboard.rs`
- `apps/desktop/src/App.tsx`
- `apps/desktop/src/style.css`
- `docs/plans/PLAN-011-worker-memory-observability.md`
- `docs/decisions/ADR-0013-worker-and-memory-home-observability.md`
- `docs/ROADMAP.md`

Dependencies: None; builds on PLAN-009/010.

Acceptance criteria:

- [x] Selecting a Worker opens and closes its dossier and exposes the task, recorded updates, report details, and available checks/worktree facts.
- [x] Unavailable report/checkpoint/worktree data is explicitly shown as unrecorded.
- [x] Class and temperature remain independent; the UI shows counts for working, canonical, episodic, evidence, and aggregate records plus HOT/WARM/COLD body-byte distribution.
- [x] Process metrics are derived from the current dashboard snapshot and are labeled to identify their scope.
- [x] No new dashboard view, schema migration, write path, or dependency is introduced.
- [x] Responsive source uses 560px-window breakpoints, min-width-safe grids, and wrapping; native visual review is pending.
- [x] Frontend production build passes.
- [x] Matched current Windows desktop/MCP release build passes.

Validation:

- [x] `npm run build` in `apps/desktop`.
- [x] `git diff --check`.
- [x] `scripts/build-current.ps1` completes and publishes the matched current pair.
- [x] Inspect the responsive CSS at minimum and wide widths; native visual review remains pending because validation did not launch the app.
- [x] No test suites were added or run.

Documentation updates:

- [x] Add ADR-0013 and update Phase 15 in `docs/ROADMAP.md`.
- [x] Record final checks, limitations, and build fingerprint here.

Applicable specialized skills: `ui-ux-research`, `ui-ux-pro-max`, `milestone-delivery`.

Expected Git checkpoint: Completed in [PR #29](https://github.com/jmsb505/cathulu/pull/29), merged to `main` at `9254a1fd14c26ad26e7b513108b4774a3aea6660`.

## Final integration validation

- [x] Frontend production build, matching Windows desktop/MCP build, Rust formatting, and clean diff checks passed.
- [x] PR #29 was mergeable, GitHub returned no configured status checks or workflow runs, and the PR merged successfully.
- [x] Local `main` fast-forwarded to `9254a1fd14c26ad26e7b513108b4774a3aea6660`.
- [ ] Native visual review remains pending. Validation did not launch the app or restart MCP.

## Rollback / recovery

- Revert the merged M31 commit if the projection or UI introduces build/runtime regressions. No persistent user data or schema migration is changed.
- Keep the current stable release pair available until the newly built pair is validated and published by the canonical build script.

## Progress

- [x] Reviewed existing dashboard snapshot, delegation/report projections, memory health, recent UX decisions, and current plan.
- [x] Created milestone branch from clean `main` at `c1ac106`.
- [x] Implement M1 source changes.
- [x] Publish matched current Windows build from the milestone source.
- [x] Merge M31 to `main` through PR #29 and fast-forward the local checkout.

## Implementation discoveries / decisions

- Existing `memoryHealth` exposes temperature bytes and three of five memory-class counts. This milestone extends the same SQLite-derived snapshot instead of introducing a parallel memory-health query or store.
- Worker report payload and worktree reconciliation are already available in the graph snapshot; the dossier can remain read-only and preserve evidence provenance.
- When no Project Manager assignment is active, the graph can retain the latest explicitly parented Workers; historical worker states remain excluded from active-agent process counts.

## Completion evidence

- Frontend production build passed on 2026-10-22 (`tsc -b && vite build`).
- `cargo fmt --all -- --check` and `git diff --check` passed.
- `scripts/build-current.ps1` passed and published the matched pair at `target/mcp-versions/226879C1CCB56BE45728EB1E`; stable MCP path: `target/mcp-current/cathulu-mcp.exe`.
- SHA-256: MCP `5DFF139590BC26350E1DC83DB2D18E095E78735A6B940A48969E4224E4A1179C`; desktop `3C226D4464C608DF75FF2E23C87FB5F2F1802C49D6552A8F45510D382B4F2D74`.
- No automated test suites were run. Native app visual review is pending; build validation did not launch the desktop or restart MCP.
- PR #29 was merged at `9254a1fd14c26ad26e7b513108b4774a3aea6660`; the local checkout is synchronized to that `main` commit.
