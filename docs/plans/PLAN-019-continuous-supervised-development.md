# PLAN-019 — Continuous supervised Cathulu development

Status: M46 policy implemented and validated locally; repository delivery is tracked in the [2026-10-25 integration record](../readiness/repository-integration-2026-10-25.md). Owner authorized the policy change on 2026-10-24 and the pending push/merge synchronization on 2026-10-25.
Created: 2026-10-24
Last updated: 2026-10-25
Decision: [ADR-0021](../decisions/ADR-0021-continuous-supervised-development.md)

## Objective

Make exercising Cathulu's complete core workflow through authorized work in other registered projects part of Cathulu development. Develop and repair Cathulu itself in a separate Codex development session, so a broken Cathulu dispatcher cannot prevent its own repair.

## Current and desired state

PLAN-003 assigns representative run generation to the Owner and delays several implementation steps until product-benefit evidence exists. PLAN-015 and PLAN-017 contain live checks that need real project work, while the installed orchestrator skill still describes a closed readiness constant and unavailable dispatch. These instructions can prevent the supervised usage needed to finish integration.

Use one continuous development loop: implement a bounded capability, check its safety and correctness locally, load the intended build when authorized and safe, exercise it through an authorized external-project task, reconcile observed behavior, and repair failures. Comparative benefit and autonomous promotion retain their evidence requirements. A completed build or an empty queue does not establish runtime readiness or failure.

## Scope and constraints

- Align repository agent instructions, product definition, architecture, operating contract, roadmap, affected plans, and the installed orchestrator skill.
- Permit supervised, reversible local core implementation and use before comparative benefit is proven. Capability-specific correctness, scope, memory, role, approval, and QA checks still apply.
- Make the Cathulu development agent responsible for arranging and recording real runs within existing project authorization; ask the Owner only for essential missing information or an action requiring Owner authority.
- Preserve independent projects, one active PM per project, two Workers total per PM assignment, parent reviews, and source-backed memory.
- Keep Cathulu development outside Cathulu-managed project execution. Desktop UI remains on hold and startup remains MCP-only.
- Preserve optional-provider consent and active-treatment gates, production/data/access boundaries, and existing local edits. This change performs no provider activation, dispatch, deployment, reload, or Git integration.

## Source-of-truth impact and tools

Update `AGENTS.md`, project definition, architecture, operating contract, roadmap, PLAN-003, PLAN-015, PLAN-017, and ADR-0021. Correct the installed `cathulu-orchestrator` skill's stale capability statements and add the same supervised bootstrap boundary. Use implementation-plan, milestone-delivery, and skill-creator; no framework/API changes are involved.

## M46 — Align the development and supervised-use policy

### Work

1. Record ADR-0021 and repository instructions for the continuous loop.
2. Move evidence-generation responsibility into development and distinguish supervised implementation/use from proven benefit and autonomous promotion.
3. Reconcile the live-validation plans and installed skill so a bounded real assignment can establish dispatch capability without claiming success from preparation or queue acceptance.
4. Validate document links, policy consistency, skill structure, whitespace, and preservation of unrelated local files.

### Acceptance and validation

- No current instruction assigns representative run generation solely to the Owner or requires proven comparative benefit before supervised core development/use.
- M10 applicability, M12/M13 calibration and comparisons, M14 autonomous-policy restrictions, and optional-provider approvals are explicitly scoped; historical evidence remains intact.
- A supervised dispatch attempt requires a real authorized project task and its normal planning/memory/preflight checks. Parent turns end after dispatch acknowledgement; subsequent routing evidence is handled in event turns. Missing runtime evidence is unknown, and observed failures remain concrete blockers.
- Source, built, loaded, and live-verified states remain distinguishable. M42/M44 stay open until actual acceptance evidence passes.
- Cathulu self-development remains a separate Codex session; external-project task authorization is not expanded by the development policy.
- Installed skill validation and repository documentation checks pass. No executable behavior changes, so Rust/frontend tests and release builds are unnecessary for this milestone.
- Review the targeted patches against the pre-write source and Git diff, verify that mutations stayed within the policy scope, and compare a persistent manifest during final validation for unrelated changed files.

### Git and recovery

The 2026-10-24 validation left changes local under the then-current no-stage/commit/push instruction. On 2026-10-25 the Owner explicitly requested bringing repository pushes and merges up to date; the integration record tracks that authorized delivery. If recovery is needed, remove only this change's added sections and restore its deliberately edited paragraphs using the recorded patch, preserving existing worktree changes. The installed skill has a persistent before-copy in the review workspace.

## Next integration work

Continue existing PLAN-015 M42 and PLAN-017 M44 rather than creating another runtime milestone. Verify one bounded external-project assignment through the actual dispatcher, then expand to eligible independent project PMs. Capture startup, tool access, scoped context/Graphify availability, routing, worker reports, QA, memory closeout, and recovery evidence. Repairs belong to the separate Cathulu development session. Safe scope and source freshness remain prerequisites; approval for this policy does not authorize new external-project tasks.

## Progress and evidence

- [x] Inspected the existing dirty worktree and the affected source before writing; 53 files already contained local changes. Applied narrow patches only within the declared policy scope.
- [x] Reviewed the relevant plans, product definition, operating contract, lifecycle skills, and installed orchestrator instructions.
- [x] Policy and installed skill edits implemented.
- [x] Acceptance checks and preservation review passed.
- [ ] Git integration, subject to Owner authorization.

Validation notes: the execution host discarded the initial sandbox-temporary copies between tool calls, so they cannot support a full-turn byte-for-byte preservation claim. Targeted source/patch review supplies the earlier edit audit; a persistent manifest captured after the repository policy edits verifies the 47 unrelated changed files during final validation. Corrected an existing roadmap link using the wrong ADR-0017 filename. The skill validator initially used Windows cp1252; rerunning it with Python UTF-8 mode passed for the existing Unicode skill text.

### Local acceptance evidence (2026-10-24)

- Documentation validation passed for 10 files and all 113 local links; Markdown fence/final-newline checks and targeted `git diff --check` passed.
- Installed `cathulu-orchestrator/SKILL.md` passed the skill-creator validator. Its SHA-256 is `83B795F7DD66AD41E20BB5CE8554C1CE0D0C4B4AAD09E9ABE2235DEED254EC86`, matching the validated candidate. Replacement checked that the installed file still matched its before-copy.
- Reviewed the instruction outcomes for an unverified/empty dispatch queue, a concrete preflight failure, an insufficient calibration cohort, optional remote/active-provider work, Cathulu repairing its own dispatcher, and a passed milestone. The policy permits the bounded supervised path, preserves the specific blockers/authority gates, and does not authorize a parent wait loop or unsupported success claim. This is a static policy review, not a live agent/runtime evaluation.
- Confirmed the existing source has `ROLE_DISPATCHER_RUNTIME_GATES_VALIDATED = true` and default-on dispatch; the installed skill's closed-readiness and unimplemented-dispatch statements were stale. No runtime code was changed.
- Relevant source/patch review and the persistent manifest confirm the change's policy scope; all 47 previously changed files outside that scope remained identical during final validation.
- No Rust/frontend tests, release build, reload, provider activation, or project dispatch was performed for this documentation/skill change. M42/M44 operational acceptance remains open.
