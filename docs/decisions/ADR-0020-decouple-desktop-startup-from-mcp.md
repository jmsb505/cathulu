# ADR-0020 — Decouple desktop startup from MCP

- **Status:** Accepted
- **Date:** 2026-10-24
- **Supersedes:** ADR-0010's automatic desktop launch behavior

## Decision

Starting Cathulu MCP starts only the stdio MCP service and its existing background role dispatcher. It never discovers or launches the desktop executable, opens a terminal, or changes window focus. The Owner may start the matched desktop executable manually when needed. The terminal monitor is likewise user-invoked. The release script still builds and publishes all three matching binaries, so manual desktop use reads the same schema as MCP.

The old `CATHULU_DISABLE_DASHBOARD_AUTOLAUNCH` override is removed because there is no launch path to disable. Role child MCP processes inherit the same no-window startup behavior. MCP stdout remains protocol-only; errors go to stderr.

## Reason

Codex can start multiple MCP processes for different chat contexts. Coupling each process to desktop startup caused visible repeated windows and interrupted the Owner. A persistent MCP service and an optional observation client have different lifecycles.

## Validation

Build the matched release, run a scratch MCP protocol smoke with dashboard absent, and inspect the process tree for new Cathulu desktop or terminal windows. The currently running Codex MCP process retains its old executable image until Cathulu MCP is reloaded.
