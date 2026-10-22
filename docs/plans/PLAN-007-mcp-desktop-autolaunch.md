# PLAN-007 — Launch the desktop dashboard with the local MCP server

**Status:** In progress
**Created:** 2026-10-22
**Updated:** 2026-10-22
**Milestone:** M26

## Objective

When Codex starts Cathulu's local stdio MCP server, automatically open the matching local read-only desktop dashboard in the background. Keep MCP available if the desktop executable is missing or fails to start, and leave an existing dashboard window untouched instead of interrupting the active application.

## Motivation

The configured M23 MCP executable starts independently from the Tauri desktop application. The matching desktop executable was absent beside it, so Codex could connect to MCP without displaying the observability window. The dashboard and MCP must use the same source revision, executable directory, and local data configuration.

## Current and desired state

- **Current:** Codex points to `target/mcp-current/cathulu-mcp.exe`, a stable Windows junction now targeting the schema-12 pair in `target/mcp-versions/ADB67AE38250F74FB9C4C503`. The user restarted Codex and the active MCP exposed `cathulu://roles`. During that restart, the dashboard visibly opened and closed repeatedly. The new matched build schedules dashboard startup off the MCP stdio path; the current MCP process still needs a Cathulu-only restart to load this follow-up.
- **Desired:** Build through `scripts/build-current.ps1`. It stages both release executables from the same source fingerprint, uses Tauri's `custom-protocol` build, verifies hashes, stores the pair in a versioned directory, and advances the stable junction. Codex configuration then remains fixed across future builds. MCP schedules the sibling desktop through the OS shell on a background thread while RMCP stdio becomes ready immediately; first launch is non-activating and repeated starts leave the existing window untouched.

## Scope

- Add fail-open sibling-process launch to the MCP binary.
- Make desktop auto-launch and single-instance behavior non-activating.
- Publish matched MCP and desktop executables behind a stable configured path without replacing binaries used by an open dashboard.
- Preserve the pending graph-first dashboard and floating-monitor changes from the local working tree on top of M23, then validate them in the matched build.
- Document the paired executable requirement and startup behavior.

## Exclusions

- No changes to agent dispatch, project tasks, MCP memory state, or QA authority.
- No new background service, remote provider, user-config schema, or dashboard write operation.
- No restart or interruption of an active project worker. MCP may be reloaded only after its replacement is ready.
- This milestone does not claim that every project/agent activity is observable; only persisted dashboard evidence is shown.

## Constraints and decisions

- SQLite remains authoritative and the desktop dashboard remains read-only.
- MCP JSON-RPC stdio remains owned by RMCP. The desktop process must never inherit or write to MCP stdin/stdout/stderr.
- An absent or failed desktop launch must not prevent MCP startup.
- Windows is the initial runtime target. `CATHULU_DATA_DIR` is inherited by the child so both processes open the same store.
- The Codex command now points to `target/mcp-current/cathulu-mcp.exe`. The M27 schema-12 pair is published under `target/mcp-versions/858B4E0C5836ED87EF3D5CFA`; the connected service was not restarted, so live reconnection remains pending. The current logical-role build was kept separate from prior locked executables.
- ADR-0010 records the startup and single-instance policy.

## Risks and mitigations

- **MCP stdout corruption or lifetime coupling:** use `ShellExecuteW` on Windows so the GUI is launched independently of MCP stdio handles, then smoke-test MCP initialization.
- **Duplicate windows:** register Tauri's single-instance plugin with a no-op callback; serialize simultaneous MCP-triggered launches and check for the plugin's event window before spawning the GUI.
- **Version mismatch or development URL in release:** build MCP and desktop from the same source fingerprint, enable Tauri `custom-protocol`, verify both SHA-256 values, and advance the stable junction only after the versioned pair passes those checks.
- **Missing desktop or launch failure:** log to MCP stderr and continue serving MCP.
- **Shared database startup:** preserve the existing local store and verify the dashboard opens it without a migration or data reset.

## Milestone M26 — Matched desktop auto-launch

**Goal:** Starting the configured MCP server opens its corresponding dashboard in the background without changing MCP transport behavior or interrupting typing in the active application.

**Affected areas:** `src/bin/cathulu-mcp.rs`, `apps/desktop/src-tauri/Cargo.toml`, `apps/desktop/src-tauri/src/lib.rs`, `Cargo.lock`, dashboard UI/core projection, `docs/architecture.md`, `docs/local-mcp.md`, `docs/roadmap.md`, `README.md`, `ADR-0010`, and this plan.

**Acceptance criteria:**

1. The matching MCP and desktop release binaries are built from the same current source and colocated at the Codex-configured directory.
2. Launching the configured MCP binary opens the desktop dashboard without activating it, and the MCP initialize/tool-list exchange still succeeds over stdio.
3. Launching MCP again does not create another window or activate, show, restore, or focus the existing dashboard.
4. If the desktop binary is absent or cannot spawn, MCP still initializes normally and reports the dashboard issue only on stderr.
5. The desktop reads the existing schema-11 database, applies additive migration 12, and renders the persistent role registry without destructive data changes.
6. The Codex MCP config still names the current MCP executable; no previous build is selected.
7. Tauri renders its window before repository reconciliation finishes; background Git commands do not open console windows on Windows.

