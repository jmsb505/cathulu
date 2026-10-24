# ADR-0015 — Selectable roles and concise work summaries

- Status: Accepted
- Date: 2026-10-22
- Decision owner: Cathulu project owner

## Context

The single persistent-role graph is the primary view of Cathulu's activity. Its current selection state is keyed only by delegation ID, so the Main Overseer and a Project Manager without an active assignment cannot open details. Worker details display the entire report, checkpoint history, and Git/QA evidence at once. The graph also uses W/C/T marks that do not explain current work and are not useful controls.

## Decision

1. Main, every persistent Project Manager, and each recorded Worker are native, keyboard-operable graph controls. Selecting one opens matching information within the existing graph page.
2. Main's details summarize coordination and recorded attention. They do not claim Main performed repository implementation.
3. A Project Manager opens its active assignment when one exists; otherwise it shows the latest available project activity and clearly states when no active assignment exists. A Worker opens its task-scoped assignment.
4. The first view is a concise status summary: role/project, current state, task or project context, latest useful update, and owner action if recorded.
5. Detailed report content, check lists, checkpoint history, and Git/QA evidence remain available behind one explicit “Full evidence” disclosure. Their source and verification status remain clear.
6. Replace the unexplained Worker “W” mark with a visible Worker label and index. Remove C/T scope/profile marks from graph nodes; they are not activity indicators and do not provide a current, actionable view of native Codex context or tool availability.
7. Keep the graph read-only and SQLite-backed. Add no tabs, schema, event, agent control, or telemetry.

## Consequences

- A persistent role stays selectable when its temporary assignment is absent.
- Users see concise current work first while retaining access to auditable source records.
- Snapshot facts, self-reported worker content, and independent Git/QA evidence remain distinct.
- Native runtime and visual behavior still require a manual app review after the matched local build.

## Alternatives considered

- **Only make active assignments selectable:** rejected because the persistent role graph would still contain nodes that appear clickable or relevant but have no details.
- **Keep every evidence field expanded:** rejected because it hides the current task and status in repeated low-level text.
- **Keep W/C/T with tooltips:** rejected because configuration hints do not communicate current work, and actual native chat/tool availability is not observable.
- **Open a separate page or window:** rejected because the graph is the one-page activity surface and details should stay beside it.

## Reconsider if

- Role details require interaction beyond read-only inspection.
- The dashboard gains a trustworthy native Codex chat identity or active tool-availability source that merits a meaningful, actionable indicator.
- Evidence volume makes one disclosure insufficient for the side-by-side dashboard layout.
