# Cathulu

Cathulu is a local project evidence layer around Codex. Codex remains the place where work and agent execution happen; Cathulu stores durable project state and presents it in a read-only dashboard with a separate explicit settings surface. Main Overseer and per-project Project Manager identities persist in Cathulu independently of the Codex chats temporarily occupying those roles, so context and known-by state can continue across chat replacement.

Cathulu is progressing through Windows-first milestones. M1 registers Git repositories through local MCP and shows their snapshots. M2 adds durable objectives, decisions, workflow runs, quality evidence, and a provenance-preserving memory foundation with bounded HOT context. M3 adds optional, sanitized local Codex hook telemetry and restart reconciliation; it does not store prompts, transcripts, tool inputs, or tool outputs. Cathulu does not run workers or change Codex settings automatically.

## Components

- `cathulu-core`: Rust domain validation, additive SQLite migrations, workflow and memory persistence, and Git reconciliation.
- `cathulu-mcp`: official RMCP stdio server with named domain tools and JSON resources; it does not expose SQL or table CRUD.
- `apps/desktop`: Tauri 2 and React/TypeScript read-only support dashboard for side-by-side use with Codex, a floating task monitor, and separate model/effort settings for project and workflow evidence. The single project graph includes recent agent-reported checkpoints, live delegation states, recorded blockers, required QA failures, and explicit owner actions.
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

Build and test the Rust core and MCP executable from the repository root. To build the production frontend and publish one matched Windows MCP/desktop pair to the stable path configured in Codex, use:

```powershell
./scripts/build-current.ps1
```

That script stages both executables, verifies their hashes, and publishes the pair into a versioned directory behind the stable `target\mcp-current` junction. Existing processes keep using their loaded build while the pointer advances; the script never terminates or restarts Codex.

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

The support dashboard opens directly to one project-spanning role graph: one persisted Main Overseer, one persistent Project Manager role for every registered project, and only recorded worker and hook activity. Each role carries a stable context scope and configured tool-profile indicator. Assignments are temporary; replacement chats reuse the role and its known-by scope. Hook activity with no recorded parent remains visibly unlinked. The tool-profile indicator describes intended role configuration and does not prove the current chat has those tools. The graph shows state and relationship indicators without turning runs into graph pages. The main window opens at 820 × 760 logical pixels and resizes down to 560 × 540. It keeps every project role visible, grows the graph with project count, places attention and recent activity alongside it when width allows, and stacks those panels on narrower screens. Role labels stay readable at the narrow breakpoint; short windows scroll the dashboard as one surface. It is designed to sit beside Codex; it is not the tiny floating monitor. Use **Float monitor** for the separate 174 × 58 translucent, read-only pill that can stay on top while the main dashboard is minimized. It shows one status signal per registered project; expand it to a 360 × 268 project board with the latest recorded work and a clear Working, No fresh signal, Blocked, Failed, Stopped, or Idle state. A signal older than 15 minutes is unconfirmed, not proof that an agent stopped. Only a recorded running task animates. Running the Vite page in a normal browser shows a launch notice because live state requires the Tauri IPC bridge.

The support-dashboard and floating-monitor implementation is tracked in [PLAN-005](docs/plans/PLAN-005-desktop-support-dashboard.md), [PLAN-010](docs/plans/PLAN-010-responsive-support-dashboard.md), and [ADR-0008](docs/decisions/ADR-0008-compact-dashboard-and-floating-monitor.md). The desktop has one graph view for the persistent Main and per-project Project Manager roles; it has no run/repository/memory view tabs. The canonical Windows build is `./scripts/build-current.ps1`; it publishes a matched release under a source fingerprint and advances the stable `target/mcp-current` junction configured in Codex. Native resize/graph/transparency/pin review remains pending.

## Connect Codex

Follow [docs/local-mcp.md](docs/local-mcp.md) to register the built MCP executable in Codex. Starting that MCP opens its matching desktop dashboard in the background without taking focus; repeated starts leave an existing window alone. Optional hook configuration is documented there and remains user-controlled; Cathulu does not edit Codex settings.

## Optional specialist providers

Jev and ScaleDown shadow integrations are disabled by default and require both a local API key and explicit per-run remote consent. See [the provider setup guide](docs/specialist-provider-setup.md) before configuring a test run.

## Current milestone

M1-M9, M11, and M15-M18b are published to `main`. M9's separate Codex model/effort settings passed local validation at `f2381ae`, and M11's optional local Graphify retrieval passed its evidence gate at `e18dc46`. The live owner Codex config and runtime-effective model remain untouched/unverified. PLAN-004 defines the optional specialist-model layer. Jev and ScaleDown shadow adapters are implemented with disabled-by-default, per-run remote consent; loopback contract tests and public MCP missing-key fallback checks do not contact providers. Three owner-approved Jev workflow-routing shadow observations are recorded, including two independent PULSO task runs; their `insufficient_context` results and safety telemetry do not establish model quality or workflow value. M18 comparative scoring still needs a representative comparable cohort. M27 role logic, migration, UI suites, full Rust suite, frontend build, and the matched Windows release have passed; the published pair is behind `target/mcp-current`. M28 now records agent checkpoints and explicit owner actions, shows active/waiting/stale signals in the one graph, and updates the Main Overseer instructions to report per-project progress without an all-project wait barrier. Rust formatting, diff checks, frontend production build, and matched Windows pair build passed locally; automated M28 suites were not run. Native visual/runtime review remains pending. M29 and M30 are merged: the main window is 820 × 760 logical pixels (560 × 540 minimum), places attention/recent activity alongside the graph when width permits, keeps every project role visible without an inner graph scrollbar, and preserves label size at narrow widths. The frontend and matched Windows build passed; native resize review remains pending, and the currently running MCP has not been restarted. The latest pair is published at `target/mcp-current` (source fingerprint `2886574883A76A4D65A9C22F4CE951C610340F5D8908A39A45C49D3D3D8B32EB`). See [the live-agent observability plan](docs/plans/PLAN-009-live-agent-observability.md) for M28 evidence, [the responsive dashboard plan](docs/plans/PLAN-010-responsive-support-dashboard.md) for M29-M30, plus the [hierarchy plan](docs/plans/PLAN-006-bounded-project-agent-hierarchy.md), [specialist-model plan](docs/plans/PLAN-004-optional-specialist-model-optimization.md), and [persistent-role plan](docs/plans/PLAN-008-persistent-logical-agent-roles.md).

