# PLAN-009 — Live agent progress and action visibility

Status: M28 implementation and matched Windows build validated on `codex/m28-live-agent-observability`; live MCP reconnection and native visual review remain pending.
Created: 2026-10-22
Last updated: 2026-10-22

## Objective

Make Cathulu's single support dashboard show meaningful, current work by each Project Manager and Worker, surface owner actions prominently, and let the Main Overseer report partial progress while independent project agents continue working.

## Motivation

The current one-graph dashboard shows delegations, reports, and optional start/stop hook telemetry, but has no durable in-progress update from an agent. Active runs can therefore look static, and action requests are not a first-class state. The orchestration contract also describes reviewing and reporting after project work without requiring prompt partial updates, which can lead to a barrier where the owner hears nothing until every project finishes.

## Current state

- SQLite already has an append-only, indexed `events` ledger with external-event deduplication.
- `DashboardSnapshot` includes projects, blockers, recent runs, delegations, hook observations, failed checks, and memory health.
- The desktop has one read-only project-spanning role graph and a compact floating monitor; it polls the snapshot every five seconds.
- Delegation status, worker reports, and hooks do not express intermediate work summaries or explicit owner requests.
- The Main Overseer contract does not require per-project progress updates while other agents continue.

## Desired state

- A bounded local MCP operation records append-only delegation updates: working/progress, waiting, action required, and action resolved. Progress stage is a controlled value; summaries are short reports, not proof of code execution or correctness.
- Explicit owner actions remain visible until resolved with the same action key. Existing blocked tasks and failed mandatory checks also appear in the action area.
- Agent status is freshness-aware: after 15 minutes without a new progress or hook signal, the dashboard marks activity as unconfirmed rather than continuing to show a green active state or claiming the agent stopped. Sustained work should report a concise checkpoint about every 10 minutes.
- The one graph shows active/waiting/blocked counts, recent update count, per-project and per-agent latest reported activity, and the exact pending owner request. The compact monitor signals projects needing attention and exposes the request when expanded.
- The Main Overseer sends each meaningful project update as soon as it arrives, handles the first completed/attention-needed project before monitoring the remaining chats, keeps other projects moving, and asks the owner about a blocker without waiting for unrelated agents to finish.
- Dashboard refresh remains passive and read-only: no focus-stealing window, popup, or fabricated percentages.

## Scope

- Reuse the current append-only `events` table; add validation, idempotency, and typed snapshot projections for agent updates and attention items.
- Update the existing MCP tool surface, dashboard graph, floating monitor, orchestration contract, relevant source-of-truth docs, and the existing orchestrator skill where writable.
- Keep metrics limited to values directly derivable from recorded events and domain state.

## Out of scope

- Agent spawning, controlling, interrupting, or declaring agent success.
- OS-level notifications, sounds, automatic window opening/focus, new tabs, or a second dashboard view.
- Model-generated progress percentages, inferred file/test activity, transcript capture, or changes to project repositories.
- Automatic policy changes or new database schema unless implementation inspection proves the existing event ledger cannot support the needed lifecycle.

## Source-of-truth impact

- [Architecture](../architecture.md): live progress evidence, attention projection, and incremental owner updates.
- [Agent operating contract](../agent-operating-contract.md): non-blocking, per-project progress protocol for Main Overseer.
- [ADR-0012](../decisions/ADR-0012-live-agent-progress-and-attention.md): event authority, reporting semantics, and action lifecycle.
- [Roadmap](../roadmap.md): add M28 and evidence state.
- [README](../../README.md): describe the dashboard's activity and attention indicators.
- This plan records acceptance evidence and remaining runtime limitations.

## Existing decisions and constraints

- ADR-0001 keeps Cathulu a local evidence layer; Codex remains the execution system.
- ADR-0008 keeps the dashboard read-only, one-graph, narrow-window, and evidence-backed.
- ADR-0009/0011 keep the Main Overseer coordinator-only and logical roles persistent across chats.
- A reported progress update is an agent statement, not proof that a tool ran, a change is correct, or QA passed.
- No active Codex chat identity is available to Cathulu; observed activity must stay linked to its recorded delegation or remain unlinked.
- A blocking owner action must not pause unrelated project coordination.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`, `ui-ux-research`, `ui-ux-pro-max`, and `ponytail-balanced`.
- Existing local SQLite event ledger, MCP server, Tauri dashboard snapshot, React graph, and current build script.

## Dependencies

- Schema 12, persistent logical roles, and the M27 single graph are already present.
- Current build dependencies are already installed; do not add a dashboard library for counters, event rows, or status indicators.

## Risks and unknowns

| Risk | Mitigation |
| --- | --- |
| Agent progress text can overstate work | Label it as reported; keep QA, Git, and artifact evidence authoritative. |
| Unresolved attention could become stale | Use explicit keyed resolution and show age; retain history in append-only events. |
| Too much telemetry may clutter the compact view | Show a few concise signals and the highest-priority action; keep history in the existing graph detail. |
| No agent posts interim updates | Improve the Main/PM operating contract and provide a simple tool; show stale/no-update as unknown instead of inventing activity. |
| High-frequency updates grow evidence | Require meaningful checkpoints and bound the dashboard projection; do not record every tool call or transcript. |

## Milestones

### M28 — Live progress and owner-action visibility

Goal: Give the Overseer and owner an immediate, truthful view of each project's current agent work and explicit requests for owner input, without waiting for every project to finish.

Subtasks:

- [x] Add a bounded, idempotent MCP progress/action update backed by append-only delegation events.
- [x] Project recent updates and unresolved actions from SQLite into `DashboardSnapshot`; include existing blocked tasks and failed mandatory checks in the attention surface.
- [x] Add compact graph metrics, per-agent last-update detail, a visible action-needed card, and attention cues in the floating monitor.
- [x] Update the normative agent contract and installed orchestrator skill so meaningful updates are forwarded as they arrive, long work refreshes its checkpoint, and independent agents continue without an all-project wait barrier.
- [x] Update ADR, architecture, roadmap, README, and this plan with verified implementation evidence.

Affected areas: `src/delegation.rs`, `src/dashboard.rs`, `src/mcp.rs`, `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, existing UI fixtures, `docs/agent-operating-contract.md`, `docs/local-mcp.md`, `docs/architecture.md`, `docs/roadmap.md`, `README.md`, `scripts/build-current.ps1`, and the orchestrator skill.

