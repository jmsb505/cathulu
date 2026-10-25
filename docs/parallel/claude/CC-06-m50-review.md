# CC-06 M50 review: provider-free non-code projects (read-only semantic review)

Agent: Claude Code lane (PLAN-021 wave 5). Requested by Codex in `docs/parallel/codex/CX-06-m50.md` ("Stable review handoff").

Reviewed commit: `9911f2499defcc677d965f6138f5e4f92c642728` (`feat(m50): add provider-free non-code projects`). For the original review interval through `33c9ee7`, the later commits `4141e17` and `33c9ee7` are documentation only, so no `src/` or `tests/` file changed after `9911f24`. The amendment commit `0d33295` does change source and tests. It is reviewed separately in the section "Amendment review of `0d33295`" below.

Method: read-only. Claude ran no builds, no tests, no provider call, and touched no live store, credential or process. Codex reports the suites as passing (fmt, check, 161 tests, `git diff --check`). This review did not re-run them. Line references are to the reviewed commit.

## Verdict

**Accept the foundation as a provider-free, fail-closed slice, with amendments A-1 to A-5.** I found no defect that lets a private non-code project's content reach a provider, a dispatch or a worktree. A-1 is a privacy-surface gap and should be fixed or stated exactly before M50 is called accepted. A-2 to A-5 are smaller.

## 1. Populated migration, triggers and foreign keys

Checked by reading `VERSION_23` (`src/migrations.rs`) and `migrate_projects_to_v23` (`src/lib.rs:882`), and comparing with the pre-M50 schema (`git show 9911f24^:src/migrations.rs`).

- **Triggers.** Before M50, the only trigger on `projects` is `projects_create_logical_pm` (pre-M50 `migrations.rs:848`). No view references `projects`. `VERSION_23` drops and recreates that trigger with an identical body, so the logical Project Manager identity is preserved. Pass.
- **Foreign keys.** Twenty foreign-key references to `projects(id)` use `CASCADE`, `SET NULL` or `RESTRICT` (pre-M50 migrations). They are not altered. The rebuild runs with `PRAGMA foreign_keys = OFF` set outside any transaction (`lib.rs:892`, after the v22 block, which commits at `lib.rs:865`), so `DROP TABLE projects` cannot cascade into children. Pass.
- **Identity sequence.** `DROP TABLE` discards the `AUTOINCREMENT` high-water mark. `lib.rs` reads it first and restores `MAX(seq, previous)` inside the transaction, so IDs are not reused. Pass.
- **Integrity.** `PRAGMA foreign_key_check` runs inside the transaction and aborts on any violation. The rollback path is correct. Pass.
- **Repository path uniqueness.** `projects_repository_path_unique` (`UNIQUE ... WHERE path IS NOT NULL`) matches the `ON CONFLICT(path) WHERE path IS NOT NULL` upsert. Pass.
- **Migrated visibility.** Existing repositories become `kind = repository, visibility = shared`, which preserves current behaviour. Pass.
- **Test.** `populated_v22_project_rows_migrate_without_losing_ids_roles_memory_or_children` (`tests/non_code_projects_m50.rs:333`) covers this. I did not re-run it.
- **Wording (A-3).** The handoff says the migration fixture verifies "FTS". The only FTS table, `memory_fts`, is on `memories`, and the `projects` rebuild does not touch it.
- **Restore (A-4).** `migrate_projects_to_v23` restores `PRAGMA foreign_keys = ON` unconditionally (`lib.rs:928`). That is correct for the current caller, but it should restore the value it found.

## 2. Private-project resource and memory boundaries

Role capabilities:
- Resource reads of a private bound project are refused (`mcp.rs:1941-1953`).
- Tool calls are refused for a private project (`dispatcher.rs:1145`).
- Non-Main roles can read only their own project's resources and their own delegation (`mcp.rs:1968-1981`).

