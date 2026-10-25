# CC-05 routing policy: design, test cases and implementation status (M54 preparation)

Agent: Claude Code lane (PLAN-021 wave 5). Status: the pure policy in `src/routing_policy.rs` is implemented against the accepted F1 types (`cathulu_core::provider_routing`, commit `09f192b`), fixture-tested in the scratch crate (§8), and registered in `src/lib.rs` as a pure, non-activating module. Codex owns shared integration; the CX-08 wiring is still open. §1–§6 are the design it implements.
Inputs and outputs come from [CC-03 §7](CC-03-provider-contract.md). The failure and recovery cases come from [CC-03 §9](CC-03-provider-contract.md) and [CC-04 §4](CC-04-adapter-evidence.md).
Owner decisions: **D-4 (2026-10-25) is decided** for the case where both providers are unavailable. On the same day the Owner decided that a review whose only independent reviewer is down also blocks and is reported upward (R6, R19; §5). Every other default is **proposed v1 for the Owner to change.** Each one is recorded with its reason, so it can be changed by editing this file and bumping `policy_version`.

## 1. Purpose and boundary

- Pure function: `decide(request, availability, policy) -> decision`. No I/O, no provider calls, no clock except the `now` passed in.
- Deterministic: the same inputs and policy version always give the same decision (test R16).
- The policy never reads the database or a provider. Cathulu builds the inputs (CC-03 §7).
- Administrative work never reaches the policy's provider choice (PLAN-020 M54: "ordinary list/priority/resume commands invoke zero providers").

## 2. Decision order (policy v1, proposed)

Earlier steps can end the decision. Each step records a reason.

1. **Administrative shape → `NoLlm`.** No provider is considered.
2. **Owner pin or exclude.** `pin` selects a provider if it is eligible after step 3. `exclude` removes one. Both beat every prior (PLAN-020 M54: explicit Owner priority commands).
3. **Disclosure.** A provider not granted the task's data classes (CC-02 P4) is removed. If nothing is left: `Block{disclosure_denied}`.
4. **Availability.**
   - `Available`: eligible.
   - `Unknown`: eligible only with `preflight_required = true`. Unknown is never treated as Available, and Unknown quota is never read as zero.
   - `Unavailable{Capacity, retry_after}`: not eligible now; `Queue{until: retry_after}` if nothing else routes.
   - `Unavailable{AccountRestricted}`: not eligible and never queued for time, because waiting does not fix it (CC-03 §3 amendment, CC-04 §4).
   - `Unavailable{NotInstalled | NotAuthenticated | Transport}`: not eligible; blocked and reported upward, not queued (Owner decision D-4, 2026-10-25, for both providers unavailable; the same block applies when a review's only reviewer is down, by the Owner's decision of 2026-10-25, §5).
5. **Write and high-risk safety.** A provider whose isolation is `Unverified` (CC-04 §3) is ineligible for write or high-risk work. Default: Claude write eligibility is `false` until isolation is verified (proposed v1).
6. **Task-shape prior.** Proposed v1, taken from PLAN-020 M54 acceptance:
   - ambiguous or architectural (UX, domain tradeoffs, critique): prefer `claude`;
   - bounded executable (implementation, tests, known failures): prefer `codex`;
   - review: the opposite provider from the author (step 7).
   A prior only orders eligible candidates. It never makes an ineligible provider eligible.
7. **Required review (never silently waived).** A review request needs an eligible reviewer other than its author. The author is a hard exclusion, never a fallback, and `required_review` does not change that. Other work with `required_review` routes only when a second provider is eligible, because one eligible provider would run it unreviewed. When the requirement cannot be met, capacity queues with `required_review_pending` and any other cause blocks, reported upward. A review whose only reviewer is down also blocks, by the Owner's decision of 2026-10-25 (§5). A review with no known author routes nowhere (R22). Never `Route` to the author, and never drop the requirement.
8. **Outcome.** `Route{provider, preflight_required, reasons}`, `Queue{until, reasons}`, or `Block{reason}`. The decision lists every considered provider with its exclusion reasons (CC-03 §7).

