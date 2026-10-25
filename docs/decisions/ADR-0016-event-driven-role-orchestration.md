# ADR-0016 — Event-driven role orchestration through local Codex threads

- Status: Accepted for local implementation and default-on dispatch in the matched Windows build, following the Owner's explicit direction at that time. Current product activation is held off under [ADR-0024](ADR-0024-workspace-leases-and-recovery.md) until exact workspace-lease, permission-manifest, and credential-source admission is enforced before provider sessions and turns. The local app-server protocol has been exercised in hidden scratch runs and against a loopback mock provider; the scoped MCP workflow, single Main binding, role event routing, and Owner-decision return are implemented. M36 live provider/account behavior, actual target-project workspace tool parity, user-visible Codex thread ownership, and real parallel/restart behavior remain unverified. M39's deterministic workflow gates remain authoritative.
- Date: 2026-10-22
- Decision owner: Cathulu project owner
- Plan: [PLAN-015](../plans/PLAN-015-event-driven-role-orchestration.md)

## Context

The Main Overseer currently dispatches PMs and then waits or monitors their work in the same model turn. A parallel fan-out does not solve this: the parent remains generating/waiting and can delay owner updates. The owner requires asynchronous independent work and immediate escalation of only the project that needs a decision.

Current Cathulu MCP can persist runs, delegations, checkpoints, reports, owner actions, and QA evidence. It cannot create or resume Codex chats or start a new turn in an idle chat. MCP is request/response; recording an event does not wake a model. The existing M21 probe proves one native nested PM → Worker chain only. The owner additionally requires an explicit workflow brief for each project before PM dispatch, project-level task decomposition by the PM, and review by each parent role before child work is represented as accepted.

Current official OpenAI documentation describes Codex app-server as an application integration that can create/resume local persistent threads, start turns, stream events, and handle approvals. The installed CLI and isolated local probes confirm the protocol lifecycle, but not the user's live provider/account behavior, target-project tool catalog, or visible UI ownership. The architecture is accepted for local implementation; those runtime properties remain explicit post-reload checks.

The first M36 smoke confirmed local stdio initialization and an ephemeral read-only thread start, but process-only MCP overrides did not isolate configured servers. A later scratch-home probe verified an empty MCP inventory, hidden stdio initialization, zero MCP servers before and after thread start, and successful deletion of an empty persistent read-only thread. Resuming that empty thread returned `-32600: no rollout found`. In a separate scratch home with only the newly built Cathulu MCP, `read_memories` and `cathulu://dashboard` succeeded and 46 tools/3 resources were visible from the Cathulu workspace context; the explicit headless flag prevented desktop startup. A scratch CWD outside the Cathulu workspace exposed the server but zero tools/resources, for an unknown reason. Finally, an isolated loopback mock Responses provider completed an assignment-only turn, exited, and then resumed the same persistent thread in a second app-server process for a second completed turn. Both requests were streamed to localhost with no authorization header; the fixture used no remote model, user config, project repository, desktop, or terminal window. This verifies local saved-turn streaming/resume mechanics, not the user's actual provider/account or runtime parity. Project-workspace tool access, role skills/config inheritance, Codex UI visibility, and approval routing remain unverified. Keep the production dispatcher gated until those remaining runtime checks are resolved.

## Decision

Subject to M36 passing, Cathulu should coordinate persistent logical role threads using the local Codex app-server. Codex remains the reasoning, code, and tool execution harness; Cathulu owns the durable hierarchy, dispatch decisions, message routing, evidence, approval records, and dashboard projection.

```text
Owner — final approval authority
  ↕ only sensitive/out-of-scope decisions
Main Overseer — cross-project coordination and bounded judgment
  ├─ Project Manager — one project; resolves in-scope details
  │    └─ up to two Workers — bounded implementation tasks
  └─ independent project managers continue without a global wait barrier

Cathulu event ledger / async dispatcher
  records every handoff; wakes only the next role that must act
```

