# ADR-0002: Cathulu hosts future agent model and effort selectors

Date: 2026-10-20; feasibility checked 2026-10-21
Status: ACCEPTED; M9 implementation complete
Scope: Future agent configuration surface
Supersedes / superseded by: Clarifies the interaction surface in ADR-0001 without changing Codex's execution ownership

## Context

The owner intends to choose the Main Overseer and sub-agent models and effort levels through selectors. The project brief keeps conversation and agent execution in Codex, while the first Cathulu dashboard is explicitly read-only. The owner clarified that Cathulu should host the future selectors.

## Decision

- Cathulu provides model and effort selectors in a dedicated settings surface.
- These selectors will live in a dedicated configuration surface, separate from the read-only portfolio dashboard.
- Each configuration preview requires a visible target choice: registered project config, user base config, a named user profile, or export-only. Cathulu does not silently select or edit a file.
- Cathulu stores the preference in SQLite first. A separate, explicit Apply action is required to write config. The UI displays the exact model/effort field changes and per-file backups are made before the write.
- TOML edits preserve comments and unrelated values. Project targets come only from the registered project table and are checked against symlink/path escape. User targets resolve through `CODEX_HOME` or the user's `.codex` directory; the frontend cannot supply arbitrary paths.
- Main Overseer settings use the main config keys, default subagent settings use `[agents]`, and named-agent preferences update only an existing custom-agent TOML file whose declared name matches. A missing or incompatible named-agent file is visibly skipped; Cathulu does not create an incomplete custom-agent definition.
- Export-only produces snippets containing only selected keys and does not read config contents or write files.
- Model/effort choices come from the local bundled Codex CLI catalog after an allowlist filter. The catalog is not evidence of account/workspace availability.
- Codex continues to own conversation, model/tool execution, and native sub-agent runs. Cathulu does not become an agent runtime.
- M1 adds no selector controls.

## Open integration questions

- How should the selector stay aligned with the models and effort values available to the signed-in account or workspace?
- Can Codex expose a supported runtime signal for the model/effort actually active in a later session?

Current official documentation confirms the configuration keys and role-level support. Cathulu can record that a reviewed config was written, but it cannot prove the account supports the selected model or that a running/new Codex session has loaded it. Keep both facts explicitly unverified until a supported source reports them.

## Consequences

- Selector ownership is decided: Cathulu will host the future UI.
- Settings implementation is delivered in M9. Runtime effectiveness and account-specific availability remain unverified.
- The dashboard stays read-only; future selector settings use a separate configuration boundary.
- Codex exposes main-session model and reasoning-effort configuration through its settings/configuration layers and one-run CLI overrides. Spawned agents support global defaults, explicit spawn overrides, and custom role files with their own `model` and `model_reasoning_effort` values.
- These documented capabilities establish that per-role choices are representable. They do not provide Cathulu a documented API for changing the currently running Codex turn. Cathulu must label stored selections as requested/configured until the user applies the supported configuration and effective runtime values can be verified.
- SQLite schema v8 stores preferences and append-only stored/written events with applied target and content-hash metadata. Cathulu MCP has no Codex-configuration write operation.
- A config preview is held in desktop memory, bounded to four pending apply previews, and is consumed once. Apply rechecks file hashes/path boundaries, creates backups, and records `written_to_config`; runtime effectiveness remains false/unknown.
- The selector surface uses native controls and an inline field-level review rather than a component library or modal. The dashboard remains read-only and no extra UI dependency is introduced.

## Evidence

- The owner's initial project note requested future model/effort selectors for the orchestrator and sub-agents.
- The owner clarified: “Cathulu selectors later.”
- The project kickoff brief and ADR-0001 continue to define Codex as the execution surface and the dashboard as read-only.
- Official OpenAI documentation, checked 2026-10-21: [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference), [Codex subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents), and [Codex developer settings](https://learn.chatgpt.com/docs/developer-settings).
- M9 local validation, 2026-10-21: bundled-catalog allowlist and preference persistence, profile/project target isolation, TOML preservation, export-only, malformed input, stale-preview refusal, backup creation, and existing custom-agent role tests passed using disposable files. The Tauri command test passed through explicit preview/apply. The Windows integration smoke remains scoped to disposable repositories. No owner Codex config was touched; account availability and runtime effectiveness are still unverified.

## Reconsider if

The target Codex runtime removes support for the documented configuration fields, or the user's environment cannot apply the approved configuration layer. Revisit the user experience with the owner before replacing the selectors with another workflow.
