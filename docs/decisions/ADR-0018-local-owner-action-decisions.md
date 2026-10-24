# ADR-0018 — Explicit local Owner-action decisions

- Status: Implemented locally; Rust, frontend, and matched Windows builds pass. Automated tests and live UI/DB behavior remain unverified.
- Date: 2026-10-23
- Decision owner: Cathulu project owner
- Plan: [PLAN-015](../plans/PLAN-015-event-driven-role-orchestration.md)
- Amends: [ADR-0008](ADR-0008-compact-dashboard-and-floating-monitor.md)

## Context

Cathulu can record an explicit request for Owner input, but previously any MCP caller could also record the Owner's answer by calling `record_owner_approval_decision`. MCP does not authenticate the human behind a model turn, so a model could claim that the Owner approved an action. The read-only dashboard showed the request but had no way for the Owner to respond locally.

## Decision

1. Keep `request_owner_approval` available to Main only as a recorded request against an unresolved PM escalation. It never executes the requested operation.
2. Remove the model-callable MCP tool that records Owner decisions. Agent tools may request and read approval state, but cannot write the Owner's answer.
3. Add one narrow action control to the selected Main/PM dossier in the local dashboard. The Owner must choose approve/decline and confirm that same choice in a second step after reviewing the exact action, request summary, and reason.
4. The desktop command accepts only the request ID, the hash from the displayed snapshot, and `approved` or `declined`. SQLite rechecks the immutable request hash and single-decision constraint inside a transaction. A stale request is rejected and must be refreshed.
5. A decision is accepted only while the referenced PM assignment remains active and the exact source action is still the latest unresolved event for its key. One source action may have only one immutable Owner request, preventing duplicate prompts with changed wording.
6. Store `decision_origin = desktop_owner_ui`; pre-existing decisions are retained as `legacy_unverified`. Decision records remain append-only and idempotent.
7. Approval records Owner intent for that exact request only. It does not execute or mark the underlying action complete, approve adjacent work, bypass Codex's own sandbox/permission prompts, or replace deterministic QA. The responsible role must separately report the action outcome; unresolved action state continues to block PM result closure.
8. The floating monitor remains read-only. The dashboard receives no generic database, project, memory, or dispatch controls.
9. The local decision is persisted but does not yet wake Main or resume the responsible PM. The dispatcher stays disabled until authenticated role identity, event delivery, selective wake/resume, and intended workspace tool access are implemented and validated.

## Consequences

- Owner intent has a local, visible, append-only record that models cannot create through MCP.
- An approval decision closes only the request for Owner input; it cannot silently close the underlying project action or unblock PM completion.
- The dashboard is observational except for this single two-step approval/decline path. No execution side effect is attached to the decision.
- Schema v15 adds decision-origin attribution. A stale or modified request cannot be approved using an earlier dashboard snapshot.
- Approval cannot be recorded for a completed Project Manager assignment or a source action that has been resolved or superseded; one source action cannot accumulate duplicate Owner requests.
- During this partial M41 state, the Owner or Overseer may need to relay the recorded decision manually because automatic event routing is not available yet.
- Role reviewer fields remain caller assertions; this decision does not solve model/chat role authentication.

## Validation required

- Verify that the MCP tool catalog no longer exposes an Owner-decision write tool.
- Verify the desktop action shows the exact request and requires a second confirmation.
- Verify stale request hashes, inactive/superseded actions, repeat decisions, invalid decision values, duplicate Owner requests, and duplicate delivery fail safely or return the original identical record.
- Verify schema-v14 to v15 migration preserves prior decisions and labels them `legacy_unverified`.
- Verify the matched Windows app/MCP build without opening or focusing a window or console.
- Verify the action never performs the requested operation and never marks a run as successful.
