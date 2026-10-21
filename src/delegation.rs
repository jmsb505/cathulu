use crate::worktree::DelegationWorktreeState;
use crate::{
    events::append_event, hash::content_hash, telemetry::DelegationWorkerBinding,
    workflow::EventRecord, Store,
};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PACKET_BYTES: usize = 16 * 1024;
const MAX_REPORT_BYTES: usize = 16 * 1024;
const REPORT_FIELDS: &[&str] = &[
    "summary",
    "outcome",
    "changed_paths",
    "commits",
    "checks",
    "blockers",
    "artifact_refs",
    "worktree",
];

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DelegationTaskPacket {
    #[serde(alias = "schema_version")]
    pub schema_version: i64,
    pub title: String,
    pub objective: String,
    #[serde(alias = "acceptance_criteria")]
    pub acceptance_criteria: Vec<String>,
    pub constraints: Vec<String>,
    #[serde(alias = "allowed_scope")]
    pub allowed_scope: Vec<String>,
    #[serde(alias = "referenced_memory_ids")]
    pub referenced_memory_ids: Vec<i64>,
    #[serde(alias = "referenced_artifact_ids")]
    pub referenced_artifact_ids: Vec<i64>,
    #[serde(alias = "requested_report_fields")]
    pub requested_report_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerCheckResult {
    pub name: String,
    pub status: String,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerReport {
    #[serde(alias = "schema_version")]
    pub schema_version: i64,
    pub summary: String,
    pub outcome: String,
    #[serde(alias = "changed_files")]
    pub changed_files: Vec<String>,
    pub commits: Vec<String>,
    #[serde(alias = "check_results")]
    pub check_results: Vec<WorkerCheckResult>,
    pub blockers: Vec<String>,
    #[serde(alias = "artifact_ids")]
    pub artifact_ids: Vec<i64>,
    #[serde(alias = "quality_check_ids")]
    pub quality_check_ids: Vec<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkerReportReviewRecord {
    pub id: i64,
    pub decision: String,
    pub note: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkerReportQualityCheckRef {
    pub quality_check_id: i64,
    pub check_key: String,
    pub mandatory: bool,
    pub status: String,
    pub details_hash: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkerReportRecord {
    pub report_id: i64,
    pub delegation_id: i64,
    pub review_status: String,
    pub report: WorkerReport,
    pub report_hash: String,
    pub reported_at: i64,
    pub quality_check_refs: Vec<WorkerReportQualityCheckRef>,
    pub review_history: Vec<WorkerReportReviewRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkerReportReviewResult {
    pub review_id: i64,
    pub delegation_id: i64,
    pub decision: String,
    pub review_status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardDelegationSummary {
    pub id: i64,
    pub project_name: String,
    pub task_title: String,
    pub run_id: i64,
    pub role: String,
    pub status: String,
    pub parent_run_status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub report: Option<WorkerReportRecord>,
    pub worktree: Option<DelegationWorktreeState>,
    pub worker_telemetry: Option<DelegationWorkerBinding>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DelegationRecord {
    pub id: i64,
    pub project_id: i64,
    pub task_id: i64,
    pub run_id: i64,
    pub role: String,
    pub status: String,
    pub task_packet: DelegationTaskPacket,
    pub packet_hash: String,
    pub requested_model: Option<String>,
    pub requested_effort: Option<String>,
    pub external_correlation_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub lifecycle_events: Vec<EventRecord>,
    pub report: Option<WorkerReportRecord>,
    pub worktree: Option<DelegationWorktreeState>,
    pub worker_telemetry: Option<DelegationWorkerBinding>,
}

impl Store {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_delegation(
        &self,
        project_id: i64,
        task_id: i64,
        run_id: i64,
        role: &str,
        task_packet: &DelegationTaskPacket,
        idempotency_key: &str,
        requested_model: Option<&str>,
        requested_effort: Option<&str>,
        external_correlation: Option<&str>,
    ) -> Result<DelegationRecord, String> {
        if !matches!(role, "worker" | "sub_overseer") {
            return Err("delegation role must be worker or sub_overseer".to_string());
        }
        let role = role.to_string();
        let task_packet = normalize_and_validate_packet(task_packet)?;
        let packet_json = serde_json::to_string(&task_packet)
            .map_err(|error| format!("cannot serialize delegation task packet: {error}"))?;
        let packet_hash = content_hash(&packet_json);
        let idempotency_key = normalize_required("idempotency key", idempotency_key, 256)?;
        let idempotency_key_hash = content_hash(&idempotency_key);
        let requested_model = normalize_optional("requested model", requested_model, 120)?;
        let requested_effort = normalize_optional("requested effort", requested_effort, 120)?;
        let external_correlation =
            normalize_optional("external correlation value", external_correlation, 512)?;
        let external_correlation_hash = external_correlation.as_deref().map(content_hash);
        let request_json = serde_json::to_string(&json!({
            "projectId": project_id,
            "taskId": task_id,
            "runId": run_id,
            "role": role,
            "packetHash": packet_hash,
            "requestedModel": requested_model,
            "requestedEffort": requested_effort,
            "externalCorrelationHash": external_correlation_hash
        }))
        .map_err(|error| format!("cannot serialize delegation request identity: {error}"))?;
        let request_hash = content_hash(&request_json);
        let now = unix_time();
        let base_commit = current_registered_head(self, project_id)?;

        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let existing = transaction
            .query_row(
                "SELECT id, request_hash FROM delegations WHERE idempotency_key_hash = ?1",
                params![idempotency_key_hash],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?;
        if let Some((existing_id, existing_hash)) = existing {
            if existing_hash != request_hash {
                return Err(
                    "idempotency key was already used for a different delegation request"
                        .to_string(),
                );
            }
            transaction.commit().map_err(sqlite_error)?;
            return self
                .get_delegation(existing_id)?
                .ok_or_else(|| "idempotent delegation record disappeared".to_string());
        }

        let objective_id = validate_parent_links(&transaction, project_id, task_id, run_id)?;
        validate_packet_references(
            &transaction,
            project_id,
            objective_id,
            task_id,
            run_id,
            &task_packet,
            now,
        )?;

        transaction
            .execute(
                "INSERT INTO delegations
                 (project_id, task_id, run_id, role, status, packet_schema_version, packet_json,
                  packet_hash, request_hash, idempotency_key_hash, requested_model, requested_effort,
                  external_correlation_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'prepared', 1, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                params![
                    project_id,
                    task_id,
                    run_id,
                    role,
                    packet_json,
                    packet_hash,
                    request_hash,
                    idempotency_key_hash,
                    requested_model,
                    requested_effort,
                    external_correlation_hash,
                    now
                ],
            )
            .map_err(sqlite_error)?;
        let id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO delegation_worktree_states (delegation_id, base_commit, created_at)
                 VALUES (?1, ?2, ?3)",
                params![id, base_commit, now],
            )
            .map_err(sqlite_error)?;
        for memory_id in &task_packet.referenced_memory_ids {
            transaction
                .execute(
                    "INSERT INTO delegation_memory_refs (delegation_id, memory_id, created_at)
                     VALUES (?1, ?2, ?3)",
                    params![id, memory_id, now],
                )
                .map_err(sqlite_error)?;
        }
        for artifact_id in &task_packet.referenced_artifact_ids {
            transaction
                .execute(
                    "INSERT INTO delegation_artifact_refs (delegation_id, artifact_id, created_at)
                     VALUES (?1, ?2, ?3)",
                    params![id, artifact_id, now],
                )
                .map_err(sqlite_error)?;
        }
        append_event(
            &transaction,
            "delegation",
            id,
            "delegation_prepared",
            &json!({
                "projectId": project_id,
                "taskId": task_id,
                "runId": run_id,
                "role": role,
                "packetHash": packet_hash
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_delegation(id)?
            .ok_or_else(|| "prepared delegation disappeared from the database".to_string())
    }

    pub fn mark_delegation_dispatched(&self, id: i64) -> Result<DelegationRecord, String> {
        self.transition_delegation(id, "dispatched", "delegation_dispatched")
    }

    pub fn cancel_delegation(&self, id: i64) -> Result<DelegationRecord, String> {
        self.transition_delegation(id, "cancelled", "delegation_cancelled")
    }

    pub fn submit_delegation_report(
        &self,
        delegation_id: i64,
        report: &WorkerReport,
        idempotency_key: &str,
    ) -> Result<WorkerReportRecord, String> {
        let report = normalize_worker_report(report)?;
        let report_json = serde_json::to_string(&report)
            .map_err(|error| format!("cannot serialize worker report: {error}"))?;
        if report_json.len() > MAX_REPORT_BYTES {
            return Err("worker report exceeds the 16 KiB limit".to_string());
        }
        let report_hash = content_hash(&report_json);
        let idempotency_key = normalize_required("idempotency key", idempotency_key, 256)?;
        let idempotency_key_hash = content_hash(&idempotency_key);
        let request_hash = content_hash(
            &serde_json::to_string(&json!({
                "delegationId": delegation_id,
                "reportHash": report_hash
            }))
            .map_err(|error| format!("cannot serialize worker report identity: {error}"))?,
        );
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;

        let existing = transaction
            .query_row(
                "SELECT id, delegation_id, request_hash FROM worker_reports
                 WHERE idempotency_key_hash = ?1",
                params![idempotency_key_hash],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        if let Some((_, existing_delegation, existing_hash)) = existing {
            if existing_hash != request_hash {
                return Err(
                    "idempotency key was already used for a different worker report".to_string(),
                );
            }
            transaction.commit().map_err(sqlite_error)?;
            if existing_delegation != delegation_id {
                return Err("idempotency key belongs to another delegation".to_string());
            }
            return self
                .get_delegation_report(delegation_id)?
                .ok_or_else(|| "idempotent worker report disappeared".to_string());
        }

        let (delegation_status, run_id, packet_json): (String, i64, String) = transaction
            .query_row(
                "SELECT status, run_id, packet_json FROM delegations WHERE id = ?1",
                params![delegation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("delegation {delegation_id} was not found"))?;
        if delegation_status != "dispatched" {
            return Err("worker reports require a dispatched delegation".to_string());
        }
        let packet: DelegationTaskPacket = serde_json::from_str(&packet_json)
            .map_err(|error| format!("stored delegation task packet is invalid: {error}"))?;
        validate_report_scope(&report, &packet)?;

        let already_reported: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM worker_reports WHERE delegation_id = ?1)",
                params![delegation_id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if already_reported {
            return Err(format!(
                "delegation {delegation_id} already has a worker report"
            ));
        }

        let mut quality_check_snapshots = Vec::new();
        for artifact_id in &report.artifact_ids {
            let belongs_to_run: bool = transaction
                .query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM evidence_artifacts
                       WHERE id = ?1 AND run_id = ?2 AND archived_at IS NULL
                     )",
                    params![artifact_id, run_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            if !belongs_to_run {
                return Err(format!(
                    "worker report artifact {artifact_id} must be available and attached to the parent run"
                ));
            }
        }
        for quality_check_id in &report.quality_check_ids {
            let snapshot = transaction
                .query_row(
                    "SELECT check_key, mandatory, status, details_json FROM quality_checks
                     WHERE id = ?1 AND run_id = ?2",
                    params![quality_check_id, run_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, bool>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| {
                    format!(
                        "worker report quality check {quality_check_id} must belong to the parent run"
                    )
                })?;
            quality_check_snapshots.push((
                *quality_check_id,
                snapshot.0,
                snapshot.1,
                snapshot.2,
                content_hash(&snapshot.3),
            ));
        }

        transaction
            .execute(
                "INSERT INTO worker_reports
                 (delegation_id, schema_version, report_json, report_hash, request_hash,
                  idempotency_key_hash, reported_at)
                 VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    delegation_id,
                    report_json,
                    report_hash,
                    request_hash,
                    idempotency_key_hash,
                    now
                ],
            )
            .map_err(sqlite_error)?;
        let report_id = transaction.last_insert_rowid();
        for artifact_id in &report.artifact_ids {
            transaction
                .execute(
                    "INSERT INTO worker_report_artifact_refs (report_id, artifact_id, created_at)
                     VALUES (?1, ?2, ?3)",
                    params![report_id, artifact_id, now],
                )
                .map_err(sqlite_error)?;
        }
        for (quality_check_id, check_key, mandatory, status, details_hash) in
            quality_check_snapshots
        {
            transaction
                .execute(
                    "INSERT INTO worker_report_quality_check_refs
                     (report_id, quality_check_id, check_key_snapshot, mandatory_snapshot,
                      status_snapshot, details_hash_snapshot, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        report_id,
                        quality_check_id,
                        check_key,
                        mandatory,
                        status,
                        details_hash,
                        now
                    ],
                )
                .map_err(sqlite_error)?;
        }
        append_event(
            &transaction,
            "delegation",
            delegation_id,
            "delegation_report_submitted",
            &json!({
                "reportId": report_id,
                "reportHash": report_hash,
                "outcome": report.outcome,
                "artifactCount": report.artifact_ids.len(),
                "qualityCheckCount": report.quality_check_ids.len()
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_delegation_report(delegation_id)?
            .ok_or_else(|| "submitted worker report disappeared".to_string())
    }

    pub fn review_delegation_report(
        &self,
        delegation_id: i64,
        decision: &str,
        note: &str,
        idempotency_key: &str,
    ) -> Result<WorkerReportReviewResult, String> {
        let decision = decision.trim();
        if !matches!(decision, "accepted" | "rejected" | "needs_follow_up") {
            return Err(
                "report decision must be accepted, rejected, or needs_follow_up".to_string(),
            );
        }
        let note = note.trim();
        validate_text("worker report review note", note, 1_000, false)?;
        if note.len() > 2_000 {
            return Err("worker report review note exceeds 2,000 bytes".to_string());
        }
        let idempotency_key = normalize_required("idempotency key", idempotency_key, 256)?;
        let idempotency_key_hash = content_hash(&idempotency_key);
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let report_id = transaction
            .query_row(
                "SELECT id FROM worker_reports WHERE delegation_id = ?1",
                params![delegation_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("delegation {delegation_id} has no worker report to review"))?;
        let request_hash = content_hash(
            &serde_json::to_string(&json!({
                "reportId": report_id,
                "decision": decision,
                "note": note
            }))
            .map_err(|error| format!("cannot serialize worker report review: {error}"))?,
        );
        let existing = transaction
            .query_row(
                "SELECT id, request_hash, decision FROM worker_report_reviews
                 WHERE idempotency_key_hash = ?1",
                params![idempotency_key_hash],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        if let Some((review_id, existing_hash, existing_decision)) = existing {
            if existing_hash != request_hash {
                return Err(
                    "idempotency key was already used for a different review decision".to_string(),
                );
            }
            let review_status = latest_review_status(&transaction, report_id)?;
            transaction.commit().map_err(sqlite_error)?;
            return Ok(WorkerReportReviewResult {
                review_id,
                delegation_id,
                decision: existing_decision,
                review_status,
            });
        }
        transaction
            .execute(
                "INSERT INTO worker_report_reviews
                 (report_id, decision, note, request_hash, idempotency_key_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    report_id,
                    decision,
                    note,
                    request_hash,
                    idempotency_key_hash,
                    now
                ],
            )
            .map_err(sqlite_error)?;
        let review_id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "delegation",
            delegation_id,
            "delegation_report_reviewed",
            &json!({
                "reportId": report_id,
                "reviewId": review_id,
                "decision": decision
            }),
            now,
        )?;
        let review_status = latest_review_status(&transaction, report_id)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(WorkerReportReviewResult {
            review_id,
            delegation_id,
            decision: decision.to_string(),
            review_status,
        })
    }

    pub fn dashboard_delegations(
        &self,
        limit: usize,
    ) -> Result<Vec<DashboardDelegationSummary>, String> {
        let connection = self.connection()?;
        let rows = connection
            .raw()
            .prepare(
                "SELECT d.id, p.name, t.title, d.run_id, d.role, d.status, r.status, d.created_at,
                        MAX(d.updated_at, COALESCE(wr.reported_at, 0),
                            COALESCE((SELECT MAX(review.created_at) FROM worker_report_reviews review
                                      WHERE review.report_id = wr.id), 0))
                 FROM delegations d
                 JOIN projects p ON p.id = d.project_id
                 JOIN tasks t ON t.id = d.task_id
                 JOIN runs r ON r.id = d.run_id
                 LEFT JOIN worker_reports wr ON wr.delegation_id = d.id
                 ORDER BY 9 DESC, d.id DESC LIMIT ?1",
            )
            .map_err(sqlite_error)?
            .query_map(params![limit.clamp(1, 20) as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        rows.into_iter()
            .map(
                |(
                    id,
                    project_name,
                    task_title,
                    run_id,
                    role,
                    status,
                    parent_run_status,
                    created_at,
                    updated_at,
                )| {
                    Ok(DashboardDelegationSummary {
                        id,
                        project_name,
                        task_title,
                        run_id,
                        role,
                        status,
                        parent_run_status,
                        created_at,
                        updated_at,
                        report: self.get_delegation_report(id)?,
                        worktree: self.delegation_worktree_state(id)?,
                        worker_telemetry: self.delegation_worker_binding(id)?,
                    })
                },
            )
            .collect()
    }

    pub fn get_delegation(&self, id: i64) -> Result<Option<DelegationRecord>, String> {
        let connection = self.connection()?;
        let row = connection
            .raw()
            .query_row(
                "SELECT id, project_id, task_id, run_id, role, status, packet_json, packet_hash,
                        requested_model, requested_effort, external_correlation_hash, created_at, updated_at
                 FROM delegations WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, i64>(11)?,
                        row.get::<_, i64>(12)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some((
            id,
            project_id,
            task_id,
            run_id,
            role,
            status,
            packet_json,
            packet_hash,
            requested_model,
            requested_effort,
            external_correlation_hash,
            created_at,
            updated_at,
        )) = row
        else {
            return Ok(None);
        };
        let task_packet = serde_json::from_str(&packet_json)
            .map_err(|error| format!("stored delegation task packet is invalid: {error}"))?;
        let lifecycle_events = self.list_events("delegation", &id.to_string())?;
        Ok(Some(DelegationRecord {
            id,
            project_id,
            task_id,
            run_id,
            role,
            status,
            task_packet,
            packet_hash,
            requested_model,
            requested_effort,
            external_correlation_hash,
            created_at,
            updated_at,
            lifecycle_events,
            report: self.get_delegation_report(id)?,
            worktree: self.delegation_worktree_state(id)?,
            worker_telemetry: self.delegation_worker_binding(id)?,
        }))
    }

    fn get_delegation_report(
        &self,
        delegation_id: i64,
    ) -> Result<Option<WorkerReportRecord>, String> {
        let connection = self.connection()?;
        let report_row = connection
            .raw()
            .query_row(
                "SELECT id, report_json, report_hash, reported_at
                 FROM worker_reports WHERE delegation_id = ?1",
                params![delegation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some((report_id, report_json, report_hash, reported_at)) = report_row else {
            return Ok(None);
        };
        let report = serde_json::from_str(&report_json)
            .map_err(|error| format!("stored worker report is invalid: {error}"))?;
        let quality_check_refs = connection
            .raw()
            .prepare(
                "SELECT quality_check_id, check_key_snapshot, mandatory_snapshot,
                        status_snapshot, details_hash_snapshot, created_at
                 FROM worker_report_quality_check_refs WHERE report_id = ?1
                 ORDER BY quality_check_id",
            )
            .map_err(sqlite_error)?
            .query_map(params![report_id], |row| {
                Ok(WorkerReportQualityCheckRef {
                    quality_check_id: row.get(0)?,
                    check_key: row.get(1)?,
                    mandatory: row.get(2)?,
                    status: row.get(3)?,
                    details_hash: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let review_history = connection
            .raw()
            .prepare(
                "SELECT id, decision, note, created_at FROM worker_report_reviews
                 WHERE report_id = ?1 ORDER BY id",
            )
            .map_err(sqlite_error)?
            .query_map(params![report_id], |row| {
                Ok(WorkerReportReviewRecord {
                    id: row.get(0)?,
                    decision: row.get(1)?,
                    note: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let review_status = review_history
            .last()
            .map(|review| review.decision.clone())
            .unwrap_or_else(|| "reported".to_string());
        Ok(Some(WorkerReportRecord {
            report_id,
            delegation_id,
            review_status,
            report,
            report_hash,
            reported_at,
            quality_check_refs,
            review_history,
        }))
    }

    fn transition_delegation(
        &self,
        id: i64,
        to_status: &str,
        event_type: &str,
    ) -> Result<DelegationRecord, String> {
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let from_status = transaction
            .query_row(
                "SELECT status FROM delegations WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("delegation {id} was not found"))?;
        if from_status == to_status {
            transaction.commit().map_err(sqlite_error)?;
            return self
                .get_delegation(id)?
                .ok_or_else(|| format!("delegation {id} disappeared from the database"));
        }
        let valid = matches!(
            (from_status.as_str(), to_status),
            ("prepared", "dispatched" | "cancelled") | ("dispatched", "cancelled")
        );
        if !valid {
            return Err(format!(
                "delegation cannot transition from {from_status} to {to_status}"
            ));
        }
        transaction
            .execute(
                "UPDATE delegations SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![to_status, now, id],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "delegation",
            id,
            event_type,
            &json!({ "fromStatus": from_status, "toStatus": to_status }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_delegation(id)?
            .ok_or_else(|| format!("delegation {id} disappeared from the database"))
    }
}

fn current_registered_head(store: &Store, project_id: i64) -> Result<Option<String>, String> {
    let project_path = store
        .connection()?
        .raw()
        .query_row(
            "SELECT path FROM projects WHERE id = ?1",
            params![project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| format!("registered project {project_id} was not found"))?;
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .map_err(|error| format!("cannot launch Git to capture delegation baseline: {error}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    let head = String::from_utf8(output.stdout)
        .map_err(|_| "Git returned a non-UTF-8 delegation baseline".to_string())?;
    let head = head.trim();
    Ok((!head.is_empty()).then(|| head.to_string()))
}

fn normalize_and_validate_packet(
    packet: &DelegationTaskPacket,
) -> Result<DelegationTaskPacket, String> {
    let mut packet = packet.clone();
    packet.title = packet.title.trim().to_string();
    packet.objective = packet.objective.trim().to_string();
    trim_items(&mut packet.acceptance_criteria);
    trim_items(&mut packet.constraints);
    trim_items(&mut packet.allowed_scope);
    trim_items(&mut packet.requested_report_fields);

    if packet.schema_version != 1 {
        return Err("delegation task packet schema_version must be 1".to_string());
    }
    validate_text("task packet title", &packet.title, 200, true)?;
    validate_text("task packet objective", &packet.objective, 4_000, true)?;
    validate_list(
        "acceptance criteria",
        &packet.acceptance_criteria,
        20,
        500,
        true,
    )?;
    validate_list("constraints", &packet.constraints, 20, 500, false)?;
    validate_list("allowed scope", &packet.allowed_scope, 64, 500, true)?;
    validate_list(
        "requested report fields",
        &packet.requested_report_fields,
        REPORT_FIELDS.len(),
        64,
        true,
    )?;
    validate_unique("allowed scope", &packet.allowed_scope)?;
    validate_unique("requested report fields", &packet.requested_report_fields)?;
    for field in &packet.requested_report_fields {
        if !REPORT_FIELDS.contains(&field.as_str()) {
            return Err(format!("unsupported requested report field: {field}"));
        }
    }
    for scope in &packet.allowed_scope {
        validate_relative_scope(scope)?;
    }
    if packet.referenced_memory_ids.len() > 64 || packet.referenced_artifact_ids.len() > 64 {
        return Err(
            "delegation task packet may reference at most 64 memories and 64 artifacts".to_string(),
        );
    }
    if packet.referenced_memory_ids.iter().any(|id| *id <= 0)
        || packet.referenced_artifact_ids.iter().any(|id| *id <= 0)
    {
        return Err("delegation reference IDs must be positive integers".to_string());
    }
    validate_unique_ids("memory references", &packet.referenced_memory_ids)?;
    validate_unique_ids("artifact references", &packet.referenced_artifact_ids)?;

    let encoded = serde_json::to_vec(&packet)
        .map_err(|error| format!("cannot size delegation task packet: {error}"))?;
    if encoded.len() > MAX_PACKET_BYTES {
        return Err("delegation task packet exceeds the 16 KiB limit".to_string());
    }
    Ok(packet)
}

fn normalize_worker_report(report: &WorkerReport) -> Result<WorkerReport, String> {
    let mut report = report.clone();
    report.summary = report.summary.trim().to_string();
    report.outcome = report.outcome.trim().to_string();
    trim_items(&mut report.changed_files);
    trim_items(&mut report.commits);
    trim_items(&mut report.blockers);

    if report.schema_version != 1 {
        return Err("worker report schema_version must be 1".to_string());
    }
    validate_text("worker report summary", &report.summary, 4_000, true)?;
    if !matches!(
        report.outcome.as_str(),
        "completed" | "partial" | "blocked" | "failed"
    ) {
        return Err(
            "worker report outcome must be completed, partial, blocked, or failed".to_string(),
        );
    }
    if report.changed_files.len() > 128 {
        return Err("worker report may list at most 128 changed files".to_string());
    }
    for path in &mut report.changed_files {
        *path = normalize_report_path(path)?;
    }
    validate_unique("changed files", &report.changed_files)?;
    if report.commits.len() > 64 {
        return Err("worker report may list at most 64 commits".to_string());
    }
    for commit in &mut report.commits {
        *commit = commit.to_ascii_lowercase();
        if !(7..=64).contains(&commit.len()) || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(
                "worker report commits must be 7 to 64 character hexadecimal IDs".to_string(),
            );
        }
    }
    validate_unique("commits", &report.commits)?;
    if report.check_results.len() > 64 {
        return Err("worker report may include at most 64 check results".to_string());
    }
    for check in &mut report.check_results {
        check.name = check.name.trim().to_string();
        check.status = check.status.trim().to_string();
        check.summary = check.summary.trim().to_string();
        validate_text("worker check name", &check.name, 120, true)?;
        validate_text("worker check summary", &check.summary, 1_000, true)?;
        if !matches!(
            check.status.as_str(),
            "passed" | "failed" | "inconclusive" | "not_run"
        ) {
            return Err(format!("unsupported worker check status: {}", check.status));
        }
    }
    validate_unique(
        "worker check names",
        &report
            .check_results
            .iter()
            .map(|check| check.name.clone())
            .collect::<Vec<_>>(),
    )?;
    validate_list("worker report blockers", &report.blockers, 32, 1_000, false)?;
    validate_unique("worker report blockers", &report.blockers)?;
    if report.artifact_ids.len() > 64 || report.quality_check_ids.len() > 64 {
        return Err(
            "worker report may reference at most 64 artifacts and 64 quality checks".to_string(),
        );
    }
    if report
        .artifact_ids
        .iter()
        .chain(report.quality_check_ids.iter())
        .any(|id| *id <= 0)
    {
        return Err("worker report reference IDs must be positive integers".to_string());
    }
    validate_unique_ids("worker report artifact references", &report.artifact_ids)?;
    validate_unique_ids(
        "worker report quality-check references",
        &report.quality_check_ids,
    )?;
    let encoded = serde_json::to_vec(&report)
        .map_err(|error| format!("cannot size worker report: {error}"))?;
    if encoded.len() > MAX_REPORT_BYTES {
        return Err("worker report exceeds the 16 KiB limit".to_string());
    }
    Ok(report)
}

fn validate_report_scope(
    report: &WorkerReport,
    packet: &DelegationTaskPacket,
) -> Result<(), String> {
    let requested = &packet.requested_report_fields;
    for (field, present) in [
        ("changed_paths", !report.changed_files.is_empty()),
        ("commits", !report.commits.is_empty()),
        (
            "checks",
            !report.check_results.is_empty() || !report.quality_check_ids.is_empty(),
        ),
        ("blockers", !report.blockers.is_empty()),
        ("artifact_refs", !report.artifact_ids.is_empty()),
    ] {
        if present
            && !requested
                .iter()
                .any(|requested_field| requested_field == field)
        {
            return Err(format!(
                "worker report field {field} was not requested by the delegation packet"
            ));
        }
    }
    for path in &report.changed_files {
        if !packet
            .allowed_scope
            .iter()
            .any(|scope| path_matches_scope(path, scope))
        {
            return Err(format!(
                "worker report changed file is outside the allowed delegation scope: {path}"
            ));
        }
    }
    Ok(())
}

fn normalize_report_path(path: &str) -> Result<String, String> {
    let path = path.trim().replace('\\', "/");
    validate_text("worker report changed file", &path, 500, true)?;
    let is_drive_path = path.as_bytes().get(1) == Some(&b':');
    if path.starts_with('/')
        || is_drive_path
        || path
            .split('/')
            .any(|component| component.is_empty() || component == ".." || component == ".")
        || path
            .chars()
            .any(|character| matches!(character, '*' | '?' | '[' | ']'))
    {
        return Err(format!(
            "worker report changed file must be a literal repository-relative path: {path}"
        ));
    }
    Ok(path)
}

fn path_matches_scope(path: &str, scope: &str) -> bool {
    let path_segments = path.split('/').collect::<Vec<_>>();
    let scope = scope.replace('\\', "/");
    let scope_segments = scope.split('/').collect::<Vec<_>>();
    let mut matches = vec![false; path_segments.len() + 1];
    matches[0] = true;
    for scope_segment in scope_segments {
        let mut next = vec![false; path_segments.len() + 1];
        if scope_segment == "**" {
            next[0] = matches[0];
            for path_index in 1..=path_segments.len() {
                next[path_index] = matches[path_index] || next[path_index - 1];
            }
        } else {
            for path_index in 1..=path_segments.len() {
                next[path_index] = matches[path_index - 1]
                    && segment_matches(scope_segment, path_segments[path_index - 1]);
            }
        }
        matches = next;
    }
    matches[path_segments.len()]
}

fn segment_matches(pattern: &str, segment: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let segment = segment.chars().collect::<Vec<_>>();
    let mut matches = vec![false; segment.len() + 1];
    matches[0] = true;
    for pattern_char in pattern {
        let mut next = vec![false; segment.len() + 1];
        if pattern_char == '*' {
            next[0] = matches[0];
            for segment_index in 1..=segment.len() {
                next[segment_index] = matches[segment_index] || next[segment_index - 1];
            }
        } else {
            for segment_index in 1..=segment.len() {
                next[segment_index] = matches[segment_index - 1]
                    && (pattern_char == '?' || pattern_char == segment[segment_index - 1]);
            }
        }
        matches = next;
    }
    matches[segment.len()]
}

fn latest_review_status(transaction: &Transaction<'_>, report_id: i64) -> Result<String, String> {
    transaction
        .query_row(
            "SELECT decision FROM worker_report_reviews WHERE report_id = ?1 ORDER BY id DESC LIMIT 1",
            params![report_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)
        .map(|decision| decision.unwrap_or_else(|| "reported".to_string()))
}

fn validate_parent_links(
    transaction: &Transaction<'_>,
    project_id: i64,
    task_id: i64,
    run_id: i64,
) -> Result<i64, String> {
    let task_parent = transaction
        .query_row(
            "SELECT o.id, o.project_id FROM tasks t
             JOIN objectives o ON o.id = t.objective_id WHERE t.id = ?1",
            params![task_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| format!("task {task_id} was not found"))?;
    if task_parent.1 != project_id {
        return Err("delegation task does not belong to the specified project".to_string());
    }
    let run_parent = transaction
        .query_row(
            "SELECT project_id, task_id, objective_id, status FROM runs WHERE id = ?1",
            params![run_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| format!("run {run_id} was not found"))?;
    if run_parent.0 != project_id || run_parent.1 != Some(task_id) {
        return Err(
            "delegation run must belong to the project and reference the specified task"
                .to_string(),
        );
    }
    if run_parent
        .2
        .is_some_and(|objective_id| objective_id != task_parent.0)
    {
        return Err("delegation run objective does not match the specified task".to_string());
    }
    if !matches!(run_parent.3.as_str(), "running" | "blocked") {
        return Err("delegation requires a running or blocked parent run".to_string());
    }
    Ok(task_parent.0)
}

fn validate_packet_references(
    transaction: &Transaction<'_>,
    project_id: i64,
    objective_id: i64,
    task_id: i64,
    run_id: i64,
    packet: &DelegationTaskPacket,
    now: i64,
) -> Result<(), String> {
    for memory_id in &packet.referenced_memory_ids {
        let memory = transaction
            .query_row(
                "SELECT scope_type, scope_id, review_state, valid_from, valid_until,
                        superseded_by_id, invalidated_at
                 FROM memories WHERE id = ?1",
                params![memory_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("referenced memory {memory_id} was not found"))?;
        let scope_matches = match memory.0.as_str() {
            "global" => memory.1.is_none(),
            "project" => memory.1.as_deref() == Some(&project_id.to_string()),
            "objective" => memory.1.as_deref() == Some(&objective_id.to_string()),
            "task" => memory.1.as_deref() == Some(&task_id.to_string()),
            _ => false,
        };
        if !scope_matches {
            return Err(format!(
                "referenced memory {memory_id} is outside the delegation project/task scope"
            ));
        }
        if memory.2 != "accepted"
            || memory.3.is_some_and(|valid_from| valid_from > now)
            || memory.4.is_some_and(|valid_until| valid_until <= now)
            || memory.5.is_some()
            || memory.6.is_some()
        {
            return Err(format!(
                "referenced memory {memory_id} is not accepted and current"
            ));
        }
    }
    for artifact_id in &packet.referenced_artifact_ids {
        let available = transaction
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM evidence_artifacts
                   WHERE id = ?1 AND (run_id IS NULL OR run_id = ?2) AND archived_at IS NULL
                 )",
                params![artifact_id, run_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error)?;
        if !available {
            return Err(format!(
                "referenced artifact {artifact_id} is unavailable or belongs to another run"
            ));
        }
    }
    Ok(())
}

fn validate_relative_scope(path: &str) -> Result<(), String> {
    let is_drive_path = path.as_bytes().get(1) == Some(&b':');
    if path.starts_with('/')
        || path.starts_with('\\')
        || is_drive_path
        || path
            .split(['/', '\\'])
            .any(|component| component == ".." || component == ".")
    {
        return Err(format!(
            "allowed scope path must be repository-relative: {path}"
        ));
    }
    Ok(())
}

fn validate_text(name: &str, value: &str, max_chars: usize, required: bool) -> Result<(), String> {
    if (required && value.trim().is_empty()) || value.chars().count() > max_chars {
        return Err(format!(
            "{name} must be non-empty and at most {max_chars} characters"
        ));
    }
    if value.chars().any(|character| character == '\0') {
        return Err(format!("{name} cannot contain a NUL character"));
    }
    Ok(())
}

fn normalize_required(name: &str, value: &str, max_chars: usize) -> Result<String, String> {
    let value = value.trim();
    validate_text(name, value, max_chars, true)?;
    Ok(value.to_string())
}

fn normalize_optional(
    name: &str,
    value: Option<&str>,
    max_chars: usize,
) -> Result<Option<String>, String> {
    value
        .map(|value| normalize_required(name, value, max_chars))
        .transpose()
}

fn validate_list(
    name: &str,
    values: &[String],
    max_items: usize,
    max_chars: usize,
    required: bool,
) -> Result<(), String> {
    if values.len() > max_items || (required && values.is_empty()) {
        return Err(format!(
            "{name} must contain between {} and {max_items} items",
            usize::from(required)
        ));
    }
    for value in values {
        validate_text(name, value, max_chars, true)?;
    }
    Ok(())
}

fn validate_unique(name: &str, values: &[String]) -> Result<(), String> {
    if values.iter().collect::<HashSet<_>>().len() != values.len() {
        return Err(format!("{name} cannot contain duplicates"));
    }
    Ok(())
}

fn validate_unique_ids(name: &str, values: &[i64]) -> Result<(), String> {
    if values.iter().collect::<HashSet<_>>().len() != values.len() {
        return Err(format!("{name} cannot contain duplicates"));
    }
    Ok(())
}

fn trim_items(values: &mut [String]) {
    for value in values {
        *value = value.trim().to_string();
    }
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}

fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
