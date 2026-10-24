# ADR-0008: Compact support dashboard and floating monitor

Status: Accepted, narrowly amended by [ADR-0018](ADR-0018-local-owner-action-decisions.md)
Date: 2026-10-21
Updated: 2026-10-23
Deciders: Project owner and Cathulu maintainers

## Context

Cathulu's desktop surface is a support dashboard used beside Codex. The previous page was text-heavy and did not make the agent graph from the supplied observability canvas the primary view. The user also needs a very small, see-through status surface while the main application is minimized. The canvas supplies graph structure and visual direction, not fabricated data. The dashboard and monitor remain local and observational; the only later exception is the narrowly scoped, explicit Owner-action decision control defined in ADR-0018. Codex remains the place where task execution occurs.

## Decision

1. Treat the desktop dashboard as an **operate/support** surface and open on one project-spanning graph with one persistent logical Main Overseer root and one persistent Project Manager role per registered project. A Codex chat is a replaceable activation that may occupy a role; chats do not create separate role identities. Show the role's recorded assignment/project state, context scope, configured tool profile, explicitly parented Workers, and observed hook evidence. Use a delegation in place of its linked hook event to avoid duplicate nodes. Do not infer chat identity, parent-child scope links, or cross-repository relationships. Historical runs remain evidence, not graph pages. The main dashboard has one graph view and no navigation tabs for runs, repositories, or memory; those signals live on role/project nodes or in evidence resources. The Codex configuration surface stays separate.
2. Keep the dashboard on the existing `dashboard_snapshot` Tauri command and shared SQLite authority. Add only read projections of already-recorded data, including run-linked Codex hook events. Do not add a new telemetry authority, model call, or second telemetry store. The only allowed dashboard write is the explicit Owner approval/decline record defined by ADR-0018.
3. Detect the native Tauri runtime before invoking IPC. A browser preview without the native bridge presents instructions to start `cargo tauri dev` rather than throwing a runtime exception or fabricating data.
4. Add one optional top-level Tauri webview window for the monitor. It is unowned, read-only, and uses the same snapshot contract. It opens at 174 × 58 logical pixels near the lower-right edge of the current display (with a centered fallback), expands to a 360 × 268 per-project status board, and includes an always-on-top toggle, a dashboard restore action, and close. Reopening reuses the stable window label. The compact view shows one status dot for each registered project; the expanded view gives each project a readable latest-run summary and Working, Blocked, Failed, Stopped, or Idle state.
5. Keep the native window and HTML document transparent. Render glass with a rounded, low-opacity CSS surface and blur so the desktop shows through the corners. Do not apply a native full-window Acrylic material: the user's Windows screenshot showed it painting a square backing around the rounded panel. This CSS glass approximation is not Apple's Liquid Glass. Use a stronger surface only when blur is unavailable or reduced transparency is requested.
6. Animate project indicators only for recorded runs whose status is `running`. Do not show synthetic percentage progress or model-generated status.
7. Classify mandatory failed checks as blocking. Keep optional failures and inconclusive/not-run checks as review notes rather than presenting every returned row as a failed test.
8. Add only scoped Tauri permissions required for the monitor and the Owner-action decision command defined by ADR-0018. Do not expose generic database, dispatch, or project-mutation commands.
9. Treat the main dashboard as a resizable side-by-side workspace, separate from the tiny floating monitor. Open it at 820 × 760 logical pixels, enforce a 560 × 540 minimum, let the graph grow with the available area and number of registered project roles, and place attention/activity beside the graph when there is room. Stack those areas at narrower widths and allow a single page scroll when the window height is constrained. Keep all Project Manager roles visible without an inner graph scrollbar and preserve readable labels at narrow widths rather than shrinking text to fit.

## Alternatives considered

- **Keep the current long page:** rejected because it does not serve the narrow side-by-side use case.
- **Copy the canvas's sample data or full-screen density:** rejected because its sample nodes and metrics are illustrative. Keep its graph-led hierarchy while binding nodes and edges only to stored run/delegation/hook facts.
- **Use an in-page floating card or owned child window:** rejected because it would disappear with or remain constrained by the main window, rather than function as a separate compact desktop companion.
- **Add a frontend window/UI library:** rejected because existing React and Tauri APIs cover the interaction without new dependency surface.
- **Apply native Acrylic across the monitor window:** rejected after visual feedback showed square opaque corners around the CSS panel. Use a clipped CSS material on a transparent document and keep the native window transparent.
- **Shrink the main agent graph into the expanded monitor:** rejected because nodes and controls became unreadable. The companion is a per-project status board; the main window retains the detailed agent graph.

## Consequences

- The 820 × 760 main window is sized for use beside Codex and may be resized down to 560 × 540 logical pixels. The graph remains the primary surface, grows in large windows and with project count, and shares wide layouts with a readable activity/attention rail; narrower layouts stack those regions. All persistent project roles remain in the page flow, with one dashboard-level scroll when the graph or window is taller than the viewport. There is no compact mode for the main dashboard; the 174 × 58 floating monitor serves that purpose.
- The activity graph is project-oriented, not chat- or run-oriented. It never creates extra Overseer nodes for additional chats or run history.
- Raw sub-agent hook activity appears in the graph even when no Cathulu delegation was explicitly linked. Linked hook events appear once through their delegation. Existing data remains the only source for graph topology.
- The monitor remains lightweight and can be toggled independently. Both windows poll the existing snapshot. The monitor remains read-only; the main dashboard adds only the explicit Owner decision record described in ADR-0018. Project state is reduced from each project's latest recorded run plus blockers and mandatory failed checks.
- The monitor requires a narrow capability file because Tauri's default permissions exclude webview-window creation and certain runtime window operations.
- Native transparency and CSS backdrop rendering vary by WebView2/Windows build. Automated build checks cannot establish that the corners reveal the desktop or that always-on-top behaves correctly; a Windows manual smoke is required before this milestone is complete.
- The Vite page remains useful for frontend work but displays an explanatory state instead of live data because it has no Tauri IPC bridge.

## References

- [Tauri v2 WebviewWindow API](https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/)
- [Tauri v2 Window API](https://v2.tauri.app/reference/javascript/api/namespacewindow/)
- [Tauri v2 capabilities](https://v2.tauri.app/security/capabilities/)
- [PLAN-005: Desktop support dashboard and floating monitor](../plans/PLAN-005-desktop-support-dashboard.md)
- [PLAN-010: Responsive side-by-side support dashboard](../plans/PLAN-010-responsive-support-dashboard.md)
