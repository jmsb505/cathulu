# CC-01 follow-up — PROVISIONAL review of CX-01 (M48) in-progress diff

> **SUPERSEDED.** Final semantic review of the committed CX-01 commit `5fc5459` is in [CC-01-review-CX-01-final.md](CC-01-review-CX-01-final.md). This file was written against an in-progress working tree and is kept only as history. Do not use it as acceptance evidence.

Agent: Claude Code lane (read-only review). Baseline: branch `codex/manual-dual-m48`, HEAD `72815dc`.
**Provisional.** I read the *uncommitted* working-tree changes in `src/lib.rs`, `src/migrations.rs` (`VERSION_19`), `src/workflow.rs`, `src/mcp.rs` while Codex was still editing. PLAN-021 asks for review of *stable* diffs, so nothing here is acceptance evidence and line numbers will drift. I ran nothing (no build, no tests); no test files had been added yet. I did not open `src/mcp.rs` beyond its tool list. Re-review when Codex reports CX-01 complete.

Good news first (maps to CC-01): the diff already implements most of CC-01's required corrections — integer task/project `revision` with optimistic checks; a guarded transition table; structured blockers with open/resolved records; derived (not stored) dependency/parent/project/objective blocking with reason codes; `completion_kind` `administrative | verified_run | legacy_unknown` with trigger-enforced provenance; `accept_task_run` requires a finished `verified_success` linked run; administrative completion refused while a run/delegation/PM assignment is open; `start_run` now rejects terminal/blocked tasks and activates queued ones; legacy `blocked`/`completed` rows get explicit `legacy_unknown` provenance instead of invented history; role allow-lists in `dispatcher.rs` are untouched, so the new tools stay unavailable to scoped agents; due dates are validated `YYYY-MM-DD` (answers CC-01 Q3's date-vs-instant half).

## Material findings (recommend fixing before M48 is called complete)

**M1 — A run finalized as `blocked` makes its task permanently uncompletable.** `ensure_task_completable` rejects completion while any linked run has status `running` **or `blocked`** (`workflow.rs` ~2355). But a run can never leave `blocked`: `ensure_run_open` requires `running`, and the only writers of `runs.status` are `finish_run` and the Codex-hook path (`workflow.rs:1770`, `telemetry.rs:260`). `finish_run` itself sets `blocked` whenever a run has **zero mandatory checks** or any pending/missing declared check. Result: task T, run R1 finished with no mandatory checks → R1 `blocked`; a later run R2 reaches `verified_success`; `accept_task_run(T, R2)` and `complete_task_administratively(T)` both fail with "linked run R1 is blocked", with no recovery path.
 Repro to add as a test: create task → `start_run` → memory review → `finish_run` (no checks) ⇒ `blocked`; second run to `verified_success`; expect completion to succeed. Fix: only `running` should block completion (a finalized `blocked` run is terminal evidence, not an open run), or define an explicit resolution for blocked runs.

**M2 — A failed child leaves the parent neither completable nor cancellable.** `valid_task_transition` has no edge out of `failed`; `ensure_no_open_task_children` treats any child not in (`completed`,`cancelled`) as open, so a `failed` child blocks both completing the parent and cancelling it. The only escape is `update_task_plan` re-parenting the child to `NULL`, which records only `parentTaskId` in the event — no reason, no waiver evidence. Fix: provide an audited path (a `waive_child`/reason-bearing cancel, or allow `failed → cancelled`/`reopen`), and test "parent with failed child".

**M3 — No retry/reopen path for terminal tasks.** `failed` and `cancelled` have no outgoing transitions and `completed` is immutable. CC-01 R2 proposed an audited `reopen` (reason, revision bump). Without it, a failed task with dependents can only be unstuck by `remove_task_dependency` (which loses the relationship) or by recreating the task. Either add `reopen_task(id, expected_revision, reason)` or record the decision "retry = new task" in the contract and add a test that dependents of a failed prerequisite stay excluded with reason `prerequisite_not_completed`.