Main (the overseer role):
- `cathulu://dashboard` is withheld while any private project exists (`mcp.rs:1993-2001`). This is conservative, and Main loses the whole dashboard. Documented as a limitation (N-1).
- Project, run, delegation, work-plan and owner-approval resources of a private project are refused (`mcp.rs:2002-2013`, `resource_project_id` at `mcp.rs:2416`).
- `cathulu://portfolio/tasks`, `cathulu://roles` and `cathulu://dispatches` are filtered to shared projects (`mcp.rs:2035-2090`).
- `read_memories` and `search_memory_candidates` require a shared project scope (`dispatcher.rs:1326-1339`).
- Main's review and update tools require an active Project Manager assignment (`dispatcher.rs:1340-1406`). A private non-code project cannot have one, because it has no repository and so no delegation (see §3).

**Gap A-1 (Medium).** `cathulu://projects` is redacted for Main, not excluded (`mcp.rs:2016-2033`). The code replaces `name`, `path`, `branch`, `head`, `dirty` and `changed_paths`. It leaves `id`, `kind`, `visibility`, lifecycle fields and timestamps. Main can therefore see that a private non-code project exists, when it was registered, and its status and priority. ADR-0025 point 6 says "excluded or denied in Main's context resources", and the handoff says "redact or deny". Neither describes this exact exposure.
- Requested: either exclude private projects from Main's project list, which matches how portfolio tasks, roles and dispatches are already filtered, or state the exposure exactly in ADR-0025 and the handoff. I recommend exclusion, so every Main resource is consistently shared-only.

## 3. Pathless refusal before dispatch and worktree use

- **Enqueue.** `dispatcher.rs:1582-1584` refuses a delegation whose project is not `repository` or has no path, before the outbox row is written. Pass.
- **Dispatch.** `dispatch_one` refuses a missing `project_path` before binding, thread creation or any provider call (`dispatcher.rs:2779-2781`). Pass, but see A-5.
- **Delegation preparation.** `prepare_delegation` calls `current_registered_head`, which calls `project_repository_path`, so a non-code project cannot get a delegation (`delegation.rs:1341`, `3214-3215`, `lib.rs:194`). Pass. Because this call comes after the function's earlier steps, I did not confirm that nothing is written before it.
- **Worktrees.** `worktree.rs:144`, `408`, `719` and `1409` refuse with `project_has_no_repository`. Pass.
- **Other paths.** Graphify returns `unavailable` (`graphify.rs:120`). Checkpoint resume reports `not_applicable` (`checkpoints.rs:502`, `521`). Reconcile returns the typed `not_applicable:project_has_no_repository` and bulk reconcile skips the project (tested at `tests/non_code_projects_m50.rs:300-308`). Pass.

**Gap A-2 (Low).** The Main-thread workspace lookup, `dispatcher.rs:2118-2130`, reads `SELECT path FROM projects WHERE name = 'cathulu' COLLATE NOCASE ORDER BY id LIMIT 1` and does not filter `kind = 'repository'`. A pathless project with the same name and a lower ID makes the Main thread fail closed even when a repository project exists. This is not a leak.
- Requested: add `AND kind = 'repository'` and a test.

**Gap A-5 (Low, test coverage).** The M50 tests exercise the enqueue refusal (`tests/non_code_projects_m50.rs:267`). They do not exercise `dispatch_one`'s own refusal (`dispatcher.rs:2779-2781`), which is the second layer.
- Requested: a dispatch-level test with a pathless item.

## 4. Are the remaining P-Q2 and privacy gaps stated accurately?

I checked ADR-0025 points 6 and 7, the handoff's "Baseline and scope" and acceptance text, and my CC-02 open questions.

- "No provider receives non-code project context": **accurate for the code I read.** Non-code work is refused at enqueue and dispatch, and Main's memory tools require shared scope.
- "Private non-code data is available through the unscoped local management surface only": **accurate.** The surface does not authenticate an Owner, and ADR-0025 says so.
- "Personal categories, profile settings, grants, correction, export, erasure, retention and provider disclosure remain out of scope": **accurate,** and it matches the unanswered P-Q2 questions in CC-02 §5.
- "Whether a private non-code project may ever be shared with Main": open in CC-02, and ADR-0025 point 7 says sharing needs a later explicit decision. **Accurate.** There is no visibility setter in the code, so sharing is not currently possible. Stating that in the handoff would make the gap clearer.
- Exposure of existence and metadata to Main (A-1): **not stated accurately** in the ADR or handoff. See §2.

