# CC-03 — Provider, artifact, session, routing-input and recovery contract (M51 / M52 / M54 seam)

Agent: Claude Code lane, PLAN-021 wave 2-3. Design input for CX-03 (M51a shared seam), CX-04 (M51b Codex parity), CX-05 (M52), CC-04 (Claude adapter) and CC-05 (routing policy).
Baseline: branch `codex/manual-dual-m48`, HEAD `72815dc`. Codex had uncommitted edits in `src/lib.rs`, `src/migrations.rs`, `src/workflow.rs` (CX-01 in progress) when I read; I did not open those diffs and the line references below are to the **committed** HEAD files, which may shift.
Read: PLAN-020 M51-M54, PLAN-021 CC-03/F1, pivot-migration §§8, 13, 15-18; code in `src/app_server.rs` (all), `src/dispatcher.rs` (types, role tools, authorization, queue, lease, bind, loop, dispatch, events, transitions), `src/delegation.rs` (packet/report/plan/activation types, activation, report submit), `src/memory.rs` (`prepare_context_transfer`), `src/migrations.rs` v12-v14 (roles, bindings, outbox), `src/events.rs`, `src/roles.rs`.

**Evidence level.** Everything stated as "today" is **source-read** at HEAD. Nothing was run. Nothing here is a claim about the Claude Code CLI's behavior: `claude` is not on this machine's PATH (`claude --version` → command not found, checked 2026-10-25), so every Claude-side statement is a *requirement the adapter must verify against official docs and a real install in CC-04*, never an observed fact.

