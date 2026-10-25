use cathulu_core::{
    agent_provider::{
        CredentialSource, FeatureSupport, ProviderCapabilities, ProviderId, UnavailableReason,
    },
    claude_code::adapter_capabilities,
    provider_routing::{
        AvailabilityObservation, AvailabilityState, ObservationSource, QuotaObservation, Risk,
        RoutingCandidate, RoutingDecision, RoutingOutcome, RoutingPolicy, RoutingReason,
        RoutingRequest, TaskShape,
    },
    routing_policy::decide,
};

const NOW: i64 = 1_000;

fn policy() -> RoutingPolicy {
    RoutingPolicy {
        version: 1,
        ambiguous_preference: vec![ProviderId::claude()],
        bounded_preference: vec![ProviderId::codex()],
        availability_ttl_seconds: 300,
    }
}

fn codex_capabilities(verified: bool) -> ProviderCapabilities {
    let mut capabilities = adapter_capabilities(None);
    capabilities.provider_id = ProviderId::codex();
    capabilities.adapter_version = "codex-fixture".to_string();
    capabilities.credential_source = CredentialSource::Subscription;
    if verified {
        capabilities.isolation.settings = FeatureSupport::Supported;
        capabilities.isolation.plugins = FeatureSupport::Supported;
        capabilities.isolation.mcp_servers = FeatureSupport::Supported;
        capabilities.isolation.instructions = FeatureSupport::Supported;
        capabilities.isolation.memory = FeatureSupport::Supported;
    }
    capabilities
}

fn candidate(
    capabilities: ProviderCapabilities,
    state: AvailabilityState,
    disclosed: bool,
) -> RoutingCandidate {
    RoutingCandidate {
        availability: AvailabilityObservation {
            provider_id: capabilities.provider_id.clone(),
            state,
            observed_at: Some(NOW - 10),
            source: ObservationSource::Preflight,
        },
        capabilities,
        all_required_disclosures_granted: disclosed,
    }
}

fn available() -> AvailabilityState {
    AvailabilityState::Available { quota: None }
}

fn unavailable(reason: UnavailableReason, retry_at: Option<i64>) -> AvailabilityState {
    AvailabilityState::Unavailable {
        reason,
        retry_at,
        quota: None,
    }
}

fn request(task_shape: TaskShape) -> RoutingRequest {
    RoutingRequest {
        task_shape,
        risk: Risk::Low,
        requires_write: false,
        required_review: false,
        owner_pin: None,
        owner_excluded: Vec::new(),
        owner_preference: Vec::new(),
        now: NOW,
    }
}

fn claude(state: AvailabilityState, disclosed: bool) -> RoutingCandidate {
    candidate(adapter_capabilities(None), state, disclosed)
}

fn codex(state: AvailabilityState, disclosed: bool, verified: bool) -> RoutingCandidate {
    candidate(codex_capabilities(verified), state, disclosed)
}

fn routed(decision: &RoutingDecision) -> &ProviderId {
    match &decision.outcome {
        RoutingOutcome::Route { provider_id, .. } => provider_id,
        other => panic!("expected Route, got {other:?}"),
    }
}

fn considered_exclusions(decision: &RoutingDecision, provider: ProviderId) -> Vec<RoutingReason> {
    decision
        .considered
        .iter()
        .find(|c| c.provider_id == provider)
        .expect("provider was considered")
        .exclusions
        .clone()
}

#[test]
fn r1_ambiguous_work_prefers_claude_with_its_prior_reason() {
    let decision = decide(
        &request(TaskShape::Ambiguous {}),
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::claude());
    assert_eq!(decision.rationale, vec![RoutingReason::PriorAmbiguous {}]);
}

