# PLAN-015 — Event-driven Codex role orchestration

Status: M39 hierarchy gates validated locally. M40/M41 are implemented, formatted, and were published in the matched Windows build at fingerprint `49403D5945B920FF8B571B1A58C0A746931AE4EEEC993028F4BDF6808F06081A`. The current M52 runtime gate holds product dispatch off until the app-server path validates an exact workspace lease, permission manifest, and credential source before provider sessions and turns; environment flags cannot override it. The implementation adds hidden on-demand dispatch, one global Main binding/capability, and selective Main↔PM event routing. M45 removes desktop auto-launch from MCP; historical M36 startup notes below predate that correction. Focused role-routing/capability tests and all eight dashboard tests pass. M36 confirms isolated startup and mock turn streaming/resume, but actual provider behavior, role skill/model/effort/approval parity, intended project-workspace tool access, Codex UI ownership, and real parallel/restart behavior remain unverified and are M42 live checks.
Created: 2026-10-22

Last updated: 2026-10-23

## Objective

Make Cathulu dispatch independent project work asynchronously through local Codex role threads. Before dispatch, Main creates a concise outcome-focused workflow brief for each requested project. Each PM turns its brief into bounded Worker tasks, reviews Worker evidence, and reports a project result for Main's review. Main must finish its dispatch turn and stay idle while PMs and Workers work. Cathulu records events and starts a new Main turn only when an update needs Main's judgment, owner attention, or a project-level result. The Owner remains the final authority.

## Motivation

Current development policy: [ADR-0021](../decisions/ADR-0021-continuous-supervised-development.md) and [PLAN-019](PLAN-019-continuous-supervised-development.md) make the live M42 checks part of Cathulu development. Develop/repair Cathulu separately; development agents arrange the already authorized external-project assignments and record their real results. After applicable local checks, a bounded supervised assignment may establish runtime capability before full M42 acceptance. Its normal brief, memory, role, scope, and tool-preflight gates still apply. Queue acceptance alone is not a role start, and an empty queue is not proof of dispatcher failure. Main ends its turn after acknowledgement and handles subsequent routed evidence without a wait/poll loop. Observed failures are concrete blockers; missing evidence remains unverified. Check the currently loaded runtime rather than assuming that dated reload notes below describe the current process. Calibration, benefit claims, autonomous promotion, optional-provider consent, and Owner-authority actions retain their separate gates.

M35 added parallel PM fan-out instructions, but its follow-up instruction to wait for the first PM result still keeps Main's model turn active. The owner clarified that parallelism must include independent progress and escalation: Main must not hold a response or a project behind the slowest project, Worker, or probe. The hierarchy also needs an explicit planning and review chain: Main defines the outcome, phases, boundaries, and acceptance evidence for each project; PMs decompose that brief into Worker tasks; PMs assess Worker results; Main assesses PM results. Routine updates may be visible in Cathulu without waking Main. Actionable information travels only as far up the role chain as needed, and owner actions surface when needed rather than being bundled at the end.

The previous MCP build records delegations, reports, checkpoints, and owner actions but does not have the new chat/thread dispatcher. The published matched build added a dispatcher that uses the local Codex app-server to start/resume persistent role threads, submit asynchronous turns, and route role inbox events. Although that matched build enabled the source by default, the current M52 runtime gate holds product dispatch off until exact workspace-lease, permission-manifest, and credential-source admission runs before provider sessions and turns. The installed provider/account behavior and user-visible thread ownership remain explicit live checks.

## Current state

- Schema 13 stores one logical Main role and one Project Manager role per registered project, with stable memory/tool-profile scopes, immutable role plans, parent reviews, and owner-approval records.
- MCP enforces plan-before-assignment, accepted-plan-before-Worker, parent-report-review, and unresolved-owner/escalation gates. The published matched build adds app-server-backed thread creation/resume and asynchronous turn delivery; the currently loaded process remains on the previous build until reload.
- M35 instructed Main to dispatch all PMs before a first-result wait. The owner confirmed that still keeps Main generating/waiting, so the dispatcher now ends Main's dispatch turn after queue acceptance and starts fresh short Main turns only for routed events. The published build contains this behavior; a reload and real multi-project run are still required to verify it end to end.
- M21 verified one nested PM-to-Worker chain and role-specific MCP access differences; it did not test independent asynchronous role threads. M39 adds deterministic stored workflow gates but cannot authenticate the calling chat's claimed role.
- The local implementation now has a durable role inbox and one hashed global Main capability. The normal MCP process starts one dispatcher thread; the hidden app-server starts only for queued dispatch or actionable inbox work. Its process-level config disables globally configured MCP servers without rewriting the user's config; each role thread gets only its scoped Cathulu MCP configuration. Invalid/unreadable Codex config blocks dispatcher startup. The migration cursor starts at the current maximum event ID, so old events are not replayed.
- A fresh scratch `CODEX_HOME` with an empty MCP inventory was verified. Hidden stdio initialization, an empty read-only thread, and deletion passed; `thread/resume` returned `-32600: no rollout found` because no turn had created a rollout. A second isolated scratch configuration with only the newly built Cathulu MCP exposed 46 tools and 3 resources; `read_memories` and `cathulu://dashboard` succeeded against a scratch database. `CATHULU_DISABLE_DASHBOARD_AUTOLAUNCH=1` kept the desktop closed, and process inspection found one MCP process and no desktop process. No user credentials, project repositories, user configuration, or model turn were used. A separate probe from a scratch CWD outside the Cathulu workspace exposed the MCP server but no tools/resources; the cause is unknown. This verifies tool/resource access only in the tested workspace context, not general trust or role-skill inheritance.