Dependencies: M27 graph and schema-12 role projection.

Acceptance criteria:

- [x] Updates accept only known states/stages, enforce bounded text, and are idempotent; the same key with different content is rejected.
- [x] Progress, waiting, and action-required/resolved events preserve history; a keyed action stays visible until its matching resolution.
- [x] Snapshot and UI distinguish agent-reported activity from hook, Git, artifact, and QA evidence.
- [x] Stale working/waiting/hook activity becomes visually unconfirmed after 15 minutes; it is never presented as verified stopped.
- [x] Main view makes current agent counts, latest reports, and owner action explicit while retaining one graph and a compact information hierarchy.
- [x] Floating monitor surfaces action-needed and stale-signal states without opening/focusing windows or blocking Codex.
- [x] Main Overseer instructions require incremental per-project owner updates and no all-agents completion barrier.
- [x] No schema migration was added; the existing append-only event store supports the update lifecycle.

Validation:

- [x] `cargo fmt --all --check` and `git diff --check` pass after freshness-state changes.
- [x] `npm run build` passes TypeScript checks and production bundling.
- [x] `scripts/build-current.ps1` builds and publishes the matched Windows app/MCP pair without starting/restarting transport or focusing the desktop app.
- [x] Review the final changed-file diff; no project repository or user database was modified.

Documentation updates: ADR-0012, architecture, agent contract, roadmap, README, and completion evidence here.

Applicable specialized skills: `implementation-plan`, `milestone-delivery`, `ui-ux-research`, `ui-ux-pro-max`, `ponytail-balanced`.

Expected Git checkpoint: One validated M28 commit on this branch; advance `main` only after the documented build gate passes.

## Final integration validation

- Build the full matched Windows desktop/MCP pair after stacked changes.
- Keep the configured MCP process and current app window untouched during validation to avoid focus changes or repeated launches.
- Confirm the snapshot remains read-only and polling does not steal focus.
- Record any runtime behavior requiring an owner-side Codex reload as pending rather than claiming it was exercised.

## Rollback / recovery

All new records use the existing append-only events table. UI projections and the MCP write operation can be reverted without a database downgrade; historical progress events remain harmless audit evidence. If the update API is rolled back, the dashboard must ignore its event types.

## Progress

- [x] Reviewed the current event ledger, delegation model, dashboard snapshot, one-graph UI, and Main/PM operating contract.
- [x] Implement M28 update lifecycle and snapshot projection.
- [x] Implement the graph and floating-monitor attention presentation.
- [x] Update incremental orchestration behavior and source-of-truth docs.
- [x] Build and record final evidence.

## Implementation discoveries / decisions

- The existing event table already has append-only triggers, aggregate indexing, and unique external event IDs. This supports progress/action history without a schema migration.
- The dashboard remains SQLite-backed and read-only. Only a narrow typed MCP operation writes progress events.
- Progress statements remain explicitly self-reported; hook events and deterministic QA/Git facts keep their existing authority.
- A 15-minute freshness threshold changes only the visual state of old signals; it never asserts that an agent stopped. Main uses first-completion/attention monitoring and checks snapshots during longer tasks.
- The Windows matched pair is published at `target/mcp-versions/585CAB4839C1DA70182E9852`; the stable Codex path is `target/mcp-current`.
- Source fingerprint: `585CAB4839C1DA70182E98525A7D635B2005A80DC5B3C6B08B6E66EE8A9D6EDC`.
- Source revision: `453b006eaa791ed8365e96ea4b3af1dd4e9597a6` with `workingTreeDirty: true` at build time.
- MCP SHA-256: `064C730A6B0820179AEFCC097E1C19AB97A5C45E49CEE87C54C30DA2D538D1AD`; desktop SHA-256: `3A934A0054BAEBA6BA91E6E12E7E2FD077A1DA5834E1E5756C7E570209589DF3`.
- Rust formatting, diff checks, and the frontend production build passed. Automated M28 test suites were not run. No app or MCP process was launched/restarted; the currently running MCP still needs a controlled reload, and native visual review remains pending.

## Completion evidence

M28 acceptance criteria and the matched local build gate passed on 2026-10-22. The built pair is published behind `target/mcp-current`. Runtime reconnection and native visual smoke remain pending; no user-visible window was opened during the build.
