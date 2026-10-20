use crate::{events::append_event, hash::content_hash, Store};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Objective {
    pub id: i64,
    pub project_id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub objective_id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub id: i64,
    pub project_id: i64,
    pub objective_id: Option<i64>,
    pub title: String,
    pub body: String,
    pub supersedes_decision_id: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowVersion {
    pub id: i64,
    pub workflow_id: i64,
    pub workflow_key: String,
    pub name: String,
    pub version: i64,
    pub definition: Value,
    pub content_hash: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QualityCheck {
    pub id: i64,
    pub run_id: i64,
    pub check_key: String,
    pub mandatory: bool,
    pub status: String,
    pub details: Value,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub id: i64,
    pub project_id: i64,
    pub objective_id: Option<i64>,
    pub task_id: Option<i64>,
    pub workflow_version_id: Option<i64>,
    pub status: String,
    pub summary: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub checks: Vec<QualityCheck>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceArtifact {
    pub id: i64,
    pub run_id: Option<i64>,
    pub artifact_kind: String,
    pub uri: String,
    pub content_hash: Option<String>,
    pub protected: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDependency {
    pub id: i64,
    pub from_project_id: i64,
    pub to_project_id: i64,
    pub relation: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EventRecord {
    pub id: i64,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub payload: Value,
    pub created_at: i64,
}

impl Store {
    pub fn record_dependency(
        &self,
        from_project_id: i64,
        to_project_id: i64,
        relation: &str,
    ) -> Result<ProjectDependency, String> {
        if from_project_id == to_project_id {
            return Err("a project cannot depend on itself".to_string());
        }
        if !matches!(
            relation,
            "depends_on" | "consumes" | "produces" | "shares_contract_with"
        ) {
            return Err("unsupported project dependency relation".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO project_dependencies (from_project_id, to_project_id, relation, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![from_project_id, to_project_id, relation, now],
            )
            .map_err(sqlite_error)?;
        let dependency = transaction
            .query_row(
                "SELECT id, from_project_id, to_project_id, relation, created_at
                 FROM project_dependencies WHERE from_project_id = ?1 AND to_project_id = ?2 AND relation = ?3",
                params![from_project_id, to_project_id, relation],
                dependency_from_row,
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "project_dependency",
            dependency.id,
            "project_dependency_recorded",
            &json!({
                "fromProjectId": from_project_id,
                "toProjectId": to_project_id,
                "relation": relation
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(dependency)
    }

    pub fn get_dependency(&self, id: i64) -> Result<Option<ProjectDependency>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT id, from_project_id, to_project_id, relation, created_at
                 FROM project_dependencies WHERE id = ?1",
                params![id],
                dependency_from_row,
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn create_objective(
        &self,
        project_id: i64,
        title: &str,
        description: &str,
    ) -> Result<Objective, String> {
        validate_text("objective title", title, 240, true)?;
        validate_text("objective description", description, 8_000, false)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO objectives (project_id, title, description, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![project_id, title.trim(), description.trim(), now],
            )
            .map_err(sqlite_error)?;
        let id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "objective",
            id,
            "objective_created",
            &json!({ "projectId": project_id, "title": title.trim() }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_objective(id)?
            .ok_or_else(|| "created objective disappeared from the database".to_string())
    }

    pub fn set_objective_status(&self, id: i64, status: &str) -> Result<Objective, String> {
        if !matches!(
            status,
            "active" | "blocked" | "completed" | "cancelled" | "archived"
        ) {
            return Err("unsupported objective status".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let current = transaction
            .query_row(
                "SELECT status FROM objectives WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("objective {id} was not found"))?;
        if current != status {
            transaction
                .execute(
                    "UPDATE objectives SET status = ?1, updated_at = ?2 WHERE id = ?3",
                    params![status, now, id],
                )
                .map_err(sqlite_error)?;
            append_event(
                &transaction,
                "objective",
                id,
                "objective_status_changed",
                &json!({ "from": current, "to": status }),
                now,
            )?;
        }
        transaction.commit().map_err(sqlite_error)?;
        self.get_objective(id)?
            .ok_or_else(|| format!("objective {id} disappeared from the database"))
    }

    pub fn get_objective(&self, id: i64) -> Result<Option<Objective>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT id, project_id, title, description, status, created_at, updated_at
                 FROM objectives WHERE id = ?1",
                params![id],
                objective_from_row,
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn create_task(
        &self,
        objective_id: i64,
        title: &str,
        description: &str,
    ) -> Result<Task, String> {
        validate_text("task title", title, 240, true)?;
        validate_text("task description", description, 8_000, false)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO tasks (objective_id, title, description, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![objective_id, title.trim(), description.trim(), now],
            )
            .map_err(sqlite_error)?;
        let id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "task",
            id,
            "task_created",
            &json!({ "objectiveId": objective_id, "title": title.trim() }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(id)?
            .ok_or_else(|| "created task disappeared from the database".to_string())
    }

    pub fn set_task_status(&self, id: i64, status: &str) -> Result<Task, String> {
        if !matches!(
            status,
            "queued" | "active" | "blocked" | "completed" | "failed" | "cancelled"
        ) {
            return Err("unsupported task status".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let current = transaction
            .query_row(
                "SELECT status FROM tasks WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("task {id} was not found"))?;
        if current != status {
            transaction
                .execute(
                    "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
                    params![status, now, id],
                )
                .map_err(sqlite_error)?;
            append_event(
                &transaction,
                "task",
                id,
                "task_status_changed",
                &json!({ "from": current, "to": status }),
                now,
            )?;
        }
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(id)?
            .ok_or_else(|| format!("task {id} disappeared from the database"))
    }

    pub fn get_task(&self, id: i64) -> Result<Option<Task>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT id, objective_id, title, description, status, created_at, updated_at
                 FROM tasks WHERE id = ?1",
                params![id],
                task_from_row,
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn record_decision(
        &self,
        project_id: i64,
        objective_id: Option<i64>,
        title: &str,
        body: &str,
        supersedes_decision_id: Option<i64>,
    ) -> Result<Decision, String> {
        validate_text("decision title", title, 240, true)?;
        validate_text("decision body", body, 16_000, true)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        if let Some(objective_id) = objective_id {
            let belongs_to_project: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM objectives WHERE id = ?1 AND project_id = ?2)",
                    params![objective_id, project_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            if !belongs_to_project {
                return Err(
                    "decision objective does not belong to the specified project".to_string(),
                );
            }
        }
        if let Some(previous_id) = supersedes_decision_id {
            let valid_predecessor: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM decisions WHERE id = ?1 AND project_id = ?2)",
                    params![previous_id, project_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            if !valid_predecessor {
                return Err("superseded decision must belong to the same project".to_string());
            }
        }
        transaction
            .execute(
                "INSERT INTO decisions (project_id, objective_id, title, body, supersedes_decision_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![project_id, objective_id, title.trim(), body.trim(), supersedes_decision_id, now],
            )
            .map_err(sqlite_error)?;
        let id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "decision",
            id,
            "decision_recorded",
            &json!({
                "projectId": project_id,
                "objectiveId": objective_id,
                "title": title.trim(),
                "supersedesDecisionId": supersedes_decision_id
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(Decision {
            id,
            project_id,
            objective_id,
            title: title.trim().to_string(),
            body: body.trim().to_string(),
            supersedes_decision_id,
            created_at: now,
        })
    }

    pub fn get_decision(&self, id: i64) -> Result<Option<Decision>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT id, project_id, objective_id, title, body, supersedes_decision_id, created_at
                 FROM decisions WHERE id = ?1",
                params![id],
                decision_from_row,
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn create_workflow_version(
        &self,
        workflow_key: &str,
        name: &str,
        version: i64,
        definition: &Value,
    ) -> Result<WorkflowVersion, String> {
        validate_text("workflow key", workflow_key, 100, true)?;
        validate_text("workflow name", name, 180, true)?;
        if version <= 0 {
            return Err("workflow version must be greater than zero".to_string());
        }
        if !definition.is_object() {
            return Err("workflow definition must be a JSON object".to_string());
        }
        let declared_checks = mandatory_check_keys(definition)?;
        if declared_checks.as_ref().is_some_and(Vec::is_empty) {
            return Err("workflow mandatory_checks cannot be an empty array".to_string());
        }
        let definition_json = serde_json::to_string(definition)
            .map_err(|error| format!("cannot serialize workflow definition: {error}"))?;
        if definition_json.len() > 64 * 1024 {
            return Err("workflow definition exceeds the 64 KiB limit".to_string());
        }
        let hash = content_hash(&definition_json);
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO workflows (workflow_key, name, created_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(workflow_key) DO NOTHING",
                params![workflow_key.trim(), name.trim(), now],
            )
            .map_err(sqlite_error)?;
        let (workflow_id, stored_name) = transaction
            .query_row(
                "SELECT id, name FROM workflows WHERE workflow_key = ?1",
                params![workflow_key.trim()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(sqlite_error)?;
        let existing = transaction
            .query_row(
                "SELECT id, content_hash, definition_json, created_at FROM workflow_versions
                 WHERE workflow_id = ?1 AND version = ?2",
                params![workflow_id, version],
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
        if let Some((id, existing_hash, existing_definition, created_at)) = existing {
            if existing_hash != hash || existing_definition != definition_json {
                return Err(
                    "workflow version already exists with different immutable content".to_string(),
                );
            }
            transaction.commit().map_err(sqlite_error)?;
            return Ok(WorkflowVersion {
                id,
                workflow_id,
                workflow_key: workflow_key.trim().to_string(),
                name: stored_name,
                version,
                definition: definition.clone(),
                content_hash: hash,
                created_at,
            });
        }
        transaction
            .execute(
                "INSERT INTO workflow_versions (workflow_id, version, definition_json, content_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![workflow_id, version, definition_json, hash, now],
            )
            .map_err(sqlite_error)?;
        let id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "workflow_version",
            id,
            "workflow_version_created",
            &json!({ "workflowKey": workflow_key.trim(), "version": version, "contentHash": hash }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(WorkflowVersion {
            id,
            workflow_id,
            workflow_key: workflow_key.trim().to_string(),
            name: stored_name,
            version,
            definition: definition.clone(),
            content_hash: hash,
            created_at: now,
        })
    }

    pub fn start_run(
        &self,
        project_id: i64,
        objective_id: Option<i64>,
        task_id: Option<i64>,
        workflow_version_id: Option<i64>,
    ) -> Result<RunRecord, String> {
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        if let Some(objective_id) = objective_id {
            let belongs_to_project: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM objectives WHERE id = ?1 AND project_id = ?2)",
                    params![objective_id, project_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            if !belongs_to_project {
                return Err("run objective does not belong to the specified project".to_string());
            }
        }
        if let Some(task_id) = task_id {
            let task_matches: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM tasks t JOIN objectives o ON o.id = t.objective_id
                     WHERE t.id = ?1 AND o.project_id = ?2 AND (?3 IS NULL OR t.objective_id = ?3))",
                    params![task_id, project_id, objective_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            if !task_matches {
                return Err(
                    "run task does not belong to the specified project/objective".to_string(),
                );
            }
        }
        transaction
            .execute(
                "INSERT INTO runs (project_id, objective_id, task_id, workflow_version_id, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![project_id, objective_id, task_id, workflow_version_id, now],
            )
            .map_err(sqlite_error)?;
        let id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "run",
            id,
            "run_started",
            &json!({
                "projectId": project_id,
                "objectiveId": objective_id,
                "taskId": task_id,
                "workflowVersionId": workflow_version_id
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_run(id)?
            .ok_or_else(|| "started run disappeared from the database".to_string())
    }

    pub fn record_quality_check(
        &self,
        run_id: i64,
        check_key: &str,
        mandatory: bool,
        status: &str,
        details: &Value,
    ) -> Result<QualityCheck, String> {
        validate_text("quality check key", check_key, 120, true)?;
        if !matches!(status, "pending" | "passed" | "failed" | "inconclusive") {
            return Err("unsupported quality check status".to_string());
        }
        if !details.is_object() {
            return Err("quality check details must be a JSON object".to_string());
        }
        let details_json = serde_json::to_string(details)
            .map_err(|error| format!("cannot serialize quality check details: {error}"))?;
        if details_json.len() > 16 * 1024 {
            return Err("quality check details exceed the 16 KiB limit".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        ensure_run_open(&transaction, run_id)?;
        transaction
            .execute(
                "INSERT INTO quality_checks (run_id, check_key, mandatory, status, details_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(run_id, check_key) DO UPDATE SET
                   mandatory = excluded.mandatory,
                   status = excluded.status,
                   details_json = excluded.details_json,
                   updated_at = excluded.updated_at",
                params![run_id, check_key.trim(), mandatory, status, details_json, now],
            )
            .map_err(sqlite_error)?;
        let check = transaction
            .query_row(
                "SELECT id, run_id, check_key, mandatory, status, details_json, updated_at
                 FROM quality_checks WHERE run_id = ?1 AND check_key = ?2",
                params![run_id, check_key.trim()],
                quality_check_from_row,
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "run",
            run_id,
            "quality_check_recorded",
            &json!({
                "checkId": check.id,
                "checkKey": check.check_key,
                "mandatory": mandatory,
                "status": status
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(check)
    }

    pub fn finish_run(&self, run_id: i64, summary: &str) -> Result<RunRecord, String> {
        validate_text("run summary", summary, 8_000, false)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        ensure_run_open(&transaction, run_id)?;
        let workflow_version_id: Option<i64> = transaction
            .query_row(
                "SELECT workflow_version_id FROM runs WHERE id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        let declared_checks = if let Some(workflow_version_id) = workflow_version_id {
            let definition_json: String = transaction
                .query_row(
                    "SELECT definition_json FROM workflow_versions WHERE id = ?1",
                    params![workflow_version_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            let definition: Value = serde_json::from_str(&definition_json)
                .map_err(|error| format!("stored workflow definition is invalid: {error}"))?;
            mandatory_check_keys(&definition)?.unwrap_or_default()
        } else {
            Vec::new()
        };
        let recorded_checks = transaction
            .prepare("SELECT check_key, mandatory, status FROM quality_checks WHERE run_id = ?1")
            .map_err(sqlite_error)?
            .query_map(params![run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let fallback_keys = if declared_checks.is_empty() {
            recorded_checks
                .iter()
                .filter(|(_, mandatory, _)| *mandatory)
                .map(|(key, _, _)| key.clone())
                .collect::<Vec<_>>()
        } else {
            declared_checks
        };
        let check_by_key = recorded_checks
            .iter()
            .map(|(key, mandatory, status)| (key.as_str(), (*mandatory, status.as_str())))
            .collect::<std::collections::HashMap<_, _>>();
        let missing_or_pending =
            fallback_keys
                .iter()
                .any(|key| match check_by_key.get(key.as_str()) {
                    Some((true, "passed" | "failed" | "inconclusive")) => false,
                    _ => true,
                });
        let declared_failed = fallback_keys.iter().any(|key| {
            check_by_key
                .get(key.as_str())
                .is_some_and(|(mandatory, status)| *mandatory && *status == "failed")
        });
        let declared_inconclusive = fallback_keys.iter().any(|key| {
            check_by_key
                .get(key.as_str())
                .is_some_and(|(mandatory, status)| *mandatory && *status == "inconclusive")
        });
        let (mandatory_count, failed_count, pending_count, inconclusive_count): (
            i64,
            i64,
            i64,
            i64,
        ) = transaction
            .query_row(
                "SELECT COUNT(*),
                       SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END),
                       SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END),
                       SUM(CASE WHEN status = 'inconclusive' THEN 1 ELSE 0 END)
                     FROM quality_checks WHERE run_id = ?1 AND mandatory = 1",
                params![run_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    ))
                },
            )
            .map_err(sqlite_error)?;
        let status = if failed_count > 0 || declared_failed {
            "failed"
        } else if inconclusive_count > 0 || declared_inconclusive {
            "inconclusive"
        } else if pending_count > 0 || mandatory_count == 0 || missing_or_pending {
            "blocked"
        } else {
            "verified_success"
        };
        transaction
            .execute(
                "UPDATE runs SET status = ?1, summary = ?2, finished_at = ?3 WHERE id = ?4",
                params![status, summary.trim(), now, run_id],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "run",
            run_id,
            "run_finished",
            &json!({
                "status": status,
                "summary": summary.trim(),
                "mandatoryCheckCount": mandatory_count,
                "failedCheckCount": failed_count,
                "pendingCheckCount": pending_count,
                "inconclusiveCheckCount": inconclusive_count,
                "declaredMandatoryCheckCount": fallback_keys.len(),
                "missingOrPendingDeclaredCheck": missing_or_pending
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_run(run_id)?
            .ok_or_else(|| format!("run {run_id} disappeared from the database"))
    }

    pub fn get_run(&self, id: i64) -> Result<Option<RunRecord>, String> {
        let connection = self.connection()?;
        let run = connection
            .raw()
            .query_row(
                "SELECT id, project_id, objective_id, task_id, workflow_version_id, status, summary,
                        started_at, finished_at
                 FROM runs WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some((
            id,
            project_id,
            objective_id,
            task_id,
            workflow_version_id,
            status,
            summary,
            started_at,
            finished_at,
        )) = run
        else {
            return Ok(None);
        };
        let checks = connection
            .raw()
            .prepare(
                "SELECT id, run_id, check_key, mandatory, status, details_json, updated_at
                 FROM quality_checks WHERE run_id = ?1 ORDER BY check_key COLLATE NOCASE, id",
            )
            .map_err(sqlite_error)?
            .query_map(params![id], quality_check_from_row)
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        Ok(Some(RunRecord {
            id,
            project_id,
            objective_id,
            task_id,
            workflow_version_id,
            status,
            summary,
            started_at,
            finished_at,
            checks,
        }))
    }

    pub fn list_events(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<EventRecord>, String> {
        let connection = self.connection()?;
        let events = connection
            .raw()
            .prepare(
                "SELECT id, aggregate_type, aggregate_id, event_type, payload_json, created_at
                 FROM events WHERE aggregate_type = ?1 AND aggregate_id = ?2 ORDER BY id",
            )
            .map_err(sqlite_error)?
            .query_map(params![aggregate_type, aggregate_id], event_from_row)
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        Ok(events)
    }

    pub fn record_evidence_artifact(
        &self,
        run_id: Option<i64>,
        artifact_kind: &str,
        uri: &str,
        content_hash: Option<&str>,
        protected: bool,
    ) -> Result<EvidenceArtifact, String> {
        validate_text("artifact kind", artifact_kind, 80, true)?;
        validate_text("artifact URI", uri, 2_000, true)?;
        let lower_uri = uri.trim().to_ascii_lowercase();
        let is_drive_path = uri.as_bytes().get(1) == Some(&b':')
            && uri
                .as_bytes()
                .get(2)
                .is_some_and(|byte| *byte == b'\\' || *byte == b'/');
        let has_scheme = lower_uri.find(':').is_some_and(|colon| {
            colon > 0
                && lower_uri[..colon]
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.' | '-'))
        });
        let unsupported_scheme = has_scheme
            && !is_drive_path
            && !lower_uri.starts_with("file://")
            && !lower_uri.starts_with("artifact://");
        let remote_file = lower_uri.strip_prefix("file://").is_some_and(|rest| {
            !rest.is_empty() && !rest.starts_with('/') && !rest.starts_with("localhost/")
        });
        if unsupported_scheme
            || remote_file
            || uri.starts_with("\\\\")
            || uri.starts_with("//")
            || uri.contains('\0')
        {
            return Err(
                "evidence artifacts must use a local reference, not a remote URL".to_string(),
            );
        }
        if let Some(hash) = content_hash {
            validate_text("artifact content hash", hash, 128, true)?;
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO evidence_artifacts (run_id, artifact_kind, uri, content_hash, protected, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(uri) DO NOTHING",
                params![run_id, artifact_kind.trim(), uri.trim(), content_hash, protected, now],
            )
            .map_err(sqlite_error)?;
        let artifact = transaction
            .query_row(
                "SELECT id, run_id, artifact_kind, uri, content_hash, protected, created_at
                 FROM evidence_artifacts WHERE uri = ?1",
                params![uri.trim()],
                artifact_from_row,
            )
            .map_err(sqlite_error)?;
        if protected && !artifact.protected {
            transaction
                .execute(
                    "UPDATE evidence_artifacts SET protected = 1 WHERE id = ?1",
                    params![artifact.id],
                )
                .map_err(sqlite_error)?;
        }
        append_event(
            &transaction,
            "evidence_artifact",
            artifact.id,
            "evidence_artifact_recorded",
            &json!({
                "runId": run_id,
                "artifactKind": artifact_kind.trim(),
                "uri": uri.trim(),
                "protected": protected
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        if protected && !artifact.protected {
            return self
                .get_artifact(artifact.id)?
                .ok_or_else(|| "artifact disappeared".to_string());
        }
        Ok(artifact)
    }

    pub fn get_artifact(&self, id: i64) -> Result<Option<EvidenceArtifact>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT id, run_id, artifact_kind, uri, content_hash, protected, created_at
                 FROM evidence_artifacts WHERE id = ?1",
                params![id],
                artifact_from_row,
            )
            .optional()
            .map_err(sqlite_error)
    }
}

fn ensure_run_open(transaction: &Transaction<'_>, run_id: i64) -> Result<(), String> {
    let status = transaction
        .query_row(
            "SELECT status FROM runs WHERE id = ?1",
            params![run_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| format!("run {run_id} was not found"))?;
    if status != "running" {
        return Err(format!("run {run_id} is already terminal ({status})"));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_bytes: usize, required: bool) -> Result<(), String> {
    if value.contains('\0') {
        return Err(format!("{field} cannot contain a NUL character"));
    }
    if required && value.trim().is_empty() {
        return Err(format!("{field} is required"));
    }
    if value.len() > max_bytes {
        return Err(format!("{field} exceeds the {max_bytes} byte limit"));
    }
    Ok(())
}

fn mandatory_check_keys(definition: &Value) -> Result<Option<Vec<String>>, String> {
    let Some(value) = definition.get("mandatory_checks") else {
        return Ok(None);
    };
    let checks = value
        .as_array()
        .ok_or_else(|| "workflow mandatory_checks must be an array of strings".to_string())?;
    if checks.len() > 64 {
        return Err("workflow mandatory_checks cannot contain more than 64 entries".to_string());
    }
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(checks.len());
    for check in checks {
        let key = check
            .as_str()
            .ok_or_else(|| "workflow mandatory_checks must contain only strings".to_string())?
            .trim();
        validate_text("workflow mandatory check key", key, 120, true)?;
        if !seen.insert(key.to_string()) {
            return Err(format!("workflow mandatory check {key:?} is duplicated"));
        }
        result.push(key.to_string());
    }
    Ok(Some(result))
}

fn objective_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Objective> {
    Ok(Objective {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        objective_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn decision_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Decision> {
    Ok(Decision {
        id: row.get(0)?,
        project_id: row.get(1)?,
        objective_id: row.get(2)?,
        title: row.get(3)?,
        body: row.get(4)?,
        supersedes_decision_id: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn dependency_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectDependency> {
    Ok(ProjectDependency {
        id: row.get(0)?,
        from_project_id: row.get(1)?,
        to_project_id: row.get(2)?,
        relation: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn quality_check_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<QualityCheck> {
    let details_json: String = row.get(5)?;
    Ok(QualityCheck {
        id: row.get(0)?,
        run_id: row.get(1)?,
        check_key: row.get(2)?,
        mandatory: row.get(3)?,
        status: row.get(4)?,
        details: serde_json::from_str(&details_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        updated_at: row.get(6)?,
    })
}

fn artifact_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvidenceArtifact> {
    Ok(EvidenceArtifact {
        id: row.get(0)?,
        run_id: row.get(1)?,
        artifact_kind: row.get(2)?,
        uri: row.get(3)?,
        content_hash: row.get(4)?,
        protected: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    let payload_json: String = row.get(4)?;
    Ok(EventRecord {
        id: row.get(0)?,
        aggregate_type: row.get(1)?,
        aggregate_id: row.get(2)?,
        event_type: row.get(3)?,
        payload: serde_json::from_str(&payload_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        created_at: row.get(5)?,
    })
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
