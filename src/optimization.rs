use crate::{
    events::append_external_event,
    hash::content_hash,
    workflow::{sqlite_error, EventRecord},
    Store,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const OPTIMIZATION_SCHEMA_VERSION: u16 = 1;
const MAX_DECISION_STATE_BYTES: usize = 16 * 1024;
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
        for outcome in &self.allowed_outcomes {
            validate_identifier("decision outcome", outcome, 80)?;
        }
        Ok(())
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
        for source in &self.sources {
            if source.artifact_id <= 0 {
                return Err("context source artifact IDs must be positive".to_string());
            }
            validate_hash("context source content hash", &source.content_hash)?;
            validate_label("context source reference", &source.source_ref, 2_000)?;
            total = total.saturating_add(source.content.len());
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
