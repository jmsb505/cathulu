use crate::{events::append_event, hash::content_hash, Store};
use rmcp::schemars;
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const DEFAULT_HOT_BUDGET_BYTES: i64 = 8 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FreshnessBasis {
    #[serde(alias = "repo_id")]
    pub repo_id: Option<i64>,
    #[serde(alias = "git_commit")]
    pub git_commit: Option<String>,
    #[serde(alias = "graph_snapshot")]
    pub graph_snapshot: Option<String>,
    #[serde(alias = "contract_hash")]
    pub contract_hash: Option<String>,
    #[serde(alias = "dependency_snapshot")]
    pub dependency_snapshot: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemorySource {
    #[serde(alias = "source_type")]
    pub source_type: String,
    #[serde(alias = "source_id")]
    pub source_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCreateInput {
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub memory_class: String,
    pub temperature: String,
    pub kind: String,
    pub body: String,
    pub sources: Vec<MemorySource>,
    pub artifact_ids: Vec<i64>,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
    pub supersedes_id: Option<i64>,
    pub freshness_basis: FreshnessBasis,
    pub importance: f64,
    pub confidence: f64,
    pub active: bool,
    pub protected: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecord {
    pub id: i64,
    pub scope_type: String,
    pub scope_id: Option<String>,
    pub memory_class: String,
    pub temperature: String,
    pub kind: String,
    pub body: String,
    pub content_hash: String,
    pub review_state: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
    pub supersedes_id: Option<i64>,
    pub superseded_by_id: Option<i64>,
    pub freshness_basis: FreshnessBasis,
    pub importance: f64,
    pub confidence: f64,
    pub reuse_count: i64,
    pub last_used_at: Option<i64>,
    pub active: bool,
    pub protected: bool,
    pub invalidated_at: Option<i64>,
    pub invalidation_reason: Option<String>,
    pub sources: Vec<MemorySource>,
    pub artifact_ids: Vec<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransferDecision {
    pub action: String,
    pub memory_id: i64,
    pub content_hash: String,
    pub memory_class: String,
    pub temperature: String,
    pub body: Option<String>,
    pub replaces_memory_id: Option<i64>,
    pub first_time_for_receiver: bool,
    pub reintroduced: bool,
    pub transfer_id: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextTransferRecord {
    pub id: i64,
    pub run_id: Option<i64>,
    pub sender_scope_key: String,
    pub receiver_scope_key: String,
    pub memory_id: i64,
    pub content_hash: String,
    pub memory_class: String,
    pub temperature: String,
    pub size_bytes: i64,
    pub token_count: Option<i64>,
    pub first_time_for_receiver: bool,
    pub reintroduced: bool,
    pub retrieval_method: String,
    pub used_downstream: Option<bool>,
    pub transferred_at: i64,
}

impl Store {
    pub fn create_memory(&self, input: &MemoryCreateInput) -> Result<MemoryRecord, String> {
        validate_memory_input(input)?;
        let body = input.body.trim();
        let hash = content_hash(body);
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let (scope_key, scope_id) =
            normalize_scope(&transaction, &input.scope_type, input.scope_id.as_deref())?;
        validate_memory_sources(&transaction, &input.sources)?;
        validate_artifact_ids(&transaction, &input.artifact_ids)?;
        if let Some(repo_id) = input.freshness_basis.repo_id {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
                    params![repo_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            if !exists {
                return Err(format!("freshness repository {repo_id} was not found"));
            }
        }
        transaction
            .execute(
                "INSERT OR IGNORE INTO memory_scope_budgets
                 (scope_key, scope_type, scope_id, hot_budget_bytes, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    scope_key,
                    input.scope_type,
                    scope_id,
                    DEFAULT_HOT_BUDGET_BYTES,
                    now
                ],
            )
            .map_err(sqlite_error)?;
        let existing_id = transaction
            .query_row(
                "SELECT id FROM memories
                 WHERE scope_key = ?1 AND kind = ?2 AND content_hash = ?3
                   AND superseded_by_id IS NULL AND invalidated_at IS NULL
                   AND (valid_until IS NULL OR valid_until > ?4)
                 ORDER BY id DESC LIMIT 1",
                params![scope_key, input.kind, hash, now],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        if let Some(existing_id) = existing_id {
            add_memory_sources(&transaction, existing_id, &input.sources, now)?;
            add_memory_artifacts(&transaction, existing_id, &input.artifact_ids, now)?;
            transaction
                .execute(
                    "UPDATE memories SET updated_at = ?1,
                       importance = MAX(importance, ?2), confidence = MAX(confidence, ?3),
                       protected = MAX(protected, ?4)
                     WHERE id = ?5",
                    params![
                        now,
                        input.importance,
                        input.confidence,
                        input.protected,
                        existing_id
                    ],
                )
                .map_err(sqlite_error)?;
            append_event(
                &transaction,
                "memory",
                existing_id,
                "memory_reinforced",
                &json!({ "sourceCount": input.sources.len(), "artifactCount": input.artifact_ids.len() }),
                now,
            )?;
            transaction.commit().map_err(sqlite_error)?;
            return self
                .get_memory(existing_id)?
                .ok_or_else(|| "reinforced memory disappeared from the database".to_string());
        }
        if let Some(previous_id) = input.supersedes_id {
            let predecessor = transaction
                .query_row(
                    "SELECT scope_key, kind, superseded_by_id FROM memories WHERE id = ?1",
                    params![previous_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| format!("memory {previous_id} to supersede was not found"))?;
            if predecessor.0 != scope_key || predecessor.1 != input.kind {
                return Err("a superseded memory must have the same scope and kind".to_string());
            }
            if predecessor.2.is_some() {
                return Err(format!("memory {previous_id} already has a successor"));
            }
        }
        let freshness_json = serde_json::to_string(&input.freshness_basis)
            .map_err(|error| format!("cannot serialize freshness basis: {error}"))?;
        transaction
            .execute(
                "INSERT INTO memories
                 (scope_key, scope_type, scope_id, memory_class, temperature, kind, body, content_hash,
                  created_at, updated_at, valid_from, valid_until, supersedes_id, freshness_basis_json,
                  freshness_repo_id, freshness_git_commit, importance, confidence, active, protected)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    scope_key,
                    input.scope_type,
                    scope_id,
                    input.memory_class,
                    input.temperature,
                    input.kind,
                    body,
                    hash,
                    now,
                    input.valid_from,
                    input.valid_until,
                    input.supersedes_id,
                    freshness_json,
                    input.freshness_basis.repo_id,
                    input.freshness_basis.git_commit,
                    input.importance,
                    input.confidence,
                    input.active,
                    input.protected
                ],
            )
            .map_err(sqlite_error)?;
        let memory_id = transaction.last_insert_rowid();
        add_memory_sources(&transaction, memory_id, &input.sources, now)?;
        add_memory_artifacts(&transaction, memory_id, &input.artifact_ids, now)?;
        if let Some(previous_id) = input.supersedes_id {
            let supersession_time = input.valid_from.unwrap_or(now);
            transaction
                .execute(
                    "UPDATE memories SET superseded_by_id = ?1, valid_until = COALESCE(valid_until, ?2), updated_at = ?2
                     WHERE id = ?3",
                    params![memory_id, supersession_time, previous_id],
                )
                .map_err(sqlite_error)?;
            append_event(
                &transaction,
                "memory",
                previous_id,
                "memory_superseded",
                &json!({ "supersededById": memory_id }),
                now,
            )?;
        }
        enforce_hot_budget(&transaction, &scope_key, Some(memory_id), now)?;
        append_event(
            &transaction,
            "memory",
            memory_id,
            "memory_recorded",
            &json!({
                "scopeKey": scope_key,
                "memoryClass": input.memory_class,
                "temperature": input.temperature,
                "kind": input.kind,
                "contentHash": hash,
                "sourceCount": input.sources.len(),
                "artifactCount": input.artifact_ids.len()
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_memory(memory_id)?
            .ok_or_else(|| "created memory disappeared from the database".to_string())
    }

    pub fn get_memory(&self, id: i64) -> Result<Option<MemoryRecord>, String> {
        let connection = self.connection()?;
        let memory = connection
            .raw()
            .query_row(
                "SELECT id, scope_type, scope_id, memory_class, temperature, kind, body, content_hash,
                        review_state, created_at, updated_at, valid_from, valid_until, supersedes_id,
                        superseded_by_id, freshness_basis_json, importance, confidence, reuse_count,
                        last_used_at, active, protected, invalidated_at, invalidation_reason
                 FROM memories WHERE id = ?1",
                params![id],
                memory_from_row,
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some(mut memory) = memory else {
            return Ok(None);
        };
        memory.sources = connection
            .raw()
            .prepare(
                "SELECT source_type, source_id FROM memory_sources WHERE memory_id = ?1
                 ORDER BY source_type, source_id",
            )
            .map_err(sqlite_error)?
            .query_map(params![id], |row| {
                Ok(MemorySource {
                    source_type: row.get(0)?,
                    source_id: row.get(1)?,
                })
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        memory.artifact_ids = connection
            .raw()
            .prepare("SELECT artifact_id FROM memory_artifact_refs WHERE memory_id = ?1 ORDER BY artifact_id")
            .map_err(sqlite_error)?
            .query_map(params![id], |row| row.get(0))
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        Ok(Some(memory))
    }

    pub fn list_memories(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
        include_history: bool,
        include_cold: bool,
    ) -> Result<Vec<MemoryRecord>, String> {
        let connection = self.connection()?;
        let scope_key = scope_key_from_parts(scope_type, scope_id)?;
        let mut sql = String::from("SELECT id FROM memories WHERE scope_key = ?1");
        if !include_history {
            sql.push_str(
                " AND review_state = 'accepted' AND superseded_by_id IS NULL AND invalidated_at IS NULL
                  AND (valid_from IS NULL OR valid_from <= ?2) AND (valid_until IS NULL OR valid_until > ?2)",
            );
        }
        if !include_cold {
            sql.push_str(" AND temperature <> 'cold'");
        }
        sql.push_str(
            " ORDER BY CASE temperature WHEN 'hot' THEN 0 WHEN 'warm' THEN 1 ELSE 2 END,
                      importance DESC, COALESCE(last_used_at, 0) DESC, created_at DESC, id DESC",
        );
        let mut statement = connection.raw().prepare(&sql).map_err(sqlite_error)?;
        let ids = if include_history {
            statement
                .query_map(params![scope_key], |row| row.get::<_, i64>(0))
                .map_err(sqlite_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sqlite_error)?
        } else {
            statement
                .query_map(params![scope_key, now()], |row| row.get::<_, i64>(0))
                .map_err(sqlite_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sqlite_error)?
        };
        ids.into_iter()
            .map(|id| {
                self.get_memory(id)?
                    .ok_or_else(|| format!("memory {id} disappeared during listing"))
            })
            .collect()
    }

    pub fn review_memory_candidate(&self, id: i64, decision: &str) -> Result<MemoryRecord, String> {
        if !matches!(decision, "accepted" | "rejected") {
            return Err("memory review decision must be accepted or rejected".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let (memory_class, current_state, source_count): (String, String, i64) = transaction
            .query_row(
                "SELECT m.memory_class, m.review_state, COUNT(s.source_id)
                 FROM memories m LEFT JOIN memory_sources s ON s.memory_id = m.id
                 WHERE m.id = ?1 GROUP BY m.id",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("memory {id} was not found"))?;
        if current_state != "candidate" {
            return Err(format!("memory {id} is no longer a review candidate"));
        }
        if decision == "accepted" && memory_class == "canonical" && source_count == 0 {
            return Err("canonical memory requires at least one provenance source".to_string());
        }
        transaction
            .execute(
                "UPDATE memories SET review_state = ?1, updated_at = ?2 WHERE id = ?3",
                params![decision, now, id],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "memory",
            id,
            "memory_reviewed",
            &json!({ "decision": decision }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_memory(id)?
            .ok_or_else(|| format!("memory {id} disappeared during review"))
    }

    pub fn set_memory_temperature(
        &self,
        id: i64,
        temperature: &str,
    ) -> Result<MemoryRecord, String> {
        if !matches!(temperature, "hot" | "warm" | "cold") {
            return Err("memory temperature must be hot, warm, or cold".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let (scope_key, current): (String, String) = transaction
            .query_row(
                "SELECT scope_key, temperature FROM memories WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("memory {id} was not found"))?;
        if current != temperature {
            transaction
                .execute(
                    "UPDATE memories SET temperature = ?1, updated_at = ?2 WHERE id = ?3",
                    params![temperature, now, id],
                )
                .map_err(sqlite_error)?;
            enforce_hot_budget(&transaction, &scope_key, Some(id), now)?;
            append_event(
                &transaction,
                "memory",
                id,
                "memory_temperature_changed",
                &json!({ "from": current, "to": temperature }),
                now,
            )?;
        }
        transaction.commit().map_err(sqlite_error)?;
        self.get_memory(id)?
            .ok_or_else(|| format!("memory {id} disappeared after temperature change"))
    }

    pub fn set_memory_active(&self, id: i64, active: bool) -> Result<MemoryRecord, String> {
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let scope_key = transaction
            .query_row(
                "SELECT scope_key FROM memories WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("memory {id} was not found"))?;
        transaction
            .execute(
                "UPDATE memories SET active = ?1, updated_at = ?2 WHERE id = ?3",
                params![active, now, id],
            )
            .map_err(sqlite_error)?;
        enforce_hot_budget(&transaction, &scope_key, Some(id), now)?;
        append_event(
            &transaction,
            "memory",
            id,
            "memory_activation_changed",
            &json!({ "active": active }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.get_memory(id)?
            .ok_or_else(|| format!("memory {id} disappeared after activation change"))
    }

    pub fn set_memory_hot_budget(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
        hot_budget_bytes: i64,
    ) -> Result<i64, String> {
        if !(0..=10 * 1024 * 1024).contains(&hot_budget_bytes) {
            return Err("HOT memory budget must be between 0 and 10 MiB".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let (scope_key, normalized_id) = normalize_scope(&transaction, scope_type, scope_id)?;
        transaction
            .execute(
                "INSERT INTO memory_scope_budgets (scope_key, scope_type, scope_id, hot_budget_bytes, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(scope_key) DO UPDATE SET hot_budget_bytes = excluded.hot_budget_bytes,
                   updated_at = excluded.updated_at",
                params![scope_key, scope_type, normalized_id, hot_budget_bytes, now],
            )
            .map_err(sqlite_error)?;
        enforce_hot_budget(&transaction, &scope_key, None, now)?;
        append_event(
            &transaction,
            "memory_scope",
            &scope_key,
            "memory_hot_budget_changed",
            &json!({ "hotBudgetBytes": hot_budget_bytes }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(hot_budget_bytes)
    }

    pub fn prepare_context_transfer(
        &self,
        memory_id: i64,
        sender_scope_type: &str,
        sender_scope_id: Option<&str>,
        receiver_scope_type: &str,
        receiver_scope_id: Option<&str>,
        run_id: Option<i64>,
        retrieval_method: &str,
        token_count: Option<i64>,
    ) -> Result<TransferDecision, String> {
        if !matches!(
            retrieval_method,
            "hot"
                | "sql"
                | "fts"
                | "graphify"
                | "manual"
                | "semantic"
                | "scope_resume"
                | "explicit"
        ) {
            return Err("unsupported context retrieval method".to_string());
        }
        if token_count.is_some_and(|count| count < 0) {
            return Err("context token count cannot be negative".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let (_, _) = normalize_scope(&transaction, sender_scope_type, sender_scope_id)?;
        let (receiver_key, _) =
            normalize_scope(&transaction, receiver_scope_type, receiver_scope_id)?;
        let memory = load_memory(&transaction, memory_id)?
            .ok_or_else(|| format!("memory {memory_id} was not found"))?;
        if memory.review_state != "accepted"
            || memory.superseded_by_id.is_some()
            || memory.invalidated_at.is_some()
            || memory.valid_from.is_some_and(|valid_from| valid_from > now)
            || memory
                .valid_until
                .is_some_and(|valid_until| valid_until <= now)
        {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(TransferDecision {
                action: "excluded_stale".to_string(),
                memory_id,
                content_hash: memory.content_hash,
                memory_class: memory.memory_class,
                temperature: memory.temperature,
                body: None,
                replaces_memory_id: memory.supersedes_id,
                first_time_for_receiver: false,
                reintroduced: false,
                transfer_id: None,
            });
        }
        if memory.temperature == "cold" {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(TransferDecision {
                action: "excluded_cold".to_string(),
                memory_id,
                content_hash: memory.content_hash,
                memory_class: memory.memory_class,
                temperature: memory.temperature,
                body: None,
                replaces_memory_id: None,
                first_time_for_receiver: false,
                reintroduced: false,
                transfer_id: None,
            });
        }
        let known = transaction
            .query_row(
                "SELECT 1 FROM memory_known_by WHERE memory_id = ?1 AND receiver_scope_key = ?2 AND content_hash = ?3",
                params![memory_id, receiver_key, memory.content_hash],
                |_| Ok(()),
            )
            .optional()
            .map_err(sqlite_error)?
            .is_some();
        if known {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(TransferDecision {
                action: "omitted_known".to_string(),
                memory_id,
                content_hash: memory.content_hash,
                memory_class: memory.memory_class,
                temperature: memory.temperature,
                body: None,
                replaces_memory_id: None,
                first_time_for_receiver: false,
                reintroduced: false,
                transfer_id: None,
            });
        }
        let predecessor_known = if let Some(previous_id) = memory.supersedes_id {
            transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM memory_known_by WHERE memory_id = ?1 AND receiver_scope_key = ?2)",
                    params![previous_id, receiver_key],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(sqlite_error)?
        } else {
            false
        };
        let any_known_for_record: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM memory_known_by WHERE memory_id = ?1 AND receiver_scope_key = ?2)",
                params![memory_id, receiver_key],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        let first_time_for_receiver = !any_known_for_record && !predecessor_known;
        let reintroduced = !first_time_for_receiver;
        let action = if reintroduced { "delta" } else { "full" };
        if let Some(run_id) = run_id {
            let exists: bool = transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM runs WHERE id = ?1)",
                    params![run_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            if !exists {
                return Err(format!("run {run_id} was not found"));
            }
        }
        let body_size = memory.body.len() as i64;
        transaction
            .execute(
                "INSERT INTO context_transfers
                 (run_id, sender_scope_key, receiver_scope_key, memory_id, content_hash, memory_class,
                  temperature, size_bytes, token_count, first_time_for_receiver, reintroduced,
                  retrieval_method, transferred_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    run_id,
                    scope_key_from_parts(sender_scope_type, sender_scope_id)?,
                    receiver_key,
                    memory_id,
                    memory.content_hash,
                    memory.memory_class,
                    memory.temperature,
                    body_size,
                    token_count,
                    first_time_for_receiver,
                    reintroduced,
                    retrieval_method,
                    now
                ],
            )
            .map_err(sqlite_error)?;
        let transfer_id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO memory_known_by
                 (memory_id, receiver_scope_key, content_hash, known_at, last_used_at, use_count)
                 VALUES (?1, ?2, ?3, ?4, ?4, 1)
                 ON CONFLICT(memory_id, receiver_scope_key, content_hash)
                 DO UPDATE SET last_used_at = excluded.last_used_at, use_count = memory_known_by.use_count + 1",
                params![memory_id, receiver_key, memory.content_hash, now],
            )
            .map_err(sqlite_error)?;
        transaction
            .execute(
                "UPDATE memories SET reuse_count = reuse_count + 1, last_used_at = ?1, updated_at = ?1 WHERE id = ?2",
                params![now, memory_id],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "context_transfer",
            transfer_id,
            "memory_transferred",
            &json!({
                "runId": run_id,
                "memoryId": memory_id,
                "contentHash": memory.content_hash,
                "senderScope": scope_key_from_parts(sender_scope_type, sender_scope_id)?,
                "receiverScope": receiver_key,
                "sizeBytes": body_size,
                "tokenCount": token_count,
                "action": action,
                "retrievalMethod": retrieval_method
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(TransferDecision {
            action: action.to_string(),
            memory_id,
            content_hash: memory.content_hash,
            memory_class: memory.memory_class,
            temperature: memory.temperature,
            body: Some(memory.body),
            replaces_memory_id: if reintroduced {
                memory.supersedes_id
            } else {
                None
            },
            first_time_for_receiver,
            reintroduced,
            transfer_id: Some(transfer_id),
        })
    }

    pub fn get_context_transfer(&self, id: i64) -> Result<Option<ContextTransferRecord>, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT id, run_id, sender_scope_key, receiver_scope_key, memory_id, content_hash,
                        memory_class, temperature, size_bytes, token_count, first_time_for_receiver,
                        reintroduced, retrieval_method, used_downstream, transferred_at
                 FROM context_transfers WHERE id = ?1",
                params![id],
                |row| {
                    Ok(ContextTransferRecord {
                        id: row.get(0)?,
                        run_id: row.get(1)?,
                        sender_scope_key: row.get(2)?,
                        receiver_scope_key: row.get(3)?,
                        memory_id: row.get(4)?,
                        content_hash: row.get(5)?,
                        memory_class: row.get(6)?,
                        temperature: row.get(7)?,
                        size_bytes: row.get(8)?,
                        token_count: row.get(9)?,
                        first_time_for_receiver: row.get(10)?,
                        reintroduced: row.get(11)?,
                        retrieval_method: row.get(12)?,
                        used_downstream: row.get(13)?,
                        transferred_at: row.get(14)?,
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)
    }

    pub(crate) fn invalidate_project_memories(
        transaction: &Transaction<'_>,
        project_id: i64,
        current_commit: Option<&str>,
        now: i64,
    ) -> Result<Vec<i64>, String> {
        let mut statement = transaction
            .prepare(
                "SELECT id FROM memories WHERE freshness_repo_id = ?1 AND invalidated_at IS NULL
                   AND superseded_by_id IS NULL AND freshness_git_commit IS NOT ?2 ORDER BY id",
            )
            .map_err(sqlite_error)?;
        let ids = statement
            .query_map(params![project_id, current_commit], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        drop(statement);
        for id in &ids {
            transaction
                .execute(
                    "UPDATE memories SET invalidated_at = ?1, invalidation_reason = 'git_commit_changed', updated_at = ?1
                     WHERE id = ?2",
                    params![now, id],
                )
                .map_err(sqlite_error)?;
        }
        Ok(ids)
    }
}

fn validate_memory_input(input: &MemoryCreateInput) -> Result<(), String> {
    if !matches!(
        input.memory_class.as_str(),
        "working" | "canonical" | "episodic" | "evidence" | "aggregate"
    ) {
        return Err("unsupported memory class".to_string());
    }
    if !matches!(input.temperature.as_str(), "hot" | "warm" | "cold") {
        return Err("unsupported memory temperature".to_string());
    }
    if !matches!(
        input.kind.as_str(),
        "fact"
            | "decision"
            | "constraint"
            | "contract"
            | "lesson"
            | "procedure"
            | "failure_pattern"
            | "summary"
    ) {
        return Err("unsupported memory kind".to_string());
    }
    if !matches!(
        input.scope_type.as_str(),
        "global" | "project" | "objective" | "task" | "scope"
    ) {
        return Err("unsupported memory scope type".to_string());
    }
    validate_text("memory body", &input.body, 32 * 1024, true)?;
    if !input.importance.is_finite() || !(0.0..=1.0).contains(&input.importance) {
        return Err("memory importance must be between zero and one".to_string());
    }
    if !input.confidence.is_finite() || !(0.0..=1.0).contains(&input.confidence) {
        return Err("memory evidence confidence must be between zero and one".to_string());
    }
    if input.valid_from.is_some()
        && input
            .valid_until
            .is_some_and(|until| until <= input.valid_from.unwrap())
    {
        return Err("memory valid_until must be later than valid_from".to_string());
    }
    if input.memory_class == "canonical" && input.sources.is_empty() {
        return Err("canonical memory requires at least one provenance source".to_string());
    }
    if input.sources.len() > 32 || input.artifact_ids.len() > 64 {
        return Err("memory provenance exceeds the per-record reference limit".to_string());
    }
    Ok(())
}

fn normalize_scope(
    transaction: &Transaction<'_>,
    scope_type: &str,
    scope_id: Option<&str>,
) -> Result<(String, Option<String>), String> {
    if scope_type == "global" {
        if scope_id.is_some() {
            return Err("global memory scope must not have a scope_id".to_string());
        }
        return Ok(("global".to_string(), None));
    }
    let raw_id = scope_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{scope_type} memory scope requires a scope_id"))?;
    if scope_type == "scope" {
        if raw_id.len() > 200 || raw_id.contains('\0') {
            return Err(
                "logical scope_id must be at most 200 bytes and contain no NUL".to_string(),
            );
        }
        let normalized = raw_id.trim().to_string();
        return Ok((format!("scope:{normalized}"), Some(normalized)));
    }
    let id = raw_id
        .parse::<i64>()
        .map_err(|_| format!("{scope_type} scope_id must be a positive Cathulu ID"))?;
    if id <= 0 {
        return Err(format!(
            "{scope_type} scope_id must be a positive Cathulu ID"
        ));
    }
    let table = match scope_type {
        "project" => "projects",
        "objective" => "objectives",
        "task" => "tasks",
        _ => return Err("unsupported memory scope type".to_string()),
    };
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)");
    let exists: bool = transaction
        .query_row(&sql, params![id], |row| row.get(0))
        .map_err(sqlite_error)?;
    if !exists {
        return Err(format!("{scope_type} scope {id} was not found"));
    }
    Ok((format!("{scope_type}:{id}"), Some(id.to_string())))
}

fn scope_key_from_parts(scope_type: &str, scope_id: Option<&str>) -> Result<String, String> {
    if scope_type == "global" {
        if scope_id.is_some() {
            return Err("global memory scope must not have a scope_id".to_string());
        }
        return Ok("global".to_string());
    }
    let raw_id = scope_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{scope_type} memory scope requires a scope_id"))?;
    if scope_type == "scope" {
        if raw_id.len() > 200 || raw_id.contains('\0') {
            return Err(
                "logical scope_id must be at most 200 bytes and contain no NUL".to_string(),
            );
        }
        return Ok(format!("scope:{}", raw_id.trim()));
    }
    let id = raw_id
        .parse::<i64>()
        .map_err(|_| format!("{scope_type} scope_id must be a positive Cathulu ID"))?;
    if id <= 0 || !matches!(scope_type, "project" | "objective" | "task") {
        return Err("invalid memory scope".to_string());
    }
    Ok(format!("{scope_type}:{id}"))
}

fn validate_memory_sources(
    transaction: &Transaction<'_>,
    sources: &[MemorySource],
) -> Result<(), String> {
    for source in sources {
        if source.source_id.trim().is_empty()
            || source.source_id.contains('\0')
            || source.source_id.len() > 2_000
        {
            return Err("memory source identifiers must be non-empty local references".to_string());
        }
        let table = match source.source_type.as_str() {
            "event" => Some("events"),
            "project" => Some("projects"),
            "objective" => Some("objectives"),
            "task" => Some("tasks"),
            "decision" => Some("decisions"),
            "workflow_version" => Some("workflow_versions"),
            "run" => Some("runs"),
            "quality_check" => Some("quality_checks"),
            "artifact" => Some("evidence_artifacts"),
            "source_file" | "git_commit" | "test_result" | "contract" | "graph_snapshot"
            | "command_result" | "human_instruction" => None,
            _ => {
                return Err(format!(
                    "unsupported memory source type {}",
                    source.source_type
                ))
            }
        };
        if let Some(table) = table {
            let source_id = source.source_id.parse::<i64>().map_err(|_| {
                format!(
                    "memory source {} requires a numeric Cathulu source ID",
                    source.source_type
                )
            })?;
            let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)");
            let exists: bool = transaction
                .query_row(&sql, params![source_id], |row| row.get(0))
                .map_err(sqlite_error)?;
            if !exists {
                return Err(format!(
                    "memory provenance source {}:{} was not found",
                    source.source_type, source_id
                ));
            }
        }
    }
    Ok(())
}

fn validate_artifact_ids(
    transaction: &Transaction<'_>,
    artifact_ids: &[i64],
) -> Result<(), String> {
    for artifact_id in artifact_ids {
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM evidence_artifacts WHERE id = ?1)",
                params![artifact_id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if !exists {
            return Err(format!("artifact {artifact_id} was not found"));
        }
    }
    Ok(())
}

fn add_memory_sources(
    transaction: &Transaction<'_>,
    memory_id: i64,
    sources: &[MemorySource],
    now: i64,
) -> Result<(), String> {
    for source in sources {
        transaction
            .execute(
                "INSERT OR IGNORE INTO memory_sources (memory_id, source_type, source_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![memory_id, source.source_type, source.source_id.trim(), now],
            )
            .map_err(sqlite_error)?;
    }
    Ok(())
}

fn add_memory_artifacts(
    transaction: &Transaction<'_>,
    memory_id: i64,
    artifact_ids: &[i64],
    now: i64,
) -> Result<(), String> {
    for artifact_id in artifact_ids {
        transaction
            .execute(
                "INSERT OR IGNORE INTO memory_artifact_refs (memory_id, artifact_id, created_at)
                 VALUES (?1, ?2, ?3)",
                params![memory_id, artifact_id, now],
            )
            .map_err(sqlite_error)?;
    }
    Ok(())
}

fn enforce_hot_budget(
    transaction: &Transaction<'_>,
    scope_key: &str,
    _preferred_memory_id: Option<i64>,
    now: i64,
) -> Result<(), String> {
    let budget = transaction
        .query_row(
            "SELECT hot_budget_bytes FROM memory_scope_budgets WHERE scope_key = ?1",
            params![scope_key],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(sqlite_error)?
        .unwrap_or(DEFAULT_HOT_BUDGET_BYTES);
    loop {
        let hot_bytes: i64 = transaction
            .query_row(
                "SELECT COALESCE(SUM(length(CAST(body AS BLOB))), 0) FROM memories
                 WHERE scope_key = ?1 AND temperature = 'hot'",
                params![scope_key],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if hot_bytes <= budget {
            return Ok(());
        }
        let candidate = transaction
            .query_row(
                "SELECT id FROM memories WHERE scope_key = ?1 AND temperature = 'hot' AND active = 0
                 ORDER BY importance ASC, COALESCE(last_used_at, 0) ASC, created_at ASC, id ASC LIMIT 1",
                params![scope_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some(memory_id) = candidate else {
            return Err(format!(
                "HOT memory budget for {scope_key} is {budget} bytes, but active HOT records use {hot_bytes} bytes"
            ));
        };
        transaction
            .execute(
                "UPDATE memories SET temperature = 'warm', updated_at = ?1 WHERE id = ?2",
                params![now, memory_id],
            )
            .map_err(sqlite_error)?;
        append_event(
            transaction,
            "memory",
            memory_id,
            "memory_demoted_for_hot_budget",
            &json!({ "scopeKey": scope_key, "hotBudgetBytes": budget }),
            now,
        )?;
    }
}

fn load_memory(transaction: &Transaction<'_>, id: i64) -> Result<Option<MemoryRecord>, String> {
    let memory = transaction
        .query_row(
            "SELECT id, scope_type, scope_id, memory_class, temperature, kind, body, content_hash,
                    review_state, created_at, updated_at, valid_from, valid_until, supersedes_id,
                    superseded_by_id, freshness_basis_json, importance, confidence, reuse_count,
                    last_used_at, active, protected, invalidated_at, invalidation_reason
             FROM memories WHERE id = ?1",
            params![id],
            memory_from_row,
        )
        .optional()
        .map_err(sqlite_error)?;
    let Some(mut memory) = memory else {
        return Ok(None);
    };
    memory.sources = transaction
        .prepare("SELECT source_type, source_id FROM memory_sources WHERE memory_id = ?1 ORDER BY source_type, source_id")
        .map_err(sqlite_error)?
        .query_map(params![id], |row| {
            Ok(MemorySource {
                source_type: row.get(0)?,
                source_id: row.get(1)?,
            })
        })
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    memory.artifact_ids = transaction
        .prepare("SELECT artifact_id FROM memory_artifact_refs WHERE memory_id = ?1 ORDER BY artifact_id")
        .map_err(sqlite_error)?
        .query_map(params![id], |row| row.get(0))
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    Ok(Some(memory))
}

fn memory_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRecord> {
    let freshness_json: String = row.get(15)?;
    let freshness_basis = serde_json::from_str(&freshness_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(15, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(MemoryRecord {
        id: row.get(0)?,
        scope_type: row.get(1)?,
        scope_id: row.get(2)?,
        memory_class: row.get(3)?,
        temperature: row.get(4)?,
        kind: row.get(5)?,
        body: row.get(6)?,
        content_hash: row.get(7)?,
        review_state: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        valid_from: row.get(11)?,
        valid_until: row.get(12)?,
        supersedes_id: row.get(13)?,
        superseded_by_id: row.get(14)?,
        freshness_basis,
        importance: row.get(16)?,
        confidence: row.get(17)?,
        reuse_count: row.get(18)?,
        last_used_at: row.get(19)?,
        active: row.get(20)?,
        protected: row.get(21)?,
        invalidated_at: row.get(22)?,
        invalidation_reason: row.get(23)?,
        sources: Vec::new(),
        artifact_ids: Vec::new(),
    })
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

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
