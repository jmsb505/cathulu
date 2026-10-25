# CC-02 — Personal / non-code project identity and privacy contract (M50)

Agent: Claude Code lane, PLAN-021 wave 1. Design input for CX-06 (M50) and, later, CX-01/CX-02 vocabulary. Changes no acceptance or source by itself.
Baseline: branch `codex/manual-dual-m48`, HEAD `72815dc`; at start of this packet the tree contained only my untracked CC-01 note (no Codex edits yet).
Depends on: [CC-01](CC-01-task-resume.md) for task vocabulary (task revision, `queued/active/blocked/...`, resume packet).
Read: PLAN-020 M50, PLAN-021, pivot-migration §§10, 12, 30, `kickoff-state.json` (P-Q2, P-Q3), operating contract privacy lines; code in `src/lib.rs` (projects, register/reconcile), `src/migrations.rs` (memories, logical roles), `src/memory.rs` (scopes, list/search), `src/mcp.rs` + `src/dispatcher.rs` (role/Main authorization, resources), `src/dashboard.rs`, `apps/desktop/src/App.tsx` (activity graph only).

Status of claims: **source-read only; nothing compiled or run.** Statements about what the code does cite lines. Everything under "Proposal" is a design for the Owner/Codex to accept or change. **No personal category, consent or retention preference below is the Owner's.** P-Q2 stays OPEN; the category list in §4 is a menu of candidates, each defaulting to *not collected*.

## 1. What the code does today

| # | Observation | Where |
| --- | --- | --- |
| F1 | A project **is** its Git root: `projects.path TEXT NOT NULL UNIQUE`; `ProjectSnapshot.path` is a non-optional `String`; `register_project` calls `inspect_git_repository` first and upserts on `path`. Identity-by-path means re-registering the same path returns the same id. | `lib.rs:543-553`, `46-56`, `321-352` |
| F2 | Inserting any project row fires `projects_create_logical_pm`, creating a PM logical role whose `role_key`, memory scope and receiver scope are fixed by CHECK to `…:<project id>` and immutable afterwards. Exactly one PM role per project. | `migrations.rs:803-817`, `820-823`, `837-854` |
| F3 | `reconcile_project` re-runs Git against the stored path and **errors** on failure; `reconcile_registered_projects` loops every project and reports a failure per bad one. A pathless project would fail every pass. | `lib.rs:404-408`, `473-488` |
| F4 | Other readers of `projects.path`: role-dispatch queue (provider thread cwd), delegation setup, Graphify freshness, worktree verification. All assume a Git repository. | `dispatcher.rs:1368`, `delegation.rs:3219`, `graphify.rs:119`, `worktree.rs:55` |
| F5 | Memory scopes are `global`, `project:<id>`, `objective:<id>`, `task:<id>`, and free-form `scope:<text>` (≤200 bytes). `scope:personal` can already be created by any unscoped client; there is no category, sensitivity or consent field on a memory row. | `memory.rs:1547-1592`, `migrations.rs:112-142` |
| F6 | `read_memories`/`list_memories` returns any scope to an unscoped client. Role capabilities: Workers and PMs may read **only** `project:<own id>`; Main may read **only** `global` or an existing `project:<id>` (no cold). So `scope:personal` is already denied to every role capability; but Main can read the memory of **every** project, including a future personal/non-code one. | `mcp.rs:1157-1167`, `memory.rs:556-599`, `dispatcher.rs:824-842`, `906-954` |
| F7 | Main may read `cathulu://projects`, `cathulu://dashboard`, `cathulu://roles`, `cathulu://dispatches`. The dashboard snapshot lists all projects and objective/task/run titles across projects. | `mcp.rs:1507-1516`, `dashboard.rs:193-263` |
| F8 | Retention is partly immutable: `events`, `decisions`, `workflow_versions` have no-update/no-delete triggers; `context_transfers.memory_id` is `ON DELETE RESTRICT`; `memory_known_by` cascades; evidence artifacts can be protected and cannot be unprotected. Memory events I read carry ids/metadata, not the memory body. | `migrations.rs:200-212`, `174`, `159`, `212`; `memory.rs:294-301`, `400-407`, `480-500` |
| F9 | `projects` has no status, purpose, kind or owner column (CC-01 F7). Memory freshness invalidation keys on `freshness_repo_id`/`freshness_git_commit`, so memories with no repo are never auto-invalidated by Git change. | `lib.rs:46-56`, `memory.rs:1224-1254` |

