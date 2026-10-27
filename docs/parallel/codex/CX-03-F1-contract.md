# CX-03 — F1 common contract candidate v1

Date: 2026-10-25
Baseline: M49 `0e6718a`, branch `codex/manual-dual-m48`.
Status: actual exported code and fixtures pass local core checks. Claude's CC-03 review must accept the stable candidate commit before F1 is treated as agreed for adapter/policy implementation. This packet is M51a only; it does not complete M51 parity or activate a provider.

## Public locations

| Consumer need | Actual export |
| --- | --- |
| Provider interface, IDs, capabilities, trusted runtime configuration, events/errors | `cathulu_core::agent_provider` in `src/agent_provider.rs` |
| No-I/O contract double and synthetic typed artifacts | `agent_provider::fixtures::{ScriptedProvider, sample_artifacts}` |
| WorkOrder/WorkResult, envelope scope/hashes, context manifest and receipt shapes | `cathulu_core::execution_artifacts` |
| Routing request/candidate/policy/decision and availability types | `cathulu_core::provider_routing` |
| Reused plan/brief/review names | `execution_artifacts::{MainWorkflowBrief, PlanSpec, PlanReview}` re-export existing delegation types |
| Checked JSON payloads | [F1-sample-payloads.json](F1-sample-payloads.json) |
| Regenerate payloads | `cargo run -p cathulu-core --example provider_contract_payloads --locked --offline` |
| Contract acceptance entry point | `cargo test -p cathulu-core --test agent_provider_contract --locked --offline` |

`AgentProvider` has bounded synchronous methods: `capabilities`, `open_session(SessionConfig)`, `resume_session(session, SessionConfig)`, `submit_turn(SubmitTurnRequest)`, `next_event(Duration)`, `answer_request(session, request_id, Decline)`, `interrupt(turn)`, `session_status(session)` and `close_session(session)`. This matches the existing synchronous app-server/dispatcher thread and adds no async dependency. The trait is object-safe and `Send`.

## Agreed implementation choices proposed for review

- `CONTRACT_VERSION = 1`; provider IDs are validated strings (`codex`, `claude` in current fixtures). Session and turn IDs are opaque validated values, namespaced by provider/session. Logical role, project and permissions are separate runtime binding inputs. Unknown session is the explicit `SessionGone` error, an addition to CC-03's five general error classes.
- Capabilities use `Supported | Unsupported | Unverified`, with separate isolation observations for settings, plugins, MCP servers, instructions and memory. Credential source is observed separately and must match the caller's explicit allowed sources. Unknown is not an approved credential source.
- `PermissionManifest` is hashable and contains the exact absolute workspace, read/write access, relative scope, network permission and allowed credential sources. `McpToolConfig` carries only the core-supplied stdio server configuration and required tool names. It deliberately implements neither Debug nor Serialize because environment values can contain capability tokens. The adapter must not widen these inputs.
- Submit requests bind session, attempt key and bounded text. The double deduplicates the complete request, including session, and retains an uncertain response without another submission. `AckUnknown` and `ProviderError::Uncertain` are distinct from definitely-not-started unavailability. Timed retry hints are valid only for capacity errors.
- Provider completion is only an observed turn outcome. The event and error types have no task-acceptance operation. Automatic approval has only the `Decline` value. Unsupported interrupt/status are explicit errors in the double, not passing stubs.
- `ProviderEvent::external_event_key` derives a deterministic provider/session/turn/kind/identity namespace for the existing external-event ledger. Replayed events retain their key; provider collisions differ. Missing event ID and sequence returns null rather than inventing durable identity. Actual persisted deduplication/state-transition enforcement remains M51b.
- WorkOrder wraps the existing normalized `DelegationTaskPacket` and a context manifest. WorkResult wraps the existing normalized `WorkerReport`, retains its scope validation and permits only `reported` submission state. Neither envelope expresses accepted canonical state. The existing Store remains the later authority for parent-row authorization, durable idempotency, QA/review and acceptance.
- Envelopes retain artifact/parent/evidence IDs, project/task/revision/run/delegation/attempt scope, role, optional observed provider/session/turn/model/harness attribution, source event, repository observation, optional lease, timestamp, permission hash and payload hash. Null remains unknown. Structural expected-scope checks are not database authorization.
- Context selection is bounded to 64 unique memory IDs/hashes with full/delta/reference delivery. Session receipts distinguish selected, submitted and agent-reported read. Receipt persistence and actual fresh-session delivery remain CX-04/M51b work.
- Routing types carry explicit Owner pin/exclusions/preferences, task shape/risk/write/review needs, time, disclosure decisions and observed availability. Unknown availability has no quota field. Empty internally tagged variants use `{}` in Rust so unknown JSON fields are actually rejected. Policy remains Claude's owned implementation; no default provider choice is wired here.

