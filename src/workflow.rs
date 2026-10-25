use crate::{
    events::append_event,
    hash::content_hash,
    optimization::{
        treatment_from_run_started, OptimizationConfig, OptimizationRunTreatment,
        WorkflowOptimizationOverrides,
    },
    Store,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
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
    pub parent_task_id: Option<i64>,
    pub priority: i64,
    pub due_date: Option<String>,
    pub effort_minutes: Option<i64>,
    pub next_action: Option<String>,
    pub revision: i64,
    pub completed_at: Option<i64>,
    pub completion_kind: Option<String>,
    pub completion_run_id: Option<i64>,
    pub cancelled_at: Option<i64>,
    pub cancellation_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPlanning {
    pub project_id: i64,
    pub status: String,
    pub priority: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanInput {
    pub title: String,
    pub description: String,
    pub parent_task_id: Option<i64>,
    pub priority: i64,
    pub due_date: Option<String>,
    pub effort_minutes: Option<i64>,
    pub next_action: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskBlocker {
    pub id: i64,
    pub task_id: i64,
    pub reason_code: String,
    pub reason: String,
    pub status: String,
    pub resolution: Option<String>,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskBlockReason {
    pub code: String,
    pub message: String,
    pub related_task_id: Option<i64>,
    pub blocker_id: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskPlanningView {
    pub project_id: i64,
    pub project_name: String,
    pub project_status: String,
    pub project_priority: i64,
    pub objective_status: String,
    pub task: Task,
    pub eligible: bool,
    pub blocking_reasons: Vec<TaskBlockReason>,
    pub dependency_unblock_count: i64,
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
    pub archived_at: Option<i64>,
    pub uri_hash: Option<String>,
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
        self.create_task_with_plan(
            objective_id,
            &TaskPlanInput {
                title: title.to_string(),
                description: description.to_string(),
                ..TaskPlanInput::default()
            },
        )
    }

    pub fn create_task_with_plan(
        &self,
        objective_id: i64,
        input: &TaskPlanInput,
    ) -> Result<Task, String> {
        validate_task_plan(input)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let objective_state = transaction
            .query_row(
                "SELECT objective.project_id, objective.status, project.status
                 FROM objectives objective JOIN projects project ON project.id = objective.project_id
                 WHERE objective.id = ?1",
                params![objective_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("objective {objective_id} was not found"))?;
        if objective_state.1 != "active" || objective_state.2 != "active" {
            return Err("tasks can be created only in an active objective and project".to_string());
        }
        if let Some(parent_task_id) = input.parent_task_id {
            ensure_parent_same_project(&transaction, objective_state.0, parent_task_id)?;
        }
        transaction
            .execute(
                "INSERT INTO tasks
                   (objective_id, title, description, created_at, updated_at, parent_task_id,
                    priority, due_date, effort_minutes, next_action)
                 VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    objective_id,
                    input.title.trim(),
                    input.description.trim(),
                    now,
                    input.parent_task_id,
                    input.priority,
                    input.due_date,
                    input.effort_minutes,
                    input.next_action.as_deref().map(str::trim),
                ],
            )
            .map_err(sqlite_error)?;
        let id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "task",
            id,
            "task_created",
            &json!({
                "objectiveId": objective_id,
                "title": input.title.trim(),
                "priority": input.priority,
                "parentTaskId": input.parent_task_id,
                "dueDate": input.due_date,
                "effortMinutes": input.effort_minutes
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(id)?
            .ok_or_else(|| "created task disappeared from the database".to_string())
    }

    pub fn update_task_plan(
        &self,
        id: i64,
        expected_revision: i64,
        input: &TaskPlanInput,
    ) -> Result<Task, String> {
        validate_task_plan(input)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (revision, status, project_id, priority): (i64, String, i64, i64) = transaction
            .query_row(
                "SELECT task.revision, task.status, objective.project_id, task.priority
                 FROM tasks task JOIN objectives objective ON objective.id = task.objective_id
                 WHERE task.id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("task {id} was not found"))?;
        ensure_revision("task", id, revision, expected_revision)?;
        if status == "completed" {
            return Err("completed tasks are immutable".to_string());
        }
        ensure_no_running_task_run(&transaction, id)?;
        if let Some(parent_task_id) = input.parent_task_id {
            if parent_task_id == id {
                return Err("a task cannot be its own parent".to_string());
            }
            ensure_parent_same_project(&transaction, project_id, parent_task_id)?;
            ensure_parent_acyclic(&transaction, id, parent_task_id)?;
        }
        transaction
            .execute(
                "UPDATE tasks SET title = ?1, description = ?2, parent_task_id = ?3,
                   priority = ?4, due_date = ?5, effort_minutes = ?6, next_action = ?7,
                   revision = revision + 1, updated_at = ?8
                 WHERE id = ?9 AND revision = ?10",
                params![
                    input.title.trim(),
                    input.description.trim(),
                    input.parent_task_id,
                    input.priority,
                    input.due_date,
                    input.effort_minutes,
                    input.next_action.as_deref().map(str::trim),
                    now,
                    id,
                    expected_revision
                ],
            )
            .map_err(sqlite_error)?;
        let new_revision = expected_revision + 1;
        append_event(
            &transaction,
            "task",
            id,
            "task_plan_updated",
            &json!({
                "revision": new_revision,
                "parentTaskId": input.parent_task_id,
                "priority": input.priority,
                "dueDate": input.due_date,
                "effortMinutes": input.effort_minutes,
                "nextAction": input.next_action.as_deref().map(str::trim)
            }),
            now,
        )?;
        if priority != input.priority {
            append_event(
                &transaction,
                "task",
                id,
                "task_priority_changed",
                &json!({
                    "from": priority,
                    "to": input.priority,
                    "revision": new_revision
                }),
                now,
            )?;
        }
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(id)?
            .ok_or_else(|| format!("task {id} disappeared from the database"))
    }

    pub fn set_task_status(&self, id: i64, status: &str) -> Result<Task, String> {
        self.set_task_status_with_revision(id, None, status)
    }

    pub fn transition_task_status(
        &self,
        id: i64,
        expected_revision: i64,
        status: &str,
    ) -> Result<Task, String> {
        self.set_task_status_with_revision(id, Some(expected_revision), status)
    }

    pub fn cancel_task(
        &self,
        id: i64,
        expected_revision: i64,
        reason: &str,
    ) -> Result<Task, String> {
        validate_text("task cancellation reason", reason, 1_000, true)?;
        if reason.trim() == "legacy_unknown" {
            return Err("legacy_unknown is reserved for migrated task cancellations".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (status, revision): (String, i64) = transaction
            .query_row(
                "SELECT status, revision FROM tasks WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("task {id} was not found"))?;
        ensure_revision("task", id, revision, expected_revision)?;
        if !matches!(status.as_str(), "queued" | "active" | "blocked" | "failed") {
            return Err(format!("a {status} task cannot be cancelled"));
        }
        ensure_no_pending_task_execution(&transaction, id)?;
        ensure_no_open_task_children(&transaction, id)?;
        transaction
            .execute(
                "UPDATE tasks SET status = 'cancelled', cancellation_reason = ?1,
                   cancelled_at = ?2, updated_at = ?2, revision = revision + 1
                 WHERE id = ?3 AND revision = ?4",
                params![reason.trim(), now, id, expected_revision],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "task",
            id,
            "task_cancelled",
            &json!({
                "from": status,
                "reason": reason.trim(),
                "revision": expected_revision + 1
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(id)?
            .ok_or_else(|| format!("task {id} disappeared from the database"))
    }

    pub fn reopen_task(
        &self,
        id: i64,
        expected_revision: i64,
        reason: &str,
    ) -> Result<Task, String> {
        validate_text("task reopen reason", reason, 1_000, true)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (status, revision, parent_task_id): (String, i64, Option<i64>) = transaction
            .query_row(
                "SELECT status, revision, parent_task_id FROM tasks WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("task {id} was not found"))?;
        ensure_revision("task", id, revision, expected_revision)?;
        if !matches!(status.as_str(), "failed" | "cancelled") {
            return Err(format!("a {status} task cannot be reopened"));
        }
        ensure_no_pending_task_execution(&transaction, id)?;
        if let Some(parent_task_id) = parent_task_id {
            let parent_status: String = transaction
                .query_row(
                    "SELECT status FROM tasks WHERE id = ?1",
                    params![parent_task_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| format!("parent task {parent_task_id} was not found"))?;
            if matches!(parent_status.as_str(), "completed" | "cancelled" | "failed") {
                return Err(format!(
                    "task cannot be reopened while parent task {parent_task_id} is {parent_status}"
                ));
            }
        }
        let has_open_blocker: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_blockers WHERE task_id = ?1 AND status = 'open')",
                params![id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        let new_status = if has_open_blocker {
            "blocked"
        } else {
            "queued"
        };
        transaction
            .execute(
                "UPDATE tasks SET status = ?1, cancellation_reason = NULL, cancelled_at = NULL,
                   updated_at = ?2, revision = revision + 1
                 WHERE id = ?3 AND revision = ?4",
                params![new_status, now, id, expected_revision],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "task",
            id,
            "task_reopened",
            &json!({
                "from": status,
                "to": new_status,
                "reason": reason.trim(),
                "revision": expected_revision + 1
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(id)?
            .ok_or_else(|| format!("task {id} disappeared from the database"))
    }

    fn set_task_status_with_revision(
        &self,
        id: i64,
        expected_revision: Option<i64>,
        status: &str,
    ) -> Result<Task, String> {
        if !matches!(status, "queued" | "active" | "failed") {
            return Err(
                "task status must be queued, active, or failed; use cancel_task for reasoned cancellation, blockers for blocked, and completion commands for completed"
                    .to_string(),
            );
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (current, revision): (String, i64) = transaction
            .query_row(
                "SELECT status, revision FROM tasks WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("task {id} was not found"))?;
        if let Some(expected_revision) = expected_revision {
            ensure_revision("task", id, revision, expected_revision)?;
        }
        if !valid_task_transition(&current, status) {
            return Err(format!("task cannot transition from {current} to {status}"));
        }
        if current == status {
            transaction.commit().map_err(sqlite_error)?;
            return self
                .get_task(id)?
                .ok_or_else(|| format!("task {id} disappeared from the database"));
        }
        ensure_no_running_task_run(&transaction, id)?;
        if status == "cancelled" {
            ensure_no_open_task_children(&transaction, id)?;
        }
        if status == "active" {
            let reasons = task_blocking_reasons(&transaction, id, false)?;
            if !reasons.is_empty() {
                return Err(format!(
                    "task is not eligible: {}",
                    format_block_reasons(&reasons)
                ));
            }
        }
        if status == "queued"
            && transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM task_blockers WHERE task_id = ?1 AND status = 'open')",
                    params![id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(sqlite_error)?
        {
            return Err("task still has unresolved blockers".to_string());
        }
        if current != status {
            transaction
                .execute(
                    "UPDATE tasks SET status = ?1, updated_at = ?2, revision = revision + 1
                     WHERE id = ?3 AND revision = ?4",
                    params![status, now, id, revision],
                )
                .map_err(sqlite_error)?;
            append_event(
                &transaction,
                "task",
                id,
                "task_status_changed",
                &json!({ "from": current, "to": status, "revision": revision + 1 }),
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
                        , parent_task_id, priority, due_date, effort_minutes, next_action,
                        revision, completed_at, completion_kind, completion_run_id,
                        cancelled_at, cancellation_reason
                 FROM tasks WHERE id = ?1",
                params![id],
                task_from_row,
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn get_project_planning(&self, project_id: i64) -> Result<Option<ProjectPlanning>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT id, status, priority, revision FROM projects WHERE id = ?1",
                params![project_id],
                project_planning_from_row,
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn update_project_planning(
        &self,
        project_id: i64,
        expected_revision: i64,
        status: &str,
        priority: i64,
    ) -> Result<ProjectPlanning, String> {
        if !matches!(status, "active" | "paused" | "completed" | "archived") {
            return Err("unsupported project status".to_string());
        }
        validate_priority(priority)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (current_status, current_priority, revision): (String, i64, i64) = transaction
            .query_row(
                "SELECT status, priority, revision FROM projects WHERE id = ?1",
                params![project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("project {project_id} was not found"))?;
        ensure_revision("project", project_id, revision, expected_revision)?;
        if current_status != status || current_priority != priority {
            transaction
                .execute(
                    "UPDATE projects SET status = ?1, priority = ?2, revision = revision + 1
                     WHERE id = ?3 AND revision = ?4",
                    params![status, priority, project_id, expected_revision],
                )
                .map_err(sqlite_error)?;
            if current_status != status {
                append_event(
                    &transaction,
                    "project",
                    project_id,
                    "project_status_changed",
                    &json!({
                        "from": current_status,
                        "to": status,
                        "revision": revision + 1
                    }),
                    now,
                )?;
            }
            if current_priority != priority {
                append_event(
                    &transaction,
                    "project",
                    project_id,
                    "project_priority_changed",
                    &json!({
                        "from": current_priority,
                        "to": priority,
                        "revision": revision + 1
                    }),
                    now,
                )?;
            }
        }
        transaction.commit().map_err(sqlite_error)?;
        self.get_project_planning(project_id)?
            .ok_or_else(|| format!("project {project_id} disappeared from the database"))
    }

    pub fn add_task_dependency(
        &self,
        task_id: i64,
        prerequisite_task_id: i64,
        expected_revision: i64,
    ) -> Result<Task, String> {
        if task_id == prerequisite_task_id {
            return Err("a task cannot depend on itself".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (revision, status, project_id): (i64, String, i64) =
            task_summary(&transaction, task_id)?;
        ensure_revision("task", task_id, revision, expected_revision)?;
        if status != "queued" {
            return Err(
                "task prerequisites can be changed only while the task is queued".to_string(),
            );
        }
        ensure_no_running_task_run(&transaction, task_id)?;
        let prerequisite_project_id = task_project_id(&transaction, prerequisite_task_id)?;
        if project_id != prerequisite_project_id {
            return Err("task prerequisite must belong to the same project".to_string());
        }
        ensure_dependency_acyclic(&transaction, task_id, prerequisite_task_id)?;
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO task_dependencies(task_id, prerequisite_task_id, created_at)
                 VALUES (?1, ?2, ?3)",
                params![task_id, prerequisite_task_id, now],
            )
            .map_err(sqlite_error)?;
        if inserted == 0 {
            transaction.commit().map_err(sqlite_error)?;
            return self
                .get_task(task_id)?
                .ok_or_else(|| format!("task {task_id} disappeared from the database"));
        }
        transaction
            .execute(
                "UPDATE tasks SET revision = revision + 1, updated_at = ?1
                 WHERE id = ?2 AND revision = ?3",
                params![now, task_id, expected_revision],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "task",
            task_id,
            "task_prerequisite_added",
            &json!({
                "prerequisiteTaskId": prerequisite_task_id,
                "revision": expected_revision + 1
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(task_id)?
            .ok_or_else(|| format!("task {task_id} disappeared from the database"))
    }

    pub fn remove_task_dependency(
        &self,
        task_id: i64,
        prerequisite_task_id: i64,
        expected_revision: i64,
    ) -> Result<Task, String> {
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (revision, status, _project_id): (i64, String, i64) =
            task_summary(&transaction, task_id)?;
        ensure_revision("task", task_id, revision, expected_revision)?;
        if status != "queued" {
            return Err(
                "task prerequisites can be changed only while the task is queued".to_string(),
            );
        }
        ensure_no_running_task_run(&transaction, task_id)?;
        let changed = transaction
            .execute(
                "DELETE FROM task_dependencies WHERE task_id = ?1 AND prerequisite_task_id = ?2",
                params![task_id, prerequisite_task_id],
            )
            .map_err(sqlite_error)?;
        if changed == 0 {
            return Err("task prerequisite edge was not found".to_string());
        }
        transaction
            .execute(
                "UPDATE tasks SET revision = revision + 1, updated_at = ?1
                 WHERE id = ?2 AND revision = ?3",
                params![now, task_id, expected_revision],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "task",
            task_id,
            "task_prerequisite_removed",
            &json!({
                "prerequisiteTaskId": prerequisite_task_id,
                "revision": expected_revision + 1
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(task_id)?
            .ok_or_else(|| format!("task {task_id} disappeared from the database"))
    }

    pub fn add_task_blocker(
        &self,
        task_id: i64,
        expected_revision: i64,
        reason_code: &str,
        reason: &str,
    ) -> Result<TaskBlocker, String> {
        if !matches!(
            reason_code,
            "waiting" | "decision" | "external" | "resource" | "other"
        ) {
            return Err("unsupported blocker reason code".to_string());
        }
        validate_text("task blocker reason", reason, 1_000, true)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (revision, status, _project_id) = task_summary(&transaction, task_id)?;
        ensure_revision("task", task_id, revision, expected_revision)?;
        if matches!(status.as_str(), "completed" | "cancelled" | "failed") {
            return Err(format!("cannot add a blocker to a {status} task"));
        }
        ensure_no_running_task_run(&transaction, task_id)?;
        transaction
            .execute(
                "INSERT INTO task_blockers(task_id, reason_code, reason, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![task_id, reason_code, reason.trim(), now],
            )
            .map_err(sqlite_error)?;
        let blocker_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "UPDATE tasks SET status = 'blocked', revision = revision + 1, updated_at = ?1
                 WHERE id = ?2 AND revision = ?3",
                params![now, task_id, expected_revision],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "task",
            task_id,
            "task_blocker_added",
            &json!({
                "blockerId": blocker_id,
                "reasonCode": reason_code,
                "reason": reason.trim(),
                "fromStatus": status,
                "revision": expected_revision + 1
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task_blocker(blocker_id)?
            .ok_or_else(|| format!("task blocker {blocker_id} disappeared from the database"))
    }

    pub fn resolve_task_blocker(
        &self,
        blocker_id: i64,
        expected_task_revision: i64,
        resolution: &str,
    ) -> Result<TaskBlocker, String> {
        validate_text("task blocker resolution", resolution, 1_000, true)?;
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let task_id: i64 = transaction
            .query_row(
                "SELECT task_id FROM task_blockers WHERE id = ?1 AND status = 'open'",
                params![blocker_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("open task blocker {blocker_id} was not found"))?;
        let (revision, status, _project_id) = task_summary(&transaction, task_id)?;
        ensure_revision("task", task_id, revision, expected_task_revision)?;
        ensure_no_running_task_run(&transaction, task_id)?;
        transaction
            .execute(
                "UPDATE task_blockers SET status = 'resolved', resolution = ?1, resolved_at = ?2
                 WHERE id = ?3 AND status = 'open'",
                params![resolution.trim(), now, blocker_id],
            )
            .map_err(sqlite_error)?;
        let remaining: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_blockers WHERE task_id = ?1 AND status = 'open')",
                params![task_id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        let new_status = if status == "blocked" && !remaining {
            "queued"
        } else {
            status.as_str()
        };
        transaction
            .execute(
                "UPDATE tasks SET status = ?1, revision = revision + 1, updated_at = ?2
                 WHERE id = ?3 AND revision = ?4",
                params![new_status, now, task_id, expected_task_revision],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "task",
            task_id,
            "task_blocker_resolved",
            &json!({
                "blockerId": blocker_id,
                "resolution": resolution.trim(),
                "status": new_status,
                "revision": expected_task_revision + 1
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task_blocker(blocker_id)?
            .ok_or_else(|| format!("task blocker {blocker_id} disappeared from the database"))
    }

    pub fn get_task_blocker(&self, blocker_id: i64) -> Result<Option<TaskBlocker>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT id, task_id, reason_code, reason, status, resolution, created_at, resolved_at
                 FROM task_blockers WHERE id = ?1",
                params![blocker_id],
                task_blocker_from_row,
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub fn complete_task_administratively(
        &self,
        task_id: i64,
        expected_revision: i64,
        reason: &str,
    ) -> Result<Task, String> {
        validate_text("administrative completion reason", reason, 1_000, true)?;
        self.complete_task(
            task_id,
            expected_revision,
            "administrative",
            None,
            Some(reason.trim()),
        )
    }

    pub fn accept_task_run(
        &self,
        task_id: i64,
        run_id: i64,
        expected_revision: i64,
    ) -> Result<Task, String> {
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (run_task_id, run_status, run_project_id, finished_at): (
            Option<i64>,
            String,
            i64,
            Option<i64>,
        ) = transaction
            .query_row(
                "SELECT task_id, status, project_id, finished_at FROM runs WHERE id = ?1",
                params![run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("run {run_id} was not found"))?;
        let project_id = task_project_id(&transaction, task_id)?;
        if run_task_id != Some(task_id) || run_project_id != project_id {
            return Err("verified run must belong to the task and its project".to_string());
        }
        if run_status != "verified_success" || finished_at.is_none() {
            return Err("task completion requires a finished verified-success run".to_string());
        }
        transaction.commit().map_err(sqlite_error)?;
        self.complete_task(
            task_id,
            expected_revision,
            "verified_run",
            Some(run_id),
            None,
        )
    }

    fn complete_task(
        &self,
        task_id: i64,
        expected_revision: i64,
        completion_kind: &str,
        completion_run_id: Option<i64>,
        administrative_reason: Option<&str>,
    ) -> Result<Task, String> {
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let (revision, status): (i64, String) = transaction
            .query_row(
                "SELECT revision, status FROM tasks WHERE id = ?1",
                params![task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("task {task_id} was not found"))?;
        ensure_revision("task", task_id, revision, expected_revision)?;
        if status == "completed" {
            return Err("task is already completed".to_string());
        }
        if !matches!(status.as_str(), "queued" | "active") {
            return Err(format!("a {status} task cannot be completed"));
        }
        ensure_task_completable(&transaction, task_id)?;
        transaction
            .execute(
                "UPDATE tasks SET status = 'completed', completed_at = ?1,
                   completion_kind = ?2, completion_run_id = ?3,
                   updated_at = ?1, revision = revision + 1
                 WHERE id = ?4 AND revision = ?5",
                params![
                    now,
                    completion_kind,
                    completion_run_id,
                    task_id,
                    expected_revision
                ],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "task",
            task_id,
            if completion_kind == "administrative" {
                "task_completed_administratively"
            } else {
                "task_completed_from_verified_run"
            },
            &json!({
                "completionKind": completion_kind,
                "runId": completion_run_id,
                "reason": administrative_reason,
                "revision": expected_revision + 1
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_task(task_id)?
            .ok_or_else(|| format!("task {task_id} disappeared from the database"))
    }

    pub fn list_tasks(
        &self,
        project_id: Option<i64>,
        eligible_only: bool,
        limit: usize,
    ) -> Result<Vec<TaskPlanningView>, String> {
        if !(1..=100).contains(&limit) {
            return Err("task query limit must be between 1 and 100".to_string());
        }
        let connection = self.connection()?;
        let eligibility_filter = if eligible_only {
            "AND task.status = 'queued'
             AND project.status = 'active' AND objective.status = 'active'
             AND NOT EXISTS (SELECT 1 FROM task_blockers blocker
               WHERE blocker.task_id = task.id AND blocker.status = 'open')
             AND NOT EXISTS (SELECT 1 FROM task_dependencies dependency
               JOIN tasks prerequisite ON prerequisite.id = dependency.prerequisite_task_id
               WHERE dependency.task_id = task.id AND prerequisite.status <> 'completed')
             AND NOT EXISTS (SELECT 1 FROM tasks parent WHERE parent.id = task.parent_task_id
               AND parent.status IN ('completed', 'cancelled', 'failed'))"
        } else {
            ""
        };
        let query = format!(
            "SELECT task.id, task.objective_id, task.title, task.description, task.status,
                    task.created_at, task.updated_at, task.parent_task_id, task.priority,
                    task.due_date, task.effort_minutes, task.next_action, task.revision,
                    task.completed_at, task.completion_kind, task.completion_run_id,
                    task.cancelled_at, task.cancellation_reason,
                    project.id, project.name, project.status, project.priority, objective.status,
                    (SELECT COUNT(*) FROM task_dependencies downstream
                     JOIN tasks dependent ON dependent.id = downstream.task_id
                     JOIN objectives dependent_objective ON dependent_objective.id = dependent.objective_id
                     WHERE downstream.prerequisite_task_id = task.id
                       AND dependent.status = 'queued'
                       AND dependent_objective.status = 'active'
                       AND NOT EXISTS (SELECT 1 FROM task_dependencies other_dependency
                         JOIN tasks other_prerequisite
                           ON other_prerequisite.id = other_dependency.prerequisite_task_id
                         WHERE other_dependency.task_id = dependent.id
                           AND other_dependency.prerequisite_task_id <> task.id
                           AND other_prerequisite.status <> 'completed')
                       AND NOT EXISTS (SELECT 1 FROM task_blockers blocker
                         WHERE blocker.task_id = dependent.id AND blocker.status = 'open'))
                     AS dependency_unblock_count
             FROM tasks task
             JOIN objectives objective ON objective.id = task.objective_id
             JOIN projects project ON project.id = objective.project_id
             WHERE (?1 IS NULL OR project.id = ?1) {eligibility_filter}
             ORDER BY project.priority DESC, task.priority DESC,
                      CASE WHEN task.due_date IS NULL THEN 1 ELSE 0 END,
                      task.due_date ASC, dependency_unblock_count DESC, task.id ASC
             LIMIT ?2"
        );
        let mut statement = connection.raw().prepare(&query).map_err(sqlite_error)?;
        let rows = statement
            .query_map(params![project_id, limit as i64], |row| {
                Ok((
                    task_from_row(row)?,
                    row.get::<_, i64>(18)?,
                    row.get::<_, String>(19)?,
                    row.get::<_, String>(20)?,
                    row.get::<_, i64>(21)?,
                    row.get::<_, String>(22)?,
                    row.get::<_, i64>(23)?,
                ))
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let mut result = Vec::with_capacity(rows.len());
        for (
            task,
            project_id,
            project_name,
            project_status,
            project_priority,
            objective_status,
            dependency_unblock_count,
        ) in rows
        {
            let blocking_reasons = task_blocking_reasons(connection.raw(), task.id, true)?;
            let eligible = blocking_reasons.is_empty();
            if eligible_only && !eligible {
                continue;
            }
            result.push(TaskPlanningView {
                project_id,
                project_name,
                project_status,
                project_priority,
                objective_status,
                task,
                eligible,
                blocking_reasons,
                dependency_unblock_count,
            });
        }
        Ok(result)
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
        self.start_run_with_optimization(
            project_id,
            objective_id,
            task_id,
            workflow_version_id,
            &OptimizationConfig::default(),
        )
    }

    pub fn start_run_with_optimization(
        &self,
        project_id: i64,
        objective_id: Option<i64>,
        task_id: Option<i64>,
        workflow_version_id: Option<i64>,
        optimization_config: &OptimizationConfig,
    ) -> Result<RunRecord, String> {
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let (workflow_content_hash, overrides_result) =
            if let Some(workflow_version_id) = workflow_version_id {
                let (definition_json, content_hash): (String, String) = transaction
                    .query_row(
                        "SELECT definition_json, content_hash FROM workflow_versions WHERE id = ?1",
                        params![workflow_version_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(sqlite_error)?;
                let definition: Value = serde_json::from_str(&definition_json)
                    .map_err(|error| format!("stored workflow definition is invalid: {error}"))?;
                let overrides = definition
                    .get("optimization")
                    .cloned()
                    .map(serde_json::from_value::<WorkflowOptimizationOverrides>)
                    .transpose()
                    .map(|overrides| overrides.unwrap_or_default());
                (Some(content_hash), overrides)
            } else {
                (None, Ok(WorkflowOptimizationOverrides::default()))
            };
        let (effective_config, configuration_state) = match overrides_result {
            Ok(overrides) => {
                let effective_config = optimization_config.with_workflow_overrides(&overrides);
                if effective_config.validate().is_ok() {
                    (
                        effective_config,
                        crate::optimization::TreatmentConfigurationState::Valid,
                    )
                } else {
                    (
                        OptimizationConfig::default(),
                        crate::optimization::TreatmentConfigurationState::InvalidDisabledFallback,
                    )
                }
            }
            Err(_) => (
                OptimizationConfig::default(),
                crate::optimization::TreatmentConfigurationState::InvalidDisabledFallback,
            ),
        };
        let treatment = OptimizationRunTreatment::new(
            effective_config,
            workflow_version_id,
            workflow_content_hash,
            configuration_state,
        )?;
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
            let task_state: Option<(i64, String)> = transaction
                .query_row(
                    "SELECT task.revision, task.status FROM tasks task
                     JOIN objectives objective ON objective.id = task.objective_id
                     WHERE task.id = ?1 AND objective.project_id = ?2
                       AND (?3 IS NULL OR task.objective_id = ?3)",
                    params![task_id, project_id, objective_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(sqlite_error)?;
            let Some((task_revision, task_status)) = task_state else {
                return Err(
                    "run task does not belong to the specified project/objective".to_string(),
                );
            };
            if !matches!(task_status.as_str(), "queued" | "active") {
                return Err(format!("a {task_status} task cannot start a run"));
            }
            let blocking_reasons = task_blocking_reasons(&transaction, task_id, false)?;
            if !blocking_reasons.is_empty() {
                return Err(format!(
                    "task is not eligible to start a run: {}",
                    format_block_reasons(&blocking_reasons)
                ));
            }
            ensure_no_running_task_run(&transaction, task_id)?;
            if task_status == "queued" {
                transaction
                    .execute(
                        "UPDATE tasks SET status = 'active', revision = revision + 1, updated_at = ?1
                         WHERE id = ?2 AND revision = ?3",
                        params![now, task_id, task_revision],
                    )
                    .map_err(sqlite_error)?;
                append_event(
                    &transaction,
                    "task",
                    task_id,
                    "task_status_changed",
                    &json!({
                        "from": "queued",
                        "to": "active",
                        "reason": "run_started",
                        "revision": task_revision + 1
                    }),
                    now,
                )?;
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
                "workflowVersionId": workflow_version_id,
                "optimizationTreatment": treatment
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_run(id)?
            .ok_or_else(|| "started run disappeared from the database".to_string())
    }

    pub fn get_run_optimization_treatment(
        &self,
        run_id: i64,
    ) -> Result<Option<OptimizationRunTreatment>, String> {
        let events = self.list_events("run", &run_id.to_string())?;
        let Some(started) = events
            .iter()
            .find(|event| event.event_type == "run_started")
        else {
            return Ok(None);
        };
        treatment_from_run_started(started)
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
        let memory_review_recorded: bool = transaction
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM events
                   WHERE aggregate_type = 'run' AND aggregate_id = ?1
                     AND event_type = 'project_memory_reviewed'
                 )",
                params![run_id.to_string()],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if !memory_review_recorded {
            return Err(format!(
                "run {run_id} needs a project memory review before it can be finalized; call record_project_memory_review"
            ));
        }
        let active_project_manager: Option<i64> = transaction
            .query_row(
                "SELECT manager_delegation_id FROM project_manager_assignments
                 WHERE run_id = ?1 AND status = 'active' ORDER BY manager_delegation_id LIMIT 1",
                params![run_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        if let Some(manager_id) = active_project_manager {
            return Err(format!(
                "run {run_id} cannot be finalized while Project Manager assignment {manager_id} is active; wait for its report and Main review"
            ));
        }
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
        if let Err(error) = self.capture_storage_snapshot(Some(run_id)) {
            eprintln!("Cathulu storage-health sampling failed for run {run_id}: {error}");
        }
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
        let uri_hash = crate::hash::content_hash(uri.trim());
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let inserted = transaction
            .execute(
                "INSERT INTO evidence_artifacts (run_id, artifact_kind, uri, content_hash, protected, created_at, uri_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(uri) DO NOTHING",
                params![run_id, artifact_kind.trim(), uri.trim(), content_hash, protected, now, uri_hash],
            )
            .map_err(sqlite_error)?;
        let artifact = transaction
            .query_row(
                "SELECT id, run_id, artifact_kind, uri, content_hash, protected, created_at,
                        archived_at, uri_hash
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
            append_event(
                &transaction,
                "evidence_artifact",
                artifact.id,
                "evidence_protected",
                &json!({ "protected": true }),
                now,
            )?;
        }
        if inserted > 0 {
            append_event(
                &transaction,
                "evidence_artifact",
                artifact.id,
                "evidence_artifact_recorded",
                &json!({
                    "runId": run_id,
                    "artifactKind": artifact_kind.trim(),
                    "contentHash": content_hash,
                    "uriHash": uri_hash,
                    "protected": protected
                }),
                now,
            )?;
        }
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
                "SELECT id, run_id, artifact_kind, uri, content_hash, protected, created_at,
                        archived_at, uri_hash
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

fn validate_priority(priority: i64) -> Result<(), String> {
    if !(-100..=100).contains(&priority) {
        return Err("priority must be between -100 and 100".to_string());
    }
    Ok(())
}

fn validate_due_date(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..].iter().all(u8::is_ascii_digit)
    {
        return Err("due date must use YYYY-MM-DD format".to_string());
    }
    let year = value[0..4]
        .parse::<i32>()
        .map_err(|_| "due date must use YYYY-MM-DD format".to_string())?;
    let month = value[5..7]
        .parse::<u32>()
        .map_err(|_| "due date must use YYYY-MM-DD format".to_string())?;
    let day = value[8..10]
        .parse::<u32>()
        .map_err(|_| "due date must use YYYY-MM-DD format".to_string())?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err("due date has an invalid month".to_string()),
    };
    if year < 1 || day == 0 || day > max_day {
        return Err("due date is not a valid calendar date".to_string());
    }
    Ok(())
}

fn validate_task_plan(input: &TaskPlanInput) -> Result<(), String> {
    validate_text("task title", &input.title, 240, true)?;
    validate_text("task description", &input.description, 8_000, false)?;
    validate_priority(input.priority)?;
    if let Some(due_date) = input.due_date.as_deref() {
        validate_due_date(due_date)?;
    }
    if input
        .effort_minutes
        .is_some_and(|value| !(0..=10_000_000).contains(&value))
    {
        return Err("effort_minutes must be between 0 and 10000000 when known".to_string());
    }
    if let Some(next_action) = input.next_action.as_deref() {
        validate_text("task next action", next_action, 1_000, false)?;
    }
    Ok(())
}

fn ensure_revision(
    entity: &str,
    id: i64,
    current_revision: i64,
    expected_revision: i64,
) -> Result<(), String> {
    if current_revision != expected_revision {
        return Err(format!(
            "stale {entity} revision for {id}: expected {expected_revision}, current {current_revision}"
        ));
    }
    Ok(())
}

fn task_summary(transaction: &Transaction<'_>, task_id: i64) -> Result<(i64, String, i64), String> {
    transaction
        .query_row(
            "SELECT task.revision, task.status, objective.project_id
             FROM tasks task JOIN objectives objective ON objective.id = task.objective_id
             WHERE task.id = ?1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| format!("task {task_id} was not found"))
}

fn task_project_id(transaction: &Transaction<'_>, task_id: i64) -> Result<i64, String> {
    transaction
        .query_row(
            "SELECT objective.project_id FROM tasks task
             JOIN objectives objective ON objective.id = task.objective_id WHERE task.id = ?1",
            params![task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| format!("task {task_id} was not found"))
}

fn ensure_parent_same_project(
    transaction: &Transaction<'_>,
    project_id: i64,
    parent_task_id: i64,
) -> Result<(), String> {
    let same_project: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks task
             JOIN objectives objective ON objective.id = task.objective_id
             WHERE task.id = ?1 AND objective.project_id = ?2)",
            params![parent_task_id, project_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error)?;
    if !same_project {
        return Err("parent task must belong to the same project".to_string());
    }
    Ok(())
}

fn ensure_parent_acyclic(
    transaction: &Transaction<'_>,
    task_id: i64,
    parent_task_id: i64,
) -> Result<(), String> {
    let would_cycle: bool = transaction
        .query_row(
            "WITH RECURSIVE parents(task_id) AS (
               SELECT ?1
               UNION
               SELECT task.parent_task_id FROM tasks task
               JOIN parents chain ON task.id = chain.task_id
               WHERE task.parent_task_id IS NOT NULL
             )
             SELECT EXISTS(SELECT 1 FROM parents WHERE task_id = ?2)",
            params![parent_task_id, task_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error)?;
    if would_cycle {
        return Err("task parent cycle is not allowed".to_string());
    }
    Ok(())
}

fn ensure_dependency_acyclic(
    transaction: &Transaction<'_>,
    task_id: i64,
    prerequisite_task_id: i64,
) -> Result<(), String> {
    let would_cycle: bool = transaction
        .query_row(
            "WITH RECURSIVE prerequisites(task_id) AS (
               SELECT ?1
               UNION
               SELECT dependency.prerequisite_task_id
               FROM task_dependencies dependency
               JOIN prerequisites chain ON dependency.task_id = chain.task_id
             )
             SELECT EXISTS(SELECT 1 FROM prerequisites WHERE task_id = ?2)",
            params![prerequisite_task_id, task_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error)?;
    if would_cycle {
        return Err("task prerequisite cycle is not allowed".to_string());
    }
    Ok(())
}

fn ensure_no_running_task_run(transaction: &Transaction<'_>, task_id: i64) -> Result<(), String> {
    let has_running_run: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1 AND status = 'running')",
            params![task_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error)?;
    if has_running_run {
        return Err("task cannot be edited while a linked run is active".to_string());
    }
    Ok(())
}

fn ensure_no_pending_task_execution(
    transaction: &Transaction<'_>,
    task_id: i64,
) -> Result<(), String> {
    ensure_no_running_task_run(transaction, task_id)?;
    let delegation: Option<(i64, String)> = transaction
        .query_row(
            "SELECT id, status FROM delegations
             WHERE task_id = ?1 AND status IN ('prepared', 'dispatched')
             ORDER BY id LIMIT 1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    if let Some((delegation_id, status)) = delegation {
        return Err(format!(
            "task cannot change terminal state while delegation {delegation_id} is {status}"
        ));
    }
    let active_manager: Option<i64> = transaction
        .query_row(
            "SELECT manager_delegation_id FROM project_manager_assignments
             WHERE task_id = ?1 AND status = 'active' ORDER BY manager_delegation_id LIMIT 1",
            params![task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)?;
    if let Some(manager_id) = active_manager {
        return Err(format!(
            "task cannot change terminal state while Project Manager assignment {manager_id} is active"
        ));
    }
    Ok(())
}

fn valid_task_transition(from: &str, to: &str) -> bool {
    (from == to && matches!(from, "queued" | "active" | "failed"))
        || matches!(
            (from, to),
            ("queued", "active" | "failed") | ("active", "queued" | "failed")
        )
}

fn task_blocking_reasons(
    connection: &rusqlite::Connection,
    task_id: i64,
    include_task_status: bool,
) -> Result<Vec<TaskBlockReason>, String> {
    let state: (String, String, String, Option<i64>) = connection
        .query_row(
            "SELECT task.status, project.status, objective.status, task.parent_task_id
             FROM tasks task
             JOIN objectives objective ON objective.id = task.objective_id
             JOIN projects project ON project.id = objective.project_id
             WHERE task.id = ?1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(sqlite_error)?
        .ok_or_else(|| format!("task {task_id} was not found"))?;
    let (status, project_status, objective_status, parent_task_id) = state;
    let mut reasons = Vec::new();
    if project_status != "active" {
        reasons.push(TaskBlockReason {
            code: "project_not_active".to_string(),
            message: format!("project status is {project_status}"),
            related_task_id: None,
            blocker_id: None,
        });
    }
    if objective_status != "active" {
        reasons.push(TaskBlockReason {
            code: "objective_not_active".to_string(),
            message: format!("objective status is {objective_status}"),
            related_task_id: None,
            blocker_id: None,
        });
    }
    if include_task_status && status != "queued" {
        reasons.push(TaskBlockReason {
            code: "task_not_queued".to_string(),
            message: format!("task status is {status}"),
            related_task_id: None,
            blocker_id: None,
        });
    }
    let blockers = connection
        .prepare(
            "SELECT id, reason_code, reason FROM task_blockers
             WHERE task_id = ?1 AND status = 'open' ORDER BY created_at, id",
        )
        .map_err(sqlite_error)?
        .query_map(params![task_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    for (blocker_id, reason_code, reason) in blockers {
        reasons.push(TaskBlockReason {
            code: format!("manual_{reason_code}"),
            message: reason,
            related_task_id: None,
            blocker_id: Some(blocker_id),
        });
    }
    let prerequisites = connection
        .prepare(
            "SELECT prerequisite.id, prerequisite.title, prerequisite.status
             FROM task_dependencies dependency
             JOIN tasks prerequisite ON prerequisite.id = dependency.prerequisite_task_id
             WHERE dependency.task_id = ?1 AND prerequisite.status <> 'completed'
             ORDER BY prerequisite.id",
        )
        .map_err(sqlite_error)?
        .query_map(params![task_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    for (prerequisite_id, title, prerequisite_status) in prerequisites {
        reasons.push(TaskBlockReason {
            code: "prerequisite_not_completed".to_string(),
            message: format!(
                "prerequisite task {prerequisite_id} ({title}) is {prerequisite_status}"
            ),
            related_task_id: Some(prerequisite_id),
            blocker_id: None,
        });
    }
    if let Some(parent_task_id) = parent_task_id {
        let parent_status: String = connection
            .query_row(
                "SELECT status FROM tasks WHERE id = ?1",
                params![parent_task_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("parent task {parent_task_id} was not found"))?;
        if matches!(parent_status.as_str(), "completed" | "cancelled" | "failed") {
            reasons.push(TaskBlockReason {
                code: "parent_not_open".to_string(),
                message: format!("parent task {parent_task_id} is {parent_status}"),
                related_task_id: Some(parent_task_id),
                blocker_id: None,
            });
        }
    }
    Ok(reasons)
}

fn ensure_task_completable(transaction: &Transaction<'_>, task_id: i64) -> Result<(), String> {
    let reasons = task_blocking_reasons(transaction, task_id, false)?;
    if !reasons.is_empty() {
        return Err(format!(
            "task cannot be completed while blocked: {}",
            format_block_reasons(&reasons)
        ));
    }
    let unresolved_run: Option<(i64, String)> = transaction
        .query_row(
            "SELECT id, status FROM runs WHERE task_id = ?1 AND status = 'running'
             ORDER BY id LIMIT 1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    if let Some((run_id, status)) = unresolved_run {
        return Err(format!(
            "task cannot be completed while linked run {run_id} is {status}"
        ));
    }
    let delegation: Option<(i64, String)> = transaction
        .query_row(
            "SELECT id, status FROM delegations
             WHERE task_id = ?1 AND status IN ('prepared', 'dispatched')
             ORDER BY id LIMIT 1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    if let Some((delegation_id, status)) = delegation {
        return Err(format!(
            "task cannot be completed while delegation {delegation_id} is {status}"
        ));
    }
    let active_manager: Option<i64> = transaction
        .query_row(
            "SELECT manager_delegation_id FROM project_manager_assignments
             WHERE task_id = ?1 AND status = 'active' ORDER BY manager_delegation_id LIMIT 1",
            params![task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)?;
    if let Some(manager_id) = active_manager {
        return Err(format!(
            "task cannot be completed while Project Manager assignment {manager_id} is active"
        ));
    }
    ensure_no_open_task_children(transaction, task_id)?;
    Ok(())
}

fn ensure_no_open_task_children(transaction: &Transaction<'_>, task_id: i64) -> Result<(), String> {
    let open_child: Option<(i64, String)> = transaction
        .query_row(
            "SELECT id, status FROM tasks
             WHERE parent_task_id = ?1
               AND (status NOT IN ('completed', 'cancelled')
                 OR (status = 'cancelled'
                   AND (cancellation_reason IS NULL OR cancellation_reason = 'legacy_unknown')))
             ORDER BY id LIMIT 1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite_error)?;
    if let Some((child_id, child_status)) = open_child {
        return Err(format!(
            "parent task cannot be completed while child task {child_id} is {child_status}"
        ));
    }
    Ok(())
}

fn format_block_reasons(reasons: &[TaskBlockReason]) -> String {
    reasons
        .iter()
        .map(|reason| format!("{}: {}", reason.code, reason.message))
        .collect::<Vec<_>>()
        .join("; ")
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

fn project_planning_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectPlanning> {
    Ok(ProjectPlanning {
        project_id: row.get(0)?,
        status: row.get(1)?,
        priority: row.get(2)?,
        revision: row.get(3)?,
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
        parent_task_id: row.get(7)?,
        priority: row.get(8)?,
        due_date: row.get(9)?,
        effort_minutes: row.get(10)?,
        next_action: row.get(11)?,
        revision: row.get(12)?,
        completed_at: row.get(13)?,
        completion_kind: row.get(14)?,
        completion_run_id: row.get(15)?,
        cancelled_at: row.get(16)?,
        cancellation_reason: row.get(17)?,
    })
}

fn task_blocker_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskBlocker> {
    Ok(TaskBlocker {
        id: row.get(0)?,
        task_id: row.get(1)?,
        reason_code: row.get(2)?,
        reason: row.get(3)?,
        status: row.get(4)?,
        resolution: row.get(5)?,
        created_at: row.get(6)?,
        resolved_at: row.get(7)?,
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
        archived_at: row.get(7)?,
        uri_hash: row.get(8)?,
    })
}

pub(crate) fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
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

pub(crate) fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
