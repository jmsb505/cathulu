# ADR-0019 — Terminal-first observability

Status: accepted for the current development track, 2026-10-24.

## Context

The desktop dashboard has not yet given the Owner a clear, reliable account of parallel project work. Its UI development is paused while role dispatch, MCP access, recovery, and recorded evidence are validated. Automatic terminal windows previously interrupted Codex and are unacceptable.

Claude Code's documented status line receives structured session JSON and renders a bounded row in its existing terminal. Codex's documented app-server emits thread, turn, item, and plan notifications. Both patterns separate the event source from its terminal presentation. Cathulu already has a SQLite event journal and a dashboard projection; it should reuse that evidence rather than parse or copy transcripts.

## Decision

- Ship a separate, user-invoked `cathulu-monitor` executable beside the matched MCP and desktop binaries. Its first version provides `--once`, `--watch`, and `--json` modes.
- Read the existing SQLite dashboard projection through an explicitly read-only connection. Do not create or migrate a database, start Codex, run Git, or write Cathulu state from this process.
- Present one logical Main role, per-project recorded assignments and latest updates, owner attention, QA failures, and memory temperature/class counts. Label the output as recorded state; a stored dispatch is not proof of live agent execution.
- Keep MCP stdio exclusively for JSON-RPC. Never launch a terminal window from MCP or Codex startup. Watch mode refreshes only in the terminal the Owner explicitly opened.
- Keep the existing desktop app build intact while its UX track is paused. This decision does not claim that the app-server's real turn stream is fully reflected in Cathulu yet.

## Follow-up

M44 will verify actual Codex role events against a controlled project task, reconcile stale dispatch states, and add richer event details only where authoritative data exists. Owner-action delivery and agent activity must be measured during live MCP operation before claiming real-time coverage.
