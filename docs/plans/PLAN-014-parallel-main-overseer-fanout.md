# PLAN-014 — Parallel Main Overseer fan-out

Status: Superseded for runtime behavior by [PLAN-015](PLAN-015-event-driven-role-orchestration.md); M35's instruction-only fan-out correction was insufficient

Created: 2026-10-22

Last updated: 2026-10-22

## Objective

Make parallel dispatch of independent Project Managers the default Main Overseer behavior, so all eligible projects begin work before Main waits for any one project's result.

## Motivation

The owner observed the Main Overseer coordinating projects serially. M35 clarified parallel PM fan-out, but a first-result wait still keeps Main generating and violates the required idle/event-driven behavior. This plan records the instruction-level correction; PLAN-015 defines the necessary runtime capability.

Cathulu MCP is an evidence/control plane; it does not spawn native Codex agents. Parallel fan-out must be required by the Codex Main Overseer skill and the normative operating contract. No new MCP spawn API is implied.

## Current state

- The Main role may dispatch at most one Project Manager per project.
- The previous contract/skill said to report the first completion/attention event and then continue monitoring.
- The contract/skill now require all eligible PM dispatch attempts before monitoring and prohibit Main waiting/polling, but current Cathulu cannot dispatch/resume idle Codex role threads.
- M21 verified one PM → one Worker path, but did not verify parallel Main → multiple PM dispatch.

## Desired state

- For two or more independent active projects, Main prepares each eligible project's run, memory packet, and Cathulu delegation without waiting for any agent.
- Main dispatches all eligible PMs concurrently in a single fan-out wave before beginning result monitoring. If the client has no batch-spawn API, it submits all spawn operations without waiting on agent progress between them, then monitors after all dispatch attempts return.
- A setup or dispatch failure blocks only its project. Main continues dispatching every other eligible project.
- After fan-out, Main returns idle. The dispatcher stores routine progress and starts a fresh short Main turn only when a meaningful decision, escalation, owner action, or project result requires it; this requires PLAN-015 and is not implemented here.
- Each PM may begin its own scoped Worker dispatch as soon as its memory/tool preflight is complete; it does not wait for unrelated projects.
- Single-project work retains the existing flow. Project independence, one PM per project, two Workers per PM assignment, memory requirements, and evidence rules remain unchanged.

## Scope

- Update the installed `cathulu-orchestrator` skill.
- Update `docs/agent-operating-contract.md`, ADR-0009, `docs/architecture.md`, `docs/roadmap.md`, and this plan.
- Validate that wording distinguishes native Codex spawning from Cathulu MCP evidence recording.

## Out of scope

- Adding an MCP API that spawns or controls Codex agents.
- Changing project/worker capacity limits, persistent role identity, memory schema, or quality gates.
- Starting a real project task or changing project repositories.
- Claiming parallel dispatch is runtime-verified without a multi-project native Codex run.

## Source-of-truth impact

- ADR-0009 defines the parallel dispatch decision within the existing bounded hierarchy.
- `docs/agent-operating-contract.md` remains the repository's normative behavioral contract.
- The installed `cathulu-orchestrator` skill must mirror this invariant so a new Main chat receives it automatically.
- Cathulu's database remains the record of runs, delegations, transfers, reports, and QA; native Codex remains the spawning mechanism.

## Existing decisions and constraints

- ADR-0001 and ADR-0009 keep Codex responsible for spawning and Cathulu responsible for local evidence/control.
- Each project is independent unless the owner explicitly requests a cross-project task.
- Main coordinates only. Project Managers coordinate one project each; Workers perform bounded repository work.
- The active desktop/MCP may not be opened or restarted during this update.

## Supporting skills / tools

- `cathulu-orchestrator`, `skill-creator`, `implementation-plan`, and `milestone-delivery`.
- Existing Codex native agent spawning, Cathulu MCP resources/tools, and `scripts/build-current.ps1`.
- No new dependency or runtime code is needed for the instruction-level correction.

## Dependencies

- Access to the existing installed `cathulu-orchestrator` skill.
- The next real multi-project orchestration provides runtime evidence of fan-out ordering.

## Risks and unknowns

- Codex client surfaces may expose different spawn APIs. The contract must require no waiting on child progress before all eligible PM dispatch attempts, without asserting a nonexistent MCP spawn endpoint.
- Per-project run/delegation setup may fail independently. The Main must isolate that failure rather than serialize or cancel unrelated projects.
- A runtime parallel fan-out has not yet been measured in the target chat/client; source-level instruction validation cannot prove actual scheduling.

