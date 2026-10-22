# PLAN-005 — Desktop support dashboard and floating monitor

Status: M27 role logic, one-graph dashboard, tests, and matched Windows release validated; stable Codex path now targets the published pair, but the loaded MCP process has not been restarted; native visual/runtime smoke pending
Created: 2026-10-21
Last updated: 2026-10-22

## Objective

Shape Cathulu's read-only desktop surface for use beside Codex: a compact dashboard that makes current work and exceptions easy to scan, plus a small translucent companion window that can stay above other apps and expose a few live status views.

## Motivation

The current dashboard is a long, spacious portfolio page. It is difficult to use in the narrow window beside Codex and spends too much space on introductory copy and repeated panels. Opening its Vite page in a normal browser also calls Tauri `invoke` without a native bridge, producing `Cannot read properties of undefined (reading 'invoke')`. The user's canvas contributes a restrained dark operations-console palette; the Apple Music screenshots inform the separate compact, glass-like window. Cathulu's current real data and support role remain authoritative.

## Current state

- Tauri 2 desktop host and React/TypeScript dashboard use the existing `dashboard_snapshot` command and the shared local SQLite store.
- The dashboard polls every five seconds, remains read-only, and reads existing projects, runs, quality checks, memory health, delegations, and Codex sub-agent hook telemetry.
- The previous page was text-heavy, missed the supplied agent graph, and did not serve the user's side-by-side support workflow.
- The main native window was 1180 × 840 with a 680 px minimum width.
- The dashboard imports and invokes Tauri APIs even when opened outside Tauri.
- All returned quality checks are styled as warnings, including optional inconclusive checks.
- No new model, database schema, telemetry authority, or UI dependency is needed.

## Desired state

- A 500 × 580 side-by-side main window (420 × 460 minimum) that opens directly to the only dashboard view: one project-spanning agent graph. Project and memory scope appear as compact indicators on persistent role nodes; configuration stays in the separate settings surface.
- The graph has one persisted Main Overseer root and one persistent Project Manager role for every registered project. Codex chats are replaceable activations of those logical roles. Each role shows its stable context scope and configured tool profile; temporary assignments, explicitly parented Workers, and observed hooks remain separate evidence. A hook without a recorded parent stays visibly unlinked.
- The graph shows current assignment/project state, not a run carousel. Run records remain available as evidence and never become role identities or graph pages.
- Clear distinct states for active work, blockers, failed required checks, and optional or inconclusive review notes.
- Normal-browser launch displays a useful explanation and the desktop launch command without attempting Tauri IPC.
- A detached, non-owned Tauri window shares the same snapshot read path, starts as a 174 × 58 logical-pixel see-through pill near the lower-right edge, defaults to always-on-top, and expands to a 360 × 268 per-project status board.
- The compact pill shows one color-coded activity signal per registered project. The expanded board shows each project name, its latest recorded run, and a clear Working, Blocked, Failed, Stopped, or Idle state.
- Motion animates only persisted active run/agent edges and indicators; it never implies a percent or phase Cathulu does not observe.
- Transparency uses a genuinely transparent native/document surface and a clipped CSS glass treatment so the rounded panel reveals the desktop outside its corners. Motion runs only for recorded active work; reduced-transparency/motion fallbacks remain available.

## Scope

- Redesign the read-only desktop dashboard's information hierarchy and visual system.
- Guard Tauri-only IPC in a browser preview.
- Add one optional, locally rendered, read-only native monitor window.
- Add only the Tauri permissions required for creating and controlling that window.
- Update architecture, roadmap, ADR, README, and this plan.

## Out of scope

- Changes to memory policy, run semantics, Codex selectors, or specialist providers.
- The additive role schema and read-only `cathulu://roles` resource required by UX-M3; these are tracked in PLAN-008.
- Dashboard mutations, task launching, agent control, or fabricated progress/quality metrics.
- macOS/Linux platform-specific window treatment in this Windows-first milestone.
- A dashboard/settings console for configuring specialist models.
- New UI libraries, remote assets, or dependency installation.

## Source-of-truth impact