Reroute rule (proposed v1, as implemented): when the top-ranked provider is ineligible only for an availability cause (capacity, account restriction, transport, install or authentication) and is otherwise selectable, the other eligible provider may take the work, and `Reroute{provider, cause}` is recorded. Only an availability cause can be recorded this way, because the F1 `Reroute` type carries an `UnavailableReason`. Owner exclude or pin, disclosure denial, review authorship and unverified isolation for write work exclude the candidate without a reroute reason; they appear in `considered`. The policy never reroutes write work to an unverified provider (step 5). Fixture: `excluded_top_provider_routes_without_a_reroute_reason`.

## 3. Reason codes (closed set for v1)

`prior:ambiguous_prefers_claude`, `prior:bounded_prefers_codex`, `owner_pin`, `owner_exclude`, `disclosure_denied`, `reroute:claude_capacity`, `reroute:codex_capacity`, `capacity_retry_after`, `account_restricted`, `preflight_required`, `isolation_unverified_write_denied`, `required_review_pending`, `administrative_no_llm`, `no_eligible_provider`.

Unknown codes are a policy-version error, not a silent default.

## 4. Test cases (fixtures; no provider calls)

| ID | Inputs (summary) | Expected | Anchor |
| --- | --- | --- | --- |
| R1 | Ambiguous UX task; both Available | `Route(claude)`, `prior:ambiguous_prefers_claude` | M54 acceptance |
| R2 | Architectural task; Claude `Capacity` with retry_after | `Route(codex)`, `reroute:claude_capacity` | M54 "suitable reroute" |
| R3 | Bounded executable task; both Available | `Route(codex)`, `prior:bounded_prefers_codex` | M54 acceptance |
| R4 | Bounded write task; Codex `Capacity`; Claude write-ineligible (default) | `Queue{until: retry_after}`, `capacity_retry_after`, `isolation_unverified_write_denied` | M54 "explicit queue on capacity loss" |
| R5 | Review of Codex-authored work; Claude Available | `Route(claude)`, `required_review_pending` cleared | cross-family review |
| R6 | Review of Codex-authored work; Claude `Unavailable{Transport}` | `Block{no_eligible_provider}` with `required_review_pending`, reported upward; never Route(codex). Owner decision, 2026-10-25 (§5). | M54 "never silently waived"; Owner decision (§5) |
| R7 | Review of Claude-authored work; Codex `Capacity` | `Queue`, `required_review_pending` | M54 "never silently waived" |
| R8 | Ambiguous task; Claude disclosure denied for the data class; Codex Available | `Route(codex)`, `disclosure_denied` on Claude | CC-02 A6 |
| R9 | Ambiguous task; Owner pin Codex; both Available | `Route(codex)`, `owner_pin` (overrides the prior) | M54 explicit Owner priority |
| R10 | Bounded task; Owner exclude Codex | `Route(claude)` only if write-eligible; otherwise `Block{no_eligible_provider}` | M54 |
| R11 | Administrative task (list, priority, resume) | `NoLlm`, `administrative_no_llm`; no provider appears in considered list | M54 "zero providers" |
| R12 | Bounded task; Codex `Unknown` | `Route(codex)` with `preflight_required = true` | Unknown is not Available |
| R13 | Bounded task; Codex `AccountRestricted` | not routed, not queued for time; `Route(claude)` only if write-eligible, otherwise `Block{account_restricted}` | CC-03 §3 amendment |
| R14 | Any task; both `Unavailable`, transport, install or authentication causes | `Block{no_eligible_provider}`, reported upward; not queued (Owner D-4). Capacity causes queue with the earliest `retry_after`. | CC-03 §9.4, D-4 |
| R15 | High-risk ambiguous task; Claude isolation `Unverified` | `Route(codex)` or `Queue`; never Claude for write or high-risk | CC-04 §3 |
| R16 | Any input, run twice, and with inputs reordered | identical decisions and reason lists | determinism |
| R17 | Any decision | `policy_version` present and matching the policy used | auditability |
| R18 | Bounded work with `required_review`; Codex available; Claude at capacity with a retry time | `Queue{until: retry}`, `required_review_pending` | CC-03 §7 relationship |
| R19 | Bounded work with `required_review`; Codex available; Claude `Transport` | `Block{no_eligible_provider}` with `required_review_pending`. Owner decision, 2026-10-25 (§5). | CC-03 §7; Owner decision (§5) |
| R20 | Review request, either flag value; the author is the only eligible candidate, or the author is at capacity | never Route(author); `Block{no_eligible_provider}` | CC-03 §7 "never waive review" |
| R21 | Owner-excluded, disclosure-denied or author candidate at capacity; the other provider unavailable or available | no Queue from the excluded candidate and no reroute reason from it | Codex CC-05 excluded-provider finding |
| R22 | Review request with no known author, either flag value; both providers available | never Route; both candidates excluded with `required_review_pending`; `Block{no_eligible_provider}` | Codex candidate review (fail closed) |
| R23 | Required review with no eligible provider. Flag set: both at capacity, or both transport-down. Flag not set: the same causes | Flag set: capacity `Queue{until}` with `required_review_pending`; transport `Block{no_eligible_provider}` with `required_review_pending` and `no_eligible_provider`. Flag not set: `capacity_retry_after` or `no_eligible_provider` only | Codex CX-07 policy finding 1 (2026-10-25) |
| R24 | Known zero quota on the preferred provider; the alternate routes | `Route(alternate)` with `Reroute{capacity}` recorded in the rationale | Codex CX-07 policy finding 2 (2026-10-25) |

