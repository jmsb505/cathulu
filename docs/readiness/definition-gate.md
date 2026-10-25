# Definition gate

## Pivot first-slice definition — 2026-10-25

Scope: PLAN-020 M48-M49, provider-free planning state for existing registered projects and checkpoint/resume.
Overall: **PASS** for this bounded definition/planning handoff.
Unresolved blocking rows: none for this scope. This is not whole-pivot integration readiness, implementation evidence, or permission to activate providers, collect personal information or resume desktop work.

| ID | Status | Blocking | Evidence | Owner / next action | Dependent scope |
| --- | --- | --- | --- | --- | --- |
| purpose | PASS | true | [Current purpose](../project-definition.md#purpose), [ADR-0022](../decisions/ADR-0022-mixed-provider-personal-project-os.md) | Owner direction recorded; development implements | M48-M49 |
| actors-workflows | PASS | true | [Pivot scope](../project-definition.md#pivot-scope-and-invariants), assessment sections 14 and 28 | Development; preserve Owner command/agent proposal separation | M48-M49 |
| scope-mvp | PASS | true | [Assessment](../pivot-migration.md) sections 12, 14, 28 and 31; [PLAN-020](../plans/PLAN-020-mixed-provider-personal-project-os.md) M48-M49 | Development; existing-project slice without providers/UI/personal collection | M48-M49 |
| domain-invariants | PASS | true | Assessment sections 12-14 and 18; existing [workflow](../../src/workflow.rs) / [schema](../../src/migrations.rs) | Development; preserve IDs, objective ownership, QA, immutable checkpoints and audited transitions | M48-M49 |
| drivers | PASS | true | Assessment section 7; [project quality drivers](../project-definition.md#constraints-and-quality-drivers) retained under ADR-0022 | Development; prove provider absence, integrity and restart rather than invent benefit thresholds | M48-M49 |
| architecture | PASS | true | Assessment sections 2, 7, 12-14; [Store](../../src/lib.rs), [MCP](../../src/mcp.rs) | Development; extend current in-process Rust/SQLite seams | M48-M49 |
| security-baseline | PASS | true | Assessment sections 17, 18, 22-24; [operating contract](../agent-operating-contract.md), [capability tests](../../tests/role_capabilities.rs) | Development; test scope denial, revision/acceptance guards and no new provider disclosure | M48-M49 |
| integrations | PASS | true | Existing local SQLite/Git/MCP seams in assessment section 2; no provider integration required by first slice | Development; public MCP fixture validation, providers absent | M48-M49 |
| feasibility | PASS | true | Existing task/decision/event/schema code and restart fixture in [MCP tests](../../tests/mcp_vertical_slice.rs); assessment sections 22 and 28 | Development; recheck dirty diffs/schema version before isolated migrations; no new external prerequisite | M48-M49 |
| validation | PASS | true | Assessment section 27 and PLAN-020 M48-M49 acceptance/validation | Development; populated migration, failure/recovery, permission and zero-provider proofs | M48-M49 |
| documentation-profile | PASS | true | [Authority map](../README.md), [profile](../pivot-migration.md#development-skills--tooling-profile), [state](../kickoff-state.json), ADR-0022 and PLAN-020 | Development; keep evidence and intended design separate | Planning handoff |

All 11 scoped criteria PASS by the definition-gate aggregation rule. This establishes a coherent design and validation strategy, not passing future tests. M47's documentation checks and Git integration state are recorded in PLAN-020.

Deferred dependency gates remain unresolved in the single [kickoff question ledger](../kickoff-state.json): P-Q1 still needs Owner login plus subscription-mode, permission and isolation verification (Claude Code 2.1.302 is installed); P-Q2 has no approved personal categories and must be revisited at M50; broader repository identity (P-Q3); actual current dispatcher observations (P-Q4); and desktop resumption (P-Q5). Their affected scopes remain pending. The Owner resolved P-Q6's integration authorization through the subsequent push/merge request; delivery evidence is in the [repository integration record](repository-integration-2026-10-25.md). Whole mixed-provider live readiness is **UNKNOWN**; this bounded PASS does not promote it. PLAN-015 M42 and PLAN-017 M44 retain their own acceptance criteria and evidence status.

## Historical initial definition — 2026-10-20

The record below is preserved as dated initial-kickoff evidence. Its empty-repository feasibility premise and Codex-only first increment are not current runtime observations or the active pivot definition.

Date: 2026-10-20
Scope: Cathulu product definition and first coherent Phase 1 increment
Overall: **PASS**
Implementation readiness: The definition is ready for a Phase 1 implementation plan. This gate does not claim that code or runtime behavior exists.

| ID | Status | Blocking | Evidence | Owner / next action | Dependent scope |
| --- | --- | --- | --- | --- | --- |
| purpose | PASS | true | [Project definition: Purpose](../project-definition.md#purpose); [ADR-0001](../decisions/ADR-0001-cathulu-is-the-local-evidence-layer.md) | Owner; resolved | All project work |
| actors-workflows | PASS | true | [Actors and ownership](../project-definition.md#actors-and-ownership); [Core workflows](../project-definition.md#core-workflows) | Owner; resolved | First increment |
| scope-mvp | PASS | true | [Scope and non-goals](../project-definition.md#scope-and-non-goals); [Roadmap](../roadmap.md) | Owner; resolved | First increment |
| domain-invariants | PASS | true | [Domain terms and invariants](../project-definition.md#domain-terms-and-invariants) | Owner; resolved | Persistence and quality state |
| drivers | PASS | true | [Constraints and quality drivers](../project-definition.md#constraints-and-quality-drivers) | Owner; resolved | Architecture |
| architecture | PASS | true | [System boundary and Phase 1 stack](../architecture.md#system-boundary); [Codex integration evidence](../architecture.md#codex-integration-and-feasibility-evidence); [ADR-0001](../decisions/ADR-0001-cathulu-is-the-local-evidence-layer.md) | Owner; validate in first milestone | First increment |
| security-baseline | PASS | true | [Security and privacy baseline](../architecture.md#security-and-privacy-baseline); attached project brief §23 (source: `C:\Users\sjuan\Downloads\CATHULU_PROJECT_KICKOFF.md`) | Owner; implement and verify controls per plan | State, MCP, telemetry |
| integrations | PASS | true | [Codex integration evidence](../architecture.md#codex-integration-and-feasibility-evidence); [Codex MCP docs](https://learn.chatgpt.com/docs/extend/mcp); [Git reconciliation design](../architecture.md#system-boundary) | Owner; verify Windows launch and Git failures in milestone | MCP and Git path |
| feasibility | PASS | true | Empty local checkout and empty remote repo; official docs support local MCP, hooks, and OTel configuration; risks and checks in [Architecture](../architecture.md#feasibility-risks-and-next-checks) | Owner; verify exact target runtime during implementation | First increment and telemetry |
| validation | PASS | true | [Success and validation](../project-definition.md#success-and-validation); user brief §§26–27 (source: `C:\Users\sjuan\Downloads\CATHULU_PROJECT_KICKOFF.md`) | Owner; define public test seams and commands in plan | First increment |
| documentation-profile | PASS | true | [Authority map](../README.md); [Architecture and development profile](../architecture.md); [Kickoff state](../kickoff-state.json) | Owner; maintain as implementation advances | Planning handoff |

All criteria pass for planning the first increment. Cathulu-hosted model/effort selectors are accepted for a later milestone; the supported Codex configuration pathway and per-role behavior remain to be verified. Exact local telemetry fields, public distribution/licensing, and numeric evaluation thresholds are also deferred or nonblocking. The implementation plan must validate the target Codex runtime before relying on telemetry, keep the local MCP path available independently, and keep selector controls outside the read-only dashboard.
