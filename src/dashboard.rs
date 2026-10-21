use crate::{delegation::DashboardDelegationSummary, ProjectSnapshot, Store};
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
    pub project_id: i64,
    pub project_name: String,
    pub title: String,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: i64,
    pub project_id: i64,
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
pub struct AgentActivitySummary {
    /// First stored hook event ID; stable for this observed agent within the run.
    pub id: i64,
    pub run_id: i64,
    pub agent_type: String,
    pub model: Option<String>,
    pub started_at: Option<i64>,
    pub stopped_at: Option<i64>,
    pub last_event_kind: String,
    pub last_event_at: i64,
    pub event_count: i64,
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
    pub hot_scope_count: i64,
    pub largest_scope_hot_bytes: i64,
    pub warm_bytes: i64,
    pub cold_bytes: i64,
    pub canonical_count: i64,
    pub episodic_count: i64,
    pub evidence_count: i64,
    pub candidate_count: i64,
    pub invalidated_count: i64,
    pub protected_artifact_count: i64,
    pub duplicate_memory_groups: i64,
    pub stale_memory_count: i64,
    pub reused_memory_count: i64,
    pub context_transfer_bytes: i64,
    pub context_reintroduced_bytes: i64,
    pub context_reintroduction_basis_points: i64,
    pub context_amplification_basis_points: i64,
    pub retrieval_usefulness_basis_points: i64,
    pub pending_maintenance_proposals: i64,
    pub sqlite_bytes: i64,
    pub sqlite_growth_bytes: i64,
    pub orphaned_evidence_count: i64,
    pub duplicate_artifact_groups: i64,
    pub bytes_collected: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub projects: Vec<ProjectSnapshot>,
    pub objectives: Vec<ObjectiveSummary>,
    pub blockers: Vec<BlockerSummary>,
    pub recent_runs: Vec<RunSummary>,
    pub agent_activity: Vec<AgentActivitySummary>,
    pub failed_checks: Vec<FailedCheckSummary>,
    pub delegations: Vec<DashboardDelegationSummary>,
    pub memory_health: MemoryHealthSummary,
}

