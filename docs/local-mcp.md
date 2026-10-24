# Connect the local MCP server to Codex

## Terminal monitor

Run `target\mcp-current\cathulu-monitor.exe --watch` from a terminal you opened to see Cathulu's recorded project, role, attention, QA, and memory state. Use `--once` for a single human-readable snapshot or `--json` for the complete read-only dashboard projection. Set `CATHULU_DATA_DIR` to select another existing Cathulu store. The monitor does not start Codex, open a window, or write to SQLite. `--watch` refreshes in place and stops with Ctrl+C. MCP stdout remains reserved for JSON-RPC.


Cathulu exposes local project, workflow, quality, evidence, and memory operations through an MCP stdio server. The server writes Cathulu's SQLite state and reads Git metadata; it does not modify repository contents. Keep MCP tools approval-gated in Codex.

## Persistent Main and Project Manager roles

Read `cathulu://roles` to get the authoritative logical-role catalog. It contains one `main-overseer` role and one `project-manager:<project_id>` role per registered project. Each record supplies the stable memory source scope, receiver/known-by scope, and versioned tool-profile key. `cathulu://dashboard` returns the same role catalog alongside current project, assignment, and worker evidence.

A Codex chat is a temporary occupant of a logical role. A replacement Main chat resumes `main-overseer`; a replacement Project Manager chat resumes the existing PM role and assignment when one is active. It must reuse that role's memory receiver scope rather than making a chat-specific scope. For a PM, pass the raw ID (for example `project-manager:42`) with `receiver_scope_type: "scope"`; the persisted receiver key is `scope:project-manager:42`. Cathulu rejects new PM search receipts and activation receipts that use another receiver scope, and checks that a ready receipt matches the role before dispatch. This preserves known-by state across chats without replaying transcripts or duplicating role records.

Only one PM task assignment can be active per project, and each assignment keeps the existing two-worker total. Finish or cancel the old assignment before preparing a new one; a replacement chat continuing the same assignment does not prepare a second PM seat. Workers remain task-scoped children. `tool_profile_key` names the role's intended tool contract; it does not confirm that a Codex chat has a particular MCP tool or native chat identity. Verify tool availability in the current Codex client and report unknown identity honestly.

## Hierarchical plans, reviews, and owner actions

Schema v13 adds deterministic workflow-order gates. Before preparing a PM, Main records a separate `main` plan for that project's run with `record_role_work_plan`. A PM then records a `project_manager` plan with bounded `worker_tasks`; Main reviews it through `review_role_work_plan`. A Worker child must pass its exact `plan_task_key` to `prepare_delegation`, and its packet must match the reviewed task. `dispatch_prepared_delegations` queues prepared assignments for the local Codex dispatcher; it returns after durable queue acceptance and does not wait for a role turn to finish.

Workers submit reports with `submit_delegation_report`; their PM reviews via `review_delegation_report` with `reviewer_role: "project_manager"`. A PM cannot submit its result until its Worker reports are accepted or cancelled. Main reviews the PM plan and report using `review_role_work_plan` and `review_delegation_report` with `reviewer_role: "main_overseer"`. PM completion is blocked until Main accepts the PM report. Deterministic QA remains separate and authoritative.

Use `record_delegation_update` for a keyed `action_required`: a Worker action routes to its PM and a PM action routes to Main. Main raises `request_owner_approval` only when an unresolved PM escalation crosses the Owner's authority boundary. It is exposed separately in `cathulu://dashboard` and `cathulu://owner-approvals/{id}`. The MCP does not provide a tool for recording an Owner decision. Open the Main or relevant PM node in Cathulu's desktop dashboard and use the two-step approve/decline control for the exact displayed request. Cathulu checks the request hash has not changed, records one append-only decision with origin `desktop_owner_ui`, and never performs the approved operation. The dispatcher routes the decision to Main, which relays it only to the affected PM; the underlying action remains open until its outcome is reported. Unresolved action/approval state blocks PM result closure. Do not label ordinary PM/Worker routing as “Needs you.”

The hierarchical tools enforce sequence and scope from stored records. Ordinary MCP calls do not authenticate the Codex chat that made them; dispatcher-created role turns use a short-lived capability scoped to the logical role, project, and assignment. The Main event thread has review/coordination tools but no implementation or Worker-dispatch tools. The user's project-workspace tool catalog, provider behavior, and visible Codex thread ownership still need live verification. The dispatcher checks each thread's required Cathulu tools before sending a model turn and blocks that task if any are missing. Never emulate asynchronous fan-out with Main waiting on PMs.

## 1. Build the matched server and dashboard

From the repository root, use the canonical matched-release build. It builds the frontend and both release executables with `custom-protocol`, stages and hash-checks the pair, then publishes it to the stable directory already used by Codex:

```powershell
./scripts/build-current.ps1
```

