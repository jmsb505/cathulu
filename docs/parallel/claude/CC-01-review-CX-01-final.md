# CC-01 final semantic review of CX-01 (M48)

Agent: Claude Code lane (PLAN-021 CC-01, read-only review). Subject: Codex commit `5fc5459` (`feat(workflow): add deterministic task portfolio planning`) and its handoff `docs/parallel/codex/CX-01-m48.md`.
Verdict: **conditional.** The fix for F1 is now a validated patch ([proposal](proposals/README.md)). Codex applies it, adds F2–F3 coverage (included in the patch), and corrects the F4 wording. The verdict becomes "accept" after Codex's commit is re-reviewed. Owner decisions and defaults are recorded on [BOARD.md](../BOARD.md). This lane has not edited production code or `tests/tasks_m48.rs`; those are Codex-owned.

## 1. Scope and evidence

- **Reviewed revision.** The committed tree at `5fc5459`, exported with `git archive` into `target/manual-dev/claude/`. The live working tree was not used: it had uncommitted CX-02 edits, and `src/lib.rs` referenced a `checkpoints` module that does not exist yet. The repository, its branch and Codex's files were not touched.
- **Read.** `docs/parallel/codex/CX-01-m48.md`; `docs/readiness/m48-task-portfolio-2026-10-25.md`; `src/workflow.rs` (`cancel_task`, `reopen_task`, `set_task_status_with_revision`, `valid_task_transition`, `ensure_no_pending_task_execution`, `ensure_task_completable`, `ensure_no_open_task_children`, `complete_task_administratively`, `accept_task_run`, `list_tasks`); the VERSION_19 ordering in `src/migrations.rs`; the test structure of `tests/tasks_m48.rs`; the diffs to `tests/delegation_lifecycle.rs` and `tests/mcp_vertical_slice.rs`.
- **Independent test run on the committed snapshot.** `cargo test --locked --offline`, log `target/manual-dev/claude/cx01-committed-tests.log`, exit 0: `tasks_m48` 6 passed; `delegation_lifecycle` 14 passed; `mcp_vertical_slice` 17 passed. The full 95-test core suite was not rerun here; Codex's report gives 95 passed.
- **Reproduction of F1.** Scratch crate `target/manual-dev/claude/scratch`, test `admin_after_run.rs`, against the same snapshot. Output: `REPRO failed: run status=failed task status=active revision=2`, then `administrative completion ACCEPTED (status=completed, completion_kind=Some("administrative"))`; the same for the blocked run.
- **Who calls administrative completion.** The MCP tool (`src/mcp.rs` ~778) and the portfolio test only.

## 2. Material finding F1 and decisions

**F1. Administrative completion can close a task whose linked run failed or was blocked.**

- **Behaviour (reproduced).** A task whose linked run finished `failed` or `blocked` can be completed with `complete_task_administratively`. The result is `status=completed, completion_kind=administrative`, with no reference to the failed run.
- **Cause (`src/workflow.rs`, committed line numbers).** `ensure_task_completable` (2522) refuses only a linked run with `status = 'running'` (2532). `complete_task` (1178) requires only `queued` or `active`. `start_run` moves the task to `active` (1554 onward), and `finish_run` does not change it, so after a failed run the task is `active` and the administrative path is open.
- **Why it matters.** PLAN-020 M48 acceptance says "existing run QA cannot be bypassed". Assessment §12 says administrative completion is for ordinary todos and that execution-linked completion "cannot bypass finish_run QA".
- **Test coverage before the decision.** None. The only administrative completions in `tests/tasks_m48.rs` (lines 495, 600, 643, 659) are in the portfolio test, which never calls `start_run`.

**Decisions.**

1. **D-1 (Owner, 2026-10-25):** refuse administrative completion when a linked run is `blocked`, `failed`, `inconclusive` or `cancelled`. No override is added in M48.
2. **D-2 (default):** refuse administrative completion when a linked run is `verified_success`; use `accept_task_run`. Built into the proposal.
3. **D-3 (default, unchanged):** reasoned cancellation remains the child waiver. Claude's earlier recommendation was to refuse cancellation over non-verified history; this lane reversed it, because refusal dead-ends an abandoned failed child, so its parent could never complete.

## 3. Required fix and tests

### 3.1 Proposal

