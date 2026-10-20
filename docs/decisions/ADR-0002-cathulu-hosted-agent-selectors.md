# ADR-0002: Cathulu hosts future agent model and effort selectors

Date: 2026-10-20
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

- Which supported Codex interface can receive selected model and effort values?
- Can the Main Overseer and each sub-agent role use distinct values?
- Which model and effort options can Cathulu present without duplicating or drifting from Codex's supported choices?
- What confirmation, persistence, and audit behavior should apply when selections change?

Resolve these questions against the target Codex runtime before implementing the selector surface. Do not silently write user-level Codex configuration or claim a selection took effect until the supported path is verified.

## Consequences

- Selector ownership is decided: Cathulu will host the future UI.
- The implementation and runtime integration remain unverified and are not part of M1 acceptance.
- The dashboard stays read-only; future selector settings use a separate configuration boundary.

## Evidence

- The owner's initial project note requested future model/effort selectors for the orchestrator and sub-agents.
- The owner clarified: “Cathulu selectors later.”
- The project kickoff brief and ADR-0001 continue to define Codex as the execution surface and the dashboard as read-only.

## Reconsider if

The target Codex runtime cannot accept the selected values through a supported interface, or separate per-role configuration is not available. Revisit the user experience with the owner before replacing the selectors with another workflow.
