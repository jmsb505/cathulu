# ADR-0008: Project activity dashboard and floating monitor

Status: Accepted
Date: 2026-10-21

## Context

Cathulu is a local support dashboard used beside Codex. Its purpose is to make current project and agent activity legible at a glance; it does not execute workflows or modify project state. The previous dashboard did not make the agent hierarchy visible, and a detached window that merely shrank the full dashboard was not useful as a desktop companion.

M21 verified a Main → Project Manager → Worker spawn sequence and Cathulu MCP access in one repository-free run. It did not produce hook events, and the PM's memory-search tool availability differed from the Worker's. The UI must therefore distinguish persisted assignments from observed runtime activity and must not invent hierarchy links or claim active work without evidence.

## Decision

- Keep the main dashboard read-only and scoped to one selected project and run at a time. Stable project IDs, not display names, determine membership.
- Render Main as the same logical portfolio-wide coordinator in every project/run view. Chat sessions are representations of that role; do not label or animate each run as a distinct Main agent.
- Render the explicit Main run, Project Manager assignment, and `parentDelegationId` relationships as the activity graph. Unparented workers and unbound hook observations remain visibly separate from that hierarchy.
- A dispatched assignment without linked `started` telemetry is unobserved. Only a linked start event on a running parent run may pulse as active; recorded stop, blocker, and failed-check evidence determines terminal/attention states.
- Keep graph and activity as the primary view. Put repository and memory summaries in secondary views to keep the support surface compact.
- Keep the detached monitor separate from the dashboard: 148×42 logical pixels when compact and 324×204 when expanded. It shows project state signals and a short per-project status row, not a scaled-down dashboard.
- On Windows, use a transparent native window and transparent document with clipped, low-opacity CSS glass. Do not apply native full-window Acrylic. Respect reduced-motion and reduced-transparency preferences.
- Scope Tauri capabilities separately for the dashboard and monitor; grant the monitor only its window controls and the dashboard only the ability to create/focus it.
- Use the existing React/CSS stack for the fixed three-level graph; do not add a graph library.

## Consequences

The dashboard can truthfully show a recorded assignment as unobserved when hooks are absent. It cannot claim that a native agent is active until the stable telemetry binding exists. Visual verification of Windows transparency, corner clipping, resizing, dragging, and always-on-top behavior remains a required native smoke test before M24 is complete.

## References

- [PLAN-006](../plans/PLAN-006-bounded-project-agent-hierarchy.md)
- [ADR-0009: Bounded per-project agent hierarchy](ADR-0009-bounded-project-agent-hierarchy.md)
- [Tauri window API](https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/)
- [Tauri configuration](https://v2.tauri.app/reference/config/)
- [Tauri window customization](https://v2.tauri.app/learn/window-customization/)