**Required validation:** frontend production build; locked Rust release builds for MCP and desktop; actual MCP initialize, `tools/list`, and dashboard-resource smoke exchange; Windows process/window smoke confirms first launch preserves the previously foreground process and repeated starts preserve the same dashboard process/window; read-only SQLite integrity/schema check.

**Documentation updates:** update ADR-0010, README, architecture, local-MCP setup, roadmap, and this plan with exact validation evidence and any remaining manual limitation.

**Expected Git checkpoint:** validated M26 commit on a branch based on the current `main`; advance `main` only after required checks pass.

## Progress and evidence

- [x] Confirmed the configured MCP path and schema-11-compatible installed MCP binary.
- [x] Confirmed no Cathulu desktop process was running and no matching desktop binary was installed beside MCP.
- [x] Confirmed the local graph-first dashboard changes apply cleanly to the current M23 source; included them in the matched build candidate without replacing the user's working-tree changes.
- [x] Implement sibling launch and non-activating, idempotent single-instance behavior in the current schema-11 source snapshot.
- [x] Install the matched schema-11 executables at the configured MCP directory and verify the bundled frontend.
- [x] Capture the live dashboard window and verify the MCP stdio initialize/resources/tools exchange.
- [x] Run a read-only SQLite integrity check; the existing store reports `ok` at schema 11.
- [ ] Reload the configured server in the active Codex client and verify its live resource/tool connection.
- [x] Reconcile the schema-11 hierarchy/activation source snapshot with the tracked checkout while preserving UX-M3 changes.
- [x] Build and verify the schema-12 desktop/MCP pair from the tracked source.
- [x] Publish a hash-verified versioned pair and stable `target/mcp-current` junction, and update the Codex command once.
- [ ] Restart the connected MCP after active orchestration work is clear; verify `cathulu://roles` and dashboard startup from the new pair.

### Production frontend correction — 2026-10-22

- User-visible evidence showed the packaged desktop window displaying a WebView connection error for `127.0.0.1:1420`.
- Root cause: the desktop crate had no `custom-protocol` feature, so direct Cargo release builds selected Tauri's development URL instead of bundled `frontendDist`.
- Added `custom-protocol = ["tauri/custom-protocol"]` to the desktop crate and documented the required release flag; development builds remain on `devUrl`.
- The schema-11 source snapshot builds the current pair: desktop SHA-256 `42A6190DF8FFC594D94E76E3A2D6092450C43720AF424E6D29C8AD29B41DF2FE`; MCP SHA-256 `597BA2B6C46CD3F1D66C48ED364CA160908937CA490D7325428B004F3E5EE413`. Both hashes match the executables now installed at the configured `target/mcp-refresh/release` path.
- Historical note: a build from the tracked checkout at the time supported schema 9 and exited with code 101 against the machine's schema-11 database. It was not installed. The M27 source reconciliation brought the tracked checkout through schema 12; the new matched release pair is validated separately and has not replaced the configured pair.
- The earlier captured window at `target/m26-current-dashboard-check.png` shows the rendered project/run graph, not the WebView error. After the responsiveness rebuild, the installed desktop was opened and Windows reported its `Cathulu` window as responding.
- A read-only SQLite check returned `PRAGMA integrity_check = ok` and `MAX(schema_migrations.version) = 11`. A direct stdio smoke against the installed pair completed `initialize`, `tools/list` (40 tools), and `resources/read` for `cathulu://dashboard` in 162 ms.
- The Cathulu entry is restored in the user's Codex config and `codex mcp list` reports it enabled at the configured path. The active chat's MCP transport still returns `Transport closed`; the app-server has no reload control exposed in this chat. The official desktop workflow is **Settings → MCP servers → Cathulu → Restart**, which reloads MCP configuration for loaded threads.
- M26's paired launch implementation and schema-11 source reconciliation are complete in the tracked worktree. M27's schema-12 pair builds successfully in isolation; active Codex reconnect and Git progression remain separate from this build validation.

### Focus-safe background launch — 2026-10-22