## Desired state

```text
Owner
  ↕ questions requiring owner authority / approvals
Main Overseer — one short dispatch or event-response turn at a time
  ├─ Project Manager: Project A ── Workers A1/A2
  ├─ Project Manager: Project B ── Workers B1/B2
  └─ Project Manager: Project N ── Workers N1/N2

Cathulu local event ledger + dispatcher
  ├─ persists project-local updates and reports
  ├─ leaves routine progress visible without waking Main
  ├─ routes a decision/blocker to the lowest role that can resolve it
  ├─ starts a fresh role turn for actionable messages
  └─ pauses only the affected project when owner input is required
```

- Main prepares and dispatches every eligible independent PM concurrently. Once each dispatch has been accepted or recorded as failed, Main returns a concise dispatch acknowledgement and its model turn ends. Main must not wait, poll, sleep, or repeatedly inspect threads while work runs.
- Before PM dispatch, Main records a separate workflow brief for each requested project: desired outcome, ordered phases/checkpoints, project boundary, relevant dependencies (if explicitly declared), acceptance/QA evidence, owner-approval gates, and known risks. Main plans coordination and outcomes, not implementation details. Missing project objectives are clarified only for the affected project; unrelated ready projects proceed.
- PM and Worker turns execute independently. Their thread IDs are associated with durable Cathulu roles/assignments, not treated as the role identity itself. SQLite remains the authority for assignments, handoffs, approvals, and evidence.
- Each PM turns its Main brief into a project plan and bounded Worker packets with explicit scope, acceptance criteria, evidence requests, and task dependencies. A PM dispatches no Worker without a concrete task packet and remains accountable for project coordination.
- Worker questions and reports go to the PM. The PM evaluates each Worker result against the packet, scope, source evidence, and required checks; the PM accepts, requests follow-up, or escalates. An unreviewed Worker claim is never forwarded as a verified result.
- Main evaluates each PM's project plan and result against the Main brief, owner priorities, recorded Worker reviews, and deterministic evidence. Main may accept the result, request PM follow-up, or handle/route a decision. These reviews occur per project and per material event; they do not create an all-project completion barrier.
- Main handles portfolio coordination, cross-project scheduling, and bounded reversible decisions inside the owner's explicit constraints. It resumes for a new short turn when a routed event needs its judgment, then ends that turn when handled.
- The owner is the final authority. Creating accounts; requesting, entering, or changing passwords, API keys, or other secrets; changing access or permissions; spending money; disclosing data outside the approved local boundary; destructive/irreversible operations; production deploys/releases/publications; commitments to third parties; and anything outside the approved task scope require explicit owner approval before execution.
- Escalations travel Worker → PM → Main → Owner. A Worker or PM must first resolve the issue if it is within their authority; Main likewise handles bounded coordination and reversible decisions. Only an action that crosses the explicit Owner-authority boundary becomes an Owner request. Main sends the exact action, reason, scope, and consequence as soon as that request arrives, not in a final batch. That request pauses only its project/task; other independent projects continue. Approval is recorded for that specific action and does not authorize adjacent actions.
- Main receives no routine heartbeat or every-tool-call message. The dashboard may project recorded progress passively; a new Main turn is reserved for actionable events and material project results.
- Verified success still requires the existing deterministic QA gates. No chat, app-server event, or self-reported update can mark work successful on its own.

## Scope

- Verify local Codex app-server behavior and suitability on Windows without accessing a project repository or opening/focusing a window.
- Add a local app-server adapter, durable logical-role thread bindings, and idempotent asynchronous dispatch/return routing if M36 passes.
- Preserve Main → PM → Worker limits, independent project scopes, stable memory scopes, and existing QA/evidence authority.
- Persist one Main workflow brief per project before PM assignment, the PM's task decomposition, and each level's review decision and evidence references without duplicating existing tasks, reports, or QA records.
- Add escalation/approval state and return routing so Main can resolve ordinary issues and request owner action only when required.
- Update the operating contract, architecture, decision records, dashboard projection, and installed `cathulu-orchestrator` skill to match implemented behavior.
- Build the matched desktop/MCP application after implementation; never auto-open or restart either application during development validation.

## Out of scope

- Replacing Codex, its model, or its reasoning/coding role with a specialist model.
- Using the hosted Agents API as a substitute for the user's signed-in local Codex runtime.
- Automatic approvals, unrestricted command execution, or automatic promotion of decisions beyond the task's authority.
- Cross-repository dependencies or handoffs that the owner has not requested.
- Polling loops or one continuously generating Main turn that tracks every project.
- Claiming that app-server-created threads appear in the Codex UI until M36 verifies it.

## Source-of-truth impact

