# PLAN-021 — Manual Claude Code / Codex development in one directory

Status: Active manual execution plan. CX-00 preflight passed on 2026-10-25. CX-01/M48 is committed at `5fc5459`, with repair `d7dff34` independently accepted by Claude. CX-02/M49 is locally validated with 107 passing core tests; its stable semantic review is next. CX-03/M51a is the next implementation packet.
Created / updated: 2026-10-25
Historical source inspection: `main` at `d2422fbd061aec9dadf876951d0f3e77bb4b6d47` after PR #33. Writing this plan then left canonical-document edits and new planning files uncommitted; that tree is not a launch baseline.
Launch prerequisite: satisfied before CX-01 writes. CX-00 records branch `codex/manual-dual-m48`, committed HEAD `72815dc21e3a12bd55539b68191054e3ac0b48fa`, tracked planning files, and an empty `git status --porcelain=v1 --untracked-files=all`. That committed HEAD, not the historical hash above, is the launch baseline. Later packets inspect status/diff and preserve disjoint work; waves 0-3 do not repeat a clean-tree launch gate before independent implementation or read-only review.
Capability plan: [PLAN-020](PLAN-020-mixed-provider-personal-project-os.md)
Workflow decision: [ADR-0023](../decisions/ADR-0023-manual-shared-directory-development.md)

## Objective and scope

Give Claude Code and Codex useful, separate assignments for the Cathulu pivot while both work in **`C:\Users\sjuan\Desktop\cathulu`**, on the same checkout and branch. The Owner starts each session and controls integration pauses and reserved decisions. Both lanes publish routine handoffs and readiness on [the shared board](../parallel/BOARD.md), inspect each other's completed evidence and continue within this accepted plan. The Owner authorized bounded monitoring on 2026-10-25. This adds no automated dispatch, cross-session messages or worker fleet.

PLAN-020 remains authoritative for M48-M55 capability requirements and acceptance. This plan owns assignment, file ownership, preparation overlap and manual handoffs. It adds no competing todo/schema/runtime authority. Product provider adapters/routing remain in the pivot roadmap; the Owner's request to avoid automation concerns coordination of these development sessions.

## Assessment of the proposed strengths split

Use the supplied table as a starting preference. It does not establish a permanent difference in capability between the harnesses. Both support implementation, investigation, tests, review and parallel sessions. Relative results also depend on the selected model, context, task definition, tools and verifier, even though this plan deliberately assigns no model names.

Codex owns the shared Rust/SQLite/runtime work because it combines bounded implementation and executable verification with a single integration owner. This is a workflow choice, not evidence of intrinsic superiority. A fresh Codex session has no automatic access to the earlier session's reasoning, just as a fresh Claude session starts without it. Both must read the same committed source, plans and PR #33 evidence; those durable artifacts supply context.

| Preference from the reminder | Assessment / assignment consequence |
| --- | --- |
| Claude for product ambiguity, domain tradeoffs, architectural critique and UX | Adopt as a provisional lead preference. Produce explicit decisions, examples and counterexamples that another agent can implement and verify. |
| Codex for bounded implementation, migrations, known failures, tests and builds | Adopt for the shared core, using committed specifications and executable acceptance criteria. Codex must still challenge unclear requirements. |
| Claude for all architecture/refactors, Codex for all coding | Reject as a rigid boundary. Claude implements the bounded Claude adapter and pure routing policy; Codex designs local details inside accepted contracts. |
| Claude for every unknown bug | Investigate locally first. The agent already holding the reproduction/context leads until evidence suggests a conceptual dead end; then transfer a compact diagnosis packet. |
| Codex has a general exact-Figma or visual-fidelity advantage | Not established by the inspected evidence. Tool availability and measured fidelity decide; no Figma work is needed for the current headless slice. |
| Codex should run a fleet or long autonomous loop | Not part of this plan. Use one session per harness, bounded packets and manual checkpoints. Neither agent gets unlimited retries or extra workers by default. |
| Opposite-harness review reduces mistakes | Use it at consequential domain/security/adapter/integration checkpoints. A different harness is a useful second perspective, not proof of independent reasoning or correctness. |

