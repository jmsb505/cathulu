# Manual Codex starter prompt

Copy this into the Codex development session you start in `C:\Users\sjuan\Desktop\cathulu` after the revised planning checkpoint is committed and the shared launch HEAD is verified clean. This file does not start a session.

```text
You are the Codex shared-core/integration lane for PLAN-021. Claude Code uses
this same checkout and branch. Read AGENTS.md, ADR-0023, PLAN-020 and PLAN-021.
Record the observed committed launch HEAD/branch and verify all planning files
are tracked. Before either session starts writing, the Owner verifies clean
Git status including untracked files. If that has not happened, report the
pending work and do not begin implementation. d2422fb is only the old source
inspection, not the launch baseline; never reset to it.

Start CX-00, then CX-01/M48 only after preflight. Both sessions are fresh unless
actually continuing their own history: read committed source and PR #33's
durable evidence instead of assuming retained conversation familiarity.

Waves 0-3 use normal implementation, review, applicable tests and commit.
Claude's separate design notes/read-only review require no two-agent freeze or
hash record. You own shared production code/schema/manifests/Git integration;
Claude owns its design paths and later the allocated adapter/policy files.
Stage only complete, reviewed files; finish any document you include before
staging it. Never switch/pull/rebase or discard work underneath an active writer.

Implement M48's deterministic metadata, dependencies/blockers/eligibility,
scoped Store/MCP boundaries and migration/permission/no-provider tests using
existing entities and QA. Do not expand into M49-M55 in this first packet.

At F1 export the agreed minimal public contract. Claude may compile its own
adapter/policy via a standalone scratch crate with a path dependency on the
real core and its own manifest/lock/target output. It must hand off compile/test
results before you register production modules/tests. Do not require empty
passing adapter stubs or duplicate contract types to enable its iteration.

Only during actual concurrent coding in waves 4-6, have the Owner relay
PAUSE WRITES and obtain both acknowledgements before shared integration,
combined acceptance checks or a combined commit. Inspect git status including
untracked files and git diff HEAD, read new candidates separately, run checks,
then recheck the Git view. No hash manifests. If relevant input changes, rerun
affected checks. One Owner-selected Claude worktree is an optional fallback
for those waves; do not create one automatically or automate coordination.

Use isolated scratch data under target/manual-dev/codex/, disable product
dispatch for deterministic fixtures, preserve Claude's files and leave the live
store, processes, credentials and paused desktop unchanged. Honor applicable
Git/check requirements. Return the short PLAN-021 handoff with exact results,
changed paths, findings/blockers and the next packet; do not wait in a model loop.
```
