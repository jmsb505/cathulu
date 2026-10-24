# PLAN-012 — Compact project companion

Status: M32 merged in PR #31; M34 implemented and matched build published; native visual review pending
Created: 2026-10-22  
Last updated: 2026-10-22

## Objective

Make the floating Cathulu companion easy to read at a glance and useful when expanded, without turning it into a shrunken dashboard.

## Motivation

The existing compact monitor represents each project with a tiny dot, but the dots are crowded, reorder as state changes, and do not open project-specific information. Its expanded rows are not interactive. The supplied Codex Mini reference suggests a small capsule that stays visually quiet until the owner asks for more detail.

## Current state

- The detached Tauri monitor opens at 174 × 58 logical pixels and has pin, expand, close, and drag controls.
- Its bubbles are 6 px and can wrap within a 48 px strip; their order follows state priority, so a color change can also move a project.
- The expanded companion now uses a 376 × 360 board with selectable project rows, inline evidence, and a workflow/memory summary.
- The read-only `DashboardSnapshot` already contains each project's latest run, agent checkpoints, pending owner actions, required check failures, and active-agent signals.

## Desired state

- Keep a small transparent pill, with slightly larger, individually accessible project bubbles in stable project order.
- Color bubbles from recorded Cathulu state, distinguishing active work, stale/no signal, owner action/blockage, failed verification, unverified completion, verified completion, stopped, and idle. Animate only active work. Each bubble's name and state remain available to keyboard and assistive technology users.
- Clicking a bubble expands the monitor and selects that project. A compact expand control opens the overview without selecting one.
- The expanded board presents concise, clickable project rows. Opening a row reveals the latest recorded checkpoint and age, a pending owner action, a required-check failure, or the run summary when no checkpoint exists.
- The expanded board also presents a compact workflow pulse (live projects, agents with current working signals, and owner actions) and a visual memory-class mix. The collapsed capsule stays unchanged.
- Keep the expanded companion read-only, transparent, bounded, and independent from the single main dashboard graph. Preserve pin, drag, restore, and close behavior.
- Respect reduced-motion preferences. Working-state motion indicates a recorded running state only.

## Scope

- Refine the React monitor and its CSS using the existing snapshot and Tauri window behavior.
- Derive compact per-project disclosure details from current snapshot records; add no telemetry source or backend write path.
- Update the monitor decision record, roadmap, and this plan.

## Out of scope

- Changes to the main graph, agent hierarchy, persistent memory lifecycle, SQLite schema, MCP behavior, app startup, OS notifications, dashboard navigation, or model use.
- Automatically launching, focusing, or restarting the dashboard or MCP server.
- New UI dependencies or fabricated progress percentages.

## Source-of-truth impact

- SQLite remains authoritative. The monitor consumes the existing `dashboard_snapshot` projection and presents recorded events without changing their meaning.
- Working, stale, blocked, failed, stopped, and idle color states are visual encodings, not new project state.
- Checkpoints, owner actions, and QA failures retain their existing provenance and are labeled as recorded evidence.

## Existing decisions and constraints