#[test]
fn r2_capacity_on_the_preferred_provider_reroutes_with_a_reason() {
    let decision = decide(
        &request(TaskShape::Architectural {}),
        &[
            claude(unavailable(UnavailableReason::Capacity, Some(1_500)), true),
            codex(available(), true, true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::codex());
    assert!(decision.rationale.contains(&RoutingReason::Reroute {
        provider_id: ProviderId::claude(),
        reason: UnavailableReason::Capacity,
    }));
}

#[test]
fn r3_bounded_work_prefers_codex() {
    let decision = decide(
        &request(TaskShape::Bounded {}),
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::codex());
    assert_eq!(decision.rationale, vec![RoutingReason::PriorBounded {}]);
}

#[test]
fn r4_write_work_queues_when_codex_is_at_capacity_and_claude_is_unverified() {
    let mut write = request(TaskShape::Bounded {});
    write.requires_write = true;
    let decision = decide(
        &write,
        &[
            codex(
                unavailable(UnavailableReason::Capacity, Some(1_500)),
                true,
                true,
            ),
            claude(available(), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Queue { until: Some(1_500) }
    );
    assert_eq!(
        decision.rationale,
        vec![RoutingReason::CapacityRetryAfter {}]
    );
    assert!(considered_exclusions(&decision, ProviderId::claude())
        .contains(&RoutingReason::IsolationUnverifiedWriteDenied {}));
}

#[test]
fn r5_review_of_codex_work_goes_to_claude_and_never_to_the_author() {
    let mut review = request(TaskShape::Review {
        author_provider: Some(ProviderId::codex()),
    });
    review.required_review = true;
    let decision = decide(
        &review,
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::claude());
    assert!(considered_exclusions(&decision, ProviderId::codex())
        .contains(&RoutingReason::RequiredReviewPending {}));
}

#[test]
fn r6_review_blocks_when_the_only_reviewer_is_down_and_is_never_waived() {
    // D-4: a transport failure blocks and is reported upward rather than queueing. The author stays
    // excluded, so the review is never routed back to Codex.
    let review = request(TaskShape::Review {
        author_provider: Some(ProviderId::codex()),
    });
    let decision = decide(
        &review,
        &[
            codex(available(), true, true),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Block {
            reason: RoutingReason::NoEligibleProvider {}
        }
    );
    assert_eq!(
        decision.rationale,
        vec![
            RoutingReason::RequiredReviewPending {},
            RoutingReason::NoEligibleProvider {}
        ]
    );
}

#[test]
fn r8_disclosure_denial_removes_the_provider_and_is_recorded() {
    let decision = decide(
        &request(TaskShape::Ambiguous {}),
        &[codex(available(), true, true), claude(available(), false)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::codex());
    assert!(considered_exclusions(&decision, ProviderId::claude())
        .contains(&RoutingReason::DisclosureDenied {}));
}

#[test]
fn r9_owner_pin_overrides_the_prior() {
    let mut pinned = request(TaskShape::Ambiguous {});
    pinned.owner_pin = Some(ProviderId::codex());
    let decision = decide(
        &pinned,
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::codex());
    assert_eq!(decision.rationale, vec![RoutingReason::OwnerPin {}]);
}

#[test]
fn r10_owner_exclusion_routes_non_write_work_to_the_other_provider() {
    let mut excluded = request(TaskShape::Bounded {});
    excluded.owner_excluded = vec![ProviderId::codex()];
    let decision = decide(
        &excluded,
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::claude());
}

#[test]
fn r11_administrative_work_never_reaches_a_provider() {
    let decision = decide(
        &request(TaskShape::Administrative {}),
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(decision.outcome, RoutingOutcome::NoLlm {});
    assert!(decision.considered.is_empty());
    assert_eq!(
        decision.rationale,
        vec![RoutingReason::AdministrativeNoLlm {}]
    );
}

#[test]
fn r12_unknown_availability_is_routed_only_with_a_preflight() {
    let decision = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(AvailabilityState::Unknown {}, true, true),
            claude(available(), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Route {
            provider_id: ProviderId::codex(),
            preflight_required: true,
        }
    );
}

#[test]
fn r13_account_restriction_is_never_queued_and_reroutes_when_possible() {
    let decision = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(
                unavailable(UnavailableReason::AccountRestricted, None),
                true,
                true,
            ),
            claude(available(), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::claude());
    assert!(decision.rationale.contains(&RoutingReason::Reroute {
        provider_id: ProviderId::codex(),
        reason: UnavailableReason::AccountRestricted,
    }));

    let blocked = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(
                unavailable(UnavailableReason::AccountRestricted, None),
                true,
                true,
            ),
            claude(
                unavailable(UnavailableReason::AccountRestricted, None),
                true,
            ),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        blocked.outcome,
        RoutingOutcome::Block {
            reason: RoutingReason::AccountRestricted {}
        }
    );
}

#[test]
fn r14_transport_failure_everywhere_blocks_with_a_reason_rather_than_queueing() {
    // Owner decision D-4 (board, 2026-10-25): transport, install and authentication failures
    // block and escalate. They are not queued (CC-05 §2 and R14).
    let decision = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(unavailable(UnavailableReason::Transport, None), true, true),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Block {
            reason: RoutingReason::NoEligibleProvider {}
        }
    );
}

#[test]
fn r15_high_risk_ambiguous_work_never_goes_to_unverified_claude() {
    let mut risky = request(TaskShape::Ambiguous {});
    risky.risk = Risk::High;
    let decision = decide(
        &risky,
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::codex());
}

#[test]
fn r16_decisions_are_deterministic_regardless_of_candidate_order() {
    let candidates = [codex(available(), true, true), claude(available(), true)];
    let reversed = [candidates[1].clone(), candidates[0].clone()];
    let forward = decide(&request(TaskShape::Ambiguous {}), &candidates, &policy()).unwrap();
    let backward = decide(&request(TaskShape::Ambiguous {}), &reversed, &policy()).unwrap();
    assert_eq!(forward, backward);
}

#[test]
fn r17_every_decision_carries_the_policy_version() {
    let decision = decide(
        &request(TaskShape::Bounded {}),
        &[codex(available(), true, true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(decision.policy_version, 1);
}

#[test]
fn quota_exhaustion_carries_its_reset_time_into_the_queue() {
    let exhausted = AvailabilityState::Available {
        quota: Some(QuotaObservation {
            remaining: 0,
            unit: "calls".to_string(),
            resets_at: Some(1_500),
        }),
    };
    let decision = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(exhausted, true, true),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Queue { until: Some(1_500) }
    );
    assert_eq!(
        decision.rationale,
        vec![RoutingReason::CapacityRetryAfter {}]
    );
}

#[test]
fn r7_review_of_claude_work_queues_while_codex_is_at_capacity() {
    let review = request(TaskShape::Review {
        author_provider: Some(ProviderId::claude()),
    });
    let decision = decide(
        &review,
        &[
            codex(
                unavailable(UnavailableReason::Capacity, Some(1_800)),
                true,
                true,
            ),
            claude(available(), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Queue { until: Some(1_800) }
    );
    assert_eq!(
        decision.rationale,
        vec![RoutingReason::RequiredReviewPending {}]
    );
}

#[test]
fn quota_exhaustion_is_capacity_and_routes_elsewhere() {
    let exhausted = AvailabilityState::Available {
        quota: Some(QuotaObservation {
            remaining: 0,
            unit: "calls".to_string(),
            resets_at: None,
        }),
    };
    let decision = decide(
        &request(TaskShape::Bounded {}),
        &[codex(exhausted, true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::claude());
    // The exhausted preferred provider is recorded as a capacity reroute, not silently skipped.
    assert_eq!(
        decision.rationale,
        vec![RoutingReason::Reroute {
            provider_id: ProviderId::codex(),
            reason: UnavailableReason::Capacity,
        }]
    );
}

#[test]
fn required_review_blocks_when_the_second_provider_is_down() {
    let mut required = request(TaskShape::Bounded {});
    required.required_review = true;
    let decision = decide(
        &required,
        &[
            codex(available(), true, true),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Block {
            reason: RoutingReason::NoEligibleProvider {}
        }
    );
    assert_eq!(
        decision.rationale,
        vec![
            RoutingReason::RequiredReviewPending {},
            RoutingReason::NoEligibleProvider {}
        ]
    );
}

#[test]
fn required_review_waits_when_the_second_provider_is_at_capacity() {
    let mut required = request(TaskShape::Bounded {});
    required.required_review = true;
    let decision = decide(
        &required,
        &[
            codex(available(), true, true),
            claude(unavailable(UnavailableReason::Capacity, Some(1_500)), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Queue { until: Some(1_500) }
    );
    assert_eq!(
        decision.rationale,
        vec![RoutingReason::RequiredReviewPending {}]
    );
}

#[test]
fn required_review_routes_when_a_second_provider_can_review_it() {
    let mut required = request(TaskShape::Bounded {});
    required.required_review = true;
    let decision = decide(
        &required,
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::codex());
    assert_eq!(decision.rationale, vec![RoutingReason::PriorBounded {}]);
}

#[test]
fn without_required_review_a_single_provider_still_routes() {
    let decision = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(available(), true, true),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&decision), &ProviderId::codex());
}

#[test]
fn excluded_providers_at_capacity_neither_queue_nor_reroute() {
    let mut excluded = request(TaskShape::Bounded {});
    excluded.owner_excluded = vec![ProviderId::codex()];
    // Codex is at capacity but Owner-excluded, so waiting for it cannot help: block, do not queue.
    let blocked = decide(
        &excluded,
        &[
            codex(
                unavailable(UnavailableReason::Capacity, Some(2_000)),
                true,
                true,
            ),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        blocked.outcome,
        RoutingOutcome::Block {
            reason: RoutingReason::NoEligibleProvider {}
        }
    );
    // Claude is chosen, and the excluded top-ranked Codex produces no capacity reroute reason.
    let routed_to_claude = decide(
        &excluded,
        &[
            codex(
                unavailable(UnavailableReason::Capacity, Some(2_000)),
                true,
                true,
            ),
            claude(available(), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&routed_to_claude), &ProviderId::claude());
    assert!(routed_to_claude.rationale.is_empty());
}

#[test]
fn review_requests_never_route_to_the_author_whatever_the_flag() {
    // Both flag values behave alike for a review: the author is never the reviewer, its capacity
    // never queues the review, and an author that is the only candidate blocks.
    for required in [false, true] {
        let mut review = request(TaskShape::Review {
            author_provider: Some(ProviderId::codex()),
        });
        review.required_review = required;

        let to_reviewer = decide(
            &review,
            &[codex(available(), true, true), claude(available(), true)],
            &policy(),
        )
        .unwrap();
        assert_eq!(routed(&to_reviewer), &ProviderId::claude());

        let author_only = decide(
            &review,
            &[
                codex(available(), true, true),
                claude(unavailable(UnavailableReason::Transport, None), true),
            ],
            &policy(),
        )
        .unwrap();
        assert_eq!(
            author_only.outcome,
            RoutingOutcome::Block {
                reason: RoutingReason::NoEligibleProvider {}
            }
        );
        assert!(considered_exclusions(&author_only, ProviderId::codex())
            .contains(&RoutingReason::RequiredReviewPending {}));

        let author_at_capacity = decide(
            &review,
            &[
                codex(
                    unavailable(UnavailableReason::Capacity, Some(2_000)),
                    true,
                    true,
                ),
                claude(unavailable(UnavailableReason::Transport, None), true),
            ],
            &policy(),
        )
        .unwrap();
        assert_eq!(
            author_at_capacity.outcome,
            RoutingOutcome::Block {
                reason: RoutingReason::NoEligibleProvider {}
            }
        );
    }
}

#[test]
fn disclosure_denied_capacity_neither_queues_nor_reroutes() {
    // Codex is at capacity but its data class is not disclosed, so waiting for it cannot help.
    let blocked = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(
                unavailable(UnavailableReason::Capacity, Some(2_000)),
                false,
                true,
            ),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        blocked.outcome,
        RoutingOutcome::Block {
            reason: RoutingReason::NoEligibleProvider {}
        }
    );
    // With Claude available, the top-ranked Codex adds no capacity reroute reason.
    let routed_to_claude = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(
                unavailable(UnavailableReason::Capacity, Some(2_000)),
                false,
                true,
            ),
            claude(available(), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&routed_to_claude), &ProviderId::claude());
    assert!(routed_to_claude.rationale.is_empty());
}

#[test]
fn required_review_with_no_eligible_provider_waits_for_capacity_with_the_review_reason() {
    // Both providers are at capacity, so the work cannot run unreviewed. The review reason is kept.
    let mut required = request(TaskShape::Bounded {});
    required.required_review = true;
    let decision = decide(
        &required,
        &[
            codex(
                unavailable(UnavailableReason::Capacity, Some(1_500)),
                true,
                true,
            ),
            claude(unavailable(UnavailableReason::Capacity, Some(1_700)), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Queue { until: Some(1_500) }
    );
    assert_eq!(
        decision.rationale,
        vec![RoutingReason::RequiredReviewPending {}]
    );

    // Without the flag the same capacity queues on the capacity reason alone.
    let plain = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(
                unavailable(UnavailableReason::Capacity, Some(1_500)),
                true,
                true,
            ),
            claude(unavailable(UnavailableReason::Capacity, Some(1_700)), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(plain.outcome, RoutingOutcome::Queue { until: Some(1_500) });
    assert_eq!(plain.rationale, vec![RoutingReason::CapacityRetryAfter {}]);
}

#[test]
fn required_review_with_no_eligible_provider_blocks_on_transport_with_the_review_reason() {
    let mut required = request(TaskShape::Bounded {});
    required.required_review = true;
    let decision = decide(
        &required,
        &[
            codex(unavailable(UnavailableReason::Transport, None), true, true),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(
        decision.outcome,
        RoutingOutcome::Block {
            reason: RoutingReason::NoEligibleProvider {}
        }
    );
    assert_eq!(
        decision.rationale,
        vec![
            RoutingReason::RequiredReviewPending {},
            RoutingReason::NoEligibleProvider {}
        ]
    );

    // Without the flag the block carries no review reason.
    let plain = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(unavailable(UnavailableReason::Transport, None), true, true),
            claude(unavailable(UnavailableReason::Transport, None), true),
        ],
        &policy(),
    )
    .unwrap();
    assert_eq!(plain.rationale, vec![RoutingReason::NoEligibleProvider {}]);
}

#[test]
fn review_with_an_unknown_author_routes_nowhere() {
    // Without the author, independence cannot be shown, so neither provider may review the work.
    for required in [false, true] {
        let mut review = request(TaskShape::Review {
            author_provider: None,
        });
        review.required_review = required;
        let decision = decide(
            &review,
            &[codex(available(), true, true), claude(available(), true)],
            &policy(),
        )
        .unwrap();
        assert_eq!(
            decision.outcome,
            RoutingOutcome::Block {
                reason: RoutingReason::NoEligibleProvider {}
            }
        );
        assert!(decision
            .rationale
            .contains(&RoutingReason::RequiredReviewPending {}));
        for provider in [ProviderId::codex(), ProviderId::claude()] {
            assert!(considered_exclusions(&decision, provider)
                .contains(&RoutingReason::RequiredReviewPending {}));
        }
    }
}

#[test]
fn excluded_top_provider_routes_without_a_reroute_reason() {
    // A reroute records only an availability cause, because the F1 `Reroute` type carries an
    // UnavailableReason. Unverified isolation and review authorship exclude the top-ranked provider,
    // and that exclusion appears in `considered` rather than in the rationale.
    let mut write = request(TaskShape::Ambiguous {});
    write.requires_write = true;
    let isolation = decide(
        &write,
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&isolation), &ProviderId::codex());
    assert!(isolation.rationale.is_empty());
    assert!(considered_exclusions(&isolation, ProviderId::claude())
        .contains(&RoutingReason::IsolationUnverifiedWriteDenied {}));

    let review = request(TaskShape::Review {
        author_provider: Some(ProviderId::codex()),
    });
    let authorship = decide(
        &review,
        &[codex(available(), true, true), claude(available(), true)],
        &policy(),
    )
    .unwrap();
    assert_eq!(routed(&authorship), &ProviderId::claude());
    assert!(authorship.rationale.is_empty());
    assert!(considered_exclusions(&authorship, ProviderId::codex())
        .contains(&RoutingReason::RequiredReviewPending {}));
}

#[test]
fn duplicate_candidates_and_unknown_pins_are_refused() {
    let duplicate = decide(
        &request(TaskShape::Bounded {}),
        &[
            codex(available(), true, true),
            codex(available(), true, true),
        ],
        &policy(),
    );
    assert!(duplicate.is_err());
    let mut pinned = request(TaskShape::Bounded {});
    pinned.owner_pin = Some(ProviderId::new("unknown").unwrap());
    assert!(decide(&pinned, &[codex(available(), true, true)], &policy()).is_err());
}
