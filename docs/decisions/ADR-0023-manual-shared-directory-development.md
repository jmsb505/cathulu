# ADR-0023 — Manual shared-directory Claude Code / Codex development

- Status: Accepted workflow direction from the Owner's 2026-10-25 request for a manually run parallel plan, followed by the explicit requirement that both agents use the same local directory. Detailed packet schedule: proposed in [PLAN-021](../plans/PLAN-021-manual-claude-codex-development.md).
- Scope: external development of Cathulu by the Owner's two manually operated coding sessions.
- Partially supersedes: ADR-0021's Codex-only self-development session wording and isolation guidance only for this explicit manual same-directory collaboration. It preserves developing Cathulu outside its own dispatcher, continuous supervised development and all product/runtime evidence and permission gates.

## Decision

The Owner may run Claude Code and Codex in `C:\Users\sjuan\Desktop\cathulu` on one shared branch. Work is divided by file and capability ownership. Codex is the designated shared core/schema/Git integrator; the Owner can perform Git operations instead. Claude owns its scoped design/review outputs and bounded adapter/routing-policy implementations. Both can code; assigned strengths are provisional task/project preferences and grant no extra authority.

Before launch, commit/integrate the planning files and verify a clean observed HEAD; the original `d2422fb` inspection is not the later launch baseline. Fresh sessions obtain context from committed source and evidence, not from assumed prior conversation familiarity.

Waves 0-3 are ordinary implement-then-review with one production-code writer and no per-packet freeze ceremony. In the actual concurrent-coding stretch (waves 4-6), briefly pause both writers for shared integration, combined acceptance and combined commits. Use acknowledgements plus Git status/diff, including separate inspection of untracked files; no per-file hash manifests. Neither agent may switch the shared branch, clear/stash/reset another agent's work or accept results from a changing relevant source set. There is no automatic coordination or worker fleet.

Claude owns a throwaway standalone scratch crate with its own manifest, lockfile and build output, depending on the real public Cathulu core by path. It compiles/tests the actual adapter/policy source before production registration; shared production manifests/schema remain Codex-owned. The scratch crate is independent of Cargo workspace membership, but its path dependency still reads the live shared core. It is not source isolation or proof of future F1/adapter correctness.

This narrow development exception does not satisfy, remove or weaken PLAN-020 M52's future exclusive writable workspaces for Cathulu-dispatched product agents. Nor does a Claude development session establish the product's Claude adapter, authentication, data disclosure or permission parity. Main/PM/Worker limits and reviews for Cathulu-managed project execution remain unchanged. Desktop implementation remains paused.

## Consequences and validation

Real concurrent coding starts after F1 in waves 4-6; the earlier design/review work is useful overlap, not two simultaneous implementation lanes. PLAN-021 defines ownership, interface agreement, short handoffs and recovery. If two tasks need the same core file, serialize that part. If shared-checkout coordination becomes costly, the Owner may choose one Claude worktree for waves 4-6 while retaining Codex in the original directory. This is a documented fallback, not permission to create one automatically; no worktree is created by this planning change.

The planning change is validated with document/coverage/consistency checks. Future implementations retain PLAN-020 acceptance, meaningful tests, data integrity, source-backed memory, actual-provider gates and applicable Git/release authority. No development session is launched and no runtime code changes through this decision.
