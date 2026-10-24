# Repository integration — 2026-10-25

Scope: the Owner requested bringing Cathulu pushes and merges up to date after the pivot assessment. This authorizes the pending repository checkpoint's branch, commit, push and PR merge, superseding earlier no-stage/commit/push instructions for this work. It does not authorize provider activation, another project's tasks, runtime reload or production rollout.

## Baseline and integration scope

Local `main` and freshly fetched `origin/main` both started at `757fd3d5c0325095e40b07963602eb6feccd5624`. There were 63 modified/untracked files carrying the pending role-orchestration, memory-closeout, Owner-decision, terminal-monitor, headless-startup, presentation/policy and pivot-documentation work. All were copied and hashed into the ignored local `target/repo-sync-20261025-baseline` before integration edits.

Use `codex/sync-pending-work-20261025` for the combined source checkpoint because these existing edits share schema, workflow, capability and documentation dependencies. Source integration does not claim that every live milestone is complete. M42/M44, native visual review, provider-specific acceptance and comparative-benefit gates remain as recorded in their plans. PLAN-020 M48-M55 remains future implementation.

The two older registered worktrees were inspected read-only. Both were clean and matched their upstream branches (`codex/m18a-public-mcp-tests` and `codex/m18b-plan-closeout`); their work is already represented by merged PRs #11 and #13. Historical remote branches were retained. Every historical PR except draft #23 was already merged. Draft #23 was closed as superseded by merged PR #24 and later dashboard PRs #25-#32; its native-review limitations were preserved in its description rather than treating the stale branch as new work to merge.

## Integration repairs

The full test run exposed stale expectations in the pending work. Repairs were limited to three existing test files:

- `tests/delegation_lifecycle.rs`: remove schema-14-through-18 fixture dependents and markers before constructing populated legacy upgrades; check the actual v18 destination; read the JSON-RPC `result.tools` envelope; explicitly resolve the synthetic approved action before testing the separate Worker-report review gate; disable background dispatch in fixture subprocesses.
- `tests/mcp_vertical_slice.rs`: validate v18 upgrade results and the new dispatch/capability/inbox tables; require the four documented resource URIs rather than the old three-resource count; disable background dispatch for fixture subprocesses.
- `tests/optimization_m15.rs`: prove telemetry leaves the schema version unchanged by comparing before/after values, rather than assuming the application's schema is still 13.

No production source, UI, credential or runtime setting was changed during these repairs. All original pending files other than these test fixes and the documented integration-status updates were preserved. This statement concerns edits in this synchronization session; the integrated checkpoint includes the preexisting production changes.

## Validation evidence

- `cargo fmt --all -- --check`: PASS.
- `cargo test --workspace --locked --offline`: PASS, 91 tests passed, zero failed, one preexisting isolated desktop integration smoke test intentionally ignored. That test requires `scripts/phase1-m4-integration-smoke.ps1`; its native desktop scenario was not run here.
- `npm test` in `apps/desktop`: PASS, eight tests.
- `npm run build` in `apps/desktop`: PASS, TypeScript and Vite production output.
- `scripts/build-current.ps1 -BuildOnly`: PASS for release MCP, desktop (`custom-protocol`) and monitor. Source fingerprint `91FC6B2D185EE9FAD52D9A035E150C769E1D23BA5468AA447039F6C5EA64E67E`; MCP SHA-256 `4DB5C980813FF63B88A7AC7765F4E5B03FFD628D317A8317EEE0D275C4517CD3`, desktop `46739516ABFE2CD880F14EC820720CF0B116E09DAE344532B9E5C6D90568ECEB`, monitor `BB7950C9C22FC501236FC9DC8B5FA26B8CC8B28ACA7B481FFF43EE6A0B7B62BB`. Build-only did not move the stable release pointer or restart a process.
- Isolated scratch release smoke: MCP initialized and discovered four resources; monitor `--once` and `--json` exited successfully; noninteractive `--watch` was rejected as required; observer operations left the SQLite main-file bytes unchanged. Dispatch was disabled and no model turn was submitted.
- Repository Markdown targets/fences and JSON records were checked, with final counts recorded in the PR validation description. `git diff --check` passed. No files under the ignored backup/build/smoke directories are included in the checkpoint.

The restricted sandbox initially prevented Git metadata access and a Tauri build-script environment read. Repeating the authorized Git/format/test operations outside that filesystem sandbox succeeded. Actual stale-test failures were repaired and the full suite rerun; they were not dismissed as environment failures.

GitHub reported `main` unprotected, no repository rulesets and zero Actions workflows at preflight. There are no configured required CI checks to wait for; the PR's exact head must still be inspected for unexpected checks/status changes before merge. No protection setting was changed, bypassed or disabled.

## Delivery record

Local validation passed on the integration branch. The PR URL and checkpoint reference are added after push; GitHub's PR state is authoritative for the merge outcome. Final local synchronization uses a fast-forward to the observed remote merge commit and a clean working-tree check. Source/build evidence above is separate from loaded-process, live-provider and product-benefit evidence.
