# CC-01 — Task / resume contract critique (M48 / M49)

Agent: Claude Code lane, PLAN-021 wave 0. Advisory input to CX-01 / CX-02; it changes no acceptance on its own and no source.
Baseline: branch `codex/manual-dual-m48`, HEAD `72815dc` (clean, `git status --porcelain --untracked-files=all` empty when read). Planning files are tracked.
Read: AGENTS.md, ADR-0023, PLAN-020 (M48/M49), PLAN-021, `docs/readiness/definition-gate.md`, `docs/pivot-migration.md` §§12, 14, 18, 22, 27-30, `docs/project-definition.md` pivot invariants; code in `src/workflow.rs`, `src/migrations.rs` (v2 tables), `src/mcp.rs`, `src/dispatcher.rs`, `src/delegation.rs`, `src/lib.rs`, `src/memory.rs`, `src/dashboard.rs`, `src/events.rs`.

Status of every claim below: **source-read only**. Nothing was compiled or run; no existing test was traced (`tests/mcp_vertical_slice.rs` restart fixture was not read). The packet's "expected" results are proposals for CX to confirm, not behavior that exists.

## 1. What the code does today (facts the examples rely on)

| # | Observation | Where |
| --- | --- | --- |
| F1 | `tasks.status` is written in exactly one place, `set_task_status`. It accepts any of `queued/active/blocked/completed/failed/cancelled` from any current state, including reopening a terminal task. A same-status call is a silent no-op (no event). | `src/workflow.rs:307-345` |
| F2 | `start_run` checks only that the task belongs to the project/objective. It does not read task status, so a `completed` or `cancelled` task can start a run. `finish_run` never reads or writes the task. There is no link today between run `verified_success` and task `completed`. | `workflow.rs:638-652`, `755-916` |
| F3 | Task has no project column; project is derived through `objectives`. No `parent`, `priority`, `due`, `effort`, `revision`. `updated_at` is whole seconds. | `migrations.rs:22-30`, `workflow.rs:1278-1283` |
| F4 | Objective status changes do not touch its tasks. Objective statuses include `archived`; the MCP description omits it. | `workflow.rs:221-259`, `mcp.rs:551` |
| F5 | MCP `set_task_status` says "pending, active, blocked, completed, or cancelled". The DB/validator value is `queued`, and `failed` is also valid. `pending` is rejected. | `mcp.rs:567-569` vs `workflow.rs:310`, `migrations.rs:27` |
| F6 | Role capability allow-lists contain none of `create_task`, `set_task_status`, `record_dependency`. The filter is applied only `if let Some(capability)`; an unscoped client sees every tool. So today "Owner command" = any unscoped MCP client. | `dispatcher.rs:117-149`, `mcp.rs:1343-1365` |
| F7 | `ProjectSnapshot` has no status/priority/purpose. "Paused project" does not exist yet. | `lib.rs:46-56` |
| F8 | `reconcile_project` runs Git, **errors** if the repo cannot be inspected, writes the project row, and invalidates memories whose `freshness_git_commit` differs from HEAD. The stored `git_head` and `reconciled_at` are readable without any of that. | `lib.rs:404-471`, `memory.rs:1224-1254` |
| F9 | Event payloads are capped at 32 KiB; events are append-only. | `events.rs:14` |
| F10 | The only reader of task status outside the setter is the dashboard "blockers" list: `status = 'blocked'`, up to 20. | `dashboard.rs:241-248` |
| F11 | Delegations reference `tasks` with `ON DELETE RESTRICT`; objectives cascade-delete their tasks. | `migrations.rs:426`, `709`, `24` |

## 2. Required corrections to existing intent

Each one is a gap or contradiction between PLAN-020 / assessment wording and the code, with the affected acceptance.

**R1 — Execution-linked completion has nothing to link to (F2).** PLAN-020 M48 says administrative completion is guarded separately from "execution-linked acceptance" and that run QA cannot be bypassed. Today a task can be set `completed` while its run is `running`, and a `verified_success` run changes nothing on the task. Required: define the linking rule before implementing either path.
- Execution path (proposed): `accept_task_run(task_id, run_id, expected_revision)` succeeds only if the run belongs to that task, is `verified_success`, and the task revision matches. It sets `completed` with `completion_kind = execution` and the run id.
- Administrative path (proposed): `complete_task(task_id, expected_revision, note)` is an Owner command; it is **refused while the task has a `running`/`blocked` run or an `active` delegation/PM assignment** (otherwise it bypasses `finish_run`). It records `completion_kind = administrative`.
- Affects M48 acceptance "existing run QA cannot be bypassed" and test "administrative-vs-run completion". Also decide whether `start_run` rejects terminal tasks (recommended: yes).

