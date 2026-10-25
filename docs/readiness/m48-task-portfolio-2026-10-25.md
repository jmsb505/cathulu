# M48 task/portfolio implementation evidence

Date: 2026-10-25
Status: The Owner accepted the M48 task and portfolio checkpoint at `5fc5459` and repair `d7dff34`. Claude's final review identified a completion-provenance bypass; repair `d7dff34` passes 104 core tests and Claude independently re-reviewed it with an accept verdict. Remote/runtime integration remains separate.
Plan: [PLAN-020 M48](../plans/PLAN-020-mixed-provider-personal-project-os.md), scheduled by [PLAN-021 CX-01](../plans/PLAN-021-manual-claude-codex-development.md).

## Baseline and scope

CX-00 recorded the committed launch baseline before implementation:

- Branch: `codex/manual-dual-m48`
- HEAD: `72815dc21e3a12bd55539b68191054e3ac0b48fa`
- All required planning/evidence files tracked; Git status including untracked files was clean.
- The historical `d2422fb` source inspection was not used as a reset or launch baseline.

M48 extends the existing project and task entities. No M49-M55 implementation, frontend work, provider activation, live project assignment, credential change, process reload, or live-store migration was performed.

## Implemented contract

- Schema v19 adds project status/priority/revision and task parent, priority, calendar due date, effort, next action, revision, completion provenance and cancellation reason/time. Existing IDs, objective ownership and status strings remain.
- Same-project prerequisite edges and parent links reject cross-project references and cycles. Manual blockers retain typed reasons and resolutions; dependency and project/objective blockers remain derived query reasons.
- Eligibility requires a queued task, active project/objective, no open manual blocker, completed prerequisites, and no terminal parent. Failed or cancelled prerequisites keep dependent tasks blocked. Reopening a prerequisite returns it to queued; dependents stay blocked until it completes.
- Ordering is project priority descending, task priority descending, known/earlier due date, downstream unblocking count descending, then task ID ascending. The Owner confirmed on 2026-10-25 that project priority outranks task priority across projects. Priority defaults to neutral `0`; unknown due date/effort remain null. Dates are `YYYY-MM-DD` calendar days with no inferred timezone or overdue flag.
- MCP exposes revisioned planning/status/edge/blocker operations, reasoned cancellation/reopen, separate administrative and verified-run completion, and bounded `list_tasks`. Results are capped at 100 with no cursor/total count in M48.
- Main can read the global portfolio and project planning/tasks. A PM can read its project's planning/tasks; Workers receive no task planning resources. Task write tools are unavailable to all scoped Main/PM/Worker capabilities. The existing unscoped local MCP remains the implicit local Owner surface and does not authenticate an Owner or calling chat. The Owner accepts this limitation for local-only use; revisit before any remote exposure.
- Administrative completion needs a reason and no active run/delegation/PM assignment, blocker, unfinished prerequisite or open child. It is also refused while any linked run is blocked, failed, inconclusive or cancelled, or while a verified run is linked. Use `accept_task_run` for a verified run, or start a new run that reaches `verified_success`. A reasoned cancellation is explicit child-waiver evidence; migrated `legacy_unknown` cancellation is not. Verified-run completion retains existing QA, memory-review and parent-review checks and can accept a later verified run after an earlier non-verified run.

## Changed paths

- `src/lib.rs`
- `src/migrations.rs`
- `src/workflow.rs`
- `src/mcp.rs`
- `tests/tasks_m48.rs`
- `tests/delegation_lifecycle.rs` (legacy downgrade fixture now removes M19 additions before replaying older migrations)
- `tests/mcp_vertical_slice.rs` (expects schema v19)
- `docs/architecture.md`
- `docs/agent-operating-contract.md`
- `docs/pivot-migration.md`
- `docs/README.md`
- `docs/plans/PLAN-020-mixed-provider-personal-project-os.md`
- `docs/plans/PLAN-021-manual-claude-codex-development.md`
- `docs/readiness/m48-task-portfolio-2026-10-25.md`
- `docs/parallel/codex/CX-00-preflight.md`
- `docs/parallel/codex/CX-01-m48.md`

Claude-owned files under `docs/parallel/claude/` were read for review context and preserved; none are part of the Codex change set.

## Validation evidence

Build output and test databases were kept under `target/manual-dev/codex/`.

- `cargo fmt --all -- --check` — pass.
- `cargo test -p cathulu-core --test tasks_m48 --locked --offline --quiet` — 6 passed, 0 failed.
- `cargo test -p cathulu-core --locked --offline --quiet` — 95 passed, 0 failed.
- Migration fixtures cover a populated v18 database and a v1 database upgraded through v19. Existing v5/v10/v11 downgrade fixtures were repaired to remove v19 additions before simulating older schemas; those migration tests pass.
- The public MCP fixture creates tasks, adds a prerequisite, reads the eligible list and project resource, rejects a stale revision, and verifies restart persistence. Product dispatch is disabled; the configured provider env file is absent and both provider keys are removed.
- No-provider assertions show deterministic M48 paths do not need to spawn provider processes or call provider APIs.
- Full test suite includes the current role-capability tests; a focused MCP unit test checks matching-PM resource access and rejects Worker and cross-project PM access. Scoped Main resource access has not been separately exercised end to end.

The first full-suite attempt found older migration tests leaving schema-v19 columns while deleting their migration markers. The test downgrade fixture was updated and both the migration-specific suite and full package suite were rerun successfully.

## Remaining review and limits

The post-review administrative-completion repair was validated from the staged M48-only tree exported to `target/manual-dev/codex/f1-candidate-1329ee37a3774ea0a5f690d434152039`, with fresh build output at `target/manual-dev/codex/f1-build-1329ee37a3774ea0a5f690d434152039`. `cargo fmt --all -- --check` passed, and `cargo test -p cathulu-core --locked --offline --quiet` passed 104 tests, including 15 M48 tests. Refusal cases cover blocked, failed, inconclusive, cancelled and unaccepted verified runs; they assert unchanged status/revision and event count. Verified recovery, failed-child waiver and legacy-cancellation constraints also pass. This isolates the repair evidence from unfinished M49 work in the shared checkout.

Claude's provisional findings led to reasoned cancellation/reopen and reserved migration provenance. Its final review of `5fc5459` reproduced administrative completion after non-verified execution. The repair follows the shared board's D-1/D-2/D-3: preserve run provenance, require `accept_task_run` for verified execution, and retain reasoned child cancellation as a waiver. Claude can re-review the repair commit through the shared board while Codex continues independent M49 work.

These results establish local source/test behavior only. They do not establish merge/push, release, an active MCP reload, live database migration, M42/M44 acceptance, live provider behavior, or measured benefit. The next packet is the stable CC-01 review of CX-01; CX-02/M49 follows after the Owner accepts the M48 checkpoint.
