# M49 checkpoint and resume evidence

Date: 2026-10-25
Branch: `codex/manual-dual-m48`
Baseline: CX-01 `5fc5459`, with the separately validated completion repair `d7dff34`.
Status: committed at `0e6718a`; Claude independently accepted the stable commit and reran all 107 core tests. Remote/runtime integration remains separate. Review: [CC-01 M49](../parallel/claude/CC-01-review-CX-02-m49.md).

## Implemented contract

Schema v20 adds immutable `project_checkpoints`, task/objective snapshots and artifact references. Revisions increase per project inside an immediate transaction. The source revision is the global `events.id` high-water mark before the checkpoint event; the command rejects a stale expected value. Unrelated project events can conservatively invalidate that expected value. Checkpoints also retain project planning revision, stored repository HEAD/dirty flag and its reconciliation timestamp. Existing project/objective/task IDs remain intact.

`create_project_checkpoint(project_id, expected_source_event_id, summary_text, artifact_ids)` accepts a 1–2,000-byte summary and at most 16 unique artifact IDs. Each artifact must belong to a run in that project; its URI/hash are captured historically. URIs above 512 bytes are refused for this compact record. `summary_source` is `unscoped_local_client`, because the existing local surface does not authenticate an Owner identity. Immutable triggers reject checkpoint/snapshot update and deletion. An immutable seal, written in the same transaction, also rejects later snapshot/artifact insertion; reads expose only sealed revisions. `latest_project_checkpoint` retrieves the latest per-project revision; `get_project_checkpoint` preserves historical lookup.

`resume_project(project_id, limit)` reads one consistent SQLite snapshot and returns current project planning, objectives, remaining/eligible/blocked tasks, blocker reasons, task revision/status deltas, current non-superseded decisions, latest-run QA, checkpoint age and stored repository observations. Current canonical tasks override the historical summary. Existing objectives supply goal state; no separate milestone entity was necessary.

Each list has a requested limit of 1–10 and exact total/omitted counts. Titles/actions/reasons are bounded excerpts with IDs pointing to full source rows. At most three blocker reasons appear per task with an omitted-reason count. The JSON packet is capped at 32,000 bytes; trimming preserves the first eligible action when one exists. Internal counting scans current project rows; the output bound is not a database-work quota.

Repository availability checks the registered directory and `.git` entry without invoking Git. HEAD/dirty values remain explicitly as of stored reconciliation. `same` compares stored HEAD with checkpoint HEAD, not current source. Missing checkpoints return null with null age; unavailable repositories retain historical observations and current task data. Refresh is the separate existing `reconcile_project` command. Resume does not change memory freshness or call a provider.

Latest-run status is historical evidence, not a claim that current source passed QA. `nonPassingMandatoryChecks` counts pending, failed and inconclusive mandatory checks. `mandatoryChecksChangedSince` compares the run's declared workflow checks with the latest version of that workflow; absent declarations remain unknown. Artifact URI/hash snapshots are not revalidated against files during a read.

MCP exposes the three semantic tools and project `resume` / `checkpoints/latest` resources. Writes remain unavailable to scoped Main/PM/Worker capabilities. Main reads project resume/checkpoint resources; the matching PM reads only its own project; Workers are denied. Personal/non-code visibility remains M50 work.

## Validation

All data and build output stayed below `target/manual-dev/codex/`. Product dispatch was disabled in public MCP fixtures, the provider env file was absent and provider keys were removed.

- `cargo fmt --all -- --check`: pass.
- Focused `checkpoints_m49` suite: 3 passed.
- `cargo test -p cathulu-core --locked --offline --quiet`: 107 passed, 0 failed.
- Populated synthetic v19 upgrade preserves task IDs/revisions, then creates/retrieves immutable v20 revisions across restart. Older v5/v10/v11 downgrade fixtures remove v20 objects before replaying migrations.
- Tests cover stale checkpoint creation, immutable history, task changes after checkpoint, dependency eligibility, missing checkpoint/repository, explicit Git refresh, bounded output/counts, same-project artifact references, changed mandatory checks and non-passing QA.
- The public MCP sequence creates work, reads current state, checkpoints, updates tasks, restarts and reads both resume and latest-checkpoint resources with providers disabled.
- Existing public role-capability fixtures now exercise matching-PM reads, cross-project resume/checkpoint denial, and scoped Main portfolio/planning/resume reads.

The first full-suite attempt exposed a fixture race: two dependent checkpoint requests were sent together and RMCP could execute either first. The fixture now uses separate sequential sessions for checkpoint creation and the intentional stale retry. The corrected focused and full suites pass.

## Limits and next packet

This establishes local source/test behavior. It does not establish a pushed/merged branch, loaded MCP process, live-store upgrade, M42/M44 live assignment, provider availability or measured benefit. The paused desktop is unchanged. Claude reviews the stable CX-02 commit through the shared board; material findings feed repairs. CX-03/M51a follows with the reviewed CC-03 proposal and actual exported common types before Claude adapter/policy compilation.