Consequence in one line: an unmodified pathless "project" cannot be created, would fail reconciliation every cycle, would get a PM role that the dispatcher could try to start in a nonexistent directory, and — if created as an ordinary project — its memory and titles would be visible to Main by design.

## 2. Required decisions and corrections

**P1 — Identity: separate "project" from "repository", zero-or-one, keep ids.** (Consistent with assessment §12 and P-Q3 "start with zero-or-one".)
- Project id stays the identity key everywhere (roles, memory scope, runs, tasks). Do **not** rekey on path or create a second project table.
- Path becomes optional (`NULL` = no repository). Uniqueness applies only to non-null paths. Legacy rows keep their path and id.
- Because `path` is `NOT NULL UNIQUE` in the table definition (F1), relaxing it needs a SQLite table rebuild with foreign-key/trigger recreation. The affected objects are everything that references `projects(id)`: logical roles and the `projects_create_logical_pm` trigger, delegations, runs, objectives, memories (`freshness_repo_id`), dependencies. Rebuild must be exercised on a **populated** copy (CX-06 acceptance already says this). A cheaper option — keep the column and store a reserved sentinel — is rejected: it invents a fake path, which M50 acceptance forbids.
- Add an explicit project `kind` (`repository` | `non_code`) rather than inferring "no path ⇒ non-code" in every reader. `kind = repository` requires a path; `kind = non_code` forbids one. Zero-or-one only; multiple repositories wait for real demand (P-Q3).

**P2 — Every reader of `path` must have a defined pathless behavior (F3, F4).** Required, with a test per row:

| Caller | Pathless behavior |
| --- | --- |
| `register_project(path)` | Unchanged for repositories. Non-code projects use a separate `create_project` command (name, kind, optional purpose); it never calls Git. |
| `reconcile_project` / `reconcile_registered_projects` | Skip `non_code` (count as `skipped`, not `failure`). Calling reconcile on a specific non-code id returns a typed `not_applicable`, not an error string. |
| Role dispatch / delegation / worktree / Graphify | Refuse with a typed reason `project_has_no_repository` **before** queueing. A queued item for a pathless project must never reach a provider cwd. |
| Resume (CC-01 R6) | Reports `repository: none` (distinct from `unknown`/`not refreshed`). |
| Memory freshness | No repo ⇒ no Git invalidation; staleness for non-code facts is time/supersession-based only (see P5). |

**P3 — The auto-created PM role (F2).** Do not try to suppress the trigger (it is part of existing identity guarantees). Instead the non-code project gets the same logical role row (preserves "one PM per project", ids, scopes) but the dispatcher refuses to start it (P2). State this explicitly in the contract so nobody reads the row as "a PM will run".

**P4 — Personal data needs its own scope kind and disclosure gate, not a naming convention.** Today `scope:personal` is only a string (F5). It is protected from roles only incidentally, because role allow-lists mention `project`/`global`. That is a coincidence, not a control. Proposal:
- Reserve a small fixed set of personal scope keys `scope:personal:<category>` (category from the Owner-accepted list, §4). The scope-id validator accepts only listed categories for the `personal` prefix; anything else under `scope:personal…` is rejected.
- Personal content is **never** written to `global` and never to a code project's `project:<id>`; writes through those scopes carry a "classification" check (see P6).
- Disclosure to any provider-backed role is **deny by default** and requires an Owner grant that names (category, receiving role/provider, purpose or task, expiry). A grant is a stored, audited record; absence ⇒ deny. No grant exists at M50 start.
- The unscoped-client reads are the Owner's own local access path. Keep them, but have them labeled (response says `scope: personal`), and keep personal scopes out of any "all memories" or default-context listing.