- [ADR-0016](../decisions/ADR-0016-event-driven-role-orchestration.md) records the proposed app-server and event-routing architecture; it remains proposed until M36 validates the local host behavior.
- [ADR-0009](../decisions/ADR-0009-bounded-project-agent-hierarchy.md) continues to define role limits and project independence; its native wait/monitor behavior is superseded by this plan when the dispatcher ships.
- [Agent operating contract](../agent-operating-contract.md) defines decision authority, asynchronous dispatch, escalation, and no-wait behavior.
- [Architecture](../architecture.md), [local MCP guide](../local-mcp.md), and [roadmap](../roadmap.md) distinguish current passive evidence capture from the planned active thread dispatcher.
- SQLite remains authoritative. A persisted role-thread binding is an execution handle, not a replacement for logical roles or memory scopes.

## Existing decisions and constraints

- Codex remains the primary model and execution harness; Cathulu coordinates, stores evidence, routes messages, and provides the dashboard.
- Exactly one logical Main role exists. There is at most one active PM assignment per project, and at most two implementation Workers per PM assignment.
- Project repositories are independent unless the owner explicitly defines a dependency or shared task.
- Existing task/run/delegation/memory/known-by/transfer/report/QA entities must be extended rather than duplicated.
- Keep local-first behavior, hidden/no-focus subprocesses on Windows, and the current matched-build convention. No new console window may open.
- User action is required for sensitive or out-of-scope operations. A task packet's explicit prior approval is narrow and auditable.

## Supporting skills / tools

- `implementation-plan`, `milestone-delivery`, `cathulu-orchestrator`, `skill-creator`, and `openai-docs`.
- Codex App Server local protocol; use current OpenAI documentation and verify the installed CLI/runtime before coding against it.
- Existing Cathulu SQLite event, delegation, logical-role, memory, hook, dashboard, and QA surfaces.
- Context7 was attempted for the Codex app-server reference but its monthly quota was exhausted; current official OpenAI app-server documentation was available and is linked from ADR-0016.

## Dependencies

- M39's deterministic planning/review/approval gates do not depend on M36.
- M36 confirms the exact local Codex binary, supported app-server protocol, persistence/reconnect semantics, MCP/tool configuration, approval handling, UI visibility, and hidden process behavior.
- M40's durable dispatcher owner and idempotent outbox are implemented. M41's global Main binding and event inbox build on those records; real provider/workspace/UI behavior still needs live confirmation.
- M41 depends on M40's event correlation and durable inbox/outbox.

## Risks and unknowns

- App-server-created threads may not appear as ordinary Codex UI chats or may not inherit the configured Cathulu MCP/skills. M36 is a stop/go gate.
- The current host may not expose a reliable caller thread ID. Cathulu must own or explicitly bind a Main role thread rather than guess which open chat is Main.
- Multiple MCP processes could race to run an app-server dispatcher. A singleton/lease and idempotent outbox are required; duplicate dispatch must be prevented.
- Process restart during a turn must recover from persisted thread/turn IDs without replaying a task or claiming completion.
- Human approval requests must be scoped, auditable, and unable to block unrelated projects.
- App-server or MCP outage must leave durable messages visible and retryable, without a busy loop or duplicate work.
- API keys and passwords must never enter logs, checkpoints, task packets, memory, or telemetry. A human can enter them only through an approved secure target.

## Milestones

### M36 — Local Codex App Server capability probe

**Goal:** Prove the documented local integration matches the owner's Windows Codex installation and can support no-wait PM role threads safely.

**Subtasks**

- [x] Identify the Codex CLI executable and installed version without launching a GUI or changing user configuration: `C:\Users\sjuan\AppData\Roaming\npm\codex.ps1`, `codex-cli 0.146.1`. Help reports app-server as experimental and supports stdio.
- [x] Initialize a hidden stdio app-server and confirm its initialization response reports remote control disabled; no user configuration changed.
- [x] Probe process-only MCP overrides. Per-server `enabled=false` overrides failed config loading with `invalid transport`; an empty `mcp_servers={}` override did not clear the effective registry.
- [x] Start one ephemeral read-only thread in the scratch directory and confirm `thread/started`; the response had `ephemeral: true`, `path: null`, and no turns. No model turn was requested. The app-server nevertheless started configured MCPs, including Cathulu, so it was stopped immediately after those startup notifications appeared. No MCP tool call was made.
- [x] Establish a scratch `CODEX_HOME` whose `codex mcp list` result is empty; launch app-server with hidden window and redirected stdio. Before and after thread start, `mcpServerStatus/list` returned zero servers.
- [x] Add the explicit per-process `CATHULU_DISABLE_DASHBOARD_AUTOLAUNCH=1` diagnostic override so a later MCP/tool-parity probe can start Cathulu MCP without opening the desktop. The unset default continues to schedule dashboard startup.
- [x] Start an ephemeral read-only thread without a model turn. Confirm it has no turns and no thread-scoped MCP servers.
- [x] Confirm that an empty no-turn thread cannot be resumed: `thread/resume` returned `-32600: no rollout found`. Start and delete a separate persistent empty read-only scratch thread; deletion succeeded and app-server exited with code 0. No model turn or MCP tool call occurred.
- [x] In a separate isolated scratch configuration, start only the newly built Cathulu MCP with a scratch data directory and headless dashboard flag. From the Cathulu workspace context, verify 46 tools and 3 resources, call `read_memories`, and read `cathulu://dashboard`; process inspection confirms no desktop process. No user project repository or credentials were used.
- [ ] Explain or resolve why an isolated no-repository scratch CWD exposes the Cathulu MCP server but zero tools/resources; do not assume those tools are available to arbitrary role workspaces.
- [x] Verify streamed turn completion, app-server exit, and resume of the same persistent thread using a local loopback mock Responses provider. Two assignment-only turns completed across two app-server processes; both exited cleanly. No remote model/account, user config, Cathulu MCP, project repository, desktop, or terminal window was used. This validates the local protocol lifecycle only.
- [ ] Verify the created thread is discoverable/reopenable in the normal Codex UI, or record that it is only visible through the app-server client.
- [x] Verify Cathulu MCP tool/resource access against a scratch store from the Cathulu workspace context. The separate no-repository CWD exposed no tools/resources.
- [ ] Verify role skills, model/effort selection, and sandbox/approval requests in the created thread.
- [ ] Record which process owns thread lifecycle and how it recovers after MCP/app restart.

