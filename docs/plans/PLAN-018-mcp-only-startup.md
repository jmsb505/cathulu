# PLAN-018 — MCP-only startup

Status: implementation built and scratch-smoked; live Codex reload remains pending.

Created: 2026-10-24

## M45 — Separate service and observer lifecycles

Remove dashboard launch, named Windows launch mutexes, and the headless override from the MCP executable. Remove obsolete child-process override configuration. Keep desktop and terminal monitor as manually invoked observation clients. Update ADR-0010, local setup, architecture, and roadmap to state the new behavior.

Acceptance: a scratch MCP process completes `initialize` and resource discovery while no desktop is running; no desktop or extra console process is created; the current Windows release publishes hash-matched MCP, desktop, and monitor binaries. Do not restart the Owner's active Codex or MCP processes during validation.

Validation (2026-10-24): the scratch MCP initialized and returned all four resources, exited cleanly, and the `cathulu-desktop` process count stayed at zero. The scratch database was isolated under `%TEMP%`, with the dispatcher disabled. `scripts/build-current.ps1` published matched binaries at fingerprint `91FC6B2D185EE9FAD52D9A03`; MCP SHA-256 `4DB5C980813FF63B88A7AC7765F4E5B03FFD628D317A8317EEE0D275C4517CD3`, desktop SHA-256 `2E967B878987BC285424548D529CF5633BBE74DAA3F442D165A7E1F4AF8A60C9`, and monitor SHA-256 `BB7950C9C22FC501236FC9DC8B5FA26B8CC8B28ACA7B481FFF43EE6A0B7B62BB`. The build succeeded; the active Codex MCP was not restarted. After reload, confirm the MCP remains available and the dashboard stays closed.

The live Codex startup check occurs after the Owner reloads Cathulu MCP. A build alone cannot change an already running MCP process.