`RoutingCandidate::all_required_disclosures_granted` is an aggregate input supplied by the trusted routing caller; the candidate type does not compute grants or grant disclosure authority. The caller may set it true only after checking every required category/provider grant for this task. Until M50 implements that evaluator and persistence, producers must set it false for personal data. A missing, expired, revoked or unrecognized grant makes the aggregate false. This F1 shape does not claim personal-disclosure enforcement already exists. CC-05's all-unavailable scenarios must retain the distinction between capacity queueing, account restriction and required-review wait; no timed retry is inferred for account restriction.

Bounds: provider ID 64 bytes; session/turn ID 256; attempt key 200; work-order text/envelope 64 KiB; result envelope 16 KiB; event/error input 8 KiB; unknown event excerpt 512 bytes; context manifest 64 entries. Existing task/report validators remain authoritative inside these envelopes.

## Consumer compilation and handoff

Claude may use its own standalone crate under `target/manual-dev/claude/scratch/`, with `[workspace]`, a `cathulu-core` path dependency on `../../../..`, and its own lockfile/build output. It can include its allocated adapter/policy source in that wrapper and import these real exports; no duplicate types or empty production module registration is needed.

The independent Codex consumer is `target/manual-dev/codex/cx03-consumer/`, with its own build output at `target/manual-dev/codex/cx03-consumer-build/`. A fresh offline resolver initially failed because cached Tokio 1.54.0 is yanked. Seeding the scratch lockfile from the unchanged repository `Cargo.lock` preserves the existing graph; Cargo permits already-locked yanked releases ([official resolver documentation](https://doc.rust-lang.org/cargo/reference/resolver.html#yanked-versions), checked through Context7). Run once offline to add the consumer package, then use `--locked --offline`. Do not update production dependencies to solve scratch resolution.

The standalone consumer passed `cargo check --manifest-path target/manual-dev/codex/cx03-consumer/Cargo.toml --locked --offline` with its own lockfile and output. It imports `Box<dyn AgentProvider>`, the real artifact fixture/validators and routing types. No production manifest, dependency or lockfile was changed.

Before production registration, Claude hands off its exact scratch compile/test commands and results. Contract amendments name affected consumers/tests on the shared board and get a new accepted revision. M51b will wrap the real Codex client and persist provider sessions/attempts/delivery; M52 will enforce leased workspaces and uncertain recovery. No live execution is enabled by this candidate.

## Validation

- `cargo check -p cathulu-core --locked --offline`: pass.
- `agent_provider_contract`: 7 passed. Covers namespace/size validation, same-attempt lost-ack deduplication, replacement-session reuse denial, duplicate/out-of-order event identity, explicit approval decline, unsupported operations, auth/capacity separation, malformed input, artifact hash/scope/parent validation and strict unknown availability.
- Full `cargo test -p cathulu-core --locked --offline --quiet`: 114 passed, 0 failed.
- The executable payload generator emits actual typed examples; the contract test parses and validates the committed JSON against the exported code.
- The first targeted run exposed Serde's acceptance of extra fields on internally tagged unit variants. Empty record variants now reject invented quota/provider/session fields; negative tests cover the correction.

No provider process, credential/configuration change, live-store migration, dispatcher activation or desktop action was performed. Fixture mechanics do not prove Codex adapter parity, session persistence, live Claude capability, routing enforcement or M51-M54 acceptance.
