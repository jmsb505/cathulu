# PLAN-010 — Responsive side-by-side support dashboard

Status: In progress — implementation and branch build passed; native visual smoke and final Git progression remain pending
Created: 2026-10-22
Last updated: 2026-10-22

## Objective

Make Cathulu’s main support dashboard comfortable beside Codex and readable when resized from its normal side-by-side width to a maximized desktop view, while retaining one project-spanning agent graph.

## Motivation

The current main window opens at 500 × 580 logical pixels and permits shrinking to 420 × 460. Important graph labels are 6–9 px, and the graph has a 590 px maximum height. The result is too small for the user’s primary workflow and wastes space when maximized.

## Current state

- Tauri opens the main window at 500 × 580 logical pixels (`minWidth` 420, `minHeight` 460).
- The graph, attention panel, and checkpoints are stacked at every width.
- Several role labels and activity summaries use micro text sizing.
- The graph stage stops growing after 590 px, even when the window has more room.
- A separate compact floating monitor already exists and is not the main dashboard.

## Desired state

- Open the main dashboard at a practical side-by-side size (820 × 760 logical pixels), with a 560 × 540 minimum that preserves graph readability.
- Let the graph use available height and width rather than capping it at a small fixed height.
- Keep attention and recent activity adjacent to the graph on wide/fullscreen layouts; stack them cleanly below the graph at narrower widths.
- Use readable text and touch/keyboard targets at the supported minimum width, with no horizontal page overflow.
- Keep a single vertical scroll surface when the window height cannot show all sections; avoid nested scroll areas for attention and checkpoints.
- Preserve the same read-only data, one logical Main Overseer graph, activity states, and floating monitor behavior.

## Scope

- Change only the main Tauri window’s default/minimum geometry.
- Reflow the existing graph, attention, and checkpoint components for wide and narrow layouts.
- Improve graph-node, count, attention, and checkpoint sizing for normal desktop viewing.
- Update the existing dashboard decision, roadmap, README, and this plan with current behavior and evidence.

## Out of scope

- Changes to the compact floating monitor, its transparency, size, pinning, or lifecycle.
- New dashboard views/tabs, data sources, metrics, schema, dependencies, or orchestration behavior.
- Changes to Cathulu startup/focus behavior or restarting the app/MCP after build.

## Source-of-truth impact

- [ADR-0008](../decisions/ADR-0008-compact-dashboard-and-floating-monitor.md): clarify the main dashboard’s side-by-side/default and responsive sizing policy.
- [Roadmap](../roadmap.md): record M29 and its validation state.
- [README](../../README.md): state the supported main-window layout and sizes.
- This plan: record decisions, validation, build artifact, and remaining runtime limitations.

## Existing decisions and constraints

- ADR-0008 keeps one evidence-backed project graph, local/read-only state, and a separate compact monitor.
- The UI must not fabricate agent identity, relationships, or progress.
- No additional frontend dependency is justified; existing React and CSS can implement the responsive layout.
- Tauri window width/height constraints are expressed in logical pixels.
- Preserve reduced-motion support, keyboard focus visibility, and accessible text labels.
- Do not launch/focus Cathulu or restart Codex’s MCP as part of building the pair.

## Supporting skills / tools

- `ui-ux-research`, `redesign-existing-projects`, `ui-ux-pro-max`, `implementation-plan`, and `milestone-delivery`.
- Existing React, CSS, Tauri v2 config, and `scripts/build-current.ps1`.
- Official Tauri v2 configuration reference was checked after Context7 returned its monthly-quota error.

## Dependencies

- M27 single graph and M28 live activity/attention presentation are already on `main`.
- Existing local build dependencies are installed; no dependency changes are planned.

## Risks and unknowns

| Risk | Mitigation |
| --- | --- |
| Wider default window competes with Codex on smaller displays | Use an 820 px default and 560 px minimum; keep the window resizable and avoid forced maximization. |
| A narrow graph row clips project or worker indicators | Reflow node/worker clusters at narrow widths and retain accessible names/tooltips. |
| Large screens make the graph overly stretched | Use a bounded activity rail and a centered dashboard content width. |
| CSS build success cannot prove native WebView2 visual behavior | Record native resize/visual review as pending; do not claim runtime validation. |

## Milestones

### M29 — Responsive side-by-side dashboard

Goal: Make the one-graph dashboard readable and useful at its side-by-side default, at the reasonable minimum width, and when maximized.

Subtasks:

