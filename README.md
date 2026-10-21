# Cathulu

Cathulu is a local project evidence layer around Codex. Codex remains the place where work and agent execution happen; Cathulu stores durable project state and presents it in a read-only dashboard with a separate explicit settings surface.

Cathulu is progressing through Windows-first milestones. M1 registers Git repositories through local MCP and shows their snapshots. M2 adds durable objectives, decisions, workflow runs, quality evidence, and a provenance-preserving memory foundation with bounded HOT context. M3 adds optional, sanitized local Codex hook telemetry and restart reconciliation; it does not store prompts, transcripts, tool inputs, or tool outputs. Cathulu does not run workers or change Codex settings automatically.

## Components

- `cathulu-core`: Rust domain validation, additive SQLite migrations, workflow and memory persistence, and Git reconciliation.
- `cathulu-mcp`: official RMCP stdio server with named domain tools and JSON resources; it does not expose SQL or table CRUD.
- `apps/desktop`: Tauri 2 and React/TypeScript read-only dashboard plus separate model/effort settings for projects, objectives, runs, quality, and compact memory health.
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

The local database defaults to `%LOCALAPPDATA%\Cathulu\cathulu.sqlite3`. If you override `CATHULU_DATA_DIR`, give the same absolute directory to both the MCP server and desktop app so they open the same database. User Codex settings resolve from `CODEX_HOME` or `%USERPROFILE%\.codex`; project settings can be written only for a repository registered in Cathulu and are read by Codex only when that repository is trusted.

## Connect Codex

Follow [docs/local-mcp.md](docs/local-mcp.md) to register the built MCP executable in Codex. Optional hook configuration is documented there and remains user-controlled; Cathulu does not edit Codex settings.

## Optional specialist providers

Jev and ScaleDown shadow integrations are disabled by default and require both a local API key and explicit per-run remote consent. See [the provider setup guide](docs/specialist-provider-setup.md) before configuring a test run.

## Current milestone

M1-M9, M11, and M15-M18a are published to `main`. M9's separate Codex model/effort settings passed local validation at `f2381ae`, and M11's optional local Graphify retrieval passed its evidence gate at `e18dc46`. The live owner Codex config and runtime-effective model remain untouched/unverified. PLAN-004 defines the optional specialist-model layer. Jev and ScaleDown shadow adapters are implemented with disabled-by-default, per-run remote consent; loopback contract tests and public MCP missing-key fallback checks do not contact providers, and no live provider call or product-value result is claimed. M18 comparative scoring still needs representative baseline runs. See [the Phase 1 first-slice plan](docs/plans/PLAN-001-phase1-first-slice.md), [the Phase 3 delegation plan](docs/plans/PLAN-002-phase3-codex-delegation.md), and [the specialist-model plan](docs/plans/PLAN-004-optional-specialist-model-optimization.md) for status and evidence.