Codex should point to `target\mcp-current\cathulu-mcp.exe`. That path is a stable Windows junction to a versioned directory containing the matched `cathulu-mcp.exe` and `cathulu-desktop.exe`. `./scripts/build-current.ps1 -BuildOnly` builds and hashes a staged pair without advancing the junction. Each successful normal build advances the junction, so Codex's configured path stays fixed while subsequent starts load the latest pair. An already-running MCP process stays on its in-memory version until the Cathulu server is restarted from **Codex Settings → MCP servers → Cathulu → Restart**; do this when it will not interrupt active orchestration. The script never kills or restarts Codex and does not steal foreground focus.

## 2. Add the server to Codex

Open Codex Settings and select **Open config.toml**, or edit the Codex configuration file directly. Add this table and replace the path with the absolute location of your built executable:

```toml
[mcp_servers.cathulu]
command = "C:\\path\\to\\cathulu\\target\\mcp-current\\cathulu-mcp.exe"
args = []
startup_timeout_sec = 20
tool_timeout_sec = 60
default_tools_approval_mode = "prompt"
```

When Codex starts Cathulu MCP, it starts only the stdio service and its background role dispatcher. It never launches the sibling desktop or a terminal window. The dispatcher launches a hidden Codex app-server only when queued work exists. Its process disables MCP servers from the user's global Codex config; each role thread receives only its scoped Cathulu MCP configuration. Background CLI processes suppress Windows console creation. Keep the matched release executables together. Start `target\mcp-current\cathulu-desktop.exe` manually if you want the desktop dashboard, or use the terminal monitor documented above. Restart Codex or reload MCP servers after saving configuration.

Workflow definitions and role work plans are JSON objects. Their MCP input schemas declare that object shape explicitly so Codex can expose `create_workflow_version` and `record_role_work_plan`; Cathulu still validates the exact contents when those tools are called.

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

Follow the role and memory protocol in [agent-operating-contract.md](agent-operating-contract.md). Main Overseer coordinates only; it does not implement. Assign one Project Manager per active project, and limit each Project Manager assignment to two worker agents total. The Project Manager and every Worker must receive the current task/run IDs, use Cathulu MCP to read scoped memory and retrieve relevant WARM candidates, and record selected context with `prepare_context_transfer` before it is handed off. Include only relevant memory/artifact IDs in `prepare_delegation`; if retrieval finds nothing useful, state that explicitly. Do not inject COLD or stale memory automatically, replay transcripts, or treat a worker report as verified success.

M21's repository-free capability probe confirmed that the current client can spawn a Worker from a Project Manager and that Main, PM, and Worker can reach Cathulu MCP. Tool availability differed by role: the PM could read `cathulu://dashboard`, but `read_memories` was unavailable in its tool context; the Worker successfully called `read_memories`. HOT/WARM memory was empty, and the run recorded “no relevant memory” for the PM. `prepare_context_transfer` requires a memory ID, so it cannot currently create an empty transfer receipt. M23 must add a structured no-relevant-memory activation receipt and validate role-specific tool availability before memory-gated dispatch. No hook events or hook metadata surfaced in the probe; do not infer a hierarchy edge from agent names or nested spawn alone. See [ADR-0009](decisions/ADR-0009-bounded-project-agent-hierarchy.md) and [PLAN-006](plans/PLAN-006-bounded-project-agent-hierarchy.md).

### No-wait orchestration status

The M21 result proves one nested PM → Worker activation only. M40/M41 provide a default-on local app-server dispatcher with a durable SQLite outbox, singleton lease, persistent logical role-thread bindings, scoped capabilities, and an actionable event inbox. Set `CATHULU_ROLE_DISPATCHER_ENABLED=0` to disable it. Normal MCP startup starts no app-server; the hidden app-server runs only when queued work exists. Prepared PM/Worker assignments are submitted independently, with no wait for completion. New actionable events route Worker → PM → Main → Owner as needed, and Main reviews/Owner decisions return only to the affected PM. Routine checkpoints remain passive. The Main event thread uses one global logical binding, so a replacement thread occupies the same Main role. The dashboard and `cathulu://dispatches` expose dispatch/inbox state. Missing Cathulu tools block before model execution; app-server approval/interaction requests are rejected, never auto-approved. Schema-v17 cursor initialization prevents historical events from being replayed after migration. Live provider/account behavior, target project workspace/tool parity, UI ownership, and real parallel/restart behavior are not yet verified. After the owner reloads the matched build, validate those capabilities using an explicitly assigned safe project task. Never emulate asynchronous orchestration with `wait_threads`, polling, or one long-running Main turn.

### Role authority and escalation