- [ADR-0008](../decisions/ADR-0008-compact-dashboard-and-floating-monitor.md) defines a transparent 174 × 58 capsule and a separate per-project expanded companion.
- [ADR-0012](../decisions/ADR-0012-live-agent-progress-and-attention.md) requires factual status and visible attention without inferring completion.
- The dashboard stays read-only and contains one persistent-role graph; the companion is not another run or project page.
- The Codex Mini screenshot is a compactness and disclosure reference, not a source for Cathulu data or branding.
- [ADR-0014](../decisions/ADR-0014-project-aware-compact-companion.md) keeps the collapsed pill quiet and uses only snapshot-backed evidence in expanded state.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`, `ui-ux-research`, `ui-ux-pro-max`, `apple-design`, and `emil-design-eng`.
- Existing React, CSS, Tauri APIs, `DashboardSnapshot`, and `scripts/build-current.ps1`.
- No external design library is needed; the current stack can implement the pill, disclosures, and motion with less maintenance.

## Dependencies

- Existing monitor window and `dashboard_snapshot` read path.
- A matched local Windows desktop/MCP build for delivery.

## Risks and unknowns

- Native transparency and sizing can only be visually confirmed in the desktop window; launching it in this milestone could interrupt the currently used Codex session, so this validation is deliberately deferred.
- Some projects may have more rows than the expanded height; the project list remains bounded and scrollable, while the selected project's disclosure stays inside that list.
- Color cannot be the only status channel; each control and expanded row must expose a text label and an accessible name.

## Milestones

### M32 — Project-aware compact companion

**Goal:** Make the compact pill scannable and make each project’s current evidence one click away.

**Subtasks**

- [x] Keep project bubbles in stable project order and increase their visible size/spacing within the compact capsule.
- [x] Make each bubble a keyboard-accessible button that opens the monitor on that project.
- [x] Make expanded project rows disclose the latest recorded, project-specific evidence and any owner action or mandatory QA failure.
- [x] Preserve transparent surfaces, window controls, bounded layout, and factual activity animation; honor reduced motion.
- [x] Update ADR-0014 and the roadmap.

**Affected areas:** `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, `docs/decisions/ADR-0014-project-aware-compact-companion.md`, `docs/roadmap.md`, and this plan.

**Dependencies:** Current desktop monitor and snapshot projection.

**Acceptance criteria**

- [x] Every registered project has a stable, colored, accessible bubble in the compact view; verified and inconclusive runs have distinct non-working signals.
- [x] Selecting a bubble expands the monitor and opens that project's details; selecting a project row toggles its detail disclosure.
- [x] Details use existing snapshot evidence and distinguish owner actions, checkpoint updates, failed mandatory checks, and run summaries.
- [x] The monitor remains read-only and does not open or focus the dashboard or MCP process as a side effect.
- [x] Active-only pulse animations stop under `prefers-reduced-motion`.

**Validation**

- [x] `npm run build` in `apps/desktop` (TypeScript and Vite production build passed).
- [x] `git diff --check` and source review for accessible labels and truthful state mappings.
- [x] Build and publish the matched current Windows pair with `scripts/build-current.ps1` without starting either process.
- [x] Native visual smoke is recorded as pending; the app/MCP were not launched or focused during this milestone.

**Documentation updates:** Add ADR-0014, update the desktop companion entry in `docs/roadmap.md`, and record build evidence and native-review limits here.

**Applicable specialized skills:** `ui-ux-research`, `ui-ux-pro-max`, `apple-design`, `emil-design-eng`, `implementation-plan`, and `milestone-delivery`.

**Expected Git checkpoint:** A validated M32 commit on `codex/m32-compact-project-companion`, submitted through a pull request to `main`.

### M34 — Docked companion metrics

**Goal:** Show the most useful workflow and memory-usage metrics in the expanded companion so its at-a-glance value matches the main dashboard.

**Subtasks**

- [x] Add live-project, currently-working-agent, and owner-action counts to the expanded companion.
- [x] Add a compact, labeled visual distribution of memory records by class, reusing the dashboard snapshot's class counts.
- [x] Keep the 174 × 58 collapsed capsule unchanged and preserve scroll behavior for the project list.
- [x] Update ADR-0014, this plan, and the roadmap.

