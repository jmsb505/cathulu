//! Pure routing inputs/outputs. Policy implementation belongs to the allocated routing lane.

use crate::agent_provider::{ProviderCapabilities, ProviderId, UnavailableReason};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskShape {
    Administrative {},
    Ambiguous {},
    Architectural {},
    Bounded {},
    Review { author_provider: Option<ProviderId> },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    Preflight,
    ErrorSignal,
    Owner,
    Probe,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuotaObservation {
    pub remaining: u64,
    pub unit: String,
    pub resets_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AvailabilityState {
    Unknown {},
    Available {
        quota: Option<QuotaObservation>,
    },
    Unavailable {
        reason: UnavailableReason,
        retry_at: Option<i64>,
        quota: Option<QuotaObservation>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvailabilityObservation {
    pub provider_id: ProviderId,
    pub state: AvailabilityState,
    pub observed_at: Option<i64>,
    pub source: ObservationSource,
}

impl AvailabilityObservation {
    pub fn effective_state(&self, now: i64, ttl_seconds: u64) -> AvailabilityState {
        if self.observed_at.map_or(true, |at| {
            at > now || now.saturating_sub(at) as u64 > ttl_seconds
        }) {
            AvailabilityState::Unknown {}
        } else {
            self.state.clone()
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingCandidate {
    pub capabilities: ProviderCapabilities,
    pub availability: AvailabilityObservation,
    /// Aggregate input from the trusted caller; true only after every required
    /// category/provider grant has been checked for this task.
    pub all_required_disclosures_granted: bool,
}

impl RoutingCandidate {
    pub fn validate(&self) -> Result<(), String> {
        self.capabilities.validate()?;
        if self.capabilities.provider_id != self.availability.provider_id {
            return Err("routing candidate provider namespace mismatch".to_string());
        }
        if !matches!(self.availability.state, AvailabilityState::Unknown {})
            && (self.availability.observed_at.is_none()
                || self.availability.source == ObservationSource::Unknown)
        {
            return Err("known availability requires a timestamp and evidence source".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingRequest {
    pub task_shape: TaskShape,
    pub risk: Risk,
    pub requires_write: bool,
    pub required_review: bool,
    pub owner_pin: Option<ProviderId>,
    pub owner_excluded: Vec<ProviderId>,
    pub owner_preference: Vec<ProviderId>,
    pub now: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingPolicy {
    pub version: u32,
    pub ambiguous_preference: Vec<ProviderId>,
    pub bounded_preference: Vec<ProviderId>,
    pub availability_ttl_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoutingReason {
    PriorAmbiguous {},
    PriorBounded {},
    OwnerPin {},
    OwnerExclude {},
    OwnerPreference {},
    DisclosureDenied {},
    CapacityRetryAfter {},
    AccountRestricted {},
    PreflightRequired {},
    IsolationUnverifiedWriteDenied {},
    RequiredReviewPending {},
    AdministrativeNoLlm {},
    NoEligibleProvider {},
    StaleObservation {},
    Unavailable {
        reason: UnavailableReason,
    },
    Reroute {
        provider_id: ProviderId,
        reason: UnavailableReason,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateDecision {
    pub provider_id: ProviderId,
    pub eligible: bool,
    pub preflight_required: bool,
    pub exclusions: Vec<RoutingReason>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RoutingOutcome {
    Route {
        provider_id: ProviderId,
        preflight_required: bool,
    },
    Queue {
        until: Option<i64>,
    },
    Block {
        reason: RoutingReason,
    },
    NoLlm {},
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingDecision {
    pub outcome: RoutingOutcome,
    pub considered: Vec<CandidateDecision>,
    pub rationale: Vec<RoutingReason>,
    pub policy_version: u32,
}