- [Architecture](../architecture.md): describes the compact support surface, detached monitor, shared snapshot, and visual fallback.
- [ADR-0008](../decisions/ADR-0008-compact-dashboard-and-floating-monitor.md): records the native-window, information-hierarchy, transparency, and progress-truth decisions.
- [Roadmap](../roadmap.md): tracks the UI work independently of evidence-gated M18/M19.
- [README](../../README.md): documents the support dashboard and how to launch the desktop app.
- This plan records milestone acceptance and validation evidence.

## Existing decisions and constraints

- ADR-0001 and the product definition keep Cathulu local-first and Codex as the execution system.
- ADR-0002 keeps model/effort selectors in a separate explicit settings surface.
- The dashboard is read-only and SQLite remains authoritative; raw hook events are a projection of existing local telemetry, not a new source.
- The user confirmed Windows first and approved a two-window monitor with live snapshot state as a later UX improvement.
- Tauri API reference lookup through Context7 reached its monthly quota; Tauri v2's official JavaScript API and capabilities documentation plus the installed `@tauri-apps/api` declarations were used as the primary fallback.
- Tauri's native window effects paint the full native surface; the Windows screenshot showed that this prevented transparent rounded corners. The monitor now uses transparent native/document backgrounds and a clipped CSS glass layer, with a stronger CSS fallback when blur is unavailable or reduced transparency is requested.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`
- `ui-ux-research`, `redesign-existing-projects`, `impeccable`
- `apple-design`, `emil-design-eng`, `minimalist-ui`
- React/TypeScript already in the repository; Tauri v2 JavaScript APIs and scoped capabilities; no additional package.
- Visual direction: adapt the dark compact observability draft and translucent player reference without copying placeholder topology or inventing telemetry.

## Dependencies

- Existing Tauri `dashboard_snapshot` command, snapshot type, local SQLite data, and React dashboard.
- Existing locked npm and Cargo dependencies.
- No dependency on M18/M19, cross-repository handoffs, remote providers, or new data migrations.

## Risks and unknowns

| Risk or unknown | Mitigation |
| --- | --- |
| WebView2/native transparency differs across Windows builds | Keep the document and native window transparent, use a rounded CSS glass surface, avoid a full-window native material, and verify the corners manually on Windows. |
| Main and monitor windows race or are opened twice | Reuse `WebviewWindow.getByLabel` before creating a window; keep a single stable label. |
| Monitor vanishes when main is minimized | Create a detached top-level webview window with no owner relationship. |
| Animated progress implies unseen percent completion | Only animate an indeterminate run indicator while a run's recorded status is `running`; show no progress percentage. |
| Project count exceeds the expanded monitor height | Keep every registered project in a vertically scrollable, fixed-height board. |
| Transparent panel reduces contrast over a busy desktop | Keep a thin glass tint, clear state colors/text, and a stronger surface only when reduced transparency is requested. |
| The native role graph cannot be visually checked without bringing a window forward | Keep automated role/layout assertions and build validation; defer native smoke until it can be reviewed without disrupting active Codex work. |

## Milestones

### UX-M1 — Compact support dashboard

**Goal:** Make the main window useful beside Codex and fail clearly when launched outside Tauri.

**Subtasks**

- [x] Guard all snapshot IPC behind Tauri runtime detection and render an actionable browser launch notice.
- [x] Replace the full-page intro/portfolio layout with a graph-first activity view and icon-only Repositories/Memory navigation.
- [x] Project existing Codex sub-agent hook events into the read-only snapshot alongside explicit delegations, without fabricating nodes or duplicates.
- [x] Keep project, run, QA, memory, and delegation evidence available through the focused views.
- [x] Distinguish required failures from optional/inconclusive review notes.
- [x] Reduce the native main window to 500 × 580 (420 × 460 minimum) and adapt the graph at narrow widths.
- [x] Add purposeful reduced-motion and high-contrast/transparency fallbacks.

**Affected areas:** `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/index.html`.

**Dependencies:** Existing snapshot contract only.

**Acceptance criteria**

- [x] Browser preview never attempts Tauri IPC and explains how to open the native dashboard.
- [ ] At 420–500 px width the persistent-role graph remains the dominant view without horizontal overflow (native visual smoke pending).
- [x] Graph nodes and links derive only from recorded run, delegation, and Codex sub-agent hook evidence; no cross-repository links are inferred.
- [x] The M1 activity, quality, memory, and delegation views were read-only; UX-M3 later consolidates the main dashboard into one role graph, while detailed evidence remains available through the MCP resources.
- [x] Inconclusive and optional failed checks are labeled as review items, separate from required failures.
- [ ] Keyboard focus and reduced-motion behavior need a native visual/interaction pass.

