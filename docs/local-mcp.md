# Connect the local MCP server to Codex

Cathulu exposes local project, workflow, quality, evidence, and memory operations through an MCP stdio server. The server writes Cathulu's SQLite state and reads Git metadata; it does not modify repository contents. Keep MCP tools approval-gated in Codex.

## 1. Build the server

From the repository root, run:

```powershell
cargo build --release --bin cathulu-mcp
```

The executable is created at `target\release\cathulu-mcp.exe`.

## 2. Add the server to Codex

Open Codex Settings and select **Open config.toml**, or edit the Codex configuration file directly. Add this table and replace the path with the absolute location of your built executable:

```toml
[mcp_servers.cathulu]
command = "C:\\path\\to\\cathulu\\target\\release\\cathulu-mcp.exe"
args = []
startup_timeout_sec = 20
tool_timeout_sec = 60
default_tools_approval_mode = "prompt"
```

Restart Codex or reload MCP servers after saving. You can also use Codex's MCP settings interface when available.

## 3. Use Cathulu through Codex

- `register_project` accepts a local Git repository path. It stores the canonical repository root and an initial branch, HEAD, dirty flag, and changed-path list.
- `reconcile_project` refreshes the Git snapshot for a registered project ID.
- Workflow tools create objectives and tasks, record immutable decisions and workflow versions, start runs, record quality checks and local evidence references, and finish runs using mandatory-check results.
- Memory tools create provenance-bearing candidates, review candidates, change temperature or scope activation, set a HOT byte budget, read scope memories, and prepare explicit context transfers. A canonical memory requires a provenance source; current reads omit rejected, stale, superseded, and COLD memory by default.
- Context transfers persist hashes, scope IDs, class/temperature, byte/token counts, retrieval method, and first-time/reintroduced state. The transfer ledger does not retain raw transferred bodies; the explicit transfer tool returns the body only when a full or delta transfer is requested.
- Events are append-only. Decisions and workflow versions are immutable; new accepted decisions or workflow versions are created as new records.
- `cathulu://projects` reads the portfolio snapshot.
- `cathulu://projects/{id}` reads one project's snapshot.
- `cathulu://dashboard` reads the compact dashboard snapshot.
- `cathulu://runs/{id}`, `cathulu://decisions/{id}`, `cathulu://memories/{id}`, and `cathulu://transfers/{id}` read one durable record.
- `cathulu://objectives/{id}`, `cathulu://tasks/{id}`, `cathulu://dependencies/{id}`, and `cathulu://artifacts/{id}` read current domain records and protection metadata.
- `cathulu://events/run/{id}` reads a run's append-only event history.

The dashboard polls the same local store through the read-only `dashboard_snapshot` Tauri command. It shows objectives, blockers, recent runs, failed or inconclusive checks, and compact memory health. It does not provide registration, reconciliation, memory, model, or effort controls.

## Data location

By default the database is `%LOCALAPPDATA%\Cathulu\cathulu.sqlite3`. If you set `CATHULU_DATA_DIR`, use the same absolute path in both the MCP server and desktop app environments. Otherwise the dashboard may open the default database while MCP writes to the override. Cathulu retains local domain state, bounded event metadata, distilled memory records, provenance links, and artifact references. Do not add secrets, raw prompts, or full transcripts. Detailed artifacts should remain local and be referenced by local paths or artifact identifiers.

Codex uses `[mcp_servers.<name>]` tables in `config.toml` for local stdio servers. Review the current [Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) before changing this setup.
