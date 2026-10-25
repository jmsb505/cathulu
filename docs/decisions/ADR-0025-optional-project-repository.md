# ADR-0025 — Projects with optional repository association

- Status: Accepted implementation choice for the provider-free M50 foundation; personal-data and provider-disclosure policy remains undecided under P-Q2.
- Scope: Project identity and repository association for code and non-code projects.
- Depends on: [ADR-0022](ADR-0022-mixed-provider-personal-project-os.md), [PLAN-020 M50](../plans/PLAN-020-mixed-provider-personal-project-os.md), and [PLAN-021](../plans/PLAN-021-manual-claude-codex-development.md).

## Context

The current `projects` table equates a project with a Git repository: its path is non-null and unique, registration inspects Git, and repository reconciliation assumes every project has a path. PLAN-020 M50 requires provider-free tasks and checkpoints for non-code work while preserving project IDs and existing repository semantics. M50 has no accepted personal-data categories or provider grants yet.

## Decision

1. Keep the existing project ID as the stable identity for project-manager roles, memory scopes, tasks, objectives, dependencies, and checkpoints. A repository is an optional association, not the project identity.
2. Support exactly zero or one associated repository in this slice. Repository projects require a path; non-code projects require `path = NULL`. Do not invent a sentinel or fake directory.
3. Migrate every existing project as `kind = repository`, `visibility = shared`, preserving its ID, canonical path, Git state, registration history, and child references. Enforce uniqueness on non-null repository paths.
4. Create non-code projects through a separate provider-free command. They default to `visibility = private`. The existing logical Project Manager role row is retained for identity consistency, but a private non-code project is not dispatchable.
5. Git-only operations report `project_has_no_repository` or skip non-code projects. Task, objective, checkpoint, and resume operations remain keyed by project ID and do not call Git.
6. Private non-code project data is denied to role capabilities and excluded or denied in Main's context resources. It remains available through the local unscoped management interface. That local interface does not itself authenticate an Owner; the product must remain local-only.
7. Do not add personal memory categories, collect personal facts, change retention, or disclose non-code content to any provider. P-Q2 remains open. Sharing a private project and enabling provider dispatch require later explicit decisions and validation.

## Consequences

- Existing repository users retain project and role identities; registration remains path-idempotent.
- Non-code project tasks and checkpoints are useful without Git or provider execution.
- Path consumers must handle no repository explicitly. Dispatch remains globally off, and non-code projects are additionally refused at admission.
- The M50 personal-memory scope, provenance, correction, export/erasure, retention, and disclosure requirements remain incomplete; this ADR does not claim M50 completion or live readiness.

## Validation

The CX-06 M50 handoff records populated migration, foreign-key/trigger, provider-free task/checkpoint, no-Git, role/resource denial, and offline test evidence. No live store or provider is used.