**Affected areas:** MCP startup guard for explicitly headless diagnostics and the local probe harness; no schema, user config, or production orchestration changes. Normal dashboard auto-launch remains the default.

**Dependencies:** Installed Windows Codex CLI and current user sign-in.

**Acceptance criteria**

- [ ] App-server can create and resume local Codex threads using the user's existing local configuration without starting unrelated processes. Scratch isolation and empty-thread creation/deletion passed; resumption requires a saved rollout and remains unverified.
- [ ] Thread turn can complete asynchronously without Main waiting on a child result.
- [ ] Cathulu tools and required role instructions are available to the thread in each intended project workspace, or the exact gap is documented as a no-go. Current tool/resource verification is limited to the Cathulu workspace context.
- [x] Isolated probes used hidden processes and redirected stdio; the headless Cathulu check launched one MCP process and no desktop process. User Codex configuration and active services were untouched.
- [ ] Codex UI visibility and thread replacement/ownership semantics are known, not inferred.
- [ ] No project repository is read or modified.

**Validation:** Protocol smoke in a disposable directory; event/turn status capture; process/window inspection; explicit MCP-resource/tool check.

**Documentation updates:** PLAN-015 findings and ADR-0016 decision status.

**Applicable specialized skills:** `openai-docs`, `implementation-plan`, `milestone-delivery`.

**Expected Git checkpoint:** One capability-probe record; no release build unless source code changes.

### M39 — Deterministic hierarchy plans, reviews, and owner-approval gates

**Goal:** Enforce the Main → Project Manager → Worker work contract in Cathulu's SQLite lifecycle before any dispatcher exists.

**Implemented behavior**

- Main must record a project/run-scoped workflow brief before Cathulu prepares that project's PM assignment.
- A PM must record its bounded Worker plan, and Main must accept that plan before any child Worker can be prepared. Each Worker packet must match its approved plan task key and packet exactly.
- PM result submission waits for Worker reports to be accepted; Worker reports are reviewed by the PM, PM reports by Main, and PM assignment completion waits for Main's accepted review.
- Worker action requests route to the PM; PM action requests route to Main. Main can create an explicit owner-approval request only for an unresolved PM escalation. An Owner decision closes the approval request only; the underlying action remains open until the responsible role reports its outcome with evidence. Pending approvals and unresolved actions prevent PM result submission/completion.
- The read-only dashboard distinguishes project-role escalations from requests that actually need the Owner.
- The gates validate ordering, scope, and supplied reviewer-role fields. Cathulu MCP does not receive authenticated Codex chat identity, so it cannot prove that the caller is in fact Main, PM, Worker, or the Owner. This is workflow enforcement, not a security boundary.

**Acceptance criteria**

- [x] PM preparation rejects a missing Main brief.
- [x] Worker preparation rejects a missing/unaccepted PM plan or a packet that differs from its approved task.
- [x] PM report submission waits for accepted Worker reports; parent report reviews gate report acceptance and PM completion.
- [x] Unresolved escalations and owner approvals block PM result closure; an Owner decision clears only the approval request, while the underlying keyed action stays open until its outcome is reported.
- [x] Dashboard state labels Main/PM routing separately from explicit Owner approval requests.
- [x] Schema migration, role lifecycle integration tests, frontend tests, and current matched build pass with recorded evidence.

**Dependencies:** Existing role hierarchy and M27 role catalog; independent of M36.

**Affected areas:** `src/delegation.rs`, `src/migrations.rs`, `src/dashboard.rs`, `src/mcp.rs`, desktop dashboard, lifecycle tests, ADR/architecture/operating contract.

**Applicable specialized skills:** `milestone-delivery`, `cathulu-orchestrator`.

**Expected Git checkpoint:** Validated M39 checkpoint before merge.

### M40 — Durable no-wait dispatcher and role threads

**Status:** The local dispatcher was enabled by default in an earlier matched build. Product dispatch is currently held off by the M52 runtime gate: the app-server path does not yet validate an exact workspace lease, permission manifest, and credential source before provider sessions and turns. Environment flags cannot override the hold. Normal MCP startup therefore does not start the role-dispatcher service. Required tool preflight and capability gates are in place, but they do not replace M52 admission. Actual provider/account behavior, intended project-workspace MCP catalog, Codex UI ownership, and real concurrency/restart behavior remain live verification items; the current build is not loaded until the Owner reloads Codex.

**Goal:** Dispatch independent PM and Worker role turns through one local Codex app-server adapter, return immediately, and persist enough state to resume safely.

**Subtasks**