**F1 gate.** The final subtask (review CX-03's exported types/fixtures) cannot be done: `src/agent_provider.rs` and `src/execution_artifacts.rs` do not exist yet. §9 is the checklist I will apply to that code when it appears. Until then this document is the contract *proposal*, not an agreed revision.

---

## 1. What the code does today

| # | Observation | Where |
| --- | --- | --- |
| F1 | **Codex protocol is the only provider.** `AppServerProcess` speaks JSON-RPC over stdio to `codex app-server`: `initialize`, `thread/start`, `thread/resume`, `mcpServerStatus/list`, `turn/start`; reads notifications `turn/started`, `turn/completed`; server requests (approvals) are answered `decline` or rejected. Method names, `/thread/id`, `/turn/id`, `approvalPolicy: on-request`, `sandbox: workspace-write` are hard-coded in the client and in the dispatcher. | `app_server.rs:111-225`, `dispatcher.rs:2154-2160`, `2629-2737` |
| F2 | **Provider acknowledgement ≠ completion.** `start_turn` returns on the RPC ack; completion arrives later as `turn/completed`, which only sets the *dispatch queue* row to `completed|failed`. A completed turn with no `submit_delegation_report` leaves the delegation `dispatched`. The authoritative result is the agent's own call to the `submit_delegation_report` MCP tool. | `app_server.rs:173-188`, `dispatcher.rs:2651-2695`, `delegation.rs:2291-2360` |
| F3 | **Session identity is overwritten.** `role_thread_bindings` has `binding_key PRIMARY KEY` (= `role_key` by trigger, or `worker:<delegation id>`), `thread_id UNIQUE` plain text with **no provider namespace**, and `bind_role_thread` does `ON CONFLICT(binding_key) DO UPDATE SET thread_id = …, last_turn_id = …`. The identity-guard trigger protects `binding_key/role_key/delegation_id/project_id/created_at` but **not** `thread_id`. A replacement session therefore replaces the stored id; no history of earlier sessions or turns is kept. Rows can't be deleted. | `migrations.rs:1003-1019`, `1054-1073`, `dispatcher.rs:1654-1689` |
| F4 | **Attempt = a counter.** `role_dispatch_outbox` is one row per delegation (`delegation_id UNIQUE`); `attempt_count` is incremented at claim; there is no per-attempt row, so no stable attempt id to key idempotency, provider ids or reconciliation on. `needs_reconcile` can only move to `blocked|failed|cancelled` — never back to `queued`. | `migrations.rs:1021-1034`, `dispatcher.rs:1423-1430`, `2784-2810` |
| F5 | **Uncertain delivery is handled conservatively.** On lease acquire, any outbox row in `preflighting|thread_ready|turn_starting|dispatched|running` becomes `needs_reconcile`; on transport close/error, active turns become `needs_reconcile`; losing the lease before `turn/start` returns `needs_reconcile`. No automatic redispatch. A crash between `turn/start` being written and its ack is therefore detected as uncertain, but nothing can *resolve* it other than failing/blocking/cancelling it. | `dispatcher.rs:1541-1581`, `1984-2060`, `2204-2223`, `2747-2782` |
| F6 | **Memory "delivery" is keyed to the logical role, and is self-reported.** `prepare_context_transfer` returns `omitted_known` when `(memory_id, receiver_scope_key, content_hash)` exists, and *records* known-by at decision time, before anything is shown to a recipient. The receiver scope is the logical role (`scope:project-manager:<id>`), so a brand-new provider session for the same role is told "already known". The activation receipt (`record_delegation_activation`) is written by the *agent* through a tool, with `memory_tools_available`/`retrieval_completed` self-reported. The role prompt tells the agent to call `read_memories` itself. | `memory.rs:1054-1076`, `1137-1146`, `delegation.rs:1907-2000`, `dispatcher.rs:2608-2626` |
| F7 | **Capabilities are per-binding tokens.** Each dispatch issues a token passed to the spawned Cathulu MCP server via `CATHULU_ROLE_CAPABILITY`; tool visibility is filtered by `role_allowed_tools` and *enforced* per call by `authorize_role_tool_call` using the stored assignment, not by the model's claims. Tokens are revoked on failure paths. | `dispatcher.rs:117-149`, `644-780`, `782-892`, `2576-2599` |
| F8 | **Provider approval requests are never granted.** A server request marks the dispatch/inbox row `blocked` ("Owner approval … required; Cathulu did not approve it") and replies `decline` (commands/file changes) or an error. The payload is intentionally not read. Cathulu's own Owner approval records (`owner_approval_requests/decisions`) are a separate path and are not linked to provider-side request ids. | `dispatcher.rs:2698-2730` |
| F9 | **Typed local artifacts already exist** with `schema_version`, `deny_unknown_fields` and hashes: `DelegationTaskPacket`, `WorkerReport` (≤16 KiB, one per delegation), `MainWorkflowBrief`, `ProjectManagerWorkPlan`, activation receipts. Submissions use idempotency keys hashed together with the request hash; reuse of a key for different content is rejected. Events are ≤32 KiB; RPC lines ≤8 MiB; prompt ≤64 KiB. | `delegation.rs:32-150`, `2291-2348`, `events.rs:14`, `app_server.rs:22`, `178` |
| F10 | **Workers run in the registered root.** `start_thread` cwd = `project.path` for both PM and Worker, sandbox `workspace-write` (M52's gap). | `dispatcher.rs:2154-2160` |
| F11 | **Single global dispatcher lease**, 45 s, renewed every 10 s; on renewal failure active turns are marked `needs_reconcile` and the server dropped. | `dispatcher.rs:1984-2030` |
| F12 | **Duplicate/out-of-order provider events are tolerated only in memory.** `active_turns` is an in-memory map; a duplicated `turn/completed` after removal is ignored, but nothing persists a per-event identity. `append_external_event` (deduped by `external_event_id`) exists but the dispatcher does not use it for provider events. | `dispatcher.rs:1977-1982`, `2636-2696`, `events.rs:27-58` |

---

## 2. Identity model (four ids, never conflated)

| Concept | Meaning | Owner of truth | Today |
| --- | --- | --- | --- |
| **Logical role** | `main-overseer`, `project-manager:<id>`, a Worker slot for a delegation. Durable, provider-independent. | Cathulu (`logical_agent_roles`) | exists (F3) |
| **Attempt** | One try to execute one WorkOrder: `(delegation_id, attempt_no)`, with its own idempotency key. A replacement after a lost session is a new attempt. | Cathulu | counter only (F4) |
| **Provider session** | `(provider_id, external_session_id)`, e.g. `("codex", "<thread id>")`. May be replaced under one logical role. | Cathulu **records**; provider owns the content | overwritten (F3) |
| **Provider turn** | `(session, external_turn_id)`; the unit that streams events. | Provider; recorded by Cathulu | only `last_turn_id` (F3) |

Required: external ids are stored namespaced (`provider_id` + opaque string, bounded, never parsed by policy code); a session row is **appended** (status `active|replaced|lost|unknown`), not overwritten; the logical-role↔session link is a relation with `started_at/ended_at`, so "which session ran this attempt" is answerable after replacement. A turn belongs to exactly one attempt. Provider names, thread vocabulary and model strings never become columns of canonical task/report tables (they appear only in session/turn/attribution records).

---

## 3. Provider boundary (smallest seam two adapters and one router can share)

Operations and their *allowed* result shapes. Anything a provider cannot do returns `Unsupported{operation}`, **never a silent no-op or a fake success**. Names are proposals; fields are limited to what the existing dispatcher already passes.

```text
capabilities()          -> ProviderCapabilities { provider_id, adapter_version, harness_version: Option<String>,
                              resume_session, interrupt, status_query, structured_result, per_turn_model,
                              permission_modes: [..], max_prompt_bytes, notes: [..] }   // each bool is Supported | Unsupported | Unverified
open_session(role, workspace, permission_manifest, tool_config)  -> SessionOpened { session: ExternalSession }  | Err(ProviderError)
resume_session(session, tool_config)                            -> Resumed | Err(SessionGone | ProviderError)
submit_turn(session, attempt_key, work_order_text)              -> Submitted { turn: Option<ExternalTurn>, ack: Acked | AckUnknown }
next_event(timeout)                                              -> Option<ProviderEvent>
answer_request(request_id, Decision)                             -> ok | Err     // Decision::Decline is the only decision this plan authorizes automatically
interrupt(turn)                                                  -> Interrupted | Unsupported | Err      // may be Unsupported on either adapter
close_session(session)                                           -> ok
```

`ProviderEvent` is a closed enum — adapters translate vendor messages into it; unknown vendor messages become `Unrecognized{kind, size}` (counted, never executed, never stored verbatim beyond a bounded excerpt):

`TurnStarted{turn}` · `TurnCompleted{turn, outcome: Completed|Failed|Interrupted}` · `ApprovalRequested{request_id, kind}` (payload deliberately dropped, as today F8) · `UsageObserved{...}` (optional) · `CapacityObserved{...}` (§7) · `TransportLost` · `Unrecognized`.

`ProviderError` has a **closed** classification the router and recovery can switch on:
`Unavailable{reason: NotInstalled|NotAuthenticated|AccountRestricted|Capacity{retry_after: Option<..>}|Transport}` (safe to try another provider or queue; `AccountRestricted` = on-hold/billing/org-not-allowed, not fixed by waiting, so never `Queue` for it — amendment from CC-04 evidence §4) · `Rejected{reason}` (request itself invalid; do not retry unchanged) · `Uncertain` (the submit may or may not have taken effect; **must** go to reconcile, never retried) · `Unsupported{operation}` · `Internal`. The existing code collapses these into strings (F1/F5); the seam needs them typed so "exhausted capacity" and "lost acknowledgement" are not treated alike.

Rules the seam must enforce, independent of adapter:
1. `TurnCompleted` never sets task or delegation success. It only makes a result *available*. Success requires the existing path: report submitted → QA/mandatory checks → required review → reconcile (assessment §18, F2).
2. `submit_turn` takes an **attempt key**; adapters must not submit twice for the same key. If the ack is lost the result is `AckUnknown` and the attempt is `needs_reconcile` (F5).
3. Tool/permission configuration (the capability token env, MCP config) is passed by Cathulu; the adapter may *narrow* it but never widen it, and must pass **only** what Cathulu supplies (no ambient user config/hooks/plugins — see §8 R-ambient).
4. Approval requests: default and only automatic decision is `Decline` + attempt `blocked` (F8). Granting anything requires a Cathulu Owner approval record that is bound to (attempt, request kind, expiry); a provider-side approval id is never accepted from model text.

---

## 4. Typed artifacts and WorkOrder/Result (what crosses the boundary)

Reuse before invent (assessment §13). Mapping:

| New name | Existing source | Change |
| --- | --- | --- |
| `WorkOrder` | `DelegationTaskPacket` + `MainWorkflowBrief` + role + project/task/run ids + packet hash | Add: `attempt_key`, `task_revision` (CC-01 R3), `permission_manifest_hash`, `context_manifest` (§5). Keep `deny_unknown_fields` and `schema_version`. |
| `WorkResult` | `WorkerReport` (+ quality-check refs, artifact ids) | Add: `attempt_key`, `session`/`turn` attribution, `observed_harness_version`, `base_commit`/`head_commit`/dirty fingerprint. Keep ≤16 KiB, one per delegation. |
| `PlanSpec`/`PlanReview` | `ProjectManagerWorkPlan` + `RoleWorkPlanReviewRecord` | Only add fields a router handoff needs (M54). |
| `ProjectCheckpoint` | none yet | CX-02, see CC-01 §4. |

Envelope fields are exactly assessment §13; **null means unknown**. Validation order (cheap to expensive, all before any state change): size cap → schema/`deny_unknown_fields` → parent scope (project/task/run/delegation ownership) → hash recomputation → idempotency lookup → state-transition guard → write+event in one transaction. Provider prose is data: the `summary`/`blockers` strings are never interpreted as instructions to Cathulu or another agent without passing through a reviewer role (matches the operating contract).

A "typed proposal" (plan, routing suggestion, priority suggestion, memory candidate) is a different record kind from an accepted decision. No adapter code can write an accepted record.

---

## 5. New-session context delivery

Problem (F6): "known" is a property of the logical role, recorded at decision time, and the receipt is agent-asserted. A replacement session for `project-manager:7` is told everything is already known, and Cathulu cannot tell whether bodies ever reached any model.

Contract:
- Delivery state is keyed `(session, memory_id, content_hash)`. Logical-role known-by remains as an optimization **hint** used only to choose between `full` and `delta` **within the same session**; it must never suppress delivery into a new session.
- On `open_session`/`resume_session` after replacement, Cathulu computes a **context manifest**: HOT memories for the role's scope plus the WorkOrder's referenced ids, within the existing bounded budget; each entry `(memory_id, content_hash, delivery: full|delta|reference)`. The manifest hash goes into the WorkOrder. Two ways to deliver: (a) inline in the prompt up to a cap; (b) by instructing the role to read via scoped `read_memories`. Both are allowed, but only (a) can be *proved* by Cathulu (it wrote the bytes into the turn). For (b) the receipt stays "agent-reported" and is labeled so.
- Receipt states, distinct and ordered: `selected` (in manifest) → `submitted` (turn ack received) → `agent_reported_read` (activation receipt) . No state means "the model understood it".
- `resume_session` on a provider that cannot guarantee conversation continuity (adapter reports `resume_session: Unverified|Unsupported`) is treated as **new session** for delivery purposes.
- Personal/private scopes are excluded from any manifest unless a disclosure grant for that receiving provider exists (CC-02 P4); the manifest records exclusions by count only.

---

## 6. Bounds (limits on schema and payload, in one place)

Carry the existing limits forward and add only these: provider id ≤64 bytes, external session/turn id ≤256 bytes (as today), `Unrecognized` excerpt ≤512 bytes, event details ≤8 KiB (existing), WorkOrder text ≤64 KiB (existing prompt cap), context manifest ≤64 entries (existing activation cap), `WorkResult` ≤16 KiB (existing), unknown/`Unverified` capability allowed but must be explicit. No adapter may enlarge a limit.

---

## 7. Routing inputs/outputs and availability observations (for CC-05 / M54)

Pure function, no I/O, deterministic.

Inputs (all supplied by Cathulu; policy never reads provider or DB itself):
```text
RoutingRequest { task_shape: Ambiguous | Architectural | Bounded | Administrative | Review{author_provider: Option<ProviderId>},
                 risk: Low | Medium | High,
                 required_review: bool,
                 disclosure: Allowed(providers) | Denied(providers) per data class,
                 owner_overrides: [ProviderPreference | Pin | Exclude],     // above all priors
                 policy_version: u32 }
Availability   { provider_id, state: Available | Unavailable{reason} | Unknown, observed_at, source: Preflight|ErrorSignal|Owner|Probe,
                 retry_after: Option<Instant>, quota: Option<..> /* None = unknown, never 0 */ }
```
Output:
```text
RoutingDecision { chosen: Option<ProviderId>, outcome: Route | Queue{until: Option<..>} | Block{reason} | NoLlm,
                  considered: [ { provider, eligible: bool, exclusions: [Reason] } ],
                  rationale: [Reason], policy_version }
```
Constraints: `Administrative` ⇒ `NoLlm`. Owner pin/exclude and disclosure denial are applied **before** priors. `Review{author}` excludes the author's provider only as a preference; if the preferred reviewer is unavailable, the decision is `Queue`/`Block`, never "waive review" (PLAN-020 M54). `Unknown` availability is not `Available`: the default is to attempt via preflight, not to assume. Unknown quota stays unknown.

**Required review relationship (CC-05, 2026-10-25).** `required_review` applies to non-review work: such work runs only when a second provider could review it, so with exactly one eligible provider it waits on capacity or blocks. The Owner decided on 2026-10-25 that a review whose only reviewer is down blocks (CC-05 §5). A review whose author is unknown routes nowhere (CC-05 R22). A `Review{author}` request always needs a reviewer other than its author. This amends the sentence above: the author is excluded outright, not only as a preference, because a fallback to the author would waive independence. `required_review` does not change the outcome for review requests. Both flag values are covered by fixtures in the scratch crate's `routing_policy` suite (`required_review_*`, `review_requests_never_route_to_the_author_whatever_the_flag`).

Availability observations are evidence with source and time; a stale observation (older than a policy-defined TTL) degrades to `Unknown`. `CapacityObserved` from an adapter is an `ErrorSignal` observation, not a permanent state.

---

## 8. Threat model

Assets: Owner's repositories and data, the SQLite truth, capability tokens, disclosure boundaries. Actors: a compromised/confused model, a malformed or hostile provider stream, a second Cathulu process, the Owner's ambient provider configuration.

| # | Threat | Today | Required control / test |
| --- | --- | --- | --- |
| T1 | **Duplicate submission** (retry after lost ack starts a second turn that edits the tree twice) | `needs_reconcile` is terminal-ish (F4/F5), so duplicate dispatch is unlikely but also unrecoverable | Attempt key on `submit_turn`; `AckUnknown` ⇒ reconcile; resolution is by *observing* the session/turn, never by re-submitting. Test: kill between write and ack → exactly zero or one turn observed, attempt `needs_reconcile`, no second submit. |
| T2 | **Lease loss** (two dispatchers, or dispatcher renewed late) | single lease row, 45/10 s (F11), turns marked uncertain on loss | Every `submit_turn` re-checks lease immediately before write (exists once at `dispatcher.rs:2204`; keep for all adapters). Test: lease stolen mid-preflight ⇒ no submit. |
| T3 | **Stale task revision** (task edited/cancelled after the WorkOrder was issued) | WorkOrder has packet hash only | `task_revision` in the WorkOrder; reconcile rejects results whose revision is no longer current (CC-01 R3), recording the result as unaccepted evidence. |
| T4 | **Approval forgery** (model text or provider event claims "owner approved") | provider requests are declined; Owner approvals are separate rows | Approvals accepted only from the Owner path, bound to (attempt, kind, expiry). A `ProviderEvent` can never create an approval. Test: inject `ApprovalRequested` + a report saying "approved" ⇒ attempt `blocked`, nothing granted. |
| T5 | **Ambient configuration** (user hooks/plugins/MCP servers/memory files of either harness widen tools or leak data) | Codex: existing MCP servers are force-disabled via `--config mcp_servers.<name>.enabled=false` after reading the user config (`app_server.rs:297-383`); other Codex config is not isolated | Adapter contract: report which ambient sources it can neutralize (`capabilities.isolation`); an adapter that cannot neutralize a class must say `Unverified`, and the router must treat that provider as ineligible for write/high-risk work. **Claude side unverified**; CC-04 resolves from official docs and a real install; `--bare`'s subscription implications remain an open P-Q1 question. |
| T6 | **Personal-data disclosure** to a provider | no disclosure concept (CC-02 F5/F6) | Manifest/WorkOrder builder applies disclosure grants before text is assembled; grants are per (category, provider); denial recorded by count. Test (CC-02 A6/A12). |
| T7 | **Session impersonation / id collision** across providers | `thread_id UNIQUE` untyped (F3) | Namespaced `(provider_id, id)` unique pair; event with unknown session ⇒ `Unrecognized`, never attached to a known attempt by id alone. |
| T8 | **Out-of-order / duplicate provider events** | in-memory only (F12) | Persist `(provider_id, session, turn, event_kind, seq?)` identity via the existing `external_event_id` dedupe; handlers idempotent. Test: replay `TurnCompleted` twice and before `TurnStarted` ⇒ one state change, no regression. |
| T9 | **Oversized/malformed stream** | 8 MiB line cap, JSON error closes the transport | Keep; add per-event and per-excerpt caps (§6). Test: truncated JSON, oversized line, binary garbage ⇒ `TransportLost`/`Unrecognized`, attempt `needs_reconcile`, no panic. |
| T10 | **Result forgery** (a report claims checks that did not run, or cites another run's artifacts) | existing: artifact ids must belong to the run, QA refs snapshot, validate_report_scope | Retain; provider adapters add nothing to trust. `verified_success` still requires `finish_run`. |
| T11 | **Shared writable root** (two writers, or a writer plus the Owner's checkout) | F10 | M52 leases; until then the router must not run more than one writer per project and must treat "workspace not exclusive" as ineligible for new writer activation (assessment §17). |
| T12 | **Billing/credential fallback** (adapter silently uses an API key or paid overflow) | n/a for Codex; unknown for Claude | Adapter reports `credential_source` observed at preflight; any source other than the Owner-approved one ⇒ `Unavailable{NotAuthenticated}` for routing; no environment key is forwarded unless named in the permission manifest. |
| T13 | **Cross-project leakage via shared adapter state** (one session reused across projects) | PM threads are per project; Workers per delegation | Session ↔ logical role is 1:1 and the role is project-scoped; adapter refuses `open_session` whose workspace does not match the role's project (CC-02 P2). |

---

## 9. Scenario walk-throughs (success, denied action, lost ack, capacity exhaustion, replacement)

Notation: `A1` = attempt 1 of delegation `D`; `S1/S2` sessions; adapter = Codex or Claude, behaviors identical at the seam.

1. **Success.** Router → `Route(codex)`. `open_session` → S1 recorded `(codex, T-abc)`. WorkOrder built (manifest hash H, `task_revision` 4). `submit_turn(A1)` → `Acked`. Events: `TurnStarted`, agent calls `submit_delegation_report` (existing path), `TurnCompleted(Completed)`. Cathulu: attempt `turn_completed`, **result available**; QA/reviews run; reconcile (revision still 4) accepts → task completed per CC-01 R1. Expect: exactly one report, no state change from `TurnCompleted` alone.
2. **Denied action.** Mid-turn `ApprovalRequested{kind: command}`. Adapter surfaces event, Cathulu answers `Decline`, attempt → `blocked` with the "Owner approval required" reason (F8). A later Owner approval record is bound to (A1, kind, expiry); the turn is not silently resumed; any continuation is a new, explicit action. Expect: no command executed from that request; report, if any, cannot cite approval.
3. **Lost acknowledgement.** Process killed after `submit_turn` bytes written, before ack. On restart: `A1` is `needs_reconcile`. Reconcile procedure: `resume_session(S1)` if supported, read session state (adapter-specific *observation*), and record one of: `turn_found(T)` → attach and continue; `no_turn` → mark `A1 failed(uncertain_resolved)` and permit **A2** with a new attempt key; `unknown` → stays `needs_reconcile` for the Owner. Never re-submit under A1's key.
4. **Exhausted capacity.** `submit_turn` returns `Err(Unavailable{Capacity{retry_after}})` (definitely not started). Attempt `A1 → queued_unavailable`; `Availability(codex) = Unavailable{Capacity}` with source ErrorSignal and `retry_after`; router re-evaluates: other provider eligible and disclosure-allowed ⇒ `Route(claude)` as **A2** on a new session; else `Queue{until}`. Required review is not waived if the reviewer-provider is unavailable. Expect: capacity is never recorded as quota 0; unknown stays unknown.
5. **Replacement session.** S1 is lost (`resume_session` → `SessionGone`). Session row S1 → `lost`; S2 opened under the same logical role; delivery computed from scratch for S2 (§5) even though logical known-by says everything is known; the new attempt `A2` references both sessions in its history. Expect: S1 row and its turns remain readable; S2's manifest includes every HOT item; receipts for S2 start at `selected`.

---

## 10. Checklist for reviewing CX-03's exported seam (F1)

When `src/agent_provider.rs` / `src/execution_artifacts.rs` exist, I will check, and report amendments rather than silently accept:
1. Session/turn ids carry `provider_id`; no `thread_id`-shaped field in canonical types.
2. `ProviderError` has the five classes in §3 and `Uncertain` is distinguishable from `Unavailable`.
3. `submit_turn` takes an attempt key; the double implements `AckUnknown`.
4. `Unsupported` exists as a value, and the double exercises it for `interrupt` and `status`.
5. `TurnCompleted` has no field or helper that sets success.
6. No type embeds a vendor concept (thread, app-server, conversation) except inside adapter modules.
7. Deterministic double covers: success, malformed event, auth failure, capacity, denial, lost ack, unknown session, duplicate/out-of-order events.
8. Payload examples validate against the types and respect §6 limits.
9. The routing input type has `Unknown` availability and no way to express quota 0 for unknown.
10. Public symbol names and module locations are final enough for the CC-04 scratch wrapper (`pub use cathulu_core::agent_provider;`) to compile unchanged.

## 11. Consistency with PLAN-020 and conflicts with source

Consistent with M51 acceptance (Codex details confined to the adapter; replacement sessions get selected context; duplicate events cannot mutate state twice; provider completion never sets success), M52 (attempt/lease semantics), and M54 (policy before priors, review never silently waived).

Conflicts / gaps against **source** (not PLAN-020): F3 (session overwrite: needs append-only sessions), F4 (no attempt identity), F5 (nothing can resolve `needs_reconcile`), F6 (delivery keyed on the logical role), F12 (provider event identity not persisted).

Not traced: `src/provider_adapters.rs` (existing "specialist provider adapters" per PLAN-020 are API integrations, not agent providers; I did not read them), `telemetry.rs`/hook ingestion for Codex, the inbox/`route_pending_role_events` paths beyond their dispatch use, `owner_approval_*` internals. Open for the Owner: whether the Claude adapter may be considered for write work before ambient-config isolation is proven (P-Q1); whether a lost-ack `no_turn` resolution is allowed to create A2 without Owner confirmation.

## 12. CX-03 pre-review of the working-tree seam (2026-10-25; uncommitted, not a commit review)

Checked against the §10 checklist on a snapshot of the uncommitted tree (`target/manual-dev/claude/precheck-cx03`):

| §10 item | Status | Note |
| --- | --- | --- |
| 1 session/turn ids carry `provider_id` | pass | `ExternalSession { provider_id, session_id }`; `ExternalTurn` wraps the session |
| 2 five error classes, `Uncertain` distinct | pass | `Unavailable`, `Rejected`, `Uncertain`, `Unsupported`, `Internal`; `SessionGone` added for §9.5 (accepted) |
| 3 submit takes an attempt key; `AckUnknown` | pass | `SubmitTurnRequest.attempt_key`; `AckState::AckUnknown` |
| 4 `Unsupported` exists and the double uses it | pass | double returns `Unsupported` for interrupt and session status |
| 5 no success path from `TurnCompleted` | pass | `TurnCompleted` carries `TurnOutcome` only; no success helper found |
| 6 no vendor concepts outside adapters | pass | only `ProviderId::codex()/claude()` (identity) and a comment about the dispatcher thread |
| 7 deterministic double covers the failure matrix | mostly | `tests/agent_provider_contract.rs` covers lost ack, out-of-order sequence, auth, capacity, malformed and oversized input, approval decline, envelope tampering. Duplicate-event identity is not clearly covered. |
| 8 payload examples validate against the types | pass | `published_payloads_validate_against_the_actual_exported_types` passes |
| 9 routing input has `Unknown` and cannot express quota 0 for unknown | **fails** | `permission_scope_and_unknown_availability_cannot_silently_widen` fails: `{"kind":"unknown","quota":{"remaining":0}}` deserializes. `AvailabilityState` uses an internally tagged enum with `deny_unknown_fields` on the enum; serde does not reliably apply that to fields of tagged variants, so the unit variant `Unknown` silently drops extra fields. Fix: reject unknown fields explicitly, e.g. a field-less deserialization helper for `Unknown`, or a validated `try_from`. |
| 10 public names compile for an external consumer | pass | `examples/provider_contract_payloads.rs` builds and runs from outside the module tree |

Other pre-review observations (design, not blocking the seam):

- **Disclosure granularity.** `AvailabilityObservation` carries a single `disclosure_allowed` bool. CC-02 P4 requires disclosure grants per data category and per provider. A bool cannot express "identity yes, finances no". Before CX-08 relies on it, disclosure should be a per-category grant checked against the WorkOrder's data classes.
- **Routing reasons.** The reason set matches CC-05 §3, plus `OwnerPreference`, `StaleObservation`, `Unavailable` and `Reroute`. Acceptable; CC-05 should be updated to match.

Result: the seam is not ready for the §10 review until item 9 passes and the duplicate-event case is covered. Formal review waits for a stable commit.

## 13. Formal CC-03 review of the CX-03 seam, commit `09f192b` (2026-10-25)

**Verdict: accept the seam as the M51a common contract.** This releases F1 for adapter and policy implementation, as the CX-03 packet requires.

Evidence:
- The committed source is identical to the working-tree snapshot I tested before the commit.
- Clean export `target/manual-dev/claude/committed-09f192b`, full core suite with `--locked --offline --no-fail-fast`: **114 passed, 0 failed** (29 lib, 7 contract, 3 checkpoints, 8, 14, 17, 4, 4, 5, 6, 2, 4, 15 across binaries).
- §10 checklist on the committed code: items 1–10 pass. Item 9 was fixed with strict empty record variants. Item 7 now includes event identity: `external_event_key` is tested for replay and collision.
- `McpToolConfig` derives only `Clone` (no `Debug`, no `Serialize`), so capability tokens in its environment cannot be logged or serialized.
- Retry hints are rejected outside capacity errors.
- The `src/delegation.rs` change only widens visibility to `pub(crate)`. There is no behaviour change.
- A Claude scratch crate compiles against the exports from outside the crate.

Conditions and limits carried forward (none blocks the seam):
1. **Unidentified events have no dedupe key.** `external_event_key` returns `None` when neither `event_id` nor `sequence` is present. M51b must require an identity for any event that mutates state, or treat unidentified events as non-mutating.
2. **`all_required_disclosures_granted` is caller-supplied.** Codex documents that no personal-data producer may set it true before the M50 grant chain exists. Per-category disclosure is still required before personal-category routing (CC-02 P4).
3. **Routing reasons** now include `DisclosureDenied {}` and the rest. CC-05 §3 must be updated to match the implemented set.
4. **Replacement-session delivery and receipts** are CX-04/M51b work, as the packet says. This review accepts the types and fixtures only, not persistence or live delivery.

Next: CC-04 (Claude adapter) can now implement against these types in the scratch crate. Its live activation still needs the P-Q1 isolation decision, M51b, M52 and the Owner's approval.

## 14. Claude review of the CX-04 / M51b persistence checkpoint, commit `fb502de` (2026-10-25)

**Verdict: accept as a non-activating persistence foundation. M51b acceptance stays open, and the amendment below is required before any live activation (M52 or later).**

Evidence:
- Clean export of `fb502de`, full core suite `--locked --offline --no-fail-fast`: **126 passed, 0 failed**, matching Codex's report.
- Live dispatch is unchanged: `dispatcher.rs` still calls `AppServerProcess::start` three times and has no `CodexAdapter` reference. The `dispatcher.rs` change clears provider-attempt links on capability rotation only.
- My modules `src/claude_code.rs` and `src/routing_policy.rs` are not registered in `lib.rs`.
- Schema v21 was read in full. Provider sessions are keyed by `(provider_id, session_id)`, so IDs are namespaced. Attempts are checked against their role session and queued record. Turn identity is unique per `(provider, session, turn)`. Attempt state transitions are restricted, and identity fields are immutable. Context receipts move only `selected → submitted → agent_reported_read`. Provider sessions, attempts and receipts cannot be deleted.
- Attempts are persisted as `created` before the provider submit call (`provider_sessions.rs` ~295), so a crash before acknowledgement leaves a recoverable row.

**Required amendment (before M52 or any live activation): one active session per logical role across providers.** The unique active index is `(provider_id, binding_key)`, and the replacement code deactivates only sessions with the same provider (`provider_sessions.rs` ~117). A Claude session and a Codex session can therefore both be active for one role at once. That allows duplicate turns across providers, which is threat T1 and T13 in §8. Enforce one active session per `binding_key` in the index and in the replacement code, and add a test that a cross-provider bind deactivates the previous session.

**Naming debt (not blocking):** Claude sessions bind through `role_thread_bindings`, whose name and `thread_id` shape are Codex-specific. This works, but M52 or M54 should rename it or document the mapping.

**Adapter compatibility:** my adapter now uses the Cathulu attempt key as its turn ID, which is unique per attempt. That satisfies `provider_attempts_turn_identity_idx` for resumed Claude sessions. My earlier session-ID turn ID would have collided, and that is fixed.

Open for Codex: M51b acceptance needs the dispatcher wired through `CodexAdapter` under an acknowledged pause and M52's workspace and credential gates. Claude does not treat this checkpoint as M51b complete.

## 15. Claude review of the CX-05 / M52 lease checkpoint, commit `a83ccf9` (2026-10-25)

**Verdict: conditional accept as a non-activating lease foundation.** One required test is missing (below). M52 acceptance stays open.

Checked:
- Clean export, full core suite `--locked --offline --no-fail-fast`: **132 passed, 0 failed**.
- Live dispatch unchanged: `dispatcher.rs` still has 3 direct `AppServerProcess::start` calls and no `CodexAdapter` reference. ADR-0024 says runtime activation remains gated, which matches.
- **Amendment from §14 is in code.** The active index is now `UNIQUE(binding_key) WHERE status='active'`, so one active session per logical role across providers. The replacement update deactivates sessions with the same binding key but a different provider or session (`provider_sessions.rs` ~135–139). The v22 migration refuses to upgrade a database that already has duplicate active bindings.
- v22 lease schema: at most one active or reconciling lease per workspace path, at most one per role, at most one writer per project, and coordinators are restricted to read-only.

**Missing (required before M52 acceptance):** no test covers the cross-provider bind. The §14 amendment asked for one that binds a Claude session and then a Codex session to the same binding, and checks that the first is deactivated. The two session tests in the module cover migration collisions and context redelivery only. Add the test.

**Not audited:** the lease APIs in `src/worktree.rs` (about 870 lines) and the recovery paths. I checked the schema invariants and the suite, not every function. Claude will review them with the M52 acceptance pass.

## 16. Claude read-only review of `ac3a3b2` and the M52 lease API audit (2026-10-25)

**Verdict: accept the cross-provider replacement test, and accept the lease checks audited below. Terminal resolution is not yet audited. M52 stays open.**

Commit review (`ac3a3b2`, the test; `d0e6aed` is docs only):
- Clean export, full core suite: **132 passed, 0 failed**.
- `tests/workspace_leases.rs::workspace_lease_recovery_quarantines_attempt_before_cross_provider_replacement` checks the amendment in full. While the Claude attempt is unresolved, a Codex session is refused. After reconciliation, exactly one session is active, the Claude session is `inactive` and kept in history, Codex is `active`, and the completed attempt still names its original Claude session. This closes the gap in §14 and §15.

Lease API audit (`src/worktree.rs`, committed `ac3a3b2`):
- **Claim (`verify_project_workspace`, ~1027):** the path must be absolute and canonical, with no alias or symlink. The registered root can never receive a writer. A linked worktree must appear in the registered repository's worktree list and share its Git common directory. Writers must be clean. The base commit must exist. These checks look sound.
- **Use (`validate_provider_workspace`, ~435):** every provider use re-checks status, expiry, the exact path, the access level, the credential source, and the path and common-directory hashes. A read-only lease cannot grant write access. A clean-tree check is deliberately skipped at use time, because the agent changes the worktree during its turn. That is correct.
- **Expiry (`expire_stale_workspace_leases`, ~611):** an expired lease with no unresolved attempt becomes `expired`. One with an unresolved attempt becomes `needs_reconcile`, and its attempts become `needs_reconcile` too. The path and role locks stay held, and capabilities are revoked. Nothing is released early, and the attempt state transitions are allowed by the v22 trigger.

Observations, not defects:
- **Lease TTL versus long turns.** A turn that outlives its lease forces reconciliation even if the process is still working. That fails closed, which is the right direction. Renewal is available (`renew_workspace_lease`), but nothing in the code drives it automatically. The dispatcher admission boundary (CX-05 follow-up) should schedule renewal for active turns.

**Not yet audited:** `resolve_workspace_lease` (~791) and `revoke_workspace_lease` (~693), which end a lease after reconciliation. Also the claim transaction's race handling, which relies on the database unique indexes. The schema guarantees one writer per project, but I have not traced every caller. Claude will finish these in the M52 acceptance pass.

## 17. Claude formal review of gate commit `650ad6b` and completion of the M52 lease API audit (2026-10-25)

**Verdict: accept `650ad6b` as a fail-closed, non-activating gate. Four amendments are required before M52 admission. The §16 open items are now audited. M52 stays open.**

Recorded under Codex's line-103 resume, which allows the result in this allocated note. The same verdict is on the Board at `docs/parallel/BOARD.md` line 77.

Evidence:
- Clean export `target/manual-dev/claude/committed-650ad6b`, build dir `build-650ad6b`: full core suite **133 passed, 0 failed**. That covers 16 test targets (lib, 3 binaries, 12 integration) plus doc-tests (0), exit 0. Log: `target/manual-dev/claude/650ad6b-full-retry.log`. Codex's run reports the same 133/133 with both dispatch flags disabled.
- Commit scope: `git show --stat 650ad6b` changes `src/mcp.rs` (53 lines) and docs. No other source file changed.

Gate, checked in committed source:
- `ROLE_DISPATCHER_RUNTIME_GATES_VALIDATED = false` (`mcp.rs:33`). `role_dispatcher_runtime_enabled` and `role_dispatcher_service_enabled` AND this constant (`mcp.rs:2195–2200`), so both are false whatever the environment says.
- Queue tool: `dispatch_prepared_delegations` returns an error before `enqueue_role_dispatches` when `dispatcher_enabled` is false (`mcp.rs:1107–1116`). `enqueue_role_dispatches` has no other caller.
- Service: `RoleDispatcherService::start` is called once, at `mcp.rs:2175`, behind `role_dispatcher_service_enabled`. `dispatcher_loop` is spawned once (`dispatcher.rs:2062`). `dispatch_one` (called at `dispatcher.rs:2156`), `dispatch_role_inbox_message` (`2173`) and `dispatch_main_inbox_message` (via `2397`) are reached only from that loop. The three direct `AppServerProcess::start` sites (`dispatcher.rs:2217, 2425, 2519`) are therefore unreachable while the constant is false.
- Tripwire: `m52_admission_gate_keeps_product_dispatch_off_even_when_requested` (`mcp.rs:2250–2257`) fails as soon as the constant is flipped, so the flip cannot happen silently. It does not test admission.

Required before M52 admission (not blocking `650ad6b`):
1. **Fail-open default once the constant flips.** `CATHULU_ROLE_DISPATCHER_ENABLED` unset makes `dispatcher_requested` true (`mcp.rs:2163–2168`). Both predicates read it, so the default would start the dispatcher service on every non-role MCP server, and enable the queue tool. Admission must default to off, or replace the env opt-out with an explicit admission record. Test the admission record, not the constant.
2. **Public spawn primitive.** `CodexAdapter::start()` (`src/codex_adapter.rs:48`) calls `AppServerProcess::start()`, and `codex_adapter` is `pub mod` (`src/lib.rs:4`). No product caller exists at `650ad6b`. The gate sits only at the dispatcher, so a library consumer can still spawn the provider. Gate the spawn or the adapter constructor as well.
3. **Lease binding is not enforced.** `provider_attempts` and `role_thread_capabilities` accept a NULL `workspace_lease_id` (`migrations.rs:1867–1886`, `1889–1898`; the triggers check only non-NULL values). `authorize_role_tool_call` skips the lease check when it is NULL (`dispatcher.rs:880`). `revoke_capability_for_lease` (`worktree.rs:1012–1025`) matches only capabilities that carry a lease ID, so a lease transition never revokes an unbound capability. `bind_role_thread_capability_to_workspace_lease` and `validate_provider_workspace` have no product caller. Admission must require a bound, active lease and a passing `validate_provider_workspace` before any provider start. It must refuse unbound worker and sub-overseer capabilities and attempts.
4. **Mismatch handling differs.** `validate_provider_workspace` returns an error on an identity mismatch and leaves the lease active (`worktree.rs:464–468`). `renew_workspace_lease` quarantines the same case (`worktree.rs:506`). Make `validate_provider_workspace` quarantine as well, so a mismatch stops every later use.

Lease API audit (closes the §16 "not yet audited" items):
- **`resolve_workspace_lease` (`worktree.rs:791–856`): sound.** It requires the owner and `needs_reconcile`, and it refuses while any linked attempt is `created`, `submitted`, `ack_unknown`, `running` or `needs_reconcile`. The v22 state guard (`migrations.rs:1656–1664`) makes those five the complete non-terminal set, because `completed`, `failed` and `blocked` are terminal. It moves the lease to `released`, which the trigger allows only from `needs_reconcile`. Capabilities were already revoked when the lease entered quarantine (`worktree.rs:662, 761, 898`). `bind_…` refuses non-active leases (`dispatcher.rs:735–738`), so a released lease cannot be rebound. Exercised by the recovery test at `tests/workspace_leases.rs:495, 525`.
- **`release_workspace_lease` and `revoke_workspace_lease` (`worktree.rs:681–704`) through `end_workspace_lease` (`worktree.rs:705–787`): sound.** Open attempts move the lease to `needs_reconcile`, so the lock stays. Otherwise the lease becomes `released` or `revoked`. Capabilities are revoked in every branch. `release` is exercised at `tests/workspace_leases.rs:286, 485, 665`. `revoke_workspace_lease` has no test. **Non-blocking:** needs_reconcile now has two exits, `resolve` (event `reconciled`) and release or revoke (events `released` and `revoked`, allowed only when no attempts are open). Both are equally safe. Choose one canonical exit before admission so the audit trail has one shape.
- **`expire_stale_workspace_leases` (`worktree.rs:611–677`): sound.** Open attempts keep the lock as needs_reconcile. Otherwise the lease expires. Capabilities are revoked either way. Exercised at `tests/workspace_leases.rs:467`.
- **`renew_workspace_lease` (`worktree.rs:477–552`): logic sound, not exercised.** It checks owner and status, sweeps an expired lease and refuses, quarantines on an identity mismatch, and extends the expiry in one guarded `UPDATE`. It has no runtime caller and no test.
- **Claim race (`claim_delegation_workspace`, `worktree.rs:306–434`): sound by construction.** The expiry sweep runs first. The claim then opens `BEGIN IMMEDIATE` with a 5 s busy timeout (`src/sqlite.rs:15, 27`) and refuses while an earlier attempt for the delegation needs reconciliation (`worktree.rs:337–348`). The INSERT (`worktree.rs:382`) is plain, with no `OR REPLACE` or `ON CONFLICT`. The partial unique indexes include `needs_reconcile` (`migrations.rs:1810–1818`), so a second claimant fails with a constraint error. Identity is verified before the transaction (`worktree.rs:324`), so the claim is safe only once use-time validation is wired (item 3). No concurrent test exists; `tests/workspace_leases.rs` has no thread or barrier use.
- **Expiry trigger:** expiry runs only inside claim and renew. No background sweeper exists. `authorize_role_tool_call` (`dispatcher.rs:888`) and `validate_provider_workspace` (`worktree.rs:447`) refuse expired leases by timestamp. This fails closed and affects recovery latency only. Non-blocking.

Gaps confirmed from Codex's `36eda23` list: no concurrent-claim test, no revoke test, no renewal caller, and `PermissionManifest` is not wired into dispatch or MCP.

Status: `650ad6b` is accepted as a fail-closed hold. The admission commit is reviewed below. The CC-04 §8 login note (no secrets) waits for the resume after the next pause.

### Admission commit `d851035` (2026-10-25)

**Verdict: accept as a non-activating, fail-closed admission boundary that meets the four items above. The runtime gate stays false. Amendments A–D are required before the next M52 packet closes. Enabling dispatch is the Owner's decision, taken after A–D and live evidence.**

Evidence:
- Clean export `target/manual-dev/claude/committed-d851035`, build dir `build-d851035`, both dispatch flags `0`: full core suite **138 passed, 0 failed**, exit 0. Log: `target/manual-dev/claude/d851035-full.log`. Per target: lib 45, 12 integration targets, 3 binaries and doc-tests 0.
- Scope: `d851035` changes the M52 source and tests. HEAD `3290cc5` changes docs only (ADR-0024, PLAN-021, the CX-05 handoff, BOARD), so the source reviewed is `d851035`.
- Gate: `ROLE_DISPATCHER_RUNTIME_GATES_VALIDATED = false` (`src/mcp.rs:35`).

Checked against the four items:
1. **Process-start gate.** `AppServerProcess::start(&ProviderStartAdmission)` (`src/app_server.rs:62–63`) and `CodexAdapter::start(ProviderStartAdmission)` (`src/codex_adapter.rs:50`) call `ensure_valid`. A default admission is refused (`app_server.rs:539–545`). The grant is private, and its constructor is `pub(crate)` (`src/agent_provider.rs:248–264`). Thread start checks the admitted root (`app_server.rs:132`). Resume and turn start check expiry (`app_server.rs:152, 190`).
2. **Lease binding.** `dispatch_one` resolves the exact active lease (`src/dispatcher.rs:2578`), runs the preflight (`2585`), issues a capability bound to that lease (`2610`), validates (`2619`), then starts (`2629`). It revalidates before resume, thread start and turn (`2641, 2662, 2746`). The Project Manager inbox path does the same (`2881–2963`). Unbound worker and Project Manager capabilities are refused in `authorize_role_tool_call` (`dispatcher.rs:1122–1128`) and in `validate_role_provider_start` (`dispatcher.rs:849–851`; test `tests/workspace_leases.rs:651–664`). Attempt creation requires a live lease with the same project, delegation and binding, and a capability bound to it (`src/provider_sessions.rs:343–389`).
3. **Quarantine.** A verification failure marks the lease `needs_reconcile` and revokes its capability (`src/worktree.rs:527–534`; test `tests/workspace_leases.rs:697–764`).
4. **Main hold.** `dispatch_role_inbox_message` refuses the Main target before any lease lookup or start (`src/dispatcher.rs:2832–2838`).

Amendments (required):
- **A. Dispatch-level provider-free tests.** No test calls `dispatch_one` (`dispatcher.rs:2535`) or `dispatch_role_inbox_message` (`dispatcher.rs:2825`) at `d851035`. Add tests that drive both with: no active lease; an unbound or mismatched capability; an unknown or unpermitted credential source; network denied; and the Main target. Each asserts a blocked outcome and that the app-server option stays `None`. This is the proof the CX-05 handoff requires.
- **B. Direct unbound-authorize test.** The refusal at `dispatcher.rs:1122–1128` has no test. The existing unbound test covers `validate_role_provider_start`, not `authorize_role_tool_call`.
- **C. Gate comment.** The comment at `src/mcp.rs:31–35` says the Owner holds dispatch "until Claude reviews". Review is a prerequisite, and the Owner decides any enablement. Reword it.
- **D. Audit reason.** `src/worktree.rs:527–534` uses one reason, "could not be verified", for verification errors and identity mismatches. Split them so a real identity change is recorded as such.

Notes (not blocking):
- **E.** Verification errors quarantine by design, as the §17 test name pins. Transient Git or I/O failures therefore need explicit reconciliation. Record this in ADR-0024. The Owner may later split transient errors from identity changes.
- **F.** Orphan capability: `issue_role_thread_capability_for_workspace` runs before the second validation (`dispatcher.rs:2610` before `2619`; Project Manager path `2912` before `2921`). A rejection after issue leaves an active row whose token is never returned. That is harmless, but validating before issuing, or revoking on failure, is cleaner.
- **G.** Trust boundary: `validate_provider_workspace` (`worktree.rs:489`) and `validate_role_provider_start` (`dispatcher.rs:840`) are `pub`. An in-process caller holding a live bound token can mint an admission from its own manifest. That is acceptable while the dispatcher is the only production caller. Record it in ADR-0024.
- **H.** `renew_workspace_lease` still has no caller, so long turns expire into reconciliation. Scheduled renewal stays open.

Known gate for live use (not a defect):
- **I.** Both adapters declare `CredentialSource::Unknown` (`src/codex_adapter.rs:546`, `src/claude_code.rs:159`), so admission refuses every provider start. That is the fail-closed state and a prerequisite for any live acceptance.

Status: `d851035` is accepted as a non-activating boundary. Its amendments are reviewed below.

### Amendment commit `3444829` (2026-10-25)

**Verdict: amendments A–D are implemented and meet the review. One small follow-up, R1, is required before the next M52 packet closes. The runtime gate stays false.**

Isolated check (run under the handoff's resume for `3444829`): clean export `committed-3444829`, build `build-3444829`, both dispatch flags `0`. Full core suite **142 passed, 0 failed**, exit 0. Per target: lib 48, 12 integration targets, 3 binaries and doc-tests 0. Log: `target/manual-dev/claude/3444829-full.log`. `85c060f` is docs-only, so no source or test changed after `3444829`. R1 is not yet in code.

Checked:
- **A. Dispatch-level provider-free tests.** `both_dispatch_routes_block_missing_lease_and_unbound_capability_before_spawn` (`src/dispatcher.rs:3616`), `both_dispatch_routes_block_unknown_unpermitted_credentials_and_denied_network_before_spawn` (`3689`), and `main_inbox_is_blocked_before_any_store_or_provider_access` (`3733`). Each calls both routes with `app_server` as `None`. `assert_refused_without_spawn` (`3600–3612`) checks status, reason, and that no app-server exists.
- **B. Direct unbound-authorize test.** `tests/role_capabilities.rs` (around line 446) issues an unbound worker capability and asserts that `authorize_role_tool_call` refuses it with "require a bound workspace lease".
- **C. Gate comment.** `src/mcp.rs:31–35` now says review is a prerequisite and that the Owner approves enablement. Accepted.
- **D. Distinct quarantine reasons.** A verification failure gives "workspace verification failed during provider admission" (`src/worktree.rs:527–530`). An identity change gives "workspace or Git identity changed during provider admission" (`src/worktree.rs:534–544`). `tests/workspace_leases.rs:767` asserts both reasons and the `needs_reconcile` status.
- **Extra change 1: binding check before preflight.** `dispatch_one` checks the existing-thread lease binding (`src/dispatcher.rs:2585–2596`) before the manifest and preflight (`2597–2602`). The order is correct, with no regression.
- **Extra change 2: revoke after a rejected admission.** Revocation covers the first validation failure (`src/dispatcher.rs:2631`; PM `2937`) and the resume, thread and tools failures (`2663`, `2694`, `2716`; PM `2964`, `2970`). R1 covers the paths that still do not revoke.

**R1 (required, small).** Not every refusal after issue revokes the capability. These paths return without revoking, so the capability stays active. Once a thread exists, its token sits in that thread's MCP config.
- App-server spawn (`src/dispatcher.rs:2637`; PM `2943`) and MCP config build (`2643`; PM `2949`).
- The second validation and `set_admission` on the resume and new-thread paths (`2649–2656`, `2670–2680`; PM `2952–2962`).
- The turn-level validation and `set_admission` before `start_turn` (`2754–2764`; PM `2975–2985`).

In `dispatch_one`, the turn-level validation runs after `mark_delegation_dispatched` (`2747`). A refused turn therefore leaves the delegation marked dispatched with no turn. The fix is to validate before the dispatched mark, and to wrap the post-issue path so that any refusal before `start_turn` revokes the capability, through one helper. Failures in `start_turn` itself should keep the capability, because the turn may already have started.

Notes:
- The docs-only changes in the commit (ADR-0024, PLAN-020, PLAN-021, the CX-05 handoff) were not reviewed beyond the gate description and the Board text.
- Follow-ups E–H from the `d851035` review still stand.

Status: `3444829` is accepted, with R1 pending with Codex, and R1 is now implemented at `6153cdc` below.

### Commit `6153cdc` (R1 fix) (2026-10-25)

**Verdict: accept R1 as implemented. Blocker B1 must be resolved before dispatch is enabled or live recovery is claimed. Decision D1 needs the Owner and Codex.**

Evidence:
- Clean export `target/manual-dev/claude/committed-6153cdc`, build dir `build-6153cdc`, both dispatch flags `0`: full core suite **143 passed, 0 failed**, exit 0. Log: `target/manual-dev/claude/6153cdc-full.log`. Per target: lib 49, 12 integration targets (94 tests), 3 binaries and doc-tests 0. An earlier background run was stopped by a session restart; this is the re-run.
- Scope: `6153cdc` changes only `src/dispatcher.rs`. HEAD `a5193a6` changes docs only (ADR-0024, BOARD, the CX-05 handoff, PLAN-020, PLAN-021), so the reviewed source is `6153cdc`.

**R1: met.**
- One RAII guard, `CapabilityRevocationGuard` (`src/dispatcher.rs:2415`), is created right after issuance. It revokes the binding's capability on drop, so every return before submission revokes it: validation, spawn, config, resume, `set_admission`, the turn-level validation and the lease-renewal check. It is disarmed only by `preserve_for_turn_submission()`, called right before `start_turn` (`dispatch_one` at `2784`; Project Manager path at `3002`).
- The dispatched mark and `turn_starting` now run after validation.
- An error from `start_turn` keeps the capability. That is correct, because the provider may already have accepted the turn.
- The test `capability_guard_revokes_before_submission_and_preserves_after_submission` (`src/dispatcher.rs:3633`) covers the guard. The dispatch-level tests now also assert that pre-issuance refusals do not rotate the capability.

**B1: blocker before enabling dispatch or claiming live recovery.**
- Product code never creates provider-attempt records. `begin_provider_attempt*` is called only from tests (`src/provider_sessions.rs:1358` onward). The only dispatcher references to `provider_attempts` are reads (`src/dispatcher.rs:753, 917`).
- Lease expiry therefore counts zero open attempts and moves the lease to `expired` (`src/worktree.rs:720–721`). That releases the path, role and writer locks while a provider may still be writing.
- The expiry sweep runs only inside claim (`src/worktree.rs:370`). Renewal has no caller (`src/worktree.rs:555`). A turn that outlives its TTL can therefore be overlapped by a new claim on the same path while its provider still writes.
- Cathulu's tool gate does not stop the provider's own file writes, so the lock is the only protection, and it has been released.
- Required before enabling dispatch or claiming live recovery: create a provider attempt bound to the lease at turn start; expire a lease with an open attempt into `needs_reconcile` (already the code path); and renew while a turn is live.

**D1: decision for the Owner and Codex.**
- A benign refusal after the thread is bound, such as a lost dispatcher lease, now revokes the thread's capability. `active_role_thread_capability_workspace_lease_id` returns None for revoked rows (`src/dispatcher.rs:822`). The next dispatch to that thread then stops at the existing-thread check with `needs_reconcile`, until reconciliation. That is fail-closed, but it changes availability. Decide whether reconciliation is the documented rule, or whether the check should use the thread's bound lease instead.

Non-blocking: add one dispatch-level test that forces a post-issue refusal and asserts revocation. The current test covers only the guard.

Status: `6153cdc` is accepted for R1, and its B1 and D1 are reviewed below.

### Commit `67266ab` (B1 and D1 fixes) (2026-10-25)

**Verdict: accept B1 and D1 as implemented in code. One new blocker for live use: no operator path can resolve a quarantined attempt or lease. This was a read-only review under Codex's resume, so Claude ran no builds.**

Checked (read-only, against `67266ab`):
- **B1, attempts exist before submission.** `persist_codex_provider_attempt` (`src/dispatcher.rs:2503`) records the session, creates the attempt under the active lease, and binds the capability to it. `ProviderAttemptSubmissionGuard` (`src/dispatcher.rs:2471`) marks the attempt `blocked` if the turn is refused before submission. The attempt is marked `submitted` only after `start_turn` succeeds, and then it follows the turn's events (`update_active_provider_attempt`, `src/dispatcher.rs:3518`).
- **B1, renewal and reconciliation.** Each active turn's lease is renewed every 10 seconds to a 60-second horizon (`src/dispatcher.rs:31–32`, heartbeat block at `2625`, `renew_active_workspace_leases` at `2534`). A renewal failure, or a transport close (`src/dispatcher.rs:3494–3504`), marks the turn and its inbox item for reconciliation and clears the active turns. The lease is then not renewed and expires. Expiry counts `needs_reconcile` attempts as open (`src/worktree.rs:715`), so the lease moves to `needs_reconcile` and keeps its lock. A dispatcher crash follows the same path.
- **D1, safe reuse.** The existing-thread check now uses the persisted lease binding, even when the bearer capability is revoked (`src/dispatcher.rs:2790`; helper at `846`). A thread is reused only when that binding equals the current exact active lease, and a fresh capability is issued. Rotation is refused while an attempt needs reconciliation (`src/dispatcher.rs:767`). The test `revoked_capability_keeps_its_thread_lease_binding_for_safe_reuse` (`src/dispatcher.rs:3993`) covers the reuse rule. The D1 decision from `6153cdc` is resolved this way.
- **Tests.** `provider_attempt_is_durable_and_bound_before_submission` (`src/dispatcher.rs:4052`) and `provider_attempt_guard_closes_pre_submit_refusals` (`src/dispatcher.rs:4136`) cover the attempt lifecycle. Expiry with an open attempt is covered by the recovery test in `tests/workspace_leases.rs`.

**New blocker for live use: no operator recovery path.**
- Nothing outside the dispatcher can resolve a `needs_reconcile` attempt or call `resolve_workspace_lease`. `set_provider_attempt_state` and `resolve_workspace_lease` have no caller in `src/mcp.rs` or `src/bin`, and there is no MCP tool or CLI for either.
- So after any quarantine, the path lock stays held until someone changes the database by hand. That is fail-closed, but it is not a recovery path.
- Required before enabling dispatch or claiming live recovery: an explicit operator action, through MCP or CLI, that resolves each open attempt to a terminal state with a reason, and then releases the lease through `resolve_workspace_lease`. The Owner should authorize that action.

Non-blocking notes:
- A renewal failure for one lease clears every live turn's bookkeeping and drops the app-server (`src/dispatcher.rs:2625–2644`). That is fail-closed. Document it, or scope the reconciliation to the affected turn.
- A lease lingers up to 60 seconds after its turn completes, because it is not released on completion.
- A dispatcher stall longer than 60 seconds makes the next renewal fail. That quarantines live turns, which is acceptable, but it should be noted.
- The new tests do not cover the renewal-failure or transport-closed quarantine paths. Add them before live use.
- Revoke and claim-race coverage, which Codex lists as remaining M52 work, is still open.

Status: `67266ab` is accepted for B1 and D1 in code. Its operator recovery path is reviewed below.

### Commit `c4f0fd8` (manual workspace lease recovery) (2026-10-25)

**Verdict: accept the recovery transaction and its exact-set, replay and role-denial behaviour. Three amendments are required before live use: R-A, R-B and R-C. This was a read-only review under the handoff's resume, so Claude ran no builds.**

Checklist from the resume, read against `c4f0fd8`:
- **Path redaction: met.** `list_workspace_recovery_cases` returns the lease record (hashes, never the canonical path) and attempt keys with status, provider and turn ID. It returns no request hash or prompt (`src/worktree.rs:963–1033`). A test asserts the serialized case does not contain the worktree path (`tests/workspace_leases.rs`, recovery test).
- **Exact-set validation: met.** The open attempts must equal the requested set, "every and only unresolved attempt for this lease" (`src/worktree.rs` ~1197–1206). Empty and partial sets are refused. The test covers an empty set.
- **Provider-stopped confirmation: met as a gate, not as evidence.** The call refuses unless `provider_stopped_confirmed` is true (`src/worktree.rs:1046`). The value is the caller's own assertion. See R-A.
- **Linked-worktree identity revalidation: met, with a timing gap.** The identity is verified against the lease's hashes before the transaction (`src/worktree.rs` ~1147–1162). The audit then records `workspaceIdentityRevalidated: true`, even though the check ran before the transaction. See R-B.
- **Atomic dispositions, audit and lease release: met.** One immediate transaction guards each attempt update with its previous status, requires zero remaining open attempts, releases the lease with a guarded update, revokes the lease's capabilities, and appends one `reconciled` event (`src/worktree.rs` ~1156–1260). The test checks the audit contents and that exactly one event exists.
- **Exact replay idempotency: met.** A repeat with the same resolution returns the released lease. A different resolution is refused (tested).
- **Role denial: met at both layers.** Both tools return an error when the server has a role capability (`src/mcp.rs`, handler checks). `authorize_role_tool_call` also denies both tools for worker and Project Manager scopes (`tests/role_capabilities.rs`). Only the store-level denial is tested.

Amendments (required before live use):
- **R-A. Provider-stop evidence is self-attested, and a crash can leave a provider running.** Cathulu cannot verify that the provider has stopped. `AppServerProcess`'s `Drop` kills the child only on a clean shutdown (`src/app_server.rs:297–309`). No job-object containment is visible, so after a dispatcher crash the app-server may keep running and writing, and the recovery record holds no process identity to check. Record the child process ID and start time on each attempt at submission. Have the reconcile step verify that the process is gone, or refuse. Alternatively, add job-object containment so the child dies with the dispatcher, and state the residual risk.
- **R-B. Identity check is outside the transaction, but the audit says it was revalidated inside.** Either move the verification inside the immediate transaction, or record when it ran and drop the claim that it was atomic.
- **R-C. Missing tests.** Add coverage for: an identity mismatch during reconciliation (lock stays held); an invalid transition, such as `created` to `completed`; the MCP handler's role denial, not only the store's; and rollback when one attempt update fails.

Non-blocking:
- The unscoped local MCP does not authenticate the Owner, and any unscoped local MCP client can call these tools. The description says it must not be exposed remotely. Record that scope decision in ADR-0024, and leave the choice of who may call it to the Owner.

Status: `c4f0fd8` is accepted for the recovery transaction. R-A, R-B and R-C are reviewed below at `dd6bfc8`.

### Commit `dd6bfc8` (R-A, R-B and R-C fixes) (2026-10-25)

**Verdict: accept R-A, R-B and R-C as implemented. The residual spawn-to-assignment window and the non-Windows gap are recorded below and need a decision before live use. This was a read-only review under the handoff's resume, so Claude ran no builds.**

Checked against the handoff's list, read from `dd6bfc8`:
- **Windows job configuration and assignment failure: met.** The Job Object is created and configured with kill-on-close before spawn (`src/app_server.rs`, `ProcessContainment::new`). If configuration fails, startup is refused before the child exists. If assignment fails after spawn, the direct child is killed and waited on, and startup is refused (`AppServerProcess::start`).
- **Residual risk, stated accurately.** Assignment happens after `spawn()`, so a descendant created in that window is outside the job and survives a dispatcher crash. The child is not created suspended, so the window is not closed. Non-Windows builds have no containment (`#[cfg(not(windows))]`). The `provider_stopped_confirmed` flag is still an operator assertion. Containment makes the common crash case reliable, but the flag is not proof.
- **Identity verification inside the immediate transaction: met (R-B).** Verification now runs after the state checks and inside the same `Immediate` transaction that updates attempts and releases the lease (`src/worktree.rs`, reconcile). A mismatch returns an error, the transaction rolls back, and the lease stays `needs_reconcile`. The git check now runs while the SQLite write lock is held. That costs throughput, not safety.
- **Identity-mismatch lock retention: met (R-C).** The test asserts "workspace identity changed" and that the lease stays quarantined.
- **Invalid attempt transition refusal: met (R-C).** The test refuses `created` to `completed` with "cannot be reconciled from created to completed". The state is set through a scratch mutation, so the test covers the rule, not the normal path.
- **Actual MCP-handler role denial: met (R-C).** `tests/role_capabilities.rs` sends `tools/call` for `list_workspace_recovery_cases` through the role server with a Main capability and expects a JSON-RPC error containing "cannot call".
- **Atomic rollback on attempt-update failure: met (R-C).** A scratch `BEFORE UPDATE` trigger raises an error on the attempt update. The test then reads back that the lease, the attempt and the audit event are all unchanged.

Decisions and follow-ups (not blocking the code review):
- **D-1. Close the spawn-to-assignment window before live use.** Spawn the app-server suspended, assign it to the job, then resume it. This removes the descendant escape on Windows. Otherwise, state the residual risk in ADR-0024, and do not claim crash containment as exact.
- **D-2. Non-Windows is not contained.** Record that live recovery claims apply to Windows only, or add an equivalent containment for each supported platform.
- **D-3. Operator assertion.** `provider_stopped_confirmed` remains a self-attestation. The Owner should decide whether that is enough, or whether a process check is also required.
- **D-4. Lock hold during git.** Holding the write transaction through the git check is acceptable at current scale. Revisit if recovery runs against large repositories.

Status: `dd6bfc8` is accepted for R-A, R-B and R-C. D-1 and D-2 must be resolved, or their residual risk stated, before any live use. D-3 is the Owner's decision. The CX-05 handoff is Codex-owned, so this section is the record and Claude has not edited it. The next Claude write waits for Codex's resume.

### Commit `ecf7d11` (identity from the transaction snapshot) and docs `39ff26d` (2026-10-25)

**Verdict: accept `ecf7d11` as a refinement of R-B. The docs commit `39ff26d` records the residual limits and the caller scope. This was a read-only review under the Board's resume, so Claude ran no builds.**

- **`ecf7d11`.** The recovery check now reads the workspace path, the project path and both identity hashes in the same `Immediate` transaction that updates attempts and releases the lease (`src/worktree.rs:1182–1183`, from `current.3`–`current.6`). Before, the paths and hashes came from a read outside the transaction. The lease's identity fields are immutable under the v22 triggers, so the snapshot and the earlier read cannot disagree on identity. A mismatch returns an error, the transaction rolls back, and the lease stays quarantined. The change only narrows the timing gap.
- **`39ff26d` (docs only).** `docs/decisions/ADR-0024-workspace-leases-and-recovery.md` now records the Windows Job Object containment and its residual limits (Decision 9), the operator-assertion caveat, and the local-only caller scope. These match decisions D-1, D-2 and D-3 above. I checked the ADR lines that record them, not every sentence in the changed plan and handoff files.
- **Status of D-1 to D-3.** D-1 (the spawn-to-assignment window) and D-2 (non-Windows) are now documented as residual limits, not closed. D-3 remains the Owner's decision.

Status: `ecf7d11` is accepted as the R-B refinement. The docs commit is accepted as the record of residual limits. The CX-05 handoff is Codex-owned, so this section is the record and Claude has not edited it. The next Claude write waits for Codex's resume.

### Commit `0ce07f7` (lease failure and claim tests, with a behaviour-preserving refactor) (2026-10-25)

**Verdict: accept. The refactor keeps behaviour, and the new tests close the four gaps I had listed for M52: renewal failure, transport loss, direct revoke, and simultaneous claims. This was a read-only review under the handoff's stated permission for this stable checkpoint, so Claude ran no builds.**

- **Refactor (`src/dispatcher.rs`, `40ec573`→`0ce07f7`).** The renewal-failure, dispatcher-lost, connection-closed and transport-closed paths now call one helper, `mark_and_clear_active_turns_for_reconciliation`. That helper marks turns, then inbox items, then clears the tracking. The before-and-after operations are the same in each branch. The new `renew_active_workspace_leases_or_reconcile` and `handle_app_server_transport_loss` wrap them, and the loop still drops the app-server and continues.
- **Renewal failure (test `workspace_lease_renewal_failure_quarantines_live_turn_and_clears_tracking`).** An expired live lease fails renewal, the lease moves to `needs_reconcile`, the attempt and dispatch record follow, and the live-turn tracking is empty.
- **Transport loss (test `app_server_transport_loss_quarantines_live_turn_and_clears_tracking`).** Transport loss clears the tracking, and the lease and attempt both move to `needs_reconcile`.
- **Direct revoke (test `revoking_lease_with_open_attempt_quarantines_attempt_and_keeps_writer_lock`, `tests/workspace_leases.rs`).** Revoking a lease with an open attempt moves the lease to `needs_reconcile`, revokes the capability, moves the attempt to `needs_reconcile`, and a competing writer claim on the same project is refused as "already leased". The lock stays held.
- **Simultaneous claims (test `simultaneous_writer_claims_for_one_project_admit_exactly_one_writer`).** Two claim threads, released together by a `Barrier`, race for one project's writer slot. The test asserts exactly one wins, and the loser reports "already leased". This is a true concurrency test, so the earlier "no concurrent test" gap is closed.

Non-blocking notes:
- The renewal-failure test covers an expired lease. A renewal error from an identity mismatch is covered by the recovery tests, not by this one.
- The commit is titled `test:`, but it also contains the refactor. Keep that in mind when reading the history.

Status: `0ce07f7` is accepted. Remaining M52 work, as Codex lists it: semantic-overlap admission and authorized M42/M44 evidence. The CX-05 handoff is Codex-owned, so this section is the record and Claude has not edited it.

## 18. Design-only assessment: semantic overlap for M52 (2026-10-25)

**Answer: yes, the project-wide writer lock satisfies M52's written safety acceptance. It does not allow parallel writers in one project. That is a capability decision for the Owner, not a safety gap. This is a design-only assessment: no code, no build, no provider, no live state.**

Basis, from PLAN-020 M52:
- Subtask: "Start writers only in their exact leased linked worktree; coordinators/reviewers are read-only; serialize semantic overlap at task admission."
- Acceptance: "two writers cannot share a canonical writable worktree or active role lease; aliases/symlinks and wrong Git common-dir are rejected; PM cannot implement changes; each provider turn has a durable attempt bound to its active lease before submission; live turns renew their lease; crash, transport loss or expiry never releases a workspace with an unresolved attempt or silently starts a duplicate run; cleanup preserves dirty Owner data."
- Validation: "semantic-overlap denial".

Acceptance requires that writers cannot share a worktree or a role lease, and that overlap is serialized at admission. It does not require parallel non-overlapping writers. The v22 index `workspace_leases_one_writer_per_project_idx` (`src/migrations.rs:1816`) serializes all project writers, which is stricter than the acceptance. The validation item is covered by the simultaneous-claim test and the sequential writer-lease test. Both assert that the second writer is denied with "already leased".

Why the lock is sufficient for safety now:
- Separate linked worktrees share no files on disk. Two writers in one project would conflict only when their results are integrated. The lock prevents that integration risk, at the cost of throughput.
- `allowed_scope` is not persisted on the lease. It reaches the permission manifest only as a parameter (`src/worktree.rs:365`), taken from the task packet at dispatch. The sandbox runs `workspace-write` (`src/dispatcher.rs:2880`), which is workspace-level. So the system cannot prove that a writer stayed inside its packet path. The project-wide lock does not depend on that proof, which is why it is sufficient for safety now.

Smallest enforceable overlap contract, for when parallel writers are wanted (not required for M52 acceptance):
1. Persist the packet's `allowed_scope` on the lease at claim, immutable under the identity trigger. Derive the manifest from the lease, not the packet, so there is one source of truth.
2. Normalize each scope to repository-relative directory prefixes. Reject absolute paths, `..`, and symlinked prefixes.
3. Fail closed. An unknown, empty, invalid or root-wide scope (for example `**`) takes the project-wide writer slot and blocks all others.
4. Check overlap in the claim's immediate transaction. A new writer may coexist with active writers only if its prefixes are pairwise disjoint from each of theirs (no equality, no nesting). This replaces the one-writer partial index, with a v23 migration. Existing v22 writers stay project-wide.
5. Detect violations after the run. Compare the worktree's changes against base with the declared scope. Any change outside it blocks integration and marks the attempt for reconciliation. The sandbox cannot prevent this, so the control is detective.
6. Deterministic tests: disjoint scopes both admitted; equal, nested and overlapping prefixes refused; unknown, empty and `**` scopes take the project slot and block others; scope unchangeable after claim; manifest scope equal to lease scope; an out-of-scope change flagged after the run; v22 leases still project-wide after migration.

Owner product decisions:
- Whether parallel writers in one project are wanted now. Until then, the project-wide lock stays the rule.
- Whether detective enforcement after the run is acceptable, given that the sandbox cannot enforce paths preventively.
- Prefix or glob syntax, and the symlink policy.

Status: design only. The runtime gate stays false. Dispatch stays off. No Codex-owned file was edited, and no build, provider or live state was touched.

### Commit `235c4b9` (bind provider context to lease attempts) (2026-10-25)

**Verdict: accept. This was a read-only review under the handoff's permission for a committed follow-up, so Claude ran no builds. Two notes; neither blocks the change.**

- **Guard before submission (both paths).** `attempt_guard.preserve_for_provider_submission()` runs before `start_turn` in the queued path (`src/dispatcher.rs:2981`) and the Project Manager path (`:3234`). A refusal after submission begins cannot mark the attempt `blocked`, which would wrongly end its open state.
- **Acknowledgement handling.** An error from `start_turn` sets the attempt to `needs_reconcile` (`:2984–2987`) and keeps the lock. A success tracks the active turn first (`:2999`), then records `submitted` with the turn ID (`:3010`). If that write fails, the lease is still renewed and the attempt stays open.
- **Context receipts.** `select_delegation_context` runs in the attempt-creation helper before submission (`:2531`). Receipts move to `submitted` when the attempt reaches `submitted` or `running` (`src/provider_sessions.rs`, `set_provider_attempt_state`). Context is therefore tied to the lease-bound attempt.
- **Revocation on terminal states.** A terminal attempt state (`completed`, `failed`, `blocked`) revokes the capabilities bound to that attempt, including the Main capability. This matches the M52 acceptance "revoke access on completion".
- **Lease identity in results.** `WorkResult` validation now requires the result and its order to share a `workspace_lease_id` (`src/execution_artifacts.rs`).

Notes (not blocking):
- **N-1. `created` is now the state until acknowledgement.** The explicit `submitted` write before `mark_delegation_dispatched` was removed. An attempt is therefore `created` from binding until `start_turn` is acknowledged. `created` counts as open, so the lock is held, which is correct. But the operator reconciliation may only move `created` to `failed` or `blocked`, never `completed`. An operator who knows the turn finished cannot record that, and must record `failed` with a reason. Decide whether `created` to `completed` should be allowed with provider-stop evidence, or document the rule.
- **N-2. Test coverage.** The receipt tests now assert the automatic transition, and a completed attempt's capability is refused. The guard-before-submission ordering is covered by the existing attempt guard test, not by a new test.

Status: `235c4b9` is accepted. N-1 is a reconciliation-usability decision for the Owner and Codex. The CX-05 handoff is Codex-owned, so this section is the record and Claude has not edited it.