**Validation**

- [x] `npm run build` in `apps/desktop`.
- [ ] Manual review of native 420–500 px graph, transparent monitor, pin/restore behavior, and settings return path.
- [x] `git diff --check`.

**Documentation updates:** Update README, architecture, roadmap, ADR-0008, and this plan.

**Applicable specialized skills:** `ui-ux-research`, `redesign-existing-projects`, `impeccable`, `apple-design`, `emil-design-eng`, `minimalist-ui`.

**Expected Git checkpoint:** A locally validated UI milestone commit on `codex/plan-005-desktop-monitor`.

### UX-M2 — Floating companion monitor

**Goal:** Add a small detached monitor that remains available while the main dashboard is minimized and can be pinned, expanded, and switched between focused views.

**Subtasks**

- [x] Add monitor route rendering in the same React app and reuse `dashboard_snapshot`.
- [x] Create/reuse the unowned Tauri webview window with a stable label, transparent surface, compact dimensions, and lower-right placement.
- [x] Show a compact state dot for every registered project and use the expanded view for a readable per-project status board instead of shrinking the dashboard graph.
- [x] Add pin toggle, size-aware expand/collapse placement, dashboard restore, close, and drag behavior.
- [x] Add minimal scoped capabilities for window creation, always-on-top, resizing, dragging, focus/restore, and close.
- [x] Keep run indication factual, data local, and all monitor interactions read-only.
- [x] Start at 174 × 58 logical pixels and expand to 360 × 268.

**Affected areas:** `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/src-tauri/capabilities/dashboard.json`, `apps/desktop/src-tauri/capabilities/monitor.json`, and the generated capability schema.

**Dependencies:** UX-M1, Tauri 2 window/webview APIs, existing snapshot command.

**Acceptance criteria**

- [ ] Opening/reusing, corner transparency, minimizing the main window, pinning, expanding/collapsing, restoring, and closing still need native manual smoke.
- [x] Every registered project is represented; state and latest-run details are derived from the same existing snapshot contract without adding telemetry sources.
- [x] The board distinguishes active work, blockers, failed required checks, stopped runs, and projects with no run; animation is limited to recorded active work.
- [x] CSS glass includes unsupported-blur, reduced-transparency, and reduced-motion fallbacks.
- [x] Only the monitor route exposes window sizing/drag controls; the dashboard remains read-only.

**Validation**

- [x] `npm run build` in `apps/desktop`.
- [x] `cargo check -p cathulu-core --locked --offline` for the read-only run-scoped agent projection.
- [x] `cargo check --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml` for Tauri capability/config validation.
- [ ] Manual Windows smoke: create/reuse, verify transparent rounded corners, inspect every project state, pin/unpin, expand/collapse without leaving the display, minimize main, restore main, and close/reopen monitor (the current computer-use surface exposes browser tabs but no native windows).
- [x] `git diff --check`.

**Documentation updates:** Update README, architecture, ADR-0008, and this plan with actual platform effects and validation limits.

**Applicable specialized skills:** `apple-design`, `emil-design-eng`, `impeccable`, `minimalist-ui`, `ui-ux-research`.

**Expected Git checkpoint:** A validated monitor-window commit; integrate only after the native smoke and required CI/checks pass.

### UX-M3 — One logical Overseer graph across projects

**Goal:** Replace the per-run carousel with one stable graph that makes the single logical Main Overseer and all registered project interactions visible together.

**Subtasks**

