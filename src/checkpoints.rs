//! Immutable project checkpoints and provider-free resume projections.

use crate::{
    events::append_event,
    workflow::{now, task_blocking_reasons, TaskBlockReason},
    Store,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

const MAX_RESUME_BYTES: usize = 32_000;
const MAX_RESUME_ITEMS: usize = 10;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointArtifact {
    pub artifact_id: i64,
    pub uri: String,
    pub content_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCheckpoint {
    pub id: i64,
    pub project_id: i64,
    pub revision: i64,
    pub summary_text: String,
    pub summary_source: String,
    pub source_event_id: i64,
    pub project_revision: i64,
    pub repository_head: Option<String>,
    pub repository_dirty: bool,
    pub repository_observed_at: Option<i64>,
    pub created_at: i64,
    pub artifacts: Vec<CheckpointArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeProjectHeader {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub priority: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeRepository {
    pub availability: String,
    pub freshness: String,
    pub checkpoint_head: Option<String>,
    pub stored_head: Option<String>,
    pub observed_at: Option<i64>,
    pub dirty_at_observation: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeObjective {
    pub id: i64,
    pub title: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeReason {
    pub code: String,
    pub message: String,
    pub related_task_id: Option<i64>,
    pub blocker_id: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeTask {
    pub id: i64,
    pub objective_id: i64,
    pub revision: i64,
    pub title: String,
    pub status: String,
    pub priority: i64,
    pub due_date: Option<String>,
    pub effort_minutes: Option<i64>,
    pub next_action: Option<String>,
    pub dependency_unblock_count: i64,
    pub reasons: Vec<ResumeReason>,
    pub omitted_reason_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeDelta {
    pub task_id: i64,
    pub change: String,
    pub from_revision: Option<i64>,
    pub to_revision: Option<i64>,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub event_id: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeDecision {
    pub id: i64,
    pub objective_id: Option<i64>,
    pub title: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeVerification {
    pub run_id: i64,
    pub status: String,
    pub finished_at: Option<i64>,
    pub workflow_version_id: Option<i64>,
    pub non_passing_mandatory_checks: i64,
    pub mandatory_checks_changed_since: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResumeLimits {
    pub max_items_per_list: usize,
    pub objective_total: usize,
    pub objective_omitted: usize,
    pub remaining_total: usize,
    pub remaining_omitted: usize,
    pub next_eligible_total: usize,
    pub next_eligible_omitted: usize,
    pub blocked_total: usize,
    pub blocked_omitted: usize,
    pub delta_total: usize,
    pub delta_omitted: usize,
    pub recent_decision_total: usize,
    pub recent_decision_omitted: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResume {
    pub packet_version: i64,
    pub project: ResumeProjectHeader,
    pub generated_at: i64,
    pub source_event_id: i64,
    pub checkpoint: Option<ProjectCheckpoint>,
    pub checkpoint_age_seconds: Option<i64>,
    pub repository: ResumeRepository,
    pub objectives: Vec<ResumeObjective>,
    pub remaining: Vec<ResumeTask>,
    pub next_eligible: Vec<ResumeTask>,
    pub blocked: Vec<ResumeTask>,
    pub delta_since_checkpoint: Vec<ResumeDelta>,
    pub recent_decisions: Vec<ResumeDecision>,
    pub verification: Option<ResumeVerification>,
    pub limits: ResumeLimits,
}

impl Store {
    pub fn create_project_checkpoint(
        &self,
        project_id: i64,
        expected_source_event_id: i64,
        summary_text: &str,
        artifact_ids: &[i64],
    ) -> Result<ProjectCheckpoint, String> {
        let summary_text = summary_text.trim();
        if summary_text.is_empty() || summary_text.len() > 2_000 {
            return Err("checkpoint summary must contain 1 to 2000 bytes".to_string());
        }
        if artifact_ids.len() > 16 || artifact_ids.iter().any(|id| *id <= 0) {
            return Err("checkpoint may reference at most 16 valid artifacts".to_string());
        }
        let mut unique = HashSet::new();
        if artifact_ids.iter().any(|id| !unique.insert(*id)) {
            return Err("checkpoint artifact IDs must be unique".to_string());
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let project: (i64, Option<String>, bool, Option<i64>) = transaction
            .query_row(
                "SELECT revision, git_head, git_dirty, reconciled_at FROM projects WHERE id = ?1",
                params![project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(db_error)?
            .ok_or_else(|| format!("project {project_id} was not found"))?;
        let source_event_id = event_high_water(&transaction)?;
        if source_event_id != expected_source_event_id {
            return Err(format!(
                "stale source event revision: expected {expected_source_event_id}, current {source_event_id}"
            ));
        }
        let mut artifacts = Vec::with_capacity(artifact_ids.len());
        for artifact_id in artifact_ids {
            let artifact: Option<(String, Option<String>)> = transaction
                .query_row(
                    "SELECT artifact.uri, artifact.content_hash
                     FROM evidence_artifacts artifact JOIN runs run ON run.id = artifact.run_id
                     WHERE artifact.id = ?1 AND run.project_id = ?2",
                    params![artifact_id, project_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(db_error)?;
            let (uri, content_hash) = artifact.ok_or_else(|| {
                format!("artifact {artifact_id} is not linked to a run in project {project_id}")
            })?;
            if uri.len() > 512 {
                return Err(format!(
                    "artifact {artifact_id} URI is too long for a checkpoint"
                ));
            }
            artifacts.push(CheckpointArtifact {
                artifact_id: *artifact_id,
                uri,
                content_hash,
            });
        }
        let revision: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(revision), 0) + 1 FROM project_checkpoints WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let created_at = now();
        transaction
            .execute(
                "INSERT INTO project_checkpoints
                 (project_id, revision, summary_text, summary_source, source_event_id,
                  project_revision, repository_head, repository_dirty, repository_observed_at, created_at)
                 VALUES (?1, ?2, ?3, 'unscoped_local_client', ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    project_id,
                    revision,
                    summary_text,
                    source_event_id,
                    project.0,
                    project.1,
                    project.2,
                    project.3,
                    created_at
                ],
            )
            .map_err(db_error)?;
        let checkpoint_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO project_checkpoint_tasks
                 (checkpoint_id, task_id, objective_id, revision, status, title)
                 SELECT ?1, task.id, task.objective_id, task.revision, task.status, task.title
                 FROM tasks task JOIN objectives objective ON objective.id = task.objective_id
                 WHERE objective.project_id = ?2",
                params![checkpoint_id, project_id],
            )
            .map_err(db_error)?;
        transaction
            .execute(
                "INSERT INTO project_checkpoint_objectives
                 (checkpoint_id, objective_id, title, status)
                 SELECT ?1, id, title, status FROM objectives WHERE project_id = ?2",
                params![checkpoint_id, project_id],
            )
            .map_err(db_error)?;
        for artifact in &artifacts {
            transaction
                .execute(
                    "INSERT INTO project_checkpoint_artifacts
                     (checkpoint_id, artifact_id, uri, content_hash) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        checkpoint_id,
                        artifact.artifact_id,
                        artifact.uri,
                        artifact.content_hash
                    ],
                )
                .map_err(db_error)?;
        }
        transaction
            .execute(
                "INSERT INTO project_checkpoint_seals(checkpoint_id, created_at) VALUES (?1, ?2)",
                params![checkpoint_id, created_at],
            )
            .map_err(db_error)?;
        append_event(
            &transaction,
            "project_checkpoint",
            checkpoint_id,
            "project_checkpoint_created",
            &json!({
                "projectId": project_id,
                "revision": revision,
                "sourceEventId": source_event_id,
                "artifactIds": artifact_ids
            }),
            created_at,
        )?;
        transaction.commit().map_err(db_error)?;
        self.get_project_checkpoint(checkpoint_id)?
            .ok_or_else(|| format!("checkpoint {checkpoint_id} disappeared from the database"))
    }

    pub fn get_project_checkpoint(&self, id: i64) -> Result<Option<ProjectCheckpoint>, String> {
        let connection = self.connection()?;
        checkpoint_from_connection(connection.raw(), id)
    }

    pub fn latest_project_checkpoint(
        &self,
        project_id: i64,
    ) -> Result<Option<ProjectCheckpoint>, String> {
        let connection = self.connection()?;
        ensure_project_exists(connection.raw(), project_id)?;
        let id: Option<i64> = connection
            .raw()
            .query_row(
                "SELECT id FROM project_checkpoints WHERE project_id = ?1
                 AND EXISTS (SELECT 1 FROM project_checkpoint_seals WHERE checkpoint_id = project_checkpoints.id)
                 ORDER BY revision DESC LIMIT 1",
                params![project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        id.map(|id| checkpoint_from_connection(connection.raw(), id))
            .transpose()
            .map(Option::flatten)
    }
}

fn db_error(error: rusqlite::Error) -> String {
    error.to_string()
}

fn event_high_water(connection: &rusqlite::Connection) -> Result<i64, String> {
    connection
        .query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |row| {
            row.get(0)
        })
        .map_err(db_error)
}

fn ensure_project_exists(connection: &rusqlite::Connection, project_id: i64) -> Result<(), String> {
    let exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            params![project_id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    if exists {
        Ok(())
    } else {
        Err(format!("project {project_id} was not found"))
    }
}

fn checkpoint_from_connection(
    connection: &rusqlite::Connection,
    id: i64,
) -> Result<Option<ProjectCheckpoint>, String> {
    let checkpoint: Option<ProjectCheckpoint> = connection
        .query_row(
            "SELECT id, project_id, revision, summary_text, summary_source, source_event_id,
                    project_revision, repository_head, repository_dirty, repository_observed_at, created_at
             FROM project_checkpoints WHERE id = ?1
               AND EXISTS (SELECT 1 FROM project_checkpoint_seals WHERE checkpoint_id = project_checkpoints.id)",
            params![id],
            |row| {
                Ok(ProjectCheckpoint {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    revision: row.get(2)?,
                    summary_text: row.get(3)?,
                    summary_source: row.get(4)?,
                    source_event_id: row.get(5)?,
                    project_revision: row.get(6)?,
                    repository_head: row.get(7)?,
                    repository_dirty: row.get(8)?,
                    repository_observed_at: row.get(9)?,
                    created_at: row.get(10)?,
                    artifacts: Vec::new(),
                })
            },
        )
        .optional()
        .map_err(db_error)?;
    let Some(mut checkpoint) = checkpoint else {
        return Ok(None);
    };
    checkpoint.artifacts = connection
        .prepare(
            "SELECT artifact_id, uri, content_hash FROM project_checkpoint_artifacts
             WHERE checkpoint_id = ?1 ORDER BY artifact_id",
        )
        .map_err(db_error)?
        .query_map(params![id], |row| {
            Ok(CheckpointArtifact {
                artifact_id: row.get(0)?,
                uri: row.get(1)?,
                content_hash: row.get(2)?,
            })
        })
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;
    Ok(Some(checkpoint))
}

#[derive(Clone)]
struct CurrentTask {
    id: i64,
    objective_id: i64,
    revision: i64,
    title: String,
    status: String,
    priority: i64,
    due_date: Option<String>,
    effort_minutes: Option<i64>,
    next_action: Option<String>,
    dependency_unblock_count: i64,
}

impl Store {
    pub fn resume_project(&self, project_id: i64, limit: usize) -> Result<ProjectResume, String> {
        if !(1..=MAX_RESUME_ITEMS).contains(&limit) {
            return Err(format!(
                "resume list limit must be between 1 and {MAX_RESUME_ITEMS}"
            ));
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(db_error)?;
        let project: (
            ResumeProjectHeader,
            String,
            Option<String>,
            Option<String>,
            bool,
            Option<i64>,
        ) = transaction
            .query_row(
                "SELECT id, name, status, priority, revision, kind, path, git_head, git_dirty, reconciled_at
                 FROM projects WHERE id = ?1",
                params![project_id],
                |row| {
                    Ok((
                        ResumeProjectHeader {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            status: row.get(2)?,
                            priority: row.get(3)?,
                            revision: row.get(4)?,
                        },
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                    ))
                },
            )
            .optional()
            .map_err(db_error)?
            .ok_or_else(|| format!("project {project_id} was not found"))?;
        let source_event_id = event_high_water(&transaction)?;
        let checkpoint_id: Option<i64> = transaction
            .query_row(
                "SELECT id FROM project_checkpoints WHERE project_id = ?1
                 AND EXISTS (SELECT 1 FROM project_checkpoint_seals WHERE checkpoint_id = project_checkpoints.id)
                 ORDER BY revision DESC LIMIT 1",
                params![project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        let checkpoint = checkpoint_id
            .map(|id| checkpoint_from_connection(&transaction, id))
            .transpose()?
            .flatten();
        let repository_applicable = project.1 == "repository";
        let repository_available = project
            .2
            .as_deref()
            .is_some_and(|path| Path::new(path).is_dir() && Path::new(path).join(".git").exists());
        let freshness = if !repository_applicable {
            "not_applicable"
        } else if !repository_available {
            "unavailable"
        } else if let (Some(checkpoint_head), Some(stored_head)) = (
            checkpoint
                .as_ref()
                .and_then(|item| item.repository_head.as_deref()),
            project.3.as_deref(),
        ) {
            if checkpoint_head == stored_head {
                "same"
            } else {
                "changed_since_checkpoint"
            }
        } else {
            "unknown"
        };
        let repository = ResumeRepository {
            availability: if !repository_applicable {
                "not_applicable"
            } else if repository_available {
                "available"
            } else {
                "unavailable"
            }
            .to_string(),
            freshness: freshness.to_string(),
            checkpoint_head: checkpoint
                .as_ref()
                .and_then(|item| item.repository_head.clone()),
            stored_head: project.3,
            observed_at: project.5,
            dirty_at_observation: project.4,
        };
        let objectives = transaction
            .prepare("SELECT id, title, status FROM objectives WHERE project_id = ?1 ORDER BY id")
            .map_err(db_error)?
            .query_map(params![project_id], |row| {
                Ok(ResumeObjective {
                    id: row.get(0)?,
                    title: bounded_text(&row.get::<_, String>(1)?, 120),
                    status: row.get(2)?,
                })
            })
            .map_err(db_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?;
        let tasks = current_tasks(&transaction, project_id)?;
        let mut remaining = Vec::new();
        let mut next_eligible = Vec::new();
        let mut blocked = Vec::new();
        let current_by_id: HashMap<i64, (i64, String)> = tasks
            .iter()
            .map(|task| (task.id, (task.revision, task.status.clone())))
            .collect();
        for task in &tasks {
            if matches!(task.status.as_str(), "completed" | "cancelled") {
                continue;
            }
            let reasons = task_blocking_reasons(&transaction, task.id, true)?;
            remaining.push(resume_task(task, &[]));
            if reasons.is_empty() {
                next_eligible.push(resume_task(task, &[]));
            } else {
                blocked.push(resume_task(task, &reasons));
            }
        }
        let delta_since_checkpoint = if let Some(checkpoint) = &checkpoint {
            checkpoint_deltas(&transaction, checkpoint, &current_by_id)?
        } else {
            Vec::new()
        };
        let recent_decisions = transaction
            .prepare(
                "SELECT decision.id, decision.objective_id, decision.title, decision.created_at
                 FROM decisions decision WHERE decision.project_id = ?1
                   AND NOT EXISTS (SELECT 1 FROM decisions newer WHERE newer.supersedes_decision_id = decision.id)
                 ORDER BY decision.created_at DESC, decision.id DESC",
            )
            .map_err(db_error)?
            .query_map(params![project_id], |row| {
                Ok(ResumeDecision {
                    id: row.get(0)?,
                    objective_id: row.get(1)?,
                    title: bounded_text(&row.get::<_, String>(2)?, 120),
                    created_at: row.get(3)?,
                })
            })
            .map_err(db_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?;
        let verification = latest_verification(&transaction, project_id)?;
        let limits = ResumeLimits {
            max_items_per_list: limit,
            objective_total: objectives.len(),
            objective_omitted: 0,
            remaining_total: remaining.len(),
            remaining_omitted: 0,
            next_eligible_total: next_eligible.len(),
            next_eligible_omitted: 0,
            blocked_total: blocked.len(),
            blocked_omitted: 0,
            delta_total: delta_since_checkpoint.len(),
            delta_omitted: 0,
            recent_decision_total: recent_decisions.len(),
            recent_decision_omitted: 0,
        };
        let generated_at = now();
        let checkpoint_age_seconds = checkpoint
            .as_ref()
            .map(|item| generated_at.saturating_sub(item.created_at).max(0));
        let mut packet = ProjectResume {
            packet_version: 1,
            project: project.0,
            generated_at,
            source_event_id,
            checkpoint,
            checkpoint_age_seconds,
            repository,
            objectives: objectives.into_iter().take(limit).collect(),
            remaining: remaining.into_iter().take(limit).collect(),
            next_eligible: next_eligible.into_iter().take(limit).collect(),
            blocked: blocked.into_iter().take(limit).collect(),
            delta_since_checkpoint: delta_since_checkpoint.into_iter().take(limit).collect(),
            recent_decisions: recent_decisions.into_iter().take(limit).collect(),
            verification,
            limits,
        };
        refresh_omitted(&mut packet);
        while serde_json::to_vec(&packet)
            .map_err(|error| error.to_string())?
            .len()
            > MAX_RESUME_BYTES
        {
            if !trim_longest_list(&mut packet) {
                return Err("resume packet exceeds its 32000-byte bound".to_string());
            }
            refresh_omitted(&mut packet);
        }
        transaction.commit().map_err(db_error)?;
        Ok(packet)
    }
}

fn current_tasks(
    connection: &rusqlite::Connection,
    project_id: i64,
) -> Result<Vec<CurrentTask>, String> {
    connection
        .prepare(
            "SELECT task.id, task.objective_id, task.revision, task.title, task.status,
                    task.priority, task.due_date, task.effort_minutes, task.next_action,
                    (SELECT COUNT(*) FROM task_dependencies downstream
                     JOIN tasks dependent ON dependent.id = downstream.task_id
                     JOIN objectives dependent_objective ON dependent_objective.id = dependent.objective_id
                     WHERE downstream.prerequisite_task_id = task.id
                       AND dependent.status = 'queued'
                       AND dependent_objective.status = 'active'
                       AND NOT EXISTS (SELECT 1 FROM task_dependencies other_dependency
                         JOIN tasks other_prerequisite ON other_prerequisite.id = other_dependency.prerequisite_task_id
                         WHERE other_dependency.task_id = dependent.id
                           AND other_dependency.prerequisite_task_id <> task.id
                           AND other_prerequisite.status <> 'completed')
                       AND NOT EXISTS (SELECT 1 FROM task_blockers blocker
                         WHERE blocker.task_id = dependent.id AND blocker.status = 'open'))
                    AS dependency_unblock_count
             FROM tasks task JOIN objectives objective ON objective.id = task.objective_id
             WHERE objective.project_id = ?1
             ORDER BY task.priority DESC,
                      CASE WHEN task.due_date IS NULL THEN 1 ELSE 0 END,
                      task.due_date ASC, dependency_unblock_count DESC, task.id ASC",
        )
        .map_err(db_error)?
        .query_map(params![project_id], |row| {
            Ok(CurrentTask {
                id: row.get(0)?,
                objective_id: row.get(1)?,
                revision: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                priority: row.get(5)?,
                due_date: row.get(6)?,
                effort_minutes: row.get(7)?,
                next_action: row.get(8)?,
                dependency_unblock_count: row.get(9)?,
            })
        })
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)
}

fn resume_task(task: &CurrentTask, reasons: &[TaskBlockReason]) -> ResumeTask {
    ResumeTask {
        id: task.id,
        objective_id: task.objective_id,
        revision: task.revision,
        title: bounded_text(&task.title, 120),
        status: task.status.clone(),
        priority: task.priority,
        due_date: task.due_date.clone(),
        effort_minutes: task.effort_minutes,
        next_action: task
            .next_action
            .as_deref()
            .map(|text| bounded_text(text, 160)),
        dependency_unblock_count: task.dependency_unblock_count,
        reasons: reasons
            .iter()
            .take(3)
            .map(|reason| ResumeReason {
                code: reason.code.clone(),
                message: bounded_text(&reason.message, 160),
                related_task_id: reason.related_task_id,
                blocker_id: reason.blocker_id,
            })
            .collect(),
        omitted_reason_count: reasons.len().saturating_sub(3),
    }
}

fn bounded_text(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes.saturating_sub(3);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

fn refresh_omitted(packet: &mut ProjectResume) {
    packet.limits.objective_omitted = packet.limits.objective_total - packet.objectives.len();
    packet.limits.remaining_omitted = packet.limits.remaining_total - packet.remaining.len();
    packet.limits.next_eligible_omitted =
        packet.limits.next_eligible_total - packet.next_eligible.len();
    packet.limits.blocked_omitted = packet.limits.blocked_total - packet.blocked.len();
    packet.limits.delta_omitted = packet.limits.delta_total - packet.delta_since_checkpoint.len();
    packet.limits.recent_decision_omitted =
        packet.limits.recent_decision_total - packet.recent_decisions.len();
}

fn trim_longest_list(packet: &mut ProjectResume) -> bool {
    let lengths = [
        packet.objectives.len(),
        packet.remaining.len(),
        packet.next_eligible.len(),
        packet.blocked.len(),
        packet.delta_since_checkpoint.len(),
        packet.recent_decisions.len(),
    ];
    let Some((index, &length)) = lengths
        .iter()
        .enumerate()
        .filter(|(index, length)| *index != 2 || **length > 1)
        .max_by_key(|(_, length)| *length)
    else {
        return false;
    };
    if length == 0 {
        return false;
    }
    match index {
        0 => {
            packet.objectives.pop();
        }
        1 => {
            packet.remaining.pop();
        }
        2 => {
            packet.next_eligible.pop();
        }
        3 => {
            packet.blocked.pop();
        }
        4 => {
            packet.delta_since_checkpoint.pop();
        }
        _ => {
            packet.recent_decisions.pop();
        }
    }
    true
}

fn checkpoint_deltas(
    connection: &rusqlite::Connection,
    checkpoint: &ProjectCheckpoint,
    current: &HashMap<i64, (i64, String)>,
) -> Result<Vec<ResumeDelta>, String> {
    let previous: HashMap<i64, (i64, String)> = connection
        .prepare(
            "SELECT task_id, revision, status FROM project_checkpoint_tasks
             WHERE checkpoint_id = ?1",
        )
        .map_err(db_error)?
        .query_map(params![checkpoint.id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                (row.get::<_, i64>(1)?, row.get::<_, String>(2)?),
            ))
        })
        .map_err(db_error)?
        .collect::<Result<HashMap<_, _>, _>>()
        .map_err(db_error)?;
    let ids: BTreeSet<i64> = previous.keys().chain(current.keys()).copied().collect();
    let mut result = Vec::new();
    for task_id in ids {
        let before = previous.get(&task_id);
        let after = current.get(&task_id);
        if before == after {
            continue;
        }
        let event_id: Option<i64> = connection
            .query_row(
                "SELECT MAX(id) FROM events WHERE id > ?1
                   AND aggregate_type = 'task' AND aggregate_id = ?2",
                params![checkpoint.source_event_id, task_id.to_string()],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        result.push(ResumeDelta {
            task_id,
            change: match (before, after) {
                (None, Some(_)) => "added",
                (Some(_), None) => "removed",
                _ => "changed",
            }
            .to_string(),
            from_revision: before.map(|item| item.0),
            to_revision: after.map(|item| item.0),
            from_status: before.map(|item| item.1.clone()),
            to_status: after.map(|item| item.1.clone()),
            event_id,
        });
    }
    Ok(result)
}

fn latest_verification(
    connection: &rusqlite::Connection,
    project_id: i64,
) -> Result<Option<ResumeVerification>, String> {
    let run: Option<(i64, String, Option<i64>, Option<i64>)> = connection
        .query_row(
            "SELECT id, status, finished_at, workflow_version_id
             FROM runs WHERE project_id = ?1 ORDER BY id DESC LIMIT 1",
            params![project_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(db_error)?;
    let Some((run_id, status, finished_at, workflow_version_id)) = run else {
        return Ok(None);
    };
    let non_passing_mandatory_checks: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM quality_checks
             WHERE run_id = ?1 AND mandatory = 1 AND status <> 'passed'",
            params![run_id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    let mandatory_checks_changed_since = if let Some(version_id) = workflow_version_id {
        let original: Option<(i64, String)> = connection
            .query_row(
                "SELECT workflow_id, definition_json FROM workflow_versions WHERE id = ?1",
                params![version_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((workflow_id, original_json)) = original {
            let current_json: String = connection
                .query_row(
                    "SELECT definition_json FROM workflow_versions
                     WHERE workflow_id = ?1 ORDER BY version DESC LIMIT 1",
                    params![workflow_id],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            match (
                declared_mandatory_checks(&original_json)?,
                declared_mandatory_checks(&current_json)?,
            ) {
                (Some(original), Some(current)) => Some(original != current),
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    };
    Ok(Some(ResumeVerification {
        run_id,
        status,
        finished_at,
        workflow_version_id,
        non_passing_mandatory_checks,
        mandatory_checks_changed_since,
    }))
}

fn declared_mandatory_checks(definition_json: &str) -> Result<Option<BTreeSet<String>>, String> {
    let definition: Value =
        serde_json::from_str(definition_json).map_err(|error| error.to_string())?;
    let Some(checks) = definition.get("mandatory_checks") else {
        return Ok(None);
    };
    let checks = checks
        .as_array()
        .ok_or_else(|| "workflow mandatory_checks is not an array".to_string())?;
    let checks = checks
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "workflow mandatory_checks contains a non-string".to_string())
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(Some(checks))
}
