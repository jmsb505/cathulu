//! Pure provider routing policy (PLAN-021 CC-05, M54 preparation). No I/O: the caller supplies the
//! request, the observed candidates and the policy, then records the decision. Decision order and
//! reason codes follow `docs/parallel/claude/CC-05-routing-policy-design.md`.

use crate::agent_provider::{ProviderId, UnavailableReason};
use crate::provider_routing::{
    AvailabilityState, CandidateDecision, Risk, RoutingCandidate, RoutingDecision, RoutingOutcome,
    RoutingPolicy, RoutingReason, RoutingRequest, TaskShape,
};

struct Evaluation {
    id: ProviderId,
    decision: CandidateDecision,
    retry_at: Option<i64>,
    unavailable: Option<UnavailableReason>,
}

/// Decides where a request goes. The same inputs and policy version always give the same decision.
pub fn decide(
    request: &RoutingRequest,
    candidates: &[RoutingCandidate],
    policy: &RoutingPolicy,
) -> Result<RoutingDecision, String> {
    if matches!(request.task_shape, TaskShape::Administrative {}) {
        return Ok(RoutingDecision {
            outcome: RoutingOutcome::NoLlm {},
            considered: Vec::new(),
            rationale: vec![RoutingReason::AdministrativeNoLlm {}],
            policy_version: policy.version,
        });
    }
    let mut sorted: Vec<&RoutingCandidate> = candidates.iter().collect();
    for candidate in &sorted {
        candidate.validate()?;
    }
    sorted.sort_by(|a, b| a.capabilities.provider_id.cmp(&b.capabilities.provider_id));
    if sorted
        .windows(2)
        .any(|pair| pair[0].capabilities.provider_id == pair[1].capabilities.provider_id)
    {
        return Err("duplicate routing candidate provider".to_string());
    }
    if let Some(pin) = &request.owner_pin {
        if !sorted.iter().any(|c| &c.capabilities.provider_id == pin) {
            return Err("owner pin is not a routing candidate".to_string());
        }
    }

    let author = match &request.task_shape {
        TaskShape::Review { author_provider } => author_provider.clone(),
        _ => None,
    };
    let evaluated: Vec<Evaluation> = sorted
        .iter()
        .map(|candidate| evaluate(request, policy, candidate, author.as_ref()))
        .collect();

    // Owner preferences rank first, then the task-shape prior. Unranked candidates follow by ID.
    let (prior, prior_reason) = match request.task_shape {
        TaskShape::Ambiguous {} | TaskShape::Architectural {} => (
            &policy.ambiguous_preference,
            RoutingReason::PriorAmbiguous {},
        ),
        _ => (&policy.bounded_preference, RoutingReason::PriorBounded {}),
    };
    let mut ranking: Vec<(ProviderId, RoutingReason)> = request
        .owner_preference
        .iter()
        .map(|id| (id.clone(), RoutingReason::OwnerPreference {}))
        .collect();
    ranking.extend(prior.iter().map(|id| (id.clone(), prior_reason.clone())));

    let mut order: Vec<usize> = Vec::new();
    for (id, _) in &ranking {
        if let Some(index) = evaluated.iter().position(|e| &e.id == id) {
            if !order.contains(&index) {
                order.push(index);
            }
        }
    }
    let unranked: Vec<usize> = (0..evaluated.len())
        .filter(|index| !order.contains(index))
        .collect();
    order.extend(unranked);

    let chosen = order
        .iter()
        .copied()
        .find(|&index| evaluated[index].decision.eligible);
    let considered: Vec<CandidateDecision> = evaluated.iter().map(|e| e.decision.clone()).collect();

    // A required review is never waived. A review request needs a reviewer other than its author,
    // and `required_review` does not change that. Other work with a required review runs only when
    // a second provider could review it, so fewer than two eligible providers waits or is blocked.
    let eligible = evaluated.iter().filter(|e| e.decision.eligible).count();
    let review_unmet = if matches!(request.task_shape, TaskShape::Review { .. }) {
        eligible == 0
    } else {
        request.required_review && eligible < 2
    };
    if review_unmet {
        return Ok(unrouted(&evaluated, considered, true, policy.version));
    }

    if let Some(index) = chosen {
        let winner = &evaluated[index];
        let mut rationale = Vec::new();
        if request.owner_pin.is_some() {
            rationale.push(RoutingReason::OwnerPin {});
        } else if let Some((_, reason)) = ranking.iter().find(|(id, _)| id == &winner.id) {
            rationale.push(reason.clone());
        }
        // Reroute only when the top-ranked provider is down for an availability reason.
        if let Some((top_id, _)) = ranking.first() {
            if let Some(top) = evaluated.iter().find(|e| &e.id == top_id) {
                if top.id != winner.id {
                    if let Some(reason) = top.unavailable {
                        rationale.push(RoutingReason::Reroute {
                            provider_id: top.id.clone(),
                            reason,
                        });
                    }
                }
            }
        }
        return Ok(RoutingDecision {
            outcome: RoutingOutcome::Route {
                provider_id: winner.id.clone(),
                preflight_required: winner.decision.preflight_required,
            },
            considered,
            rationale,
            policy_version: policy.version,
        });
    }

    Ok(unrouted(&evaluated, considered, false, policy.version))
}

