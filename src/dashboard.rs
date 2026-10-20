use crate::{ProjectSnapshot, Store};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveSummary {
    pub id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub title: String,
    pub status: String,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockerSummary {
    pub id: i64,
    pub objective_id: i64,
    pub project_name: String,
    pub title: String,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: i64,
    pub project_name: String,
    pub objective_title: Option<String>,
    pub task_title: Option<String>,
    pub workflow_key: Option<String>,
    pub workflow_version: Option<i64>,
    pub status: String,
    pub summary: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FailedCheckSummary {
    pub id: i64,
    pub run_id: i64,
    pub project_name: String,
    pub check_key: String,
    pub mandatory: bool,
    pub status: String,
    pub details: Value,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryHealthSummary {
    pub hot_bytes: i64,
    pub warm_bytes: i64,
    pub cold_bytes: i64,
    pub canonical_count: i64,
    pub episodic_count: i64,
    pub evidence_count: i64,
    pub candidate_count: i64,
    pub invalidated_count: i64,
    pub protected_artifact_count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub projects: Vec<ProjectSnapshot>,
    pub objectives: Vec<ObjectiveSummary>,
    pub blockers: Vec<BlockerSummary>,
    pub recent_runs: Vec<RunSummary>,
    pub failed_checks: Vec<FailedCheckSummary>,
    pub memory_health: MemoryHealthSummary,
}

impl Store {
    pub fn dashboard_snapshot(&self) -> Result<DashboardSnapshot, String> {
        let connection = self.connection()?;
        let projects = self.list_projects()?;
        let objectives = connection
            .raw()
            .prepare(
                "SELECT o.id, o.project_id, p.name, o.title, o.status, o.updated_at
                 FROM objectives o JOIN projects p ON p.id = o.project_id
                 WHERE o.status IN ('active', 'blocked')
                 ORDER BY CASE o.status WHEN 'blocked' THEN 0 ELSE 1 END, o.updated_at DESC, o.id DESC
                 LIMIT 20",
            )
            .map_err(sqlite_error)?
            .query_map([], |row| {
                Ok(ObjectiveSummary {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    project_name: row.get(2)?,
                    title: row.get(3)?,
                    status: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let blockers = connection
            .raw()
            .prepare(
                "SELECT t.id, t.objective_id, p.name, t.title, t.updated_at
                 FROM tasks t JOIN objectives o ON o.id = t.objective_id
                 JOIN projects p ON p.id = o.project_id
                 WHERE t.status = 'blocked'
                 ORDER BY t.updated_at DESC, t.id DESC LIMIT 20",
            )
            .map_err(sqlite_error)?
            .query_map([], |row| {
                Ok(BlockerSummary {
                    id: row.get(0)?,
                    objective_id: row.get(1)?,
                    project_name: row.get(2)?,
                    title: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let recent_runs = connection
            .raw()
            .prepare(
                "SELECT r.id, p.name, o.title, t.title, w.workflow_key, v.version,
                        r.status, r.summary, r.started_at, r.finished_at
                 FROM runs r JOIN projects p ON p.id = r.project_id
                 LEFT JOIN objectives o ON o.id = r.objective_id
                 LEFT JOIN tasks t ON t.id = r.task_id
                 LEFT JOIN workflow_versions v ON v.id = r.workflow_version_id
                 LEFT JOIN workflows w ON w.id = v.workflow_id
                 ORDER BY r.started_at DESC, r.id DESC LIMIT 10",
            )
            .map_err(sqlite_error)?
            .query_map([], |row| {
                Ok(RunSummary {
                    id: row.get(0)?,
                    project_name: row.get(1)?,
                    objective_title: row.get(2)?,
                    task_title: row.get(3)?,
                    workflow_key: row.get(4)?,
                    workflow_version: row.get(5)?,
                    status: row.get(6)?,
                    summary: row.get(7)?,
                    started_at: row.get(8)?,
                    finished_at: row.get(9)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let failed_checks = connection
            .raw()
            .prepare(
                "SELECT q.id, q.run_id, p.name, q.check_key, q.mandatory, q.status, q.details_json, q.updated_at
                 FROM quality_checks q JOIN runs r ON r.id = q.run_id JOIN projects p ON p.id = r.project_id
                 WHERE q.status IN ('failed', 'inconclusive')
                 ORDER BY q.updated_at DESC, q.id DESC LIMIT 20",
            )
            .map_err(sqlite_error)?
            .query_map([], |row| {
                let details_json: String = row.get(6)?;
                let details = serde_json::from_str(&details_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(FailedCheckSummary {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    project_name: row.get(2)?,
                    check_key: row.get(3)?,
                    mandatory: row.get(4)?,
                    status: row.get(5)?,
                    details,
                    updated_at: row.get(7)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let memory_health = connection
            .raw()
            .query_row(
                "SELECT
                   COALESCE(SUM(CASE WHEN temperature = 'hot' THEN length(CAST(body AS BLOB)) ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN temperature = 'warm' THEN length(CAST(body AS BLOB)) ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN temperature = 'cold' THEN length(CAST(body AS BLOB)) ELSE 0 END), 0),
                   SUM(CASE WHEN memory_class = 'canonical' THEN 1 ELSE 0 END),
                   SUM(CASE WHEN memory_class = 'episodic' THEN 1 ELSE 0 END),
                   SUM(CASE WHEN memory_class = 'evidence' THEN 1 ELSE 0 END),
                   SUM(CASE WHEN review_state = 'candidate' THEN 1 ELSE 0 END),
                   SUM(CASE WHEN invalidated_at IS NOT NULL THEN 1 ELSE 0 END)
                 FROM memories",
                [],
                |row| {
                    Ok(MemoryHealthSummary {
                        hot_bytes: row.get(0)?,
                        warm_bytes: row.get(1)?,
                        cold_bytes: row.get(2)?,
                        canonical_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                        episodic_count: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                        evidence_count: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                        candidate_count: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                        invalidated_count: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                        protected_artifact_count: 0,
                    })
                },
            )
            .map_err(sqlite_error)?;
        let protected_artifact_count: i64 = connection
            .raw()
            .query_row(
                "SELECT COUNT(*) FROM evidence_artifacts WHERE protected = 1",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        let mut memory_health = memory_health;
        memory_health.protected_artifact_count = protected_artifact_count;
        Ok(DashboardSnapshot {
            projects,
            objectives,
            blockers,
            recent_runs,
            failed_checks,
            memory_health,
        })
    }
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}