Workers raise project questions to their Project Manager. PMs resolve routine choices within the owner's task scope and escalate only issues they cannot resolve or actions requiring owner authority. Main resolves bounded cross-project coordination and reversible choices within the explicit scope. Owner approval is required before creating an account; creating, entering, or changing a password, API key, or other credential; changing permissions/access; spending money; disclosing data outside the approved local boundary; taking destructive or irreversible actions; deploying, releasing, or publishing to production; making third-party commitments; or exceeding the authorized task scope. Pause only the affected project while awaiting the owner; keep unrelated projects moving. Routine progress checkpoints update Cathulu/dashboard state and must not wake Main.

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
- `record_delegation_update` appends a bounded, idempotent progress/waiting checkpoint or keyed owner-action request/resolution for a dispatched delegation. Use it at meaningful changes and about every 10 minutes during sustained work, not for every tool call. Updates are self-reported and do not establish that code changed, a command ran, or QA passed. An action request remains open until `action_resolved` uses the same action key. The dashboard marks signals older than 15 minutes as unconfirmed; this does not mean the agent is verified stopped.
- `review_delegation_report` appends an idempotent `accepted`, `rejected`, or `needs_follow_up` review. A report with no review is `reported`. Review state does not accept code or change parent-run quality; mandatory parent checks still gate verified success.
- `reconcile_delegation_worktree` reads an already-existing linked worktree and records its branch, observed HEAD, cleanliness, changed paths, and report verification. The worktree must be listed by the registered repository, be distinct from its registered root, and share its Git common directory. It verifies report commits are between the delegation baseline and observed HEAD, and report paths occur in the observed changes. Mismatches remain evidence. It never creates, moves, removes, or merges worktrees; the absolute local path is retained only in SQLite and the resource returns its hash.
- `associate_delegation_worker` explicitly binds a dispatched delegation to a recorded `SubagentStart` event from the same parent run. Later `SubagentStop` evidence attaches through hashed session/turn/agent identifiers. Requested model/effort stay separate from observed model; effort remains unknown unless a supported source is added.
- Requested model/effort values are preferences only. Idempotency and optional external correlation values are SHA-256 hashed and the raw values are discarded. Effective model/effort values are not inferred.
- `record_codex_hook_event` accepts only an allowlist of Codex lifecycle metadata. One observed session/turn maps to a run; stopping or interrupting it records an inconclusive outcome, never verified success. Subagent events attach only to an already-mapped parent run and never create a run from worker identity alone.
- Memory tools create provenance-bearing candidates, review candidates, change temperature or scope activation, set a HOT byte budget, read scope memories, and prepare explicit context transfers. A canonical memory requires a provenance source; current reads omit rejected, stale, superseded, and COLD memory by default.
- Context transfers persist hashes, scope IDs, class/temperature, byte/token counts, retrieval method, and first-time/reintroduced state. The transfer ledger does not retain raw transferred bodies; the explicit transfer tool returns the body only when a full or delta transfer is requested.
- Events are append-only. Decisions and workflow versions are immutable; new accepted decisions or workflow versions are created as new records.
- `cathulu://projects` reads the portfolio snapshot.
- `cathulu://projects/{id}` reads one project's snapshot.
- `cathulu://dashboard` reads the compact dashboard snapshot.
- `cathulu://runs/{id}`, `cathulu://decisions/{id}`, `cathulu://memories/{id}`, and `cathulu://transfers/{id}` read one durable record.
- `cathulu://delegations/{id}` reads one structured task packet, its stable packet hash, requested settings, artifact/memory IDs, append-only lifecycle history, any worker report and review history, the captured baseline and latest worktree verification, and explicitly associated worker telemetry. It does not return the absolute worktree path or raw hook identifiers.
- `cathulu://objectives/{id}`, `cathulu://tasks/{id}`, `cathulu://dependencies/{id}`, and `cathulu://artifacts/{id}` read current domain records and protection metadata.
- `cathulu://events/run/{id}` reads a run's append-only event history.

The dashboard reads the same local store through `dashboard_snapshot`. It shows one project-spanning role graph, recent agent checkpoints, unresolved explicit owner actions, stale/unconfirmed activity signals, blocked tasks, failed required checks, compact memory health, and up to ten recently active delegations with their report/review status, parent-run status, coarse worktree verification, and observed worker model when available. Agent-reported updates remain distinct from hook telemetry, Git verification, artifacts, and parent-run QA. The selected Main/PM dossier has one narrowly scoped control to record an explicit Owner approval or decline; the UI cannot dispatch work or execute the action. Registration, reconciliation, role reviews, dispatch, memory, model, and effort remain outside the dashboard. The compact monitor remains read-only and signals pending Owner actions or projects with no fresh agent activity.

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

MCP startup is always headless. The former `CATHULU_DISABLE_DASHBOARD_AUTOLAUNCH` setting is obsolete and has no effect.

Codex uses `[mcp_servers.<name>]` tables in `config.toml` for local stdio servers. Review the current [Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) before changing this setup.
