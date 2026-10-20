# Cathulu

Cathulu is a local project evidence layer around Codex. Codex remains the place where work and agent execution happen; Cathulu stores durable project state and presents it in a read-only desktop dashboard.

Cathulu is progressing through Windows-first milestones. M1 registers Git repositories through local MCP and shows their snapshots. M2 adds durable objectives, decisions, workflow runs, quality evidence, and a provenance-preserving memory foundation with bounded HOT context. Cathulu does not run workers or change Codex settings automatically.

## Components

- `cathulu-core`: Rust domain validation, additive SQLite migrations, workflow and memory persistence, and Git reconciliation.
- `cathulu-mcp`: official RMCP stdio server with named domain tools and JSON resources; it does not expose SQL or table CRUD.
- `apps/desktop`: Tauri 2 and React/TypeScript read-only dashboard for projects, objectives, runs, quality, and compact memory health.
- `docs/`: product definition, architecture, decisions, roadmap, and implementation plans.

Cathulu will host model and reasoning-effort selectors for the Main Overseer and future sub-agents in a later milestone, in a settings surface separate from the dashboard. M1 has no such controls, and the dashboard remains read-only. How selections will be applied to Codex and whether separate values per role are supported remain to be verified.

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

Build and test the Rust core and MCP executable from the repository root:

```powershell
cargo test -p cathulu-core
cargo test -p cathulu-desktop
cargo build --release --bin cathulu-mcp
```

Run the dashboard checks from `apps/desktop`:

```powershell
npm test
npm run build
```

The local database defaults to `%LOCALAPPDATA%\Cathulu\cathulu.sqlite3`. If you override `CATHULU_DATA_DIR`, give the same absolute directory to both the MCP server and desktop app so they open the same database.

## Connect Codex

Follow [docs/local-mcp.md](docs/local-mcp.md) to register the built MCP executable in Codex. The instructions are manual by design; Cathulu does not edit user-level Codex configuration.

## Current milestone

M1 is published to `main`. M2's workflow and memory foundations are implemented and validated on `codex/phase1-workflow-state`: seven Rust tests, three React tests, formatting, frontend and Windows release builds, isolated startup/database smoke, and a dashboard preview/focus check pass. Run native Rust commands from an x64 Visual Studio developer shell with the MSVC linker and Windows SDK available. See [the Phase 1 first-slice plan](docs/plans/PLAN-001-phase1-first-slice.md) for current milestone status, commands, evidence, and the owner-controlled Codex connection follow-up.