**R2 — "Guarded transitions" needs a stated matrix (F1).** The assessment says guards exist but gives none. Proposed minimum: `queued → active|blocked|cancelled`; `active → queued|blocked|completed|failed|cancelled`; `blocked → queued|active|cancelled`; `completed|failed|cancelled` terminal except an explicit audited `reopen` (reason required, revision bump, returns to `queued`). Existing caller `set_task_status(task.id, "active")` from `queued` (`tests/optimization_m16.rs:139`) stays legal. Decide whether the legacy MCP tool keeps accepting arbitrary transitions for compatibility; if it does, say so, otherwise the guard is bypassable through the old tool.

**R3 — Revision must be a counter, not `updated_at` (F3).** `updated_at` has 1-second resolution, so two edits in one second cannot be told apart and "stale revision" tests would be flaky or vacuous. Required: integer `revision`, incremented in the same transaction as every task mutation (status, priority, due, effort, parent, blockers, edges), and used as `expected_revision` on guarded commands. The checkpoint packet must carry per-task revision.

**R4 — Dependency-blocked and manually blocked are different kinds, and only one is stored.** Keep `status = 'blocked'` for manual blockers only; derive dependency blocking at query time and never store it. Otherwise the stored value drifts when a prerequisite changes and two truths exist. Consequence: the dashboard blocker list (F10) shows manual blockers only; either leave it and say so, or add the derived list to it deliberately. A task with both kinds reports both reasons; resolving one must not unblock the other.

**R5 — Eligibility must consider the parent chain (F4, F7).** An objective that is `completed/cancelled/archived/blocked` makes its queued tasks ineligible even though task rows are unchanged (reason `objective_inactive`). Likewise a `paused|archived` project (new field, F7) excludes its tasks from portfolio queries (reason `project_paused`). Project status is net-new: legacy rows default to `active`, and that default is a stated migration choice, not an observation.

**R6 — Resume must not call `reconcile_project` (F8).** The assessment says "Git refresh explicit", which is right, but `resume_project` as a read must use the stored `git_head`/`reconciled_at` and report them as "as of `reconciled_at`". Calling reconcile from a read would (a) fail on a missing repo, (b) write, (c) invalidate memories. The refresh is a separate command whose result is shown in the next resume.

**R7 — "Owner command vs agent proposal" has no enforcement point yet (F6).** PLAN-020 asks for scoped authorization and separate paths. The code has role-scoped capabilities for Main/PM/Worker but only one unscoped class for everyone else. CX-01 must say where "Owner" is decided (unscoped client? a new scope token?) rather than assume it. I do not know what the Owner intends for non-role clients; this is an Owner/Codex decision, listed in §6.

**R8 — Tool descriptions are wrong (F5).** Fix `set_task_status` and `set_objective_status` descriptions as part of M48; they are what an agent client actually reads. Low risk, but they currently tell clients to send `pending`.

## 3. Examples (state → command → expected)

Notation: `P` project, `O` objective, tasks `A, B, C…`. "Eligible" = appears in the next-action list. Unless stated, projects are active, objectives `active`, `now` = D0.

**E1 Parent completion.** Parent `P1` with children `C1 completed`, `C2 cancelled`, `C3 completed`.
- `complete_task(P1)` → rejected: `required child C2 is cancelled without a waiver`.
- `waive_child(P1, C2, evidence_ref, note)` (Owner) → revision bump on P1 and an event; then `complete_task(P1)` → ok.
- Parent with `C4 queued` → rejected `child C4 open`. Parent with no children → ordinary task.
- Parent cancel with open children → rejected unless the command names `cancel_children`; no implicit cascade. (Proposal; PLAN-020 is silent.)

**E2 Failed / cancelled prerequisite.** `B` depends on `A`.
- `A queued` → B excluded, reason `prerequisite A not completed (queued)`.
- `A completed` → B eligible.
- `A failed` or `A cancelled` → B excluded, reason `prerequisite A failed|cancelled`, and it stays excluded until a reviewed `remove_prerequisite(B, A, reason)` or `A` is reopened and later completed. Not auto-unblocked.
- `A` reopened after `B` already `active` → B is not force-changed; B's row reports `warning: prerequisite A reopened`.
- Edge creation: `B→A` when `A→…→B` exists → rejected as a cycle; self-edge rejected; edge to a task in another project without a declared project relation and scope → rejected, nothing written.