/// Nothing routes now. A capacity wait is queued with the earliest known retry time. Any other cause
/// blocks with its most specific reason and is reported upward, never queued (Owner decision D-4, and
/// the 2026-10-25 Owner decision that a review whose only reviewer is down also blocks).
fn unrouted(
    evaluated: &[Evaluation],
    considered: Vec<CandidateDecision>,
    review_pending: bool,
    policy_version: u32,
) -> RoutingDecision {
    if evaluated.iter().any(waitable) {
        let reason = if review_pending {
            RoutingReason::RequiredReviewPending {}
        } else {
            RoutingReason::CapacityRetryAfter {}
        };
        return RoutingDecision {
            outcome: RoutingOutcome::Queue {
                until: earliest_capacity_retry(evaluated),
            },
            considered,
            rationale: vec![reason],
            policy_version,
        };
    }
    // Waiting cannot fix these, so the request is blocked with the most specific cause.
    let blocked_by = |reason: &RoutingReason| {
        !evaluated.is_empty()
            && evaluated
                .iter()
                .all(|e| e.decision.exclusions.contains(reason))
    };
    let reason = if blocked_by(&RoutingReason::DisclosureDenied {}) {
        RoutingReason::DisclosureDenied {}
    } else if blocked_by(&RoutingReason::AccountRestricted {}) {
        RoutingReason::AccountRestricted {}
    } else if blocked_by(&RoutingReason::IsolationUnverifiedWriteDenied {}) {
        RoutingReason::IsolationUnverifiedWriteDenied {}
    } else {
        RoutingReason::NoEligibleProvider {}
    };
    let mut rationale = Vec::new();
    if review_pending {
        rationale.push(RoutingReason::RequiredReviewPending {});
    }
    rationale.push(reason.clone());
    RoutingDecision {
        outcome: RoutingOutcome::Block { reason },
        considered,
        rationale,
        policy_version,
    }
}

fn evaluate(
    request: &RoutingRequest,
    policy: &RoutingPolicy,
    candidate: &RoutingCandidate,
    author: Option<&ProviderId>,
) -> Evaluation {
    let id = candidate.capabilities.provider_id.clone();
    let mut exclusions = Vec::new();
    let mut retry_at = None;
    let mut unavailable = None;
    let mut preflight_required = false;

    if request.owner_excluded.contains(&id) {
        exclusions.push(RoutingReason::OwnerExclude {});
    }
    if request.owner_pin.as_ref().is_some_and(|pin| pin != &id) {
        exclusions.push(RoutingReason::OwnerPin {});
    }
    // A provider never reviews work it authored.
    if author == Some(&id) {
        exclusions.push(RoutingReason::RequiredReviewPending {});
    }
    // A review with no known author cannot show that its reviewer is independent, so it routes nowhere.
    if matches!(
        request.task_shape,
        TaskShape::Review {
            author_provider: None
        }
    ) {
        exclusions.push(RoutingReason::RequiredReviewPending {});
    }
    if !candidate.all_required_disclosures_granted {
        exclusions.push(RoutingReason::DisclosureDenied {});
    }
    if (request.requires_write || request.risk == Risk::High)
        && !candidate.capabilities.isolation.verified()
    {
        exclusions.push(RoutingReason::IsolationUnverifiedWriteDenied {});
    }
    match candidate
        .availability
        .effective_state(request.now, policy.availability_ttl_seconds)
    {
        // Unknown availability is never Available: it is eligible only after a preflight.
        AvailabilityState::Unknown {} => preflight_required = true,
        AvailabilityState::Available { quota } => {
            // A known remaining quota of zero is exhaustion, not an unknown value. Its reset time
            // is the retry time, so a queued request keeps the earliest known resume point.
            if let Some(quota) = quota.filter(|quota| quota.remaining == 0) {
                exclusions.push(RoutingReason::CapacityRetryAfter {});
                retry_at = quota.resets_at;
                // Exhaustion is a capacity cause, so routing to another provider records it as a reroute.
                unavailable = Some(UnavailableReason::Capacity);
            }
        }
        AvailabilityState::Unavailable {
            reason,
            retry_at: at,
            ..
        } => {
            unavailable = Some(reason);
            match reason {
                UnavailableReason::Capacity => {
                    retry_at = at;
                    exclusions.push(RoutingReason::CapacityRetryAfter {});
                }
                UnavailableReason::AccountRestricted => {
                    exclusions.push(RoutingReason::AccountRestricted {});
                }
                other => exclusions.push(RoutingReason::Unavailable { reason: other }),
            }
        }
    }

    // A candidate this request cannot select is neither a reroute source nor a reason to wait.
    if !may_be_selected(&exclusions) {
        unavailable = None;
        retry_at = None;
    }

    Evaluation {
        id: id.clone(),
        decision: CandidateDecision {
            provider_id: id,
            eligible: exclusions.is_empty(),
            preflight_required,
            exclusions,
        },
        retry_at,
        unavailable,
    }
}

/// Exclusions that keep a candidate from being selected for this request. Waiting cannot fix them.
fn may_be_selected(exclusions: &[RoutingReason]) -> bool {
    !exclusions.iter().any(|reason| {
        matches!(
            reason,
            RoutingReason::OwnerExclude {}
                | RoutingReason::OwnerPin {}
                | RoutingReason::DisclosureDenied {}
                | RoutingReason::RequiredReviewPending {}
                | RoutingReason::IsolationUnverifiedWriteDenied {}
        )
    })
}

/// A capacity exclusion is worth waiting for only when the candidate could be selected.
fn waitable(evaluation: &Evaluation) -> bool {
    evaluation
        .decision
        .exclusions
        .contains(&RoutingReason::CapacityRetryAfter {})
        && may_be_selected(&evaluation.decision.exclusions)
}

fn earliest_capacity_retry(evaluated: &[Evaluation]) -> Option<i64> {
    evaluated
        .iter()
        .filter(|e| waitable(e))
        .filter_map(|e| e.retry_at)
        .min()
}