- [x] Add schema-v14 foundations for durable role-thread bindings, a SQLite dispatcher lease, append-only dispatch events, an idempotent outbox, turn state, and delivery attempts. Ambiguous in-flight work becomes `needs_reconcile`; it is never replayed automatically.
- [x] Implement a default-on single background app-server RPC owner with bounded protocol requests, a SQLite lease, Windows no-console creation, and process-local suppression of the user's globally configured MCP servers. It starts only after a prepared delegation is queued or an actionable role event arrives.
- [x] Add a batch MCP dispatch operation that accepts prepared delegation IDs independently and returns after durable queue acceptance. It can be explicitly disabled with `CATHULU_ROLE_DISPATCHER_ENABLED=0`; the existing M39 dispatch gates still run before a model turn starts.
- [x] Require a project-specific Main workflow brief before a PM delegation can dispatch; pass the existing brief with the assignment packet to the PM thread without creating a second plan store.
- [x] Queue independent prepared assignments in one short MCP call; the background owner submits each async turn and returns on its start acknowledgement without waiting for any turn to complete. The Main model still has to end its own turn after the dispatch response; app-server completion cannot force-stop an unrelated interactive Main chat.
- [x] Preserve the existing task packet and memory scope in the role turn prompt, require thread-scoped Cathulu memory/report/planning tools before starting the turn, and persist logical role-thread bindings. Model effort and role tool allowlisting are not yet mapped.
- [x] Persist one project-less Main thread binding and hashed global Main capability without duplicating the logical Main role or existing PM/Worker bindings.
- [x] Start the role event cursor at migration-time `MAX(events.id)` so enabling the dispatcher does not replay the historical event ledger.
- [x] Make queue admission idempotent per delegation and move ambiguous in-flight deliveries to `needs_reconcile` instead of replaying them; fixture validation of concurrent ownership/restart remains pending.

**Affected areas:** `src/` MCP/delegation/roles/migrations, app-server adapter, desktop/service lifecycle, dashboard read projection, docs and skill.

**Dependencies:** M36 passes and M39's local gates pass.

**Acceptance criteria**

- [ ] Main receives dispatch acknowledgement while PM turns are still running and has no active wait loop.
- [ ] All independent eligible PM turns are launched without serializing on another PM's completion.
- [ ] Exactly one active logical Main thread binding exists; each project has at most one active PM assignment; Worker cap remains two total per PM assignment.
- [ ] No PM assignment can dispatch without a workflow brief for that same registered project; briefs for independent projects remain independent and do not invent cross-repository edges.
- [ ] A stale MCP/app-server process cannot create duplicate role turns after retry/restart.
- [ ] Thread IDs and raw messages are private local control metadata; dashboard exposes role/status but no secrets or unnecessary thread internals.
- [ ] Local operation remains functional without optional specialist-model providers.

**Validation:** Migration from current schema; idempotency/reconnect scenarios; concurrent dispatch with isolated local fixtures; matched frontend/MCP build; no process auto-open/focus smoke.

**Documentation updates:** Accept/amend ADR-0016, update ADR-0009, architecture, local MCP guide, agent contract, skill, and roadmap.

**Applicable specialized skills:** `milestone-delivery`, `cathulu-orchestrator`, `skill-creator`, `openai-docs`.

**Expected Git checkpoint:** Validated M40 branch/checkpoint before merge.

### M41 — Event routing and selective owner wake-up

**Goal:** Enforce the Main → PM → Worker planning and review chain, route each update to the lowest role able to resolve it, and wake Main only for meaningful review, escalation, owner-action, or project-result events.

**Progress:** Schema v15 records decision origin and the dashboard's two-step Owner approve/decline action remains authorization-only. Schema v17 adds a durable event cursor/inbox; schema v18 adds the single global Main binding and hashed Main capability. Actionable Worker events route to their PM; PM plans/results/escalations route to Main; Main reviews and Owner-decision relays return only to the affected PM. Inbox insertion is idempotent, routine progress is passive, active roles are not double-woken, and a Main-resolved PM action cannot wake Main again. The earlier default-on behavior is superseded by the current M52 runtime hold, which blocks activation until workspace and credential admission is integrated. Live provider/workspace parity, UI thread ownership, and actual restart/concurrency behavior remain unverified.

**Subtasks**

- [x] Route actionable Worker reports and keyed actions to the matching active PM; keep routine progress passive, suppress a PM turn caused by its own report review, and skip a PM that already has an active turn. `cargo test --test role_event_routing --locked --offline` passes against two isolated projects; duplicate delivery produced no second inbox entry and an active PM did not block the other project's message.
- [x] Route PM plans, material decisions, escalations, and project results to the single logical Main role; route Owner decisions back through Main. `role_event_routing` tests verify Main→PM review/relay routing and project isolation.
- [ ] Require each PM to turn the Main workflow brief into scoped Worker task packets before dispatch; validate task scope and acceptance criteria against the brief.
- [ ] Require PM review of every Worker report before the PM can report the corresponding result as accepted; support accept, follow-up/rework, and escalation with evidence references.
- [ ] Require Main review of each PM plan/result against its project brief and supporting evidence; support accept, follow-up, and a qualified/inconclusive result without waiting for other projects.
- [x] Start a new Main turn on routed events, provide compact event context, then let that turn end when handled; live app-server timing remains to be verified.
- [x] Let Main resolve bounded coordination/reversible in-scope decisions using its restricted role capability; escalate unresolved/ambiguous or authority-sensitive actions to the Owner.
- [x] Route Owner decisions through Main immediately and only to the affected PM; never wait for all projects. Deterministic approval gates leave unrelated projects independent.
- [x] Persist owner decisions against the immutable request hash with an explicit `desktop_owner_ui` origin; reject inactive or superseded source actions, prevent duplicate Owner requests, and prevent model tools from recording Owner decisions.
- [x] Provide a two-step local dashboard approve/decline control that records Owner intent only and never executes the requested action.
- [x] Route the recorded Owner decision back through Main to the responsible PM; wake/resume only the affected role chain.
- [ ] Pause only the affected project while awaiting owner input; continue all independent projects.
- [ ] Keep dashboard and floating monitor current from stored events without generating model turns for routine telemetry.
- [ ] Require deterministic QA for success and keep sensitive credentials out of memory, messages, telemetry, and logs.

