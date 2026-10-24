# Cathulu development

Follow [ADR-0021](docs/decisions/ADR-0021-continuous-supervised-development.md) and [PLAN-019](docs/plans/PLAN-019-continuous-supervised-development.md) when developing Cathulu.

- Exercise Cathulu's complete core workflow through real, already authorized work in other registered projects. Arranging those runs, capturing evidence, and fixing the resulting integration defects are development responsibilities.
- Develop and repair Cathulu itself in a separate Codex development session. Its own dispatcher is not a prerequisite for changing or validating Cathulu.
- Permit bounded, supervised, reversible local implementation and use after applicable correctness, scope, and safety checks pass. Comparative benefit is measured during use; it is not a prerequisite for these supervised attempts. Calibration, benefit claims, autonomous promotion, remote processing, and production actions retain their specific evidence and approval gates.
- Continue within the approved outcome through implementation, relevant checks, documentation, integration, and follow-up fixes. Reuse accepted plans. Milestones are validation checkpoints; passing one does not itself require another Owner prompt. Respect the session's authorization for Git and external actions.
- Prioritize the operational path in PLAN-015 M42 and PLAN-017 M44: real assignment, scoped context and Graphify where relevant, execution, event routing, reviewed QA, memory closeout, and recovery. Desktop UI work remains on hold; MCP startup is headless.
- A safe real assignment may establish dispatcher capability. Require its normal brief, plan, memory activation, scope, and tool preflight. Queue acceptance is not a PM start; an empty queue is not proof dispatch is unavailable. End the Main turn after dispatch acknowledgement and handle routed evidence in subsequent turns. Report observed failures precisely without replacing dispatch with a wait/poll loop.
- Inspect existing diffs before writing and preserve unrelated edits. A conflict blocks its affected scope; independent safe work continues.
- Track source implementation, built artifact, loaded process, live acceptance, and measured benefit separately. Missing observations remain unknown. Check the current runtime before repeating a reload or treating a historical status note as current evidence.

Project boundaries, role limits, parent reviews, deterministic QA, source-backed scoped memory, credentials/access controls, and production/data permissions remain governed by the [operating contract](docs/agent-operating-contract.md).
