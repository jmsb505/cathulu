use crate::{events::append_event, hash::content_hash, workflow::EventRecord, Store};
use rmcp::schemars;
use rmcp::schemars::JsonSchema;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_PACKET_BYTES: usize = 16 * 1024;
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