**P5 — Non-code projects use their own project scope, but Main must not read it by default (F6).** Today Main can read `project:<any id>`. For a `non_code` project whose `visibility = private` (new field, default **private** for non-code), Main's `read_memories`, `search_memory_candidates`, project listing detail, and dashboard titles must apply a project-level visibility filter. Without this, "create a personal project" silently equals "disclose it to Main", and from there into the Main provider session. This is the single most important change in this packet.
- `visibility`: `private` (Owner/local only), `shared` (Main + the project's own roles after Owner opt-in). Default for `kind = repository` stays today's behavior (`shared`) so existing flows don't change.
- The dashboard/resource filter (F7) must redact private projects' task/objective/run titles for role capabilities and show only an opaque entry (`Private project #N`, status counts) unless the Owner has shared it.

**P6 — Classification at write time.** Memory rows have no sensitivity field (F5). Add one coarse `classification` on memory (`ordinary` | `personal`) — not a rich label taxonomy. Rule: a `personal` memory may only live in a personal scope or a private project scope; moving it to a shared scope requires an explicit Owner command and produces an audit record. This makes the P4 rule enforceable by check rather than by convention.

**P7 — Correction, retention and deletion cannot be promised as-is (F8).** The schema makes some history immutable and some references `RESTRICT`. The contract must state what each personal category supports, since "delete" will otherwise mean different things:
- *Correct*: supersede (existing `supersedes_id` chain) keeps the old body in history (`include_history`). That is **not** deletion.
- *Redact/erase body*: not implemented. A new, narrowly scoped, audited `erase_personal_memory` is required: it nulls/replaces the body and FTS entry, keeps id/hash-of-nothing/audit event, and must handle `context_transfers` rows (F8: RESTRICT) — which are metadata plus content hash. Whether a retained content hash of erased text is acceptable is an Owner/security call (hash of a short sensitive string can be guessable). Recommend replacing it with a tombstone hash.
- *Immutable stores*: a personal **decision** cannot be erased (`decisions` no-delete trigger). So personal facts must not be recorded as `decisions` rows; use memory only. State this as a rule.
- *Events*: audit events carry ids/metadata only (verify in CX-06 that new personal events obey this; the 32 KiB payload cap is not a privacy control).
- *Export*: a per-category export command that reads only that category's scope; nothing cross-scope.
- *Retention*: per category, an Owner-chosen `retain_until` or `keep until erased`; no automatic expiry decisions by a model. Default: keep until the Owner erases (unset ≠ infinite promise; it is "no schedule chosen").

**P8 — Cross-project reads and edges.** Task/project dependency edges to a private project from another project require the declared relation plus Owner scope grant (CC-01 §3 E2 edge rules) and must not leak the private side's titles in the *other* project's blocked-reason text: show `blocked by a private task`.

## 3. Attack / denial scenarios (each is a test case for CX-06)

Setup: repo project `R1` (shared), non-code project `N1` (kind=non_code, private) with a task `N1-T "Renew passport"` and a personal memory in `scope:personal:identity`; **no disclosure grants exist**. A provider role is dispatched on `R1`.

| # | Action | Expected |
| --- | --- | --- |
| A1 | PM/Worker of R1 calls `read_memories(scope=project:N1)` | Denied (already true today for roles, F6). Keep as regression test. |
| A2 | Same role calls `read_memories(scope=scope, id=personal:identity)` | Denied by the dedicated personal gate, **not** by the incidental `scope_type != project` branch. Test must fail if a future role allow-list adds `scope`. |
| A3 | Main calls `read_memories(project:N1)` / `search_memory_candidates(project:N1)` | Denied while `visibility = private`; allowed after an Owner share command. |
| A4 | Main reads `cathulu://dashboard` and `cathulu://projects` | N1 appears as an opaque private entry; `N1-T`'s title, objective and run summaries absent from the payload (F7). |
| A5 | Unscoped client writes a `personal` memory into `global` or `project:R1` | Rejected (P6). |
| A6 | `prepare_context_transfer` of a personal memory to any receiver scope with no grant | Rejected and audited; no `context_transfers` row; the response does not echo the body. |
| A7 | Create a task in R1 with a prerequisite pointing at `N1-T` without scope/relation | Rejected atomically. With a grant: R1's blocked reason says `blocked by a private task`. |
| A8 | `reconcile_registered_projects` with N1 present | N1 counted `skipped`; no failure entry; no Git call attempted for it. |
| A9 | Queue a role dispatch for N1 | Refused `project_has_no_repository` before queue; no outbox row. |
| A10 | Stale personal summary: a `personal` summary memory is superseded; role asks for "current" | Only the current accepted row is returned; history only on explicit `include_history` by the Owner path. |
| A11 | `erase_personal_memory` on a row referenced by `context_transfers` | Succeeds as a tombstone (body/FTS cleared); references remain valid; a subsequent read returns `erased`, never the old text. |
| A12 | Grant expiry: grant for category X to PM of R1 expires at T; request at T+1 | Denied; the denial is an event; no cached context is silently reused (known-by receipts must not count as a standing grant — see assessment §11 on session delivery). |
| A13 | Migration of a populated v18 database with 3 repo projects, 1 PM role each | After M50 migration: ids, roles, memory scope keys, events, FTS results identical; every old project `kind=repository`, `visibility=shared`; `PRAGMA foreign_key_check` clean; triggers recreated (spot-check `projects_create_logical_pm` fires for a new repo project). |

## 4. Candidate personal categories (menu for the Owner — nothing assumed)

The assessment says personal collection is "opt-in, inspectable and correctable" and that only settings needing deterministic queries belong in structured fields. P-Q2 asks which categories and disclosures the Owner wants. This note does **not** answer it. Candidates, each defaulting to *not collected*:

| Candidate category | Typical content | Structured field or memory | Risk to decide |
| --- | --- | --- | --- |
| `planning` | working hours, planning timezone, day/week boundaries | structured setting | Low sensitivity; needed by "overdue" (CC-01 Q3) |
| `preferences` | how the Owner likes work organized, communication style | memory (accepted) | Inferred preferences must not be auto-profiled |
| `commitments` | non-code obligations the Owner states (appointments, errands) | tasks in a `non_code` project | Dates and places are sensitive |
| `health`, `finance`, `identity`, `relationships` | — | **not proposed** | Highest-risk; recommend not offering until a specific need and retention plan exist |

Rules that hold whichever categories are accepted: Owner-typed or Owner-confirmed only; no transcript mining; every fact has a provenance row (who, when, source); each category has its own disclosure grant and erase path (P7); a category with no accepted retention/erase behavior is not enabled.

Open Owner questions (not answerable from the repository): which categories above, if any; which providers may ever receive any personal category; whether a non-code project may be shared with Main at all; and desired retention. Until answered, M50 can ship schema, `kind/visibility`, pathless behavior and denial tests with **zero personal data collected**; CX-06 acceptance already allows this ("independent provider-free work continues if consent is unresolved").

## 5. Migration implications (for CX-06 / integrator; shared-schema requests)

- Table rebuild of `projects` (path nullable, partial-unique on non-null path, `kind`, `visibility`), preserving ids and re-creating the `projects_create_logical_pm` trigger and every FK (`ON DELETE` behaviors differ across tables: `CASCADE` for objectives/dependencies, `RESTRICT` for delegations/plans, `SET NULL` for `memories.freshness_repo_id`). Verify with `PRAGMA foreign_key_check` and row-count/ID comparison on a copy, per assessment §22.
- `memories`: new `classification` column with CHECK; existing rows default `ordinary` (a truthful default: nothing existing was collected as personal). Rebuild FTS triggers only if the table is rebuilt.
- New small tables: disclosure grants (category, grantee role/provider, purpose/task, created, expires, revoked) with no-delete and revoke-once triggers in the existing style; erase tombstone support.
- The next schema version number is not chosen here; recheck the initializer when implementing (assessment §22).
- Requests to the integrator: none require editing production files from the Claude lane. This note is advisory.

## 6. Future home / resume content specification (design only; desktop UI remains paused)

Not an unpause claim (P-Q5 stays open) and not a visual design. It fixes **information order** so any later UI or headless output reuses one structure. Language follows the current product (project, objective, task, checkpoint, run, evidence).

Home (portfolio), top to bottom:
1. **Needs you** — owner approvals and unresolved owner actions (already first in the existing attention panel), then manual blockers with their reasons.
2. **Next up** — the ranked eligible tasks (CC-01 §3 E4), each with project, priority, due or `due unknown`, effort or `effort unknown`, and the one-line reason it ranked.
3. **Projects** — each with status (active/paused), kind marker only when `non_code`, open/blocked counts, last checkpoint age, and — for repository projects — `repository: changed since checkpoint | same | unknown`. Private projects show an opaque row until opened by the Owner.
4. **Recent** — accepted decisions and verified runs, newest first.

Resume view for one project, top to bottom: project name + status + kind → purpose line → checkpoint age and repo freshness banner (stale ⇒ prominent, not buried) → remaining and next eligible → blocked with reasons → changes since checkpoint → recent decisions and verification state. Empty states say what is missing (`no checkpoint`, `no repository`, `nothing eligible`), never a success phrase.

Requirements any resumed UI must meet (checked later, not now): full keyboard operation with visible focus; usable at 200% zoom with no horizontal scroll at common widths; screen-reader names for status and counts (not color alone); stale/offline/unavailable-repo states shown as text; `prefers-reduced-motion` respected; private entries never expose titles in accessible names or tooltips either.

Graph reuse: the existing "Project activity graph" in `apps/desktop/src/App.tsx` (activity view, attention panel, checkpoints rail) is **ADAPT** — extend it to show kind/visibility/freshness and the opaque private entry, rather than build a parallel view. Any **new component or library dependency** (charting, calendar, drag-and-drop board, graph layout engine) is **DEFER** until the Owner resumes the UI track and a concrete need is shown; the headless packet above is sufficient for M48-M50.

## 7. Acceptance and consistency check

- Consistent with PLAN-020 M50 acceptance (no Git calls or fake paths for non-code; existing repos/role keys still resolve; personal memory not in ordinary global/project context; explicit edits keep provenance; no provider invocation) and with assessment §§10, 12.
- **Findings against existing source**, not against PLAN-020: F3 (pathless projects would fail reconcile every cycle), F6/F7 (Main sees every project's memory and titles by design, so a private project needs a visibility filter), F8 (hard delete/retention claims are unsupported by the immutable stores and `RESTRICT` references).
- **Conflict with PLAN-020 wording:** M50 says introduce "explicit reviewed personal preference scope"; this note recommends enforcing it with a dedicated scope-id validator and a write-time `classification` check rather than relying on role allow-lists that happen to exclude `scope`.
- Not traced: `src/gardener.rs` (maintenance proposals could read personal memories — CX-06 should confirm it respects scope/classification); `src/optimization.rs`/telemetry content (whether task or project titles are copied into telemetry exports); the desktop settings/export paths. These are open leakage surfaces to check, not clearances.
- Unanswered by sources, left open: all Owner facts and consent in §4; whether Main may ever see a private project; retention periods.