[proposals/README.md](proposals/README.md) and [proposals/F1-admin-completion-gate.patch](proposals/F1-admin-completion-gate.patch). The patch:

- puts the check on the administrative path only, inside `complete_task`, so `accept_task_run` and the verified path are unaffected;
- adds one helper, `ensure_no_linked_run_blocks_administrative_completion`, with one query over the task's runs, non-verified runs first;
- adds nine tests to `tests/tasks_m48.rs`, reusing its helpers, and leaves existing tests unchanged.

### 3.2 Rule and message

- Refuse when any linked run has status `blocked`, `failed`, `inconclusive` or `cancelled` (D-1), or `verified_success` (D-2).
- The error names the run id and its status, and points to `accept_task_run` or to a new verified run.
- The refusal is atomic: no status change, no revision bump, no event.
- Tasks with no linked run keep the current administrative behaviour.
- `cancelled` has no current public producer; the rule covers it for completeness. `inconclusive` is reachable from `finish_run` and the telemetry turn-stop path.

### 3.3 Tests

| Test in the proposal | Covers | Result on `5fc5459` (unpatched) | Result with the patch |
| --- | --- | --- | --- |
| `f1_refuses_administrative_completion_after_failed_run` | T1 | fails | passes |
| `f1_refuses_administrative_completion_after_blocked_run` | T2 | fails | passes |
| `f1_verified_completion_still_works_after_earlier_failed_run` | T3, guard | passes | passes |
| `f1_refuses_administrative_completion_when_failed_run_precedes_verified_run` | T4 | fails | passes |
| `f1_refuses_administrative_completion_after_inconclusive_run` | T5 | fails | passes |
| `f1_refuses_administrative_completion_after_cancelled_run` | T6, fixture only | fails | passes |
| `f1_refuses_administrative_completion_while_verified_run_is_unaccepted` | D-2 | fails | passes |
| `f1_failed_child_keeps_parent_open_until_cancelled_with_reason` | F2 | passes | passes |
| `f1_legacy_cancelled_child_keeps_parent_open` | F3 | passes | passes |
| existing portfolio and run-completion tests | T7 and the verified path | pass | pass |

Totals with the patch, scratch copy: `tasks_m48` 15 passed; `delegation_lifecycle` 14 passed; `mcp_vertical_slice` 17 passed. Not run: the full 95-test core suite, `cargo fmt --all -- --check` on the whole repository, clippy.

### 3.4 Docs (Codex-owned, not in the patch)

- Readiness record, administrative completion: state the refusal rule (see the proposal README).
- Readiness record, F4: "Failed or cancelled prerequisites keep dependent tasks blocked. Reopening a prerequisite returns it to `queued`; dependents stay blocked until it completes." Today's wording implies a reopen unblocks them.

### 3.5 Claude-side check after the commit

Export Codex's commit, run the three test binaries against it, then re-review the diff. This lane will not edit the repository.

## 4. Other findings

- **F2 (test gap, M2 path).** Covered by the proposal (`f1_failed_child_keeps_parent_open_until_cancelled_with_reason`).
- **F3 (test gap, M4 legacy).** Covered by the proposal (`f1_legacy_cancelled_child_keeps_parent_open`).
- **F4 (documentation).** Wording change in §3.4.
- **F5 (test gap, documented).** The record says scoped Main resource access was not exercised end to end. Add that test before relying on Main's portfolio reads.
- **F6 (input to M49, not a defect in M48).** `list_tasks` returns at most 100 items with no total, truncation marker or cursor. The resume packet in CC-01 §4 needs omitted counts. CX-02 must add them before M49 acceptance.
- **F7 (Owner decision, default kept).** Ordering uses project priority as its primary key, so one project's priority outranks every task priority in another project. This is documented. Confirm it is intended; until then it stands.
- **F8 (accepted limitation, documented).** The unscoped local MCP is the implicit Owner surface and does not authenticate the caller. Acceptable for M48 as local-only. It needs a decision before any non-local exposure.
- **F9 (minor).** `cancel_task` from `blocked` leaves manual blockers open, and the task is then `cancelled` with open blockers. `reopen_task` recomputes `blocked` correctly when blockers remain, so behaviour is consistent. Consider resolving or documenting the blockers at cancellation.

## 5. Status of the provisional findings (`CC-01-review-CX-01-provisional.md`)