**M4 — Cancelled children silently satisfy parent completion.** Assessment §12 says a parent completes only when children are complete **or explicitly waived with evidence**. Code treats `cancelled` as done (`workflow.rs` ~2404), but cancellation takes no reason, so there is no waiver evidence. Either document "cancellation is the waiver" and capture a cancel reason in the event/record, or require a waiver. (CC-01 E1 expected the stricter behavior; the assessment text is the authority, so flag it rather than assume.)

## Suggestions / questions (not blockers)

- **S1 same-status calls:** `valid_task_transition("queued","queued")` is false, so the `current == status` no-op branch right after it (`~569`) is dead code and a repeat call errors instead of being an idempotent no-op. Decide which behavior is intended.
- **S2 unset priority:** `priority` is `INTEGER NOT NULL DEFAULT 0`, so "unset" and an explicit `0` are indistinguishable (CC-01 E4 wanted `unprioritized` reported separately). Acceptable if 0 is documented as the neutral default.
- **S3 list bounds:** `list_tasks` caps at 100 with no `truncated`/total, no cursor. The M49 resume packet (CC-01 §4) needs omitted counts. Also the eligibility rule exists twice (SQL filter and `task_blocking_reasons`); add an equivalence test so they cannot drift, and note `LIMIT` is applied before the Rust post-filter.
- **S4 ordering:** `ORDER BY project.priority, task.priority, due_date, unblock_count, id` ranks every task of a higher-priority project above any other project's tasks. Confirm that is the Owner's intent for the portfolio view.
- **S5 stuck `running` runs:** `ensure_no_running_task_run` blocks every edit, blocker change, cancel and priority change while a run is `running`; a run abandoned without a Codex hook stays `running` forever (nothing cancels a run). Confirm there is an operator path (e.g., finalizing the run) and test it; also decide whether priority edits during execution should really be blocked.
- **S6 legacy API:** `set_task_status(id, status)` remains with `expected_revision = None` (no stale check) for existing Rust callers; the MCP tool requires a revision. Fine if intended; say so in the contract (CC-01 Q2).
- **S7 tests:** no new tests were present in the tree when I read it. CC-01 §3 E1-E7 and the M1-M4 repros above are ready-made cases; CC-02 A8/A9 (pathless projects) belong to M50, not here.

## Status update against the current working copy

Codex kept editing after my first read (HEAD still `72815dc`, nothing committed). I re-checked each finding against the current code:

- **M1 (finalized `blocked` run blocks completion): appears fixed.** The `status IN ('running', 'blocked')` guard is gone; only `running` runs block completion (`ensure_no_running_task_run`, `workflow.rs` ~2344). Needs a regression test.
- **M2 (failed child blocks parent): appears addressed.** A failed child still counts as open, but `cancel_task` now accepts `failed`, so the child can be cancelled with a reason and the parent can complete.
- **M3 (no reopen path): addressed.** `reopen_task(id, expected_revision, reason)` exists (`workflow.rs` ~591; MCP tool `reopen_task`, `mcp.rs` ~706).
- **M4 (cancellation is an unevidenced waiver): addressed in part.** `cancel_task` requires a reason and stores `cancellation_reason` and `cancelled_at`. A cancelled child closes its parent only when it has a reason (`ensure_no_open_task_children`, ~2574-2581). Open point: `cancel_task` does not reject the sentinel string `legacy_unknown`. Typing it keeps the child open, so this is not a bypass, but the sentinel should be reserved. The `VERSION_19` text I read does not set it for legacy cancelled rows, so check the migration.
- **S1 (same-status call was dead code): addressed.** `valid_task_transition` now allows same-status calls for `queued`, `active` and `failed`.

I have not run any of this. Re-review at a stable commit still applies.

## Not covered

Everything in `mcp.rs` beyond tool names/descriptions, the `Store` ↔ MCP argument structs, migration behavior on populated databases (I only read the SQL: `ALTER TABLE … ADD COLUMN … REFERENCES … DEFAULT NULL` is valid; legacy-row `UPDATE`s run before the new triggers are created, which is the right order), concurrency beyond the `BEGIN IMMEDIATE` + revision pattern, and any test results.