Scenario walk-throughs that need an event sequence (lost acknowledgement, replacement session) stay in CC-03 §9; they are integration tests, not policy tests.

## 5. Owner decisions the defaults stand in for

These are not answerable from the repository. Each default above is what the policy does until the Owner edits this file.

- **Priors.** Confirm the ambiguous-to-Claude and bounded-to-Codex split (PLAN-020 M54 wording).
- **Claude write eligibility.** Default `false` until isolation is verified (CC-04). The Owner decides whether read-only Claude work is acceptable before that.
- **Both providers unavailable (decided, D-4, 2026-10-25).** Capacity queues with `until` when a retry time is known, otherwise with no time. Transport, install and authentication failures block and are reported upward, not queued. Account restriction blocks.
- **Required review with its only reviewer down (decided by the Owner, 2026-10-25: Block).** A review, or required review of other work, whose only independent reviewer has a transport, install or authentication failure blocks and is reported upward (R6, R19). The accepted D-4 covers only the case where both providers are unavailable, so this is a separate Owner decision. The block branch of `unrouted` in `src/routing_policy.rs` already implements it. Capacity still queues with a retry time. Account restriction, disclosure denial and isolation block as before.
- **Reroute of ambiguous work to Codex.** Default: allowed when Claude is unavailable, with `reroute:<cause>` recorded.

## 6. Integration notes (for CX-08, Codex)

- The policy returns decisions only. Persistence of availability observations, queueing and reconciliation belong to Codex's CX-08.
- Availability observations need a source and a timestamp (CC-03 §7). A stale observation degrades to `Unknown`.
- `policy_version` is stored with every routing decision, so later evaluation can attribute outcomes (M55).

## 7. Status

- Current state: implemented as a pure policy (§8), fixture-tested in the scratch crate, and registered as a pure, non-activating module (uncommitted). Codex's independent review of the candidates (`8d41732`) found an unknown-author review that could route (now R22, fail closed) and the reviewer-down case, now decided by the Owner as Block (§5). The §2 decision order and §4 cases are the v1 baseline. Changing a case needs a recorded reason and a policy version bump. The Codex candidate-review findings (2026-10-25) are addressed: `required_review` is used, and excluded, disclosure-denied and author candidates no longer queue or reroute. `policy_version` stays 1: the module is registered but non-activating, and no routing decision has been persisted, so this decision is recorded here, with its reason, rather than by a version bump.
- Codex owns registration and the CX-08 wiring. Nothing calls a provider. Neither module is called by dispatch, persistence, MCP routing or provider-start code.
- No claim of provider superiority; the priors are routing defaults, not measured results.

## 8. Implementation status (2026-10-25): pure policy, fixture-tested, registered (non-activating)

