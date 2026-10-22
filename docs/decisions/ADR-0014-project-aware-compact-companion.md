# ADR-0014: Project-aware compact companion

Status: Accepted for M32 implementation  
Date: 2026-10-22  
Deciders: Project owner and Cathulu maintainers

## Context

The separate floating monitor was designed as a tiny transparent pill, but its project indicators are difficult to see, move when state priority changes, and do not lead to the selected project's current work. Its expanded board lists status text without a way to inspect the latest project evidence. The owner wants a Codex Mini-like compact capsule that reveals relevant information per project on demand.

## Decision

1. Keep the monitor as one small transparent, always-on-top-capable desktop companion. Do not replace or duplicate the single graph in the main dashboard.
2. Render one larger, accessible bubble per registered project in stable project order. A bubble's color and restrained working pulse derive from the existing read-only `DashboardSnapshot`; verified and inconclusive runs use distinct signals, and labels remain available to hover, keyboard, and assistive technology.
3. Clicking a bubble opens the expanded companion with its project selected. Clicking a project row toggles a concise inline disclosure; it may show a pending owner action, latest checkpoint and age, failed mandatory checks, or the current run summary when no checkpoint exists. Do not fabricate progress or treat self-reported work as verified.
4. Keep the project overview bounded and scrollable. The expanded view remains a per-project status surface, not a miniaturized dashboard or a new navigation tab.
5. Honor reduced-motion preferences and preserve pinning, drag, dashboard restore, and close behavior. Project data remains local and read-only.
6. Do not open, focus, or restart the monitor or MCP during a build or milestone delivery.

This refines ADR-0008's monitor interaction and evidence presentation. Its existing window model, read path, transparent surface, and separation from the main graph remain in force.

## Alternatives considered

- **Keep passive, state-sorted dots:** rejected because state changes reorder projects and dots are too small to scan.
- **Put project names and full activity text in the compact pill:** rejected because the pill must remain unobtrusive beside Codex.
- **Open the full dashboard when a bubble is clicked:** rejected because the separate companion should expose project detail without focus-stealing.
- **Add a new telemetry API or UI dependency:** rejected because the existing snapshot already contains relevant recorded evidence.
- **Show synthetic percentages or inferred task progress:** rejected because no deterministic progress source exists.

## Consequences

- The compact pill communicates state quickly while hover/accessible names and expansion provide project identity and evidence.
- The expanded companion requires slightly more vertical space and may scroll when several projects are registered.
- Native transparency and interaction still need manual visual review in the user's desktop session.