- Removed the explicit `set_focus`, `show`, and `unminimize` actions from Tauri's single-instance callback. The configured main window now has `focus: false`.
- The Windows MCP launcher now checks for the registered dashboard window while holding its startup mutex and returns without launching or restoring it when already open. First launch uses `SW_SHOWNOACTIVATE`.
- Follow-up startup review found that the launcher ran synchronously before RMCP stdio and could wait up to eight seconds for the desktop window. That could delay MCP initialization during a cold Codex start and plausibly trigger client retries matching the reported open/close cycle. The launcher now runs on a named background thread; MCP initialization proceeds independently while the existing launch mutex and non-activating GUI start remain in place.
- Built and published the updated matched pair with `scripts/build-current.ps1`. Source fingerprint: `ADB67AE38250F74FB9C4C503D6372AAA306AD18D068050AAFCE1B8AEAA26D8AF`; MCP SHA-256: `0E6F42F06C084C30D1B0DA7091BBE19C69C1581CEFFA273EA60B72A945312569`; desktop SHA-256: `D0637371B6196E08270652C0098655192749EE8BA42D74235DCA5C9C095F5F18`. The stable `target/mcp-current` junction resolves to this pair. Live verification is pending a Cathulu-only MCP restart; the dashboard was not manually reopened during this build.
- Built the MCP and desktop from `target/m26-current-source` and installed the matched pair at the configured release path. MCP SHA-256: `126C0205D3BA0D2BD3E3061429AF4EB9D8A0D06BAB817297DDB5B02C87D72D20`; desktop SHA-256: `00688B790B5A2D41D90FDD6310902F0D24F53DF4BBDCFE886B18D4A4EF44C96F`.
- Ran two hidden MCP stdio smoke launches. Both completed `initialize`, `tools/list` (40 tools), and `resources/read` for `cathulu://dashboard`; neither wrote to stderr. The first launch opened one `Cathulu` window (PID 42772, responding); the second reused that same process and window.
- Windows foreground remained PID 6452 (`ChatGPT`) before and after both MCP launches. This verifies that first startup and repeated MCP starts did not take keyboard focus from the active Codex host.
- M26 remains **in progress** pending live reconnection of the schema-12 matched pair and final validated Git progression; the stable path now resolves to the new pair.

### Startup responsiveness and console recovery — 2026-10-22

- Root cause: desktop `.setup` synchronously reconciled every registered repository before Tauri created the window. MCP startup performed the same full scan, causing duplicate Git launches. Windows Git/Codex child processes had no `CREATE_NO_WINDOW` flag.
- Moved desktop reconciliation to `tauri::async_runtime::spawn_blocking` after the store is managed, removed the duplicate full scan from MCP startup, and applied Windows no-console creation flags to Cathulu's Git and Codex CLI subprocesses. Explicit project reconciliation remains available.
- Rebuilt the matched schema-11 pair from `target/m26-current-source` with the desktop `custom-protocol` feature and installed it at the configured `target/mcp-refresh/release` path. MCP SHA-256: `597BA2B6C46CD3F1D66C48ED364CA160908937CA490D7325428B004F3E5EE413`; desktop SHA-256: `42A6190DF8FFC594D94E76E3A2D6092450C43720AF424E6D29C8AD29B41DF2FE`. This is the currently installed pair until the schema-12 build is validated.
- Opened the installed desktop; Windows reports the Cathulu window responding. A bounded stdio smoke against the installed MCP completed `initialize`, `tools/list` (40 tools), and `resources/read` for `cathulu://dashboard` in 162 ms.
- The currently loaded Codex MCP transport in this chat still reports `Transport closed`; restarting the Cathulu server from **Settings → MCP servers → Cathulu → Restart** is still required for this chat to use the replacement binary. The full Codex app does not need to be restarted.
- Targeted `rustfmt --check` passes for every Rust source file changed by this fix. Workspace-wide `cargo fmt --check` still reports two pre-existing formatting differences in `src/bin/cathulu-mcp.rs`; this file was not changed by the responsiveness fix. `git diff --check` passes.
- The post-start process snapshot shows no Git process and no Cathulu/Git parent among the 17 console-host processes; those hosts map to Codex/runtime, MSI, command, and already-exited parents.
- M26 remains **in progress** pending live Codex reconnection to the validated schema-12 pair and final Git checkpoint validation. Schema-11 source reconciliation is complete in the tracked worktree.

### Stable current-build refresh — 2026-10-22

- Added `scripts/build-current.ps1` as the canonical Windows matched-release build. It builds the frontend through Tauri CLI, builds MCP from the same source tree, computes a source fingerprint, verifies executable hashes, and publishes into `target/mcp-versions/<fingerprint>`.
- Codex's Cathulu command was updated once to `target/mcp-current/cathulu-mcp.exe`. The stable path is a junction, so each future build can advance to a new pair without overwriting an executable currently open by Windows.
- Published schema-12 source `858B4E0C5836ED87EF3D5CFA`. MCP SHA-256: `A77BBACF1A5B3439A13FA7F5B69A69C158CFEA4C4E47780A3E473A5FF856D851`; desktop SHA-256: `2315B70A9FD896F87D1FCEC8D5C6B780F93F3CF57FB9D91FB34F5721CE71BC57`.
- The script and configuration update did not restart the already-running MCP process or open a window. The current connection remains on its loaded prior build until a controlled MCP restart.

## Recovery

If the dashboard fails to start, MCP remains usable. Rebuild both binaries from this plan's validated source into the configured directory. To revert the behavior, remove the MCP startup launch call and single-instance registration, then rebuild the same source revision; do not select an unrelated debug or stale executable.
