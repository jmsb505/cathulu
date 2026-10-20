# Connect the local MCP server to Codex

Cathulu exposes local repository registration and reconciliation through an MCP stdio server. The server writes Cathulu's SQLite state; it reads Git metadata and does not modify repository contents. MCP tools should remain approval-gated in Codex.

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
- `cathulu://projects` reads the portfolio snapshot.
- `cathulu://projects/{id}` reads one project's snapshot.

The dashboard polls the same local store and remains read-only. It does not provide registration, reconciliation, model, or effort controls.

## Data location

By default the database is `%LOCALAPPDATA%\Cathulu\cathulu.sqlite3`. If you set `CATHULU_DATA_DIR`, use the same absolute path in both the MCP server and desktop app environments. Otherwise the dashboard may open the default database while MCP writes to the override. Do not add secrets, full prompts, transcripts, or repository file contents to this first milestone; it stores only project identity and compact Git state.

Codex uses `[mcp_servers.<name>]` tables in `config.toml` for local stdio servers. Review the current [Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) before changing this setup.