1. Main dispatches every independent eligible PM through the Cathulu dispatcher in parallel. Once the dispatch tool acknowledges accepted/failed requests, Main's current turn ends. There is no Main wait loop, sleep loop, `wait_threads`, or all-project completion barrier.
2. PM and Worker threads run independently. Role and assignment identity remain in Cathulu; Codex thread IDs are private replaceable execution handles. Cathulu must never guess which arbitrary chat is the Main role.
3. Workers report to their PM. PMs handle normal project decisions inside the accepted task packet. Main handles cross-project coordination and bounded reversible choices within explicit owner constraints.
4. Cathulu persists reports first and routes actionable updates upward. Routine progress is available to the dashboard but does not start Main generations. A blocker, escalation, owner action, or material project result starts a new short Main turn with compact state and relevant deltas. After handling it, Main becomes idle again.
5. Owner action is required for creating accounts; creating, entering, or changing credentials/secrets; changing access/permissions; spending; unapproved external disclosure; destructive/irreversible actions; production deploy/release/publication; external commitments; and actions outside the explicit authorized scope. Each approval applies only to the exact recorded action. The affected project pauses; other independent projects continue.
6. Preserve one Main role, one active PM assignment per project, two Worker assignments per PM assignment, stable memory scopes, context-transfer accounting, and deterministic QA. No specialist model or agent report can establish success or silently approve an action.
7. App-server/process failure leaves durable dispatches and messages retryable. Do not start a duplicate task or fall back to a wait-coupled parent turn without first reconciling thread/turn state.
8. Before dispatch, Main records a distinct workflow brief per requested project: objective, phases/checkpoints, scope boundary, acceptance/QA evidence, risks, and approval gates. It plans project outcomes and coordination, not implementation details. A missing objective blocks only that project's PM dispatch.
9. A PM converts its brief into bounded Worker tasks with scope, acceptance criteria, and evidence requirements. PM reviews each Worker report and records accept, follow-up/rework, or escalation. Main reviews each PM's plan/result against the brief and evidence; parent review applies per project and never waits for other projects to finish.
10. Escalations travel Worker → PM → Main → Owner. Each role resolves what falls within its authority before escalating. If Owner input is necessary, Main surfaces the exact action, scope, reason, and consequence as soon as the event arrives. Only that project's dependent work pauses; unrelated work continues. Owner requests are never collected for a final batch.

## Consequences

