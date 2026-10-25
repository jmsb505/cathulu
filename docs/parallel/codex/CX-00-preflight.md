# CX-00 — Codex preflight

Date: 2026-10-25
Packet: PLAN-021 CX-00, before CX-01 / PLAN-020 M48

## Launch baseline

Observed before the first packet write:

- Branch: `codex/manual-dual-m48`
- HEAD: `72815dc21e3a12bd55539b68191054e3ac0b48fa` (merged PR #34)
- `git status --porcelain=v1 --untracked-files=all`: empty
- `git status --short --branch --untracked-files=all`: `## codex/manual-dual-m48`
- The observed HEAD is the committed planning launch baseline. Historical `d2422fb` is only the prior source inspection and is not a reset target.

The launch files are tracked: `AGENTS.md`, ADR-0023, PLAN-020, PLAN-021, both manual starter packets, and the PR #33 repository integration record. The complete 14-file PR #34 planning checkpoint is present in the index at HEAD. No planning draft or prior local edit was pending when this check ran.

## Ownership and execution boundary

- Codex is the sole shared production/schema/manifest/Git writer for waves 0-3. Claude owns its separate design/review paths and does not need a freeze for read-only review.
- Keep the observed checkout and branch stable. Do not switch, pull, rebase, stash, reset, or discard another writer's work.
- Keep scratch databases, fixtures, logs and Cargo output under `target/manual-dev/codex/`; deterministic MCP fixtures disable product dispatch.
- M48 is the provider-free registered-project task/portfolio slice. No live project task, provider invocation, M42/M44 acceptance run, process restart, credential change, or desktop action is needed or authorized by this packet.

## Durable PR #33 evidence reused

The committed [repository integration record](../../readiness/repository-integration-2026-10-25.md) reports source/test integration at PR #33's merge (`d2422fbd`), including `cargo fmt --all -- --check`, `cargo test --workspace --locked --offline` (91 passed, 0 failed, 1 existing native desktop smoke ignored), desktop tests/build, and isolated build/scratch smoke. It explicitly leaves M42/M44 live acceptance and M48-M55 implementation open. Those results are historical evidence for that checkpoint; M48 changes require their own targeted and full-core validation.

## M48 baseline observations

- SQLite is at schema v18; migrations are sequential in `src/lib.rs` and `src/migrations.rs`.
- `projects` have Git/reconciliation identity only. `tasks` retain `id`, `objective_id`, title/description, existing status strings and timestamps.
- `create_task`, `set_task_status`, `get_task`, and `start_run` live in `src/workflow.rs`; `finish_run` gates verified run outcome on declared mandatory QA and required memory/parent review. Direct task completion currently has no such guard.
- MCP uses semantic Store calls and role-capability filtering. Worker/PM capabilities do not receive the general project-management tools; new mutation tools remain unavailable to scoped agents.
- The M48 definition gate is PASS for planning, not implementation. Current tests use isolated data and disable the product dispatcher; new M48 fixture data/build output will stay below `target/manual-dev/codex/`.

Preflight result: **PASS**. CX-01 may begin on this observed baseline. Scope is limited to M48; M49-M55 remain untouched.
