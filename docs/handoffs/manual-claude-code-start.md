# Manual Claude Code starter prompt

Copy this into the Claude Code session you start in `C:\Users\sjuan\Desktop\cathulu` after the revised planning checkpoint is committed and the shared launch HEAD is verified clean. This file does not start a session.

```text
You are the Claude Code lane for PLAN-021. Codex uses this same checkout and
branch. Read AGENTS.md, ADR-0023, PLAN-020 and PLAN-021 from the committed launch
checkpoint. Record the observed HEAD/branch and confirm the planning files are
tracked. The Owner verifies clean status before either session starts writing;
if that launch check has not happened, report the uncommitted work and stop
before edits. Do not reset to the historical d2422fb inspection commit.

Start CC-01 only: review task/resume semantics and write concise examples,
required corrections and optional suggestions to
docs/parallel/claude/CC-01-task-resume.md.
Both fresh sessions must read committed source/evidence; do not assume either
inherits the conversation behind PR #33. After CC-01, report and recommend CC-02.

Waves 0-3 are ordinary implement-then-review. You own your design notes; Codex
owns production code and runs normal tests. There is no per-packet freeze or
hash manifest. Finish a document before the integrator stages that document.
Do not stage/commit/pull/rebase/stash/reset/clean/switch branches, touch shared
schema/manifests or rewrite Codex's files. Request shared changes in your report.

At F1 and later CC-04/CC-05, you own the named adapter/policy source and a
throwaway standalone crate under target/manual-dev/claude/scratch/, including
its Cargo.toml/Cargo.lock, wrapper/tests and separate build output. Seed its
lock from the committed production lock. Compile/test the actual source
against cathulu-core by path before handoff; do not copy/redefine its public
contract or defer the first compile until integration. The live path dependency
still reads Codex's shared core; report transient core-edit failures accurately.

During real concurrent coding in waves 4-6, acknowledge PAUSE WRITES before
shared integration/checks/combined commits; use Git status/diff, including new
files, with no hash manifests. A single separate Claude worktree for that
stretch is an optional Owner-selected fallback, not something to create now.

Return the short PLAN-021 handoff: packet/base/contract, changed or unfinished
paths, exact check results, findings/blockers and next packet. Use scratch data,
not the live store. Do not launch product agents, change provider auth/settings
or resume the paused desktop track. You are not alone; preserve the other lane.
```
