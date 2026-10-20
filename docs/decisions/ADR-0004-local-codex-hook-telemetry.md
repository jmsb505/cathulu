# ADR-0004 — Local Codex hook telemetry

- Status: Accepted for M3
- Date: 2026-10-21
- Decision owners: Cathulu project owner

## Context

Phase 1 needs local evidence that associates Codex activity with project runs while keeping prompts, transcripts, tool arguments, and tool outputs out of Cathulu's durable store. Codex lifecycle hooks expose event and identifier fields, but their payloads can also include sensitive prompt and tool content. OpenTelemetry is opt-in and its documented tool-result event can contain an output snippet.

## Decision

Use Codex MCP tool hooks to call the existing `record_codex_hook_event` operation. Support only `UserPromptSubmit`, `PostToolUse`, `Stop`, and `Interrupt`. The hook configuration passes a fixed allowlist of event name, session ID, turn ID, current directory, and—only after a tool call—tool name and tool-use ID. It never passes prompt text, transcript paths, tool input, or tool response.

Cathulu maps a registered Git working directory to its canonical project root and stores no directory string from the hook. Session, turn, tool-use, and source-event identifiers are SHA-256 hashed before persistence. Tool names are reduced to the categories `shell`, `edit`, `mcp`, or `other`. An external event ID makes repeated delivery idempotent. A Codex session/turn pair maps to one existing Cathulu run record; the run remains unverified and becomes `inconclusive` on `Stop` or `Interrupt` unless a separate Cathulu quality workflow has already established another terminal state.

The existing context-transfer table remains the only transfer ledger. `prepare_context_transfer` records Cathulu-controlled transfers explicitly; hooks do not infer prompt contents, token counts, or downstream use.

OpenTelemetry export is not enabled by Cathulu. Its prompt redaction and exporter configuration are useful for later evaluation, but tool-result snippets are not needed for the first local evidence path. Reconsider OTel only when a concrete metric needs it and its field-level privacy boundary can be demonstrated.

Hook configuration remains owner-controlled. The repository documents an optional snippet but does not edit global Codex configuration. Hooks must be reviewed and trusted in Codex. The local MCP server must already be connected; a missing server or unsupported event yields no telemetry and must not interrupt persistence or normal Codex work.

On desktop and MCP process startup, Cathulu best-effort reconciles every registered Git repository. A failure for one missing/unavailable repository is reported to stderr and does not prevent reconciliation of other projects or startup. Its last known snapshot remains available for inspection.

## Consequences

- SQLite v3 adds an optional unique source-event ID to the existing append-only event journal and a hash-only Codex session/turn-to-run mapping.
- Run history can include an observed Codex turn without claiming that its work passed QA.
- Hook metadata is evidence of reported lifecycle events, not an authorization boundary or proof of successful tool output.
- Context transfers, quality gates, and verified success continue to use their existing semantic operations.
- No prompt, transcript, tool input/output, or raw Codex identifier is required for telemetry deduplication or run association.

## References

- [Codex Hooks](https://learn.chatgpt.com/docs/hooks)
- [Codex agent approvals, security, and monitoring](https://learn.chatgpt.com/docs/agent-approvals-security#monitoring-and-telemetry)