**Affected areas:** `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, `docs/decisions/ADR-0014-project-aware-compact-companion.md`, `docs/roadmap.md`, and this plan.

**Dependencies:** Existing `DashboardSnapshot` workflow and memory-health projections.

**Acceptance criteria**

- [x] Expanded companion shows snapshot-derived live projects, agents with current working signals, and pending owner actions.
- [x] Memory class counts use the same Working, Canonical, Episodic, Evidence, and Aggregate values as the dashboard home view.
- [x] The metric strip and memory mix are not rendered in the collapsed capsule.
- [x] No new data source, memory write path, telemetry, or inferred progress is introduced.

**Validation**

- [x] `npm run build` in `apps/desktop` (TypeScript and Vite production build passed).
- [x] `git diff --check` and source review of metric derivations, accessible memory chart labeling, and unchanged compact layout.
- [x] Build and publish the matched current Windows pair without starting either process.
- [ ] Native visual review remains pending; do not open or restart the app during delivery.

**Documentation updates:** Extend ADR-0014, update UX-M5 in the roadmap, and record build evidence and native-review limits here.

**Expected Git checkpoint:** Stack M34 with the current Cathulu UI work; no commit or push is part of this request.

## Final integration validation

- Build the React/TypeScript desktop frontend and the matched Windows MCP/desktop pair.
- Confirm `git diff --check` passes and review the final source/docs diff.
- Check the pull request's required status before merging.
- Leave native visual/runtime smoke explicitly pending rather than claiming it passed.

## Rollback / recovery

Revert the M32 commit if build or desktop integration fails. This milestone changes no persistent data or schema and can be rolled back without migration.

## Progress

- [x] Confirmed clean `main` baseline at `9481bf1` and created the M32 branch.
- [x] Reviewed ADR-0008, PLAN-005, UX-M2, and the existing snapshot-backed monitor.
- [x] Implement compact bubbles and project detail disclosures.
- [x] Build and publish matched pair fingerprint `FBC933195F17AAEE0532F97440E94556D04110DC562BA044D2CDD24786A32025`.
- [x] PR #31 merged to `main`; local `main` fast-forwarded to merge commit `3ca661de36e67b6b4440c419c082dfcc6bf216e8`. GitHub reported no commit statuses or workflow runs.
- [ ] Native visual review in the desktop session remains pending.

## Implementation discoveries / decisions

- The existing bubbles sort by state priority, which changes their project position as work changes. The compact strip will instead use stable project IDs while the expanded board may keep state priority.
- Clicking a compact bubble will both expand the companion and select its project, avoiding an extra navigation step.
- The user asked for an informative compact surface but the app has previously interrupted Codex when launched. The matched pair will be built without launching or restarting it.
- M34 adds only three workflow counts and a small memory-class distribution to the expanded board. The collapsed 174 × 58 capsule stays unchanged.
- The docked board's workflow pulse is derived from the latest recorded run and agent/action signals. The memory chart reuses the same class-count projection as the main dashboard.

## Completion evidence

Frontend build passed with `npm run build` in `apps/desktop`; `git diff --check` passed. The matched release pair is published at `target/mcp-current`, resolving to `target/mcp-versions/FBC933195F17AAEE0532F974`. MCP SHA-256: `5997BDD0C7EDCE1A36AE2C7FD53F9933FFC440A2E07C88D0C10612F1C8A8C4EA`; desktop SHA-256: `A5D9C836B1760FD6BE3C4955986506BB6C6F7D41C9F2F4593641E8DA8137FED3`. PR #31 merged; GitHub returned no statuses or workflows for the commit. Local `main` is at merge commit `3ca661de36e67b6b4440c419c082dfcc6bf216e8`. No app or MCP process was opened or restarted. Native visual review is still pending.

M34 frontend build and `git diff --check` passed on 2026-10-22. The matched pair was published at `target/mcp-versions/68FBA5F7D8C3B3DC45B73270` and the stable `target/mcp-current` path. Source fingerprint: `68FBA5F7D8C3B3DC45B7327033C9943692A79E057BBFFF2AFB54B54E51AFC45D`; MCP SHA-256: `5997BDD0C7EDCE1A36AE2C7FD53F9933FFC440A2E07C88D0C10612F1C8A8C4EA`; desktop SHA-256: `030F484191C073063EA8ABA13BCFF04D9F5A51603C317FFEC5C16264223059C5`. Neither app nor MCP was started, focused, stopped, or restarted. Native visual review remains pending.