**Affected areas:** Event/delegation/approval state, message routing, dashboard/compact projection, operating contract, security boundaries.

**Dependencies:** M40.

**Acceptance criteria**

- [ ] A PM blocker or owner-action request reaches Main/Owner promptly without waiting for other PMs or Workers.
- [ ] Worker → PM review and PM → Main review are recorded separately; an unreviewed child report cannot be promoted as verified parent-level success.
- [ ] Main workflow briefs exist before PM dispatch, and PM task plans remain inside those briefs.
- [ ] An owner action is surfaced as soon as Main determines it is necessary; routine choices are resolved by PM/Main inside their authority.
- [ ] Main can return a routine in-scope decision to the relevant PM without involving the Owner.
- [ ] Credentials/accounts, permission changes, spending, external disclosure, destructive/irreversible actions, deployment/publication, and out-of-scope work require explicit Owner authorization.
- [ ] Only an explicit local Owner action can record approval; a stale/mismatched request cannot be approved; approval does not perform the operation.
- [ ] A held approval for one project does not stop another independent project.
- [ ] Repeated event delivery does not duplicate turns or approvals.
- [ ] No specialist model or agent report can bypass approval or QA gates.

**Validation:** Permission/approval matrix scenarios, event-delivery failure/retry scenarios, selective pause/resume scenarios, UI owner-action review, matched build.

**Documentation updates:** ADR-0016, agent operating contract, local MCP guide, architecture, roadmap, and changelog for shipped behavior.

**Applicable specialized skills:** `milestone-delivery`, `openai-docs`, `ui-ux-research` only if dashboard interaction changes materially.

**Expected Git checkpoint:** Validated M41 checkpoint before merge.

### M42 — Multi-project end-to-end verification

**Goal:** Demonstrate the no-wait, event-driven hierarchy with auditable evidence and no cross-project coupling.

**Subtasks**

- [ ] Run independent safe tasks in at least two disposable/local fixture repositories through Main → PM → Worker.
- [ ] Inject a Worker issue that its PM resolves, a PM issue Main resolves, and one owner-authority request while another project continues.
- [ ] Verify the owner-authority request reaches the owner promptly rather than waiting for either project to finish.
- [ ] Verify each Main workflow brief precedes its PM assignment, each PM task plan precedes Worker dispatch, and each hierarchy layer reviews the layer below it.
- [ ] Verify Main is idle between dispatch and routed action, then starts a fresh turn for that action and returns idle again.
- [ ] Restart the dispatcher during a running fixture task and verify recovery without duplicate work or lost messages.
- [ ] Reconcile dashboard role states, message timing, memory transfers, decisions, approvals, Git/QA, and final outcomes.
- [ ] Record remaining real-project limitations; do not manufacture cross-repository work.

**Affected areas:** End-to-end harness, run records, app-server integration, dashboard.

**Dependencies:** M39, M40, and M41.

**Acceptance criteria**

- [ ] All independent PM tasks begin before any PM finishes.
- [ ] Main generates no turns while no actionable event is pending.
- [ ] A project-specific owner action surfaces promptly and pauses only its own workstream.
- [ ] Main reviews each completed PM result independently while other projects remain in progress.
- [ ] Other projects progress and report independently during that pause.
- [ ] Event and approval histories are attributable, idempotent, and inspectable.
- [ ] All mandatory deterministic QA passes before any run is marked verified.

**Validation:** End-to-end evidence report using disposable repositories, restart/retry evidence, Git cleanliness, and matched Windows build; no production deployment.

**Documentation updates:** Close PLAN-015 with exact evidence, update roadmap and relevant ADRs, and record limitations.

**Applicable specialized skills:** `milestone-delivery`, `release-readiness` only if a wider rollout is requested.

**Expected Git checkpoint:** Validated M42 completion; release/push remains governed by existing repo policy and prior owner authorization.

## Final integration validation

- Confirm all role turns use the same documented local Codex configuration and Cathulu memory/evidence tools.
- Validate schema upgrade/rollback, message idempotency, dispatcher single ownership, approval isolation, process recovery, and end-to-end event timing.
- Run the matched frontend/MCP/desktop build without starting, focusing, or restarting Cathulu, Codex, or a terminal window.
- Compare actual runtime evidence to intended behavior. Unknown capability stays unknown; no claim of full automation from docs or a mock alone.

## Rollback / recovery

