use crate::{events::append_event, hash::content_hash, Store};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceProposal {
    pub id: i64,
    pub maintenance_run_id: i64,
    pub proposal_type: String,
    pub subject_ids: Vec<i64>,
    pub reason: String,
    pub minimum_support: Option<i64>,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryGardenerReport {
    pub maintenance_run_id: i64,
    pub started_at: i64,
    pub finished_at: i64,
    pub proposal_count: usize,
    pub findings: Value,
    pub proposals: Vec<MaintenanceProposal>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRetentionPolicy {
    pub minimum_age_seconds: Option<i64>,
    pub require_extraction: bool,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceCollectionCandidate {
    pub artifact_id: i64,
    pub artifact_kind: String,
    pub eligible: bool,
    pub reasons: Vec<String>,
    pub logical_bytes: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceCollectionResult {
    pub archived_artifact_ids: Vec<i64>,
    pub logical_bytes_collected: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageSnapshot {
    pub id: i64,
    pub run_id: Option<i64>,
    pub project_id: Option<i64>,
    pub workflow_version_id: Option<i64>,
    pub sampled_at: i64,
    pub sqlite_bytes: i64,
    pub growth_bytes: i64,
    pub hot_bytes: i64,
    pub warm_bytes: i64,
    pub cold_bytes: i64,
    pub artifact_reference_bytes: i64,
    pub orphaned_evidence_count: i64,
    pub duplicate_artifact_groups: i64,
    pub bytes_compacted: i64,
    pub bytes_collected: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicyInput {
    pub policy_key: String,
    pub version: i64,
    pub retrieval_method: String,
    pub hot_budget_bytes: i64,
    pub warm_candidate_limit: i64,
    pub minimum_promotion_support: i64,
    pub retention_minimum_age_seconds: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicyVersion {
    pub id: i64,
    pub policy_key: String,
    pub version: i64,
    pub retrieval_method: String,
    pub hot_budget_bytes: i64,
    pub warm_candidate_limit: i64,
    pub minimum_promotion_support: i64,
    pub retention_minimum_age_seconds: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicyEvaluationInput {
    pub policy_id: i64,
    pub experiment_key: String,
    pub task_type: String,
    pub run_id: i64,
    pub missing_context_failures: i64,
    pub regressions: i64,
    pub rework_count: i64,
    pub context_bytes: i64,
    pub reintroduced_bytes: i64,
    pub useful_retrievals: i64,
    pub retrieval_count: i64,
    pub latency_ms: Option<i64>,
    pub human_interventions: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicyComparison {
    pub experiment_key: String,
    pub task_type: String,
    pub policy_id: i64,
    pub policy_key: String,
    pub version: i64,
    pub run_count: i64,
    pub verified_successes: i64,
    pub missing_context_failures: i64,
    pub regressions: i64,
    pub average_context_bytes: f64,
    pub average_reintroduced_bytes: f64,
    pub useful_retrieval_rate: f64,
    pub average_latency_ms: Option<f64>,
    pub human_interventions: i64,
}

impl Store {
    /// Runs deterministic maintenance checks and stores reviewable proposals. It never deletes evidence.
    pub fn run_memory_gardener(
        &self,
        minimum_promotion_support: i64,
    ) -> Result<MemoryGardenerReport, String> {
        if !(2..=100).contains(&minimum_promotion_support) {
            return Err("minimum promotion support must be between 2 and 100".to_string());
        }
        let started_at = now();
        let connection = self.connection()?;
        let proposals = collect_proposals(connection.raw(), started_at, minimum_promotion_support)?;
        let findings = json!({
            "inactiveHot": proposals.iter().filter(|proposal| proposal.0 == "demote_inactive_hot").count(),
            "staleMemories": proposals.iter().filter(|proposal| proposal.0 == "demote_stale_memory").count(),
            "promotionCandidates": proposals.iter().filter(|proposal| proposal.0 == "promote_episodic").count(),
            "duplicateMemoryGroups": proposals.iter().filter(|proposal| proposal.0 == "duplicate_memories").count(),
            "duplicateArtifactGroups": proposals.iter().filter(|proposal| proposal.0 == "duplicate_artifacts").count(),
            "orphanArtifacts": proposals.iter().filter(|proposal| proposal.0 == "orphan_evidence").count(),
        });
        let proposal_count = proposals.len();
        let finished_at = now();
        let sqlite_bytes = database_size(self.database_path())?;
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        transaction.execute(
            "INSERT INTO memory_maintenance_runs (started_at, finished_at, proposal_count, findings_json, sqlite_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![started_at, finished_at, proposal_count as i64, findings.to_string(), sqlite_bytes],
        ).map_err(sqlite_error)?;
        let maintenance_run_id = transaction.last_insert_rowid();
        let mut result = Vec::with_capacity(proposal_count);
        for (proposal_type, subject_ids, reason, minimum_support) in proposals {
            let subject_ids_json = serde_json::to_string(&subject_ids)
                .map_err(|error| format!("cannot encode maintenance proposal subjects: {error}"))?;
            transaction
                .execute(
                    "INSERT INTO memory_maintenance_proposals
                 (maintenance_run_id, proposal_type, subject_ids_json, reason, minimum_support, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        maintenance_run_id,
                        proposal_type,
                        subject_ids_json,
                        reason,
                        minimum_support,
                        finished_at
                    ],
                )
                .map_err(sqlite_error)?;
            result.push(MaintenanceProposal {
                id: transaction.last_insert_rowid(),
                maintenance_run_id,
                proposal_type,
                subject_ids,
                reason,
                minimum_support,
                status: "proposed".to_string(),
            });
        }
        append_event(
            &transaction,
            "memory_maintenance_run",
            maintenance_run_id,
            "memory_gardener_run",
            &json!({ "proposalCount": proposal_count, "minimumPromotionSupport": minimum_promotion_support }),
            finished_at,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(MemoryGardenerReport {
            maintenance_run_id,
            started_at,
            finished_at,
            proposal_count,
            findings,
            proposals: result,
        })
    }

    /// Applies only reversible, policy-bounded class/temperature changes after explicit review.
    pub fn review_memory_maintenance_proposal(
        &self,
        proposal_id: i64,
        decision: &str,
    ) -> Result<MaintenanceProposal, String> {
        if !matches!(decision, "accepted" | "rejected") {
            return Err("maintenance proposal decision must be accepted or rejected".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let proposal = load_proposal(&transaction, proposal_id)?
            .ok_or_else(|| format!("maintenance proposal {proposal_id} was not found"))?;
        if proposal.status != "proposed" {
            return Err(format!(
                "maintenance proposal {proposal_id} was already reviewed"
            ));
        }
        if decision == "accepted" {
            apply_proposal(&transaction, &proposal, now)?;
        }
        transaction.execute(
            "UPDATE memory_maintenance_proposals SET status = ?1, reviewed_at = ?2 WHERE id = ?3",
            params![decision, now, proposal_id],
        ).map_err(sqlite_error)?;
        append_event(
            &transaction,
            "memory_maintenance_proposal",
            proposal_id,
            "memory_maintenance_proposal_reviewed",
            &json!({ "decision": decision, "proposalType": proposal.proposal_type }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        let mut reviewed = proposal;
        reviewed.status = decision.to_string();
        Ok(reviewed)
    }

    pub fn set_evidence_retention_policy(
        &self,
        minimum_age_seconds: Option<i64>,
        require_extraction: bool,
    ) -> Result<EvidenceRetentionPolicy, String> {
        if minimum_age_seconds.is_some_and(|age| age < 0) {
            return Err("evidence retention age cannot be negative".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        transaction
            .execute(
                "UPDATE memory_retention_policy
             SET minimum_age_seconds = ?1, require_extraction = ?2, updated_at = ?3
             WHERE singleton_id = 1",
                params![minimum_age_seconds, require_extraction, now],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "memory_retention_policy",
            1,
            "evidence_retention_policy_changed",
            &json!({ "minimumAgeSeconds": minimum_age_seconds, "requireExtraction": require_extraction }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        self.evidence_retention_policy()
    }

    pub fn evidence_retention_policy(&self) -> Result<EvidenceRetentionPolicy, String> {
        self.connection()?.raw().query_row(
            "SELECT minimum_age_seconds, require_extraction, updated_at FROM memory_retention_policy WHERE singleton_id = 1",
            [],
            |row| Ok(EvidenceRetentionPolicy {
                minimum_age_seconds: row.get(0)?,
                require_extraction: row.get(1)?,
                updated_at: row.get(2)?,
            }),
        ).map_err(sqlite_error)
    }

    pub fn record_evidence_extraction(
        &self,
        artifact_id: i64,
        preserved_memory_ids: &[i64],
    ) -> Result<i64, String> {
        if preserved_memory_ids.is_empty() || preserved_memory_ids.len() > 64 {
            return Err(
                "an extraction receipt requires between 1 and 64 durable memory IDs".to_string(),
            );
        }
        let unique = preserved_memory_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        if unique.len() != preserved_memory_ids.len() {
            return Err("extraction receipt memory IDs must be unique".to_string());
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let (content_hash, uri_hash, uri, archived_at): (Option<String>, Option<String>, String, Option<i64>) = transaction.query_row(
            "SELECT content_hash, uri_hash, uri, archived_at FROM evidence_artifacts WHERE id = ?1",
            params![artifact_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional().map_err(sqlite_error)?.ok_or_else(|| format!("artifact {artifact_id} was not found"))?;
        if archived_at.is_some() {
            return Err("cannot add an extraction receipt after evidence was archived".to_string());
        }
        let uri_hash = uri_hash.unwrap_or_else(|| content_hash_fn(&uri));
        let mut preserved = Vec::with_capacity(preserved_memory_ids.len());
        for memory_id in preserved_memory_ids {
            let (memory_class, review_state, memory_hash, source_count): (
                String,
                String,
                String,
                i64,
            ) = transaction
                .query_row(
                    "SELECT m.memory_class, m.review_state, m.content_hash, COUNT(s.source_id)
                 FROM memories m LEFT JOIN memory_sources s ON s.memory_id = m.id
                 WHERE m.id = ?1 GROUP BY m.id",
                    params![memory_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| format!("memory {memory_id} was not found"))?;
            if !matches!(memory_class.as_str(), "canonical" | "aggregate")
                || review_state != "accepted"
                || source_count == 0
                || durable_memory_content_hash(&transaction, *memory_id, now)?.as_deref()
                    != Some(memory_hash.as_str())
            {
                return Err(format!(
                    "memory {memory_id} is not accepted durable knowledge with provenance"
                ));
            }
            preserved.push(json!({ "memoryId": memory_id, "contentHash": memory_hash }));
        }
        let preserved_json = serde_json::to_string(&preserved)
            .map_err(|error| format!("cannot encode extracted memory references: {error}"))?;
        transaction.execute(
            "INSERT INTO evidence_extraction_receipts
             (artifact_id, artifact_content_hash, artifact_uri_hash, preserved_memories_json, extracted_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![artifact_id, content_hash, uri_hash, preserved_json, now],
        ).map_err(sqlite_error)?;
        let receipt_id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "evidence_artifact",
            artifact_id,
            "evidence_extraction_recorded",
            &json!({ "receiptId": receipt_id, "preservedMemoryCount": preserved_memory_ids.len(), "uriHash": uri_hash }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(receipt_id)
    }

    pub fn hold_evidence(&self, artifact_id: i64, reason: &str) -> Result<i64, String> {
        let reason = reason.trim();
        if reason.is_empty() || reason.len() > 1_000 || reason.contains('\0') {
            return Err(
                "evidence hold reason must be 1 to 1,000 bytes and contain no NUL".to_string(),
            );
        }
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM evidence_artifacts WHERE id = ?1 AND archived_at IS NULL)",
            params![artifact_id],
            |row| row.get(0),
        ).map_err(sqlite_error)?;
        if !exists {
            return Err(format!("active artifact {artifact_id} was not found"));
        }
        transaction
            .execute(
                "INSERT INTO evidence_holds (artifact_id, reason, created_at) VALUES (?1, ?2, ?3)",
                params![artifact_id, reason, now],
            )
            .map_err(sqlite_error)?;
        let hold_id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "evidence_artifact",
            artifact_id,
            "evidence_hold_created",
            &json!({ "holdId": hold_id, "reason": reason }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(hold_id)
    }

    pub fn release_evidence_hold(&self, hold_id: i64) -> Result<bool, String> {
        let now = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let artifact_id: Option<i64> = transaction
            .query_row(
                "SELECT artifact_id FROM evidence_holds WHERE id = ?1 AND released_at IS NULL",
                params![hold_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        let Some(artifact_id) = artifact_id else {
            transaction.commit().map_err(sqlite_error)?;
            return Ok(false);
        };
        transaction
            .execute(
                "UPDATE evidence_holds SET released_at = ?1 WHERE id = ?2 AND released_at IS NULL",
                params![now, hold_id],
            )
            .map_err(sqlite_error)?;
        append_event(
            &transaction,
            "evidence_artifact",
            artifact_id,
            "evidence_hold_released",
            &json!({ "holdId": hold_id }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(true)
    }

    pub fn preview_evidence_collection(&self) -> Result<Vec<EvidenceCollectionCandidate>, String> {
        let now = now();
        let connection = self.connection()?;
        let policy = connection.raw().query_row(
            "SELECT minimum_age_seconds, require_extraction FROM memory_retention_policy WHERE singleton_id = 1",
            [],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, bool>(1)?)),
        ).map_err(sqlite_error)?;
        let artifacts = connection.raw().prepare(
            "SELECT id, run_id, artifact_kind, uri, content_hash, protected, created_at, archived_at, uri_hash
             FROM evidence_artifacts WHERE archived_at IS NULL ORDER BY id"
        ).map_err(sqlite_error)?.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, bool>(5)?, row.get::<_, i64>(6)?, row.get::<_, Option<i64>>(7)?, row.get::<_, Option<String>>(8)?))
        }).map_err(sqlite_error)?.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?;
        let mut candidates = Vec::with_capacity(artifacts.len());
        for (
            id,
            run_id,
            kind,
            uri,
            artifact_hash,
            protected,
            created_at,
            archived_at,
            stored_uri_hash,
        ) in artifacts
        {
            let mut reasons = Vec::new();
            if protected {
                reasons.push("protected".to_string());
            }
            if archived_at.is_some() {
                reasons.push("already_archived".to_string());
            }
            match policy.0 {
                None => reasons.push("retention_policy_disabled".to_string()),
                Some(age) if now.saturating_sub(created_at) < age => {
                    reasons.push("minimum_age_not_met".to_string())
                }
                _ => {}
            }
            if policy.1
                && !self.has_valid_extraction(
                    id,
                    artifact_hash.as_deref(),
                    stored_uri_hash.as_deref().unwrap_or(&content_hash_fn(&uri)),
                )?
            {
                reasons.push("verified_extraction_missing".to_string());
            }
            let reference_count: i64 = connection.raw().query_row(
                "SELECT
                   (SELECT COUNT(*) FROM memory_artifact_refs WHERE artifact_id = ?1) +
                   (SELECT COUNT(*) FROM memory_sources WHERE source_type = 'artifact' AND source_id = CAST(?1 AS TEXT))",
                params![id], |row| row.get(0),
            ).map_err(sqlite_error)?;
            if reference_count > 0 {
                reasons.push("memory_provenance_reference".to_string());
            }
            let hold_count: i64 = connection.raw().query_row(
                "SELECT COUNT(*) FROM evidence_holds WHERE artifact_id = ?1 AND released_at IS NULL",
                params![id], |row| row.get(0),
            ).map_err(sqlite_error)?;
            if hold_count > 0 {
                reasons.push("active_investigation_hold".to_string());
            }
            if has_legacy_uri_event(connection.raw(), id)? {
                reasons.push("immutable_event_contains_uri".to_string());
            }
            if let Some(run_id) = run_id {
                let state: Option<String> = connection
                    .raw()
                    .query_row(
                        "SELECT status FROM runs WHERE id = ?1",
                        params![run_id],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(sqlite_error)?;
                if state
                    .as_deref()
                    .is_some_and(|status| status != "verified_success")
                {
                    reasons.push("run_not_verified_success".to_string());
                }
                let unresolved_checks: i64 = connection.raw().query_row(
                    "SELECT COUNT(*) FROM quality_checks WHERE run_id = ?1 AND status IN ('pending', 'failed', 'inconclusive')",
                    params![run_id], |row| row.get(0),
                ).map_err(sqlite_error)?;
                if unresolved_checks > 0 {
                    reasons.push("quality_investigation_required".to_string());
                }
            }
            let logical_bytes = uri.len() as i64;
            candidates.push(EvidenceCollectionCandidate {
                artifact_id: id,
                artifact_kind: kind,
                eligible: reasons.is_empty(),
                reasons,
                logical_bytes,
            });
        }
        Ok(candidates)
    }

    /// Archives URI references only. It preserves the artifact row, stable ID, hashes, receipts, and referenced files.
    pub fn collect_eligible_evidence(&self) -> Result<EvidenceCollectionResult, String> {
        let ids = self
            .preview_evidence_collection()?
            .into_iter()
            .filter(|candidate| candidate.eligible)
            .map(|candidate| candidate.artifact_id)
            .collect::<Vec<_>>();
        self.collect_evidence_references(&ids)
    }

    /// Archives only the exact set reviewed by the caller, revalidating every reference in a write transaction.
    pub fn collect_evidence_references(
        &self,
        requested_artifact_ids: &[i64],
    ) -> Result<EvidenceCollectionResult, String> {
        let unique = requested_artifact_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        if unique.len() != requested_artifact_ids.len() {
            return Err("evidence collection IDs must be unique".to_string());
        }
        let candidates = self.preview_evidence_collection()?;
        let mut by_id = candidates
            .into_iter()
            .map(|candidate| (candidate.artifact_id, candidate))
            .collect::<std::collections::HashMap<_, _>>();
        let mut eligible = Vec::with_capacity(requested_artifact_ids.len());
        for id in requested_artifact_ids {
            let candidate = by_id
                .remove(id)
                .ok_or_else(|| format!("artifact {id} was not present in the reviewed preview"))?;
            if !candidate.eligible {
                return Err(format!(
                    "artifact {id} is no longer eligible: {}",
                    candidate.reasons.join(", ")
                ));
            }
            eligible.push(candidate);
        }
        let mut archived_artifact_ids = Vec::with_capacity(eligible.len());
        let mut logical_bytes_collected = 0_i64;
        for candidate in eligible {
            let now = now();
            let mut connection = self.connection()?;
            let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
            let (run_id, kind, uri, content_hash_value, uri_hash, protected, created_at): (Option<i64>, String, String, Option<String>, Option<String>, bool, i64) = transaction.query_row(
                "SELECT run_id, artifact_kind, uri, content_hash, uri_hash, protected, created_at
                 FROM evidence_artifacts WHERE id = ?1 AND archived_at IS NULL",
                params![candidate.artifact_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            ).optional().map_err(sqlite_error)?.ok_or_else(|| format!("artifact {} changed after preview", candidate.artifact_id))?;
            let (minimum_age, require_extraction): (Option<i64>, bool) = transaction.query_row(
                "SELECT minimum_age_seconds, require_extraction FROM memory_retention_policy WHERE singleton_id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ).map_err(sqlite_error)?;
            let age = now.saturating_sub(created_at);
            let reference_count: i64 = transaction.query_row(
                "SELECT (SELECT COUNT(*) FROM memory_artifact_refs WHERE artifact_id = ?1) +
                        (SELECT COUNT(*) FROM memory_sources WHERE source_type = 'artifact' AND source_id = CAST(?1 AS TEXT))",
                params![candidate.artifact_id], |row| row.get(0),
            ).map_err(sqlite_error)?;
            let active_holds: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM evidence_holds WHERE artifact_id = ?1 AND released_at IS NULL",
                params![candidate.artifact_id], |row| row.get(0),
            ).map_err(sqlite_error)?;
            let qa_investigation = if let Some(run_id) = run_id {
                let state: Option<String> = transaction
                    .query_row(
                        "SELECT status FROM runs WHERE id = ?1",
                        params![run_id],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(sqlite_error)?;
                let unresolved: i64 = transaction.query_row(
                    "SELECT COUNT(*) FROM quality_checks WHERE run_id = ?1 AND status IN ('pending', 'failed', 'inconclusive')",
                    params![run_id], |row| row.get(0),
                ).map_err(sqlite_error)?;
                !state
                    .as_deref()
                    .is_some_and(|value| value == "verified_success")
                    || unresolved > 0
            } else {
                false
            };
            if protected
                || minimum_age.is_none_or(|minimum| age < minimum)
                || reference_count > 0
                || active_holds > 0
                || has_legacy_uri_event(&transaction, candidate.artifact_id)?
                || qa_investigation
            {
                return Err(format!(
                    "artifact {} became ineligible after preview",
                    candidate.artifact_id
                ));
            }
            let uri_hash = uri_hash.unwrap_or_else(|| content_hash_fn(&uri));
            let receipt_id = if require_extraction {
                Some(
                    latest_valid_receipt_id(
                        &transaction,
                        candidate.artifact_id,
                        content_hash_value.as_deref(),
                        &uri_hash,
                    )?
                    .ok_or_else(|| {
                        format!(
                            "artifact {} no longer has a valid extraction receipt",
                            candidate.artifact_id
                        )
                    })?,
                )
            } else {
                None
            };
            let archived_uri = format!("cathulu-archived://artifact/{}", candidate.artifact_id);
            let changed = transaction
                .execute(
                    "UPDATE evidence_artifacts SET uri = ?1, uri_hash = ?2, archived_at = ?3
                 WHERE id = ?4 AND protected = 0 AND archived_at IS NULL",
                    params![archived_uri, uri_hash, now, candidate.artifact_id],
                )
                .map_err(sqlite_error)?;
            if changed != 1 {
                return Err(format!(
                    "artifact {} became ineligible after preview",
                    candidate.artifact_id
                ));
            }
            transaction.execute(
                "INSERT INTO evidence_gc_ledger (artifact_id, run_id, artifact_kind, content_hash, uri_hash,
                 created_at, collected_at, extraction_receipt_id, logical_bytes_collected)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![candidate.artifact_id, run_id, kind, content_hash_value, uri_hash, created_at, now, receipt_id, candidate.logical_bytes],
            ).map_err(sqlite_error)?;
            append_event(
                &transaction,
                "evidence_artifact",
                candidate.artifact_id,
                "evidence_reference_archived",
                &json!({ "artifactKind": kind, "contentHash": content_hash_value, "uriHash": uri_hash, "logicalBytesCollected": candidate.logical_bytes }),
                now,
            )?;
            transaction.commit().map_err(sqlite_error)?;
            archived_artifact_ids.push(candidate.artifact_id);
            logical_bytes_collected += candidate.logical_bytes;
        }
        Ok(EvidenceCollectionResult {
            archived_artifact_ids,
            logical_bytes_collected,
        })
    }

    pub fn create_memory_policy(
        &self,
        input: &MemoryPolicyInput,
    ) -> Result<MemoryPolicyVersion, String> {
        let key = input.policy_key.trim();
        if key.is_empty() || key.len() > 100 || key.contains('\0') || input.version <= 0 {
            return Err("memory policy requires a valid key and positive version".to_string());
        }
        if input.retrieval_method != "sql_fts"
            || !(0..=10 * 1024 * 1024).contains(&input.hot_budget_bytes)
            || !(1..=50).contains(&input.warm_candidate_limit)
            || !(2..=100).contains(&input.minimum_promotion_support)
            || input
                .retention_minimum_age_seconds
                .is_some_and(|age| age < 0)
        {
            return Err(
                "memory policy contains unsupported retrieval or out-of-range limits".to_string(),
            );
        }
        let created_at = now();
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let latest_version: Option<i64> = transaction
            .query_row(
                "SELECT MAX(version) FROM memory_policies WHERE policy_key = ?1",
                params![key],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if latest_version.is_some_and(|latest| input.version <= latest) {
            return Err("memory policy versions must increase monotonically".to_string());
        }
        transaction.execute(
            "INSERT INTO memory_policies (policy_key, version, retrieval_method, hot_budget_bytes,
             warm_candidate_limit, minimum_promotion_support, retention_minimum_age_seconds, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![key, input.version, input.retrieval_method, input.hot_budget_bytes, input.warm_candidate_limit,
                input.minimum_promotion_support, input.retention_minimum_age_seconds, created_at],
        ).map_err(sqlite_error)?;
        let id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "memory_policy",
            id,
            "memory_policy_version_created",
            &json!({ "policyKey": key, "version": input.version, "retrievalMethod": input.retrieval_method }),
            created_at,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(MemoryPolicyVersion {
            id,
            policy_key: key.to_string(),
            version: input.version,
            retrieval_method: input.retrieval_method.clone(),
            hot_budget_bytes: input.hot_budget_bytes,
            warm_candidate_limit: input.warm_candidate_limit,
            minimum_promotion_support: input.minimum_promotion_support,
            retention_minimum_age_seconds: input.retention_minimum_age_seconds,
            created_at,
        })
    }

    pub fn record_memory_policy_evaluation(
        &self,
        input: &MemoryPolicyEvaluationInput,
    ) -> Result<i64, String> {
        if input.experiment_key.trim().is_empty()
            || input.experiment_key.len() > 120
            || input.experiment_key.contains('\0')
            || input.task_type.trim().is_empty()
            || input.task_type.len() > 120
            || input.task_type.contains('\0')
            || input.missing_context_failures < 0
            || input.regressions < 0
            || input.rework_count < 0
            || input.context_bytes < 0
            || input.reintroduced_bytes < 0
            || input.useful_retrievals < 0
            || input.retrieval_count < input.useful_retrievals
            || input.latency_ms.is_some_and(|value| value < 0)
            || input.human_interventions < 0
        {
            return Err(
                "memory policy evaluation contains invalid identifiers or metrics".to_string(),
            );
        }
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let status: String = transaction
            .query_row(
                "SELECT status FROM runs WHERE id = ?1 AND finished_at IS NOT NULL",
                params![input.run_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| format!("finished run {} was not found", input.run_id))?;
        if !matches!(
            status.as_str(),
            "verified_success" | "failed" | "inconclusive" | "blocked"
        ) {
            return Err(
                "only terminal Cathulu runs can be used in memory-policy evaluations".to_string(),
            );
        }
        let created_at = now();
        transaction.execute(
            "INSERT INTO memory_policy_evaluations (policy_id, experiment_key, task_type, run_id,
             missing_context_failures, regressions, rework_count, context_bytes, reintroduced_bytes,
             useful_retrievals, retrieval_count, latency_ms, human_interventions, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![input.policy_id, input.experiment_key.trim(), input.task_type.trim(), input.run_id,
                input.missing_context_failures, input.regressions, input.rework_count, input.context_bytes,
                input.reintroduced_bytes, input.useful_retrievals, input.retrieval_count, input.latency_ms,
                input.human_interventions, created_at],
        ).map_err(sqlite_error)?;
        let evaluation_id = transaction.last_insert_rowid();
        append_event(
            &transaction,
            "memory_policy_evaluation",
            evaluation_id,
            "memory_policy_evaluation_recorded",
            &json!({ "policyId": input.policy_id, "experimentKey": input.experiment_key, "taskType": input.task_type, "runId": input.run_id }),
            created_at,
        )?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(evaluation_id)
    }

    pub fn compare_memory_policy_experiment(
        &self,
        experiment_key: &str,
    ) -> Result<Vec<MemoryPolicyComparison>, String> {
        if experiment_key.trim().is_empty()
            || experiment_key.len() > 120
            || experiment_key.contains('\0')
        {
            return Err("experiment key must be 1 to 120 bytes and contain no NUL".to_string());
        }
        let connection = self.connection()?;
        let rows = connection.raw().prepare(
            "SELECT e.experiment_key, e.task_type, p.id, p.policy_key, p.version,
                    COUNT(*), SUM(CASE WHEN r.status = 'verified_success' THEN 1 ELSE 0 END),
                    SUM(e.missing_context_failures), SUM(e.regressions), AVG(e.context_bytes),
                    AVG(e.reintroduced_bytes),
                    CASE WHEN SUM(e.retrieval_count) = 0 THEN 0.0 ELSE CAST(SUM(e.useful_retrievals) AS REAL) / SUM(e.retrieval_count) END,
                    AVG(e.latency_ms), SUM(e.human_interventions)
             FROM memory_policy_evaluations e JOIN memory_policies p ON p.id = e.policy_id
             JOIN runs r ON r.id = e.run_id WHERE e.experiment_key = ?1
             GROUP BY e.experiment_key, e.task_type, p.id, p.policy_key, p.version
             ORDER BY e.task_type, p.policy_key, p.version"
        ).map_err(sqlite_error)?.query_map(params![experiment_key.trim()], |row| Ok(MemoryPolicyComparison {
            experiment_key: row.get(0)?, task_type: row.get(1)?, policy_id: row.get(2)?, policy_key: row.get(3)?, version: row.get(4)?,
            run_count: row.get(5)?, verified_successes: row.get(6)?, missing_context_failures: row.get(7)?, regressions: row.get(8)?,
            average_context_bytes: row.get(9)?, average_reintroduced_bytes: row.get(10)?, useful_retrieval_rate: row.get(11)?,
            average_latency_ms: row.get(12)?, human_interventions: row.get(13)?,
        })).map_err(sqlite_error)?.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?;
        Ok(rows)
    }

    pub fn capture_storage_snapshot(&self, run_id: Option<i64>) -> Result<StorageSnapshot, String> {
        let sampled_at = now();
        let sqlite_bytes = database_size(self.database_path())?;
        let mut connection = self.connection()?;
        let transaction = connection.raw_mut().transaction().map_err(sqlite_error)?;
        let (project_id, workflow_version_id): (Option<i64>, Option<i64>) = match run_id {
            Some(id) => transaction
                .query_row(
                    "SELECT project_id, workflow_version_id FROM runs WHERE id = ?1",
                    params![id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| format!("run {id} was not found"))?,
            None => (None, None),
        };
        if let Some(id) = run_id {
            if let Some(snapshot) = load_snapshot_by_run(&transaction, id)? {
                transaction.commit().map_err(sqlite_error)?;
                return Ok(snapshot);
            }
        }
        let previous_bytes: i64 = transaction.query_row(
            "SELECT COALESCE((SELECT sqlite_bytes FROM memory_storage_snapshots ORDER BY sampled_at DESC, id DESC LIMIT 1), ?1)",
            params![sqlite_bytes], |row| row.get(0),
        ).map_err(sqlite_error)?;
        let (hot_bytes, warm_bytes, cold_bytes): (i64, i64, i64) = transaction.query_row(
            "SELECT
               COALESCE(SUM(CASE WHEN temperature = 'hot' THEN length(CAST(body AS BLOB)) ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN temperature = 'warm' THEN length(CAST(body AS BLOB)) ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN temperature = 'cold' THEN length(CAST(body AS BLOB)) ELSE 0 END), 0)
             FROM memories", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).map_err(sqlite_error)?;
        let artifact_reference_bytes: i64 = transaction.query_row(
            "SELECT COALESCE(SUM(length(CAST(uri AS BLOB))), 0) FROM evidence_artifacts WHERE archived_at IS NULL",
            [], |row| row.get(0),
        ).map_err(sqlite_error)?;
        let orphaned_evidence_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM evidence_artifacts a WHERE a.archived_at IS NULL AND a.protected = 0
             AND NOT EXISTS (SELECT 1 FROM memory_artifact_refs r WHERE r.artifact_id = a.id)
             AND NOT EXISTS (SELECT 1 FROM memory_sources s WHERE s.source_type = 'artifact' AND s.source_id = CAST(a.id AS TEXT))",
            [], |row| row.get(0),
        ).map_err(sqlite_error)?;
        let duplicate_artifact_groups: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM (SELECT content_hash FROM evidence_artifacts WHERE archived_at IS NULL AND content_hash IS NOT NULL GROUP BY content_hash HAVING COUNT(*) > 1)",
            [], |row| row.get(0),
        ).map_err(sqlite_error)?;
        let bytes_collected: i64 = transaction
            .query_row(
                "SELECT COALESCE(SUM(logical_bytes_collected), 0) FROM evidence_gc_ledger",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        transaction.execute(
            "INSERT INTO memory_storage_snapshots (run_id, project_id, workflow_version_id, sampled_at, sqlite_bytes,
             growth_bytes, hot_bytes, warm_bytes, cold_bytes, artifact_reference_bytes, orphaned_evidence_count,
             duplicate_artifact_groups, bytes_collected)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![run_id, project_id, workflow_version_id, sampled_at, sqlite_bytes, sqlite_bytes - previous_bytes,
                hot_bytes, warm_bytes, cold_bytes, artifact_reference_bytes, orphaned_evidence_count,
                duplicate_artifact_groups, bytes_collected],
        ).map_err(sqlite_error)?;
        let snapshot = load_snapshot_by_id(&transaction, transaction.last_insert_rowid())?
            .ok_or_else(|| "storage snapshot disappeared".to_string())?;
        transaction.commit().map_err(sqlite_error)?;
        Ok(snapshot)
    }

    pub fn latest_storage_snapshot(&self) -> Result<Option<StorageSnapshot>, String> {
        let connection = self.connection()?;
        connection.raw().query_row(
            "SELECT id, run_id, project_id, workflow_version_id, sampled_at, sqlite_bytes, growth_bytes,
                    hot_bytes, warm_bytes, cold_bytes, artifact_reference_bytes, orphaned_evidence_count,
                    duplicate_artifact_groups, bytes_compacted, bytes_collected
             FROM memory_storage_snapshots ORDER BY sampled_at DESC, id DESC LIMIT 1",
            [], storage_snapshot_from_row,
        ).optional().map_err(sqlite_error)
    }

    fn has_valid_extraction(
        &self,
        artifact_id: i64,
        artifact_hash: Option<&str>,
        uri_hash: &str,
    ) -> Result<bool, String> {
        let connection = self.connection()?;
        let receipts = connection.raw().prepare(
            "SELECT artifact_content_hash, artifact_uri_hash, preserved_memories_json
             FROM evidence_extraction_receipts WHERE artifact_id = ?1 ORDER BY extracted_at DESC, id DESC"
        ).map_err(sqlite_error)?.query_map(params![artifact_id], |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
            .map_err(sqlite_error)?.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?;
        for (receipt_artifact_hash, receipt_uri_hash, memories_json) in receipts {
            if receipt_artifact_hash.as_deref() != artifact_hash || receipt_uri_hash != uri_hash {
                continue;
            }
            let preserved: Vec<Value> = serde_json::from_str(&memories_json)
                .map_err(|error| format!("stored extraction receipt is invalid: {error}"))?;
            let mut valid = !preserved.is_empty();
            for item in preserved {
                let Some(memory_id) = item.get("memoryId").and_then(Value::as_i64) else {
                    valid = false;
                    break;
                };
                let Some(memory_hash) = item.get("contentHash").and_then(Value::as_str) else {
                    valid = false;
                    break;
                };
                if durable_memory_content_hash(connection.raw(), memory_id, now())?.as_deref()
                    != Some(memory_hash)
                {
                    valid = false;
                    break;
                }
            }
            if valid {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn collect_proposals(
    connection: &rusqlite::Connection,
    now: i64,
    minimum_support: i64,
) -> Result<Vec<(String, Vec<i64>, String, Option<i64>)>, String> {
    let mut proposals = Vec::new();
    let mut statement = connection.prepare(
        "SELECT id FROM memories WHERE review_state = 'accepted' AND temperature = 'hot' AND active = 0
         AND superseded_by_id IS NULL AND invalidated_at IS NULL AND (valid_until IS NULL OR valid_until > ?1)
         ORDER BY importance ASC, COALESCE(last_used_at, 0), id"
    ).map_err(sqlite_error)?;
    let ids = statement
        .query_map(params![now], |row| row.get::<_, i64>(0))
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    for id in ids {
        proposals.push((
            "demote_inactive_hot".to_string(),
            vec![id],
            "Inactive accepted HOT memory can move to WARM without losing its record.".to_string(),
            None,
        ));
    }

    let mut statement = connection.prepare(
        "SELECT m.id FROM memories m LEFT JOIN projects p ON p.id = m.freshness_repo_id
         WHERE m.review_state = 'accepted' AND m.temperature <> 'cold'
           AND (m.invalidated_at IS NOT NULL OR (m.valid_until IS NOT NULL AND m.valid_until <= ?1)
                OR (m.freshness_repo_id IS NOT NULL AND m.freshness_git_commit IS NOT NULL AND p.git_head IS NOT NULL AND p.git_head <> m.freshness_git_commit))
         ORDER BY m.id"
    ).map_err(sqlite_error)?;
    let ids = statement
        .query_map(params![now], |row| row.get::<_, i64>(0))
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    for id in ids {
        proposals.push(("demote_stale_memory".to_string(), vec![id], "Memory freshness or validity no longer matches current project state; move it to COLD for historical retrieval.".to_string(), None));
    }

    let mut statement = connection.prepare(
        "SELECT m.id, m.reuse_count, COUNT(s.source_id) AS source_count
         FROM memories m LEFT JOIN memory_sources s ON s.memory_id = m.id
         WHERE m.memory_class = 'episodic' AND m.review_state = 'accepted' AND m.superseded_by_id IS NULL
           AND m.invalidated_at IS NULL AND (m.valid_until IS NULL OR m.valid_until > ?1)
         GROUP BY m.id HAVING m.reuse_count >= ?2 AND source_count >= ?2 ORDER BY m.id"
    ).map_err(sqlite_error)?;
    let ids = statement
        .query_map(params![now, minimum_support], |row| row.get::<_, i64>(0))
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    for id in ids {
        proposals.push(("promote_episodic".to_string(), vec![id], format!("Accepted episodic memory has reuse and provenance support of at least {minimum_support}; review promotion to canonical."), Some(minimum_support)));
    }

    let mut statement = connection
        .prepare(
            "SELECT scope_key, kind, content_hash FROM memories
         WHERE review_state = 'accepted' AND memory_class IN ('episodic', 'evidence')
         GROUP BY scope_key, kind, content_hash HAVING COUNT(*) > 1
         ORDER BY scope_key, kind, content_hash",
        )
        .map_err(sqlite_error)?;
    let duplicate_groups = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    for (scope_key, kind, hash) in duplicate_groups {
        let mut statement = connection.prepare(
            "SELECT id FROM memories WHERE scope_key = ?1 AND kind = ?2 AND content_hash = ?3
             AND review_state = 'accepted' AND memory_class IN ('episodic', 'evidence') ORDER BY id",
        ).map_err(sqlite_error)?;
        let ids = statement
            .query_map(params![scope_key, kind, hash], |row| row.get::<_, i64>(0))
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        proposals.push((
            "duplicate_memories".to_string(),
            ids,
            "Accepted episodic/evidence records repeat the same knowledge in this scope; consolidate into a canonical candidate while retaining all source IDs.".to_string(),
            None,
        ));
    }

    let mut statement = connection.prepare(
        "SELECT content_hash, COUNT(*) FROM evidence_artifacts WHERE archived_at IS NULL AND content_hash IS NOT NULL
         GROUP BY content_hash HAVING COUNT(*) > 1 ORDER BY MIN(id)"
    ).map_err(sqlite_error)?;
    let groups = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    for (hash, count) in groups {
        let mut ids = connection.prepare("SELECT id FROM evidence_artifacts WHERE archived_at IS NULL AND content_hash = ?1 ORDER BY id")
            .map_err(sqlite_error)?
            .query_map(params![hash], |row| row.get::<_, i64>(0))
            .map_err(sqlite_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error)?;
        proposals.push(("duplicate_artifacts".to_string(), std::mem::take(&mut ids), format!("{count} artifacts share a content hash; review whether redundant local references can be archived after extraction."), None));
    }

    let mut statement = connection.prepare(
        "SELECT a.id FROM evidence_artifacts a WHERE a.archived_at IS NULL AND a.protected = 0
         AND NOT EXISTS (SELECT 1 FROM memory_artifact_refs r WHERE r.artifact_id = a.id)
         AND NOT EXISTS (SELECT 1 FROM memory_sources s WHERE s.source_type = 'artifact' AND s.source_id = CAST(a.id AS TEXT))
         ORDER BY a.id"
    ).map_err(sqlite_error)?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite_error)?;
    for id in ids {
        proposals.push(("orphan_evidence".to_string(), vec![id], "Artifact has no durable memory reference; eligibility still depends on retention, extraction, QA, holds, and provenance checks.".to_string(), None));
    }
    let mut seen = std::collections::HashSet::new();
    let mut unresolved = Vec::with_capacity(proposals.len());
    for proposal in proposals {
        let subject_ids = serde_json::to_string(&proposal.1)
            .map_err(|error| format!("cannot encode maintenance proposal subjects: {error}"))?;
        let already_pending: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM memory_maintenance_proposals
             WHERE proposal_type = ?1 AND subject_ids_json = ?2 AND status = 'proposed')",
                params![proposal.0, subject_ids],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;
        if !already_pending && seen.insert((proposal.0.clone(), subject_ids)) {
            unresolved.push(proposal);
        }
    }
    Ok(unresolved)
}

fn load_proposal(
    transaction: &Transaction<'_>,
    id: i64,
) -> Result<Option<MaintenanceProposal>, String> {
    transaction
        .query_row(
            "SELECT id, maintenance_run_id, proposal_type, subject_ids_json, reason, minimum_support, status
         FROM memory_maintenance_proposals WHERE id = ?1",
            params![id],
            |row| {
                let subjects_json: String = row.get(3)?;
                let subject_ids = serde_json::from_str(&subjects_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(MaintenanceProposal {
                    id: row.get(0)?,
                    maintenance_run_id: row.get(1)?,
                    proposal_type: row.get(2)?,
                    subject_ids,
                    reason: row.get(4)?,
                    minimum_support: row.get(5)?,
                    status: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(sqlite_error)
}

fn apply_proposal(
    transaction: &Transaction<'_>,
    proposal: &MaintenanceProposal,
    now: i64,
) -> Result<(), String> {
    if !matches!(
        proposal.proposal_type.as_str(),
        "demote_inactive_hot" | "demote_stale_memory" | "promote_episodic"
    ) {
        return Ok(());
    }
    if proposal.subject_ids.len() != 1 {
        return Err("memory maintenance proposal must identify exactly one memory".to_string());
    }
    let id = proposal.subject_ids[0];
    let (class, temp, review, active, protected, reuse_count, source_count, stale): (String, String, String, bool, bool, i64, i64, bool) = transaction.query_row(
        "SELECT m.memory_class, m.temperature, m.review_state, m.active, m.protected, m.reuse_count, COUNT(s.source_id),
                (m.superseded_by_id IS NOT NULL OR m.invalidated_at IS NOT NULL
                 OR (m.valid_until IS NOT NULL AND m.valid_until <= ?2)
                 OR (m.freshness_repo_id IS NOT NULL AND m.freshness_git_commit IS NOT NULL
                     AND EXISTS (SELECT 1 FROM projects p WHERE p.id = m.freshness_repo_id
                                 AND p.git_head IS NOT NULL AND p.git_head <> m.freshness_git_commit)))
         FROM memories m LEFT JOIN memory_sources s ON s.memory_id = m.id WHERE m.id = ?1 GROUP BY m.id",
        params![id, now], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
    ).optional().map_err(sqlite_error)?.ok_or_else(|| format!("memory {id} no longer exists"))?;
    if review != "accepted" || protected {
        return Err(format!(
            "memory {id} is not eligible for automatic maintenance"
        ));
    }
    let (new_class, new_temp) = match proposal.proposal_type.as_str() {
        "demote_inactive_hot" if temp == "hot" && !active => (class.clone(), "warm"),
        "demote_stale_memory" if temp != "cold" && stale => (class.clone(), "cold"),
        "promote_episodic"
            if class == "episodic"
                && reuse_count >= proposal.minimum_support.unwrap_or(2)
                && source_count >= proposal.minimum_support.unwrap_or(2)
                && !stale =>
        {
            ("canonical".to_string(), "warm")
        }
        _ => {
            return Err(format!(
                "memory {id} no longer meets the proposal conditions"
            ))
        }
    };
    if new_class == "canonical" && source_count == 0 {
        return Err("canonical memory requires provenance".to_string());
    }
    transaction.execute("UPDATE memories SET memory_class = ?1, temperature = ?2, updated_at = ?3 WHERE id = ?4", params![new_class, new_temp, now, id]).map_err(sqlite_error)?;
    append_event(
        transaction,
        "memory",
        id,
        "memory_gardener_change",
        &json!({ "proposalId": proposal.id, "fromClass": class, "fromTemperature": temp, "toClass": new_class, "toTemperature": new_temp }),
        now,
    )?;
    Ok(())
}

fn has_legacy_uri_event(
    connection: &rusqlite::Connection,
    artifact_id: i64,
) -> Result<bool, String> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM events WHERE aggregate_type = 'evidence_artifact' AND aggregate_id = CAST(?1 AS TEXT)
         AND json_valid(payload_json) AND json_type(payload_json, '$.uri') IS NOT NULL)",
        params![artifact_id], |row| row.get(0),
    ).map_err(sqlite_error)
}

fn durable_memory_content_hash(
    connection: &rusqlite::Connection,
    memory_id: i64,
    now: i64,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT m.content_hash FROM memories m
             WHERE m.id = ?1 AND m.review_state = 'accepted' AND m.memory_class IN ('canonical', 'aggregate')
               AND m.superseded_by_id IS NULL AND m.invalidated_at IS NULL
               AND (m.valid_from IS NULL OR m.valid_from <= ?2)
               AND (m.valid_until IS NULL OR m.valid_until > ?2)
               AND NOT EXISTS (
                 SELECT 1 FROM projects p WHERE p.id = m.freshness_repo_id
                   AND m.freshness_git_commit IS NOT NULL AND p.git_head IS NOT NULL
                   AND p.git_head <> m.freshness_git_commit
               )",
            params![memory_id, now],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_error)
}

fn latest_valid_receipt_id(
    transaction: &Transaction<'_>,
    artifact_id: i64,
    artifact_hash: Option<&str>,
    uri_hash: &str,
) -> Result<Option<i64>, String> {
    let ids = transaction.prepare(
        "SELECT id, artifact_content_hash, artifact_uri_hash, preserved_memories_json FROM evidence_extraction_receipts
         WHERE artifact_id = ?1 ORDER BY extracted_at DESC, id DESC"
    ).map_err(sqlite_error)?.query_map(params![artifact_id], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)))
        .map_err(sqlite_error)?.collect::<Result<Vec<_>, _>>().map_err(sqlite_error)?;
    for (id, receipt_hash, receipt_uri_hash, memories_json) in ids {
        if receipt_hash.as_deref() != artifact_hash || receipt_uri_hash != uri_hash {
            continue;
        }
        let preserved: Vec<Value> = serde_json::from_str(&memories_json)
            .map_err(|error| format!("stored extraction receipt is invalid: {error}"))?;
        let mut valid = !preserved.is_empty();
        for item in preserved {
            let (Some(memory_id), Some(memory_hash)) = (
                item.get("memoryId").and_then(Value::as_i64),
                item.get("contentHash").and_then(Value::as_str),
            ) else {
                valid = false;
                break;
            };
            if durable_memory_content_hash(transaction, memory_id, now())?.as_deref()
                != Some(memory_hash)
            {
                valid = false;
                break;
            }
        }
        if valid {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn load_snapshot_by_run(
    transaction: &Transaction<'_>,
    run_id: i64,
) -> Result<Option<StorageSnapshot>, String> {
    transaction.query_row(
        "SELECT id, run_id, project_id, workflow_version_id, sampled_at, sqlite_bytes, growth_bytes,
                hot_bytes, warm_bytes, cold_bytes, artifact_reference_bytes, orphaned_evidence_count,
                duplicate_artifact_groups, bytes_compacted, bytes_collected
         FROM memory_storage_snapshots WHERE run_id = ?1", params![run_id], storage_snapshot_from_row,
    ).optional().map_err(sqlite_error)
}

fn load_snapshot_by_id(
    transaction: &Transaction<'_>,
    id: i64,
) -> Result<Option<StorageSnapshot>, String> {
    transaction.query_row(
        "SELECT id, run_id, project_id, workflow_version_id, sampled_at, sqlite_bytes, growth_bytes,
                hot_bytes, warm_bytes, cold_bytes, artifact_reference_bytes, orphaned_evidence_count,
                duplicate_artifact_groups, bytes_compacted, bytes_collected
         FROM memory_storage_snapshots WHERE id = ?1", params![id], storage_snapshot_from_row,
    ).optional().map_err(sqlite_error)
}

fn storage_snapshot_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StorageSnapshot> {
    Ok(StorageSnapshot {
        id: row.get(0)?,
        run_id: row.get(1)?,
        project_id: row.get(2)?,
        workflow_version_id: row.get(3)?,
        sampled_at: row.get(4)?,
        sqlite_bytes: row.get(5)?,
        growth_bytes: row.get(6)?,
        hot_bytes: row.get(7)?,
        warm_bytes: row.get(8)?,
        cold_bytes: row.get(9)?,
        artifact_reference_bytes: row.get(10)?,
        orphaned_evidence_count: row.get(11)?,
        duplicate_artifact_groups: row.get(12)?,
        bytes_compacted: row.get(13)?,
        bytes_collected: row.get(14)?,
    })
}

fn database_size(database_path: &std::path::Path) -> Result<i64, String> {
    let mut total = 0_u64;
    for path in [
        database_path.to_path_buf(),
        PathBuf::from(format!("{}-wal", database_path.display())),
    ] {
        match std::fs::metadata(&path) {
            Ok(metadata) => total = total.saturating_add(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot measure local SQLite storage: {error}")),
        }
    }
    i64::try_from(total).map_err(|_| "SQLite storage size exceeds supported range".to_string())
}

fn content_hash_fn(value: &str) -> String {
    content_hash(value)
}
fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
