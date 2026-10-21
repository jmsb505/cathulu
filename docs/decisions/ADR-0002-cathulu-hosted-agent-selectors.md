# ADR-0002: Cathulu hosts future agent model and effort selectors

Date: 2026-10-20; feasibility checked 2026-10-21
Status: ACCEPTED; implementation deferred
Scope: Future agent configuration surface
Supersedes / superseded by: Clarifies the interaction surface in ADR-0001 without changing Codex's execution ownership

## Context

The owner intends to choose the Main Overseer and sub-agent models and effort levels through selectors. The project brief keeps conversation and agent execution in Codex, while the first Cathulu dashboard is explicitly read-only. The owner clarified that Cathulu should host the future selectors.

## Decision

- Cathulu will provide model and effort selectors in a later milestone.
- These selectors will live in a dedicated configuration surface, separate from the read-only portfolio dashboard.
- Codex continues to own conversation, model/tool execution, and native sub-agent runs. Cathulu does not become an agent runtime.
- M1 adds no selector controls.

## Open integration questions

- Which approved configuration layer should Cathulu manage or export for the owner's active Codex setup?
- How should the selector stay aligned with the models and effort values available to the signed-in account or workspace?
- What confirmation, persistence, and audit behavior should apply when selections change?

The configuration pathway and role-level support are confirmed in current official documentation. The remaining questions concern how Cathulu should present, preview, and apply settings in the owner's installation. Do not silently write user-level Codex configuration or claim a selection took effect until the owner explicitly applies it and the resulting Codex configuration is verified.

## Consequences

- Selector ownership is decided: Cathulu will host the future UI.
- The implementation and runtime integration remain unverified and are not part of M1 acceptance.
- The dashboard stays read-only; future selector settings use a separate configuration boundary.
- Codex exposes main-session model and reasoning-effort configuration through its settings/configuration layers and one-run CLI overrides. Spawned agents support global defaults, explicit spawn overrides, and custom role files with their own `model` and `model_reasoning_effort` values.
- These documented capabilities establish that per-role choices are representable. They do not provide Cathulu a documented API for changing the currently running Codex turn. Cathulu must label stored selections as requested/configured until the user applies the supported configuration and effective runtime values can be verified.

## Evidence

- The owner's initial project note requested future model/effort selectors for the orchestrator and sub-agents.
- The owner clarified: “Cathulu selectors later.”
- The project kickoff brief and ADR-0001 continue to define Codex as the execution surface and the dashboard as read-only.
- Official OpenAI documentation, checked 2026-10-21: [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference), [Codex subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents), and [Codex developer settings](https://learn.chatgpt.com/docs/developer-settings).

## Reconsider if

The target Codex runtime removes support for the documented configuration fields, or the user's environment cannot apply the approved configuration layer. Revisit the user experience with the owner before replacing the selectors with another workflow.