- Keep current role/delegation/evidence tables authoritative; all new bindings/outbox records are additive and can be disabled without deleting history.
- If app-server becomes unavailable, leave dispatches pending/failed and show the reason; never silently fall back to a parent turn that waits on every project.
- If the adapter is disabled or rolled back, existing manual MCP evidence operations remain available. Do not launch duplicate agents to recover a stale/unknown turn; reconcile thread state first.
- Preserve thread IDs and event history for diagnosis. Never clear a project assignment or report solely because a process disconnected.

## Progress

- [x] Reproduced the design gap from user observation: M35 requires first-result waiting, so Main still stays active after fan-out.
- [x] Read current OpenAI docs: local Codex app-server supports persistent threads, starting/resuming turns, streamed events, and approval flows. Its integration with this user's Codex UI/config remains unverified.
- [x] Confirmed the installed `codex-cli 0.146.1` exposes an experimental `app-server` command with stdio transport. The CLI printed access-denied warnings while attempting temporary argument aliases under the restricted tool sandbox; this is not evidence of an app-server failure, and no user configuration was changed.
- [x] Limited runtime smoke found that a process-only `mcp_servers={}` override did not isolate the user's configured servers; the probe was stopped and did not call tools or change user configuration.
- [x] Establish a fresh scratch `CODEX_HOME` with no MCP inventory and verify hidden app-server initialization, empty-thread lifecycle, and cleanup without starting unrelated configured MCPs.
- [x] Verify the newly built local Cathulu MCP in a separate scratch home/data directory: 46 tools and 3 resources were visible from the Cathulu workspace context; `read_memories` and `cathulu://dashboard` succeeded. The headless flag prevented dashboard startup. A no-repository CWD produced a catalog with zero tools/resources; cause remains unknown.
- [x] Verify saved-turn resume and streamed event delivery against the isolated loopback mock: first turn completed, server exited, a second server resumed the same thread, and a second assignment-only turn completed. This does not validate the user's actual provider/account or Codex UI integration.
- [ ] Verify thread UI ownership, role instructions, model/effort mapping, sandbox/approval behavior, and tool availability from intended project workspaces.
- [x] Implemented the schema-v14 outbox, lease, role-thread binding, no-wait turn submission loop, per-thread Cathulu tool preflight, and dashboard/MCP queue projection. At the matched-build checkpoint, `dispatch_prepared_delegations` was enabled by default and returned after queue acceptance; the current M52 runtime gate disables service startup and queue acceptance regardless of environment flags.
- [x] The dispatcher never approves server requests. Command/file approvals are declined; unsupported interactive requests are rejected, and the dispatch is marked blocked. A user-visible approval/resolve/resume control is still missing, so this is fail-closed plumbing, not complete Owner routing.
- [x] Added schema v15 and a local Owner-action decision surface. Agent MCP tools can create/read pending requests, but cannot record the Owner's decision. The local control uses the displayed request hash, requires a second explicit confirm, permits one approved/declined decision, records `desktop_owner_ui`, and does not execute the requested operation. Main wake-up and PM resume remain unavailable.
- [ ] Validate queue migration/recovery, lease exclusivity, duplicate prevention, and concurrency against isolated fixtures; the source check/build alone does not prove these runtime properties.
- [x] On 2026-10-23, `cargo check --all-targets --locked --offline`, `npm run build`, `cargo fmt --all -- --check`, and `git diff --check` passed; no tests were run. `scripts/build-current.ps1` published the matched pair at source fingerprint `D2C612A8CAB95476735C8DF229272298FE4F322E9250FC8C3A81C8B6EBAE8294`; MCP SHA-256 `D4DA56F89B64542329475BCCE19ED26DED25C6B9167585D5104E24EACD882E71`; desktop SHA-256 `03D0496AEE6429FABFBAA1E80E2ADF05DE6BBCFBC58044517D70C0E7DD494221`. The build manifest records revision `757fd3d5c0325095e40b07963602eb6feccd5624` and a dirty worktree. Neither app nor MCP was launched or restarted. The active MCP has not applied schema v15 until it is safely reloaded.
- [x] Queried Context7; the Codex app-server library lookup was unavailable because the monthly quota was exhausted. Used the current official OpenAI app-server documentation as the source.
- [x] Implemented M39 independently of M36: schema-v13 Main briefs, PM plans, parent reviews, exact Worker task matching, routed role actions, keyed owner approvals, and the dashboard's separate Owner-action state. Updated ADR-0009, ADR-0016, architecture, local MCP guide, operating contract, roadmap, and the installed Orchestrator skill.
- [x] M39 validation passed: `cargo test --all-targets` (73 tests), desktop `npm test` (6 tests), Rust formatting, and the matched Windows build. Follow-up M36 work added two MCP startup-guard tests; the complete Rust suite passed with 75 tests and the updated matched Windows pair was built and published. No user app, MCP server, Codex window, or terminal window was launched or restarted.
- [ ] Complete remaining M36 checks: actual configured provider behavior; intended project-workspace tool/skill access; role instructions, model/effort and approval parity; and normal Codex UI ownership. The local mock confirms saved-turn streaming/resume mechanics only.
- [ ] Complete live verification of the intended project-workspace tool catalog, Codex provider/account behavior, role instruction parity, and practical thread ownership. The dispatcher is enabled for this controlled test path; a role is blocked before model execution if required scoped Cathulu tools are missing.
- [x] M41 first event-routing slice: actionable Worker reports and keyed actions route to only their active project PM; routine progress remains passive; PMs with active turns are skipped while other projects can proceed; and a PM's own Worker-report review does not wake that PM again. The two-project `role_event_routing` fixture verifies idempotency and project isolation.
- [x] On 2026-10-23, `cargo test --test role_event_routing --test role_capabilities --locked --offline` passed (2 tests), and `cargo check --all-targets --locked --offline` passed. The full suite was attempted: several unrelated tests could not access the sandboxed Codex config root, loopback provider fixture, or temporary Git repository; the complete suite is therefore not validated. `cargo fmt --all -- --check` is also blocked by `Access is denied` while rustfmt canonicalizes the restricted Windows workspace path.
- [x] Follow-up validation on 2026-10-23 found and fixed an outdated dashboard test fixture that omitted the new `roleDispatches` snapshot field; `npm run test` now passes all 6 tests. `git diff --check` passes. `scripts/build-current.ps1` published the matched pair at source fingerprint `EEF5D57DCC36143D57F8542E52215ED938F0CCB72B6BD3B4330ECCEE101AB9F1`; MCP SHA-256 `0846A289A35E476A0101BE406D44707A8ADF37DDB87B7053733B7796D42B88BD`; desktop SHA-256 `718B05C2BA9BD6BDDB7F70DC869334B33D9A7DA5A75A81B11897BCA9A878139B`. The matched build is published at the configured stable `target/mcp-current` path. The first sandboxed Tauri packaging attempt could not canonicalize `apps/desktop/dist`; the same build completed with workspace filesystem access. No app, MCP process, Codex window, or terminal window was launched or restarted.
- [x] Read-only checks against the registered real projects reconciled `artificial_inc` (ID 2), `MARX` (ID 5), and `mecanografica-proposals` (ID 4) and confirmed accepted HOT canonical project memories for all three (one, one, and two respectively). The repositories were already dirty; MARX's registered HEAD remains unknown. No project repository files were changed and no role assignments were dispatched. This is status/memory evidence only, not M42 hierarchy validation.
- [x] Main inbox delivery, one global Main thread binding, PM-to-Main routing, Main-to-PM review/decision routing, and Owner-decision return are implemented and covered by role-routing/capability tests. The new schema and dispatcher become live only after the Owner reloads the matched build; then validate provider/workspace access, thread ownership, and actual parallel/restart behavior.
- [x] Codex's 2026-10-24 local log rejected `record_role_work_plan.plan` and `create_workflow_version.definition` as invalid tool schemas while Cathulu MCP itself reported ready. Both fields are JSON objects in Cathulu's deterministic validators; their MCP schemas now declare that object shape explicitly. A tool-router schema check passes. Confirm Codex exposes both tools after loading the published matched build; a locally generated schema alone does not prove the desktop client's acceptance.