## Milestones

### M35 — Parallel Main Overseer fan-out

**Goal:** Make a no-wait parallel PM dispatch barrier explicit in Cathulu's skill and normative operating instructions.

**Subtasks**

- [x] Add a clear prepare-all → dispatch-all → monitor-first-result sequence to the installed skill.
- [x] Make per-project setup failures independent and keep unrelated dispatches moving.
- [x] Align ADR-0009, the agent operating contract, architecture, and roadmap.
- [x] Preserve boundaries: Main does not implement, PMs own one project, and Cathulu MCP records but does not spawn agents.

**Affected areas:** User-installed `cathulu-orchestrator/SKILL.md`, `docs/agent-operating-contract.md`, ADR-0009, `docs/architecture.md`, `docs/roadmap.md`, and this plan.

**Dependencies:** Existing native Codex agent spawning and Cathulu evidence tools.

**Acceptance criteria**

- [x] Skill and normative instructions require all eligible PMs to be dispatched before Main waits for any PM result.
- [x] Instructions isolate project setup/spawn failures so unrelated PM dispatches continue.
- [x] Main sends early per-project updates after fan-out and does not use an all-project completion barrier.
- [x] PM → Worker hierarchy and scoped memory/MCP requirements remain intact.
- [x] No docs or skill text claims that Cathulu MCP itself spawns Codex agents.
- [ ] A live multi-project run demonstrates concurrent PM dispatch; M21 only tested one PM → Worker chain.

M35's monitor-first-result instruction is insufficient for the owner's no-wait requirement. PLAN-015 supersedes this plan as the implementation path for event-driven role work; do not treat this document's instruction-only correction as proof that Main returns idle.

**Validation**

- [x] Validate the installed skill structure with the skill creator validator (`quick_validate.py`; UTF-8 mode).
- [x] Review contract/ADR/architecture wording against M21 evidence and the actual MCP capability boundary.
- [x] `git diff --check`.
- [x] Confirm the matched current app/MCP pair per the standing project instruction; the current source fingerprint was already published, so no duplicate build was needed. Neither process was launched.
- [ ] Verify the dispatch ordering in the next real multi-project orchestration. Until then, runtime behavior remains unverified.

**Documentation updates:** Update ADR-0009, the agent operating contract, architecture, roadmap, and this plan.

**Applicable specialized skills:** `cathulu-orchestrator`, `skill-creator`, `implementation-plan`, `milestone-delivery`.

**Expected Git checkpoint:** Stack the validated instruction/skill change on the current Cathulu working tree; no commit or push is part of this plan.

## Final integration validation

- Validate the skill and cross-document consistency.
- Confirm `git diff --check` passes.
- Run the matched local build without starting or restarting the app/MCP.
- Clearly separate instruction-level completion from pending live multi-project runtime verification.

## Rollback / recovery

Revert the narrow skill and documentation changes if they conflict with native Codex capabilities. No database, project repository, app state, or service state is changed.

## Progress

- [x] Traced serial behavior to a missing dispatch-order invariant: the contract specifies limits and partial updates, but not parallel fan-out before waiting.
- [x] Update the installed skill and repository source-of-truth instructions.
- [x] Validate documents and skill.
- [x] Confirm the matched pair is published for the current app source.
- [ ] Verify at runtime during the next requested multi-project orchestration.

## Implementation discoveries / decisions

- The agent operating contract already uses first-completion monitoring after multiple PM chats exist; the missing behavior is creating those PM chats concurrently before entering that wait.
- Fix the orchestration behavior in Codex instructions, not by adding a Cathulu MCP spawn method that the architecture explicitly excludes.

## Completion evidence

The installed `cathulu-orchestrator` skill now mandates parallel PM fan-out and explicitly prohibits a spawn/wait/spawn loop. The normative operating contract, ADR-0009, architecture, and roadmap match this requirement and state that native Codex performs spawning while Cathulu MCP records evidence. Skill validation passed with `quick_validate.py` in UTF-8 mode; `git diff --check` passed. `scripts/build-current.ps1` confirmed the current matched app/MCP pair is already published, so it created no duplicate; neither app nor MCP was launched or restarted. Multi-PM concurrency has not yet been exercised in a live multi-project run, so runtime behavior remains unverified.