- Schema v13 now persists immutable Main/PM work plans, parent review records, plan-to-delegation references, and explicit owner-approval requests/decisions. SQLite gates PM/Worker assignment and result-review ordering without depending on the app-server probe. This does not implement asynchronous dispatch or wake an idle Main chat.
- The local gates validate declared role fields and state order; Cathulu MCP does not receive authenticated Codex chat identity. They are workflow integrity controls, not an authorization boundary.
- `PLAN-014`'s parallel fan-out instruction is a useful dispatch-order correction but insufficient as the intended runtime: first-result monitoring still keeps Main active. `PLAN-015` replaces it as the behavior plan.
- Implementation requires additive SQLite role-thread bindings and an idempotent message inbox/outbox, a single dispatcher owner, thread lifecycle recovery, and explicit escalation/approval routing.
- Schema v14 adds a durable dispatch outbox, SQLite singleton lease, logical role-thread bindings, append-only dispatch events, and a no-wait app-server worker. `dispatch_prepared_delegations` is enabled by default on normal MCP startup; `CATHULU_ROLE_DISPATCHER_ENABLED=0` disables it. MCP startup creates only the background dispatcher thread; the hidden app-server starts on demand when queued work or a routed event exists. It returns after turn-start acknowledgement and never waits for model completion. Ambiguous sends move to reconciliation and are never replayed automatically. A PM dispatch includes the existing Main workflow brief and original assignment packet.
- The M40 worker checks the thread-scoped Cathulu MCP tools before it sends a role turn. Missing tools block that assignment before model execution. It requests on-demand sandbox approvals only for work outside the granted workspace; every server approval or interaction request is rejected. No approval is automatic.
- Schema v15 records `decision_origin` for Owner decisions. New requests appear in the selected Main/PM dossier with an explicit two-step decision. SQLite checks the displayed hash, active PM assignment, latest unresolved source action, duplicate requests, and single-decision constraint; the dashboard records Owner intent only and never executes or closes the action. The MCP decision tool was removed so a model cannot claim the Owner's choice. Existing pre-v15 decisions are labeled `legacy_unverified`.
- Schema v17 adds the actionable role inbox and event cursor. The cursor starts at the migration-time maximum event ID, so legacy event history does not get replayed. New actionable Worker events route to the active PM; PM plan reviews/results and PM escalations route to the single Main role; Main plan/result reviews and relayed Owner decisions return to only the affected PM. Routine progress remains passive, inbox insertion is idempotent, and a role with an active turn is not given a second concurrent turn.
- Schema v18 supports one project-less `main-overseer` thread binding and one hashed global Main capability, without replacing existing PM/Worker bindings. Dispatcher-created turns use hashed role capabilities scoped to a logical role, project, and delegation. Main's event capability allows read/review/escalation operations needed for short event turns; it does not grant implementation or Worker-dispatch tools. Regular MCP calls outside these role-bound threads still do not authenticate a Codex chat identity; the role capability is the dispatcher trust boundary.
- The desktop Owner control persists intent only. A recorded decision creates an idempotent return event to Main; Main relays the exact decision to the affected active PM, and the underlying action stays open until its result is reported. This does not run the requested operation.
- The dispatcher process reads only the names of MCP servers from the user's base Codex config and passes process-local `enabled=false` overrides for them. It does not rewrite config or copy server commands/secrets. Per-thread configuration then adds only the scoped Cathulu MCP server; Cathulu MCP is headless by default and background CLI processes suppress Windows console creation. Invalid/unreadable config fails closed. This prevents app-server startup from launching the user's unrelated MCP servers.
- The dispatcher prompt carries the authoritative assignment packet and project memory scope. A role MCP tool catalog is checked before each turn; missing tools block before model execution. App-server approval/interaction requests remain rejected unless a separately implemented Owner path handles them. Actual Codex account/provider behavior, intended project workspace access, UI ownership, and real parallel/restart execution remain runtime checks; the source is enabled so those checks can be performed after reload rather than blocked by a second code gate.
- The installed skill and contract must not describe native parent-child waiting as satisfying the no-wait requirement.
- Per-project plans and parent review decisions make responsibility visible: PMs remain accountable for Worker acceptance; Main remains accountable for checking PM results against the Owner's project brief. These review events are separate from deterministic QA and do not themselves establish code correctness.
- Main's workflow brief and each PM's reviewed result are delivered as project-scoped deltas. An event-driven dispatcher must preserve prompt owner escalation without waking Main for routine checkpoints or creating an all-project barrier.
- Threads, tool availability, project status, and approval outcomes remain independently observable in Cathulu. Missing app-server or hook events remain unknown.
- App-server integration is not a new remote specialist model and does not send Cathulu memory to an external provider by default. Codex's normal account/provider policies still apply.
- The isolated no-turn probe proves hidden thread lifecycle in a scratch `CODEX_HOME`; the loopback mock verifies saved-turn streaming/resume mechanics, not the user's provider/account. A scratch MCP probe returned Cathulu tools only from the Cathulu workspace context; a no-repository CWD returned no tool catalog and the reason remains unknown. The matched build must be reloaded before schema v18 and event routing are live. New tests cover the stored Main/PM routes, role capability limits, idempotency, project isolation, no self-wake loops, and Main's independent dashboard activity signal. Live role workspace/tool parity, user-visible thread ownership, queue/restart concurrency, and real parallel work remain open validation items in PLAN-015; the runtime is no longer held behind an independent false readiness constant.

## Reconsider if

- M36 shows the installed client cannot create/resume role threads, use the current local model configuration, access required Cathulu MCP tools, or surface needed owner approvals.
- App-server threads cannot be visibly or practically used as the user's Main/PM chats and the user rejects a Cathulu-owned thread surface.
- Process recovery cannot be made idempotent without duplicate work or unsafe approval behavior.
- The owner changes the hierarchy, authority limits, privacy boundary, or local-first requirement.

## References

- [Codex App Server](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server) — official local app-server stdio setup and thread/turn lifecycle.
- [Codex app-server protocol README](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) — current protocol behavior and MCP status methods.
- [Codex as a platform](https://developers.openai.com/blog/codex-as-a-platform) — application-owned workflow and Codex harness integration options.
- [Multi-agent](https://developers.openai.com/api/docs/guides/responses-multi-agent) — root/subagent dispatch, messaging, and waiting semantics.
- [Run and continue sessions](https://developers.openai.com/api/docs/guides/agents-api/sessions) — hosted Agents API session continuation; not the target for Cathulu local execution.