## Implementation discoveries / decisions

- Parallel spawn ordering is not enough. The Main generation turn itself must end after dispatch; event arrival must create a new short Main turn.
- Native multi-agent coordination and `wait_threads` are not the target scheduling mechanism because they keep the parent engaged in waiting/monitoring.
- Local Codex app-server is accepted as the role execution transport based on the installed CLI and isolated protocol probes. Actual provider/account behavior, target project workspace tools, and Codex UI thread ownership remain unverified.
- Human approval is a per-action gate. Main is empowered to resolve bounded routine decisions; owner requests are reserved for sensitive, irreversible, external, or out-of-scope actions.
- Main-to-PM and PM-to-Worker plans are separate layers: Main owns project outcomes and guardrails; PM owns the task breakdown. Each parent reviews the child output, while deterministic QA remains the truth source. Owner actions surface when necessary; they are not held for project closeout or a portfolio-wide summary.
- Reserve M37-M38 for PLAN-016 memory capture and onboarding; orchestration implementation now uses M39-M41 to avoid overlapping project milestone identifiers.

## Completion evidence

M39 hierarchy and Owner-approval gates are implemented locally. Owner decisions come only from the local dashboard, are rejected for inactive/superseded source actions, cannot duplicate requests, and do not close the underlying action. M36 verifies hidden scratch startup and saved-turn streaming/resume against an isolated loopback provider; it does not validate the user's provider/account. Six focused role-routing/capability tests and all eight desktop tests pass. `cargo check --all-targets --locked --offline`, Rust formatting, and `git diff --check` pass. A broader library run has four environment failures (two cannot access the sandboxed Codex config root; two cannot reach the loopback provider fixture); the remaining 21 library tests pass. App-server startup invokes the Codex executable directly (no shell wrapper), disables global configured MCPs with process-only overrides, and injects only the scoped Cathulu MCP per role thread. The migration cursor excludes pre-migration history from automatic routing. The matched Windows build is published at fingerprint `49403D5945B920FF8B571B1A58C0A746931AE4EEEC993028F4BDF6808F06081A`, MCP SHA-256 `F41AB97908D9FB5CA67679D029106E7295CB301202DE74B669B042114D0F9C9D`, and desktop SHA-256 `2B7897BCCF08F57F8DC1B48D9FFECE69D9BEA957A7B014455A2D21D6E823D82C`. The active MCP was not restarted. M42 must verify the loaded schema, actual project-workspace tool availability, provider/account behavior, UI thread ownership, real parallel execution, event timing, and restart/recovery before the workflow can be called operational.
