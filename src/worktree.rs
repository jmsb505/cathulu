use crate::agent_provider::{
    validate_provider_credential_source, CredentialSource, PermissionManifest,
    ProviderCapabilities, WorkspaceAccess,
};
use crate::{events::append_event, hash::content_hash, Store};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeReportVerification {
    pub status: String,
    pub verified_commits: Vec<String>,
    pub unverified_commits: Vec<String>,
    pub verified_paths: Vec<String>,
    pub unverified_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DelegationWorktreeState {
    pub worktree_path_hash: Option<String>,
    pub branch: Option<String>,
    pub base_commit: Option<String>,
    pub observed_head: Option<String>,
    pub is_clean: Option<bool>,
    pub changed_paths: Vec<String>,
    pub report_verification: Option<WorktreeReportVerification>,
    pub last_reconciled_at: Option<i64>,
}

#[derive(Debug)]
struct GitState {
    path: PathBuf,
    branch: Option<String>,
    head: Option<String>,
    clean: bool,
    changed_paths: Vec<String>,
}

const MAX_WORKSPACE_LEASE_SECONDS: i64 = 3_600;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceLeaseStatus {
    Active,
    NeedsReconcile,
    Released,
    Revoked,
    Expired,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLeaseRecord {
    pub id: i64,
    pub project_id: i64,
    pub delegation_id: i64,
    pub role_kind: String,
    pub access: WorkspaceAccess,
    pub owner_id: String,
    pub workspace_path_hash: String,
    pub common_dir_hash: String,
    pub base_commit: String,
    pub status: WorkspaceLeaseStatus,
    pub lease_expires_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Runtime-only result. It carries the canonical path only after revalidating the lease and Git
/// identity; public records retain only the path hash.
#[derive(Clone, Debug)]
pub struct ValidatedWorkspaceLease {
    pub record: WorkspaceLeaseRecord,
    pub workspace_root: PathBuf,
    pub credential_source: CredentialSource,
}

impl Store {
    pub fn reconcile_delegation_worktree(
        &self,
        delegation_id: i64,
        worktree_path: &Path,
    ) -> Result<DelegationWorktreeState, String> {
        if delegation_id <= 0 {
            return Err("delegation ID must be positive".to_string());
        }
        let (project_path, base_commit, report_json) = {
            let connection = self.connection()?;
            connection
                .raw()
                .query_row(
                    "SELECT p.path, state.base_commit, report.report_json
                     FROM delegations d
                     JOIN projects p ON p.id = d.project_id
                     LEFT JOIN delegation_worktree_states state ON state.delegation_id = d.id
                     LEFT JOIN worker_reports report ON report.delegation_id = d.id
                     WHERE d.id = ?1",
                    params![delegation_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| format!("delegation {delegation_id} was not found"))?
        };

        let registered_path = PathBuf::from(project_path)
            .canonicalize()
            .map_err(|error| format!("registered project path is unavailable: {error}"))?;
        let registered_root = git_text(&registered_path, &["rev-parse", "--show-toplevel"])?;
        let registered_root = canonical_path(PathBuf::from(registered_root.trim()))?;
        if !same_path(&registered_root, &registered_path) {
            return Err("registered project path no longer resolves to its Git root".to_string());
        }

        let requested = worktree_path
            .canonicalize()
            .map_err(|error| format!("worktree path is unavailable: {error}"))?;
        if !requested.is_dir() {
            return Err("worktree path is not a directory".to_string());
        }
        let worktree_root = git_text(&requested, &["rev-parse", "--show-toplevel"])?;
        let worktree_root = canonical_path(PathBuf::from(worktree_root.trim()))?;
        if !listed_worktree(&registered_root, &worktree_root)? {
            return Err("path is not an exact Git worktree registered to this project".to_string());
        }
        if same_path(&registered_root, &worktree_root) {
            return Err("delegation reconciliation requires a linked worktree distinct from the registered project root".to_string());
        }
        let registered_common = common_dir(&registered_root)?;
        let worktree_common = common_dir(&worktree_root)?;
        if !same_path(&registered_common, &worktree_common) {
            return Err(
                "worktree does not share the registered project's Git common directory".to_string(),
            );
        }

        let observed = inspect_worktree(&worktree_root, base_commit.as_deref())?;
        let verification = verify_report(
            &worktree_root,
            base_commit.as_deref(),
            observed.head.as_deref(),
            &observed.changed_paths,
            report_json.as_deref(),
        );
        let now = unix_time();
        let path = display_path(&observed.path);
        let path_hash = content_hash(&format!("delegation-worktree-path-v1\0{path}"));
        let changed_json = serde_json::to_string(&observed.changed_paths)
            .map_err(|error| format!("cannot serialize Git changed paths: {error}"))?;
        let verification_json = serde_json::to_string(&verification)
            .map_err(|error| format!("cannot serialize worker report verification: {error}"))?;

        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let exists = transaction
            .query_row(
                "SELECT 1 FROM delegation_worktree_states WHERE delegation_id = ?1",
                params![delegation_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(sqlite_error)?
            .is_some();
        if exists {
            transaction
                .execute(
                    "UPDATE delegation_worktree_states SET worktree_path = ?1, worktree_path_hash = ?2,
                     branch = ?3, observed_head = ?4, is_clean = ?5, changed_paths_json = ?6,
                     report_verification_json = ?7, last_reconciled_at = ?8 WHERE delegation_id = ?9",
                    params![
                        path,
                        path_hash,
                        observed.branch,
                        observed.head,
                        observed.clean,
                        changed_json,
                        verification_json,
                        now,
                        delegation_id
                    ],
                )
                .map_err(sqlite_error)?;
        } else {
            transaction
                .execute(
                    "INSERT INTO delegation_worktree_states
                     (delegation_id, base_commit, worktree_path, worktree_path_hash, branch, observed_head,
                      is_clean, changed_paths_json, report_verification_json, last_reconciled_at, created_at)
                     VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                    params![
                        delegation_id,
                        path,
                        path_hash,
                        observed.branch,
                        observed.head,
                        observed.clean,
                        changed_json,
                        verification_json,
                        now
                    ],
                )
                .map_err(sqlite_error)?;
        }
        append_event(
            &transaction,
            "delegation",
            delegation_id,
            "delegation_worktree_reconciled",
            &json!({
                "worktreePathHash": path_hash,
                "branch": observed.branch,
                "observedHead": observed.head,
                "isClean": observed.clean,
                "changedPathCount": observed.changed_paths.len(),
                "reportVerification": verification.status,
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;

        Ok(DelegationWorktreeState {
            worktree_path_hash: Some(path_hash),
            branch: observed.branch,
            base_commit,
            observed_head: observed.head,
            is_clean: Some(observed.clean),
            changed_paths: observed.changed_paths,
            report_verification: Some(verification),
            last_reconciled_at: Some(now),
        })
    }

    pub(crate) fn delegation_worktree_state(
        &self,
        delegation_id: i64,
    ) -> Result<Option<DelegationWorktreeState>, String> {
        let connection = self.connection()?;
        let row = connection
            .raw()
            .query_row(
                "SELECT worktree_path_hash, branch, base_commit, observed_head, is_clean,
                        changed_paths_json, report_verification_json, last_reconciled_at
                 FROM delegation_worktree_states WHERE delegation_id = ?1",
                params![delegation_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<bool>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        row.map(
            |(path_hash, branch, base, head, clean, changed, verification, reconciled_at)| {
                Ok(DelegationWorktreeState {
                    worktree_path_hash: path_hash,
                    branch,
                    base_commit: base,
                    observed_head: head,
                    is_clean: clean,
                    changed_paths: changed
                        .map(|changed| serde_json::from_str(&changed))
                        .transpose()
                        .map_err(|error| format!("stored Git changed paths are invalid: {error}"))?
                        .unwrap_or_default(),
                    report_verification: verification
                        .map(|verification| serde_json::from_str(&verification))
                        .transpose()
                        .map_err(|error| {
                            format!("stored report verification is invalid: {error}")
                        })?,
                    last_reconciled_at: reconciled_at,
                })
            },
        )
        .transpose()
    }
}

impl Store {
    /// Acquire one bounded lease for a prepared/dispatched delegation. Writable leases are
    /// worker-only, require a clean linked worktree, and serialize writers per project until a
    /// narrower semantic-overlap contract exists.
    pub fn claim_delegation_workspace(
        &self,
        delegation_id: i64,
        owner_id: &str,
        workspace_path: &Path,
        access: WorkspaceAccess,
        now: i64,
        lease_seconds: i64,
    ) -> Result<WorkspaceLeaseRecord, String> {
        validate_lease_request(delegation_id, owner_id, now, lease_seconds)?;
        self.expire_stale_workspace_leases(now)?;
        let scope = self.delegation_workspace_scope(delegation_id)?;
        if !matches!(scope.status.as_str(), "prepared" | "dispatched") {
            return Err("workspace lease requires a prepared or dispatched delegation".into());
        }
        if scope.role_kind == "sub_overseer" && access != WorkspaceAccess::ReadOnly {
            return Err("Project Manager/coordinator workspace access must be read-only".into());
        }
        let verified = verify_project_workspace(
            &scope.project_path,
            workspace_path,
            access,
            access == WorkspaceAccess::WorkspaceWrite,
        )?;
        let canonical_workspace_path = display_path(&verified.workspace_root);

        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let active_attempt: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM provider_attempts
                 WHERE delegation_id = ?1
                   AND status IN ('created','submitted','ack_unknown','running','needs_reconcile'))",
                params![delegation_id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if active_attempt {
            return Err("an earlier provider attempt still requires reconciliation".into());
        }

        let existing = transaction
            .query_row(
                "SELECT id, project_id, delegation_id, role_kind, access, owner_id,
                        workspace_path_hash, common_dir_hash, base_commit, status,
                        lease_expires_at, created_at, updated_at
                 FROM workspace_leases WHERE delegation_id = ?1 AND status = 'active'
                 ORDER BY id DESC LIMIT 1",
                params![delegation_id],
                workspace_lease_from_row,
            )
            .optional()
            .map_err(sqlite_error)?;
        if let Some(existing) = existing {
            let requested_hash = verified.workspace_path_hash.as_str();
            if existing.owner_id == owner_id
                && existing.access == access
                && existing.workspace_path_hash == requested_hash
                && existing.lease_expires_at > now
            {
                transaction.commit().map_err(sqlite_error)?;
                return Ok(existing);
            }
            return Err("delegation already has a different active workspace lease".into());
        }

        let role_binding_key = match scope.role_kind.as_str() {
            "sub_overseer" => format!("project-manager:{}", scope.project_id),
            "worker" => format!("worker:{delegation_id}"),
            _ => return Err("unsupported delegation role for workspace lease".into()),
        };
        transaction
            .execute(
                "INSERT INTO workspace_leases
                 (project_id, delegation_id, role_kind, role_binding_key, access, owner_id,
                  workspace_path, workspace_path_hash, common_dir_hash, base_commit, status,
                  lease_expires_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'active', ?11, ?12, ?12)",
                params![
                    scope.project_id,
                    delegation_id,
                    scope.role_kind,
                    role_binding_key,
                    access_name(access),
                    owner_id,
                    canonical_workspace_path,
                    verified.workspace_path_hash,
                    verified.common_dir_hash,
                    verified.head,
                    now.saturating_add(lease_seconds),
                    now,
                ],
            )
            .map_err(|error| {
                format!("workspace is already leased or its role is active: {error}")
            })?;
        let lease_id = transaction.last_insert_rowid();
        append_workspace_lease_event(
            &transaction,
            lease_id,
            "claimed",
            &json!({
                "access": access_name(access),
                "workspacePathHash": verified.workspace_path_hash,
                "commonDirHash": verified.common_dir_hash,
                "baseCommit": verified.head,
                "expiresAt": now.saturating_add(lease_seconds),
            }),
            now,
        )?;
        let record = transaction
            .query_row(
                "SELECT id, project_id, delegation_id, role_kind, access, owner_id,
                        workspace_path_hash, common_dir_hash, base_commit, status,
                        lease_expires_at, created_at, updated_at
                 FROM workspace_leases WHERE id = ?1",
                params![lease_id],
                workspace_lease_from_row,
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(record)
    }

    /// Validate the exact leased path immediately before provider startup. This does not start a
    /// provider or inspect credential material; unknown/unpermitted credential sources fail.
    pub fn validate_provider_workspace(
        &self,
        lease_id: i64,
        permissions: &PermissionManifest,
        capabilities: &ProviderCapabilities,
        now: i64,
    ) -> Result<ValidatedWorkspaceLease, String> {
        let credential_source = validate_provider_credential_source(capabilities, permissions)?;
        if lease_id <= 0 || now < 0 {
            return Err("workspace lease identity or validation time is invalid".into());
        }
        let (record, workspace_path, project_path) = self.workspace_lease_context(lease_id)?;
        if record.status != WorkspaceLeaseStatus::Active || record.lease_expires_at <= now {
            return Err("workspace lease is expired, revoked or requires reconciliation".into());
        }
        if permissions.workspace_root != PathBuf::from(&workspace_path) {
            return Err("provider workspace does not match the exact leased path".into());
        }
        if permissions.access == WorkspaceAccess::WorkspaceWrite
            && record.access != WorkspaceAccess::WorkspaceWrite
        {
            return Err("read-only workspace lease cannot grant provider write access".into());
        }
        let verified = verify_project_workspace(
            &project_path,
            &permissions.workspace_root,
            record.access,
            false,
        )?;
        if verified.workspace_path_hash != record.workspace_path_hash
            || verified.common_dir_hash != record.common_dir_hash
        {
            return Err("leased workspace path or Git common directory has changed".into());
        }
        Ok(ValidatedWorkspaceLease {
            record,
            workspace_root: verified.workspace_root,
            credential_source,
        })
    }

    /// Extend an active lease only while its owner and canonical Git workspace still match.
    pub fn renew_workspace_lease(
        &self,
        lease_id: i64,
        owner_id: &str,
        now: i64,
        lease_seconds: i64,
    ) -> Result<WorkspaceLeaseRecord, String> {
        validate_lease_request(lease_id, owner_id, now, lease_seconds)?;
        let (record, workspace_path, project_path) = self.workspace_lease_context(lease_id)?;
        if record.owner_id != owner_id || record.status != WorkspaceLeaseStatus::Active {
            return Err("workspace lease is not active for this owner".into());
        }
        if record.lease_expires_at <= now {
            self.expire_stale_workspace_leases(now)?;
            return Err("expired workspace lease requires reconciliation before renewal".into());
        }
        let verified = match verify_project_workspace(
            &project_path,
            Path::new(&workspace_path),
            record.access,
            false,
        ) {
            Ok(verified)
                if verified.workspace_path_hash == record.workspace_path_hash
                    && verified.common_dir_hash == record.common_dir_hash =>
            {
                verified
            }
            _ => {
                self.mark_workspace_lease_needs_reconcile(
                    lease_id,
                    owner_id,
                    now,
                    "workspace path or Git common directory could not be revalidated",
                )?;
                return Err("workspace lease lost its assigned Git identity".into());
            }
        };
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let updated = transaction
            .execute(
                "UPDATE workspace_leases SET lease_expires_at = ?1, updated_at = ?2
                 WHERE id = ?3 AND owner_id = ?4 AND status = 'active' AND lease_expires_at > ?2",
                params![now.saturating_add(lease_seconds), now, lease_id, owner_id],
            )
            .map_err(sqlite_error)?;
        if updated != 1 {
            return Err("workspace lease changed before renewal completed".into());
        }
        append_workspace_lease_event(
            &transaction,
            lease_id,
            "renewed",
            &json!({
                "workspacePathHash": verified.workspace_path_hash,
                "expiresAt": now.saturating_add(lease_seconds),
            }),
            now,
        )?;
        let renewed = transaction
            .query_row(
                "SELECT id, project_id, delegation_id, role_kind, access, owner_id,
                        workspace_path_hash, common_dir_hash, base_commit, status,
                        lease_expires_at, created_at, updated_at
                 FROM workspace_leases WHERE id = ?1",
                params![lease_id],
                workspace_lease_from_row,
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(renewed)
    }

    fn delegation_workspace_scope(
        &self,
        delegation_id: i64,
    ) -> Result<DelegationWorkspaceScope, String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT delegation.project_id, delegation.role, delegation.status, project.path
                 FROM delegations delegation JOIN projects project ON project.id = delegation.project_id
                 WHERE delegation.id = ?1",
                params![delegation_id],
                |row| {
                    Ok(DelegationWorkspaceScope {
                        project_id: row.get(0)?,
                        role_kind: row.get(1)?,
                        status: row.get(2)?,
                        project_path: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("delegation {delegation_id} was not found"))
    }

    fn workspace_lease_context(
        &self,
        lease_id: i64,
    ) -> Result<(WorkspaceLeaseRecord, String, String), String> {
        let connection = self.connection()?;
        connection
            .raw()
            .query_row(
                "SELECT lease.id, lease.project_id, lease.delegation_id, lease.role_kind,
                        lease.access, lease.owner_id, lease.workspace_path_hash,
                        lease.common_dir_hash, lease.base_commit, lease.status,
                        lease.lease_expires_at, lease.created_at, lease.updated_at,
                        lease.workspace_path, project.path
                 FROM workspace_leases lease JOIN projects project ON project.id = lease.project_id
                 WHERE lease.id = ?1",
                params![lease_id],
                |row| {
                    Ok((
                        workspace_lease_from_row(row)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, String>(14)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("workspace lease {lease_id} was not found"))
    }

    /// Expire idle leases and quarantine expired leases whose provider outcome is still unknown.
    /// Quarantined leases keep their role/path locks until explicit reconciliation.
    pub fn expire_stale_workspace_leases(&self, now: i64) -> Result<usize, String> {
        if now < 0 {
            return Err("workspace lease time cannot be negative".into());
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let ids = {
            let mut statement = transaction
                .prepare(
                    "SELECT id FROM workspace_leases
                     WHERE status = 'active' AND lease_expires_at <= ?1 ORDER BY id",
                )
                .map_err(sqlite_error)?;
            let rows = statement
                .query_map(params![now], |row| row.get::<_, i64>(0))
                .map_err(sqlite_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?
        };
        for lease_id in &ids {
            let attempt_count: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM provider_attempts
                     WHERE workspace_lease_id = ?1
                       AND status IN ('created','submitted','ack_unknown','running','needs_reconcile')",
                    params![lease_id],
                    |row| row.get(0),
                )
                .map_err(sqlite_error)?;
            let next = if attempt_count == 0 {
                "expired"
            } else {
                transaction
                    .execute(
                        "UPDATE provider_attempts SET status = 'needs_reconcile', updated_at = ?1
                         WHERE workspace_lease_id = ?2
                           AND status IN ('created','submitted','ack_unknown','running')",
                        params![now, lease_id],
                    )
                    .map_err(sqlite_error)?;
                "needs_reconcile"
            };
            transaction
                .execute(
                    "UPDATE workspace_leases SET status = ?1, updated_at = ?2
                     WHERE id = ?3 AND status = 'active'",
                    params![next, now, lease_id],
                )
                .map_err(sqlite_error)?;
            revoke_capability_for_lease(&transaction, *lease_id, now)?;
            append_workspace_lease_event(
                &transaction,
                *lease_id,
                if next == "expired" {
                    "expired"
                } else {
                    "needs_reconcile"
                },
                &json!({ "activeAttemptCount": attempt_count }),
                now,
            )?;
        }
        transaction.commit().map_err(sqlite_error)?;
        Ok(ids.len())
    }

    /// End a lease without deleting, moving or cleaning its worktree. If the provider attempt is
    /// unresolved, preserve the exclusive lock and require an explicit reconciliation step.
    pub fn release_workspace_lease(
        &self,
        lease_id: i64,
        owner_id: &str,
        now: i64,
        reason: &str,
    ) -> Result<WorkspaceLeaseRecord, String> {
        validate_lease_request(lease_id, owner_id, now, 1)?;
        validate_lease_reason(reason)?;
        self.end_workspace_lease(lease_id, owner_id, now, reason, false)
    }

    pub fn revoke_workspace_lease(
        &self,
        lease_id: i64,
        owner_id: &str,
        now: i64,
        reason: &str,
    ) -> Result<WorkspaceLeaseRecord, String> {
        validate_lease_request(lease_id, owner_id, now, 1)?;
        validate_lease_reason(reason)?;
        self.end_workspace_lease(lease_id, owner_id, now, reason, true)
    }

    fn end_workspace_lease(
        &self,
        lease_id: i64,
        owner_id: &str,
        now: i64,
        reason: &str,
        revoke: bool,
    ) -> Result<WorkspaceLeaseRecord, String> {
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let current = transaction
            .query_row(
                "SELECT owner_id, status FROM workspace_leases WHERE id = ?1",
                params![lease_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("workspace lease {lease_id} was not found"))?;
        if current.0 != owner_id || !matches!(current.1.as_str(), "active" | "needs_reconcile") {
            return Err("workspace lease is not open for this owner".into());
        }
        let active_attempts: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM provider_attempts
                 WHERE workspace_lease_id = ?1
                   AND status IN ('created','submitted','ack_unknown','running','needs_reconcile')",
                params![lease_id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        let next = if active_attempts > 0 {
            transaction
                .execute(
                    "UPDATE provider_attempts SET status = 'needs_reconcile', updated_at = ?1
                     WHERE workspace_lease_id = ?2
                       AND status IN ('created','submitted','ack_unknown','running')",
                    params![now, lease_id],
                )
                .map_err(sqlite_error)?;
            "needs_reconcile"
        } else if revoke {
            "revoked"
        } else {
            "released"
        };
        transaction
            .execute(
                "UPDATE workspace_leases SET status = ?1, updated_at = ?2
                 WHERE id = ?3 AND status = ?4",
                params![next, now, lease_id, current.1],
            )
            .map_err(sqlite_error)?;
        revoke_capability_for_lease(&transaction, lease_id, now)?;
        append_workspace_lease_event(
            &transaction,
            lease_id,
            if next == "needs_reconcile" {
                "needs_reconcile"
            } else if revoke {
                "revoked"
            } else {
                "released"
            },
            &json!({ "reason": reason, "activeAttemptCount": active_attempts }),
            now,
        )?;
        let record = transaction
            .query_row(
                "SELECT id, project_id, delegation_id, role_kind, access, owner_id,
                        workspace_path_hash, common_dir_hash, base_commit, status,
                        lease_expires_at, created_at, updated_at
                 FROM workspace_leases WHERE id = ?1",
                params![lease_id],
                workspace_lease_from_row,
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(record)
    }

    /// Release a quarantined lease only after every provider attempt has a terminal disposition.
    /// This records recovery; it deliberately performs no filesystem cleanup.
    pub fn resolve_workspace_lease(
        &self,
        lease_id: i64,
        owner_id: &str,
        now: i64,
        resolution: &str,
    ) -> Result<WorkspaceLeaseRecord, String> {
        validate_lease_request(lease_id, owner_id, now, 1)?;
        validate_lease_reason(resolution)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let current = transaction
            .query_row(
                "SELECT owner_id, status FROM workspace_leases WHERE id = ?1",
                params![lease_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("workspace lease {lease_id} was not found"))?;
        if current.0 != owner_id || current.1 != "needs_reconcile" {
            return Err("workspace lease is not awaiting reconciliation for this owner".into());
        }
        let active_attempts: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM provider_attempts
                 WHERE workspace_lease_id = ?1
                   AND status IN ('created','submitted','ack_unknown','running','needs_reconcile')",
                params![lease_id],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if active_attempts > 0 {
            return Err("provider attempt remains uncertain; workspace stays locked".into());
        }
        transaction
            .execute(
                "UPDATE workspace_leases SET status = 'released', updated_at = ?1
                 WHERE id = ?2 AND owner_id = ?3 AND status = 'needs_reconcile'",
                params![now, lease_id, owner_id],
            )
            .map_err(sqlite_error)?;
        append_workspace_lease_event(
            &transaction,
            lease_id,
            "reconciled",
            &json!({ "resolution": resolution }),
            now,
        )?;
        let record = transaction
            .query_row(
                "SELECT id, project_id, delegation_id, role_kind, access, owner_id,
                        workspace_path_hash, common_dir_hash, base_commit, status,
                        lease_expires_at, created_at, updated_at
                 FROM workspace_leases WHERE id = ?1",
                params![lease_id],
                workspace_lease_from_row,
            )
            .map_err(sqlite_error)?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(record)
    }

    fn mark_workspace_lease_needs_reconcile(
        &self,
        lease_id: i64,
        owner_id: &str,
        now: i64,
        reason: &str,
    ) -> Result<(), String> {
        validate_lease_reason(reason)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let updated = transaction
            .execute(
                "UPDATE workspace_leases SET status = 'needs_reconcile', updated_at = ?1
                 WHERE id = ?2 AND owner_id = ?3 AND status = 'active'",
                params![now, lease_id, owner_id],
            )
            .map_err(sqlite_error)?;
        if updated == 0 {
            let status = transaction
                .query_row(
                    "SELECT status FROM workspace_leases WHERE id = ?1 AND owner_id = ?2",
                    params![lease_id, owner_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(sqlite_error)?;
            if status.as_deref() != Some("needs_reconcile") {
                return Err("workspace lease changed before it could be quarantined".into());
            }
        }
        transaction
            .execute(
                "UPDATE provider_attempts SET status = 'needs_reconcile', updated_at = ?1
                 WHERE workspace_lease_id = ?2
                   AND status IN ('created','submitted','ack_unknown','running')",
                params![now, lease_id],
            )
            .map_err(sqlite_error)?;
        revoke_capability_for_lease(&transaction, lease_id, now)?;
        append_workspace_lease_event(
            &transaction,
            lease_id,
            "needs_reconcile",
            &json!({ "reason": reason }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)
    }
}

#[derive(Debug)]
struct DelegationWorkspaceScope {
    project_id: i64,
    role_kind: String,
    status: String,
    project_path: String,
}

struct VerifiedWorkspace {
    workspace_root: PathBuf,
    head: String,
    workspace_path_hash: String,
    common_dir_hash: String,
}

fn validate_lease_request(
    identity: i64,
    owner_id: &str,
    now: i64,
    lease_seconds: i64,
) -> Result<(), String> {
    if identity <= 0
        || now < 0
        || owner_id.trim().is_empty()
        || owner_id.trim() != owner_id
        || owner_id.len() > 256
        || owner_id.chars().any(char::is_control)
        || !(1..=MAX_WORKSPACE_LEASE_SECONDS).contains(&lease_seconds)
    {
        return Err("workspace lease identity, owner, time or duration is invalid".into());
    }
    Ok(())
}

fn validate_lease_reason(reason: &str) -> Result<(), String> {
    if reason.trim().is_empty()
        || reason.trim() != reason
        || reason.len() > 1_000
        || reason.chars().any(char::is_control)
    {
        return Err("workspace lease reason must be bounded plain text".into());
    }
    Ok(())
}

fn access_name(access: WorkspaceAccess) -> &'static str {
    match access {
        WorkspaceAccess::ReadOnly => "read_only",
        WorkspaceAccess::WorkspaceWrite => "workspace_write",
    }
}

fn workspace_lease_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceLeaseRecord> {
    let access = match row.get::<_, String>(4)?.as_str() {
        "read_only" => WorkspaceAccess::ReadOnly,
        "workspace_write" => WorkspaceAccess::WorkspaceWrite,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let status = match row.get::<_, String>(9)?.as_str() {
        "active" => WorkspaceLeaseStatus::Active,
        "needs_reconcile" => WorkspaceLeaseStatus::NeedsReconcile,
        "released" => WorkspaceLeaseStatus::Released,
        "revoked" => WorkspaceLeaseStatus::Revoked,
        "expired" => WorkspaceLeaseStatus::Expired,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(WorkspaceLeaseRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        delegation_id: row.get(2)?,
        role_kind: row.get(3)?,
        access,
        owner_id: row.get(5)?,
        workspace_path_hash: row.get(6)?,
        common_dir_hash: row.get(7)?,
        base_commit: row.get(8)?,
        status,
        lease_expires_at: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn append_workspace_lease_event(
    transaction: &Transaction<'_>,
    lease_id: i64,
    event_type: &str,
    details: &Value,
    now: i64,
) -> Result<(), String> {
    let details = serde_json::to_string(details)
        .map_err(|error| format!("cannot serialize workspace lease event: {error}"))?;
    transaction
        .execute(
            "INSERT INTO workspace_lease_events(lease_id, event_type, details_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![lease_id, event_type, details, now],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn revoke_capability_for_lease(
    transaction: &Transaction<'_>,
    lease_id: i64,
    now: i64,
) -> Result<(), String> {
    transaction
        .execute(
            "UPDATE role_thread_capabilities SET status = 'revoked', updated_at = ?1
             WHERE workspace_lease_id = ?2 AND status = 'active'",
            params![now, lease_id],
        )
        .map_err(sqlite_error)?;
    Ok(())
}

fn verify_project_workspace(
    project_path: &str,
    requested_path: &Path,
    access: WorkspaceAccess,
    require_clean: bool,
) -> Result<VerifiedWorkspace, String> {
    if !requested_path.is_absolute() {
        return Err("workspace lease path must be absolute and canonical".into());
    }
    let registered_path = PathBuf::from(project_path)
        .canonicalize()
        .map_err(|error| format!("registered project path is unavailable: {error}"))?;
    if !registered_path.is_dir() {
        return Err("registered project path is not a directory".into());
    }
    let registered_root = git_text(&registered_path, &["rev-parse", "--show-toplevel"])?;
    let registered_root = canonical_path(PathBuf::from(registered_root.trim()))?;
    if !same_path(&registered_root, &registered_path) {
        return Err("registered project path no longer resolves to its Git root".into());
    }

    let requested_display = display_path(requested_path);
    let workspace_root = canonical_path(requested_path.to_path_buf())?;
    let canonical_display = display_path(&workspace_root);
    if normalized_path_for_compare(&requested_display)
        != normalized_path_for_compare(&canonical_display)
    {
        return Err("workspace aliases and symlink paths cannot receive a lease".into());
    }
    if !workspace_root.is_dir() {
        return Err("workspace path is not a directory".into());
    }
    let worktree_root = git_text(&workspace_root, &["rev-parse", "--show-toplevel"])?;
    let worktree_root = canonical_path(PathBuf::from(worktree_root.trim()))?;
    if !same_path(&workspace_root, &worktree_root) {
        return Err("workspace path must be the exact Git worktree root".into());
    }
    let is_registered_root = same_path(&registered_root, &workspace_root);
    if access == WorkspaceAccess::WorkspaceWrite && is_registered_root {
        return Err("writers cannot receive the registered project root".into());
    }
    if !is_registered_root {
        if !listed_worktree(&registered_root, &workspace_root)? {
            return Err(
                "workspace is not an exact linked worktree of the registered project".into(),
            );
        }
        if !same_path(
            &common_dir(&registered_root)?,
            &common_dir(&workspace_root)?,
        ) {
            return Err(
                "workspace Git common directory does not match the registered project".into(),
            );
        }
    }
    let observed = inspect_worktree(&workspace_root, None)?;
    let head = observed
        .head
        .ok_or_else(|| "workspace must have a committed Git base".to_string())?;
    if require_clean && !observed.clean {
        return Err("writer worktree must be clean before a lease is issued".into());
    }
    let workspace_path_hash = content_hash(&format!(
        "workspace-path-v1\0{}",
        stable_path_key(&display_path(&workspace_root))
    ));
    let common = common_dir(&workspace_root)?;
    let common_dir_hash = content_hash(&format!(
        "workspace-common-dir-v1\0{}",
        stable_path_key(&display_path(&common))
    ));
    Ok(VerifiedWorkspace {
        workspace_root,
        head,
        workspace_path_hash,
        common_dir_hash,
    })
}

fn stable_path_key(path: &str) -> String {
    #[cfg(windows)]
    {
        path.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string()
    }
}

fn normalized_path_for_compare(path: &str) -> String {
    #[cfg(windows)]
    {
        path.replace('/', "\\").to_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string()
    }
}

fn inspect_worktree(root: &Path, base: Option<&str>) -> Result<GitState, String> {
    let branch = git_optional(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
        .filter(|value| !value.is_empty());
    let head = git_optional(root, &["rev-parse", "--verify", "HEAD"])?
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
        .filter(|value| !value.is_empty());
    let status = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let clean = status.is_empty();
    let mut changed = BTreeSet::new();
    if let Some(base) = base {
        let output = git(root, &["diff", base, "--name-only", "-z", "--"])?;
        add_nul_paths(&mut changed, &output);
    }
    let untracked = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    add_nul_paths(&mut changed, &untracked);
    if base.is_none() {
        add_status_paths(&mut changed, &status);
    }
    Ok(GitState {
        path: root.to_path_buf(),
        branch,
        head,
        clean,
        changed_paths: changed.into_iter().collect(),
    })
}

fn verify_report(
    root: &Path,
    base: Option<&str>,
    head: Option<&str>,
    changed_paths: &[String],
    report_json: Option<&str>,
) -> WorktreeReportVerification {
    let mut result = WorktreeReportVerification {
        status: "no_report".to_string(),
        verified_commits: Vec::new(),
        unverified_commits: Vec::new(),
        verified_paths: Vec::new(),
        unverified_paths: Vec::new(),
    };
    let Some(report_json) = report_json else {
        return result;
    };
    let Ok(report) = serde_json::from_str::<Value>(report_json) else {
        result.status = "unavailable".to_string();
        return result;
    };
    let commits = report
        .get("commits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let paths = report
        .get("changedFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(normalize_report_path)
        .collect::<Vec<_>>();
    if commits.is_empty() && paths.is_empty() {
        result.status = if base.is_some() && head.is_some() {
            "no_claims"
        } else {
            "unavailable"
        }
        .to_string();
        return result;
    }
    let (Some(base), Some(head)) = (base, head) else {
        result.status = "unavailable".to_string();
        result.unverified_commits = commits;
        result.unverified_paths = paths;
        return result;
    };
    if !is_commit(root, base) || !is_commit(root, head) || !is_ancestor(root, base, head) {
        result.status = "unavailable".to_string();
        result.unverified_commits = commits;
        result.unverified_paths = paths;
        return result;
    }
    for commit in commits {
        if is_commit(root, &commit)
            && is_ancestor(root, base, &commit)
            && is_ancestor(root, &commit, head)
        {
            result.verified_commits.push(commit);
        } else {
            result.unverified_commits.push(commit);
        }
    }
    let changed = changed_paths.iter().cloned().collect::<BTreeSet<_>>();
    for path in paths {
        if changed.contains(&path) {
            result.verified_paths.push(path);
        } else {
            result.unverified_paths.push(path);
        }
    }
    let mismatch = !result.unverified_commits.is_empty() || !result.unverified_paths.is_empty();
    result.status = if mismatch { "mismatch" } else { "verified" }.to_string();
    result
}

fn listed_worktree(registered_root: &Path, target_root: &Path) -> Result<bool, String> {
    let bytes = git(registered_root, &["worktree", "list", "--porcelain", "-z"])?;
    let mut current_path = None;
    for field in bytes.split(|byte| *byte == 0) {
        if field.is_empty() {
            if let Some(path) = current_path.take() {
                let path = canonical_path(path)?;
                if same_path(&path, target_root) {
                    return Ok(true);
                }
            }
        } else if let Some(path) = field.strip_prefix(b"worktree ") {
            current_path = Some(PathBuf::from(String::from_utf8_lossy(path).into_owned()));
        }
    }
    if let Some(path) = current_path {
        return Ok(same_path(&canonical_path(path)?, target_root));
    }
    Ok(false)
}

fn common_dir(root: &Path) -> Result<PathBuf, String> {
    let value = git_text(root, &["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(value.trim());
    canonical_path(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

fn canonical_path(path: PathBuf) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("Git path could not be resolved: {error}"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let bytes = git(root, args)?;
    String::from_utf8(bytes).map_err(|_| "Git returned a non-UTF-8 metadata value".to_string())
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    crate::suppress_windows_console(&mut command);
    let output = command
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("cannot launch Git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_error(&output, args))
    }
}

fn git_optional(root: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, String> {
    let mut command = Command::new("git");
    crate::suppress_windows_console(&mut command);
    let output = command
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("cannot launch Git: {error}"))?;
    if output.status.success() {
        Ok(Some(output.stdout))
    } else {
        Ok(None)
    }
}

fn is_commit(root: &Path, commit: &str) -> bool {
    !commit.is_empty()
        && !commit.chars().any(char::is_whitespace)
        && git(root, &["cat-file", "-e", &format!("{commit}^{{commit}}")]).is_ok()
}

fn is_ancestor(root: &Path, ancestor: &str, descendant: &str) -> bool {
    let mut command = Command::new("git");
    crate::suppress_windows_console(&mut command);
    let output = command
        .arg("-C")
        .arg(root)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .output();
    matches!(output, Ok(output) if output.status.success())
}

fn add_nul_paths(paths: &mut BTreeSet<String>, bytes: &[u8]) {
    for path in bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        paths.insert(String::from_utf8_lossy(path).into_owned());
    }
}

fn add_status_paths(paths: &mut BTreeSet<String>, bytes: &[u8]) {
    let fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut index = 0;
    while index < fields.len() {
        let record = fields[index];
        index += 1;
        if record.len() < 4 {
            continue;
        }
        let status = &record[..2];
        paths.insert(String::from_utf8_lossy(&record[3..]).into_owned());
        if status.contains(&b'R') || status.contains(&b'C') {
            if let Some(path) = fields.get(index).filter(|path| !path.is_empty()) {
                paths.insert(String::from_utf8_lossy(path).into_owned());
            }
            index += 1;
        }
    }
}

fn normalize_report_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(unc_path) = value.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc_path}");
        }
        if let Some(normal_path) = value.strip_prefix(r"\\?\") {
            return normal_path.to_string();
        }
    }
    value.into_owned()
}

fn git_error(output: &Output, args: &[&str]) -> String {
    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if message.is_empty() {
        format!("Git command {:?} failed with {}", args, output.status)
    } else {
        message
    }
}

fn unix_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}
