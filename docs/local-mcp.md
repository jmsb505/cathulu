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

To record native subagent lifecycle metadata, add `SubagentStart` and `SubagentStop` hooks that call the same `record_codex_hook_event` tool with this allowlist:

```json
{
  "hook_event_name": "SubagentStart",
  "session_id": "${session_id}",
  "turn_id": "${turn_id}",
  "agent_id": "${agent_id}",
  "agent_type": "${agent_type}",
  "model": "${model}"
}
```

Use `SubagentStop` for the stop entry. Cathulu records subagent events only when the parent session/turn already maps to a run. The tool returns an `eventId`; call `associate_delegation_worker` with that start-event ID and the dispatched delegation ID to make an explicit run-checked association. Repeated delivery is idempotent. Raw session and agent IDs are hashed; the documented active model may be stored, while effective effort remains unknown. Transcript paths, prompts, tool bodies, and last assistant messages are not accepted.

MCP tool hooks use an already connected server. Review and trust the hook in Codex before it runs. The snippet omits `SessionStart` because that event can fire before MCP is ready, and `SessionEnd` because MCP hooks do not support it. If telemetry is unavailable, the normal MCP and desktop state paths continue to work. See the [Codex Hooks guide](https://learn.chatgpt.com/docs/hooks) for current event fields and hook trust behavior.

## 4. Use Cathulu through Codex

### Required for Main Overseer, Project Managers, and Workers

Follow the role and memory protocol in [agent-operating-contract.md](agent-operating-contract.md). Main Overseer coordinates only; it does not implement. Assign one Project Manager per active project, and limit each Project Manager assignment to two worker agents total. Prepare a `sub_overseer` delegation for the PM; this keeps the stored role compatible while creating an explicit Project Manager assignment and reserving that project's active-manager slot. Prepare each worker with `parent_delegation_id` set to that PM delegation. Before each PM or Worker dispatch, call `search_memory_candidates` with the prepared `delegation_id`, the project/objective/task scope, and that receiver's stable logical scope (`receiver_scope_type: "scope"`). This records a compact successful retrieval event and returns candidate IDs/hashes without bodies. Transfer only selected candidates with `prepare_context_transfer`; an unchanged known memory is recorded as `already_known`, while changed content is delivered as a delta. Then call `record_delegation_activation` with the search outcome and exact transfer/known-by references, or use `no_relevant_memory` with no references after a successful search. Call `mark_delegation_dispatched` only after the latest receipt is ready. A missing or incomplete receipt blocks dispatch. Do not inject COLD or stale memory automatically, replay transcripts, or treat a worker report as verified success.

M21 repository-free probe confirmed this client can spawn a Worker from a Project Manager and that Main, PM, and Worker can reach Cathulu MCP. Tool availability differed by role: the PM could read `cathulu://dashboard`, but `read_memories` was unavailable in its tool context; the Worker successfully called `read_memories`. HOT/WARM memory was empty, and the run recorded no relevant memory for the PM. `prepare_context_transfer` requires a memory ID, so it cannot create an empty transfer receipt. M23 now requires an actual bounded `search_memory_candidates` call associated with the exact delegation run and receiver before a ready activation receipt can be recorded; a boolean claim or generic resource read is insufficient. The current client still needs a real PM activation attempt to demonstrate that the bounded search tool is available in that role. Until it succeeds, leave that PM prepared and blocked. No hook events or hook metadata surfaced; do not infer a hierarchy edge from agent names or nested spawn alone. See [ADR-0009](decisions/ADR-0009-bounded-project-agent-hierarchy.md) and [PLAN-006](plans/PLAN-006-bounded-project-agent-hierarchy.md).

M22 admission checks are enforced transactionally in SQLite. A second hierarchy-aware PM cannot be prepared while the project slot is active. The PM's slot is released by `complete_project_manager_assignment` after every child is reported or cancelled, or by cancelling the PM when no child remains unfinished. A completed/cancelled PM cannot accept new children. The two-worker count includes every child delegation ever prepared for that PM, including cancelled or replaced workers. Legacy delegation rows remain readable and are not assigned inferred parents; only new hierarchy-aware PM records can own new worker children. Rejected over-cap attempts appear as `delegation_admission_rejected` events on the run. These records report native Codex handoffs; they do not spawn or control agents.

M23 uses additive schema version 11. `search_memory_candidates` can receive a delegation ID and records only the run, source scope, receiver scope, result count, and a hash of returned IDs/hashes. `record_delegation_activation` stores an append-only receipt and memory references. Each reference must name current accepted, non-COLD knowledge in the same project/task scope and prove either a matching context transfer in this run to this receiver or a matching `memory_known_by` record. HOT references also pass the existing scope budget. A ready receipt must reference a successful bounded-search event for that delegation and receiver; incomplete receipts are preserved but cannot dispatch. The delegation resource exposes the latest receipt and its search event ID. Cathulu cannot inspect Codex's per-agent tool inventory or identify which native agent made an MCP call. The receipt therefore proves a scoped search was recorded, not which agent made it; the operating contract requires the receiver to perform the activation call, and a runtime probe must still confirm the correct role has the tool.

- `register_project` accepts a local Git repository path. It stores the canonical repository root and an initial branch, HEAD, dirty flag, and changed-path list.
- `reconcile_project` refreshes the Git snapshot for a registered project ID.
- `trace_code_path` optionally traces a directed path between exact code symbols in `<repository>\graphify-out\graph.json`. It defaults to three hops (maximum four), accepts optional repository-relative source/target file filters, and returns node IDs, edge relation/confidence, and current source locations without source contents. Graphify metadata is recorded in SQLite; a changed or unavailable snapshot invalidates only memories that name the previous graph snapshot. Missing, stale, malformed, or unsupported snapshots return no candidates so current SQLite/SQL/FTS and source inspection remain the fallback.
- `cathulu://projects/{id}/graphify-snapshot` reads the current Graphify snapshot identity and freshness state. Cathulu does not install or run Graphify. If Graphify 0.9.82 is already installed and you want to create an optional local code-only snapshot, run this from the repository root in PowerShell:

  ```powershell
  $env:GRAPHIFY_BACKEND = "none"
  $env:GRAPHIFY_QUERY_LOG_DISABLE = "1"
  python -m graphify extract . --code-only --out .
  ```

  The supported export is schema 1 from Graphify 0.9.82. Newer versions remain unavailable until their export contract is reviewed. The generated `graphify-out` directory is a local retrieval artifact; keep it out of source commits unless the repository owner explicitly wants to version it.
- Workflow tools create objectives and tasks, record immutable decisions and workflow versions, start runs, record quality checks and local evidence references, and finish runs using mandatory-check results.
- `prepare_delegation` stores a typed v1 task packet against a matching project, task, and running/blocked run. Packets are limited to 16 KiB and include an objective, acceptance criteria, constraints, repository-relative allowed paths, current memory/artifact references, and requested report fields. Invalid or cross-scope references are rejected. New delegations capture the actual registered Git HEAD as the immutable comparison baseline; existing records are not backfilled.
- `mark_delegation_dispatched` and `cancel_delegation` append validated lifecycle events. These only record the owner's native Codex action or Cathulu cancellation state; they do not spawn or signal a worker and do not change parent-run quality.
- `submit_delegation_report` stores one idempotent, schema-v1 report (maximum 16 KiB) for a dispatched delegation. Changed paths must fit the packet's allowed scope; referenced artifacts and quality checks must belong to the parent run. The report remains a worker claim, even when its own checks say `passed`.
- `review_delegation_report` appends an idempotent `accepted`, `rejected`, or `needs_follow_up` review. A report with no review is `reported`. Review state does not accept code or change parent-run quality; mandatory parent checks still gate verified success.
- `reconcile_delegation_worktree` reads an already-existing linked worktree and records its branch, observed HEAD, cleanliness, changed paths, and report verification. The worktree must be listed by the registered repository, be distinct from its registered root, and share its Git common directory. It verifies report commits are between the delegation baseline and observed HEAD, and report paths occur in the observed changes. Mismatches remain evidence. It never creates, moves, removes, or merges worktrees; the absolute local path is retained only in SQLite and the resource returns its hash.
- `associate_delegation_worker` explicitly binds a dispatched delegation to a recorded `SubagentStart` event from the same parent run. Later `SubagentStop` evidence attaches through hashed session/turn/agent identifiers. Requested model/effort stay separate from observed model; effort remains unknown unless a supported source is added.
- Requested model/effort values are preferences only. Idempotency and optional external correlation values are SHA-256 hashed and the raw values are discarded. Effective model/effort values are not inferred.
- `record_codex_hook_event` accepts only an allowlist of Codex lifecycle metadata. One observed session/turn maps to a run; stopping or interrupting it records an inconclusive outcome, never verified success. Subagent events attach only to an already-mapped parent run and never create a run from worker identity alone.
- Memory tools create provenance-bearing candidates, review candidates, change temperature or scope activation, set a HOT byte budget, read scope memories, and prepare explicit context transfers. `search_memory_candidates` returns compact HOT/WARM candidate IDs and hashes; when used for a hierarchy delegation it records the successful retrieval event required by activation. A canonical memory requires a provenance source; current reads omit rejected, stale, superseded, and COLD memory by default.
- `record_delegation_activation` records a revisioned ready/incomplete memory activation receipt for hierarchy-aware Project Managers and child Workers. Readiness requires a successful bounded search, plus current exact-scope transfer/known-by references or an explicit `no_relevant_memory` outcome; `mark_delegation_dispatched` rejects a missing or incomplete receipt.
- Context transfers persist hashes, scope IDs, class/temperature, byte/token counts, retrieval method, and first-time/reintroduced state. The transfer ledger does not retain raw transferred bodies; the explicit transfer tool returns the body only when a full or delta transfer is requested.
- Events are append-only. Decisions and workflow versions are immutable; new accepted decisions or workflow versions are created as new records.
- `cathulu://projects` reads the portfolio snapshot.
- `cathulu://projects/{id}` reads one project's snapshot.
- `cathulu://dashboard` reads the compact dashboard snapshot.
- `cathulu://runs/{id}`, `cathulu://decisions/{id}`, `cathulu://memories/{id}`, and `cathulu://transfers/{id}` read one durable record.
- `cathulu://delegations/{id}` reads one structured task packet, its stable packet hash, requested settings, artifact/memory IDs, the latest activation receipt and search-event reference, append-only lifecycle history, any worker report and review history, the captured baseline and latest worktree verification, and explicitly associated worker telemetry. It does not return the absolute worktree path or raw hook identifiers.
- `cathulu://objectives/{id}`, `cathulu://tasks/{id}`, `cathulu://dependencies/{id}`, and `cathulu://artifacts/{id}` read current domain records and protection metadata.
- `cathulu://events/run/{id}` reads a run's append-only event history.

The dashboard polls the same local store through the read-only `dashboard_snapshot` Tauri command. It shows objectives, blockers, recent runs, failed or inconclusive checks, compact memory health, and up to ten recently active delegations with their report/review status, parent-run status, coarse worktree verification, and observed worker model when available. Worker-reported checks and outcomes remain distinct from Git verification and parent-run verification. It does not provide registration, reconciliation, review, dispatch, memory, model, or effort controls.

### Manual native worktree handoff and recovery

Cathulu records evidence for a worktree already managed by native Codex/Git. After dispatch, use the worker's existing linked worktree and submit its structured report. Then call `reconcile_delegation_worktree` with that worktree's path. Cathulu accepts only a path that resolves to an exact worktree listed by the registered project and shares its Git common directory. The dashboard and delegation resource expose a path hash, not the absolute local path.

If reconciliation reports a missing path or a repository/common-directory mismatch, inspect `git worktree list --porcelain` from the registered repository and confirm the path still exists. Restore or reattach the worktree through native Codex/Git, then retry reconciliation. If the worker worktree is gone, leave its Git verification unavailable and review the saved report and evidence manually. Cathulu does not recreate, relocate, or remove worker worktrees.

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