- [x] Remove the run-history dots and selected-run paging state from the activity view.
- [x] Render one Main Overseer node, then one Project Manager role node for every registered project, using persistent role identity across Codex chats.
- [x] Show the role's configured memory scope/tool profile and recorded activation state from the persistent role registry.
- [x] Show only explicitly parented Workers beneath a PM assignment; keep hook events without a recorded parent visible as unlinked evidence.
- [x] Show current role/assignment state instead of the latest run title; run records remain behind evidence resources and do not become graph pages.
- [x] Remove separate repository, memory, and run views from the main dashboard; keep the role graph as its only dashboard view.
- [x] Keep persistent PM role nodes visible when no active assignment or recent run is recorded, without inventing a chat occupant, Worker, or relationship.
- [x] Project the latest run and its related agent activity for every registered project from the existing SQLite snapshot; keep recent historical runs available to the snapshot without using them as graph pages.
- [x] Implement one graph layout with compact project/agent indicators and no dashboard view tabs.

**Affected areas:** `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, `src/dashboard.rs`, `src/delegation.rs`, architecture docs, ADR-0008, README, and this plan.

**Dependencies:** UX-M1 and UX-M2; current project/run/delegation/hook snapshot contract. No migration or new telemetry source.

**Acceptance criteria**

- [x] The activity view shows exactly one logical Main Overseer node regardless of the number of Codex chats or projects.
- [x] Every registered project appears in the graph at the same time; there is no run-selection carousel or bottom page indicator.
- [x] Project status and agent nodes are derived from that project's latest recorded run and linked evidence only.
- [x] The main dashboard has one graph view; role context/tool indicators sit on persistent nodes, with Codex configuration kept separate.
- [x] Historical runs do not create duplicate Overseer nodes or replace the unified project graph.
- [x] Empty projects and missing role/agent evidence remain explicit and do not imply an unrecorded assignment.
- [x] The read-only snapshot remains authoritative and the floating monitor continues to show the same per-project status picture.

**Validation**

- [x] `npm run build` and `npm test -- --reporter=dot` in `apps/desktop`.
- [x] Full Rust test suite and matched Windows MCP/desktop release build with Tauri `custom-protocol`.
- [x] `cargo check --locked --offline` for the snapshot query changes.
- [x] `git diff --check`.
- [ ] Native visual review at 420–500 px: one root, every project in the same graph, no bottom run dots, and no horizontal overflow. Native review remains pending if the available session cannot expose app windows.
Automated tests were not run, following the current development instruction.

**Documentation updates:** Update README, architecture, ADR-0008, roadmap, and this plan to describe one logical Overseer and a project-spanning graph.

**Applicable specialized skills:** `ui-ux-research`, `ui-ux-pro-max`, `implementation-plan`, `milestone-delivery`.

**Expected Git checkpoint:** A validated UX-M3 checkpoint on `codex/plan-005-desktop-monitor`; integrate only after local validation and required checks pass.

## Final integration validation

- Build the frontend and check Tauri Rust configuration/capabilities.
- Run the desktop app and inspect both main and monitor windows on Windows, including 420–500 px graph layout, browser launch state, failed/inconclusive QA presentation, expanded/collapsed monitor, actual surface transparency, always-on-top, and minimized-main behavior. Browser launch state is verified; native-window inspection remains pending because native windows were not available through this session's computer-use surface.
- Confirm the monitor and dashboard read the same snapshot contract and introduce no database schema or write path.
- Inspect keyboard focus, text contrast, reduced-motion, and the visual fallback when native blur is not available.
- Run `git diff --check`, inspect the final diff, and document outcomes and limitations below.
- Automated tests are not added or run in this plan unless separately requested; compiler/build and documented manual checks are used for this UI milestone.

## Rollback / recovery

Revert the UI/capability/config changes together with the corresponding ADR/architecture/README text. No database migration exists. Removing `dashboard.json` and `monitor.json` restores the prior Tauri capability set. Closing the monitor does not affect the main window or local state.

## Progress

- [x] Audited the existing dashboard, Tauri configuration, source documentation, user canvas, and installed Tauri API declarations.
- [x] Recorded the design and windowing approach in ADR-0008 and the milestones above.
- [x] UX-M1 — graph-first implementation, frontend build, and core projection check.
- [x] UX-M2 — compact monitor and expanded per-project board; frontend build and Tauri capability/configuration checks pass. Native visual smoke is still pending.
- [x] UX-M3 — replace per-run selection with one project-spanning logical Overseer graph; local build and snapshot compilation pass.
- [ ] Native manual visual/interaction smoke for UX-M1, UX-M2, and UX-M3.

## Implementation discoveries / decisions

- The reported browser `invoke` error occurs because `@tauri-apps/api/core` relies on the Tauri IPC runtime; the supported `isTauri()` guard can prevent the call outside the native shell.
- The HTML canvas supplies graph structure and visual direction. Its sample topology, metrics, percentages, and run identifiers are not Cathulu evidence and are not copied into the UI.
- `dashboard_snapshot` includes a bounded projection of recent run-scoped, unlinked sub-agent hook groups. Linked hook groups remain represented by their explicit delegation; the graph draws no cross-project edges.
- The run-history dots were selectors for separate runs, not separate logical Overseers. UX-M3 removes those selectors and represents one Main Overseer across all Codex chat activations, with project activity under that single node.
- UX-M3 shows one Main role and one persistent PM role for every registered project. The snapshot includes each project's newest run plus the ten newest runs overall, active PM assignments and their explicit children, and unlinked hook activity from current project runs. Schema 12 provides durable role IDs and stable PM memory scopes. It does not add chat identity, inferred project dependencies, or unrecorded PM-to-worker edges.
- UX-M3 removed the earlier Repositories/Memory dashboard navigation. The main window has one graph view; project and memory context is represented by compact role indicators, and detailed evidence remains available through local MCP.
- At the time of the initial UX-M3 graph, the tracked checkout supported schema 9 while the installed pair supported schema 11. M27 reconciled the schema-11 source into the tracked checkout, added schema 12, and built the matched Windows release pair under `target/m27-build/release`. The configured pair was not replaced, and no running process was restarted or replaced for this graph change.
- The repository-free M21 probe recorded Cathulu run 4, a Project Manager delegation and Worker delegation, and a Worker report. No hook events surfaced, so the graph can show the persisted role delegations but must not draw an unrecorded Project Manager-to-Worker edge.
- The compact monitor is 174 × 58 logical pixels; its expanded project board is 360 × 268. Every project dot and row is derived from the latest project run plus recorded blockers and mandatory failed checks. Active work is animated; stopped/idle states remain still.
- The monitor document has no global opaque background or minimum-width overflow. The native Acrylic request was removed after the user's screenshot showed a square backing; glass now comes from a rounded, low-opacity CSS surface over the transparent native window. The native corners still need manual Windows review.
- Tauri supports a detached `WebviewWindow`, window transparency/effects, and runtime always-on-top/size controls through scoped capabilities. The monitor will have no owner, so minimizing the main dashboard does not own/hide it.
- The user requested a compact translucent surface. The CSS backdrop blur/tint is a glass approximation and not Apple Liquid Glass; platform rendering still requires review.
- Context7 quota was exhausted; official Tauri v2 docs and installed declarations were consulted.

## Completion evidence

- `npm run build` in `apps/desktop`: PASS after the per-project status-board refinement (TypeScript compile and Vite production build; CSS 67.10 KB, JavaScript 276.05 KB).
- `cargo check -p cathulu-core --locked --offline`: PASS after adding the read-only agent activity projection.
- `cargo check --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml`: PASS after adding scoped permission for keeping the expanded window on-screen; Tauri config and capability compilation completed after retrying the Windows build-script access denial with local build permission.
- `cargo fmt --all --check`: PASS.
- `git diff --check`: PASS.
- UX-M3 `npm run build` in `apps/desktop`: PASS after replacing run selection with the project-spanning graph.
- UX-M3 `cargo check --locked --offline`: PASS after extending the read-only dashboard projection to include each project's latest run and associated delegation/hook/check evidence.
- UX-M3 `git diff --check`: PASS. The full Rust suite and five desktop UI tests pass. Native graph visual review at 420–500 px remains pending to avoid interrupting Codex.
- M27's original persistent-role validation used a separate matching pair at `target/m27-build/release`. The release-refresh follow-up publishes through `target/mcp-current`; the already-running MCP process was not restarted.
- Browser preview at `http://127.0.0.1:1420/`: verified actionable native-launch notice, no Tauri `invoke` error, zero console errors after reload, and favicon `/favicon.svg` returns HTTP 200. Initial missing favicon was fixed.
- Earlier UX work compiled and launched `cargo tauri dev`; M27 validation did not launch any native windows. Native visual/interaction behavior remains a separate review item.
