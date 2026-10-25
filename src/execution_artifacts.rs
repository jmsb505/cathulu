//! Bounded submission envelopes around Cathulu's existing task/report authority.

use crate::agent_provider::{bounded, AttemptKey, ExternalSession, ExternalTurn, MAX_PROMPT_BYTES};
use crate::delegation::{
    normalize_and_validate_packet, normalize_worker_report, validate_report_scope,
    DelegationTaskPacket, WorkerReport,
};
pub use crate::delegation::{
    MainWorkflowBrief, ProjectManagerWorkPlan as PlanSpec, RoleWorkPlanReviewRecord as PlanReview,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const ARTIFACT_SCHEMA_VERSION: u32 = 1;
pub const MAX_RESULT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionScope {
    pub project_id: i64,
    pub task_id: i64,
    pub task_revision: i64,
    pub run_id: i64,
    pub delegation_id: i64,
    pub attempt_key: AttemptKey,
}

impl ExecutionScope {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.project_id,
            self.task_id,
            self.task_revision,
            self.run_id,
            self.delegation_id,
        ]
        .iter()
        .any(|id| *id <= 0)
        {
            return Err("execution scope IDs and task revision must be positive".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryObservation {
    pub base_commit: Option<String>,
    pub head_commit: Option<String>,
    pub dirty_fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionVerification {
    Unverified,
    Reported,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactEnvelope {
    pub schema_version: u32,
    pub artifact_id: i64,
    pub scope: ExecutionScope,
    pub parent_artifact_ids: Vec<i64>,
    pub role: String,
    pub session: Option<ExternalSession>,
    pub turn: Option<ExternalTurn>,
    pub observed_model: Option<String>,
    pub observed_harness_version: Option<String>,
    pub source_event_id: Option<i64>,
    pub repository: RepositoryObservation,
    pub workspace_lease_id: Option<i64>,
    pub created_at: i64,
    pub permission_manifest_hash: String,
    pub verification: SubmissionVerification,
    pub payload_hash: String,
    pub evidence_artifact_ids: Vec<i64>,
}

impl ArtifactEnvelope {
    pub fn validate(&self, expected_scope: &ExecutionScope) -> Result<(), String> {
        self.scope.validate()?;
        if self.schema_version != ARTIFACT_SCHEMA_VERSION
            || self.artifact_id <= 0
            || self.created_at < 0
        {
            return Err("invalid artifact schema, identity or timestamp".to_string());
        }
        if &self.scope != expected_scope {
            return Err("artifact parent/project/task/run/attempt scope mismatch".to_string());
        }
        if self.source_event_id.is_some_and(|id| id < 0)
            || self.workspace_lease_id.is_some_and(|id| id <= 0)
        {
            return Err("invalid artifact source or workspace reference".to_string());
        }
        bounded("artifact role", &self.role, 200, true)?;
        for value in [&self.observed_model, &self.observed_harness_version]
            .into_iter()
            .flatten()
        {
            bounded("observed provider version", value, 200, true)?;
        }
        for value in [
            &self.repository.base_commit,
            &self.repository.head_commit,
            &self.repository.dirty_fingerprint,
        ]
        .into_iter()
        .flatten()
        {
            bounded("repository observation", value, 256, true)?;
        }
        validate_hash("permission manifest hash", &self.permission_manifest_hash)?;
        validate_hash("payload hash", &self.payload_hash)?;
        validate_ids("parent artifacts", &self.parent_artifact_ids)?;
        if self.parent_artifact_ids.contains(&self.artifact_id) {
            return Err("artifact cannot parent itself".to_string());
        }
        validate_ids("evidence artifacts", &self.evidence_artifact_ids)?;
        if let Some(turn) = &self.turn {
            if self.session.as_ref() != Some(&turn.session) {
                return Err("artifact turn/session namespace mismatch".to_string());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextDelivery {
    Full,
    Delta,
    Reference,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextEntry {
    pub memory_id: i64,
    pub content_hash: String,
    pub delivery: ContextDelivery,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextManifest {
    pub entries: Vec<ContextEntry>,
    pub excluded_private_count: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Selected,
    Submitted,
    AgentReportedRead,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionContextReceipt {
    pub session: ExternalSession,
    pub attempt_key: AttemptKey,
    pub memory_id: i64,
    pub content_hash: String,
    pub state: DeliveryState,
    pub observed_at: i64,
}

impl ContextManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.entries.len() > 64 {
            return Err("context manifest exceeds 64 entries".to_string());
        }
        let mut seen = HashSet::new();
        for entry in &self.entries {
            if entry.memory_id <= 0 || !seen.insert(entry.memory_id) {
                return Err("invalid or duplicate context memory ID".to_string());
            }
            validate_hash("context content hash", &entry.content_hash)?;
        }
        Ok(())
    }
    pub fn content_hash(&self) -> Result<String, String> {
        self.validate()?;
        encoded_hash(self)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkOrderPayload {
    pub task_packet: DelegationTaskPacket,
    pub context_manifest: ContextManifest,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkOrder {
    pub envelope: ArtifactEnvelope,
    pub payload: WorkOrderPayload,
}

impl WorkOrder {
    pub fn new(
        mut envelope: ArtifactEnvelope,
        task_packet: DelegationTaskPacket,
        context_manifest: ContextManifest,
    ) -> Result<Self, String> {
        let payload = WorkOrderPayload {
            task_packet: normalize_and_validate_packet(&task_packet)?,
            context_manifest,
        };
        envelope.payload_hash = encoded_hash(&payload)?;
        let order = Self { envelope, payload };
        order.validate(&order.envelope.scope)?;
        Ok(order)
    }
    pub fn parse(bytes: &[u8], expected_scope: &ExecutionScope) -> Result<Self, String> {
        if bytes.len() > MAX_PROMPT_BYTES {
            return Err("work order exceeds 64 KiB".to_string());
        }
        let order: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        order.validate(expected_scope)?;
        Ok(order)
    }
    pub fn validate(&self, expected_scope: &ExecutionScope) -> Result<(), String> {
        self.envelope.validate(expected_scope)?;
        if self.envelope.verification != SubmissionVerification::Unverified {
            return Err("work order must remain unverified".to_string());
        }
        self.payload.context_manifest.validate()?;
        if normalize_and_validate_packet(&self.payload.task_packet)? != self.payload.task_packet {
            return Err("work order task packet is not canonical".to_string());
        }
        for memory_id in &self.payload.task_packet.referenced_memory_ids {
            if !self
                .payload
                .context_manifest
                .entries
                .iter()
                .any(|entry| entry.memory_id == *memory_id)
            {
                return Err("referenced task memory is absent from context manifest".to_string());
            }
        }
        if self
            .payload
            .task_packet
            .referenced_artifact_ids
            .iter()
            .any(|id| !self.envelope.evidence_artifact_ids.contains(id))
        {
            return Err("task artifact is absent from envelope evidence references".to_string());
        }
        if encoded_hash(&self.payload)? != self.envelope.payload_hash {
            return Err("work order payload hash mismatch".to_string());
        }
        ensure_encoded_size(self, MAX_PROMPT_BYTES, "work order")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkResult {
    pub envelope: ArtifactEnvelope,
    pub report: WorkerReport,
}

impl WorkResult {
    pub fn new(mut envelope: ArtifactEnvelope, report: WorkerReport) -> Result<Self, String> {
        let report = normalize_worker_report(&report)?;
        envelope.payload_hash = encoded_hash(&report)?;
        let result = Self { envelope, report };
        result.validate(&result.envelope.scope)?;
        Ok(result)
    }
    pub fn parse(bytes: &[u8], order: &WorkOrder) -> Result<Self, String> {
        if bytes.len() > MAX_RESULT_BYTES {
            return Err("work result exceeds 16 KiB".to_string());
        }
        let result: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        result.validate_against(order)?;
        Ok(result)
    }
    pub fn validate(&self, expected_scope: &ExecutionScope) -> Result<(), String> {
        self.envelope.validate(expected_scope)?;
        if self.envelope.verification != SubmissionVerification::Reported {
            return Err("work result must be reported evidence, not accepted state".to_string());
        }
        if self.envelope.session.is_none() {
            return Err("work result requires provider session attribution".to_string());
        }
        if normalize_worker_report(&self.report)? != self.report {
            return Err("work result report is not canonical".to_string());
        }
        if encoded_hash(&self.report)? != self.envelope.payload_hash {
            return Err("work result payload hash mismatch".to_string());
        }
        if self
            .report
            .artifact_ids
            .iter()
            .any(|id| !self.envelope.evidence_artifact_ids.contains(id))
        {
            return Err(
                "reported artifact is absent from envelope evidence references".to_string(),
            );
        }
        ensure_encoded_size(self, MAX_RESULT_BYTES, "work result")
    }
    pub fn validate_against(&self, order: &WorkOrder) -> Result<(), String> {
        order.validate(&order.envelope.scope)?;
        self.validate(&order.envelope.scope)?;
        if self.envelope.artifact_id == order.envelope.artifact_id
            || !self
                .envelope
                .parent_artifact_ids
                .contains(&order.envelope.artifact_id)
        {
            return Err("work result must reference its distinct work-order parent".to_string());
        }
        if self.envelope.permission_manifest_hash != order.envelope.permission_manifest_hash
            || self.envelope.role != order.envelope.role
            || self.envelope.workspace_lease_id != order.envelope.workspace_lease_id
            || order
                .envelope
                .session
                .as_ref()
                .is_some_and(|session| self.envelope.session.as_ref() != Some(session))
        {
            return Err(
                "work result permission/role/session/lease differs from work order".to_string(),
            );
        }
        validate_report_scope(&self.report, &order.payload.task_packet)
    }
}

fn validate_hash(label: &str, value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}
fn validate_ids(label: &str, ids: &[i64]) -> Result<(), String> {
    let unique: HashSet<i64> = ids.iter().copied().collect();
    if ids.len() > 64 || unique.len() != ids.len() || ids.iter().any(|id| *id <= 0) {
        return Err(format!("invalid {label}"));
    }
    Ok(())
}
fn encoded_hash(value: &impl Serialize) -> Result<String, String> {
    Ok(crate::hash::content_hash(
        &serde_json::to_string(value).map_err(|error| error.to_string())?,
    ))
}
fn ensure_encoded_size(value: &impl Serialize, maximum: usize, label: &str) -> Result<(), String> {
    if serde_json::to_vec(value)
        .map_err(|error| error.to_string())?
        .len()
        > maximum
    {
        return Err(format!("{label} exceeds its byte bound"));
    }
    Ok(())
}
