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

## 3. Optional local Codex activity hooks

After the `cathulu` MCP server is connected, Codex can send a small allowlist of lifecycle metadata to Cathulu. Add the following to a user-controlled `hooks.json` or the `hooks` table in Codex config. This repository does not edit your Codex settings automatically.

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "mcp_tool",
            "server": "cathulu",
            "tool": "record_codex_hook_event",
            "input": {
              "hook_event_name": "UserPromptSubmit",
              "session_id": "${session_id}",
              "turn_id": "${turn_id}",
              "cwd": "${cwd}"
            },
            "timeout": 5
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "hooks": [
          {
            "type": "mcp_tool",
            "server": "cathulu",
            "tool": "record_codex_hook_event",
            "input": {
              "hook_event_name": "PostToolUse",
              "session_id": "${session_id}",
              "turn_id": "${turn_id}",
              "cwd": "${cwd}",
              "tool_name": "${tool_name}",
              "tool_use_id": "${tool_use_id}"
            },
            "timeout": 5
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "mcp_tool",
            "server": "cathulu",
            "tool": "record_codex_hook_event",
            "input": {
              "hook_event_name": "Stop",
              "session_id": "${session_id}",
              "turn_id": "${turn_id}",
              "cwd": "${cwd}"
            },
            "timeout": 5
          }
        ]
      }
    ],
    "Interrupt": [
      {
        "hooks": [
          {
            "type": "mcp_tool",
            "server": "cathulu",
            "tool": "record_codex_hook_event",
            "input": {
              "hook_event_name": "Interrupt",
              "session_id": "${session_id}",
              "turn_id": "${turn_id}",
              "cwd": "${cwd}"
            },
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

This hook setup records only event names and opaque IDs, a temporary working-directory lookup, and a coarse tool category. Cathulu hashes identifiers and stores no prompt, transcript path, tool input, tool response, or working-directory string. It records an observed turn as `inconclusive` when Codex stops or is interrupted; only Cathulu's existing quality checks can establish verified success. Hooks do not fill in token counts or context-transfer bodies; `prepare_context_transfer` remains the explicit transfer ledger operation.

MCP tool hooks use an already connected server. Review and trust the hook in Codex before it runs. The snippet omits `SessionStart` because that event can fire before MCP is ready, and `SessionEnd` because MCP hooks do not support it. If telemetry is unavailable, the normal MCP and desktop state paths continue to work. See the [Codex Hooks guide](https://learn.chatgpt.com/docs/hooks) for current event fields and hook trust behavior.

## 4. Use Cathulu through Codex

- `register_project` accepts a local Git repository path. It stores the canonical repository root and an initial branch, HEAD, dirty flag, and changed-path list.
- `reconcile_project` refreshes the Git snapshot for a registered project ID.
- Workflow tools create objectives and tasks, record immutable decisions and workflow versions, start runs, record quality checks and local evidence references, and finish runs using mandatory-check results.
- `prepare_delegation` stores a typed v1 task packet against a matching project, task, and running/blocked run. Packets are limited to 16 KiB and include an objective, acceptance criteria, constraints, repository-relative allowed paths, current memory/artifact references, and requested report fields. Invalid or cross-scope references are rejected.
- `mark_delegation_dispatched` and `cancel_delegation` append validated lifecycle events. These only record the owner's native Codex action or Cathulu cancellation state; they do not spawn or signal a worker and do not change parent-run quality.
- Requested model/effort values are preferences only. Idempotency and optional external correlation values are SHA-256 hashed and the raw values are discarded. Effective model/effort values are not inferred.
- `record_codex_hook_event` accepts only an allowlist of Codex lifecycle metadata. One observed session/turn maps to a run; stopping or interrupting it records an inconclusive outcome, never verified success.
- Memory tools create provenance-bearing candidates, review candidates, change temperature or scope activation, set a HOT byte budget, read scope memories, and prepare explicit context transfers. A canonical memory requires a provenance source; current reads omit rejected, stale, superseded, and COLD memory by default.
- Context transfers persist hashes, scope IDs, class/temperature, byte/token counts, retrieval method, and first-time/reintroduced state. The transfer ledger does not retain raw transferred bodies; the explicit transfer tool returns the body only when a full or delta transfer is requested.
- Events are append-only. Decisions and workflow versions are immutable; new accepted decisions or workflow versions are created as new records.
- `cathulu://projects` reads the portfolio snapshot.
- `cathulu://projects/{id}` reads one project's snapshot.
- `cathulu://dashboard` reads the compact dashboard snapshot.
- `cathulu://runs/{id}`, `cathulu://decisions/{id}`, `cathulu://memories/{id}`, and `cathulu://transfers/{id}` read one durable record.
- `cathulu://delegations/{id}` reads one structured task packet, its stable packet hash, requested settings, artifact/memory IDs, and append-only lifecycle history.
- `cathulu://objectives/{id}`, `cathulu://tasks/{id}`, `cathulu://dependencies/{id}`, and `cathulu://artifacts/{id}` read current domain records and protection metadata.
- `cathulu://events/run/{id}` reads a run's append-only event history.

The dashboard polls the same local store through the read-only `dashboard_snapshot` Tauri command. It shows objectives, blockers, recent runs, failed or inconclusive checks, and compact memory health. It does not provide registration, reconciliation, memory, model, or effort controls.

Both the desktop app and MCP server attempt to refresh registered Git snapshots at process startup. A missing repository is reported on stderr while other projects continue to reconcile and the application remains available.

## 5. Memory lifecycle and local maintenance

- `search_memory_candidates` returns IDs, hashes, HOT candidates, and selective WARM matches. It does not return WARM bodies; call `prepare_context_transfer` for an auditable full or delta transfer. COLD, stale, invalidated, and superseded memories are not injected.
- `consolidate_memories` creates a reviewable canonical candidate from accepted episodic/evidence memories while retaining their memory IDs, provenance, and artifact links.
- `run_memory_gardener` records deterministic, reviewable proposals. `review_memory_maintenance_proposal` can apply only safe memory class/temperature changes. Neither tool collects evidence.
- `create_memory_policy`, `record_memory_policy_evaluation`, and `compare_memory_policy_experiment` record immutable policy versions and compare measured results tied to completed, quality-classified Cathulu runs.

Build the local owner maintenance utility from the repository root:

```powershell
cargo build --release --bin cathulu-maintain
```

It uses the same `%LOCALAPPDATA%\Cathulu` database as the MCP server unless `CATHULU_DATA_DIR` is set. Preview evidence collection before configuring retention:

```powershell
target\release\cathulu-maintain.exe evidence-gc preview
```

Collection is disabled until an owner configures a minimum age. To configure a 30-day interval, create extraction receipts for eligible evidence as needed, review the preview, and then run the interactive confirmation:

```powershell
# Replace the example IDs with the artifact and accepted canonical/aggregate memory IDs.
target\release\cathulu-maintain.exe evidence-gc set-retention 2592000
target\release\cathulu-maintain.exe evidence-gc mark-extracted 17 42
target\release\cathulu-maintain.exe evidence-gc preview
target\release\cathulu-maintain.exe evidence-gc collect
```

The prompt requires `ARCHIVE N` for the exact previewed ID set. Cathulu rechecks policy, extraction, protected state, direct memory and delegation references, active holds, QA outcome, and legacy URI-bearing events before each change. It replaces only the local URI with a tombstone and preserves the artifact row, stable ID, hashes, extraction receipt, and external file. Collection does not delete external files or promise physical SQLite disk compaction. Use `evidence-gc hold ARTIFACT_ID REASON` and `evidence-gc release-hold HOLD_ID` to maintain an auditable investigation hold. `evidence-gc disable-retention` returns collection to its disabled-by-default state.

## Data location

By default the database is `%LOCALAPPDATA%\Cathulu\cathulu.sqlite3`. If you set `CATHULU_DATA_DIR`, use the same absolute path in both the MCP server and desktop app environments. Otherwise the dashboard may open the default database while MCP writes to the override. Cathulu retains local domain state, bounded event metadata, distilled memory records, provenance links, and artifact references. Do not add secrets, raw prompts, or full transcripts. Detailed artifacts should remain local and be referenced by local paths or artifact identifiers.

Codex uses `[mcp_servers.<name>]` tables in `config.toml` for local stdio servers. Review the current [Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) before changing this setup.
