# CC-01 semantic review of CX-02 (M49): checkpoints and resume

Agent: Claude Code lane (PLAN-021 CC-01, read-only review). Subject: Codex commit `0e6718a` (`feat(checkpoints): add immutable project resume snapshots`) and its handoff `docs/parallel/codex/CX-02-m49.md`.
Verdict: **accept (Claude review), 2026-10-25.** The M49 design meets the CC-01 requirements (R6, F6, E7, §4 packet). Owner checkpoint acceptance of M49 is still the Owner's step. This lane has not edited production code or tests.

## 1. Evidence

- **Identity.** `src/checkpoints.rs` and `tests/checkpoints_m49.rs` in `0e6718a` are byte-identical to the snapshot I pre-reviewed and tested before the commit.
- **Independent full run.** `git archive 0e6718a` into `target/manual-dev/claude/committed-0e6718a`, fresh target `build-0e6718a`, `--locked --offline`: 29 (lib) + 3 (checkpoints_m49) + 8 + 14 + 17 + 4 + 5 + 6 + 2 + 4 + 15 (tasks_m48) = **107 passed, 0 failed**. Matches Codex's report. Log: `target/manual-dev/claude/0e6718a-full.log`.

## 2. Design checks against CC-01 and CC-02

| Requirement (source) | Committed behaviour | Evidence |
| --- | --- | --- |
| Resume makes no Git call; stored freshness, timestamped (CC-01 R6) | `resume_project` reads `projects.git_head`, `git_dirty` and `reconciled_at`; no `reconcile` or `git` call in `src/checkpoints.rs` | source grep; test at `checkpoints_m49.rs` ~231–244 (freshness `same` then `changed_since_checkpoint`) |
| Bounded lists with truncation counts (CC-01 F6) | Items capped by `limit` (1–10); `*_total` and `*_omitted` recomputed after any trim (`refresh_omitted`, called at lines 604 and 613) | test asserts `remaining_omitted == 18` (~line 371) |
| Byte bound (CC-01 §4) | Trims the longest list until the packet is ≤ 32,000 bytes; errors if no trim is possible | `MAX_RESUME_BYTES` loop |
| Missing repository is explicit, not an error (CC-01 E7) | `availability: unavailable`, `freshness: unavailable`, stored head and timestamp kept | test ~245–264 |
| No checkpoint ⇒ honest current-state packet (CC-01 E7) | `checkpoint: null`, `checkpoint_age_seconds: null` | test ~130 and ~569 |
| Stale checkpoint shows deltas against current tasks (CC-01 E7) | `checkpoint_deltas` compares snapshot revisions with current tasks; current tasks are authoritative | `delta_since_checkpoint` covered in the resume test |
| Historical revisions immutable (CC-01 §3) | `project_checkpoints`, task, objective and artifact snapshot tables have no-update and no-delete triggers; a seal row, written in the same transaction, blocks later snapshot inserts | v20 migration; tests `populated_v19_upgrade_…` (now passing) and late-append regression |
| Revision race (CC-01 §4 / SQLite) | Revision is `MAX + 1` per project inside an immediate transaction, backed by `UNIQUE(project_id, revision)` | `create_project_checkpoint` transaction mode; migration |
| Artifacts belong to the project (CC-02 scope) | Each artifact ID must be a run artifact in that project; URI and hash captured at snapshot time | `create_project_checkpoint` query (~212–219) |
| Provider-free (PLAN-020 M49) | No provider, process or network call in the M49 path; public MCP restart test runs with dispatch and providers disabled | `public_mcp_checkpoint_resume_survives_restart_without_providers` passes |
| Role access (CC-02 scope, PLAN-020) | Checkpoint tools are in no role allowlist. Read: Main any project; a PM its own project only; Workers denied. The resource filter and the read check agree. | `mcp.rs` template filter (~1797–1812) and `role_can_read_task_resource` (~2186); `role_capabilities` tests changed in this commit |

## 3. Points for the Owner (design choices, not defects)

- **Source revision guard is global and conservative.** `create_project_checkpoint` requires `expected_source_event_id` to equal the global `events.id` high-water mark. Any event from any project or any memory write makes the expected value stale, and the caller must re-read and retry. Codex documents this as a conservative choice. The concern is operational: with Claude and Codex working in one live store, checkpoint creation may often be refused as stale. Options: keep it (strongest guarantee); scope the guard to the project's own events (cheaper, still safe for one project's state); or accept retries as the cost. My recommendation is to scope it to the project's events once the manual dual-agent workflow runs against the live store. This does not block M49.
- **Summary is local and unauthenticated.** `summary_source` is fixed to `unscoped_local_client`. This matches the M48 trust boundary, which is local-only.
- **Verification is copied from the latest run.** The resume packet reports the latest run's QA state and whether mandatory checks changed after the checkpoint. A missing run gives `null`, not `passed`. Matches CC-01 §4.

## 4. Open items (not blocking M49 acceptance)

- **F5 (scoped Main reads end to end):** Codex reports this closed for the resume slice. The `role_capabilities` changes in `0e6718a` carry the test evidence. I checked the read path and the templates, but did not audit those test bodies in depth. Owner can ask for a targeted read of them.
- **F7 (project-priority-first ordering):** still needs Owner confirmation.
- **F8 (unscoped local MCP):** unchanged, local-only.
- **F9 and S1/S5:** unchanged from the M48 review.
- **Board, `d7dff34`, `0e6718a`:** both commits contain Claude-lane docs (the board, the final review, the proposal). Their contents match my working copies. This is still worth knowing for the checkpoint.

## 5. Next

- CX-03/M51a: Codex is tracing callers and CC-03/CC-05 inputs before exporting the common interface. CC-02 and CC-03 review steps and the CC-04 compile step wait for those exported types.
