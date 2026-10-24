# Cathulu

Cathulu is pivoting into a local-first personal project operating system. It owns durable project state, tasks, priorities, memory, policy and evidence; Claude Code and Codex are intended to become replaceable cognition and execution providers. The [migration assessment](docs/pivot-migration.md) and [PLAN-020](docs/plans/PLAN-020-mixed-provider-personal-project-os.md) define the staged migration, beginning with provider-free task and checkpoint/resume workflows.

The current implementation remains the Rust/SQLite evidence and memory layer with Codex-specific dispatch, a terminal monitor and an observational desktop dashboard. Main Overseer and per-project Project Manager identities already persist independently of chats. Mixed-provider adapters and the new planning workflows are not implemented by the migration documents. See [ADR-0022](docs/decisions/ADR-0022-mixed-provider-personal-project-os.md) for the accepted direction and retained boundaries.

Cathulu is progressing through Windows-first milestones. M1 registers Git repositories through local MCP and shows their snapshots. M2 adds durable objectives, decisions, workflow runs, quality evidence, and a provenance-preserving memory foundation with bounded HOT context. M3 adds optional, sanitized local Codex hook telemetry and restart reconciliation; it does not store prompts, transcripts, tool inputs, or tool outputs. Cathulu does not run workers or change Codex settings automatically.

## Components

- `cathulu-core`: Rust domain validation, additive SQLite migrations, workflow and memory persistence, and Git reconciliation.
- `cathulu-mcp`: official RMCP stdio server with named domain tools and JSON resources; it does not expose SQL or table CRUD.
- `apps/desktop`: Tauri 2 and React/TypeScript support dashboard for side-by-side use with Codex, a read-only floating task monitor, and separate model/effort settings. The single project graph includes recent agent-reported checkpoints, live delegation states, recorded blockers, required QA failures, and explicit owner actions. Only a selected Main/PM dossier can record an Owner approve/decline decision for its exact current request; it cannot execute or close the underlying action.
- `docs/`: product definition, architecture, decisions, roadmap, and implementation plans.

The settings surface stores choices locally, offers a reviewed TOML diff before any project/user/profile config write, creates backups, and supports export-only. Named-agent settings update only matching existing custom-agent files. Cathulu does not infer account availability or claim that a config was loaded by Codex; those states remain unverified. MCP remains unable to edit Codex configuration.

## Windows development setup

Install Rust with the MSVC target, Node.js 22.12 or newer, Git, Microsoft C++ Build Tools with a Windows SDK, and the WebView2 Runtime.

Install the Tauri CLI v2 if `cargo tauri` is not already available:

```powershell
cargo install tauri-cli --version "^2"
```

Install the dashboard dependencies:

```powershell
cd apps/desktop
npm install
```

Run the desktop app from `apps/desktop`:

```powershell
cargo tauri dev
```

Build and test the Rust core and MCP executable from the repository root. To build the production frontend and publish one matched Windows MCP/desktop/monitor release to the stable path configured in Codex, use:

```powershell
./scripts/build-current.ps1
```

That script stages all three executables, verifies their hashes, and publishes the release into a versioned directory behind the stable `target\mcp-current` junction. Existing processes keep using their loaded build while the pointer advances; the script never terminates or restarts Codex.

Run the Rust checks separately:

```powershell
cargo test -p cathulu-core
cargo test -p cathulu-desktop
```

The `custom-protocol` feature is required for a directly built release desktop executable to load its bundled `frontendDist`; without it, Tauri uses the development URL and the window fails when Vite is not running.

Run the dashboard checks from `apps/desktop`:

```powershell
npm test
npm run build
```

The local database defaults to `%LOCALAPPDATA%\Cathulu\cathulu.sqlite3`. If you override `CATHULU_DATA_DIR`, give the same absolute directory to both the MCP server and desktop app so they open the same database. User Codex settings resolve from `CODEX_HOME` or `%USERPROFILE%\.codex`; project settings can be written only for a repository registered in Cathulu and are read by Codex only when that repository is trusted.

The support dashboard opens directly to one project-spanning role graph: one persisted Main Overseer, one persistent Project Manager role for every registered project, and only recorded worker and hook activity. Each role carries a stable context scope and configured tool-profile indicator. Assignments are temporary; replacement chats reuse the role and its known-by scope. Hook activity with no recorded parent remains visibly unlinked. The tool-profile indicator describes intended role configuration and does not prove the current chat has those tools. The graph shows state and relationship indicators without turning runs into graph pages. The main window opens at 820 × 760 logical pixels and resizes down to 560 × 540. It keeps every project role visible, grows the graph with project count, places attention and recent activity alongside it when width allows, and stacks those panels on narrower screens. Role labels stay readable at the narrow breakpoint; short windows scroll the dashboard as one surface. It is designed to sit beside Codex; it is not the tiny floating monitor. Select Main or a Project Manager to inspect a pending Owner action. The two-step approve/decline control records intent for that exact request; it never performs the operation. The floating monitor remains a read-only 174 × 58 translucent pill that can stay on top while the main dashboard is minimized. It shows one status signal per registered project; expand it to a 360 × 268 project board with the latest recorded work and a clear Working, No fresh signal, Blocked, Failed, Stopped, or Idle state. A signal older than 15 minutes is unconfirmed, not proof that an agent stopped. Only a recorded running task animates. Running the Vite page in a normal browser shows a launch notice because live state requires the Tauri IPC bridge.