**E3 Manual blocker.** `B` has no dependencies.
- `block_task(B, reason="waiting on bank letter", source=owner)` → status `blocked`, structured blocker row, revision+1, excluded with `manual blocker: waiting on bank letter`.
- `block_task` with an empty reason → rejected.
- `resolve_blocker(B, resolution)` → blocker closed, status returns to `queued` (not `active`: resolving a blocker does not claim work is in progress). If B also has an incomplete prerequisite it remains excluded for that reason.

**E4 Explicit priority and ordering.** Tasks: `T1 priority=high, due unset`; `T2 priority=normal, due D0-3 (overdue)`; `T3 priority=high, due D0+1`; `T4 priority unset`.
- Proposed total order: priority (explicit high→low; unset after all explicit) → due (earliest first, overdue first, unset last) → number of tasks it unblocks (desc) → task id (asc). Result: `T3, T1, T2, T4`.
- Why this needs saying: the assessment lists "Owner priority, overdue/due ordering, unblocking value, stable ID" without saying whether overdue can override priority. As a lexicographic order it cannot; if the Owner wants overdue to outrank priority, say so now.
- `set_priority(T2, high, expected_revision)` by Owner → audited event with from/to and revision. The same call from an agent-scoped client → rejected; an agent may only file a proposal (separate record, never changes order).
- Unset priority is not inferred as "normal". It is reported as `unprioritized`.

**E5 Unknown effort / due.** `E1 effort=45min`, `E2 effort=90min`, `E3 effort unset`.
- Query `fits_within=60` → `fits: [E1]`, `exceeds: [E2]`, `unknownEffort: [E3]`. E3 is never silently included or excluded from "fits".
- Due unset: never "overdue", sorts last, shown as `due: unknown`; no default is invented. `overdue` needs a defined "now" and due representation; see §6 Q3.

**E6 Paused / inactive parents.**
- `P` set `paused`: its tasks vanish from the portfolio next-action list with `project paused` available on an explain query; `resume_project(P)` still works and shows `projectStatus: paused` at the top (pause is not a hiding mechanism).
- `O` cancelled while `A queued` → A ineligible, reason `objective cancelled`; A's own row is unchanged (F4).
- Nothing in this path starts a provider.

**E7 Stale checkpoint.** Checkpoint `rev 3` taken at D0 with head `H1`, open tasks `A(rev 2), B(rev 1)`. Afterward: `A` completed (rev 3), `C` created, `reconcile_project` run, head `H2`, two project memories invalidated.
- `resume_project` returns tasks from **current** tables (open: B, C), with `deltaSinceCheckpoint`: A completed, C added, B unchanged, listed by task id + revision pair (`2→3`).
- Repository: `checkpointHead=H1`, `storedHead=H2`, `storedHeadObservedAt=reconciled_at`, `changedSinceCheckpoint=true`. The checkpoint's old summary text is displayed as historical and labelled with its age; where it disagrees with current tasks, current tasks win.
- Repo path gone: `repository: not refreshed (unavailable)`; no error from the read; stored values returned with their timestamps. No guessing.
- No checkpoint exists: packet returns current state with `checkpoint: null` and `checkpointAge: null`, not an empty-success summary.
- Immutability: writing `rev 4` leaves `rev 3` byte-identical; there is no edit-in-place command.

## 4. Compact expected resume packet (M49)

Proposal for CX-02 to adapt; field names are not binding. Every field is data from a named source; nothing is model-authored except the checkpoint's own Owner/agent-written summary, which is labelled as such.

```json
{
  "packetVersion": 1,
  "project": { "id": 7, "name": "…", "status": "active", "src": {"table":"projects","rev":null} },
  "generatedAt": 1790000000,
  "checkpoint": { "id": 12, "revision": 3, "createdAt": 1789000000, "ageSeconds": 1000000,
                  "summaryText": "…(historical; written by <actor kind>)…", "sourceStateRevision": 4410 },
  "repository": { "checkpointHead": "H1", "storedHead": "H2", "observedAt": 1789900000,
                  "dirtyAtObservation": false, "freshness": "changed_since_checkpoint | same | unknown" },
  "objective": { "id": 3, "title": "…", "status": "active" },
  "remaining": [ { "taskId": 21, "revision": 1, "title": "…", "status": "queued" } ],
  "nextEligible": [ { "taskId": 22, "revision": 1, "rank": 1, "why": ["priority:high","unblocks:2"] } ],
  "blocked": [ { "taskId": 23, "revision": 2, "reasons": [ {"kind":"manual","text":"…"}, {"kind":"prerequisite","taskId":21,"state":"queued"} ] } ],
  "unknowns": { "effort": [24], "due": [24, 25], "priority": [25] },
  "deltaSinceCheckpoint": [ { "taskId": 20, "change": "completed", "revFrom": 2, "revTo": 3, "eventId": 9120 } ],
  "recentDecisions": [ { "decisionId": 5, "title": "…", "createdAt": 1789500000 } ],
  "verification": { "lastRunId": 88, "lastRunStatus": "verified_success", "finishedAt": 1789800000,
                    "mandatoryChecksChangedSince": false },
  "limits": { "maxItemsPerList": 25, "truncated": { "remaining": 0, "nextEligible": 0, "blocked": 4 } }
}
```