Current primary documentation supports verification and isolated parallel workflows, rather than comparative superiority: [Claude best practices](https://code.claude.com/docs/en/best-practices), [Claude worktrees](https://code.claude.com/docs/en/worktrees), [Codex worktrees](https://learn.chatgpt.com/docs/environments/git-worktrees), [Codex app-server](https://learn.chatgpt.com/docs/app-server). These were checked on 2026-10-25, including Context7 lookups for Claude Code and Codex. The Owner explicitly chose a shared checkout; this plan addresses that choice without creating worktrees.

## What parallel means in this checkout

Both sessions can read and edit **different owned files** concurrently. They cannot have independent branches, source snapshots, Git indexes, dependency files or acceptance builds inside one checkout. A checkout/pull/reset affects both sessions. A test can accidentally compile the other agent's unfinished changes. One passing run is not attributable to one lane unless its input state is recorded.

Use two operating modes instead of a pause ceremony for every packet:

- **Waves 0-3: implement, then review.** Codex is the only production-code writer; Claude prepares its owned design notes and reviews completed diffs. Codex runs normal targeted/full checks and commits its completed scope without a two-agent freeze or hash record for each packet. Claude finishes any document explicitly included in that commit before it is staged. Branch changes, pulls/rebases and other operations affecting the whole checkout still happen only at a normal handoff when neither session is editing.
- **Waves 4-6: concurrent coding where useful.** Both write only their allocated files. Before shared module registration, shared dependency/schema edits that affect both lanes, a combined acceptance run or a commit of both contributions, the Owner relays **PAUSE WRITES** and gets both acknowledgements. Codex inspects `git status --short --untracked-files=all` and `git diff HEAD` (including staged changes and separately inspecting new files), runs checks, and checks the Git view again. There are no per-file hash manifests. Resume only the owner of a needed fix, then repeat the short pause for affected verification.

Common rules are limited to the shared resources:

1. **One shared branch and Git writer.** Codex or the Owner handles Git mutations. Claude may inspect Git but does not stage/commit/pull/merge/rebase/stash/reset/clean/switch. Never switch the branch or clear another agent's work underneath an editing session; stage only complete, reviewed paths.
2. **One shared schema/dependency writer.** Codex owns production migrations, common types, module registration and repository manifests/lockfiles. Claude owns its throwaway scratch crate's separate manifest and lockfile; it requests changes to the production ones. Transfer any other shared-file ownership explicitly before editing.
3. **Separate scratch data and output.** Use `target/manual-dev/codex/` and `target/manual-dev/claude/`, disable role dispatch for deterministic tests, and leave the Owner's live store/processes/credentials unchanged. A separate target directory prevents output contention, not shared-source changes. Exploratory checks while the other lane edits are provisional; milestone acceptance uses stable relevant source.

If two tasks need the same core file, serialize that part. If shared-checkout friction repeatedly interrupts useful coding in waves 4-6, the Owner may choose **one separate Claude worktree for that stretch**; keep Codex in the current directory, transfer complete reviewed commits normally, and retire the extra checkout only when its work is preserved. This is an optional manual fallback, not the default or an action authorized for an agent to perform without the Owner choosing it. No worktree is created by this plan.

## File ownership

Existing source references below identify real files. Proposed paths are allocations for future packets, not claims that those modules exist today.

| Owner | Exclusive writable scope | Other lane's interaction |
| --- | --- | --- |
| Codex — shared core and integrator | Existing `src/lib.rs`, `src/migrations.rs`, `src/workflow.rs`, `src/memory.rs`, `src/mcp.rs`, `src/dispatcher.rs`, `src/worktree.rs`, `src/app_server.rs`, `src/roles.rs`, `src/telemetry.rs`, `src/optimization.rs`; relevant existing tests and scripts | Claude reads/reviews and proposes scoped patches. |
| Codex — new shared foundations | Proposed `src/agent_provider.rs`, `src/execution_artifacts.rs`, `src/project_checkpoints.rs`, `src/provider_routing.rs`, shared session/availability persistence and public contract types | Claude implements against the merged/frozen public contract, without parallel alternatives. |
| Codex — shared configuration and canonical records | Production Cargo/package manifests and lockfiles, module exports, AGENTS.md, existing architecture/definition/contract/roadmap/PLAN-020, this schedule and kickoff state | Claude owns only its separate scratch manifest/lockfile; integrator applies accepted production updates. |
| Claude — design and review outputs | Proposed `docs/parallel/claude/` for CC packet contracts, acceptance matrices, threat models, experience specs and review reports | Codex reads only while Claude edits; accepted contract revisions are frozen by the manual handoff. |
| Claude — bounded implementation | Proposed `src/claude_code.rs` and `src/routing_policy.rs`, with harness-specific fixtures and a compilable standalone scratch crate, wrapper and tests | Claude compiles/tests actual source locally; Codex handles final module/test registration and shared wiring at an integration pause. |
| Codex — review outputs | Proposed `docs/parallel/codex/` for CX packet evidence and review reports | Claude reads/reviews. |
| Neither during active headless track | `apps/desktop/src/App.tsx`, `apps/desktop/src/style.css`, new visual frontend features or global provider settings | UI implementation remains paused; design-only proposals are allowed. |

### Claude's compilable scratch crate

Claude may leave its production modules unregistered while developing, but must compile and test their **actual source** before integration. It owns a throwaway standalone crate at `target/manual-dev/claude/scratch/`, including that crate's `Cargo.toml`, lockfile, wrapper and tests. Its own empty `[workspace]` makes it independent of Cathulu's package membership; the local dependency still uses Cathulu's real public types.

```toml
[package]
name = "cathulu-claude-scratch"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
cathulu-core = { path = "../../../.." }
```

The path is relative to the scratch manifest, four directories below the repository root. Add only the dependencies actually needed by Claude's module/tests to this scratch manifest, using the production versions; do not edit production manifests as a side effect. Seed the scratch `Cargo.lock` from the committed production lock before its first offline resolution. This preserves already locked cached versions (including any now-yanked version) instead of forcing a fresh offline selection; Cargo may update only the scratch lock to add the scratch package. Use `--locked` for subsequent unchanged scratch runs once that lock is generated. In the scratch wrapper re-export the actual F1 public modules (for example `pub use cathulu_core::agent_provider;`) so production code using `crate::agent_provider` resolves to those types. Include the source under development directly, rather than copying it or redefining the contract. From `scratch/src/lib.rs`, the adapter example is:

```rust
pub use cathulu_core::agent_provider;
#[path = "../../../../../src/claude_code.rs"]
pub mod claude_code;
```

Only use names F1 actually exports, and add the routing-policy include when that file exists. If an implementation needs a private core helper, request an agreed public seam instead of inventing a substitute. With the real F1 types available, Claude runs its own iteration loop:

```powershell
# From the Cathulu repository root; --target-dir is the command-local equivalent
# of assigning a separate CARGO_TARGET_DIR for this scratch process.
# First create the standalone manifest/wrapper and copy Cargo.lock into the scratch crate.
cargo check --manifest-path target/manual-dev/claude/scratch/Cargo.toml --target-dir target/manual-dev/claude/build --offline
cargo test --manifest-path target/manual-dev/claude/scratch/Cargo.toml --target-dir target/manual-dev/claude/build --offline
```

The scratch crate and its output stay ignored under `target/`; complete reusable fixtures/tests are transferred to the repository's normal test locations during integration. Do not commit build output or depend on a production `target/` path. Preserve the source, tests and exact scratch commands/results in Claude's packet before discarding the throwaway wrapper. Do not run shared `cargo clean` or delete the shared target tree while either lane's scratch work is still there. No empty passing adapter stub is used as a substitute for compilation.

A path dependency reads the **live shared core**. If Codex is halfway through a core edit or changes F1, Claude may see a transient dependency failure. Finish the small core change or request a short core-write pause, then retry; do not misclassify it as an adapter defect or claim independent checkout isolation. If that becomes recurring overhead, use the optional Owner-selected worktree fallback.

Cargo behavior is grounded in its official [workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html), [path dependency](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html) and [target-directory](https://doc.rust-lang.org/cargo/reference/config.html) documentation, checked through Context7 on 2026-10-25.

## Milestone ownership at a glance

| PLAN-020 milestone | Implementation owner | Claude Code assignment | Codex assignment |
| --- | --- | --- | --- |
| M48 tasks / portfolio | Codex | CC-01: challenge semantics and supply edge cases; review important gaps | CX-01: schema, task/dependency/blocker commands, scoped MCP and tests |
| M49 checkpoint / resume | Codex | CC-01/CC-02: resume experience, stale-state and provenance critique | CX-02: immutable checkpoints, bounded resume projection, restart/no-provider proof |
| M50 personal / non-code projects | Codex for persistence | CC-02: identity/privacy/retention contract and adversarial cases | CX-06: optional repository identity, controlled personal scope and migration/permission tests |
| M51 artifacts / provider boundary / Codex parity | Codex | CC-03: boundary/permission/session contract and critique | CX-03/CX-04: shared types, session delivery receipts and CodexAdapter integration |
| M52 workspaces / recovery | Codex | CC-03/CC-06: threat model and semantic/recovery review | CX-05: leases, writer allocation, read-only coordinators and crash/retry checks |
| M53 ClaudeCodeAdapter | Claude for adapter; Codex for shared wiring and independent QA | CC-04: feasibility, adapter code, parser/error/permission tests and honest limitations | CX-07: scoped integration, cross-contract/failure/recovery validation; live evidence only when authorized |
| M54 routing / handoffs | Split at a frozen interface | CC-05: pure task-shape/risk routing policy code and scenario tests | CX-08: availability persistence, queue integration, handoff/reconciliation and public tests |
| M55 additional roles / empirical proposals | Codex for shared implementation | CC-06/CC-07: actual role need, scoped design, convergence assumptions and conceptual review | CX-09: bounded role/capability migrations and reproducible evidence reports |
| Future UI, conditional | Claude for experience and novel UI; Codex for explicit integration/testing | Design now; source implementation only after the Owner resumes the paused UI track | Then own IPC/state wiring, accessibility/regression/performance verification in disjoint files |

## Manual wave schedule and dependencies

Waves 0-3 are plain implement-then-review: useful analysis overlaps implementation, but there is no claim of two concurrent coding lanes and no per-packet freeze ceremony. Waves 4-6 are the real concurrent-coding window. The packet IDs are scope references, not 17 separate ceremonies.

These are readiness waves, not calendar promises. An agent finishes the current bounded packet and reports; the Owner manually supplies the next. There is no need to keep a model generating while waiting.

| Wave | Claude Code lane | Codex lane | Checkpoint / overlap limit |
| --- | --- | --- | --- |
| 0 — committed launch and preflight | CC-01: M48/M49 acceptance/experience review | CX-00: confirm shared-workspace ownership and baseline; check existing M42/M44 evidence needs | Completed at `72815dc21e3a12bd55539b68191054e3ac0b48fa`; required planning files were tracked and status clean. No fabricated live task. |
| 1 — useful tasks | CC-02: M50 domain/privacy contract | CX-01: implement M48 using the already defined PLAN-020 baseline | Claude's advisory improvements do not silently change in-flight acceptance. |
| 2 — resume | CC-03: provider/artifact/session/security contract | CX-02: implement M49 | Implement, then review/validate M48 and M49 normally; no two-agent write freeze. Provider-free value does not wait for Claude installation. |
| 3 — common seam | Finalize/review CC-03; prepare safe Claude fixtures and policy scenarios without runtime integration | CX-03: implement M51a, the minimal exported common contract and fixture seam | **F1:** review/commit the actual shared types, payload examples and supported operations before adapter code assumes them; this is an interface agreement, not a write-pause ceremony. |
| 4 — substantial parallel coding | CC-04: implement, compile and test the Claude adapter against F1 in its scratch crate | CX-04: finish M51 Codex parity, then CX-05: M52 workspace/recovery | M53 preparation overlaps M51b/M52. M53 activation and milestone completion still wait for all original prerequisites. |
| 5 — policy + domain | CC-05: implement pure routing policy against F1 (and an explicit reviewed amendment if needed) | CX-06: M50 persistence, only for the defined categories/identity scope | Claude compiles/tests its policy module in the scratch crate; Codex writes persistence. Personal collection remains gated by P-Q2. |
| 6 — integrate providers and handoffs | CC-06: architecture/threat review of M50/M52 and both integration diffs | CX-07: M53 integration/QA; after M53 acceptance, CX-08: M54 scheduler/availability/handoffs | Use one short integration pause for complete modules and the combined checks. Required live/access evidence cannot be replaced by protocol fixtures. |
| 7 — selective expansion | CC-07: actual specialist/dependency need and evidence/routing review | CX-09: only the justified parts of M55 | Requires original M50/M54 dependencies and a real scope need; no speculative permanent hierarchy. |

PLAN-020 M51 is split for development scheduling into **M51a shared seam** (CX-03) and **M51b Codex parity** (CX-04). They are not separate claims of milestone completion. Similarly M53 preparation can be written/tested offline after F1 while M52 progresses, but M53 activation/acceptance still requires M52, safe subscription tooling, permission boundaries and actual evidence. M54's pure policy can be prepared before provider activation; its integrated milestone cannot complete early.

Existing PLAN-015 M42 / PLAN-017 M44 keep priority when real already authorized operational work is available. CX-00 identifies current evidence; CX-05/CX-07 arrange applicable scoped checks within existing authorization. Do not require a broken dispatcher, an invented project assignment or a new cohort before independent local tasks can proceed. Manual coordination here does not call Cathulu to dispatch its own development sessions.

## Claude Code packets

### CC-01 — Task/resume contract critique

Goal and scope: make M48/M49 behavior precise without redesigning the whole product or blocking an already clear implementation.

Subtasks:

- [ ] Read PLAN-020, project definition, workflow/task code and the first-slice gate.
- [ ] Write examples for parent completion, cancelled/failed prerequisites, manual blockers, explicit priorities, unknown effort/due dates, paused projects and a stale checkpoint.
- [ ] Separate required corrections to existing intent from optional additions; identify source/rationale and affected acceptance.
- [ ] Review CX-01/CX-02 stable diffs for conceptual gaps when the Owner provides them.

Ownership/output: `docs/parallel/claude/CC-01-task-resume.md`; no shared source edits. Dependencies: current baseline only. Acceptance: examples agree with existing task/objective/QA authority or name the exact conflict; a compact expected resume packet includes freshness/provenance and never implies provider use. Validation: trace examples against real code/PLAN-020; no unsupported claim of implementation. Documentation: this packet plus review findings. Skills: implementation-plan, ponytail-balanced, ui-ux-research for experience questions. Git checkpoint: included by the integrator in the next normal checkpoint; no Claude Git mutations.

### CC-02 — Personal/non-code identity and privacy design

Goal and scope: settle the semantic/security choices needed by M50; design the future home/resume information order without implementing desktop UI.

Subtasks:

- [ ] Propose zero-or-one repository association while preserving project IDs/PM scopes; identify affected triggers/readers.
- [ ] Define accepted personal categories, correction/retention/deletion expectations and explicit provider disclosure; leave Owner facts/consent unanswered rather than infer them.
- [ ] Write denial/edge-case scenarios for global vs personal scope, cross-project reads, stale summaries and non-code projects.
- [ ] Add a short future home/resume content specification using the current product language; classify existing graph reuse as ADAPT and new component dependencies as DEFER.

Ownership/output: `docs/parallel/claude/CC-02-personal-project-contract.md`; optional experience section in the same file. Dependencies: CC-01 for task vocabulary; P-Q2 must be resolved before collection/disclosure, not before drafting. Acceptance: explicit data/authority boundaries, migration implications and unresolved decisions; no invented user profile, fake Git path or UI-unpause claim. Validation: source/trigger review and attack scenarios, plus later keyboard/focus/200%-zoom/offline/stale-state requirements for any resumed UI. Documentation: packet; integrator promotes accepted decisions into canonical records. Skills: implementation-plan, ponytail-balanced, ui-ux-research; impeccable only if later UI work is actually resumed. Git checkpoint: reviewed contract in a normal checkpoint.

### CC-03 — Provider, artifact, routing-input and recovery contract

Goal and scope: define the smallest common seam that two actual adapters and one router can share.

Subtasks:

- [ ] Review current app_server, dispatcher, memory known-by/transfer, role capability and event/artifact code.
- [ ] Specify logical role vs attempt vs provider-session identity; new-session context delivery; schema/payload bounds; unknown/unsupported operations; approval and completion semantics.
- [ ] Define narrow pure routing inputs/outputs and availability observations needed by M54, with Owner policy above provider preferences.
- [ ] Threat-model duplicate submission, lease loss, stale task revision, approval forgery, ambient configuration and personal-data disclosure.
- [ ] Review CX-03's actual exported types and sample fixtures before F1 is recorded; propose an amendment if code and contract differ.

Ownership/output: `docs/parallel/claude/CC-03-provider-contract.md`; no migration or dispatcher edits. Dependencies: existing architecture and current source; F1 needs actual CX-03 code. Acceptance: both adapters can represent the same task/result/error without vendor concepts becoming canonical truth; genuine provider limitations remain explicit; fields are limited to current callers. Validation: walk one success, denied action, lost acknowledgement, exhausted capacity and replacement-session scenario through the contract. Documentation: versioned contract and threat model, with accepted revision/commit in handoff. Skills: implementation-plan, ponytail-balanced, Context7/current vendor docs, openai-docs for Codex details. Git checkpoint: contract plus CX-03 exported seam reviewed and committed together at F1, without the concurrent-coding pause protocol.

### CC-04 — Claude adapter implementation (M53 preparation)

Goal and scope: implement the bounded ClaudeCodeAdapter against F1, including its own tests; this is a coding assignment.

Subtasks:

- [ ] Verify current supported harness/version/configuration assumptions with official docs and available local evidence. A missing installation or subscription-safe envelope is a recorded prerequisite, not permission to install or change auth.
- [ ] Implement only supported lifecycle/stream/error/permission behavior in `src/claude_code.rs`, using the actual common types.
- [ ] Supply complete harness fixtures and tests for success, malformed/truncated events, auth/capacity errors, denied permissions, interruption and unknown session state.
- [ ] Demonstrate that failure cannot mark task success, widen tools or silently switch to API billing; retain the known `--bare`/subscription feasibility question until resolved.
- [ ] Compile/test the actual production module through the standalone scratch crate against F1 before handing it off; then provide complete code/test entry points for the short integration pause.

Ownership/output: `src/claude_code.rs`, Claude-specific fixtures, and its standalone crate/wrapper/tests under `target/manual-dev/claude/scratch/`; `docs/parallel/claude/CC-04-adapter-evidence.md`. Codex owns final test registration/shared wiring. Dependencies: F1; activation also requires M51b/M52 and P-Q1/disclosure/scope prerequisites. Acceptance: module plus meaningful tests implement F1 without duplicate provider/session authority; supported vs unverified behavior is enumerated. Validation: Claude runs scratch `cargo check` and contract/parser/error tests during iteration; Codex then runs registered production/contract tests at integration; real authenticated task only in CX-07 after prerequisites. Documentation: exact observed version/features and limitations. Skills: milestone-delivery, ponytail-balanced, Context7 and official Claude docs. Git checkpoint: compile-tested adapter preparation in an integration checkpoint; M53 remains incomplete until activation evidence passes.

### CC-05 — Pure routing policy implementation (M54 preparation)

Goal and scope: implement decision semantics without touching persistence, queues, provider processes or permissions.

Subtasks:

- [ ] Implement `src/routing_policy.rs` using frozen task-shape/risk/availability inputs and explicit versioned policy.
- [ ] Return deterministic eligibility/exclusion/rationale, appropriate fallback or queue outcome; preserve unknown capacity.
- [ ] Cover ambiguous UX/architecture work, bounded executable work, unavailable providers, mandatory review, denied disclosure and both providers unavailable.
- [ ] Ensure an unavailable opposite-harness reviewer does not silently waive required review; deterministic administrative operations request no LLM.

Ownership/output: routing-policy module, compile-tested scratch scenario tests/fixtures, `docs/parallel/claude/CC-05-routing-evidence.md`. Dependencies: F1 and a frozen routing input schema; CC-04 need not be live. Acceptance: same inputs/policy yield the same decision, with policy and security constraints taking precedence over priors; no source-level claim of provider superiority. Validation: Claude compiles/tests table-driven positive/negative cases through the scratch crate before handoff; the normal test locations are wired at integration; Codex independently verifies integration and scope denial. Documentation: policy version, cases and rationale. Skills: milestone-delivery, ponytail-balanced. Git checkpoint: pure-policy candidate before CX-08 integration, not completion of M54.

### CC-06 / CC-07 — Consequential reviews and justified hierarchy design

Goal and scope: challenge M50/M52/M53/M54 and later M55 without taking over the implementation lane.

Subtasks:

- [ ] Review stable integration diffs against privacy, session delivery, role/worker caps, workspace ownership, stale acceptance and recovery invariants.
- [ ] Return actionable findings with exact source/evidence and required tests; separate material blockers from suggestions.
- [ ] Propose only specialist/dependency scopes justified by actual work; inspect convergence/comparison assumptions and insufficient-evidence behavior.

Ownership/output: separate CC-06 and CC-07 reports under `docs/parallel/claude/`. Dependencies: relevant completed diffs; M55 implementation still needs its original prerequisites. Acceptance: reviews cannot self-accept Claude-authored code, invent real usage or widen project access; M55 proposal names its actual need. Validation: evidence-backed scenario review and reproduction of critical findings; Codex verifies resulting code/test repairs. Documentation: findings/dispositions and any proposed ADR. Skills: milestone-delivery, ponytail-balanced, Graphify when a fresh scoped graph exists, release-readiness only for actual rollout. Git checkpoint: review record and fixes in the appropriate validated batch; no separate pause for a read-only review.

## Codex packets

All CX packets preserve PLAN-020 acceptance, existing role/QA/memory/approval gates and unrelated work. The affected-file column uses the exclusive ownership table; no production change is authorized merely by writing this schedule.

| Packet | Goal and explicit work | Dependencies | Required acceptance / validation | Documentation and Git checkpoint |
| --- | --- | --- | --- | --- |
| **CX-00 — preflight** | Confirm commit/diffs, same-directory ownership, Git writer and isolated scratch settings. Reuse PR #33's evidence; inspect current M42/M44 facts only when relevant. Identify already authorized live tasks without manufacturing one. | Owner launches packet; both sessions know file rules. | Baseline and exact writable scope recorded; unknown live state stays unknown. Reproduce only checks needed for changed evidence/environment, rather than rerunning everything for reassurance. | `docs/parallel/codex/CX-00-preflight.md`; no branch mutation while Claude edits. |
| **CX-01 — M48** | Implement project/task metadata, guarded transitions, dependency/blocker validation, stable eligibility queries and scoped Store/MCP boundaries; retain objective ownership and QA. | Existing first-slice definition PASS; CX-00. CC-01 is advisory unless it identifies a material contradiction. | Populated schema upgrade/restart, cycles/self-edge/cross-scope denial, stale revisions, cancelled/failed prerequisites, parent completion, administrative-vs-execution completion and zero-provider checks. | Current domain/API docs and evidence; validated M48 checkpoint reviewed by Claude on material semantics. |
| **CX-02 — M49** | Implement immutable checkpoint revisions and bounded resume projection using current tasks, blockers, decisions and freshness. Add only needed milestone state. | M48 checkpoint. | Missing/stale checkpoint or repo, changes since checkpoint, scope denial, immutable revisions, restart and no-provider public workflow all pass. | Checkpoint/resume contract and evidence; validated first useful pivot slice. |
| **CX-03 — M51a** | Implement minimal common AgentProvider/artifact/session/routing-input types and safe test seams from CC-03. Namespace provider IDs and make unsupported features explicit. | M49; reviewed CC-03 contract. | Actual exported code compiles; payload examples validate; deterministic double demonstrates success/error/unknown handling. No provider activation, fake adapter parity or speculative plugin framework. | Publish F1's commit/revision, type locations and test fixtures; shared seam checkpoint. |
| **CX-04 — M51b** | Wrap existing AppServerProcess as CodexAdapter; keep Cathulu policy and durable queues separate. Add context delivery receipts and compatibility readers. | F1. Can overlap CC-04's independently compile-tested scratch work; the live core path dependency remains shared. | Existing plan/report/capability/event gates pass; duplicate/out-of-order events and replacement-session delivery tested. All original M51 criteria required before marking M51 complete. | Actual common/Codex contract, session migration and evidence; frozen parity checkpoint. |
| **CX-05 — M52** | Add exclusive product writer-workspace allocation/leases, read-only coordinators, ownership/revocation and uncertain-submission recovery; reuse existing verification. | M51b. Can overlap CC-04/CC-05's owned preparation. | Disposable Git paths/aliases/common-dir, lease contention/loss, semantic overlap, no duplicate turns, safe cleanup, crash/retry and applicable M42/M44 evidence. This product isolation is distinct from the Owner's manual development exception. | Recovery/security/workspace evidence and current live limits; frozen runtime-safety checkpoint. |
| **CX-06 — M50** | Implement the accepted personal/non-code contract: preserve IDs/roles, optional repository association, controlled personal scope and semantic commands. | M49 and reviewed CC-02; P-Q2 before collecting/disclosing personal facts. Normally after CX-05 to unlock adapter concurrency first. | Mixed populated migration, trigger/FK/FTS/restart, no fake Git paths/calls, global-personal leakage denial and defined correction/retention scenarios. Independent provider-free work continues if consent is unresolved. | Data/permission model and decision evidence; frozen M50 checkpoint. |
| **CX-07 — M53 integration** | At a short integration pause register complete, scratch-tested CC-04 code/tests, perform shared wiring and independent adapter/security QA. Repair integration defects in Codex-owned files; return adapter-local defects to Claude. Arrange actual scoped provider acceptance only when its gates pass. | CC-04 candidate, M51b/M52, safe harness/auth/disclosure/tool prerequisites. | Common contract plus malformed/auth/quota/permission/interruption/recovery tests; genuine bounded task/reconciliation evidence for live acceptance. Fixture pass alone leaves live M53 open. | Exact supported features, observed environment, acceptance/unknowns; frozen Claude integration checkpoint. |
| **CX-08 — M54 integration** | Persist availability and routing attribution; invoke CC-05 pure policy; wire queues, typed Plan/Review handoffs and idempotent acceptance/checkpoint/memory reconciliation. | M53 acceptance, CC-05 candidate, M52 and applicable disclosure grants. | Both handoff directions, required-review wait, stale task/HEAD, repeated events, provider loss, appropriate queue/fallback and zero-provider administrative queries; full original M54 proof. | Accepted routing/persistence/handoff contract and evidence; frozen mixed-provider checkpoint. |
| **CX-09 — M55** | Implement only justified role/capability extensions and reproducible convergence/evaluation projections from CC-07; preserve role keys/caps and advisory-only promotion. | M50/M54 as applicable, actual scope need and reviewed design. | Migration/isolation/cap/review/replay cases; uncalibrated/insufficient-evidence output; real held-out evidence before comparative/calibrated claims. | Role/permission ADR if needed, source docs and cohort limits; one frozen capability checkpoint at a time. |

Applicable skills: milestone-delivery and ponytail-balanced for each implementation; Context7 for current library/CLI/protocol details, openai-docs for Codex integration, existing Rust/MCP test conventions, Graphify only when fresh/scoped/available, release-readiness before meaningful rollout. Use the selected [development profile](../pivot-migration.md#development-skills--tooling-profile); no missing skill/tool is silently assumed available.

## F1 contract-freeze contents

The Owner relays one accepted packet containing the baseline/checkpoint, contract revision and commit, exported type/signature locations, sample WorkOrder/event/result/error payloads, permission/context manifest semantics, unsupported-operation behavior, task revision/reconciliation ownership, routing inputs/outputs and meaningful fixture entry points. Claude and Codex work against that exact revision. A change is a short written amendment describing consumers and tests affected; update both lanes before coding against it. No extra service, automatic lock or coordination agent is needed.

## Manual integration and definition of done

For waves 0-3, use the ordinary implement -> review -> checks -> commit flow. Claude's read-only review or separate document packet needs no extra freeze. The single code writer controls its test inputs. Neither session changes the shared branch underneath active edits.

For a combined integration in waves 4-6 only:

1. Owner relays **PAUSE WRITES**; both agents acknowledge and identify any unfinished files. Codex runs `git status --short --untracked-files=all` and `git diff HEAD`, then reads the complete contents of candidate untracked files. Ordinary `git diff` does not include those files. Stage only the explicit complete paths.
2. Review the relevant combined diff, wire complete scratch-tested Claude modules/tests, and fix material findings in owned files. Do not create a per-file hash manifest or a separate review ceremony for every packet.
3. With writes still paused, run the milestone's relevant tests/builds. Recheck status/diff, inspect any changed untracked candidate, and rerun affected checks if relevant code changed. Git's view plus the acknowledgements is the agreed procedural safeguard; it is not an OS lock or proof against an uncooperative writer. Keep exact commands/results and the normal source/build/live evidence distinctions.
4. Update canonical docs/progress, commit the complete validated contribution and perform authorized push/merge/checks. Never stash/reset/clean another lane's unfinished work. Resume assignments at the next normal handoff; neither model waits in a generating loop.

A batch is done when its acceptance, required checks, review findings, documentation and applicable Git progression are recorded. Producing a design, unregistered module, passing fixtures or queue acknowledgement alone never completes the corresponding live capability milestone. Routine low-risk documentation does not need dual review; the substantive integration/security checkpoints above do.

## Handoff report template

```text
Packet / agent:
Baseline commit and accepted contract revision:
Changed paths (including new/untracked files); incomplete work:
Scope delivered; source evidence / key decisions:
Commands and exact results (scratch/targeted vs integrated acceptance):
Fixture / build / loaded / live / benefit evidence, separately:
Schema, dependency or shared-file changes requested from integrator:
Review findings and disposition:
Unresolved prerequisite and affected scope:
Next packet; pause acknowledgement only if concurrent integration requires it:
```

The Owner copies these reports between the two sessions. Keep them compact; pass the specific diff/contract/evidence, not full conversation transcripts. An unclear high-impact requirement, changed contract, repeated same failure or ownership conflict triggers a report and bounded replan, not endless same-strategy retries.

## Starting the two sessions

First commit/integrate these planning files and verify a clean working tree. Then both sessions record the same observed HEAD and branch in the existing Cathulu directory. Do not start implementation against the uncommitted planning draft or reset to the historical `d2422fb` inspection hash. Give Claude [the Claude starter packet](../handoffs/manual-claude-code-start.md) and Codex [the Codex starter packet](../handoffs/manual-codex-start.md). These are prompts for the Owner to copy, not automatic messages. The planning session commits/integrates the documents under the Owner's request; it launches no development session or coordination automation and creates no worktree.

## Risks, rollback and final integration

The main risk is accidental shared-file/Git/build interference. Ownership and explicit pauses reduce it but are procedural safeguards, not operating-system isolation. If acknowledgement is missing, shared ownership is unclear or unexpected edits appear, stop the affected shared operation; retain both agents' files/diffs and resolve ownership. Do not use stash/reset/clean to erase the conflict.

Migration rollback continues to require consistent SQLite backups and compatible binaries; an old v18 binary cannot read a newer schema. No live upgrade belongs to a concurrent editing phase. Reverting a combined commit also affects both contributors: identify their changes and use a reviewed recovery commit, preserving subsequent work.

Final product integration retains PLAN-020's two proofs: provider-free tasks/checkpoint/restart/resume, and authorized typed work through provider/workspace/QA/review/reconciliation. Add the manual-workflow proof that every shared Git/acceptance operation used a stable tree and no agent's unfinished work was overwritten. No automatic release or learned-policy promotion follows.

## Progress and evidence

- [x] Reused PLAN-020 and the existing first-slice definition; preserved capability/activation gates.
- [x] Corrected the baseline: `d2422fb` was the original inspection, followed by uncommitted planning changes; launch requires the revised documents committed and a newly observed clean HEAD.
- [x] Removed the assumption that a fresh Codex session inherits PR #33 conversation familiarity; both sessions receive committed context.
- [x] Made waves 0-3 ordinary implement-then-review and waves 4-6 the actual concurrent-coding stretch.
- [x] Added Claude's standalone scratch-crate compilation/testing route, its own manifest/lock/output and the live path-dependency caveat.
- [x] Replaced per-file hash manifests with pause acknowledgements and Git status/diff, explicitly covering untracked files.
- [x] Kept a single Claude worktree in waves 4-6 as an Owner-selected fallback; no worktree created.
- [x] Validate the revised documentation and scratch-crate feasibility; record exact scope/results.
- [x] Complete planning checkpoint committed and pushed in [PR #34](https://github.com/jmsb505/cathulu/pull/34); its final merge outcome and the clean observed HEAD must be checked before manual sessions launch.
- [ ] Owner launches sessions; all implementation and actual manual-collaboration outcomes remain unverified.

The initial draft's 58-file / 521-link static check was evidence about that draft, not a clean committed launch state. Revised documentation, scratch-probe and delivery evidence is recorded here after the corresponding checks. No model/provider run, product code change or automatic dispatch belongs to this planning repair.

Revised validation (2026-10-25): 58 Markdown files and 521 local links/anchors passed; all 17 packet references and eight M48-M55 ownership rows remain covered. JSON claim/question identities are unique, obsolete familiarity/hash-manifest instructions were removed, and `git diff --check` passed. Production source, tests, configuration and lockfiles are unchanged from the source baseline.

Scratch feasibility probe: a standalone crate under ignored `target/manual-dev/claude/plan-021-probe/` used an empty workspace, a real core path dependency and a wrapper including the actual `src/terminal.rs` with real exported Cathulu types. Its isolated offline `cargo test` compiled successfully and passed one existing module test. Initial fresh offline resolution rejected a cached yanked Tokio version; seeding only the scratch lock from the committed root lock resolved it. This validates the proposed wrapper/build route, not future F1 signatures, Claude adapter behavior or product integration. The production lock was not modified.

Delivery prerequisite: the complete revised planning/canonical-document patch must be committed and integrated before manual launch; its PR records the final merge outcome. Observe the resulting HEAD and clean Git status at launch rather than embedding a self-referential commit ID in these files.

The validated document checkpoint is `1af69c4efdd34df6f4c66dc158c0ae1f8ff58bd3` on `codex/manual-plan-review-fixes`, submitted as PR #34. This delivery note is a documentation-only follow-up in that PR. No early implementation or manual-agent launch is inferred from PR submission.
