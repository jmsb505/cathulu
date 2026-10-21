use crate::{
    events::append_external_event,
    hash::content_hash,
    workflow::{sqlite_error, EventRecord},
    Store,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

pub const OPTIMIZATION_SCHEMA_VERSION: u16 = 1;
pub const DECISION_SCHEMA_VERSION: u16 = 1;
const MAX_DECISION_STATE_BYTES: usize = 16 * 1024;
const MAX_DECISION_RESPONSE_BYTES: usize = 16 * 1024;
const MAX_CONTEXT_INPUT_BYTES: usize = 512 * 1024;
const MAX_CONTEXT_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationCapability {
    WorkflowRouting,
    DelegationRouting,
    MemoryReranking,
    ContextExtract,
    ContextCompress,
    ContextSummarize,
    ContextClassify,
}

impl OptimizationCapability {
    pub const ALL: [Self; 7] = [
        Self::WorkflowRouting,
        Self::DelegationRouting,
        Self::MemoryReranking,
        Self::ContextExtract,
        Self::ContextCompress,
        Self::ContextSummarize,
        Self::ContextClassify,
    ];

    pub fn model_kind(self) -> ModelKind {
        match self {
            Self::WorkflowRouting | Self::DelegationRouting | Self::MemoryReranking => {
                ModelKind::Decision
            }
            Self::ContextExtract
            | Self::ContextCompress
            | Self::ContextSummarize
            | Self::ContextClassify => ModelKind::Context,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Decision,
    Context,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationMode {
    #[default]
    Shadow,
    Active,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveOptimizationMode {
    Disabled,
    Shadow,
    Active,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLocality {
    Local,
    Remote,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilitySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: OptimizationMode,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider_locality: Option<ProviderLocality>,
    #[serde(default)]
    pub remote_data_allowed: bool,
}

impl Default for CapabilitySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: OptimizationMode::Shadow,
            provider: None,
            model: None,
            provider_locality: None,
            remote_data_allowed: false,
        }
    }
}

impl CapabilitySettings {
    pub fn effective_mode(&self) -> EffectiveOptimizationMode {
        if !self.enabled {
            EffectiveOptimizationMode::Disabled
        } else {
            match self.mode {
                OptimizationMode::Shadow => EffectiveOptimizationMode::Shadow,
                OptimizationMode::Active => EffectiveOptimizationMode::Active,
            }
        }
    }

    fn validate(&self, capability: OptimizationCapability) -> Result<(), String> {
        if self.provider.is_some() != self.provider_locality.is_some() {
            return Err(format!(
                "{capability:?} provider and provider_locality must be configured together"
            ));
        }
        if let Some(provider) = &self.provider {
            validate_identifier("optimization provider", provider, 80)?;
        }
        if let Some(model) = &self.model {
            validate_label("optimization model", model, 120)?;
        }
        if self.enabled && self.provider.is_none() {
            return Err(format!(
                "{capability:?} cannot be enabled without a configured provider"
            ));
        }
        if self.enabled
            && self.provider_locality == Some(ProviderLocality::Remote)
            && !self.remote_data_allowed
        {
            return Err(format!(
                "{capability:?} remote processing requires explicit remote_data_allowed"
            ));
        }
        if self.mode == OptimizationMode::Active {
            return Err("active specialist treatments are gated until M18 approval".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptimizationConfig {
    #[serde(default)]
    pub capabilities: BTreeMap<OptimizationCapability, CapabilitySettings>,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            capabilities: OptimizationCapability::ALL
                .into_iter()
                .map(|capability| (capability, CapabilitySettings::default()))
                .collect(),
        }
    }
}

impl OptimizationConfig {
    pub fn settings(&self, capability: OptimizationCapability) -> CapabilitySettings {
        self.capabilities
            .get(&capability)
            .cloned()
            .unwrap_or_default()
    }

    pub fn effective_mode(&self, capability: OptimizationCapability) -> EffectiveOptimizationMode {
        self.capabilities
            .get(&capability)
            .map(CapabilitySettings::effective_mode)
            .unwrap_or(EffectiveOptimizationMode::Disabled)
    }

    pub fn validate(&self) -> Result<(), String> {
        for (capability, settings) in &self.capabilities {
            settings.validate(*capability)?;
        }
        Ok(())
    }

    pub fn with_workflow_overrides(&self, overrides: &WorkflowOptimizationOverrides) -> Self {
        let mut merged = Self::default();
        for capability in OptimizationCapability::ALL {
            let settings = overrides
                .capabilities
                .get(&capability)
                .cloned()
                .unwrap_or_else(|| self.settings(capability));
            merged.capabilities.insert(capability, settings);
        }
        merged
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowOptimizationOverrides {
    #[serde(default)]
    pub capabilities: BTreeMap<OptimizationCapability, CapabilitySettings>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationRunTreatment {
    pub schema_version: u16,
    pub workflow_version_id: Option<i64>,
    pub workflow_content_hash: Option<String>,
    pub configuration_hash: String,
    pub configuration_state: TreatmentConfigurationState,
    pub configuration: OptimizationConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TreatmentConfigurationState {
    Valid,
    InvalidDisabledFallback,
}

impl OptimizationRunTreatment {
    pub(crate) fn new(
        configuration: OptimizationConfig,
        workflow_version_id: Option<i64>,
        workflow_content_hash: Option<String>,
        configuration_state: TreatmentConfigurationState,
    ) -> Result<Self, String> {
        configuration.validate()?;
        let serialized = serde_json::to_string(&configuration)
            .map_err(|error| format!("cannot serialize optimization configuration: {error}"))?;
        Ok(Self {
            schema_version: OPTIMIZATION_SCHEMA_VERSION,
            workflow_version_id,
            workflow_content_hash,
            configuration_hash: content_hash(&serialized),
            configuration_state,
            configuration,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionRequest {
    pub capability: OptimizationCapability,
    pub schema_version: u16,
    pub state: Value,
    pub allowed_outcomes: Vec<String>,
}

impl DecisionRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.capability.model_kind() != ModelKind::Decision {
            return Err("decision request must use a decision capability".to_string());
        }
        if self.schema_version == 0 {
            return Err("decision schema version must be greater than zero".to_string());
        }
        if !self.state.is_object() {
            return Err("decision state must be a JSON object".to_string());
        }
        if serde_json::to_vec(&self.state)
            .map_err(|error| format!("cannot serialize decision state: {error}"))?
            .len()
            > MAX_DECISION_STATE_BYTES
        {
            return Err("decision state exceeds the 16 KiB limit".to_string());
        }
        if self.allowed_outcomes.is_empty() || self.allowed_outcomes.len() > 64 {
            return Err("decision requests need between 1 and 64 allowed outcomes".to_string());
        }
        let mut seen = BTreeSet::new();
        for outcome in &self.allowed_outcomes {
            validate_identifier("decision outcome", outcome, 80)?;
            if !seen.insert(outcome) {
                return Err("decision outcomes must be unique".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingDecisionSchema {
    WorkflowClass,
    DelegationStrategy,
}

impl RoutingDecisionSchema {
    pub fn capability(self) -> OptimizationCapability {
        match self {
            Self::WorkflowClass => OptimizationCapability::WorkflowRouting,
            Self::DelegationStrategy => OptimizationCapability::DelegationRouting,
        }
    }

    pub fn allowed_outcomes(self) -> &'static [&'static str] {
        match self {
            Self::WorkflowClass => &[
                "bug_fix",
                "feature",
                "dependency_fix",
                "architecture",
                "research",
                "verification",
                "other",
                "unknown",
                "insufficient_context",
                "needs_codex",
                "needs_more_evidence",
                "not_applicable",
            ],
            Self::DelegationStrategy => &[
                "main_only",
                "sub_overseer",
                "worker",
                "sub_plus_worker",
                "unknown",
                "insufficient_context",
                "needs_codex",
                "needs_more_evidence",
                "not_applicable",
            ],
        }
    }

    pub fn request(self, state: &RoutingDecisionState) -> Result<DecisionRequest, String> {
        let request = DecisionRequest {
            capability: self.capability(),
            schema_version: DECISION_SCHEMA_VERSION,
            state: serde_json::to_value(state)
                .map_err(|error| format!("cannot encode routing decision state: {error}"))?,
            allowed_outcomes: self
                .allowed_outcomes()
                .iter()
                .map(|outcome| (*outcome).to_string())
                .collect(),
        };
        request.validate()?;
        Ok(request)
    }
}

/// A deliberately small projection of existing SQLite state for shadow routing.
/// It contains no repository path, changed-file names, task text, run summary,
/// artifact body, memory content, or transcript.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoutingDecisionState {
    pub schema_version: u16,
    pub run_status: String,
    pub task_record_present: bool,
    pub task_status: Option<String>,
    pub objective_record_present: bool,
    pub workflow_version_present: bool,
    pub project_dirty: bool,
    pub changed_path_count: u32,
    pub mandatory_check_count: u32,
    pub mandatory_failed_count: u32,
    pub mandatory_pending_count: u32,
    pub mandatory_inconclusive_count: u32,
    pub active_worker_count: u32,
    pub active_sub_overseer_count: u32,
}

impl Store {
    /// Builds decision inputs from authoritative run/project/delegation rows.
    /// Free-form task and repository content is intentionally excluded.
    pub fn routing_decision_state(&self, run_id: i64) -> Result<RoutingDecisionState, String> {
        let run = self
            .get_run(run_id)?
            .ok_or_else(|| format!("run {run_id} was not found"))?;
        let project = self
            .get_project(run.project_id)?
            .ok_or_else(|| format!("project {} for run {run_id} was not found", run.project_id))?;
        let task_status = run
            .task_id
            .map(|task_id| self.get_task(task_id))
            .transpose()?
            .flatten()
            .map(|task| task.status);
        let connection = self.connection()?;
        let (active_worker_count, active_sub_overseer_count): (i64, i64) = connection
            .raw()
            .query_row(
                "SELECT
                   COALESCE(SUM(CASE WHEN role = 'worker' THEN 1 ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN role = 'sub_overseer' THEN 1 ELSE 0 END), 0)
                 FROM delegations
                 WHERE run_id = ?1 AND status IN ('prepared', 'dispatched')",
                params![run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(sqlite_error)?;
        let mandatory_checks = run.checks.iter().filter(|check| check.mandatory);
        let mandatory_check_count = mandatory_checks.clone().count();
        let mandatory_failed_count = mandatory_checks
            .clone()
            .filter(|check| check.status == "failed")
            .count();
        let mandatory_pending_count = mandatory_checks
            .clone()
            .filter(|check| check.status == "pending")
            .count();
        let mandatory_inconclusive_count = mandatory_checks
            .filter(|check| check.status == "inconclusive")
            .count();

        Ok(RoutingDecisionState {
            schema_version: DECISION_SCHEMA_VERSION,
            run_status: run.status,
            task_record_present: task_status.is_some(),
            task_status,
            objective_record_present: run.objective_id.is_some(),
            workflow_version_present: run.workflow_version_id.is_some(),
            project_dirty: project.dirty,
            changed_path_count: bounded_count(project.changed_paths.len()),
            mandatory_check_count: bounded_count(mandatory_check_count),
            mandatory_failed_count: bounded_count(mandatory_failed_count),
            mandatory_pending_count: bounded_count(mandatory_pending_count),
            mandatory_inconclusive_count: bounded_count(mandatory_inconclusive_count),
            active_worker_count: bounded_count(active_worker_count.max(0) as usize),
            active_sub_overseer_count: bounded_count(active_sub_overseer_count.max(0) as usize),
        })
    }

    /// Runs one local-only shadow prediction and records metadata through the
    /// existing append-only run event ledger. The prediction is never returned
    /// as a routing action and cannot change run/delegation/QA state.
    pub fn run_decision_shadow_trial(
        &self,
        run_id: i64,
        invocation_id: &str,
        request: &DecisionRequest,
        model: &dyn DecisionModel,
    ) -> Result<Option<ShadowTrialReceipt>, String> {
        request.validate()?;
        if !matches!(
            request.capability,
            OptimizationCapability::WorkflowRouting | OptimizationCapability::DelegationRouting
        ) {
            return Err(
                "M16 shadow trials support workflow and delegation routing only".to_string(),
            );
        }
        let treatment = self
            .get_run_optimization_treatment(run_id)?
            .ok_or_else(|| format!("run {run_id} has no optimization treatment snapshot"))?;
        let settings = treatment.configuration.settings(request.capability);
        match settings.effective_mode() {
            EffectiveOptimizationMode::Disabled => return Ok(None),
            EffectiveOptimizationMode::Active => {
                return Err("M16 decision trials are shadow-only until M18 approval".to_string());
            }
            EffectiveOptimizationMode::Shadow => {}
        }
        let schema = match request.capability {
            OptimizationCapability::WorkflowRouting => RoutingDecisionSchema::WorkflowClass,
            OptimizationCapability::DelegationRouting => RoutingDecisionSchema::DelegationStrategy,
            _ => return Err("M16 shadow trials support routing schemas only".to_string()),
        };
        let expected_request = schema.request(&self.routing_decision_state(run_id)?)?;
        if request != &expected_request {
            return Err(
                "M16 shadow trials accept only the minimized SQLite routing-state projection"
                    .to_string(),
            );
        }
        let provider = settings
            .provider
            .clone()
            .ok_or_else(|| "shadow capability has no configured provider".to_string())?;
        let input_bytes = serde_json::to_vec(request)
            .map_err(|error| format!("cannot size decision request: {error}"))?
            .len() as u64;
        if settings.provider_locality != Some(ProviderLocality::Local) {
            let locality = settings.provider_locality.ok_or_else(|| {
                "shadow capability has no configured provider locality".to_string()
            })?;
            let invocation = OptimizationInvocation {
                invocation_id: invocation_id.to_string(),
                capability: request.capability,
                provider,
                model: settings.model,
                model_version: None,
                provider_locality: locality,
                remote_data_allowed: settings.remote_data_allowed,
                schema_version: request.schema_version,
                mode: EffectiveOptimizationMode::Shadow,
                input_bytes,
                output_bytes: 0,
                input_tokens: None,
                output_tokens: None,
                latency_ms: 0,
                status: InvocationStatus::Skipped,
                prediction: None,
                model_confidence: None,
                probabilities: BTreeMap::new(),
                fallback_used: true,
                influenced_execution: false,
            };
            let event = self.record_optimization_invocation(run_id, &invocation)?;
            return Ok(Some(ShadowTrialReceipt {
                event_id: event.id,
                status: invocation.status,
            }));
        }
        let started = Instant::now();
        let response = model.decide(request);
        let latency_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        let (status, prediction, model_confidence, probabilities, output_bytes) = match response {
            Ok(Some(result)) => {
                let response_bytes = serde_json::to_vec(&result)
                    .map(|bytes| bytes.len() as u64)
                    .unwrap_or(0);
                match result.validate_for(request) {
                    Ok(normalized) => {
                        let output_bytes = serde_json::to_vec(&normalized)
                            .map(|bytes| bytes.len() as u64)
                            .unwrap_or(response_bytes);
                        (
                            InvocationStatus::Completed,
                            Some(normalized.outcome),
                            normalized.model_confidence,
                            normalized.probabilities,
                            output_bytes,
                        )
                    }
                    Err(_) => (
                        InvocationStatus::InvalidResponse,
                        None,
                        None,
                        BTreeMap::new(),
                        response_bytes,
                    ),
                }
            }
            Ok(None) => (InvocationStatus::Skipped, None, None, BTreeMap::new(), 0),
            Err(ModelCallFailure::ProviderUnavailable) => (
                InvocationStatus::ProviderError,
                None,
                None,
                BTreeMap::new(),
                0,
            ),
            Err(ModelCallFailure::Timeout) => {
                (InvocationStatus::Timeout, None, None, BTreeMap::new(), 0)
            }
            Err(ModelCallFailure::InvalidResponse) => (
                InvocationStatus::InvalidResponse,
                None,
                None,
                BTreeMap::new(),
                0,
            ),
        };
        let invocation = OptimizationInvocation {
            invocation_id: invocation_id.to_string(),
            capability: request.capability,
            provider,
            model: settings.model,
            model_version: None,
            provider_locality: ProviderLocality::Local,
            remote_data_allowed: false,
            schema_version: request.schema_version,
            mode: EffectiveOptimizationMode::Shadow,
            input_bytes,
            output_bytes,
            input_tokens: None,
            output_tokens: None,
            latency_ms,
            status,
            prediction,
            model_confidence,
            probabilities,
            fallback_used: status != InvocationStatus::Completed,
            influenced_execution: false,
        };
        let event = self.record_optimization_invocation(run_id, &invocation)?;
        Ok(Some(ShadowTrialReceipt {
            event_id: event.id,
            status: invocation.status,
        }))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionResult {
    pub schema_version: u16,
    pub outcome: String,
    pub model_confidence: Option<f64>,
    #[serde(default)]
    pub probabilities: BTreeMap<String, f64>,
}

impl DecisionResult {
    /// Validates a bounded response and returns a copy with probability values
    /// normalized for stable telemetry. Model confidence remains a model signal.
    pub fn validate_for(&self, request: &DecisionRequest) -> Result<Self, String> {
        request.validate()?;
        if self.schema_version != request.schema_version {
            return Err("decision response schema version does not match the request".to_string());
        }
        if !request.allowed_outcomes.contains(&self.outcome) {
            return Err("decision response outcome is outside the allowed schema".to_string());
        }
        validate_probability("model confidence", self.model_confidence)?;

        let mut normalized = self.clone();
        if !normalized.probabilities.is_empty() {
            if normalized.probabilities.len() != request.allowed_outcomes.len()
                || normalized
                    .probabilities
                    .keys()
                    .any(|outcome| !request.allowed_outcomes.contains(outcome))
            {
                return Err(
                    "decision probabilities must cover exactly the allowed outcomes".to_string(),
                );
            }
            let mut total = 0.0;
            for probability in normalized.probabilities.values() {
                if !probability.is_finite() || !(0.0..=1.0).contains(probability) {
                    return Err(
                        "decision probabilities must be finite values from 0 to 1".to_string()
                    );
                }
                total += probability;
            }
            if !total.is_finite() || !(0.95..=1.05).contains(&total) {
                return Err("decision probabilities must sum to approximately 1".to_string());
            }
            let selected_probability = normalized
                .probabilities
                .get(&normalized.outcome)
                .copied()
                .ok_or_else(|| {
                "decision probabilities omit the selected outcome".to_string()
            })?;
            if normalized
                .probabilities
                .values()
                .any(|probability| *probability > selected_probability + 1e-9)
            {
                return Err(
                    "decision outcome must be one of the highest-probability choices".to_string(),
                );
            }
            for probability in normalized.probabilities.values_mut() {
                *probability /= total;
            }
        }
        Ok(normalized)
    }
}

/// Parses only Cathulu's bounded canonical response shape. Provider-specific
/// wire responses must be mapped by an adapter before reaching this boundary.
pub fn parse_decision_result(
    bytes: &[u8],
    request: &DecisionRequest,
) -> Result<DecisionResult, String> {
    if bytes.len() > MAX_DECISION_RESPONSE_BYTES {
        return Err("decision response exceeds the 16 KiB limit".to_string());
    }
    let response: DecisionResult = serde_json::from_slice(bytes)
        .map_err(|error| format!("decision response is invalid JSON: {error}"))?;
    response.validate_for(request)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCapability {
    Extract,
    Compress,
    Summarize,
    Classify,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextSource {
    pub artifact_id: i64,
    pub content_hash: String,
    pub source_ref: String,
    pub content: String,
    /// Absolute byte ranges in the original artifact represented by `content`.
    /// M17 extraction requests use these ranges to keep model citations tied to
    /// the unmodified evidence file. Earlier request versions may omit them.
    #[serde(default)]
    pub source_spans: Vec<ContextSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextRequest {
    pub capability: ContextCapability,
    pub schema_version: u16,
    pub task: String,
    pub sources: Vec<ContextSource>,
    pub max_output_bytes: usize,
}

impl ContextRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version == 0 {
            return Err("context schema version must be greater than zero".to_string());
        }
        validate_label("context task", &self.task, 2_000)?;
        if self.sources.is_empty() || self.sources.len() > 32 {
            return Err("context requests need between 1 and 32 sources".to_string());
        }
        if self.max_output_bytes == 0 || self.max_output_bytes > MAX_CONTEXT_OUTPUT_BYTES {
            return Err("context output limit must be between 1 byte and 64 KiB".to_string());
        }
        let mut total = 0usize;
        let mut artifact_ids = BTreeSet::new();
        for source in &self.sources {
            if source.artifact_id <= 0 {
                return Err("context source artifact IDs must be positive".to_string());
            }
            if !artifact_ids.insert(source.artifact_id) {
                return Err("context source artifact IDs must be unique".to_string());
            }
            validate_hash("context source content hash", &source.content_hash)?;
            validate_label("context source reference", &source.source_ref, 2_000)?;
            total = total.saturating_add(source.content.len());
            if self.schema_version >= 2 {
                if source.source_spans.len() > 128 {
                    return Err("context source has more than 128 selected spans".to_string());
                }
                let mut mapped_bytes = 0u64;
                let mut previous_end = 0u64;
                for span in &source.source_spans {
                    if span.artifact_id != source.artifact_id
                        || span.start_byte >= span.end_byte
                        || span.start_byte < previous_end
                    {
                        return Err(
                            "selected context spans must be ordered, non-overlapping ranges in their source artifact"
                                .to_string(),
                        );
                    }
                    mapped_bytes = mapped_bytes.saturating_add(span.end_byte - span.start_byte);
                    previous_end = span.end_byte;
                }
                if mapped_bytes != source.content.len() as u64 {
                    return Err(
                        "selected context spans must map byte-for-byte to their excerpt"
                            .to_string(),
                    );
                }
            }
        }
        if total > MAX_CONTEXT_INPUT_BYTES {
            return Err("context request sources exceed the 512 KiB input limit".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextResult {
    pub schema_version: u16,
    pub content: Value,
    pub source_artifact_ids: Vec<i64>,
    pub source_content_hashes: Vec<String>,
    #[serde(default)]
    pub source_spans: Vec<ContextSourceSpan>,
    pub model_confidence: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextSourceSpan {
    pub artifact_id: i64,
    pub start_byte: u64,
    pub end_byte: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCallFailure {
    ProviderUnavailable,
    Timeout,
    InvalidResponse,
}

pub trait DecisionModel: Send + Sync {
    fn decide(&self, request: &DecisionRequest)
        -> Result<Option<DecisionResult>, ModelCallFailure>;
}

pub trait ContextModel: Send + Sync {
    fn process(&self, request: &ContextRequest) -> Result<Option<ContextResult>, ModelCallFailure>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DisabledDecisionModel;

impl DecisionModel for DisabledDecisionModel {
    fn decide(
        &self,
        _request: &DecisionRequest,
    ) -> Result<Option<DecisionResult>, ModelCallFailure> {
        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DisabledContextModel;

impl ContextModel for DisabledContextModel {
    fn process(
        &self,
        _request: &ContextRequest,
    ) -> Result<Option<ContextResult>, ModelCallFailure> {
        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationStatus {
    Completed,
    Skipped,
    ProviderError,
    Timeout,
    InvalidResponse,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowTrialReceipt {
    pub event_id: i64,
    pub status: InvocationStatus,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptimizationInvocation {
    pub invocation_id: String,
    pub capability: OptimizationCapability,
    pub provider: String,
    pub model: Option<String>,
    pub model_version: Option<String>,
    pub provider_locality: ProviderLocality,
    pub remote_data_allowed: bool,
    pub schema_version: u16,
    pub mode: EffectiveOptimizationMode,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub latency_ms: u64,
    pub status: InvocationStatus,
    pub prediction: Option<String>,
    pub model_confidence: Option<f64>,
    #[serde(default)]
    pub probabilities: BTreeMap<String, f64>,
    pub fallback_used: bool,
    pub influenced_execution: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowOutcomeAssessment {
    WouldHaveHelped,
    WouldHaveHurt,
    NoMaterialDifference,
    Unclear,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OptimizationShadowReview {
    pub invocation_id: String,
    pub assessment: ShadowOutcomeAssessment,
    /// None means human correction was not observed or assessed.
    pub human_corrected_prediction: Option<bool>,
    pub evidence_event_ids: Vec<i64>,
}

impl OptimizationInvocation {
    fn validate(&self) -> Result<(), String> {
        validate_identifier("optimization invocation ID", &self.invocation_id, 100)?;
        validate_identifier("optimization provider", &self.provider, 80)?;
        if let Some(model) = &self.model {
            validate_label("optimization model", model, 120)?;
        }
        if let Some(model_version) = &self.model_version {
            validate_label("optimization model version", model_version, 120)?;
        }
        if self.schema_version == 0 {
            return Err("optimization decision schema version must be positive".to_string());
        }
        if let Some(prediction) = &self.prediction {
            validate_identifier("optimization prediction", prediction, 80)?;
        }
        validate_probability("model confidence", self.model_confidence)?;
        if self.probabilities.len() > 64 {
            return Err("optimization probabilities exceed the 64-outcome limit".to_string());
        }
        for (outcome, probability) in &self.probabilities {
            validate_identifier("optimization probability outcome", outcome, 80)?;
            if !probability.is_finite() || !(0.0..=1.0).contains(probability) {
                return Err(
                    "optimization probabilities must be finite values from 0 to 1".to_string(),
                );
            }
        }
        if self.mode == EffectiveOptimizationMode::Shadow && self.influenced_execution {
            return Err("shadow invocations cannot influence execution".to_string());
        }
        if self.provider_locality == ProviderLocality::Remote
            && self.status != InvocationStatus::Skipped
            && !self.remote_data_allowed
        {
            return Err(
                "remote invocation telemetry requires explicit egress permission".to_string(),
            );
        }
        Ok(())
    }
}

impl Store {
    pub fn record_optimization_invocation(
        &self,
        run_id: i64,
        invocation: &OptimizationInvocation,
    ) -> Result<EventRecord, String> {
        invocation.validate()?;
        let now = crate::workflow::now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let workflow_version_id: Option<i64> = transaction
            .query_row(
                "SELECT workflow_version_id FROM runs WHERE id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("run {run_id} was not found"))?;
        let treatment_payload: Option<String> = transaction
            .query_row(
                "SELECT payload_json FROM events
                 WHERE aggregate_type = 'run' AND aggregate_id = ?1 AND event_type = 'run_started'
                 ORDER BY id LIMIT 1",
                params![run_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        let configuration_hash = treatment_payload
            .map(|payload| serde_json::from_str::<Value>(&payload))
            .transpose()
            .map_err(|error| format!("stored run start event is invalid: {error}"))?
            .and_then(|payload| {
                payload
                    .get("optimizationTreatment")
                    .and_then(|treatment| treatment.get("configurationHash"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            });
        let external_event_id = format!(
            "optimization:{}",
            content_hash(&format!("{run_id}:{}", invocation.invocation_id))
        );
        let payload = json!({
            "telemetrySchemaVersion": 1,
            "invocationId": invocation.invocation_id,
            "capability": invocation.capability,
            "provider": invocation.provider,
            "model": invocation.model,
            "modelVersion": invocation.model_version,
            "providerLocality": invocation.provider_locality,
            "remoteDataAllowed": invocation.remote_data_allowed,
            "schemaVersion": invocation.schema_version,
            "workflowVersionId": workflow_version_id,
            "configurationHash": configuration_hash,
            "mode": invocation.mode,
            "inputBytes": invocation.input_bytes,
            "outputBytes": invocation.output_bytes,
            "inputTokens": invocation.input_tokens,
            "outputTokens": invocation.output_tokens,
            "latencyMs": invocation.latency_ms,
            "status": invocation.status,
            "prediction": invocation.prediction,
            "modelConfidence": invocation.model_confidence,
            "probabilities": invocation.probabilities,
            "fallbackUsed": invocation.fallback_used,
            "influencedExecution": invocation.influenced_execution,
            "payloadsPersisted": false
        });
        let inserted_id = append_external_event(
            &transaction,
            "run",
            run_id,
            "optimization_invocation_recorded",
            &external_event_id,
            &payload,
            now,
        )?;
        let event_id = if let Some(event_id) = inserted_id {
            event_id
        } else {
            let (event_id, existing_payload): (i64, String) = transaction
                .query_row(
                    "SELECT id, payload_json FROM events WHERE external_event_id = ?1",
                    params![external_event_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(sqlite_error)?;
            let requested_payload = serde_json::to_string(&payload)
                .map_err(|error| format!("cannot serialize optimization telemetry: {error}"))?;
            if requested_payload != existing_payload {
                return Err(
                    "optimization invocation ID was reused with different telemetry".to_string(),
                );
            }
            event_id
        };
        let event = transaction
            .query_row(
                "SELECT id, aggregate_type, aggregate_id, event_type, payload_json, created_at
                 FROM events WHERE id = ?1",
                params![event_id],
                crate::workflow::event_from_row,
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(event)
    }

    /// Appends a human-reviewed counterfactual assessment after a shadow run
    /// finishes. It references final run/QA events and reuses the event ledger.
    pub fn record_optimization_shadow_review(
        &self,
        run_id: i64,
        review: &OptimizationShadowReview,
    ) -> Result<EventRecord, String> {
        validate_identifier("optimization invocation ID", &review.invocation_id, 100)?;
        if review.evidence_event_ids.is_empty() || review.evidence_event_ids.len() > 32 {
            return Err("shadow reviews need between 1 and 32 evidence event IDs".to_string());
        }
        let mut evidence_event_ids = review.evidence_event_ids.clone();
        evidence_event_ids.sort_unstable();
        evidence_event_ids.dedup();
        if evidence_event_ids.len() != review.evidence_event_ids.len()
            || evidence_event_ids.iter().any(|id| *id <= 0)
        {
            return Err("shadow review evidence IDs must be unique positive integers".to_string());
        }

        let run = self
            .get_run(run_id)?
            .ok_or_else(|| format!("run {run_id} was not found"))?;
        if run.finished_at.is_none() {
            return Err("shadow reviews can be recorded only after the run finishes".to_string());
        }
        let events = self.list_events("run", &run_id.to_string())?;
        let invocation = events
            .iter()
            .find(|event| {
                event.event_type == "optimization_invocation_recorded"
                    && event.payload.get("invocationId").and_then(Value::as_str)
                        == Some(review.invocation_id.as_str())
            })
            .ok_or_else(|| "shadow review invocation does not belong to this run".to_string())?;
        if invocation.payload.get("mode").and_then(Value::as_str) != Some("shadow")
            || invocation
                .payload
                .get("influencedExecution")
                .and_then(Value::as_bool)
                != Some(false)
            || invocation.payload.get("status").and_then(Value::as_str) != Some("completed")
            || !matches!(
                invocation.payload.get("capability").and_then(Value::as_str),
                Some("workflow_routing" | "delegation_routing")
            )
            || invocation
                .payload
                .get("prediction")
                .and_then(Value::as_str)
                .is_none()
        {
            return Err(
                "only completed, non-influencing shadow predictions can be reviewed".to_string(),
            );
        }
        let referenced_events = events
            .iter()
            .filter(|event| evidence_event_ids.contains(&event.id))
            .collect::<Vec<_>>();
        if referenced_events.len() != evidence_event_ids.len()
            || !referenced_events
                .iter()
                .any(|event| event.event_type == "run_finished")
            || referenced_events.iter().any(|event| {
                !matches!(
                    event.event_type.as_str(),
                    "run_finished" | "quality_check_recorded"
                )
            })
        {
            return Err(
                "shadow review evidence must reference this run's final outcome or QA events"
                    .to_string(),
            );
        }

        let external_event_id = format!(
            "optimization-review:{}",
            content_hash(&format!("{run_id}:{}", review.invocation_id))
        );
        let payload = json!({
            "reviewSchemaVersion": 1,
            "invocationId": review.invocation_id,
            "assessment": review.assessment,
            "humanCorrectedPrediction": review.human_corrected_prediction,
            "finalRunStatus": run.status,
            "evidenceEventIds": evidence_event_ids
        });
        let now = crate::workflow::now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let inserted_id = append_external_event(
            &transaction,
            "run",
            run_id,
            "optimization_shadow_review_recorded",
            &external_event_id,
            &payload,
            now,
        )?;
        let event_id = if let Some(event_id) = inserted_id {
            event_id
        } else {
            let (event_id, existing_payload): (i64, String) = transaction
                .query_row(
                    "SELECT id, payload_json FROM events WHERE external_event_id = ?1",
                    params![external_event_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(sqlite_error)?;
            let requested_payload = serde_json::to_string(&payload)
                .map_err(|error| format!("cannot serialize shadow review: {error}"))?;
            if requested_payload != existing_payload {
                return Err("shadow review was already recorded with different content".to_string());
            }
            event_id
        };
        let event = transaction
            .query_row(
                "SELECT id, aggregate_type, aggregate_id, event_type, payload_json, created_at
                 FROM events WHERE id = ?1",
                params![event_id],
                crate::workflow::event_from_row,
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(event)
    }
}

pub(crate) fn treatment_from_run_started(
    event: &EventRecord,
) -> Result<Option<OptimizationRunTreatment>, String> {
    if event.event_type != "run_started" {
        return Ok(None);
    }
    let treatment = event.payload.get("optimizationTreatment").cloned();
    treatment
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| format!("stored optimization treatment is invalid: {error}"))
}

fn bounded_count(count: usize) -> u32 {
    count.min(u32::MAX as usize) as u32
}

fn validate_identifier(name: &str, value: &str, max_len: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > max_len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(format!("{name} must be a short identifier"));
    }
    Ok(())
}

fn validate_label(name: &str, value: &str, max_len: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > max_len || value.chars().any(char::is_control) {
        return Err(format!(
            "{name} is empty, too long, or contains control characters"
        ));
    }
    Ok(())
}

fn validate_hash(name: &str, value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{name} must be a 64-character hexadecimal SHA-256 hash"
        ));
    }
    Ok(())
}

fn validate_probability(name: &str, value: Option<f64>) -> Result<(), String> {
    if value
        .is_some_and(|probability| !probability.is_finite() || !(0.0..=1.0).contains(&probability))
    {
        return Err(format!("{name} must be a finite value from 0 to 1"));
    }
    Ok(())
}
