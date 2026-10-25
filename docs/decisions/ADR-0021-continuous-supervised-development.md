# ADR-0021 — Continuous supervised Cathulu development

- Status: Accepted by the Owner on 2026-10-24; policy implementation tracked in PLAN-019.
- Scope: Cathulu development cadence, supervised core use, and responsibility for generating real usage evidence.
- Supersedes: PLAN-003's requirement for the Owner to generate representative runs and evidence-before-implementation restrictions for supervised, reversible local core work. Existing empirical findings and capability-specific promotion gates remain valid.
- Related: [PLAN-019](../plans/PLAN-019-continuous-supervised-development.md), [PLAN-003](../plans/PLAN-003-evidence-gated-scaling.md), [PLAN-015](../plans/PLAN-015-event-driven-role-orchestration.md), [PLAN-017](../plans/PLAN-017-terminal-observability.md), [ADR-0007](ADR-0007-optional-specialist-model-optimization.md).
- Later amendment: [ADR-0023](ADR-0023-manual-shared-directory-development.md) records the Owner's manual same-directory Claude Code / Codex development choice. It supersedes only the Codex-only self-development wording for that workflow; the supervised-development, managed hierarchy, evidence and permission boundaries below remain in force.

## Context

Cathulu has separately validated state, memory, Graphify retrieval, delegation, and orchestration components. Several live integration checks remain open. Requiring representative product-use evidence before allowing safe implementation or supervised use, while assigning run generation to the Owner, delays the feedback needed to finish the product. Built artifacts, loaded processes, and verified live behavior have also been reported at different stages; none establishes the others automatically.

## Decision

1. Cathulu development owns a continuous integration loop: implement a bounded capability, validate its relevant local contracts, load the intended build when authorized and safe, exercise it on an already authorized task in another registered project, review actual results, and repair observed failures. Reuse the existing run, event, artifact, memory, and QA authorities.
2. Cathulu itself is developed and repaired in a separate Codex development session. Its own dispatcher need not work before that session can repair it. External project execution uses the normal Main → PM → Worker hierarchy and remains scoped to the Owner's project directions.
3. Supervised, reversible local core development and use may proceed before comparative benefit is established. Correctness, project access, role capabilities, scope, memory activation, data integrity, and required QA remain prerequisites. A small or incomplete cohort is an evaluation limitation, not a blanket stop on implementation or supervised use.
4. Development agents arrange real usage and capture evidence within existing authorization. If an essential credential, workspace grant, reload that cannot safely be performed, or Owner-level decision is missing, report the exact action needed. Do not ask the Owner to manufacture tasks or infer dependencies between independent projects.
5. A bounded real assignment may be the live dispatcher check. Its standard planning, memory, and tool-preflight gates apply. Observe queue acceptance, role start, tool access, and routed events separately. An empty queue or absence of prior live evidence does not prove dispatch is disabled. Main ends its dispatch turn after acknowledgement; it does not wait or poll for the probe result. Observed runtime/preflight failures block the affected path without fictional roles or success claims.
6. Keep distinct evidence levels: implemented source, validated build, loaded artifact, accepted live behavior, and measured comparative benefit. A build alone does not complete a live milestone. Tests and fixture runs establish mechanics; live task results establish operational behavior; comparable cohorts support benefit and calibrated policy claims.
7. Milestones are internal quality checkpoints within an authorized outcome. Continue through accepted work and corrective fixes when validation passes and authority remains sufficient. Review and record material decisions; obtain fresh Owner input only when required by scope, missing essential information, or an explicit authority boundary.

## Boundaries and consequences

- Existing independent-project boundaries, one active PM per project, two Workers total per PM assignment, parent reviews, deterministic success gates, source-backed memory, and known-by/freshness rules remain in force.
- Graphify is part of the core retrieval workflow when structurally relevant and a valid scoped snapshot is available. Missing/stale snapshots are recorded integration or freshness findings; they do not justify fabricated graph evidence or stopping unrelated safe work.
- Additional cross-project scopes still require a real scope need; independent PM coordination does not depend on PLAN-003 M10. M12/M13 mechanics can be implemented and exercised with honest insufficient-evidence states while calibration and improvement claims wait for suitable real data. M14 learned-policy activation remains advisory/offline and Owner-controlled.
- Optional Jev/ScaleDown providers remain disabled by default. ADR-0007's remote-data consent, shadow validation, and explicit active-treatment approval are not granted by this policy. Credentials/access changes, spending, production changes, destructive operations, and external disclosure retain their existing gates.
- Desktop UI remains on hold; MCP startup remains headless. Existing M42/M44 live checks take priority over surrounding feature expansion. This decision does not mark those checks passed or activate runtime features.
- Preserve existing local changes and session-specific Git permissions. Update current policy without rewriting historical evidence as if it had been gathered under this decision.

## Validation

PLAN-019 records policy consistency, installed skill validation, document links, whitespace, and worktree preservation. Operational completion continues to require the actual PLAN-015 M42 and PLAN-017 M44 evidence.