- [x] Set the main window to 820 × 760 logical pixels with 560 × 540 minimum dimensions.
- [x] Place the graph beside attention/activity on wide layouts and stack sections on narrow layouts.
- [x] Remove the graph’s small fixed maximum height and improve legibility of graph and activity text.
- [x] Ensure constrained-height layouts use one page scroll and reflow project rows at narrow widths.
- [x] Update ADR-0008, roadmap, README, and this plan.

Affected areas: `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, `docs/decisions/ADR-0008-compact-dashboard-and-floating-monitor.md`, `docs/roadmap.md`, and `README.md`.

Dependencies: M27/M28 dashboard and current Windows build script.

Acceptance criteria:

- [x] Main dashboard starts at 820 × 760 and cannot be resized below 560 × 540 logical pixels.
- [x] At wide/fullscreen widths, the graph and activity/attention rail are arranged together; the graph is no longer capped at a short fixed height.
- [x] At the minimum supported width, the layout stacks and moves worker signals below each project node; page minimum width is removed so zoom can reflow instead of forcing horizontal overflow.
- [x] At short heights, dashboard content remains reachable through the page scroll; attention/checkpoint lists no longer create nested scroll panes.
- [x] The single graph and detached monitor retain their existing behavior and data authority; monitor implementation and snapshot/data paths are unchanged.

Validation:

- [x] `git diff --check`.
- [x] `npm run build` in `apps/desktop` (TypeScript project build and Vite production build passed).
- [x] `./scripts/build-current.ps1` created and published a matched current Windows MCP/desktop pair.
- [x] Review responsive DOM/CSS structure at the 560 px minimum, 820 px default, and wide/fullscreen breakpoints; native WebView2 visual review remains pending because the app/MCP were not launched or restarted.
- [x] Do not run automated test suites unless the user requests them.

Documentation updates:

- [x] Update ADR-0008, roadmap, README, and this plan.

Applicable specialized skills: `ui-ux-research`, `redesign-existing-projects`, `ui-ux-pro-max`, `implementation-plan`, `milestone-delivery`.

Expected Git checkpoint: `codex/m29-responsive-dashboard`, then a checked PR/merge to `main` if required checks allow.

## Final integration validation

Run whitespace validation, the desktop TypeScript/Vite production build, and the canonical Windows pair build. Automated tests are intentionally omitted under the current project instruction. Verify the pair’s source revision/fingerprint and hashes; do not launch or restart the application or MCP server.

## Rollback / recovery

Revert the M29 commit to restore the prior window geometry and stacked layout. This milestone changes no database state or migrations.

## Progress

- [x] Review current Tauri window configuration, graph markup/styles, README, ADR-0008, roadmap, and M28 plan evidence.
- [x] Confirm Tauri v2 window dimensions/minimum constraints use logical pixels from the official configuration reference (Context7 unavailable due quota).
- [x] Implement responsive layout and publish the branch build.
- [ ] Complete native visual review and final Git progression.

## Implementation discoveries / decisions

- The main window’s 500 × 580 default and micro-sized graph type are the root of the “tiny dashboard” feel; the separate floating monitor is correctly a different surface and remains unchanged.
- Use a CSS-first responsive composition with a fixed-width readable activity rail at wide sizes and a single-column stack below that threshold. No UI library or schema work is needed.
- Tauri/React/CSS builds passed. A headless Edge render was attempted for screenshot review but could not start because the restricted environment rejected Edge's Crashpad data-directory creation; no app window or MCP was opened/restarted. CSS and JSX were reviewed directly, so the native display should still be visually confirmed after the updated binary is loaded.

## Completion evidence

- Frontend build: `npm run build` passed on 2026-10-22 (`tsc -b` and Vite 8.3.4).
- Matched Windows build: `scripts/build-current.ps1` passed on 2026-10-22 and published `target/mcp-versions/084903F389BF9579D5AC1EC2` for source fingerprint `084903F389BF9579D5AC1EC27CB2140B75C0A798AEFE6C1558FF24003183F43F`. Branch build manifest records `workingTreeDirty: true`, base source revision `c485a141d2fd46c14885993f39ee65076eadfc13`, MCP SHA-256 `63B95FBE602A46746B83EF425847AC72E78E998B1BAE968F291A32AB4CD03AE5`, and desktop SHA-256 `FC2ACC38D63720C5A88C31FCCCB27F76FEFB6C6FA2430BD204C6F9609B5D389E`.
- `git diff --check` and `tauri.conf.json` JSON parsing passed.
- Automated tests were not run. Native visual/resize review and final main-branch build remain pending.
