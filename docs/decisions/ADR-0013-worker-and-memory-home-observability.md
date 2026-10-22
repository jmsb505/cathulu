# ADR-0013 — Worker dossiers and memory usage on the home graph

Status: Accepted for M31 implementation  
Date: 2026-10-22

## Context

The persistent-role graph already exposes live state, checkpoints, pending owner actions, worker reports, reconciled worktree facts, QA checks, and a memory-health snapshot. Workers are selectable, but their selection only shows a short summary. Memory-health values are not visible in the home experience, and the projected class counts omit `working` and `aggregate`.

## Decision

1. Keep one read-only home graph; do not add a detail tab or separate memory view.
2. Selecting a Worker expands an in-flow dossier in the graph page. Show its recorded task, checkpoint history, worker report, checks, blockers, changed files, commits, and available reconciled worktree facts. A missing report or checkpoint must be explicit.
3. Label report contents as worker-reported. Keep those distinct from reconciled Git/worktree facts and deterministic QA results; no report text establishes verified success.
4. Extend the existing SQLite-derived `DashboardSnapshot.memoryHealth` with counts for all five memory classes. Show class record counts independently from HOT/WARM/COLD temperature distribution.
5. Define class usage as counts of stored memory rows, including historical/candidate rows. Define temperature footprint as UTF-8 bytes in stored memory bodies. Label these scopes and do not imply they are SQLite database size or artifact size.
6. Add a compact process pulse from already-projected run, delegation, update, attention, and QA data. State the population/window where the source is bounded.
7. Keep the view responsive at the minimum side-by-side width and respect keyboard focus and reduced-motion preferences.
8. Add no new schema, write path, telemetry, dependency, agent control, or dashboard tab.
9. Keep children of the active Project Manager assignment visible; when a project has no active assignment, keep the latest recorded assignment's explicitly parented Workers visible for inspection. Historical Workers do not contribute to current active-agent counts.

## Consequences

- SQLite remains authoritative and the dashboard remains a projection.
- The owner can inspect current worker evidence and memory composition without leaving the home graph.
- Counts include retained historical records, so they describe stored composition rather than only currently injectable memory.
- The most recent Worker nodes remain inspectable after their manager assignment completes; process pulse counts still describe only the active assignment.
- Temperature body-byte totals exclude DB index/metadata and linked evidence artifacts.
- Native visual review is still needed after a local build; source/build checks alone cannot confirm all window-size behavior.

## Alternatives considered

- **Open a dedicated Worker or Memory tab:** rejected because Cathulu is intentionally a one-graph support dashboard.
- **Use a Worker report as proof of work or success:** rejected because it is self-reported; Git and QA evidence remain separate.
- **Show a single combined memory score:** rejected because it would conflate memory class with retrieval temperature and hide the underlying distribution.
- **Add a separate analytics subsystem:** rejected because the current dashboard snapshot already projects the needed authoritative records.
