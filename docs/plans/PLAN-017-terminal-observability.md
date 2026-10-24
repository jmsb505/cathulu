# PLAN-017 — Terminal-first Cathulu observability

Status: M43 validated and published locally; M44 planned.

Created: 2026-10-24

## Objective

Pause desktop UI development and make Cathulu's actual multi-project state readable from an existing terminal while hardening MCP and role event truth. The monitor is an observation client; SQLite remains authoritative and Codex remains the execution harness.

## M43 — Read-only terminal monitor

Deliver a separate executable in the matched release directory with a concise portfolio view, refresh-in-place watch mode, and full JSON snapshot for investigation. It must not spawn a console or mutate the store. Show one logical Main, every registered project, recorded assignment/dispatch and latest update, attention, QA failures, and memory usage. Never infer that a role is running merely because a record exists.

Acceptance: the command reads an existing store without writes; missing store fails clearly; `--once` and `--json` exit; `--watch` requires a terminal; a 60-column rendering remains legible; the published release has a hash-verified monitor beside MCP and desktop. Validate with focused renderer/read-only tests, Rust format/check, a local snapshot smoke, and the matched Windows build.

Validation (2026-10-24): focused terminal rendering and SQLite read-only tests passed; `cargo check --all-targets --locked --offline` and `git diff --check` passed. The published `--once` read five registered projects and correctly classified historical dispatched delegations by their parent run outcomes rather than calling them active. The published `--json` parsed with five projects, six logical roles, and seven canonical memories. The canonical Windows build published a matched trio at source fingerprint `9E424F9E92F34F52893FBCD6`; monitor SHA-256 is `2FC8695EEE1B1C97A40ACF3965F6E22FDBF5C57936CFCA14128F0FD99C5B8941`. The live Cathulu MCP still exposes projects, roles, dashboard, and dispatches resources, but no actual role turn was started for this validation. `--watch` has only been checked as an interactive-terminal code path; visual live refresh remains for M44.

The live MCP `cathulu://dispatches` resource returned an empty queue. Its full `cathulu://dashboard` payload exceeded the connected tool's text allowance and was truncated in this chat, so it is unsuitable as the default compact agent status read. M44 should provide or validate a bounded status projection for MCP clients in addition to the terminal monitor; the full dashboard remains available for detailed inspection where the client can receive it.

## M44 — Live event and MCP truth check

Under [ADR-0021](../decisions/ADR-0021-continuous-supervised-development.md), Cathulu development agents arrange and record these already authorized external-project runs. Develop and repair Cathulu itself in a separate Codex session. A supervised bounded assignment can establish live capability before M44 is complete; normal task/memory/role/preflight/QA gates apply. Do not require the Owner to generate a cohort or prove dispatch before permitting its own safe validation attempt. Keep source, build, loaded process, live acceptance, and benefit evidence distinct. An actual scope/access/runtime failure pauses its affected path; an empty queue only shows no queued work. Main ends its dispatch turn and handles resulting evidence through routed events rather than polling. Completion still requires the acceptance evidence below.

With explicitly scoped real project work, compare Codex app-server turn/item events, Cathulu dispatch/inbox state, MCP tool/resource availability, and the terminal output. Verify parallel PM dispatch, individual escalation delivery, recovery after restart, and stale-state handling. Add only the missing normalized events or status transitions; keep raw transcripts and sensitive tool output out of memory and terminal views. If provider/account or workspace tool access fails, show a concrete unavailable state rather than pretending work continues.

Acceptance: a controlled run shows each role's actual lifecycle and actionable Owner requests promptly; event IDs and recorded state reconcile after process restart; no duplicate role turns or visible terminal windows appear. Evidence must distinguish synthetic protocol checks from real provider activity. M44 is not complete based on M43 tests.

## Sources and constraints

- [Claude Code status line](https://code.claude.com/docs/en/statusline) uses structured session JSON and event/timer refreshes in the current terminal.
- [Codex app-server events](https://learn.chatgpt.com/docs/app-server) supply thread, turn, and item notifications; Cathulu should normalize only the needed fields.
- Preserve ADR-0016 role hierarchy and ADR-0019 presentation boundary. The desktop UX track is on hold, not removed.