impl Store {
    pub fn dashboard_snapshot(&self) -> Result<DashboardSnapshot, String> {
        let connection = self.connection()?;
        let projects = self.list_projects()?;
        let delegations = self.dashboard_delegations(10)?;
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
                "SELECT t.id, t.objective_id, p.id, p.name, t.title, t.updated_at
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
                    project_id: row.get(2)?,
                    project_name: row.get(3)?,
                    title: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let recent_runs = connection
            .raw()
            .prepare(
                "SELECT r.id, r.project_id, p.name, o.title, t.title, w.workflow_key, v.version,
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
                    project_id: row.get(1)?,
                    project_name: row.get(2)?,
                    objective_title: row.get(3)?,
                    task_title: row.get(4)?,
                    workflow_key: row.get(5)?,
                    workflow_version: row.get(6)?,
                    status: row.get(7)?,
                    summary: row.get(8)?,
                    started_at: row.get(9)?,
                    finished_at: row.get(10)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        let agent_activity = connection
            .raw()
            .prepare(
                "SELECT MIN(h.event_id), h.run_id, MAX(h.agent_type), MAX(h.model_slug),
                        MIN(CASE WHEN h.event_kind = 'started' THEN h.created_at END),
                        MAX(CASE WHEN h.event_kind = 'stopped' THEN h.created_at END),
                        (SELECT latest.event_kind FROM codex_subagent_hook_events latest
                         WHERE latest.run_id = h.run_id AND latest.session_hash = h.session_hash
                           AND latest.turn_hash = h.turn_hash AND latest.agent_id_hash = h.agent_id_hash
                         ORDER BY latest.created_at DESC, latest.event_id DESC LIMIT 1),
                        MAX(h.created_at), COUNT(*)
                 FROM codex_subagent_hook_events h
                 WHERE h.run_id IN (
                     SELECT id FROM runs ORDER BY started_at DESC, id DESC LIMIT 10
                 )
                   AND NOT EXISTS (
                     SELECT 1
                     FROM delegation_worker_bindings b
                     JOIN delegations d ON d.id = b.delegation_id
                     WHERE d.run_id = h.run_id
                       AND b.session_hash = h.session_hash
                       AND b.turn_hash = h.turn_hash
                       AND b.agent_id_hash = h.agent_id_hash
                   )
                 GROUP BY h.run_id, h.session_hash, h.turn_hash, h.agent_id_hash
                 ORDER BY MAX(h.created_at) DESC, MIN(h.event_id) DESC
                 LIMIT 40",
            )
            .map_err(sqlite_error)?
            .query_map([], |row| {
                Ok(AgentActivitySummary {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    agent_type: row.get(2)?,
                    model: row.get(3)?,
                    started_at: row.get(4)?,
                    stopped_at: row.get(5)?,
                    last_event_kind: row.get(6)?,
                    last_event_at: row.get(7)?,
                    event_count: row.get(8)?,
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
        let mut memory_health = connection
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
                        hot_scope_count: 0,
                        largest_scope_hot_bytes: 0,
                        warm_bytes: row.get(1)?,
                        cold_bytes: row.get(2)?,
                        canonical_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                        episodic_count: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                        evidence_count: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                        candidate_count: row.get::<_, Option<i64>>(6)?.unwrap_or(0),
                        invalidated_count: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                        protected_artifact_count: 0,
                        duplicate_memory_groups: 0,
                        stale_memory_count: 0,
                        reused_memory_count: 0,
                        context_transfer_bytes: 0,
                        context_reintroduced_bytes: 0,
                        context_reintroduction_basis_points: 0,
                        context_amplification_basis_points: 0,
                        retrieval_usefulness_basis_points: 0,
                        pending_maintenance_proposals: 0,
                        sqlite_bytes: 0,
                        sqlite_growth_bytes: 0,
                        orphaned_evidence_count: 0,
                        duplicate_artifact_groups: 0,
                        bytes_collected: 0,
                    })
                },
            )
            .map_err(sqlite_error)?;
        (
            memory_health.hot_scope_count,
            memory_health.largest_scope_hot_bytes,
        ) = connection
            .raw()
            .query_row(
                "SELECT COUNT(*), COALESCE(MAX(scope_bytes), 0) FROM (
               SELECT scope_key, SUM(length(CAST(body AS BLOB))) AS scope_bytes
               FROM memories WHERE temperature = 'hot' GROUP BY scope_key
             )",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
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
        memory_health.protected_artifact_count = protected_artifact_count;
        let (duplicate_memory_groups, reused_memory_count): (i64, i64) = connection.raw().query_row(
            "SELECT
               (SELECT COUNT(*) FROM (SELECT scope_key, kind, content_hash FROM memories
                WHERE review_state = 'accepted' AND superseded_by_id IS NULL AND invalidated_at IS NULL
                GROUP BY scope_key, kind, content_hash HAVING COUNT(*) > 1)),
               COALESCE((SELECT SUM(reuse_count) FROM memories), 0)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(sqlite_error)?;
        memory_health.duplicate_memory_groups = duplicate_memory_groups;
        memory_health.reused_memory_count = reused_memory_count;
        memory_health.stale_memory_count = connection.raw().query_row(
            "SELECT COUNT(*) FROM memories WHERE invalidated_at IS NOT NULL OR superseded_by_id IS NOT NULL
             OR (valid_until IS NOT NULL AND valid_until <= CAST(strftime('%s','now') AS INTEGER))",
            [], |row| row.get(0),
        ).map_err(sqlite_error)?;
        (memory_health.context_transfer_bytes, memory_health.context_reintroduced_bytes) = connection.raw().query_row(
            "SELECT COALESCE(SUM(size_bytes), 0), COALESCE(SUM(CASE WHEN reintroduced = 1 THEN size_bytes ELSE 0 END), 0)
             FROM context_transfers",
            [], |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(sqlite_error)?;
        if memory_health.context_transfer_bytes > 0 {
            memory_health.context_reintroduction_basis_points = memory_health
                .context_reintroduced_bytes
                .saturating_mul(10_000)
                / memory_health.context_transfer_bytes;
        }
        let unique_memory_transfer_bytes: i64 = connection
            .raw()
            .query_row(
                "SELECT COALESCE(SUM(bytes), 0) FROM (
               SELECT memory_id, content_hash, MAX(size_bytes) AS bytes FROM context_transfers
               GROUP BY memory_id, content_hash
             )",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if unique_memory_transfer_bytes > 0 {
            memory_health.context_amplification_basis_points =
                memory_health.context_transfer_bytes.saturating_mul(10_000)
                    / unique_memory_transfer_bytes;
        }
        let (useful_retrievals, retrieval_count): (i64, i64) = connection
            .raw()
            .query_row(
                "SELECT COALESCE(SUM(useful_retrievals), 0), COALESCE(SUM(retrieval_count), 0)
             FROM memory_policy_evaluations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(sqlite_error)?;
        if retrieval_count > 0 {
            memory_health.retrieval_usefulness_basis_points =
                useful_retrievals.saturating_mul(10_000) / retrieval_count;
        }
        memory_health.pending_maintenance_proposals = connection
            .raw()
            .query_row(
                "SELECT COUNT(*) FROM memory_maintenance_proposals WHERE status = 'proposed'",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if let Some(snapshot) = self.latest_storage_snapshot()? {
            memory_health.sqlite_bytes = snapshot.sqlite_bytes;
            memory_health.sqlite_growth_bytes = snapshot.growth_bytes;
            memory_health.orphaned_evidence_count = snapshot.orphaned_evidence_count;
            memory_health.duplicate_artifact_groups = snapshot.duplicate_artifact_groups;
            memory_health.bytes_collected = snapshot.bytes_collected;
        }
        Ok(DashboardSnapshot {
            projects,
            objectives,
            blockers,
            recent_runs,
            agent_activity,
            failed_checks,
            delegations,
            memory_health,
        })
    }
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}