## 5. Notes that need no change

- **N-1.** Withholding the whole Main dashboard while any private project exists is conservative. The handoff describes it accurately.
- **N-2.** `visibility` defaults to `shared` for migrated repositories and to `private` for non-code projects. The inserts state both explicitly, so the defaults do not affect new rows.

## 6. Not verified

- I did not run the suites, and I did not run the populated-v22 migration against a live or copied database. The verdict on §1 rests on reading the SQL and the Rust migration, plus Codex's reported test results.
- I did not trace every tool in the role allowlists. I checked the resource and tool paths listed above.
- I did not review `src/checkpoints.rs`, `src/workflow.rs` or `src/terminal.rs` diffs beyond the references listed.

## Amendments requested from Codex

| ID | Severity | Request |
| --- | --- | --- |
| A-1 | Medium | Exclude private projects from Main's `cathulu://projects`, or state the exposed fields exactly in ADR-0025 point 6 and the handoff. |
| A-2 | Low | Filter `kind = 'repository'` in the Main-thread workspace lookup (`dispatcher.rs:2118-2130`), with a test. |
| A-3 | Low | Reword the handoff's "FTS" claim: `memory_fts` is on `memories` and is untouched by the projects rebuild. |
| A-4 | Low | In `migrate_projects_to_v23`, record and restore the previous `foreign_keys` value instead of forcing `ON`. |
| A-5 | Low | Add a dispatch-level test for a pathless item, covering `dispatch_one`'s refusal. |

No Codex-owned file was edited.

## Amendment review of `0d33295` (A-1 to A-5)

Scope: the amendment commit `0d33295` ("fix(m50): address review amendments A1-A5"), read-only, with no builds and no provider calls. Codex reports 164 passing tests at this base, and I did not re-run them.

| ID | Status | What I checked |
| --- | --- | --- |
| A-1 | Implemented | `src/mcp.rs`: Main's `cathulu://projects` now keeps shared projects only (`retain(visibility == Shared)`), so private projects are removed rather than redacted. The role branch is unchanged. Test `mcp_creates_private_non_code_project_and_withholds_it_from_main_context` asserts the private ID is absent from Main's list. ADR-0025 point 6 now states this. |
| A-2 | Implemented | `src/dispatcher.rs`: the Main-thread workspace lookup adds `AND kind = 'repository'`. Test `main_workspace_lookup_skips_a_pathless_project_with_the_same_name` creates a pathless project first and asserts the repository path resolves. |
| A-3 | Implemented | CX-06 now says the FTS check covers `memory_fts` on `memories` and that the `projects` rebuild does not alter it. |
| A-4 | Implemented | `src/lib.rs` `migrate_projects_to_v23`: `PRAGMA foreign_keys` is read before it is set OFF (lines 883-887 before 897), then restored to the prior value (lines 933-936). Test `project_migration_restores_the_existing_foreign_key_setting` covers the OFF case. |
| A-5 | Implemented | `src/dispatcher.rs`: `dispatch_one_refuses_pathless_project_before_any_provider_start` asserts `blocked` with `project_has_no_repository` and that no app-server is spawned. The helper `assert_refused_without_spawn` is defined at `dispatcher.rs:4099`. |

**Verdict: accept A-1 to A-5 as implemented.** I found no defect in the amendment diff.

Notes (none blocks acceptance):

- **N-1 (commit scope).** `0d33295` also adds `docs/parallel/codex/CX-07-m53.md`, which is an M53 handoff. Codex's stated plan was an M50-only checkpoint, so that file belongs in a separate commit. Documentation only; no code effect.
- **N-2 (wording).** PLAN-020's M50 bullet says the 164-test run was on the "uncommitted follow-up based at `33c9ee7`". That follow-up is now committed at `0d33295`, so the wording should say so.
- **N-3 (test coverage, optional).** The A-4 test checks only the prior-OFF case, which is the one that would expose a forced-ON bug. A matching assertion for the prior-ON case would make the restore explicit.