- **M1** (finalized blocked run blocks later completion): fixed. The task-run test completes a task with a verified run after a blocked run.
- **M2** (failed child blocks parent with no exit): fixed in code. Covered by the proposal (F2).
- **M3** (no reopen path): fixed. `reopen_task` for `failed` or `cancelled`, with a reason.
- **M4** (cancellation as unevidenced waiver): fixed. `cancel_task` requires a reason; a child waiver needs a non-null, non-sentinel reason. Legacy case covered by the proposal (F3).
- **S1** (same-status call was dead code): fixed. `valid_task_transition` allows same-status for `queued`, `active` and `failed`. No test found; add one with the F4 docs change if Codex wants.
- **Sentinel reservation**: fixed and tested.
- **S2** (unset priority equals 0): accepted as documented neutral default.
- **S3** (list cap, no total): deferred to M49 (F6).
- **S4** (project priority dominance): documented; see F7.
- **S5** (abandoned running run): the operator path is `finish_run`. Not tested in `tasks_m48.rs`.
- **S6** (legacy setter without revision): kept for non-cancel, non-blocked, non-completed transitions only. Accepted.
- **S7** (test coverage): covered by the proposal, except F5 and the S1 test.

## 6. Open questions and defaults

- **Q1 — resolved (2026-10-25):** include `inconclusive` and `cancelled` in the refusal (D-1).
- **Q2 — default applied:** cancellation over non-verified history stays allowed as the child waiver (D-3). Owner may override on the board. A reasoned cancelled child counts as closed for its parent, so a parent can complete over a child that failed QA; that is the accepted waiver design.
- **Q3 — default applied:** a verified but unaccepted run blocks administrative completion; use `accept_task_run` (D-2). Built into the proposal.

## 7. Acceptance verdict

- **Accept** the M48 schema and semantic design: revisions, guarded transitions, same-project prerequisite and parent edges with cycle checks, structured blockers, derived eligibility, reasoned cancel and reopen, and verified-run completion.
- **Conditional on:** (1) Codex applies the F1 proposal (or an equivalent change) and commits it; (2) Codex makes the readiness-record wording change (F4); (3) Claude re-reviews Codex's commit.
- **Order of work.** F1 before CX-02. Completion provenance feeds M49's resume packet.

## 8. Claude-owned notes: status for the shared Git checkpoint

Nothing has been staged or committed by this lane. Under ADR-0023 Codex or the Owner performs Git writes.

| File | Status | Ready to stage? |
| --- | --- | --- |
| `docs/parallel/BOARD.md` (shared) | Current. Codex reads it on resume. | Yes, with Codex's sections when they exist |
| `docs/parallel/claude/CC-01-task-resume.md` | Complete (critique and examples). Several of its required corrections (R1–R8) are now implemented in CX-01; the note stays as the critique record. | Yes |
| `docs/parallel/claude/CC-01-review-CX-01-final.md` (this file) | Complete for `5fc5459`, the decisions and the proposal. Verdict conditional until Codex's commit is re-reviewed. | Yes |
| `docs/parallel/claude/proposals/README.md` and `F1-admin-completion-gate.patch` | Complete and validated against `5fc5459`. Codex applies. | Yes, as a proposal |
| `docs/parallel/claude/CC-01-review-CX-01-provisional.md` | Superseded (header added). Not acceptance evidence. | Only with its SUPERSEDED header, or leave it out |
| `docs/parallel/claude/CC-02-personal-project-contract.md` | Complete as a draft M50 contract. Open Owner questions (P-Q2 categories, disclosure, retention) are recorded in §4. | Yes |
| `docs/parallel/claude/CC-03-provider-contract.md` | Complete as a proposal, including the `AccountRestricted` amendment. The §10 checklist cannot run until F1 types exist. | Yes, labelled as a proposal |
| `docs/parallel/claude/CC-04-adapter-evidence.md` | Updated with documented credential, config-dir and settings facts. Adapter parts still wait on F1 and a real install. | Yes, as a feasibility record |

## 9. Next steps (Claude lane)

- Watch the board and Codex's commits. When Codex applies the F1 proposal, export its commit, run the three test binaries, and re-review.
- CC-02 and CC-03 wait for Codex's CX-02 and CX-03 F1 types before their remaining review steps.
- CC-04 compile step waits for `src/agent_provider.rs`.
