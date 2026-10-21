# ADR-0005 — Codex worker correlation and worktree verification

Status: Accepted for Phase 3 M8 implementation
Date: 2026-10-21

## Context

M8 connects a reported worker result to Git worktree evidence and to native Codex lifecycle events. The connection must be strong enough to audit without making Cathulu a worker runtime or storing transcripts.

Current official OpenAI Hooks documentation defines `SubagentStart` and `SubagentStop` fields `agent_id` and `agent_type`, plus common `session_id`, `turn_id`, and `model` fields. For subagents, `session_id` identifies the parent session. The documented `model` value is the active model slug; no reasoning-effort field is documented. The transcript path is explicitly not a stable hook interface, and the latest assistant message may contain arbitrary conversation content. See the [Hooks guide](https://learn.chatgpt.com/docs/hooks).

Git exposes linked worktrees through machine-readable `git worktree list --porcelain`; `git rev-parse --show-toplevel` identifies a worktree root, and `git rev-parse --git-common-dir` identifies the shared repository metadata. A valid worker worktree can live outside the main checkout, so filesystem ancestry alone is not a sufficient identity check. See [git-worktree](https://git-scm.com/docs/git-worktree), [git-rev-parse](https://git-scm.com/docs/git-rev-parse), and [git-merge-base](https://git-scm.com/docs/git-merge-base).

## Decision

- A worktree can be reconciled only when its canonical root appears in `git worktree list --porcelain` for the registered repository, is distinct from the registered main worktree, and resolves to the same canonical Git common directory. Missing, escaped, malformed, and foreign-repository paths fail before any Cathulu state changes.
- Capture the registered project HEAD as a delegation's baseline when preparing the delegation. Existing delegations without a captured baseline remain baseline-unknown; migration must not guess from today's repository state.
- Store the latest Git-derived worktree snapshot as a state projection and append a delegation event for successful reconciliation. Verify worker-reported commits as commits reachable from the observed HEAD and changed paths as present in the baseline-to-worktree change set. Record mismatches as evidence; do not turn them into accepted or verified outcomes.
- Do not create, move, remove, or merge worktrees. The owner and native Codex remain responsible for worker handoff and recovery.
- Correlate worker telemetry only through an explicit association to a recorded `SubagentStart` event whose run matches the delegation's parent run. Subsequent events match the same hashed agent/session identifiers. Store no raw session or agent identifier, transcript path, prompt, tool body, or last assistant message.
- Keep requested model/effort separate from observed worker values. Use the documented active model slug as the observed effective model when present; leave effective effort unknown because the hook contract does not expose it.

## Consequences

The M8 MCP boundary adds read-only worktree reconciliation and explicit worker-event association operations. Correlation is intentionally unavailable when no matching run and start-event reference exists. The dashboard may show unknown model/effort or unverified report claims; it must not infer missing values. SQLite remains authoritative, and all worktree paths, commit claims, and hook metadata remain local.

## Validation

Disposable Git fixtures must cover valid linked worktrees, dirty state, additional commits, missing and unregistered paths, escaped paths, and foreign repositories. Hook tests must cover explicit correlation, duplicate delivery, privacy minimization, and unsupported fields. The existing parent mandatory-check gate remains authoritative.