Rules this packet encodes:
- Per-item `revision`/`eventId`/timestamps are the provenance; no free-text claim without a source row.
- All lists are bounded; `truncated` carries the omitted counts so a short list is never read as complete.
- Nothing implies a provider ran, was available, or is assigned. There is deliberately no "assignee agent", "model" or "continue with …" field; "Continue" is a separate scheduling command bound to a task revision (assessment §14).
- `verification` is copied from the latest run row. If there is none: `null`, not `passed`. If a mandatory check was added or changed after the checkpoint: `mandatoryChecksChangedSince: true`.
- The packet must fit well below the 32 KiB event limit (F9) if it is ever referenced from an event; store a reference, not the body.
- `sourceStateRevision` needs a definition. The only monotonic global counter in the schema is `events.id`; recommend that as the source state revision unless Codex prefers a dedicated counter. Flag as a decision (Q4).

## 5. Optional suggestions (not required for M48/M49 acceptance)

1. Add an `explain_task_eligibility(task_id)` read: one task, every reason it is in or out of the list. Cheap, reuses the same predicate, and makes the exclusion rules testable one task at a time.
2. Use cursor pagination keyed on `(rank key…, task id)` so lists are stable across concurrent edits.
3. `next_action` free text: Owner-written only, bounded (e.g. 500 bytes), shown in the packet as Owner text; not generated.
4. Persist a short `completion_kind`/`completed_by` pair (`owner`/`execution`, run id) now; it makes later reconciliation (M54) additive.
5. Reject `start_run` on a terminal task (see R1) even if the broader decision slips; it is a one-line guard with a clear test.
6. Include in the migration test a populated pre-M48 row set where every legacy task keeps its id and status string and all new fields are null (assessment already says "backfill unknown as null").
7. Dashboard: leave the existing blockers panel as manual-only for M48 and note it; do not fold derived blockers into the desktop code while the UI track is paused.

## 6. Open questions for the Owner / integrator (not answered by the sources)

- **Q1 (R7):** who is "Owner" for write commands — any unscoped MCP client, or a new scoped credential? Needed before "scoped authorization" can be implemented or tested.
- **Q2 (R2):** may the legacy `set_task_status` tool remain a permissive path, or must it route through the guards?
- **Q3 (E5):** is `due` a date or an instant, and in whose timezone is "overdue" decided? Assessment §10 mentions a planning timezone but only as a later profile field. Until decided, treat `overdue` as `due_instant < now`.
- **Q4 (§4):** `sourceStateRevision` = `events.id` high-water mark, or a dedicated counter?
- **Q5 (E1/E2):** confirm the waiver/override vocabulary: PLAN-020 says "only completed prerequisites unblock *by default*", the assessment says "reviewed edge change or blocker resolution". This note assumes the override is `remove_prerequisite`/`waive_child` with a reason and evidence reference.

## 7. Acceptance / consistency check against existing authority

- Examples E1-E7 agree with PLAN-020 M48/M49 acceptance and with assessment §§12, 14, 18, 28; the §28 example (B blocked by A, completing A via administrative or verified-execution path exposes B) is covered by E2/R1.
- **Conflicts with source** (not with PLAN-020): R1 (no run→task link), R3 (second-resolution `updated_at`), R6 (resume vs `reconcile_project` side effects), R8 (wrong status names in tool text).
- Not traced: the existing restart/vertical-slice fixture; whether any external caller sends arbitrary transitions through `set_task_status`; the desktop Tauri test (`apps/desktop/src-tauri/src/phase1_m4_integration_test.rs`) beyond seeing it calls `create_task`. CX-01 should grep callers before tightening the setter.
- No CX-01/CX-02 diff was available, so the "review stable diffs" subtask is open and waits for the Owner to hand one over.
