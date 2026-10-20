# ADR-0001: Cathulu is the local evidence layer around Codex

Date: 2026-10-20
Status: ACCEPTED
Scope: Product boundary and first implementation increment
Supersedes / superseded by: None

## Context and architectural drivers

The owner wants portfolio-level durable state and quality evidence around Codex while continuing to interact only in Codex. Rebuilding agent execution would duplicate Codex capabilities and undermine the desired local evidence layer. Quality evidence, local privacy, and low operational overhead are primary drivers.

## Decision and invariants

Cathulu stores local portfolio/project state, memory, decisions, workflow versions, run telemetry, quality evidence, and compact context. Codex remains responsible for conversation, model/tool loops, coding, Git actions, approvals, sandboxing, and native agents. The desktop dashboard is read-only. Semantic MCP operations provide the boundary for Codex-facing reads and state transitions.

`verified_success` requires acceptance criteria and all mandatory gates to pass. An agent report alone never sets that state. Events are append-only, and decisions/workflow versions are immutable.

## Alternatives and rationale

- **Build a Cathulu agent runtime:** rejected because the owner explicitly wants Codex to remain the agent and existing Codex capabilities should be reused.
- **Make Cathulu a second chat UI:** rejected because all user conversation and approvals remain in Codex.
- **Make the dashboard a control surface:** rejected because the product brief requires a read-only observational UI.

## Consequences, risks and reversibility

The local MCP and desktop UI depend on stable semantic domain contracts. Available hooks and telemetry can improve measurement but are optional and must not be treated as a complete audit or permission boundary. The architecture can be revised through a superseding ADR if measured evidence or Codex platform constraints require it.

The dashboard remains read-only. This does not rule out a separate Cathulu settings surface for future model/effort selectors; that product direction is recorded in [ADR-0002](ADR-0002-cathulu-hosted-agent-selectors.md), while the Codex configuration mechanism remains to be validated.

## Evidence and intended verification

- User-provided `CATHULU_PROJECT_KICKOFF.md`, sections Purpose, Product model, Design principles, Local MCP, Desktop dashboard, MVP, and Non-negotiable product constraints.
- Local MCP configuration and hook support are documented in current [Codex MCP docs](https://learn.chatgpt.com/docs/extend/mcp) and [Codex hook docs](https://learn.chatgpt.com/docs/hooks).
- First milestone validation will check dashboard read-only behavior, semantic MCP boundaries, mandatory QA enforcement, and state/event persistence.

## Reconsider if

The owner changes the interaction boundary, Codex cannot support a local MCP connection on the target platform, or observed run evidence shows the architecture fails to improve verified work.

## Affected canonical documents

- [Project definition](../project-definition.md)
- [Architecture](../architecture.md)
- [Roadmap](../roadmap.md)