The support-dashboard and floating-monitor implementation is tracked in [PLAN-005](docs/plans/PLAN-005-desktop-support-dashboard.md), [PLAN-010](docs/plans/PLAN-010-responsive-support-dashboard.md), and [ADR-0008](docs/decisions/ADR-0008-compact-dashboard-and-floating-monitor.md). The desktop has one graph view for the persistent Main and per-project Project Manager roles; it has no run/repository/memory view tabs. The canonical Windows build is `./scripts/build-current.ps1`; it publishes a matched release under a source fingerprint and advances the stable `target/mcp-current` junction configured in Codex. Native resize/graph/transparency/pin review remains pending.

## Connect Codex

Follow [docs/local-mcp.md](docs/local-mcp.md) to register the built MCP executable in Codex. Starting MCP does not open the desktop dashboard or a terminal window. Start the matching desktop executable or terminal monitor manually when you want to observe work. Optional hook configuration is documented there and remains user-controlled; Cathulu does not edit Codex settings.

## Optional specialist providers

Jev and ScaleDown shadow integrations are disabled by default and require both a local API key and explicit per-run remote consent. See [the provider setup guide](docs/specialist-provider-setup.md) before configuring a test run.

## Current milestone

M40's local assignment dispatcher is included as a source path, but a separate code-level runtime-readiness gate is currently closed. Setting `CATHULU_ROLE_DISPATCHER_ENABLED=1` alone cannot start any role turns. M41 adds an explicit local Owner-action decision control; the Owner decision records authorization only and does not close the underlying action, which remains blocked until the responsible role reports its outcome. Role identity, intended project-workspace MCP access, Main event wake/resume, and runtime concurrency/recovery remain unverified. The matched app/MCP pair was rebuilt at the source fingerprint recorded in [PLAN-015](docs/plans/PLAN-015-event-driven-role-orchestration.md); no tests were run and no app or MCP process was launched or restarted. The active MCP will apply schema v15 only after a controlled reload.

M1-M9, M11, and M15-M18b are published to `main`. M9's separate Codex model/effort settings passed local validation at `f2381ae`, and M11's optional local Graphify retrieval passed its evidence gate at `e18dc46`. The live owner Codex config and runtime-effective model remain untouched/unverified. PLAN-004 defines the optional specialist-model layer. Jev and ScaleDown shadow adapters are implemented with disabled-by-default, per-run remote consent; loopback contract tests and public MCP missing-key fallback checks do not contact providers. Three owner-approved Jev workflow-routing shadow observations are recorded, including two independent PULSO task runs; their `insufficient_context` results and safety telemetry do not establish model quality or workflow value. M18 comparative scoring still needs a representative comparable cohort. M27 role logic, migration, UI suites, full Rust suite, frontend build, and the matched Windows release have passed; the published pair is behind `target/mcp-current`. M28 now records agent checkpoints and explicit owner actions, shows active/waiting/stale signals in the one graph, and updates the Main Overseer instructions to report per-project progress without an all-project wait barrier. Rust formatting, diff checks, frontend production build, and matched Windows pair build passed locally; automated M28 suites were not run. Native visual/runtime review remains pending. M29 and M30 are merged: the main window is 820 × 760 logical pixels (560 × 540 minimum), places attention/recent activity alongside the graph when width permits, keeps every project role visible without an inner graph scrollbar, and preserves label size at narrow widths. The frontend and matched Windows build passed; native resize review remains pending, and the currently running MCP has not been restarted. The M30 pair was published at `target/mcp-current` with source fingerprint `2886574883A76A4D65A9C22F4CE951C610340F5D8908A39A45C49D3D3D8B32EB`. See [the live-agent observability plan](docs/plans/PLAN-009-live-agent-observability.md) for M28 evidence, [the responsive dashboard plan](docs/plans/PLAN-010-responsive-support-dashboard.md) for M29-M30, plus the [hierarchy plan](docs/plans/PLAN-006-bounded-project-agent-hierarchy.md), [specialist-model plan](docs/plans/PLAN-004-optional-specialist-model-optimization.md), and [persistent-role plan](docs/plans/PLAN-008-persistent-logical-agent-roles.md).

M37/M38 now make source-based project memory part of run closeout and seed initial context for all five registered projects. The local store contains seven accepted HOT canonical records (including a global project-independence constraint and Mecanografica's RUC-history constraint); no transcripts or remote models were used. The dashboard snapshot reports 7 canonical memories and 3,230 HOT bytes. The M38 matched app/MCP pair was published at source fingerprint `559903471A6F790382FD64C62BA6928D009873B9D005F345268249A41EEFEED9`; the active MCP process has not been restarted, so its newly built closeout gate loads only after a controlled Cathulu-only restart. Git freshness remains unpinned for source files that are modified or lack a registered HEAD. See [PLAN-016](docs/plans/PLAN-016-project-context-capture.md) and [ADR-0017](docs/decisions/ADR-0017-project-memory-capture.md).

