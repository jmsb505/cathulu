# F1 proposal: administrative completion gate (M48)

Status: **applied.** Codex committed the F1 repair at `d7dff34`, which matches this patch. The patch applies to its base `5fc5459` only. Do not re-apply it to HEAD. Claude re-reviewed `d7dff34` (see [CC-01-review-CX-01-final.md](../CC-01-review-CX-01-final.md) §0).

Patch: [F1-admin-completion-gate.patch](F1-admin-completion-gate.patch)

## Decisions implemented (see [BOARD.md](../../BOARD.md))

- **D-1 (Owner, 2026-10-25):** administrative completion is refused when a linked run is `blocked`, `failed`, `inconclusive` or `cancelled`.
- **D-2 (default):** administrative completion is refused when a linked run is `verified_success`. Use `accept_task_run`.
- **D-3 (default, unchanged):** reasoned cancellation remains the child waiver. The patch makes no change to cancellation.

## What the patch changes

- `src/workflow.rs`
  - `complete_task` calls the new check only when `completion_kind == "administrative"`. The verified path is unaffected.
  - New helper `ensure_no_linked_run_blocks_administrative_completion`. It runs one query over the task's runs. Non-verified runs sort first, so the error names the run that blocks completion.
- `tests/tasks_m48.rs`
  - New helpers: `finish_linked_run`, `qa_workflow`, `accepted_project_summary`, `assert_administrative_completion_refused`, `parent_and_child`.
  - Nine new tests (table below). Existing tests are unchanged.

Diff: three hunks, 403 added lines, 0 removed lines. Line endings are LF, to match the committed blobs.

## Tests

| Test | Covers |
| --- | --- |
| `f1_refuses_administrative_completion_after_failed_run` | T1 |
| `f1_refuses_administrative_completion_after_blocked_run` | T2 |
| `f1_verified_completion_still_works_after_earlier_failed_run` | T3, guard for the verified path |
| `f1_refuses_administrative_completion_when_failed_run_precedes_verified_run` | T4 |
| `f1_refuses_administrative_completion_after_inconclusive_run` | T5 |
| `f1_refuses_administrative_completion_after_cancelled_run` | T6. Fixture only: no public path sets `cancelled`. |
| `f1_refuses_administrative_completion_while_verified_run_is_unaccepted` | D-2 |
| `f1_failed_child_keeps_parent_open_until_cancelled_with_reason` | F2 |
| `f1_legacy_cancelled_child_keeps_parent_open` | F3 |

Existing tests that must stay green: `task_run_completion_requires_finished_verified_success_and_task_revision`, and the no-run administrative completions in `task_portfolio_enforces_revisions_scope_blockers_order_and_completion_guards` (T7).

## Validation (Claude, 2026-10-25)

- Patched scratch copy, `target/manual-dev/claude/f1-edited` (from `git archive 5fc5459`, with the patch applied): `tasks_m48` 15 passed, `delegation_lifecycle` 14 passed, `mcp_vertical_slice` 17 passed.
- Unpatched `5fc5459` with the same test file (`target/manual-dev/claude/pristine-f1tests`): six refusal tests fail (T1, T2, T4, T5, T6, D-2). F2, F3 and T3 pass, because they describe existing behaviour. The refusal tests therefore detect the defect.
- `git apply --cached --check` against the `5fc5459` index: OK.
- `rustfmt --check` on the two changed files: clean.
- Not run: the full 95-test core suite, `cargo fmt --all -- --check` on the whole repository, and clippy.

Caveat: an early run reused stale test binaries across two copies that shared a target directory. The results above come from fresh builds. When repeating this, use one target directory per copy.

## Applying (Codex)

From the repository root, after the CX-02 work is committed or parked:

```bash
git apply docs/parallel/claude/proposals/F1-admin-completion-gate.patch
```

Then run `cargo fmt --all -- --check`, `cargo test -p cathulu-core --locked --offline`, and commit F1 as its own commit.

## Documentation to update (Codex-owned, not in the patch)

- Readiness record, administrative completion: "Administrative completion is refused while any linked run is blocked, failed, inconclusive or cancelled, or while a verified run is linked. Use `accept_task_run` for a verified run, or start a new run that reaches `verified_success`."
- Readiness record, F4 wording: "Failed or cancelled prerequisites keep dependent tasks blocked. Reopening a prerequisite returns it to `queued`; dependents stay blocked until it completes."

## Behaviour changes

- A task whose verified run is not yet accepted must be completed with `accept_task_run`.
- A task with a failed, blocked, inconclusive or cancelled run needs a new run that reaches `verified_success`, then `accept_task_run`.
- Reasoned cancellation still provides the child waiver (D-3).