Source: `src/routing_policy.rs` (Claude-owned; registered in `src/lib.rs` as a pure, non-activating module in Codex's uncommitted candidate, with its tests in `tests/routing_policy.rs`; Codex owns the CX-08 wiring). Entry point: `decide(request, candidates, policy) -> Result<RoutingDecision, String>`, using the accepted F1 types from `cathulu_core::provider_routing` (`09f192b`).

Decision order as implemented: administrative shape returns `NoLlm` first. Candidates are validated, sorted by provider ID, and checked for duplicates. An Owner pin must name a candidate. Each candidate is then evaluated for Owner exclusion and pin, author-as-reviewer, disclosure, isolation for write or high-risk work, and availability. Unknown availability is eligible only with `preflight_required`. Known zero quota and capacity exhaustion count as capacity, not unknown. Owner preferences rank first, then the task-shape prior, then the rest by ID. The first eligible candidate is routed, with a `Reroute` reason when the top-ranked provider is down for an availability reason. Known zero quota counts as a capacity cause, so it also records `Reroute{capacity}`.

Candidates that cannot be selected for this request (Owner-excluded or pinned away, disclosure-denied, the review author, or unverified isolation for write work) never contribute a capacity wait or a reroute reason.

Required review: a review request needs an eligible reviewer other than its author, and `required_review` does not change that. Other work with `required_review` routes only when at least two providers are eligible. With fewer than two eligible providers (zero or one) it would run unreviewed, so it waits or blocks instead, and the review reason stays on the rationale.

When the required review cannot be met, or nothing is eligible: capacity queues with the earliest retry among selectable candidates, and the rationale is `required_review_pending` when a review is the reason. Any other cause blocks with the most specific shared reason (disclosure, account restriction, isolation, or `NoEligibleProvider`), and a required review adds `required_review_pending` to the rationale. Waiting cannot fix a block, so none is queued. D-4 decides this for both providers unavailable. For a review whose only reviewer is down, the Owner decided the block on 2026-10-25 (§5).

Reason set: the implemented enum (`provider_routing::RoutingReason`) is the source of truth. It includes `OwnerPreference`, `StaleObservation`, `Unavailable { reason }` and `Reroute { provider_id, reason }`, which are not in §3. `StaleObservation` is declared but not yet emitted, because an expired observation is reported as Unknown. Remove it or emit it in a later revision.

**Resolved by Owner policy (board decision D-4, 2026-10-25).** R14 said a both-unavailable task is queued; §2 said install, authentication and transport failures are not queued. The Owner's decision: a transport, install or authentication failure is blocked and reported upward, not queued silently. The implementation and its fixture (`r14_…`) already match this, so no code change is needed. The routing function returns `Block` with `NoEligibleProvider`. The escalation itself, meaning who sees the blocker and who acts on it, belongs to the Overseer layer in CX-08. It should route the blocker to the Project Manager first, then to the Main Overseer, and to the Owner only when the Owner must act. Capacity still queues with a retry time, and account restriction still blocks.

Verification (2026-10-25, after the Codex candidate-review fixes): `cargo test --manifest-path target/manual-dev/claude/scratch/Cargo.toml --offline --test routing_policy` passes 31 of 31 with no warnings at HEAD `0d33295`, after Codex's CX-07 policy findings 1 and 2 (the scratch crate depends on the checkout's `cathulu-core` by path). The new fixtures are `required_review_with_no_eligible_provider_waits_for_capacity_with_the_review_reason` and `required_review_with_no_eligible_provider_blocks_on_transport_with_the_review_reason` (R23), and `quota_exhaustion_is_capacity_and_routes_elsewhere` now asserts the `Reroute{capacity}` rationale (R24). The adapter suite passes 27 of 27 at the same head, and the two existing scratch suites pass 2/2 and 6/6. The current-head log is `target/manual-dev/claude/cx07-current-head-0d33295.log`. `rustfmt --check` is clean on the policy and both scratch tests. The new fixture `excluded_top_provider_routes_without_a_reroute_reason` pins the §2 reroute rule. Covered: R1–R17, including R7 (Claude-authored review while Codex is at capacity, which queues with `required_review_pending`), and quota exhaustion, whose reset time is carried into the queue. R14 follows Owner decision D-4. R6 and R19 block by the Owner's decision of 2026-10-25 (§5), which applies to a review whose only reviewer is down. R22 covers the unknown-author fail-closed. R18–R21 cover the required-review and excluded-candidate fixes (`required_review_*`, `review_requests_never_route_to_the_author_whatever_the_flag`, `excluded_providers_at_capacity_neither_queue_nor_reroute`, `disclosure_denied_capacity_neither_queues_nor_reroutes`). Determinism holds across candidate order (R16). Nothing calls a provider, and nothing is wired into dispatch or persistence. Registration candidate (2026-10-25, Codex, uncommitted, based on `646cb03`): `cargo test --offline --all-targets` passes 236/236 with both dispatch flags false, and the scratch suites pass 66/66 (adapter 27/27, policy 31/31, existing suites 2/2 and 6/6). `rustfmt --check` passes on all four Claude-owned files.
