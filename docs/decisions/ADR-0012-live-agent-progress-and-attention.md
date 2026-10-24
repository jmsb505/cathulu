# ADR-0012 — Live agent progress and owner-action visibility

Status: Accepted for M28 implementation
Date: 2026-10-22

## Context

The existing one-graph dashboard projects delegation lifecycle and optional Codex hook observations, but agents have no narrow operation for reporting a useful intermediate checkpoint or requesting owner input. The Main Overseer contract can therefore leave the owner waiting for all independent projects to finish before communicating progress.

## Decision

1. Keep Codex responsible for spawning and running agents. Cathulu records evidence and never controls agent execution.
2. Record concise progress, waiting, action-required, and action-resolved updates as append-only events on the existing delegation aggregate. Updates use bounded text, a controlled update kind/stage, and idempotency keys. A reused key with changed content is invalid.
3. An owner-action request uses a stable action key and remains open until a matching resolution event. Existing blocked tasks and failed mandatory checks remain visible in the same attention surface.
4. Treat agent updates as self-reported checkpoints. They do not establish that code changed, a command ran, a test passed, or a task succeeded. Hook events, Git reconciliation, artifacts, and deterministic QA remain separately labeled sources.
5. Project the latest updates and pending actions through the read-only dashboard snapshot. Keep one graph and the compact floating monitor; do not launch or focus a window and do not add OS notifications in this milestone.
6. Main Overseer forwards each meaningful update or owner action as soon as it arrives, and continues coordinating unrelated projects. It must not wait at an all-agents-finished barrier to give the owner useful partial results.
7. Treat working/waiting updates older than 15 minutes as stale and unconfirmed in the UI. This presentation threshold is not evidence that an agent stopped; sustained work should emit a concise checkpoint about every 10 minutes.

## Consequences

- The current SQLite event table is sufficient; no migration is needed unless an implementation constraint is discovered.
- A dashboard action item may be incomplete or stale if the responsible agent does not report or resolve it. The UI must show its source and age rather than infer completion. Stale agent signals use a separate visual state and never claim that work is verified stopped.
- Existing evidence and run status retain authority over agent-reported summaries.

## Alternatives considered

- **Infer detailed progress from Codex tool hooks:** rejected because hooks are optional, privacy-minimized, and do not reliably describe intent or user actions.
- **Capture transcripts or every tool call:** rejected for privacy, context/storage growth, and signal-to-noise reasons.
- **Add a separate dashboard/activity tab:** rejected; the owner requires one project-spanning graph and a compact support surface.
- **Wait until every PM/Worker completes before reporting:** rejected because independent project work can proceed while one task is blocked or waiting for owner input.

## Scheduling amendment — 2026-10-22

M28 added durable progress and owner-action evidence, but its first-update-then-monitor instruction still keeps Main's generation active. The event ledger and dashboard projection remain valid; the scheduling behavior is insufficient for the owner's no-wait requirement. PLAN-015 supersedes it with passive routine progress and fresh short Main turns only for actionable events or material results. The current MCP cannot yet provide that dispatcher or wake-up behavior.
