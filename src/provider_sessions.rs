//! Durable provider session, attempt, event-deduplication, and context receipt records.
//!
//! The existing role-thread rows remain the compatibility surface for old callers. New writes
//! mirror those bindings into provider-namespaced history so a replacement session does not
//! inherit another session's context-read state.

use crate::agent_provider::{
    AttemptKey, ExternalId, ExternalSession, ProviderEvent, ProviderEventKind, ProviderId,
    TurnOutcome,
};
use crate::delegation::{validate_packet_references, DelegationTaskPacket};
use crate::events::append_external_event;
use crate::execution_artifacts::{
    ContextDelivery, ContextEntry, ContextManifest, DeliveryState, SessionContextReceipt,
};
use crate::hash::content_hash;
use crate::memory::MemoryRecord;
use crate::Store;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSessionRecord {
    pub provider_id: ProviderId,
    pub session_id: ExternalId,
    pub binding_key: String,
    pub project_id: Option<i64>,
    pub role_key: Option<String>,
    pub delegation_id: Option<i64>,
    pub status: String,
    pub last_turn_id: Option<ExternalId>,
    pub permission_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAttemptRecord {
    pub attempt_key: AttemptKey,
    pub session: ExternalSession,
    pub binding_key: String,
    pub dispatch_id: Option<i64>,
    pub inbox_id: Option<i64>,
    pub delegation_id: Option<i64>,
    pub request_hash: String,
    pub status: String,
    pub turn_id: Option<ExternalId>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderExecutionBinding {
    pub session: ExternalSession,
    pub attempt_key: AttemptKey,
}

impl Store {
    /// Record a provider-specific session while preserving the existing role-thread pointer.
    /// The legacy `role_thread_bindings` row must already point at this session.
    pub fn record_provider_session(
        &self,
        session: &ExternalSession,
        binding_key: &str,
        status: &str,
        last_turn_id: Option<&ExternalId>,
        permission_hash: Option<&str>,
    ) -> Result<ProviderSessionRecord, String> {
        validate_session(session, binding_key, status, permission_hash)?;
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let legacy = transaction
            .query_row(
                "SELECT role_key, delegation_id, project_id, thread_id, status
                 FROM role_thread_bindings WHERE binding_key = ?1",
                params![binding_key],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| {
                "provider session requires an existing role-thread binding".to_string()
            })?;
        if session.provider_id == ProviderId::codex()
            && (legacy.3 != session.session_id.as_str() || legacy.4 != status)
        {
            return Err(
                "provider session does not match the compatibility role-thread pointer".into(),
            );
        }
        if let Some(existing) = read_provider_session(&transaction, session)? {
            if let (Some(previous), Some(next)) =
                (existing.permission_hash.as_deref(), permission_hash)
            {
                if previous != next {
                    return Err("provider session permission identity cannot change".into());
                }
            }
        }
        transaction
            .execute(
                "UPDATE provider_sessions SET status = 'inactive', updated_at = ?1
                 WHERE provider_id = ?2 AND binding_key = ?3 AND session_id <> ?4
                   AND status = 'active'",
                params![
                    now,
                    session.provider_id.as_str(),
                    binding_key,
                    session.session_id.as_str()
                ],
            )
            .map_err(sqlite_error)?;
        transaction
            .execute(
                "INSERT INTO provider_sessions
                 (provider_id, session_id, binding_key, project_id, role_key, delegation_id,
                  status, last_turn_id, permission_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
                 ON CONFLICT(provider_id, session_id) DO UPDATE SET
                   status = excluded.status,
                   last_turn_id = COALESCE(excluded.last_turn_id, provider_sessions.last_turn_id),
                   permission_hash = COALESCE(provider_sessions.permission_hash, excluded.permission_hash),
                   updated_at = excluded.updated_at",
                params![
                    session.provider_id.as_str(),
                    session.session_id.as_str(),
                    binding_key,
                    legacy.2,
                    legacy.0,
                    legacy.1,
                    status,
                    last_turn_id.map(ExternalId::as_str),
                    permission_hash,
                    now
                ],
            )
            .map_err(sqlite_error)?;
        let record = read_provider_session(&transaction, session)?
            .ok_or_else(|| "provider session disappeared after recording".to_string())?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(record)
    }

    /// Read a provider session by logical role, falling back to the legacy Codex-only pointer
    /// for databases/callers that have not yet produced a namespaced row.
    pub fn provider_session_for_binding(
        &self,
        provider_id: &ProviderId,
        binding_key: &str,
    ) -> Result<Option<ProviderSessionRecord>, String> {
        let connection = self.connection()?;
        let found = connection
            .raw()
            .query_row(
                "SELECT provider_id, session_id, binding_key, project_id, role_key,
                        delegation_id, status, last_turn_id, permission_hash, created_at, updated_at
                 FROM provider_sessions
                 WHERE provider_id = ?1 AND binding_key = ?2 AND status = 'active'
                 ORDER BY updated_at DESC LIMIT 1",
                params![provider_id.as_str(), binding_key],
                provider_session_from_row,
            )
            .optional()
            .map_err(sqlite_error)?;
        if found.is_some() {
            return Ok(found);
        }

        if provider_id != &ProviderId::codex() {
            return Ok(None);
        }
        let legacy = self.get_role_thread_binding(binding_key)?;
        let Some(legacy) = legacy.filter(|record| record.status == "active") else {
            return Ok(None);
        };
        let session_id = ExternalId::new(legacy.thread_id)?;
        Ok(Some(ProviderSessionRecord {
            provider_id: provider_id.clone(),
            session_id,
            binding_key: legacy.binding_key,
            project_id: legacy.project_id,
            role_key: legacy.role_key,
            delegation_id: legacy.delegation_id,
            status: legacy.status,
            last_turn_id: legacy.last_turn_id.map(ExternalId::new).transpose()?,
            permission_hash: None,
            created_at: legacy.created_at,
            updated_at: legacy.updated_at,
        }))
    }

    /// Persist the attempt before the provider submit call. Repeated calls with the same key and
    /// identical identity return the original record; a conflicting reuse is rejected.
    #[allow(clippy::too_many_arguments)]
    pub fn begin_provider_attempt(
        &self,
        attempt_key: &AttemptKey,
        session: &ExternalSession,
        dispatch_id: Option<i64>,
        inbox_id: Option<i64>,
        delegation_id: Option<i64>,
        request_hash: &str,
    ) -> Result<ProviderAttemptRecord, String> {
        validate_hash(request_hash)?;
        if (dispatch_id.is_some()) == (inbox_id.is_some())
            || dispatch_id.is_some_and(|id| id <= 0)
            || inbox_id.is_some_and(|id| id <= 0)
            || delegation_id.is_some_and(|id| id <= 0)
        {
            return Err(
                "provider attempt must identify exactly one dispatch or inbox record".into(),
            );
        }
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let binding_key = transaction
            .query_row(
                "SELECT binding_key FROM provider_sessions
                 WHERE provider_id = ?1 AND session_id = ?2 AND status = 'active'",
                params![session.provider_id.as_str(), session.session_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| "provider attempt requires an active persisted session".to_string())?;

        if let Some(existing) = read_provider_attempt(&transaction, attempt_key)? {
            if existing.session == *session
                && existing.binding_key == binding_key
                && existing.dispatch_id == dispatch_id
                && existing.inbox_id == inbox_id
                && existing.delegation_id == delegation_id
                && existing.request_hash == request_hash
            {
                transaction.commit().map_err(sqlite_error)?;
                return Ok(existing);
            }
            return Err(
                "provider attempt key was reused with conflicting identity or payload".into(),
            );
        }

        let in_flight = if let Some(id) = dispatch_id {
            transaction
                .query_row(
                    "SELECT attempt_key FROM provider_attempts WHERE dispatch_id = ?1
                     AND status IN ('created','submitted','ack_unknown','running','needs_reconcile')
                     LIMIT 1",
                    params![id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sqlite_error)?
                .is_some()
        } else {
            transaction
                .query_row(
                    "SELECT attempt_key FROM provider_attempts WHERE inbox_id = ?1
                     AND status IN ('created','submitted','ack_unknown','running','needs_reconcile')
                     LIMIT 1",
                    params![inbox_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sqlite_error)?
                .is_some()
        };
        if in_flight {
            return Err("an earlier provider attempt still requires reconciliation".into());
        }
        transaction
            .execute(
                "INSERT INTO provider_attempts
                 (attempt_key, provider_id, session_id, binding_key, dispatch_id, inbox_id,
                  delegation_id, request_hash, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'created', ?9, ?9)",
                params![
                    attempt_key.as_str(),
                    session.provider_id.as_str(),
                    session.session_id.as_str(),
                    binding_key,
                    dispatch_id,
                    inbox_id,
                    delegation_id,
                    request_hash,
                    now
                ],
            )
            .map_err(sqlite_error)?;
        let record = read_provider_attempt(&transaction, attempt_key)?
            .ok_or_else(|| "provider attempt disappeared after insertion".to_string())?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(record)
    }

    pub fn set_provider_attempt_state(
        &self,
        attempt_key: &AttemptKey,
        state: &str,
        turn_id: Option<&ExternalId>,
    ) -> Result<ProviderAttemptRecord, String> {
        if !matches!(
            state,
            "submitted"
                | "ack_unknown"
                | "running"
                | "completed"
                | "failed"
                | "blocked"
                | "needs_reconcile"
        ) {
            return Err("unsupported provider attempt state".into());
        }
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let current = transaction
            .query_row(
                "SELECT status, turn_id FROM provider_attempts WHERE attempt_key = ?1",
                params![attempt_key.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| "provider attempt was not found".to_string())?;
        if current.0 != state && !valid_attempt_transition(&current.0, state) {
            return Err(format!(
                "invalid provider attempt transition: {} -> {state}",
                current.0
            ));
        }
        if let (Some(previous), Some(next)) = (&current.1, turn_id) {
            if previous != next.as_str() {
                return Err(
                    "provider attempt is already bound to a different external turn".into(),
                );
            }
        }
        transaction
            .execute(
                "UPDATE provider_attempts SET status = ?1, turn_id = COALESCE(?2, turn_id), updated_at = ?3
                 WHERE attempt_key = ?4",
                params![state, turn_id.map(ExternalId::as_str), now, attempt_key.as_str()],
            )
            .map_err(sqlite_error)?;
        let record = transaction
            .query_row(
                "SELECT attempt_key, provider_id, session_id, binding_key, dispatch_id, inbox_id,
                        delegation_id, request_hash, status, turn_id, created_at, updated_at
                 FROM provider_attempts WHERE attempt_key = ?1",
                params![attempt_key.as_str()],
                provider_attempt_from_row,
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(record)
    }

    /// Read the current memory IDs explicitly referenced by the stored delegation packet. This
    /// read-only preview lets callers hash the exact prompt before creating a durable attempt.
    pub fn preview_delegation_context(
        &self,
        delegation_id: i64,
    ) -> Result<ContextManifest, String> {
        if delegation_id <= 0 {
            return Err("delegation ID must be positive".into());
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(sqlite_error)?;
        let manifest = delegation_context_manifest(&transaction, delegation_id, unix_time())?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(manifest)
    }

    /// Select and receipt context only after the request hash and attempt key are durable.
    /// The receiver identity is the external provider session, never the stable logical role.
    pub fn select_delegation_context(
        &self,
        delegation_id: i64,
        session: &ExternalSession,
        attempt_key: &AttemptKey,
    ) -> Result<ContextManifest, String> {
        let manifest = self.preview_delegation_context(delegation_id)?;
        self.record_selected_provider_context(session, attempt_key, &manifest)?;
        Ok(manifest)
    }

    /// Revalidate a preview against the current packet and memory hashes before recording it.
    /// A stale preview therefore fails closed before provider submission.
    pub fn record_selected_provider_context(
        &self,
        session: &ExternalSession,
        attempt_key: &AttemptKey,
        manifest: &ContextManifest,
    ) -> Result<(), String> {
        manifest.validate()?;
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let delegation_id: Option<i64> = transaction
            .query_row(
                "SELECT delegation_id FROM provider_attempts
                 WHERE attempt_key = ?1 AND provider_id = ?2 AND session_id = ?3
                   AND status = 'created'",
                params![
                    attempt_key.as_str(),
                    session.provider_id.as_str(),
                    session.session_id.as_str()
                ],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .flatten();
        let delegation_id = delegation_id
            .ok_or_else(|| "context selection requires a created delegation attempt".to_string())?;
        let current = delegation_context_manifest(&transaction, delegation_id, now)?;
        if &current != manifest {
            return Err("selected provider context changed after the request was hashed".into());
        }
        for entry in &manifest.entries {
            transaction
                .execute(
                    "INSERT INTO provider_context_receipts
                     (provider_id, session_id, attempt_key, memory_id, content_hash, delivery, state, observed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'full', 'selected', ?6)
                     ON CONFLICT(provider_id, session_id, attempt_key, memory_id) DO NOTHING",
                    params![
                        session.provider_id.as_str(),
                        session.session_id.as_str(),
                        attempt_key.as_str(),
                        entry.memory_id,
                        entry.content_hash,
                        now
                    ],
                )
                .map_err(sqlite_error)?;
            let stored: (String, String) = transaction
                .query_row(
                    "SELECT content_hash, delivery FROM provider_context_receipts
                     WHERE provider_id = ?1 AND session_id = ?2 AND attempt_key = ?3 AND memory_id = ?4",
                    params![session.provider_id.as_str(), session.session_id.as_str(), attempt_key.as_str(), entry.memory_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(sqlite_error)?;
            if stored.0 != entry.content_hash || stored.1 != "full" {
                return Err("selected provider context changed within one attempt".into());
            }
        }
        transaction.commit().map_err(sqlite_error)?;
        Ok(())
    }

    pub fn set_provider_context_state(
        &self,
        session: &ExternalSession,
        attempt_key: &AttemptKey,
        state: DeliveryState,
    ) -> Result<usize, String> {
        let state = delivery_state_name(state);
        let now = unix_time();
        let connection = self.connection()?;
        let changed = connection
            .raw()
            .execute(
                "UPDATE provider_context_receipts SET state = ?1, observed_at = ?2
                 WHERE provider_id = ?3 AND session_id = ?4 AND attempt_key = ?5
                   AND ((state = 'selected' AND ?1 = 'submitted')
                     OR (state = 'submitted' AND ?1 = 'agent_reported_read')
                     OR state = ?1)",
                params![
                    state,
                    now,
                    session.provider_id.as_str(),
                    session.session_id.as_str(),
                    attempt_key.as_str()
                ],
            )
            .map_err(sqlite_error)?;
        Ok(changed)
    }

    /// Mark only selected memories actually returned by the scoped MCP read as agent-reported.
    pub fn record_session_context_read(
        &self,
        capability_hash: &str,
        memories: &[MemoryRecord],
    ) -> Result<usize, String> {
        validate_hash(capability_hash)?;
        let Some(binding) = self.provider_execution_for_capability(capability_hash)? else {
            return Ok(0);
        };
        let now = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let mut changed = 0;
        for memory in memories {
            changed += transaction
                .execute(
                    "UPDATE provider_context_receipts SET state = 'agent_reported_read', observed_at = ?1
                     WHERE provider_id = ?2 AND session_id = ?3 AND attempt_key = ?4
                       AND memory_id = ?5 AND content_hash = ?6 AND state = 'submitted'",
                    params![
                        now,
                        binding.session.provider_id.as_str(),
                        binding.session.session_id.as_str(),
                        binding.attempt_key.as_str(),
                        memory.id,
                        memory.content_hash
                    ],
                )
                .map_err(sqlite_error)?;
        }
        transaction.commit().map_err(sqlite_error)?;
        Ok(changed)
    }

    pub fn provider_execution_for_capability(
        &self,
        capability_hash: &str,
    ) -> Result<Option<ProviderExecutionBinding>, String> {
        validate_hash(capability_hash)?;
        let connection = self.connection()?;
        let row = connection
            .raw()
            .query_row(
                "SELECT capability.provider_id, capability.provider_session_id, capability.attempt_key
                 FROM role_thread_capabilities capability
                 WHERE capability.capability_hash = ?1 AND capability.status = 'active'
                   AND capability.provider_id IS NOT NULL
                   AND capability.provider_session_id IS NOT NULL
                   AND capability.attempt_key IS NOT NULL
                   AND EXISTS (
                     SELECT 1 FROM provider_attempts attempt
                     WHERE attempt.attempt_key = capability.attempt_key
                       AND attempt.provider_id = capability.provider_id
                       AND attempt.session_id = capability.provider_session_id
                       AND attempt.binding_key = capability.binding_key
                       AND attempt.delegation_id = capability.delegation_id
                       AND attempt.status IN ('created','submitted','ack_unknown','running','needs_reconcile')
                   )
                 UNION ALL
                 SELECT capability.provider_id, capability.provider_session_id, capability.attempt_key
                 FROM main_overseer_capability capability
                 WHERE capability.capability_hash = ?1 AND capability.status = 'active'
                   AND capability.provider_id IS NOT NULL
                   AND capability.provider_session_id IS NOT NULL
                   AND capability.attempt_key IS NOT NULL
                   AND EXISTS (
                     SELECT 1 FROM provider_attempts attempt
                     WHERE attempt.attempt_key = capability.attempt_key
                       AND attempt.provider_id = capability.provider_id
                       AND attempt.session_id = capability.provider_session_id
                       AND attempt.binding_key = 'main-overseer'
                       AND attempt.inbox_id IS NOT NULL AND attempt.delegation_id IS NULL
                       AND attempt.status IN ('created','submitted','ack_unknown','running','needs_reconcile')
                   )
                 LIMIT 1",
                params![capability_hash],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        row.map(|(provider, session, attempt)| {
            Ok(ProviderExecutionBinding {
                session: ExternalSession {
                    provider_id: ProviderId::new(provider)?,
                    session_id: ExternalId::new(session)?,
                },
                attempt_key: AttemptKey::new(attempt)?,
            })
        })
        .transpose()
    }

    /// Bind the already-issued role capability to the concrete provider attempt. MCP processes
    /// resolve this lazily by hash, so thread startup can precede the external session ID.
    pub fn bind_capability_to_provider_attempt(
        &self,
        token: &str,
        session: &ExternalSession,
        attempt_key: &AttemptKey,
    ) -> Result<(), String> {
        if token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("role capability is invalid".into());
        }
        let capability_hash = content_hash(token);
        let connection = self.connection()?;
        let role_scope = connection
            .raw()
            .query_row(
                "SELECT binding_key, project_id, delegation_id FROM role_thread_capabilities
                 WHERE capability_hash = ?1 AND status = 'active'",
                params![capability_hash],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        Some(row.get::<_, i64>(1)?),
                        Some(row.get::<_, i64>(2)?),
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        let (binding_key, project_id, delegation_id, is_main) = if let Some(scope) = role_scope {
            (scope.0, scope.1, scope.2, false)
        } else {
            let main_role = connection
                .raw()
                .query_row(
                    "SELECT role_key FROM main_overseer_capability
                     WHERE capability_hash = ?1 AND status = 'active'",
                    params![capability_hash],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| "active role capability was not found".to_string())?;
            (main_role, None, None, true)
        };
        if is_main && binding_key != "main-overseer" {
            return Err("Main capability resolved to an unexpected logical role".into());
        }
        let matching_attempt: bool = connection
            .raw()
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM provider_attempts attempt
                   JOIN provider_sessions provider_session
                     ON provider_session.provider_id = attempt.provider_id
                    AND provider_session.session_id = attempt.session_id
                   WHERE attempt.attempt_key = ?1 AND attempt.provider_id = ?2
                     AND attempt.session_id = ?3 AND attempt.binding_key = ?4
                     AND provider_session.project_id IS ?5
                     AND attempt.delegation_id IS ?6
                     AND (?7 = 0 OR (attempt.inbox_id IS NOT NULL AND attempt.delegation_id IS NULL))
                     AND attempt.status IN ('created','submitted','ack_unknown','running','needs_reconcile')
                 )",
                params![
                    attempt_key.as_str(),
                    session.provider_id.as_str(),
                    session.session_id.as_str(),
                    binding_key,
                    project_id,
                    delegation_id,
                    is_main
                ],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if !matching_attempt {
            return Err("capability and provider attempt scopes do not match".into());
        }
        let updated = if is_main {
            connection.raw().execute(
                "UPDATE main_overseer_capability SET provider_id = ?1, provider_session_id = ?2,
                   attempt_key = ?3, updated_at = ?4 WHERE capability_hash = ?5 AND status = 'active'",
                params![session.provider_id.as_str(), session.session_id.as_str(), attempt_key.as_str(), unix_time(), capability_hash],
            )
        } else {
            connection.raw().execute(
                "UPDATE role_thread_capabilities SET provider_id = ?1, provider_session_id = ?2,
                   attempt_key = ?3, updated_at = ?4 WHERE capability_hash = ?5 AND status = 'active'",
                params![session.provider_id.as_str(), session.session_id.as_str(), attempt_key.as_str(), unix_time(), capability_hash],
            )
        }
        .map_err(sqlite_error)?;
        if updated == 1 {
            Ok(())
        } else {
            Err("active role capability changed before provider attempt binding".into())
        }
    }

    /// Persist and deduplicate a normalized provider event. Unidentified events are observations
    /// only and cannot update attempts or other canonical state.
    pub fn record_provider_event(&self, event: &ProviderEvent) -> Result<bool, String> {
        event.validate()?;
        let Some(event_key) = event.external_event_key()? else {
            return Ok(false);
        };
        let now = unix_time();
        let payload = serde_json::to_value(event).map_err(|error| error.to_string())?;
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let aggregate_id = event_session_id(event)
            .map(|session| format!("{}:{}", event.provider_id, session.session_id))
            .unwrap_or_else(|| event.provider_id.to_string());
        let Some(_) = append_external_event(
            &transaction,
            "provider_session",
            &aggregate_id,
            event_type(event),
            &event_key,
            &payload,
            now,
        )?
        else {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(false);
        };
        apply_event_to_attempts(&transaction, event, now)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(true)
    }

    pub fn context_receipts_for_attempt(
        &self,
        session: &ExternalSession,
        attempt_key: &AttemptKey,
    ) -> Result<Vec<SessionContextReceipt>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .raw()
            .prepare(
                "SELECT memory_id, content_hash, state, observed_at FROM provider_context_receipts
                 WHERE provider_id = ?1 AND session_id = ?2 AND attempt_key = ?3 ORDER BY memory_id",
            )
            .map_err(sqlite_error)?;
        let receipts = statement
            .query_map(
                params![
                    session.provider_id.as_str(),
                    session.session_id.as_str(),
                    attempt_key.as_str()
                ],
                |row| {
                    let state: String = row.get(2)?;
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        state,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .map_err(sqlite_error)?
            .map(|result| {
                let (memory_id, content_hash, state, observed_at) = result.map_err(sqlite_error)?;
                Ok(SessionContextReceipt {
                    session: session.clone(),
                    attempt_key: attempt_key.clone(),
                    memory_id,
                    content_hash,
                    state: parse_delivery_state(&state)?,
                    observed_at,
                })
            })
            .collect();
        receipts
    }
}

fn delegation_context_manifest(
    transaction: &Transaction<'_>,
    delegation_id: i64,
    now: i64,
) -> Result<ContextManifest, String> {
    let (project_id, task_id, run_id, objective_id, packet_json, packet_hash) = transaction
        .query_row(
            "SELECT delegation.project_id, delegation.task_id, delegation.run_id,
                    task.objective_id, delegation.packet_json, delegation.packet_hash
             FROM delegations delegation JOIN tasks task ON task.id = delegation.task_id
             WHERE delegation.id = ?1",
            params![delegation_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .map_err(sqlite_error)?;
    if content_hash(&packet_json) != packet_hash {
        return Err("stored delegation packet hash does not match its contents".into());
    }
    let packet: DelegationTaskPacket =
        serde_json::from_str(&packet_json).map_err(|error| error.to_string())?;
    validate_packet_references(
        transaction,
        project_id,
        objective_id,
        task_id,
        run_id,
        &packet,
        now,
    )?;
    let mut manifest = ContextManifest {
        entries: Vec::with_capacity(packet.referenced_memory_ids.len()),
        excluded_private_count: 0,
    };
    for memory_id in packet.referenced_memory_ids {
        let content_hash: String = transaction
            .query_row(
                "SELECT content_hash FROM memories WHERE id = ?1",
                params![memory_id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        manifest.entries.push(ContextEntry {
            memory_id,
            content_hash,
            delivery: ContextDelivery::Full,
        });
    }
    manifest.validate()?;
    Ok(manifest)
}

fn validate_session(
    _session: &ExternalSession,
    binding_key: &str,
    status: &str,
    permission_hash: Option<&str>,
) -> Result<(), String> {
    if binding_key.trim().is_empty()
        || binding_key.len() > 256
        || !matches!(status, "active" | "inactive" | "unknown")
    {
        return Err("provider session identity or status is invalid".into());
    }
    if let Some(hash) = permission_hash {
        validate_hash(hash)?;
    }
    Ok(())
}

fn validate_hash(hash: &str) -> Result<(), String> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("content hashes must be 64 hexadecimal characters".into());
    }
    Ok(())
}

fn read_provider_session(
    transaction: &Transaction<'_>,
    session: &ExternalSession,
) -> Result<Option<ProviderSessionRecord>, String> {
    transaction
        .query_row(
            "SELECT provider_id, session_id, binding_key, project_id, role_key, delegation_id,
                    status, last_turn_id, permission_hash, created_at, updated_at
             FROM provider_sessions WHERE provider_id = ?1 AND session_id = ?2",
            params![session.provider_id.as_str(), session.session_id.as_str()],
            provider_session_from_row,
        )
        .optional()
        .map_err(sqlite_error)
}

fn provider_session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderSessionRecord> {
    let provider_id: String = row.get(0)?;
    let session_id: String = row.get(1)?;
    let last_turn_id: Option<String> = row.get(7)?;
    Ok(ProviderSessionRecord {
        provider_id: ProviderId::new(provider_id).map_err(to_sql_error)?,
        session_id: ExternalId::new(session_id).map_err(to_sql_error)?,
        binding_key: row.get(2)?,
        project_id: row.get(3)?,
        role_key: row.get(4)?,
        delegation_id: row.get(5)?,
        status: row.get(6)?,
        last_turn_id: last_turn_id
            .map(ExternalId::new)
            .transpose()
            .map_err(to_sql_error)?,
        permission_hash: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn read_provider_attempt(
    transaction: &Transaction<'_>,
    attempt_key: &AttemptKey,
) -> Result<Option<ProviderAttemptRecord>, String> {
    transaction
        .query_row(
            "SELECT attempt_key, provider_id, session_id, binding_key, dispatch_id, inbox_id,
                    delegation_id, request_hash, status, turn_id, created_at, updated_at
             FROM provider_attempts WHERE attempt_key = ?1",
            params![attempt_key.as_str()],
            provider_attempt_from_row,
        )
        .optional()
        .map_err(sqlite_error)
}

fn provider_attempt_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProviderAttemptRecord> {
    let attempt_key: String = row.get(0)?;
    let provider_id: String = row.get(1)?;
    let session_id: String = row.get(2)?;
    let turn_id: Option<String> = row.get(9)?;
    Ok(ProviderAttemptRecord {
        attempt_key: AttemptKey::new(attempt_key).map_err(to_sql_error)?,
        session: ExternalSession {
            provider_id: ProviderId::new(provider_id).map_err(to_sql_error)?,
            session_id: ExternalId::new(session_id).map_err(to_sql_error)?,
        },
        binding_key: row.get(3)?,
        dispatch_id: row.get(4)?,
        inbox_id: row.get(5)?,
        delegation_id: row.get(6)?,
        request_hash: row.get(7)?,
        status: row.get(8)?,
        turn_id: turn_id
            .map(ExternalId::new)
            .transpose()
            .map_err(to_sql_error)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn valid_attempt_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        (
            "created",
            "submitted" | "ack_unknown" | "running" | "failed" | "blocked" | "needs_reconcile"
        ) | (
            "submitted",
            "running" | "completed" | "failed" | "blocked" | "needs_reconcile"
        ) | (
            "ack_unknown",
            "running" | "completed" | "failed" | "blocked" | "needs_reconcile"
        ) | (
            "running",
            "completed" | "failed" | "blocked" | "needs_reconcile"
        ) | ("needs_reconcile", "completed" | "failed" | "blocked")
    )
}

fn apply_event_to_attempts(
    transaction: &Transaction<'_>,
    event: &ProviderEvent,
    now: i64,
) -> Result<(), String> {
    match &event.event {
        ProviderEventKind::TurnStarted { turn } => {
            if let Some(attempt_key) = active_attempt_for_turn(
                transaction,
                &event.provider_id,
                &turn.session.session_id,
                &turn.turn_id,
            )? {
                let current_status: String = transaction
                    .query_row(
                        "SELECT status FROM provider_attempts WHERE attempt_key = ?1",
                        params![attempt_key],
                        |row| row.get(0),
                    )
                    .map_err(sqlite_error)?;
                // A late start event can resolve an uncertain attempt without making it safe
                // to resume blindly; keep the reconciliation state until a terminal event.
                let next_status = if current_status == "needs_reconcile" {
                    "needs_reconcile"
                } else {
                    "running"
                };
                transaction
                    .execute(
                        "UPDATE provider_attempts SET status = ?1, turn_id = ?2, updated_at = ?3
                         WHERE attempt_key = ?4 AND (turn_id IS NULL OR turn_id = ?2)",
                        params![next_status, turn.turn_id.as_str(), now, attempt_key],
                    )
                    .map_err(sqlite_error)?;
            }
        }
        ProviderEventKind::TurnCompleted { turn, outcome } => {
            let state = match outcome {
                TurnOutcome::Completed => "completed",
                TurnOutcome::Failed | TurnOutcome::Interrupted => "failed",
            };
            if let Some(attempt_key) = active_attempt_for_turn(
                transaction,
                &event.provider_id,
                &turn.session.session_id,
                &turn.turn_id,
            )? {
                transaction
                    .execute(
                        "UPDATE provider_attempts SET status = ?1, turn_id = ?2, updated_at = ?3
                         WHERE attempt_key = ?4 AND (turn_id IS NULL OR turn_id = ?2)",
                        params![state, turn.turn_id.as_str(), now, attempt_key],
                    )
                    .map_err(sqlite_error)?;
            }
        }
        ProviderEventKind::ApprovalRequested { session, .. } => {
            if let Some(attempt_key) =
                unique_active_attempt(transaction, &event.provider_id, &session.session_id)?
            {
                transaction
                    .execute(
                        "UPDATE provider_attempts SET status = 'blocked', updated_at = ?1
                         WHERE attempt_key = ?2",
                        params![now, attempt_key],
                    )
                    .map_err(sqlite_error)?;
            }
        }
        ProviderEventKind::TransportLost {
            session: Some(session),
        } => {
            transaction
                .execute(
                    "UPDATE provider_attempts SET status = 'needs_reconcile', updated_at = ?1
                     WHERE provider_id = ?2 AND session_id = ?3
                       AND status IN ('created','submitted','ack_unknown','running')",
                    params![now, event.provider_id.as_str(), session.session_id.as_str()],
                )
                .map_err(sqlite_error)?;
        }
        _ => {}
    }
    Ok(())
}

/// Resolve a turn event only to its exact durable turn identity. Before the submit receipt
/// supplies a turn ID, a unique unbound attempt may be correlated; ambiguous or already-terminal
/// matches are observations and cannot mutate another attempt.
fn active_attempt_for_turn(
    transaction: &Transaction<'_>,
    provider_id: &ProviderId,
    session_id: &ExternalId,
    turn_id: &ExternalId,
) -> Result<Option<String>, String> {
    let exact = {
        let mut statement = transaction
            .prepare(
                "SELECT attempt_key, status FROM provider_attempts
                 WHERE provider_id = ?1 AND session_id = ?2 AND turn_id = ?3",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map(
                params![provider_id.as_str(), session_id.as_str(), turn_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(sqlite_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?
    };
    if !exact.is_empty() {
        if exact.len() != 1 || !is_active_attempt_status(&exact[0].1) {
            return Ok(None);
        }
        return Ok(Some(exact[0].0.clone()));
    }

    let unbound = {
        let mut statement = transaction
            .prepare(
                "SELECT attempt_key FROM provider_attempts
                 WHERE provider_id = ?1 AND session_id = ?2 AND turn_id IS NULL
                   AND status IN ('created','submitted','ack_unknown','running','needs_reconcile')",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map(params![provider_id.as_str(), session_id.as_str()], |row| {
                row.get::<_, String>(0)
            })
            .map_err(sqlite_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?
    };
    Ok((unbound.len() == 1).then(|| unbound[0].clone()))
}

/// Approval events carry a session and request ID but no turn ID in F1. Only associate one when
/// exactly one unresolved attempt exists for that session.
fn unique_active_attempt(
    transaction: &Transaction<'_>,
    provider_id: &ProviderId,
    session_id: &ExternalId,
) -> Result<Option<String>, String> {
    let mut statement = transaction
        .prepare(
            "SELECT attempt_key FROM provider_attempts
             WHERE provider_id = ?1 AND session_id = ?2
               AND status IN ('created','submitted','ack_unknown','running','needs_reconcile')",
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map(params![provider_id.as_str(), session_id.as_str()], |row| {
            row.get::<_, String>(0)
        })
        .map_err(sqlite_error)?;
    let attempts = rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?;
    Ok((attempts.len() == 1).then(|| attempts[0].clone()))
}

fn is_active_attempt_status(status: &str) -> bool {
    matches!(
        status,
        "created" | "submitted" | "ack_unknown" | "running" | "needs_reconcile"
    )
}

fn event_session_id(event: &ProviderEvent) -> Option<&ExternalSession> {
    match &event.event {
        ProviderEventKind::TurnStarted { turn } | ProviderEventKind::TurnCompleted { turn, .. } => {
            Some(&turn.session)
        }
        ProviderEventKind::ApprovalRequested { session, .. }
        | ProviderEventKind::UsageObserved { session, .. } => Some(session),
        ProviderEventKind::TransportLost { session } => session.as_ref(),
        ProviderEventKind::CapacityObserved { .. } | ProviderEventKind::Unrecognized { .. } => None,
    }
}

fn event_type(event: &ProviderEvent) -> &'static str {
    match &event.event {
        ProviderEventKind::TurnStarted { .. } => "provider_turn_started",
        ProviderEventKind::TurnCompleted { .. } => "provider_turn_completed",
        ProviderEventKind::ApprovalRequested { .. } => "provider_approval_requested",
        ProviderEventKind::UsageObserved { .. } => "provider_usage_observed",
        ProviderEventKind::CapacityObserved { .. } => "provider_capacity_observed",
        ProviderEventKind::TransportLost { .. } => "provider_transport_lost",
        ProviderEventKind::Unrecognized { .. } => "provider_unrecognized_observation",
    }
}

fn delivery_state_name(state: DeliveryState) -> &'static str {
    match state {
        DeliveryState::Selected => "selected",
        DeliveryState::Submitted => "submitted",
        DeliveryState::AgentReportedRead => "agent_reported_read",
    }
}

fn parse_delivery_state(state: &str) -> Result<DeliveryState, String> {
    match state {
        "selected" => Ok(DeliveryState::Selected),
        "submitted" => Ok(DeliveryState::Submitted),
        "agent_reported_read" => Ok(DeliveryState::AgentReportedRead),
        _ => Err("stored provider context receipt state is invalid".into()),
    }
}

fn to_sql_error(error: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
    )
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("cannot read or write provider session state: {error}")
}

fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn scratch_database(label: &str) -> PathBuf {
        let path = std::env::current_dir()
            .unwrap()
            .join("target/manual-dev/codex")
            .join(format!("{label}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path.join("cathulu.sqlite3")
    }

    #[test]
    fn version_20_upgrade_backfills_legacy_codex_session_and_namespaces_collisions() {
        let database_path = scratch_database("provider-sessions-migration");
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
                 CREATE TABLE projects (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   name TEXT NOT NULL,
                   path TEXT NOT NULL UNIQUE,
                   git_branch TEXT,
                   git_head TEXT,
                   git_dirty INTEGER NOT NULL DEFAULT 0,
                   changed_paths_json TEXT NOT NULL DEFAULT '[]',
                   registered_at INTEGER NOT NULL,
                   reconciled_at INTEGER NOT NULL
                 );
                 INSERT INTO schema_migrations(version, applied_at) VALUES (1, 1);",
            )
            .unwrap();
        for migration in [
            migrations::VERSION_2,
            migrations::VERSION_3,
            migrations::VERSION_4,
            migrations::VERSION_5,
            migrations::VERSION_6,
            migrations::VERSION_7,
            migrations::VERSION_8,
            migrations::VERSION_9,
            migrations::VERSION_10,
            migrations::VERSION_11,
            migrations::VERSION_12,
            migrations::VERSION_13,
            migrations::VERSION_14,
            migrations::VERSION_15,
            migrations::VERSION_16,
            migrations::VERSION_17,
            migrations::VERSION_18,
            migrations::VERSION_19,
            migrations::VERSION_20,
        ] {
            connection.execute_batch(migration).unwrap();
        }
        connection
            .execute(
                "INSERT INTO projects(name, path, registered_at, reconciled_at)
                 VALUES ('fixture', 'C:/fixture', 1, 1)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO role_thread_bindings
                 (binding_key, role_key, delegation_id, project_id, thread_id, status,
                  last_turn_id, created_at, updated_at)
                 VALUES ('main-overseer', 'main-overseer', NULL, NULL, 'same-provider-id',
                         'active', 'turn-1', 1, 1)",
                [],
            )
            .unwrap();

        connection.execute_batch(migrations::VERSION_21).unwrap();
        connection
            .execute(
                "INSERT INTO provider_sessions
                 (provider_id, session_id, binding_key, project_id, role_key, delegation_id,
                  status, last_turn_id, permission_hash, created_at, updated_at)
                 VALUES ('claude', 'same-provider-id', 'main-overseer', NULL, 'main-overseer',
                         NULL, 'active', NULL, NULL, 2, 2)",
                [],
            )
            .unwrap();
        let namespaced_rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM provider_sessions WHERE session_id = 'same-provider-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let legacy_turn: String = connection
            .query_row(
                "SELECT last_turn_id FROM provider_sessions
                 WHERE provider_id = 'codex' AND session_id = 'same-provider-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(namespaced_rows, 2);
        assert_eq!(legacy_turn, "turn-1");
    }

    #[test]
    fn out_of_order_turn_events_never_attach_to_a_different_attempt() {
        let mut connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE provider_attempts (
                   attempt_key TEXT PRIMARY KEY,
                   provider_id TEXT NOT NULL,
                   session_id TEXT NOT NULL,
                   turn_id TEXT,
                   status TEXT NOT NULL,
                   updated_at INTEGER NOT NULL
                 );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO provider_attempts VALUES ('old', 'codex', 'session', 'turn-old', 'completed', 1)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO provider_attempts VALUES ('next', 'codex', 'session', NULL, 'created', 2)",
                [],
            )
            .unwrap();

        let provider = ProviderId::codex();
        let session = ExternalSession {
            provider_id: provider.clone(),
            session_id: ExternalId::new("session").unwrap(),
        };
        let transaction = connection.transaction().unwrap();
        let late_start = ProviderEvent {
            provider_id: provider.clone(),
            event_id: Some(ExternalId::new("event-late-start").unwrap()),
            sequence: None,
            event: ProviderEventKind::TurnStarted {
                turn: crate::agent_provider::ExternalTurn {
                    session: session.clone(),
                    turn_id: ExternalId::new("turn-old").unwrap(),
                },
            },
        };
        apply_event_to_attempts(&transaction, &late_start, 3).unwrap();
        let next_status: String = transaction
            .query_row(
                "SELECT status FROM provider_attempts WHERE attempt_key = 'next'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_status, "created");

        let current_start = ProviderEvent {
            provider_id: provider.clone(),
            event_id: Some(ExternalId::new("event-current-start").unwrap()),
            sequence: None,
            event: ProviderEventKind::TurnStarted {
                turn: crate::agent_provider::ExternalTurn {
                    session: session.clone(),
                    turn_id: ExternalId::new("turn-current").unwrap(),
                },
            },
        };
        apply_event_to_attempts(&transaction, &current_start, 4).unwrap();
        let next_turn: Option<String> = transaction
            .query_row(
                "SELECT turn_id FROM provider_attempts WHERE attempt_key = 'next'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_turn.as_deref(), Some("turn-current"));

        transaction
            .execute(
                "INSERT INTO provider_attempts VALUES ('ambiguous', 'codex', 'session', NULL, 'created', 5)",
                [],
            )
            .unwrap();
        transaction
            .execute(
                "INSERT INTO provider_attempts VALUES ('also-ambiguous', 'codex', 'session', NULL, 'created', 5)",
                [],
            )
            .unwrap();
        let ambiguous_start = ProviderEvent {
            provider_id: provider,
            event_id: Some(ExternalId::new("event-ambiguous-start").unwrap()),
            sequence: None,
            event: ProviderEventKind::TurnStarted {
                turn: crate::agent_provider::ExternalTurn {
                    session,
                    turn_id: ExternalId::new("turn-ambiguous").unwrap(),
                },
            },
        };
        apply_event_to_attempts(&transaction, &ambiguous_start, 6).unwrap();
        let ambiguous_status: String = transaction
            .query_row(
                "SELECT status FROM provider_attempts WHERE attempt_key = 'ambiguous'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ambiguous_status, "created");
        transaction.commit().unwrap();
    }

    #[test]
    fn selected_context_is_redelivered_per_provider_session_and_read_receipt() {
        let database_path = scratch_database("provider-context-receipts");
        let data_dir = database_path.parent().unwrap();
        let store = Store::open(data_dir).unwrap();
        let connection = rusqlite::Connection::open(store.database_path()).unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .unwrap();

        connection
            .execute(
                "INSERT INTO projects(name, path, registered_at, reconciled_at)
                 VALUES ('fixture', 'provider-context-fixture', 1, 1)",
                [],
            )
            .unwrap();
        let project_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO objectives(project_id, title, description, created_at, updated_at)
                 VALUES (?1, 'objective', '', 1, 1)",
                params![project_id],
            )
            .unwrap();
        let objective_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO tasks(objective_id, title, description, created_at, updated_at)
                 VALUES (?1, 'task', '', 1, 1)",
                params![objective_id],
            )
            .unwrap();
        let task_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO runs(project_id, objective_id, task_id, status, started_at)
                 VALUES (?1, ?2, ?3, 'running', 1)",
                params![project_id, objective_id, task_id],
            )
            .unwrap();
        let run_id = connection.last_insert_rowid();

        let body = "fixture context retained for the replacement session";
        let memory_hash = content_hash(body);
        connection
            .execute(
                "INSERT INTO memories
                 (scope_key, scope_type, scope_id, memory_class, temperature, kind, body,
                  content_hash, review_state, created_at, updated_at, active)
                 VALUES (?1, 'project', ?2, 'canonical', 'warm', 'fact', ?3, ?4,
                         'accepted', 1, 1, 1)",
                params![
                    format!("project:{project_id}"),
                    project_id.to_string(),
                    body,
                    memory_hash
                ],
            )
            .unwrap();
        let memory_id = connection.last_insert_rowid();
        let packet = DelegationTaskPacket {
            schema_version: 1,
            title: "fixture assignment".into(),
            objective: "exercise provider context receipts".into(),
            acceptance_criteria: vec!["context is scoped to each attempt".into()],
            constraints: Vec::new(),
            allowed_scope: vec!["src/**".into()],
            referenced_memory_ids: vec![memory_id],
            referenced_artifact_ids: Vec::new(),
            requested_report_fields: vec!["summary".into()],
        };
        let packet_json = serde_json::to_string(&packet).unwrap();
        let packet_hash = content_hash(&packet_json);
        let request_hash = content_hash("delegation request");
        let idempotency_hash = content_hash("delegation key");
        connection
            .execute(
                "INSERT INTO delegations
                 (project_id, task_id, run_id, role, status, packet_schema_version, packet_json,
                  packet_hash, request_hash, idempotency_key_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'sub_overseer', 'prepared', 1, ?4, ?5, ?6, ?7, 1, 1)",
                params![
                    project_id,
                    task_id,
                    run_id,
                    packet_json,
                    packet_hash,
                    request_hash,
                    idempotency_hash
                ],
            )
            .unwrap();
        let delegation_id = connection.last_insert_rowid();
        let binding_key = format!("project-manager:{project_id}");
        let session_id = "shared-provider-session-id";
        connection
            .execute(
                "INSERT INTO role_thread_bindings
                 (binding_key, role_key, delegation_id, project_id, thread_id, status,
                  last_turn_id, created_at, updated_at)
                 VALUES (?1, ?1, NULL, ?2, ?3, 'active', NULL, 1, 1)",
                params![binding_key, project_id, session_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO role_dispatch_outbox
                 (delegation_id, idempotency_key_hash, status, created_at, updated_at)
                 VALUES (?1, ?2, 'queued', 1, 1)",
                params![delegation_id, content_hash("outbox key")],
            )
            .unwrap();
        let dispatch_id = connection.last_insert_rowid();
        drop(connection);

        let codex_session = ExternalSession {
            provider_id: ProviderId::codex(),
            session_id: ExternalId::new(session_id).unwrap(),
        };
        store
            .record_provider_session(&codex_session, &binding_key, "active", None, None)
            .unwrap();
        let manifest = store.preview_delegation_context(delegation_id).unwrap();
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].memory_id, memory_id);
        let first_attempt = AttemptKey::new("codex-attempt").unwrap();
        store
            .begin_provider_attempt(
                &first_attempt,
                &codex_session,
                Some(dispatch_id),
                None,
                Some(delegation_id),
                &content_hash(&serde_json::to_string(&manifest).unwrap()),
            )
            .unwrap();
        store
            .record_selected_provider_context(&codex_session, &first_attempt, &manifest)
            .unwrap();
        let codex_capability = store
            .issue_role_thread_capability(&binding_key, "sub_overseer", project_id, delegation_id)
            .unwrap();
        store
            .bind_capability_to_provider_attempt(
                &codex_capability.token,
                &codex_session,
                &first_attempt,
            )
            .unwrap();
        store
            .set_provider_attempt_state(&first_attempt, "submitted", None)
            .unwrap();
        store
            .set_provider_context_state(&codex_session, &first_attempt, DeliveryState::Submitted)
            .unwrap();
        let memories = store
            .list_memories("project", Some(&project_id.to_string()), false, false)
            .unwrap();
        assert_eq!(
            store
                .record_session_context_read(&content_hash(&codex_capability.token), &memories)
                .unwrap(),
            1
        );
        store
            .set_provider_attempt_state(&first_attempt, "completed", None)
            .unwrap();

        // The same provider session text is a separate identity under Claude's namespace.
        let claude_session = ExternalSession {
            provider_id: ProviderId::claude(),
            session_id: ExternalId::new(session_id).unwrap(),
        };
        store
            .record_provider_session(&claude_session, &binding_key, "active", None, None)
            .unwrap();
        let second_attempt = AttemptKey::new("claude-attempt").unwrap();
        store
            .begin_provider_attempt(
                &second_attempt,
                &claude_session,
                Some(dispatch_id),
                None,
                Some(delegation_id),
                &content_hash("replacement request"),
            )
            .unwrap();
        let second_manifest = store.preview_delegation_context(delegation_id).unwrap();
        store
            .record_selected_provider_context(&claude_session, &second_attempt, &second_manifest)
            .unwrap();
        let claude_capability = store
            .issue_role_thread_capability(&binding_key, "sub_overseer", project_id, delegation_id)
            .unwrap();
        store
            .bind_capability_to_provider_attempt(
                &claude_capability.token,
                &claude_session,
                &second_attempt,
            )
            .unwrap();
        store
            .set_provider_attempt_state(&second_attempt, "submitted", None)
            .unwrap();
        store
            .set_provider_context_state(&claude_session, &second_attempt, DeliveryState::Submitted)
            .unwrap();
        assert_eq!(
            store
                .record_session_context_read(&content_hash(&claude_capability.token), &memories)
                .unwrap(),
            1
        );
        let codex_receipts = store
            .context_receipts_for_attempt(&codex_session, &first_attempt)
            .unwrap();
        let claude_receipts = store
            .context_receipts_for_attempt(&claude_session, &second_attempt)
            .unwrap();
        assert_eq!(codex_receipts.len(), 1);
        assert_eq!(claude_receipts.len(), 1);
        assert_eq!(codex_receipts[0].state, DeliveryState::AgentReportedRead);
        assert_eq!(claude_receipts[0].state, DeliveryState::AgentReportedRead);
        assert_eq!(codex_receipts[0].memory_id, claude_receipts[0].memory_id);
    }
}
