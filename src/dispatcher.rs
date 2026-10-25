//! Durable, idempotent role-dispatch state.
//!
//! This module deliberately stores assignment IDs and delivery metadata only. The existing
//! delegation packet, project state, memory, and evidence tables remain authoritative. A queued
//! record is not evidence that a Codex turn started or completed.

use crate::app_server::{AppServerMessage, AppServerProcess};
use crate::delegation::DelegationTaskPacket;
use crate::events::append_event;
use crate::hash::content_hash;
use crate::Store;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uuid::Uuid;

const MAX_BATCH_SIZE: usize = 32;
const MAX_ERROR_BYTES: usize = 2_000;
const MAX_EVENT_DETAILS_BYTES: usize = 8_192;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleDispatchRecord {
    pub id: i64,
    pub delegation_id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub task_title: String,
    pub role: String,
    pub status: String,
    pub attempt_count: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleDispatchAdmission {
    pub delegation_id: i64,
    pub accepted: bool,
    pub status: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleDispatchWorkItem {
    pub id: i64,
    pub delegation_id: i64,
    pub project_id: i64,
    pub project_name: String,
    pub project_path: String,
    pub run_id: i64,
    pub role: String,
    pub parent_delegation_id: Option<i64>,
    pub task_packet: DelegationTaskPacket,
    pub main_workflow_brief: Option<Value>,
    pub packet_hash: String,
    pub requested_model: Option<String>,
    pub requested_effort: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleThreadBinding {
    pub binding_key: String,
    pub role_key: Option<String>,
    pub delegation_id: Option<i64>,
    pub project_id: Option<i64>,
    pub thread_id: String,
    pub status: String,
    pub last_turn_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleCapability {
    pub token: String,
    pub role_kind: String,
    pub project_id: i64,
    pub delegation_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleCapabilityScope {
    pub role_kind: String,
    pub project_id: Option<i64>,
    pub delegation_id: Option<i64>,
    pub capability_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleTurnInboxRecord {
    pub id: i64,
    pub source_event_id: i64,
    pub source_delegation_id: i64,
    pub target_binding_key: String,
    pub target_delegation_id: Option<i64>,
    pub project_id: i64,
    pub event_type: String,
    pub payload_json: String,
    pub status: String,
    pub attempt_count: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn role_allowed_tools(role_kind: &str) -> &'static [&'static str] {
    match role_kind {
        "main_overseer" => &[
            "read_memories",
            "search_memory_candidates",
            "record_delegation_update",
            "review_role_work_plan",
            "review_delegation_report",
            "request_owner_approval",
            "relay_owner_decision_to_project_manager",
        ],
        "sub_overseer" => &[
            "read_memories",
            "search_memory_candidates",
            "record_delegation_update",
            "record_role_work_plan",
            "prepare_delegation",
            "record_delegation_activation",
            "dispatch_prepared_delegations",
            "submit_delegation_report",
            "review_delegation_report",
            "complete_project_manager_assignment",
        ],
        "worker" => &[
            "read_memories",
            "search_memory_candidates",
            "record_delegation_update",
            "submit_delegation_report",
            "record_delegation_quality_check",
        ],
        _ => &[],
    }
}

impl Store {
    /// Route only actionable role events to the lowest logical role that can act. Ordinary
    /// progress stays in the event stream; PM reviews, PM results, and Owner decisions wake Main.
    pub fn route_pending_role_events(&self, limit: usize) -> Result<usize, String> {
        if !(1..=1000).contains(&limit) {
            return Err("role event routing limit must be between 1 and 1000".to_string());
        }
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let last_event_id: i64 = transaction
            .query_row(
                "SELECT last_event_id FROM role_event_cursor WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        let events = {
            let mut statement = transaction
                .prepare(
                    "SELECT id, aggregate_type, aggregate_id, event_type, payload_json, created_at
                     FROM events WHERE id > ?1 ORDER BY id LIMIT ?2",
                )
                .map_err(sqlite_error)?;
            let rows = statement
                .query_map(params![last_event_id, limit as i64], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                })
                .map_err(sqlite_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?
        };
        let mut routed = 0;
        for (event_id, aggregate_type, aggregate_id, event_type, payload_json, created_at) in
            &events
        {
            let payload = serde_json::from_str::<Value>(payload_json).unwrap_or_else(|_| json!({}));
            let target = match (aggregate_type.as_str(), event_type.as_str()) {
                (
                    "delegation",
                    "agent_action_required"
                    | "agent_action_resolved"
                    | "delegation_report_submitted"
                    | "delegation_report_reviewed"
                    | "role_work_plan_recorded"
                    | "owner_decision_relayed",
                ) => {
                    let Ok(source_id) = aggregate_id.parse::<i64>() else {
                        continue;
                    };
                    let source = transaction.query_row(
                        "SELECT role, project_id, parent_delegation_id FROM delegations WHERE id = ?1",
                        params![source_id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<i64>>(2)?)),
                    ).optional().map_err(sqlite_error)?;
                    let Some((role, project_id, parent_id)) = source else {
                        continue;
                    };
                    match role.as_str() {
                        "worker"
                            if matches!(
                                event_type.as_str(),
                                "agent_action_required"
                                    | "agent_action_resolved"
                                    | "delegation_report_submitted"
                            ) =>
                        {
                            let Some(parent_id) = parent_id else { continue };
                            let active_manager = active_project_manager(&transaction, project_id)?;
                            let target_key = format!("project-manager:{project_id}");
                            let is_active = active_manager == Some(parent_id);
                            Some((source_id, target_key, active_manager, project_id, if is_active { "queued" } else { "unroutable" }, (!is_active).then_some("Worker event has no matching active Project Manager assignment")))
                        }
                        "worker" if event_type == "delegation_report_reviewed" => None,
                        "sub_overseer"
                            if event_type == "role_work_plan_recorded"
                                && payload.get("planLevel").and_then(Value::as_str)
                                    == Some("project_manager") =>
                        {
                            let active_manager = active_project_manager(&transaction, project_id)?;
                            let is_active = active_manager == Some(source_id);
                            Some((
                                source_id,
                                "main-overseer".to_string(),
                                None,
                                project_id,
                                if is_active { "queued" } else { "unroutable" },
                                (!is_active).then_some(
                                    "Project Manager plan has no matching active assignment",
                                ),
                            ))
                        }
                        "sub_overseer"
                            if event_type == "agent_action_resolved"
                                && payload.get("source").and_then(Value::as_str)
                                    == Some("main_overseer") =>
                        {
                            let active_manager = active_project_manager(&transaction, project_id)?;
                            let is_active = active_manager == Some(source_id);
                            Some((source_id, format!("project-manager:{project_id}"), active_manager, project_id, if is_active { "queued" } else { "unroutable" }, (!is_active).then_some("Main's decision has no matching active Project Manager assignment")))
                        }
                        "sub_overseer"
                            if matches!(
                                event_type.as_str(),
                                "agent_action_required"
                                    | "agent_action_resolved"
                                    | "delegation_report_submitted"
                            ) =>
                        {
                            let active_manager = active_project_manager(&transaction, project_id)?;
                            let is_active = active_manager == Some(source_id);
                            Some((
                                source_id,
                                "main-overseer".to_string(),
                                None,
                                project_id,
                                if is_active { "queued" } else { "unroutable" },
                                (!is_active).then_some(
                                    "Project Manager event has no matching active assignment",
                                ),
                            ))
                        }
                        "sub_overseer"
                            if matches!(
                                event_type.as_str(),
                                "delegation_report_reviewed" | "owner_decision_relayed"
                            ) && (event_type != "delegation_report_reviewed"
                                || payload.get("reviewerRole").and_then(Value::as_str)
                                    == Some("main_overseer")) =>
                        {
                            let active_manager = active_project_manager(&transaction, project_id)?;
                            let is_active = active_manager == Some(source_id);
                            Some((
                                source_id,
                                format!("project-manager:{project_id}"),
                                active_manager,
                                project_id,
                                if is_active { "queued" } else { "unroutable" },
                                (!is_active).then_some(
                                    "Project Manager response has no matching active assignment",
                                ),
                            ))
                        }
                        _ => None,
                    }
                }
                ("role_work_plan", "role_work_plan_reviewed")
                    if payload.get("reviewerRole").and_then(Value::as_str)
                        == Some("main_overseer") =>
                {
                    let Ok(plan_id) = aggregate_id.parse::<i64>() else {
                        continue;
                    };
                    let plan = transaction
                        .query_row(
                            "SELECT project_id, manager_delegation_id FROM role_work_plans
                         WHERE id = ?1 AND plan_level = 'project_manager'",
                            params![plan_id],
                            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
                        )
                        .optional()
                        .map_err(sqlite_error)?;
                    let Some((project_id, Some(manager_id))) = plan else {
                        continue;
                    };
                    let active_manager = active_project_manager(&transaction, project_id)?;
                    let is_active = active_manager == Some(manager_id);
                    Some((
                        manager_id,
                        format!("project-manager:{project_id}"),
                        active_manager,
                        project_id,
                        if is_active { "queued" } else { "unroutable" },
                        (!is_active).then_some(
                            "reviewed plan has no matching active Project Manager assignment",
                        ),
                    ))
                }
                ("run", "owner_approval_decided")
                    if payload.get("decisionOrigin").and_then(Value::as_str)
                        == Some("desktop_owner_ui") =>
                {
                    let request_id = payload.get("requestId").and_then(Value::as_i64);
                    let Some(request_id) = request_id else {
                        continue;
                    };
                    let source = transaction.query_row(
                        "SELECT manager_delegation_id, project_id FROM owner_approval_requests WHERE id = ?1",
                        params![request_id],
                        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
                    ).optional().map_err(sqlite_error)?;
                    let Some((manager_id, project_id)) = source else {
                        continue;
                    };
                    Some((
                        manager_id,
                        "main-overseer".to_string(),
                        None,
                        project_id,
                        "queued",
                        None,
                    ))
                }
                _ => None,
            };
            let Some((source_id, target_key, target_delegation, project_id, status, error)) =
                target
            else {
                continue;
            };
            routed += insert_role_inbox_message(
                &transaction,
                *event_id,
                source_id,
                &target_key,
                target_delegation,
                project_id,
                event_type,
                payload_json,
                status,
                error,
                now.max(*created_at),
            )? as usize;
        }
        if let Some((last_id, ..)) = events.last() {
            transaction
                .execute(
                    "UPDATE role_event_cursor SET last_event_id = ?1, updated_at = ?2
                     WHERE singleton = 1 AND last_event_id = ?3",
                    params![last_id, now, last_event_id],
                )
                .map_err(sqlite_error)?;
        }
        transaction.commit().map_err(sqlite_error)?;
        Ok(routed)
    }

    /// Compatibility alias retained for callers written before the Main route was added.
    pub fn route_pending_worker_events(&self, limit: usize) -> Result<usize, String> {
        self.route_pending_role_events(limit)
    }

    pub fn claim_next_role_inbox_message(
        &self,
        active_binding_keys: &[String],
    ) -> Result<Option<RoleTurnInboxRecord>, String> {
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let queued = {
            let mut statement = transaction
                .prepare(
                    "SELECT id, source_event_id, source_delegation_id, target_binding_key, target_delegation_id,
                            project_id, event_type, payload_json, attempt_count, created_at
                     FROM role_turn_inbox WHERE status = 'queued' ORDER BY id LIMIT 100",
                )
                .map_err(sqlite_error)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                    ))
                })
                .map_err(sqlite_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?
        };
        let message = queued
            .into_iter()
            .find(|(_, _, _, binding_key, ..)| !active_binding_keys.contains(binding_key));
        let Some((
            id,
            source_event_id,
            source_delegation_id,
            target_binding_key,
            target_delegation_id,
            project_id,
            event_type,
            payload_json,
            attempt_count,
            created_at,
        )) = message
        else {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(None);
        };
        transaction
            .execute(
                "UPDATE role_turn_inbox SET status = 'starting', attempt_count = attempt_count + 1,
                   updated_at = ?1 WHERE id = ?2 AND status = 'queued'",
                params![now, id],
            )
            .map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO role_turn_inbox_events (inbox_id, event_type, details_json, created_at)
                 VALUES (?1, 'starting', '{}', ?2)",
                params![id, now],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(Some(RoleTurnInboxRecord {
            id,
            source_event_id,
            source_delegation_id,
            target_binding_key,
            target_delegation_id,
            project_id,
            event_type,
            payload_json,
            status: "starting".to_string(),
            attempt_count: attempt_count + 1,
            last_error: None,
            created_at,
            updated_at: now,
        }))
    }

    pub fn set_role_inbox_status(
        &self,
        inbox_id: i64,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        const STATUSES: &[&str] = &[
            "queued",
            "starting",
            "dispatched",
            "running",
            "completed",
            "blocked",
            "needs_reconcile",
            "unroutable",
        ];
        if !STATUSES.contains(&status) {
            return Err("unsupported role inbox status".to_string());
        }
        if error.is_some_and(|message| message.trim().is_empty() || message.len() > MAX_ERROR_BYTES)
        {
            return Err("role inbox error must be non-empty and at most 2000 bytes".to_string());
        }
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let previous: Option<String> = transaction
            .query_row(
                "SELECT status FROM role_turn_inbox WHERE id = ?1",
                params![inbox_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        let previous =
            previous.ok_or_else(|| format!("role inbox message {inbox_id} was not found"))?;
        if previous == status {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(());
        }
        let valid = matches!(
            (previous.as_str(), status),
            ("queued", "starting" | "unroutable")
                | ("starting", "dispatched" | "blocked" | "needs_reconcile")
                | (
                    "dispatched",
                    "running" | "completed" | "blocked" | "needs_reconcile"
                )
                | ("running", "completed" | "blocked" | "needs_reconcile")
                | ("needs_reconcile", "blocked")
        );
        if !valid {
            return Err(format!(
                "role inbox cannot transition from {previous} to {status}"
            ));
        }
        transaction
            .execute(
                "UPDATE role_turn_inbox SET status = ?1, last_error = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![status, error, now, inbox_id],
            )
            .map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO role_turn_inbox_events (inbox_id, event_type, details_json, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    inbox_id,
                    status,
                    json!({ "from": previous, "errorHash": error.map(content_hash) }).to_string(),
                    now,
                ],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)
    }

    pub fn mark_uncertain_role_inbox_for_reconciliation(&self) -> Result<usize, String> {
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let ids = {
            let mut statement = transaction
                .prepare("SELECT id FROM role_turn_inbox WHERE status IN ('starting', 'dispatched', 'running')")
                .map_err(sqlite_error)?;
            let rows = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(sqlite_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?
        };
        for id in &ids {
            transaction
                .execute(
                    "UPDATE role_turn_inbox SET status = 'needs_reconcile', updated_at = ?1,
                       last_error = COALESCE(last_error, 'dispatcher restarted during event delivery; inspect before retry')
                     WHERE id = ?2",
                    params![now, id],
                )
                .map_err(sqlite_error)?;
            transaction
                .execute(
                    "INSERT INTO role_turn_inbox_events (inbox_id, event_type, details_json, created_at)
                     VALUES (?1, 'needs_reconcile', '{\"reason\":\"dispatcher_restart\"}', ?2)",
                    params![id, now],
                )
                .map_err(sqlite_error)?;
        }
        transaction.commit().map_err(sqlite_error)?;
        Ok(ids.len())
    }

    pub fn role_turn_inbox(&self, limit: usize) -> Result<Vec<RoleTurnInboxRecord>, String> {
        if !(1..=1000).contains(&limit) {
            return Err("role inbox limit must be between 1 and 1000".to_string());
        }
        let connection = self.connection()?;
        let mut statement = connection
            .raw()
            .prepare(
                "SELECT id, source_event_id, source_delegation_id, target_binding_key, target_delegation_id, project_id,
                        event_type, payload_json, status, attempt_count, last_error, created_at, updated_at
                 FROM role_turn_inbox ORDER BY id DESC LIMIT ?1",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map(params![limit as i64], |row| {
                Ok(RoleTurnInboxRecord {
                    id: row.get(0)?,
                    source_event_id: row.get(1)?,
                    source_delegation_id: row.get(2)?,
                    target_binding_key: row.get(3)?,
                    target_delegation_id: row.get(4)?,
                    project_id: row.get(5)?,
                    event_type: row.get(6)?,
                    payload_json: row.get(7)?,
                    status: row.get(8)?,
                    attempt_count: row.get(9)?,
                    last_error: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })
            .map_err(sqlite_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)
    }

    /// Rotate a per-thread bearer capability. The plaintext is returned once for the local
    /// app-server MCP environment; only its hash is retained in SQLite.
    pub fn issue_role_thread_capability(
        &self,
        binding_key: &str,
        role_kind: &str,
        project_id: i64,
        delegation_id: i64,
    ) -> Result<RoleCapability, String> {
        if binding_key.trim().is_empty()
            || !matches!(role_kind, "sub_overseer" | "worker")
            || project_id <= 0
            || delegation_id <= 0
            || (role_kind == "sub_overseer"
                && binding_key != format!("project-manager:{project_id}"))
            || (role_kind == "worker" && binding_key != format!("worker:{delegation_id}"))
        {
            return Err("role thread capability identity is invalid".to_string());
        }
        let token = Uuid::new_v4().simple().to_string();
        let token_hash = content_hash(&token);
        let now = unix_time();
        let connection = self.connection()?;
        connection
            .raw()
            .execute(
                "INSERT INTO role_thread_capabilities
                 (binding_key, role_kind, project_id, delegation_id, capability_hash, status,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, ?6)
                 ON CONFLICT(binding_key) DO UPDATE SET delegation_id = excluded.delegation_id,
                   capability_hash = excluded.capability_hash, status = 'active',
                   provider_id = NULL, provider_session_id = NULL, attempt_key = NULL,
                   updated_at = excluded.updated_at",
                params![
                    binding_key,
                    role_kind,
                    project_id,
                    delegation_id,
                    token_hash,
                    now
                ],
            )
            .map_err(sqlite_error)?;
        Ok(RoleCapability {
            token,
            role_kind: role_kind.to_string(),
            project_id,
            delegation_id,
        })
    }

    pub fn issue_main_overseer_capability(&self) -> Result<RoleCapability, String> {
        let token = Uuid::new_v4().simple().to_string();
        let token_hash = content_hash(&token);
        let now = unix_time();
        let connection = self.connection()?;
        connection
            .raw()
            .execute(
                "INSERT INTO main_overseer_capability
                 (singleton, role_key, capability_hash, status, created_at, updated_at)
                 VALUES (1, 'main-overseer', ?1, 'active', ?2, ?2)
                 ON CONFLICT(singleton) DO UPDATE SET capability_hash = excluded.capability_hash,
                   status = 'active', provider_id = NULL, provider_session_id = NULL,
                   attempt_key = NULL, updated_at = excluded.updated_at",
                params![token_hash, now],
            )
            .map_err(sqlite_error)?;
        Ok(RoleCapability {
            token,
            role_kind: "main_overseer".to_string(),
            project_id: 0,
            delegation_id: 0,
        })
    }

    pub fn revoke_role_thread_capability(&self, binding_key: &str) -> Result<(), String> {
        let connection = self.connection()?;
        connection
            .raw()
            .execute(
                "UPDATE role_thread_capabilities SET status = 'revoked', updated_at = ?1
                 WHERE binding_key = ?2 AND status = 'active'",
                params![unix_time(), binding_key],
            )
            .map_err(sqlite_error)?;
        if binding_key == "main-overseer" {
            connection
                .raw()
                .execute(
                    "UPDATE main_overseer_capability SET status = 'revoked', updated_at = ?1
                     WHERE singleton = 1 AND status = 'active'",
                    params![unix_time()],
                )
                .map_err(sqlite_error)?;
        }
        Ok(())
    }

    pub fn resolve_role_thread_capability(
        &self,
        token: &str,
    ) -> Result<RoleCapabilityScope, String> {
        if token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("role thread capability is invalid".to_string());
        }
        let token_hash = content_hash(token);
        let connection = self.connection()?;
        let main_role = connection
            .raw()
            .query_row(
                "SELECT role.role_kind FROM main_overseer_capability capability
                 JOIN logical_agent_roles role ON role.role_key = capability.role_key
                 WHERE capability.capability_hash = ?1 AND capability.status = 'active'",
                params![token_hash],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        if let Some(role_kind) = main_role {
            return Ok(RoleCapabilityScope {
                role_kind,
                project_id: None,
                delegation_id: None,
                capability_hash: token_hash,
            });
        }
        let capability = connection
            .raw()
            .query_row(
                "SELECT role_kind, project_id, delegation_id
                 FROM role_thread_capabilities
                 WHERE capability_hash = ?1 AND status = 'active'",
                params![token_hash],
                |row| {
                    Ok(RoleCapabilityScope {
                        role_kind: row.get(0)?,
                        project_id: Some(row.get(1)?),
                        delegation_id: Some(row.get(2)?),
                        capability_hash: token_hash.clone(),
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| "role thread capability is unknown or revoked".to_string())?;
        Ok(capability)
    }

    /// Authorization is evaluated from the stored role assignment and the exact tool arguments.
    /// Tool names are filtered for discoverability, but this check is the enforcement boundary.
    pub fn authorize_role_tool_call(
        &self,
        capability: &RoleCapabilityScope,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<(), String> {
        if !role_allowed_tools(&capability.role_kind).contains(&tool_name) {
            return Err(format!("role capability cannot call `{tool_name}`"));
        }
        if capability.role_kind == "main_overseer" {
            if capability.project_id.is_some() || capability.delegation_id.is_some() {
                return Err(
                    "global Main capability unexpectedly has project/delegation scope".to_string(),
                );
            }
            return self.authorize_main_tool_call(tool_name, arguments);
        }
        let project_id = capability
            .project_id
            .ok_or_else(|| "role capability has no project scope".to_string())?;
        let delegation_id = capability
            .delegation_id
            .ok_or_else(|| "role capability has no delegation assignment".to_string())?;
        let object = arguments.as_object();
        let number = |key: &str| {
            object
                .and_then(|values| values.get(key))
                .and_then(Value::as_i64)
        };
        let string = |key: &str| {
            object
                .and_then(|values| values.get(key))
                .and_then(Value::as_str)
        };
        let project_scope_id = project_id.to_string();
        let project_matches = || number("project_id") == Some(project_id);
        let delegation_matches = |key: &str| number(key) == Some(delegation_id);
        let active_delegation = self.capability_assignment_is_active(capability)?;
        if !active_delegation {
            return Err("role capability is not attached to an active assignment".to_string());
        }

        let authorized = match capability.role_kind.as_str() {
            "worker" => match tool_name {
                "read_memories" | "search_memory_candidates" => {
                    string("scope_type") == Some("project")
                        && string("scope_id") == Some(project_scope_id.as_str())
                }
                "record_delegation_update" | "submit_delegation_report" => {
                    delegation_matches("delegation_id")
                }
                "record_delegation_quality_check" => {
                    number("run_id") == self.capability_run_id(capability)?
                }
                _ => false,
            },
            "sub_overseer" => match tool_name {
                "read_memories" | "search_memory_candidates" => {
                    string("scope_type") == Some("project")
                        && string("scope_id") == Some(project_scope_id.as_str())
                }
                "record_delegation_update"
                | "submit_delegation_report"
                | "complete_project_manager_assignment" => delegation_matches("delegation_id"),
                "record_role_work_plan" => {
                    project_matches()
                        && string("plan_level") == Some("project_manager")
                        && number("manager_delegation_id") == Some(delegation_id)
                        && number("run_id") == self.capability_run_id(capability)?
                }
                "prepare_delegation" => {
                    project_matches()
                        && string("role") == Some("worker")
                        && number("parent_delegation_id") == Some(delegation_id)
                        && number("run_id") == self.capability_run_id(capability)?
                }
                "record_delegation_activation" => number("delegation_id").is_some_and(|child_id| {
                    self.is_child_delegation(child_id, capability)
                        .unwrap_or(false)
                }),
                "dispatch_prepared_delegations" => object
                    .and_then(|values| values.get("delegation_ids"))
                    .and_then(Value::as_array)
                    .is_some_and(|ids| {
                        !ids.is_empty()
                            && ids.iter().all(|id| {
                                id.as_i64().is_some_and(|child_id| {
                                    self.is_child_delegation(child_id, capability)
                                        .unwrap_or(false)
                                })
                            })
                    }),
                "review_delegation_report" => {
                    string("reviewer_role") == Some("project_manager")
                        && number("delegation_id").is_some_and(|child_id| {
                            self.is_child_delegation(child_id, capability)
                                .unwrap_or(false)
                        })
                }
                _ => false,
            },
            _ => false,
        };
        if authorized {
            Ok(())
        } else {
            Err(format!(
                "role capability scope does not allow `{tool_name}`"
            ))
        }
    }

    fn authorize_main_tool_call(&self, tool_name: &str, arguments: &Value) -> Result<(), String> {
        let object = arguments.as_object();
        let number = |key: &str| {
            object
                .and_then(|values| values.get(key))
                .and_then(Value::as_i64)
        };
        let string = |key: &str| {
            object
                .and_then(|values| values.get(key))
                .and_then(Value::as_str)
        };
        let project_scope_is_valid = || -> Result<bool, String> {
            match string("scope_type") {
                Some("global") => Ok(object
                    .and_then(|values| values.get("scope_id"))
                    .is_none_or(Value::is_null)),
                Some("project") => {
                    let Some(project_id) =
                        string("scope_id").and_then(|value| value.parse::<i64>().ok())
                    else {
                        return Ok(false);
                    };
                    let connection = self.connection()?;
                    connection
                        .raw()
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                            params![project_id],
                            |row| row.get(0),
                        )
                        .map_err(sqlite_error)
                }
                _ => Ok(false),
            }
        };
        let active_manager = |manager_id: i64| -> Result<bool, String> {
            let connection = self.connection()?;
            connection
                .raw()
                .query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM delegations delegation
                       JOIN project_manager_assignments assignment
                         ON assignment.manager_delegation_id = delegation.id
                       WHERE delegation.id = ?1 AND delegation.role = 'sub_overseer'
                         AND delegation.status = 'dispatched' AND assignment.status = 'active'
                     )",
                    params![manager_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)
        };
        let authorized = match tool_name {
            "read_memories" => {
                project_scope_is_valid()?
                    && !object
                        .and_then(|values| values.get("include_cold"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
            }
            "search_memory_candidates" => {
                project_scope_is_valid()?
                    && string("receiver_scope_type") == Some("scope")
                    && string("receiver_scope_id") == Some("main-overseer")
                    && number("delegation_id").is_none()
            }
            "review_role_work_plan" => {
                let manager_id = if let Some(plan_id) = number("plan_id") {
                    let connection = self.connection()?;
                    connection
                        .raw()
                        .query_row(
                            "SELECT manager_delegation_id FROM role_work_plans
                             WHERE id = ?1 AND plan_level = 'project_manager'",
                            params![plan_id],
                            |row| row.get::<_, i64>(0),
                        )
                        .optional()
                        .map_err(sqlite_error)?
                } else {
                    None
                };
                string("reviewer_role") == Some("main_overseer")
                    && manager_id.map(active_manager).transpose()?.unwrap_or(false)
            }
            "review_delegation_report" => {
                string("reviewer_role") == Some("main_overseer")
                    && number("delegation_id")
                        .map(active_manager)
                        .transpose()?
                        .unwrap_or(false)
            }
            "record_delegation_update" => {
                let Some(manager_id) = number("delegation_id") else {
                    return Err(
                        "Main can resolve only a keyed Project Manager escalation".to_string()
                    );
                };
                let action_key = string("action_key");
                let unresolved = if string("update_type") == Some("action_resolved")
                    && active_manager(manager_id)?
                {
                    let connection = self.connection()?;
                    connection
                        .raw()
                        .query_row(
                            "SELECT EXISTS(
                               SELECT 1 FROM events action
                               WHERE action.aggregate_type = 'delegation' AND action.aggregate_id = ?1
                                 AND action.event_type = 'agent_action_required'
                                 AND json_extract(action.payload_json, '$.actionKey') = ?2
                                 AND NOT EXISTS (
                                   SELECT 1 FROM events newer
                                   WHERE newer.aggregate_type = action.aggregate_type
                                     AND newer.aggregate_id = action.aggregate_id
                                     AND json_extract(newer.payload_json, '$.actionKey') = ?2
                                     AND newer.event_type IN ('agent_action_required', 'agent_action_resolved')
                                     AND newer.id > action.id
                                 )
                             )",
                            params![manager_id.to_string(), action_key],
                            |row| row.get(0),
                        )
                        .map_err(sqlite_error)?
                } else {
                    false
                };
                unresolved
            }
            "request_owner_approval" => number("manager_delegation_id")
                .map(active_manager)
                .transpose()?
                .unwrap_or(false),
            "relay_owner_decision_to_project_manager" => {
                let Some(request_id) = number("request_id") else {
                    return Err("owner approval request ID is required".to_string());
                };
                let connection = self.connection()?;
                let manager_id = connection
                    .raw()
                    .query_row(
                        "SELECT request.manager_delegation_id
                         FROM owner_approval_requests request
                         JOIN owner_approval_decisions decision ON decision.request_id = request.id
                         WHERE request.id = ?1 AND decision.decision_origin = 'desktop_owner_ui'",
                        params![request_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()
                    .map_err(sqlite_error)?;
                manager_id.map(active_manager).transpose()?.unwrap_or(false)
            }
            _ => false,
        };
        if authorized {
            Ok(())
        } else {
            Err(format!(
                "global Main role does not allow `{tool_name}` for this scope or state"
            ))
        }
    }

    fn capability_assignment_is_active(
        &self,
        capability: &RoleCapabilityScope,
    ) -> Result<bool, String> {
        let connection = self.connection()?;
        let (role, project_id, status): (String, i64, String) = connection
            .raw()
            .query_row(
                "SELECT role, project_id, status FROM delegations WHERE id = ?1",
                params![capability
                    .delegation_id
                    .ok_or_else(|| "role capability has no delegation assignment".to_string())?],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| "role capability assignment no longer exists".to_string())?;
        Ok(role == capability.role_kind
            && Some(project_id) == capability.project_id
            && status == "dispatched")
    }

    fn capability_run_id(&self, capability: &RoleCapabilityScope) -> Result<Option<i64>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT run_id FROM delegations WHERE id = ?1 AND project_id = ?2",
                params![
                    capability
                        .delegation_id
                        .ok_or_else(|| "role capability has no delegation assignment".to_string())?,
                    capability
                        .project_id
                        .ok_or_else(|| "role capability has no project scope".to_string())?
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)
    }

    fn is_child_delegation(
        &self,
        child_id: i64,
        capability: &RoleCapabilityScope,
    ) -> Result<bool, String> {
        let connection = self.connection()?;
        let matches = connection
            .raw()
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM delegations child JOIN delegations parent
                     ON parent.id = child.parent_delegation_id
                   WHERE child.id = ?1 AND child.project_id = ?2
                     AND parent.id = ?3 AND child.role = 'worker'
                     AND child.status IN ('prepared', 'dispatched', 'completed')
                 )",
                params![
                    child_id,
                    capability
                        .project_id
                        .ok_or_else(|| "role capability has no project scope".to_string())?,
                    capability
                        .delegation_id
                        .ok_or_else(|| "role capability has no delegation assignment".to_string())?
                ],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error)?;
        Ok(matches)
    }

    pub fn role_capability_can_read_delegation(
        &self,
        capability: &RoleCapabilityScope,
        delegation_id: i64,
    ) -> Result<bool, String> {
        if Some(delegation_id) == capability.delegation_id {
            return Ok(true);
        }
        if capability.role_kind == "sub_overseer" {
            return self.is_child_delegation(delegation_id, capability);
        }
        Ok(false)
    }

    /// Queue each prepared assignment independently. The caller can return immediately after this
    /// method; it never waits for an app-server turn. Duplicate IDs are idempotent.
    pub fn enqueue_role_dispatches(
        &self,
        delegation_ids: &[i64],
    ) -> Result<Vec<RoleDispatchAdmission>, String> {
        if delegation_ids.is_empty() || delegation_ids.len() > MAX_BATCH_SIZE {
            return Err(format!(
                "provide between 1 and {MAX_BATCH_SIZE} prepared delegation IDs"
            ));
        }
        let mut admissions = Vec::with_capacity(delegation_ids.len());
        for delegation_id in delegation_ids {
            match self.enqueue_role_dispatch(*delegation_id) {
                Ok(admission) => admissions.push(admission),
                Err(message) => admissions.push(RoleDispatchAdmission {
                    delegation_id: *delegation_id,
                    accepted: false,
                    status: None,
                    message,
                }),
            }
        }
        Ok(admissions)
    }

    pub fn enqueue_role_dispatch(
        &self,
        delegation_id: i64,
    ) -> Result<RoleDispatchAdmission, String> {
        if delegation_id <= 0 {
            return Err("delegation ID must be positive".to_string());
        }
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let delegation = transaction
            .query_row(
                "SELECT status, packet_hash FROM delegations WHERE id = ?1",
                params![delegation_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("delegation {delegation_id} was not found"))?;
        let idempotency_key_hash = content_hash(&format!(
            "cathulu-role-dispatch-v1:{delegation_id}:{}",
            delegation.1
        ));
        if let Some((existing_id, status, existing_key_hash)) = transaction
            .query_row(
                "SELECT id, status, idempotency_key_hash FROM role_dispatch_outbox
                 WHERE delegation_id = ?1",
                params![delegation_id],
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
        {
            if existing_key_hash != idempotency_key_hash {
                return Err(format!(
                    "delegation {delegation_id} has a dispatch record for a different packet"
                ));
            }
            transaction.commit().map_err(sqlite_error)?;
            return Ok(RoleDispatchAdmission {
                delegation_id,
                accepted: matches!(
                    status.as_str(),
                    "queued"
                        | "preflighting"
                        | "thread_ready"
                        | "turn_starting"
                        | "dispatched"
                        | "running"
                        | "completed"
                ),
                status: Some(status.clone()),
                message: format!(
                    "dispatch already recorded as {status} (queue item {existing_id})"
                ),
            });
        }
        if delegation.0 != "prepared" {
            return Err(format!(
                "delegation {delegation_id} must be prepared before dispatch; it is {}",
                delegation.0
            ));
        }
        transaction
            .execute(
                "INSERT INTO role_dispatch_outbox
                 (delegation_id, idempotency_key_hash, status, created_at, updated_at)
                 VALUES (?1, ?2, 'queued', ?3, ?3)",
                params![delegation_id, idempotency_key_hash, now],
            )
            .map_err(sqlite_error)?;
        let outbox_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO role_dispatch_events (outbox_id, event_type, details_json, created_at)
                 VALUES (?1, 'queued', '{}', ?2)",
                params![outbox_id, now],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "delegation",
            delegation_id,
            "role_dispatch_queued",
            &json!({ "dispatchId": outbox_id, "status": "queued" }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(RoleDispatchAdmission {
            delegation_id,
            accepted: true,
            status: Some("queued".to_string()),
            message: format!("queued for asynchronous dispatch (queue item {outbox_id})"),
        })
    }

    pub fn role_dispatches(&self, limit: usize) -> Result<Vec<RoleDispatchRecord>, String> {
        let limit = limit.clamp(1, 200) as i64;
        let connection = self.connection()?;
        let mut statement = connection
            .raw()
            .prepare(
                "SELECT queue.id, delegation.id, project.id, project.name, task.title,
                        delegation.role, queue.status, queue.attempt_count, queue.last_error,
                        queue.created_at, queue.updated_at
                 FROM role_dispatch_outbox queue
                 JOIN delegations delegation ON delegation.id = queue.delegation_id
                 JOIN projects project ON project.id = delegation.project_id
                 JOIN tasks task ON task.id = delegation.task_id
                 ORDER BY queue.updated_at DESC, queue.id DESC LIMIT ?1",
            )
            .map_err(sqlite_error)?;
        let records = statement
            .query_map(params![limit], |row| {
                Ok(RoleDispatchRecord {
                    id: row.get(0)?,
                    delegation_id: row.get(1)?,
                    project_id: row.get(2)?,
                    project_name: row.get(3)?,
                    task_title: row.get(4)?,
                    role: row.get(5)?,
                    status: row.get(6)?,
                    attempt_count: row.get(7)?,
                    last_error: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        Ok(records)
    }

    pub fn has_queued_role_dispatch(&self) -> Result<bool, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM role_dispatch_outbox WHERE status = 'queued')",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)
    }

    /// Claim one queued item under SQLite's write lock. Uncertain in-flight items are never
    /// reclaimed automatically: a process restart must reconcile thread/turn state first.
    pub fn claim_next_role_dispatch(&self) -> Result<Option<RoleDispatchWorkItem>, String> {
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let stale_ids = {
            let mut statement = transaction
                .prepare(
                    "SELECT queue.id FROM role_dispatch_outbox queue
                     JOIN delegations delegation ON delegation.id = queue.delegation_id
                     WHERE queue.status = 'queued' AND delegation.status <> 'prepared'",
                )
                .map_err(sqlite_error)?;
            let ids = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(sqlite_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sqlite_error)?;
            ids
        };
        for stale_id in stale_ids {
            transaction
                .execute(
                    "UPDATE role_dispatch_outbox SET status = 'cancelled', updated_at = ?1,
                       last_error = 'delegation was cancelled before dispatch' WHERE id = ?2 AND status = 'queued'",
                    params![now, stale_id],
                )
                .map_err(sqlite_error)?;
            transaction
                .execute(
                    "INSERT INTO role_dispatch_events (outbox_id, event_type, details_json, created_at)
                     VALUES (?1, 'cancelled', '{\"reason\":\"delegation_cancelled\"}', ?2)",
                    params![stale_id, now],
                )
                .map_err(sqlite_error)?;
        }
        let candidate = transaction
            .query_row(
                "SELECT queue.id, delegation.id, project.id, project.name, project.path,
                        delegation.run_id, delegation.role, delegation.parent_delegation_id,
                        delegation.packet_json, delegation.packet_hash,
                        delegation.requested_model, delegation.requested_effort,
                        (SELECT main_plan.plan_json
                         FROM delegation_work_plan_refs main_ref
                         JOIN role_work_plans main_plan ON main_plan.id = main_ref.plan_id
                         WHERE main_ref.delegation_id = delegation.id
                           AND main_ref.task_key IS NULL AND main_plan.plan_level = 'main'
                         LIMIT 1)
                 FROM role_dispatch_outbox queue
                 JOIN delegations delegation ON delegation.id = queue.delegation_id
                 JOIN projects project ON project.id = delegation.project_id
                 WHERE queue.status = 'queued' AND delegation.status = 'prepared'
                 ORDER BY queue.created_at, queue.id LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some((
            dispatch_id,
            delegation_id,
            project_id,
            project_name,
            project_path,
            run_id,
            role,
            parent_delegation_id,
            packet_json,
            packet_hash,
            requested_model,
            requested_effort,
            main_workflow_brief_json,
        )) = candidate
        else {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(None);
        };
        transaction
            .execute(
                "UPDATE role_dispatch_outbox SET status = 'preflighting',
                   attempt_count = attempt_count + 1, updated_at = ?1
                 WHERE id = ?2 AND status = 'queued'",
                params![now, dispatch_id],
            )
            .map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO role_dispatch_events (outbox_id, event_type, details_json, created_at)
                 VALUES (?1, 'preflighting', '{}', ?2)",
                params![dispatch_id, now],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        let task_packet = serde_json::from_str(&packet_json)
            .map_err(|error| format!("stored delegation packet is invalid: {error}"))?;
        let main_workflow_brief = main_workflow_brief_json
            .map(|brief| serde_json::from_str(&brief))
            .transpose()
            .map_err(|error| format!("stored Main workflow brief is invalid: {error}"))?;
        Ok(Some(RoleDispatchWorkItem {
            id: dispatch_id,
            delegation_id,
            project_id,
            project_name,
            project_path,
            run_id,
            role,
            parent_delegation_id,
            task_packet,
            main_workflow_brief,
            packet_hash,
            requested_model,
            requested_effort,
        }))
    }

    pub fn set_role_dispatch_status(
        &self,
        dispatch_id: i64,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        const STATUSES: &[&str] = &[
            "queued",
            "preflighting",
            "thread_ready",
            "turn_starting",
            "dispatched",
            "running",
            "completed",
            "blocked",
            "failed",
            "needs_reconcile",
            "cancelled",
        ];
        if !STATUSES.contains(&status) {
            return Err("unsupported role dispatch status".to_string());
        }
        let error = error.map(str::trim).filter(|value| !value.is_empty());
        if error.is_some_and(|value| value.len() > MAX_ERROR_BYTES) {
            return Err(format!("dispatch error exceeds {MAX_ERROR_BYTES} bytes"));
        }
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let previous: Option<String> = transaction
            .query_row(
                "SELECT status FROM role_dispatch_outbox WHERE id = ?1",
                params![dispatch_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        let previous =
            previous.ok_or_else(|| format!("dispatch record {dispatch_id} was not found"))?;
        if previous == status {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(());
        }
        if !valid_transition(&previous, status) {
            return Err(format!(
                "role dispatch cannot transition from {previous} to {status}"
            ));
        }
        transaction
            .execute(
                "UPDATE role_dispatch_outbox SET status = ?1, last_error = ?2, updated_at = ?3,
                   acknowledged_at = CASE WHEN ?1 = 'dispatched' AND acknowledged_at IS NULL
                     THEN ?3 ELSE acknowledged_at END
                 WHERE id = ?4",
                params![status, error, now, dispatch_id],
            )
            .map_err(sqlite_error)?;
        let details =
            json!({ "from": previous, "to": status, "errorHash": error.map(content_hash) });
        let details_json = serde_json::to_string(&details)
            .map_err(|error| format!("cannot encode dispatch event details: {error}"))?;
        if details_json.len() > MAX_EVENT_DETAILS_BYTES {
            return Err("dispatch event details exceed the storage limit".to_string());
        }
        transaction
            .execute(
                "INSERT INTO role_dispatch_events (outbox_id, event_type, details_json, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![dispatch_id, status, details_json, now],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)
    }

    /// Converts all ambiguous in-flight delivery states to a visible reconciliation state.
    /// It intentionally does not retry them, because turn/start may have reached Codex.
    pub fn mark_uncertain_role_dispatches_for_reconciliation(&self) -> Result<usize, String> {
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let ids = {
            let mut statement = transaction
                .prepare(
                    "SELECT id FROM role_dispatch_outbox
                     WHERE status IN ('preflighting', 'thread_ready', 'turn_starting', 'dispatched', 'running')",
                )
                .map_err(sqlite_error)?;
            let ids = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(sqlite_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sqlite_error)?;
            ids
        };
        for dispatch_id in &ids {
            transaction
                .execute(
                    "UPDATE role_dispatch_outbox SET status = 'needs_reconcile', updated_at = ?1,
                       last_error = COALESCE(last_error, 'dispatcher restarted during delivery; reconcile before retry')
                     WHERE id = ?2",
                    params![now, dispatch_id],
                )
                .map_err(sqlite_error)?;
            transaction
                .execute(
                    "INSERT INTO role_dispatch_events (outbox_id, event_type, details_json, created_at)
                     VALUES (?1, 'needs_reconcile', '{\"reason\":\"dispatcher_restart\"}', ?2)",
                    params![dispatch_id, now],
                )
                .map_err(sqlite_error)?;
        }
        transaction.commit().map_err(sqlite_error)?;
        Ok(ids.len())
    }

    pub fn try_acquire_role_dispatcher_lease(
        &self,
        owner_id: &str,
        now: i64,
        lease_seconds: i64,
    ) -> Result<bool, String> {
        let owner_id = owner_id.trim();
        if owner_id.is_empty() || owner_id.len() > 256 || lease_seconds <= 0 {
            return Err("dispatcher lease owner and duration are invalid".to_string());
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO role_dispatcher_lease
                 (singleton, owner_id, lease_expires_at, heartbeat_at, status, updated_at)
                 VALUES (1, NULL, 0, NULL, 'idle', ?1)",
                params![now],
            )
            .map_err(sqlite_error)?;
        let updated = transaction
            .execute(
                "UPDATE role_dispatcher_lease SET owner_id = ?1, lease_expires_at = ?2,
                   heartbeat_at = ?3, status = 'running', last_error = NULL, updated_at = ?3
                 WHERE singleton = 1 AND (owner_id = ?1 OR owner_id IS NULL OR lease_expires_at <= ?3)",
                params![owner_id, now.saturating_add(lease_seconds), now],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(updated == 1)
    }

    pub fn renew_role_dispatcher_lease(
        &self,
        owner_id: &str,
        now: i64,
        lease_seconds: i64,
    ) -> Result<bool, String> {
        if lease_seconds <= 0 {
            return Err("dispatcher lease duration must be positive".to_string());
        }
        let connection = self.connection()?;
        let updated = connection
            .raw()
            .execute(
                "UPDATE role_dispatcher_lease SET lease_expires_at = ?1, heartbeat_at = ?2,
                   updated_at = ?2, status = 'running'
                 WHERE singleton = 1 AND owner_id = ?3 AND lease_expires_at > ?2",
                params![now.saturating_add(lease_seconds), now, owner_id],
            )
            .map_err(sqlite_error)?;
        Ok(updated == 1)
    }

    pub fn release_role_dispatcher_lease(&self, owner_id: &str, now: i64) -> Result<bool, String> {
        let connection = self.connection()?;
        let updated = connection
            .raw()
            .execute(
                "UPDATE role_dispatcher_lease SET owner_id = NULL, lease_expires_at = ?1,
                   status = 'idle', updated_at = ?1
                 WHERE singleton = 1 AND owner_id = ?2",
                params![now, owner_id],
            )
            .map_err(sqlite_error)?;
        Ok(updated == 1)
    }

    pub fn bind_role_thread(
        &self,
        binding_key: &str,
        role_key: Option<&str>,
        delegation_id: Option<i64>,
        project_id: i64,
        thread_id: &str,
        status: &str,
        last_turn_id: Option<&str>,
    ) -> Result<RoleThreadBinding, String> {
        if binding_key.trim().is_empty()
            || binding_key.len() > 256
            || thread_id.trim().is_empty()
            || thread_id.len() > 256
            || (role_key.is_none() && delegation_id.is_none())
            || !matches!(status, "active" | "inactive" | "unknown")
        {
            return Err("role thread binding identity is invalid".to_string());
        }
        let now = unix_time();
        let connection = self.connection()?;
        connection
            .raw()
            .execute(
                "INSERT INTO role_thread_bindings
                 (binding_key, role_key, delegation_id, project_id, thread_id, status,
                  last_turn_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                 ON CONFLICT(binding_key) DO UPDATE SET thread_id = excluded.thread_id,
                   status = excluded.status, last_turn_id = excluded.last_turn_id, updated_at = excluded.updated_at",
                params![binding_key, role_key, delegation_id, project_id, thread_id, status, last_turn_id, now],
            )
            .map_err(sqlite_error)?;
        self.get_role_thread_binding(binding_key)?
            .ok_or_else(|| "saved role thread binding disappeared".to_string())
    }

    pub fn bind_main_role_thread(
        &self,
        thread_id: &str,
        status: &str,
        last_turn_id: Option<&str>,
    ) -> Result<RoleThreadBinding, String> {
        if thread_id.trim().is_empty()
            || thread_id.len() > 256
            || !matches!(status, "active" | "inactive" | "unknown")
        {
            return Err("Main role thread binding identity is invalid".to_string());
        }
        let now = unix_time();
        let connection = self.connection()?;
        connection
            .raw()
            .execute(
                "INSERT INTO role_thread_bindings
                 (binding_key, role_key, delegation_id, project_id, thread_id, status,
                  last_turn_id, created_at, updated_at)
                 VALUES ('main-overseer', 'main-overseer', NULL, NULL, ?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(binding_key) DO UPDATE SET thread_id = excluded.thread_id,
                   status = excluded.status, last_turn_id = excluded.last_turn_id,
                   updated_at = excluded.updated_at",
                params![thread_id, status, last_turn_id, now],
            )
            .map_err(sqlite_error)?;
        self.get_role_thread_binding("main-overseer")?
            .ok_or_else(|| "saved global Main role thread binding disappeared".to_string())
    }

    pub fn cathulu_workspace_path(&self) -> Result<PathBuf, String> {
        let connection = self.connection()?;
        let path = connection
            .raw()
            .query_row(
                "SELECT path FROM projects WHERE name = 'cathulu' COLLATE NOCASE
                 ORDER BY id LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| {
                "the registered Cathulu project workspace is required for the Main thread"
                    .to_string()
            })?;
        let path = PathBuf::from(path);
        if !path.is_dir() {
            return Err(
                "the registered Cathulu project workspace is not available on disk".to_string(),
            );
        }
        Ok(path)
    }

    pub fn relay_owner_approval_decision(&self, request_id: i64) -> Result<Value, String> {
        if request_id <= 0 {
            return Err("owner approval request ID must be positive".to_string());
        }
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        if let Some(event_id) = transaction
            .query_row(
                "SELECT event_id FROM owner_approval_relays WHERE request_id = ?1",
                params![request_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sqlite_error)?
        {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(
                json!({"requestId": request_id, "eventId": event_id, "alreadyRelayed": true}),
            );
        }
        let (manager_id, project_id, action_key, decision_id, decision, response_summary, origin) =
            transaction
                .query_row(
                    "SELECT request.manager_delegation_id, request.project_id, request.action_key,
                            decision.id, decision.decision, decision.response_summary, decision.decision_origin
                     FROM owner_approval_requests request
                     JOIN owner_approval_decisions decision ON decision.request_id = request.id
                     WHERE request.id = ?1",
                    params![request_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                        ))
                    },
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| "owner approval request has no recorded decision".to_string())?;
        if origin != "desktop_owner_ui" {
            return Err(
                "only an explicit Owner decision from the local dashboard can be relayed"
                    .to_string(),
            );
        }
        let manager_active = transaction
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM project_manager_assignments
                   WHERE manager_delegation_id = ?1 AND project_id = ?2 AND status = 'active'
                 )",
                params![manager_id, project_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(sqlite_error)?;
        if !manager_active {
            return Err("the Project Manager assignment is no longer active; the Owner decision remains recorded but cannot be relayed to that assignment".to_string());
        }
        let event_id = append_event(
            &transaction,
            "delegation",
            manager_id,
            "owner_decision_relayed",
            &json!({
                "requestId": request_id,
                "decisionId": decision_id,
                "decision": decision,
                "responseSummary": response_summary,
                "actionKey": action_key,
                "decisionOrigin": origin,
            }),
            now,
        )?;
        transaction
            .execute(
                "INSERT INTO owner_approval_relays(request_id, manager_delegation_id, event_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![request_id, manager_id, event_id, now],
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(json!({
            "requestId": request_id,
            "managerDelegationId": manager_id,
            "projectId": project_id,
            "eventId": event_id,
            "decisionId": decision_id,
            "relayed": true
        }))
    }

    pub fn get_role_thread_binding(
        &self,
        binding_key: &str,
    ) -> Result<Option<RoleThreadBinding>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT binding_key, role_key, delegation_id, project_id, thread_id, status,
                        last_turn_id, created_at, updated_at
                 FROM role_thread_bindings WHERE binding_key = ?1",
                params![binding_key],
                |row| {
                    Ok(RoleThreadBinding {
                        binding_key: row.get(0)?,
                        role_key: row.get(1)?,
                        delegation_id: row.get(2)?,
                        project_id: row.get(3)?,
                        thread_id: row.get(4)?,
                        status: row.get(5)?,
                        last_turn_id: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)
    }
}

fn active_project_manager(
    transaction: &Transaction<'_>,
    project_id: i64,
) -> Result<Option<i64>, String> {
    transaction
        .query_row(
            "SELECT manager_delegation_id FROM project_manager_assignments
             WHERE project_id = ?1 AND status = 'active'",
            params![project_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(sqlite_error)
}

#[allow(clippy::too_many_arguments)]
fn insert_role_inbox_message(
    transaction: &Transaction<'_>,
    event_id: i64,
    source_delegation_id: i64,
    target_binding_key: &str,
    target_delegation_id: Option<i64>,
    project_id: i64,
    event_type: &str,
    payload_json: &str,
    status: &str,
    error: Option<&str>,
    created_at: i64,
) -> Result<bool, String> {
    let inserted = transaction
        .execute(
            "INSERT INTO role_turn_inbox
             (source_event_id, source_delegation_id, target_binding_key, target_delegation_id, project_id,
              event_type, payload_json, status, attempt_count, last_error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?10)
             ON CONFLICT(source_event_id) DO NOTHING",
            params![
                event_id,
                source_delegation_id,
                target_binding_key,
                target_delegation_id,
                project_id,
                event_type,
                payload_json,
                status,
                error,
                created_at,
            ],
        )
        .map_err(sqlite_error)?;
    if inserted == 0 {
        return Ok(false);
    }
    let inbox_id = transaction.last_insert_rowid();
    transaction
        .execute(
            "INSERT INTO role_turn_inbox_events (inbox_id, event_type, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                inbox_id,
                status,
                json!({ "sourceEventId": event_id, "sourceDelegationId": source_delegation_id })
                    .to_string(),
                created_at,
            ],
        )
        .map_err(sqlite_error)?;
    Ok(true)
}

/// Starts the one-per-database background dispatcher when the caller has explicitly enabled
/// it. The thread owns app-server I/O, so MCP tool calls only persist a queue item and return.
pub struct RoleDispatcherService {
    stop: Arc<AtomicBool>,
    _thread: JoinHandle<()>,
}

impl RoleDispatcherService {
    pub fn start(store: Store) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::Builder::new()
            .name("cathulu-role-dispatcher".into())
            .spawn(move || dispatcher_loop(store, thread_stop))
            .map_err(|error| format!("cannot start local role dispatcher: {error}"))?;
        Ok(Self {
            stop,
            _thread: handle,
        })
    }
}

impl Drop for RoleDispatcherService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[derive(Clone, Debug)]
struct ActiveTurn {
    dispatch_id: Option<i64>,
    inbox_id: Option<i64>,
    binding_key: String,
    turn_id: Option<String>,
}

fn dispatcher_loop(store: Store, stop: Arc<AtomicBool>) {
    let owner_id = format!("{}:{}", std::process::id(), unix_time());
    const LEASE_SECONDS: i64 = 45;
    let mut lease_held = false;
    let mut next_heartbeat = Instant::now();
    let mut app_server: Option<AppServerProcess> = None;
    let mut active_turns: HashMap<String, ActiveTurn> = HashMap::new();

    while !stop.load(Ordering::Acquire) {
        let now = unix_time();
        if !lease_held {
            match store.try_acquire_role_dispatcher_lease(&owner_id, now, LEASE_SECONDS) {
                Ok(true) => {
                    lease_held = true;
                    next_heartbeat = Instant::now() + Duration::from_secs(10);
                    if store
                        .mark_uncertain_role_dispatches_for_reconciliation()
                        .is_err()
                        || store
                            .mark_uncertain_role_inbox_for_reconciliation()
                            .is_err()
                    {
                        lease_held = false;
                        thread::sleep(Duration::from_millis(800));
                        continue;
                    }
                }
                Ok(false) | Err(_) => {
                    thread::sleep(Duration::from_millis(800));
                    continue;
                }
            }
        }

        if Instant::now() >= next_heartbeat {
            match store.renew_role_dispatcher_lease(&owner_id, unix_time(), LEASE_SECONDS) {
                Ok(true) => next_heartbeat = Instant::now() + Duration::from_secs(10),
                _ => {
                    mark_active_turns_for_reconciliation(&store, &active_turns);
                    mark_active_inbox_for_reconciliation(&store, &active_turns);
                    active_turns.clear();
                    app_server = None;
                    lease_held = false;
                    continue;
                }
            }
        }

        if let Some(server) = app_server.as_mut() {
            match server.next_event(Duration::from_millis(30)) {
                Ok(Some(event)) => {
                    handle_app_server_event(&store, server, &mut active_turns, event)
                }
                Ok(None) => {}
                Err(_) => {
                    mark_active_turns_for_reconciliation(&store, &active_turns);
                    mark_active_inbox_for_reconciliation(&store, &active_turns);
                    active_turns.clear();
                    app_server = None;
                }
            }
        }

        let item = match store.claim_next_role_dispatch() {
            Ok(item) => item,
            Err(_) => {
                thread::sleep(Duration::from_millis(300));
                continue;
            }
        };
        if let Some(item) = item {
            let result = dispatch_one(&store, &mut app_server, &mut active_turns, &owner_id, item);
            if let Err((dispatch_id, status, message)) = result {
                let _ = store.set_role_dispatch_status(dispatch_id, status, Some(&message));
            }
            continue;
        }

        let _ = store.route_pending_role_events(100);
        let active_binding_keys = active_turns
            .values()
            .map(|turn| turn.binding_key.clone())
            .collect::<Vec<_>>();
        let message = store
            .claim_next_role_inbox_message(&active_binding_keys)
            .unwrap_or_default();
        if let Some(message) = message {
            if let Err((inbox_id, status, error)) =
                dispatch_role_inbox_message(&store, &mut app_server, &mut active_turns, message)
            {
                let _ = store.set_role_inbox_status(inbox_id, status, Some(&error));
            }
            continue;
        }
        thread::sleep(Duration::from_millis(120));
    }

    mark_active_turns_for_reconciliation(&store, &active_turns);
    mark_active_inbox_for_reconciliation(&store, &active_turns);
    drop(app_server);
    if lease_held {
        let _ = store.release_role_dispatcher_lease(&owner_id, unix_time());
    }
}

fn dispatch_one(
    store: &Store,
    app_server: &mut Option<AppServerProcess>,
    active_turns: &mut HashMap<String, ActiveTurn>,
    owner_id: &str,
    item: RoleDispatchWorkItem,
) -> Result<(), (i64, &'static str, String)> {
    if !matches!(item.role.as_str(), "sub_overseer" | "worker") {
        return Err((item.id, "failed", "unsupported delegation role".to_string()));
    }
    let binding_key = match item.role.as_str() {
        "sub_overseer" => format!("project-manager:{}", item.project_id),
        "worker" => format!("worker:{}", item.delegation_id),
        _ => unreachable!(),
    };
    if active_turns
        .values()
        .any(|turn| turn.binding_key == binding_key)
    {
        return Err((
            item.id,
            "blocked",
            "this logical role already has an active Codex turn".to_string(),
        ));
    }
    if app_server.is_none() {
        *app_server =
            Some(AppServerProcess::start().map_err(|message| (item.id, "failed", message))?);
    }
    let server = app_server.as_mut().expect("app-server was started");
    let binding = store
        .get_role_thread_binding(&binding_key)
        .map_err(|message| (item.id, "failed", message))?;
    let existing_thread_id = match binding {
        Some(binding) if binding.status == "active" => Some(binding.thread_id),
        Some(_) => {
            return Err((
                item.id,
                "needs_reconcile",
                "the stored logical role thread is not active; manual reconciliation is required"
                    .to_string(),
            ));
        }
        None => None,
    };

    let capability = store
        .issue_role_thread_capability(
            &binding_key,
            &item.role,
            item.project_id,
            item.delegation_id,
        )
        .map_err(|message| (item.id, "blocked", message))?;
    let config = role_mcp_server_config(store, &capability)
        .map_err(|message| (item.id, "blocked", message))?;
    let new_thread = existing_thread_id.is_none();
    let thread_id = match existing_thread_id {
        Some(thread_id) => match server.resume_thread(&thread_id, config) {
            Ok(()) => thread_id,
            Err(message) => {
                let _ = store.revoke_role_thread_capability(&binding_key);
                return Err((item.id, "needs_reconcile", message));
            }
        },
        None => match server.start_thread(
            Path::new(&item.project_path),
            item.requested_model.as_deref(),
            "on-request",
            "workspace-write",
            config,
        ) {
            Ok(thread_id) => thread_id,
            Err(message) => {
                let _ = store.revoke_role_thread_capability(&binding_key);
                return Err((item.id, "failed", message));
            }
        },
    };

    let required_tools = required_tools_for_role(&item.role);
    server
        .require_cathulu_tools(&thread_id, required_tools)
        .map_err(|message| {
            if new_thread {
                let _ = store.bind_role_thread(
                    &binding_key,
                    (item.role == "sub_overseer").then_some(binding_key.as_str()),
                    (item.role == "worker").then_some(item.delegation_id),
                    item.project_id,
                    &thread_id,
                    "unknown",
                    None,
                );
            }
            let _ = store.revoke_role_thread_capability(&binding_key);
            (item.id, "blocked", message)
        })?;

    if new_thread {
        store
            .bind_role_thread(
                &binding_key,
                (item.role == "sub_overseer").then_some(binding_key.as_str()),
                (item.role == "worker").then_some(item.delegation_id),
                item.project_id,
                &thread_id,
                "active",
                None,
            )
            .map_err(|message| (item.id, "failed", message))?;
    }
    store
        .set_role_dispatch_status(item.id, "thread_ready", None)
        .map_err(|message| (item.id, "failed", message))?;
    if !store
        .renew_role_dispatcher_lease(owner_id, unix_time(), 45)
        .map_err(|message| (item.id, "failed", message))?
    {
        return Err((
            item.id,
            "needs_reconcile",
            "dispatcher lease was lost before turn start".to_string(),
        ));
    }
    store
        .mark_delegation_dispatched(item.delegation_id)
        .map_err(|message| (item.id, "blocked", message))?;
    store
        .set_role_dispatch_status(item.id, "turn_starting", None)
        .map_err(|message| (item.id, "needs_reconcile", message))?;
    let prompt = role_turn_prompt(&item);
    let acknowledgement = server
        .start_turn(&thread_id, &prompt)
        .map_err(|message| (item.id, "needs_reconcile", message))?;
    let turn_id = acknowledgement
        .pointer("/turn/id")
        .or_else(|| acknowledgement.get("turnId"))
        .and_then(Value::as_str)
        .map(str::to_string);
    store
        .bind_role_thread(
            &binding_key,
            (item.role == "sub_overseer").then_some(binding_key.as_str()),
            (item.role == "worker").then_some(item.delegation_id),
            item.project_id,
            &thread_id,
            "active",
            turn_id.as_deref(),
        )
        .map_err(|message| (item.id, "needs_reconcile", message))?;
    store
        .set_role_dispatch_status(item.id, "dispatched", None)
        .map_err(|message| (item.id, "needs_reconcile", message))?;
    active_turns.insert(
        thread_id,
        ActiveTurn {
            dispatch_id: Some(item.id),
            inbox_id: None,
            binding_key,
            turn_id,
        },
    );
    Ok(())
}

fn required_tools_for_role(role: &str) -> &'static [&'static str] {
    match role {
        "main_overseer" => &[
            "read_memories",
            "search_memory_candidates",
            "record_delegation_update",
            "review_role_work_plan",
            "review_delegation_report",
            "request_owner_approval",
            "relay_owner_decision_to_project_manager",
        ],
        "sub_overseer" => &[
            "read_memories",
            "search_memory_candidates",
            "record_delegation_update",
            "record_role_work_plan",
            "prepare_delegation",
            "record_delegation_activation",
            "dispatch_prepared_delegations",
            "submit_delegation_report",
            "review_delegation_report",
            "complete_project_manager_assignment",
        ],
        "worker" => &[
            "read_memories",
            "search_memory_candidates",
            "record_delegation_update",
            "submit_delegation_report",
            "record_delegation_quality_check",
        ],
        _ => &[],
    }
}

fn dispatch_role_inbox_message(
    store: &Store,
    app_server: &mut Option<AppServerProcess>,
    active_turns: &mut HashMap<String, ActiveTurn>,
    message: RoleTurnInboxRecord,
) -> Result<(), (i64, &'static str, String)> {
    if message.target_binding_key == "main-overseer" {
        return dispatch_main_inbox_message(store, app_server, active_turns, message);
    }
    if message.target_binding_key != format!("project-manager:{}", message.project_id) {
        return Err((
            message.id,
            "unroutable",
            "role inbox message targets an unsupported logical role".to_string(),
        ));
    }
    let Some(manager_delegation_id) = message.target_delegation_id else {
        return Err((
            message.id,
            "unroutable",
            "role inbox message has no active Project Manager assignment".to_string(),
        ));
    };
    if active_turns
        .values()
        .any(|turn| turn.binding_key == message.target_binding_key)
    {
        return Err((
            message.id,
            "blocked",
            "the Project Manager already has an active Codex turn".to_string(),
        ));
    }
    if app_server.is_none() {
        *app_server =
            Some(AppServerProcess::start().map_err(|error| (message.id, "blocked", error))?);
    }
    let server = app_server.as_mut().expect("app-server was started");
    let binding = store
        .get_role_thread_binding(&message.target_binding_key)
        .map_err(|error| (message.id, "blocked", error))?
        .ok_or_else(|| {
            (
                message.id,
                "blocked",
                "the Project Manager has no persisted Codex thread binding".to_string(),
            )
        })?;
    if binding.status != "active" {
        return Err((
            message.id,
            "blocked",
            "the Project Manager Codex thread binding is not active".to_string(),
        ));
    }
    let capability = store
        .issue_role_thread_capability(
            &message.target_binding_key,
            "sub_overseer",
            message.project_id,
            manager_delegation_id,
        )
        .map_err(|error| (message.id, "blocked", error))?;
    let config = role_mcp_server_config(store, &capability)
        .map_err(|error| (message.id, "blocked", error))?;
    if let Err(error) = server.resume_thread(&binding.thread_id, config) {
        let _ = store.revoke_role_thread_capability(&message.target_binding_key);
        return Err((message.id, "needs_reconcile", error));
    }
    if let Err(error) =
        server.require_cathulu_tools(&binding.thread_id, required_tools_for_role("sub_overseer"))
    {
        let _ = store.revoke_role_thread_capability(&message.target_binding_key);
        return Err((message.id, "blocked", error));
    }
    let prompt = role_event_prompt(&message);
    let acknowledgement = server
        .start_turn(&binding.thread_id, &prompt)
        .map_err(|error| (message.id, "needs_reconcile", error))?;
    let turn_id = acknowledgement
        .pointer("/turn/id")
        .or_else(|| acknowledgement.get("turnId"))
        .and_then(Value::as_str)
        .map(str::to_string);
    store
        .bind_role_thread(
            &message.target_binding_key,
            Some(message.target_binding_key.as_str()),
            None,
            message.project_id,
            &binding.thread_id,
            "active",
            turn_id.as_deref(),
        )
        .map_err(|error| (message.id, "needs_reconcile", error))?;
    store
        .set_role_inbox_status(message.id, "dispatched", None)
        .map_err(|error| (message.id, "needs_reconcile", error))?;
    active_turns.insert(
        binding.thread_id,
        ActiveTurn {
            dispatch_id: None,
            inbox_id: Some(message.id),
            binding_key: message.target_binding_key,
            turn_id,
        },
    );
    Ok(())
}

fn dispatch_main_inbox_message(
    store: &Store,
    app_server: &mut Option<AppServerProcess>,
    active_turns: &mut HashMap<String, ActiveTurn>,
    message: RoleTurnInboxRecord,
) -> Result<(), (i64, &'static str, String)> {
    let binding_key = "main-overseer";
    if active_turns
        .values()
        .any(|turn| turn.binding_key == binding_key)
    {
        return Err((
            message.id,
            "blocked",
            "Main already has an active event turn".to_string(),
        ));
    }
    if app_server.is_none() {
        *app_server =
            Some(AppServerProcess::start().map_err(|error| (message.id, "blocked", error))?);
    }
    let server = app_server.as_mut().expect("app-server was started");
    let binding = store
        .get_role_thread_binding(binding_key)
        .map_err(|error| (message.id, "blocked", error))?;
    let existing_thread_id = match binding {
        Some(binding) if binding.status == "active" && binding.project_id.is_none() => {
            Some(binding.thread_id)
        }
        Some(_) => {
            return Err((
                message.id,
                "needs_reconcile",
                "the stored global Main thread binding is not active or is not global".to_string(),
            ))
        }
        None => None,
    };
    let capability = store
        .issue_main_overseer_capability()
        .map_err(|error| (message.id, "blocked", error))?;
    let config = role_mcp_server_config(store, &capability)
        .map_err(|error| (message.id, "blocked", error))?;
    let new_thread = existing_thread_id.is_none();
    let thread_id = match existing_thread_id {
        Some(thread_id) => match server.resume_thread(&thread_id, config) {
            Ok(()) => thread_id,
            Err(error) => {
                let _ = store.revoke_role_thread_capability(binding_key);
                return Err((message.id, "needs_reconcile", error));
            }
        },
        None => {
            let workspace = store
                .cathulu_workspace_path()
                .map_err(|error| (message.id, "blocked", error))?;
            match server.start_thread(&workspace, None, "on-request", "read-only", config) {
                Ok(thread_id) => thread_id,
                Err(error) => {
                    let _ = store.revoke_role_thread_capability(binding_key);
                    return Err((message.id, "failed", error));
                }
            }
        }
    };
    if let Err(error) =
        server.require_cathulu_tools(&thread_id, required_tools_for_role("main_overseer"))
    {
        if new_thread {
            let _ = store.bind_main_role_thread(&thread_id, "unknown", None);
        }
        let _ = store.revoke_role_thread_capability(binding_key);
        return Err((message.id, "blocked", error));
    }
    if new_thread {
        store
            .bind_main_role_thread(&thread_id, "active", None)
            .map_err(|error| (message.id, "failed", error))?;
    }
    let prompt = main_event_prompt(&message);
    let acknowledgement = server
        .start_turn(&thread_id, &prompt)
        .map_err(|error| (message.id, "needs_reconcile", error))?;
    let turn_id = acknowledgement
        .pointer("/turn/id")
        .or_else(|| acknowledgement.get("turnId"))
        .and_then(Value::as_str)
        .map(str::to_string);
    store
        .bind_main_role_thread(&thread_id, "active", turn_id.as_deref())
        .map_err(|error| (message.id, "needs_reconcile", error))?;
    store
        .set_role_inbox_status(message.id, "dispatched", None)
        .map_err(|error| (message.id, "needs_reconcile", error))?;
    active_turns.insert(
        thread_id,
        ActiveTurn {
            dispatch_id: None,
            inbox_id: Some(message.id),
            binding_key: binding_key.to_string(),
            turn_id,
        },
    );
    Ok(())
}

fn main_event_prompt(message: &RoleTurnInboxRecord) -> String {
    let payload = serde_json::from_str::<Value>(&message.payload_json)
        .unwrap_or_else(|_| json!({ "unavailable": true }));
    let action = match message.event_type.as_str() {
        "role_work_plan_recorded" => format!(
            "Review this Project Manager plan against the recorded Main brief and project boundary. Read cathulu://work-plans/{} and project memory, then record accepted, needs_follow_up, or rejected. The plan review event will wake only that Project Manager.",
            payload.get("planId").and_then(Value::as_i64).unwrap_or_default()
        ),
        "delegation_report_submitted" => format!(
            "Review this Project Manager report against its Main brief, accepted plan, Worker reviews, and deterministic evidence. Read cathulu://delegations/{} and project memory, then record a qualified review. Do not infer success from a report.",
            message.source_delegation_id
        ),
        "agent_action_required" => "Resolve this Project Manager escalation if it is a bounded, reversible decision within the Owner's constraints. If it crosses the Owner authority boundary, create the exact owner approval request immediately. Do not wait for other projects.".to_string(),
        "agent_action_resolved" => "A Project Manager reports an escalated action resolved. Inspect the current delegation and its evidence, then decide whether a new project-level review is needed. Do not wait for other projects.".to_string(),
        "owner_approval_decided" => format!(
            "An Owner decision arrived. Read cathulu://owner-approvals/{} and relay it to the affected active Project Manager with relay_owner_decision_to_project_manager. This records the decision for that project; it does not execute the action.",
            payload.get("requestId").and_then(Value::as_i64).unwrap_or_default()
        ),
        _ => "Process this project event using Cathulu's current authoritative records and your authority boundary.".to_string(),
    };
    format!(
        "You are the one persistent global Main Overseer for Cathulu. This is a short event-driven turn; handle only the event below, read the relevant project memory and authoritative records, then end the turn. Do not poll, wait for agents, inspect unrelated repositories, or keep a generation alive while projects work. Other project events remain queued independently. Escalate only actions that require Owner authority, and relay an Owner decision only to its affected Project Manager. Deterministic QA and stored evidence remain authoritative.\n\n{action}\n\nInbox message: {}\nProject ID: {}\nSource delegation: {}\nEvent ID: {}\nEvent type: {}\nPayload: {}",
        message.id,
        message.project_id,
        message.source_delegation_id,
        message.source_event_id,
        message.event_type,
        payload,
    )
}

fn role_event_prompt(message: &RoleTurnInboxRecord) -> String {
    let payload = serde_json::from_str::<Value>(&message.payload_json)
        .unwrap_or_else(|_| json!({ "unavailable": true }));
    let event_type = message.event_type.as_str();
    let action = match event_type {
        "role_work_plan_reviewed" => format!(
            "Main reviewed your Project Manager plan at cathulu://work-plans/{}. If accepted, prepare and dispatch only the approved Worker tasks; if follow-up or rejected, revise the plan and wait for Main's next review. End this turn after the bounded next step.",
            payload.get("planId").and_then(Value::as_i64).unwrap_or_default()
        ),
        "delegation_report_submitted" => "Review this Worker report against its task packet, scope, artifacts, checks, and acceptance criteria. Record accept, follow-up, or escalation with evidence. If accepted, continue only the remaining bounded Project Manager work.".to_string(),
        "delegation_report_reviewed" => {
            if payload.get("reviewerRole").and_then(Value::as_str) == Some("main_overseer") {
                "Main reviewed your Project Manager result. Read your current report and review decision. If accepted, complete the assignment; if follow-up or rejected, continue only the requested project work and report again with evidence.".to_string()
            } else {
                "A parent review was recorded for this Worker. Read the current report and review decision, then continue only the remaining bounded Project Manager work or report the project result when ready.".to_string()
            }
        }
        "owner_decision_relayed" => "The Owner decision was relayed through Main. Read the current approval record and action state. Proceed only within the exact decision, record the keyed action outcome with evidence, and do not infer adjacent approval.".to_string(),
        "agent_action_required" => "A Worker has raised an action or blocker. Resolve it within your Project Manager authority if safe; otherwise escalate a concise, exact decision to Main through Cathulu. Do not forward it to the Owner unless the authority boundary requires it.".to_string(),
        "agent_action_resolved" => {
            if payload.get("source").and_then(Value::as_str) == Some("main_overseer") {
                "Main recorded its bounded decision for your keyed action. Apply only that decision, then continue the affected task within its original scope.".to_string()
            } else {
                "A Worker reports that a previously raised action is resolved. Verify the stored action state and resume only the affected task if its acceptance criteria remain unmet.".to_string()
            }
        }
        _ => "Review this project event and determine the smallest safe next action.".to_string(),
    };
    format!(
        "You are the persistent Project Manager for project {}. A new actionable Cathulu event was routed to you; process this event now, then end the turn. Do not wait, poll, or inspect unrelated projects. Use the authoritative delegation and event records rather than trusting this summary.\n\n{action}\n\nSource event ID: {}\nSource event type: {}\nSource Worker delegation: {}\nProject ID: {}\nPayload: {}\n\nRead the source delegation at cathulu://delegations/{} and the project memory needed for the decision. Preserve provenance. Do not copy transcripts into memory. Escalate to Main only when the decision exceeds your authority. Deterministic QA and parent review remain required before any success claim.",
        message.project_id,
        message.source_event_id,
        event_type,
        message.source_delegation_id,
        message.project_id,
        payload,
        message.source_delegation_id,
    )
}

fn role_mcp_server_config(store: &Store, capability: &RoleCapability) -> Result<Value, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot locate this Cathulu MCP executable: {error}"))?;
    let data_dir = store
        .database_path()
        .parent()
        .ok_or_else(|| "Cathulu database has no data directory".to_string())?;
    Ok(json!({
        "mcp_servers": {
            "cathulu": {
                "command": executable,
                "args": [],
                "env": {
                    "CATHULU_DATA_DIR": data_dir,
                    "CATHULU_ROLE_CAPABILITY": capability.token,
                    "CATHULU_ROLE_DISPATCHER_ENABLED": "0",
                    "CATHULU_ROLE_DISPATCHER_CLIENT_ENABLED": "1"
                },
                "enabled": true,
                "startup_timeout_ms": 20000
            }
        }
    }))
}

fn role_turn_prompt(item: &RoleDispatchWorkItem) -> String {
    let packet = serde_json::to_string(&item.task_packet).unwrap_or_else(|_| "{}".to_string());
    let main_workflow_brief = item
        .main_workflow_brief
        .as_ref()
        .and_then(|brief| serde_json::to_string(brief).ok())
        .unwrap_or_else(|| "unavailable; do not widen scope beyond the task packet".to_string());
    let memory_scope = format!("scope_type=project, scope_id={}", item.project_id);
    let role_instructions = if item.role == "sub_overseer" {
        "You are this project's persistent Project Manager role. Read the project's Cathulu memory before planning. Create and record a bounded Project Manager plan, then report that the plan is ready and end this turn. Do not claim Main approval or self-review as Main. Prepare and dispatch at most two scoped Workers only after the latest plan review is explicitly accepted. Once Workers are dispatched, end this turn; never wait or poll for their completion. Review only results returned through later routed turns. Escalate only decisions outside your authority."
    } else {
        "You are a bounded implementation Worker assigned by the project's Project Manager. Read Cathulu project memory and only the referenced context relevant to this task. Work only in the registered project and allowed scope. Record meaningful progress and deterministic checks through Cathulu, submit an evidence-based report, and let the Project Manager review it. Do not claim verified success without the required QA evidence."
    };
    format!(
        "{role_instructions}\n\nCathulu is authoritative for role assignment, memory references, approvals, and reports. Do not copy transcripts into memory. Do not access another repository unless the task packet explicitly names an approved dependency. No credential, account, permission, external disclosure, destructive operation, publication, or out-of-scope action is pre-approved. If Owner input is required, record the exact request through Cathulu and stop only this project task.\n\nBefore acting, call read_memories for {memory_scope}; search HOT/WARM candidates selectively when needed. The activation record associated with this delegation is the delivery receipt; do not assume absent memories exist.\n\nDelegation ID: {}\nProject: {}\nProject path: {}\nRun ID: {}\nRole: {}\nParent delegation: {}\nMain workflow brief (authoritative): {}\nTask packet (authoritative): {}",
        item.delegation_id,
        item.project_name,
        item.project_path,
        item.run_id,
        item.role,
        item.parent_delegation_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none".to_string()),
        main_workflow_brief,
        packet
    )
}

fn handle_app_server_event(
    store: &Store,
    server: &mut AppServerProcess,
    active_turns: &mut HashMap<String, ActiveTurn>,
    event: AppServerMessage,
) {
    match event {
        AppServerMessage::Notification { method, params } => {
            if method == "turn/started" {
                if let Some(thread_id) = params.get("threadId").and_then(Value::as_str) {
                    if let Some(active) = active_turns.get(thread_id) {
                        if event_turn_id(&params).is_none_or(|id| Some(id) == active.turn_id) {
                            if let Some(dispatch_id) = active.dispatch_id {
                                let _ =
                                    store.set_role_dispatch_status(dispatch_id, "running", None);
                            }
                            if let Some(inbox_id) = active.inbox_id {
                                let _ = store.set_role_inbox_status(inbox_id, "running", None);
                            }
                        }
                    }
                }
            } else if method == "turn/completed" {
                if let Some(thread_id) = params.get("threadId").and_then(Value::as_str) {
                    if let Some(active) = active_turns.get(thread_id).cloned() {
                        if event_turn_id(&params).is_none_or(|id| Some(id) == active.turn_id) {
                            active_turns.remove(thread_id);
                            let outcome = params
                                .pointer("/turn/status")
                                .or_else(|| params.get("status"))
                                .and_then(Value::as_str);
                            let turn_failed =
                                matches!(outcome, Some("failed" | "interrupted" | "cancelled"));
                            if let Some(dispatch_id) = active.dispatch_id {
                                let status = store
                                    .role_dispatches(200)
                                    .ok()
                                    .and_then(|items| {
                                        items.into_iter().find(|item| item.id == dispatch_id)
                                    })
                                    .map(|item| item.status)
                                    .unwrap_or_default();
                                if status != "blocked" {
                                    let final_status =
                                        if turn_failed { "failed" } else { "completed" };
                                    let _ = store.set_role_dispatch_status(
                                        dispatch_id,
                                        final_status,
                                        None,
                                    );
                                }
                            }
                            if let Some(inbox_id) = active.inbox_id {
                                if turn_failed {
                                    let _ = store.set_role_inbox_status(
                                        inbox_id,
                                        "blocked",
                                        Some("the routed role turn ended without completing"),
                                    );
                                } else {
                                    let _ =
                                        store.set_role_inbox_status(inbox_id, "completed", None);
                                }
                            }
                        }
                    }
                }
            }
        }
        AppServerMessage::Request { id, method, params } => {
            // Read only thread identity; approval payloads can contain commands and file paths.
            let thread_id = params.get("threadId").and_then(Value::as_str);
            let active = thread_id.and_then(|thread_id| active_turns.get(thread_id));
            if let Some(active) = active {
                if let Some(dispatch_id) = active.dispatch_id {
                    let _ = store.set_role_dispatch_status(
                        dispatch_id,
                        "blocked",
                        Some("Owner approval or interactive input is required; Cathulu did not approve it."),
                    );
                }
                if let Some(inbox_id) = active.inbox_id {
                    let _ = store.set_role_inbox_status(
                        inbox_id,
                        "blocked",
                        Some("Owner approval or interactive input is required; Cathulu did not approve it."),
                    );
                }
            }
            if matches!(
                method.as_str(),
                "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
            ) {
                let _ = server.respond(id, json!({ "decision": "decline" }));
            } else {
                let _ = server.reject_request(
                    id,
                    -32000,
                    "Cathulu cannot route this interactive request yet; no approval was granted.",
                );
            }
        }
        AppServerMessage::TransportError(_) | AppServerMessage::TransportClosed => {
            mark_active_turns_for_reconciliation(store, active_turns);
            mark_active_inbox_for_reconciliation(store, active_turns);
            active_turns.clear();
        }
    }
}

fn event_turn_id(params: &Value) -> Option<String> {
    params
        .pointer("/turn/id")
        .or_else(|| params.get("turnId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn mark_active_turns_for_reconciliation(store: &Store, active: &HashMap<String, ActiveTurn>) {
    for turn in active.values() {
        let Some(dispatch_id) = turn.dispatch_id else {
            continue;
        };
        let current = store
            .role_dispatches(200)
            .ok()
            .and_then(|items| items.into_iter().find(|item| item.id == dispatch_id))
            .map(|item| item.status);
        if current.is_some_and(|status| {
            !matches!(
                status.as_str(),
                "blocked" | "completed" | "failed" | "cancelled" | "needs_reconcile"
            )
        }) {
            let _ = store.set_role_dispatch_status(
                dispatch_id,
                "needs_reconcile",
                Some("dispatcher connection closed; verify the Codex thread before retrying"),
            );
        }
    }
}

fn mark_active_inbox_for_reconciliation(store: &Store, active: &HashMap<String, ActiveTurn>) {
    for turn in active.values() {
        if let Some(inbox_id) = turn.inbox_id {
            let _ = store.set_role_inbox_status(
                inbox_id,
                "needs_reconcile",
                Some("dispatcher connection closed; inspect the Codex thread before retrying"),
            );
        }
    }
}

fn valid_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("queued", "preflighting" | "cancelled")
            | (
                "preflighting",
                "thread_ready" | "blocked" | "failed" | "needs_reconcile" | "cancelled"
            )
            | (
                "thread_ready",
                "turn_starting" | "blocked" | "failed" | "needs_reconcile" | "cancelled"
            )
            | (
                "turn_starting",
                "dispatched" | "failed" | "blocked" | "needs_reconcile"
            )
            | (
                "dispatched",
                "running" | "completed" | "blocked" | "failed" | "needs_reconcile"
            )
            | (
                "running",
                "completed" | "blocked" | "failed" | "needs_reconcile"
            )
            | ("needs_reconcile", "blocked" | "failed" | "cancelled")
    )
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("role dispatcher SQLite operation failed: {error}")
}

fn unix_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
