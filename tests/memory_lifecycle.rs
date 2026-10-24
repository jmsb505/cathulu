use cathulu_core::{
    gardener::{MemoryPolicyEvaluationInput, MemoryPolicyInput},
    memory::{FreshnessBasis, MemoryCreateInput, MemorySource},
    Store,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    root: PathBuf,
    store: Store,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cathulu-memory-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create test directory");
        let store = Store::open(root.join("data")).expect("open test store");
        Self { root, store }
    }

    fn project(&self) -> i64 {
        let path = self.root.join("repo");
        fs::create_dir_all(&path).expect("create repository");
        git(&path, &["init", "--quiet", "-b", "main"]);
        git(&path, &["config", "user.name", "Cathulu Test"]);
        git(
            &path,
            &["config", "user.email", "cathulu-test@example.invalid"],
        );
        fs::write(path.join("README.md"), "memory test\n").expect("write repository file");
        git(&path, &["add", "README.md"]);
        git(&path, &["commit", "--quiet", "-m", "initial"]);
        self.store
            .register_project(&path)
            .expect("register repository")
            .id
    }

    fn verified_run(&self, project_id: i64) -> i64 {
        let project_scope_id = project_id.to_string();
        let has_profile = self
            .store
            .list_memories("project", Some(&project_scope_id), false, false)
            .expect("read project memory before run")
            .iter()
            .any(|memory| memory.memory_class == "canonical" && memory.kind == "summary");
        let workflow = self
            .store
            .create_workflow_version(
                "memory_test",
                "Memory test",
                1,
                &json!({ "mandatory_checks": ["test"] }),
            )
            .expect("create test workflow");
        let run = self
            .store
            .start_run(project_id, None, None, Some(workflow.id))
            .expect("start run");
        self.store
            .record_quality_check(run.id, "test", true, "passed", &json!({}))
            .expect("record passing check");
        assert!(self
            .store
            .finish_run(run.id, "A run cannot close without memory review.")
            .is_err());
        let (disposition, reason_code, memory_ids) = if has_profile {
            (
                "no_durable_change",
                "existing_knowledge_remains_current",
                vec![],
            )
        } else {
            assert!(self
                .store
                .record_project_memory_review(
                    run.id,
                    "no_durable_change",
                    "task_produced_no_reusable_knowledge",
                    &[],
                )
                .is_err());
            let profile = self.project_profile(project_id);
            (
                "knowledge_saved",
                "project_profile_initialized",
                vec![profile],
            )
        };
        self.store
            .record_project_memory_review(run.id, disposition, reason_code, &memory_ids)
            .expect("record project memory review");
        self.store
            .record_project_memory_review(run.id, disposition, reason_code, &memory_ids)
            .expect("exact review retry is idempotent");
        let conflicting = if disposition == "no_durable_change" {
            self.store.record_project_memory_review(
                run.id,
                "knowledge_saved",
                "durable_knowledge_updated",
                &[],
            )
        } else {
            self.store.record_project_memory_review(
                run.id,
                "no_durable_change",
                "task_produced_no_reusable_knowledge",
                &[],
            )
        };
        assert!(conflicting.is_err());
        let finished = self
            .store
            .finish_run(run.id, "Memory lifecycle fixture passed.")
            .expect("finish run");
        assert_eq!(finished.status, "verified_success");
        let review_event = self
            .store
            .list_events("run", &run.id.to_string())
            .expect("read run events")
            .into_iter()
            .find(|event| event.event_type == "project_memory_reviewed")
            .expect("project memory review receipt");
        assert_eq!(review_event.payload.as_object().unwrap().len(), 4);
        assert!(review_event.payload.get("body").is_none());
        assert!(review_event.payload.get("transcript").is_none());
        self.store
            .record_project_memory_review(run.id, disposition, reason_code, &memory_ids)
            .expect("exact retry remains idempotent after run closeout");
        finished.id
    }

    fn project_profile(&self, project_id: i64) -> i64 {
        let memory = self
            .store
            .create_memory(&MemoryCreateInput {
                scope_type: "project".to_string(),
                scope_id: Some(project_id.to_string()),
                memory_class: "canonical".to_string(),
                temperature: "hot".to_string(),
                kind: "summary".to_string(),
                body: "This test repository exercises Cathulu's local memory lifecycle. Its README identifies the fixture; the Rust integration tests define the behavior under test.".to_string(),
                sources: vec![
                    MemorySource {
                        source_type: "project".to_string(),
                        source_id: project_id.to_string(),
                    },
                    MemorySource {
                        source_type: "source_file".to_string(),
                        source_id: "README.md".to_string(),
                    },
                ],
                artifact_ids: Vec::new(),
                valid_from: None,
                valid_until: None,
                supersedes_id: None,
                freshness_basis: FreshnessBasis::default(),
                importance: 0.9,
                confidence: 0.95,
                active: true,
                protected: false,
            })
            .expect("create canonical project profile");
        self.store
            .review_memory_candidate(memory.id, "accepted")
            .expect("accept source-backed project profile")
            .id
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn source(source_id: &str) -> MemorySource {
    MemorySource {
        source_type: "human_instruction".to_string(),
        source_id: source_id.to_string(),
    }
}

fn memory(
    memory_class: &str,
    temperature: &str,
    body: &str,
    sources: Vec<MemorySource>,
    artifact_ids: Vec<i64>,
    active: bool,
) -> MemoryCreateInput {
    MemoryCreateInput {
        scope_type: "scope".to_string(),
        scope_id: Some("phase2".to_string()),
        memory_class: memory_class.to_string(),
        temperature: temperature.to_string(),
        kind: "lesson".to_string(),
        body: body.to_string(),
        sources,
        artifact_ids,
        valid_from: None,
        valid_until: None,
        supersedes_id: None,
        freshness_basis: FreshnessBasis::default(),
        importance: 0.8,
        confidence: 0.9,
        active,
        protected: false,
    }
}

#[test]
fn gardener_records_proposals_then_applies_reviewed_changes_without_changing_identity() {
    let fixture = Fixture::new("gardener");
    let hot = fixture
        .store
        .create_memory(&memory(
            "canonical",
            "hot",
            "Inactive architecture summary remains durable.",
            vec![source("decision-1")],
            vec![],
            false,
        ))
        .expect("create HOT memory");
    fixture
        .store
        .review_memory_candidate(hot.id, "accepted")
        .expect("accept HOT memory");

    let episodic = fixture
        .store
        .create_memory(&memory(
            "episodic",
            "warm",
            "A recurring timeout occurs when the adapter retries indefinitely.",
            vec![source("run-11"), source("run-19")],
            vec![],
            true,
        ))
        .expect("create episodic memory");
    fixture
        .store
        .review_memory_candidate(episodic.id, "accepted")
        .expect("accept episodic memory");
    for receiver in ["worker-a", "worker-b"] {
        fixture
            .store
            .prepare_context_transfer(
                episodic.id,
                "scope",
                Some("main"),
                "scope",
                Some(receiver),
                None,
                "fts",
                None,
            )
            .expect("record use");
    }

    let report = fixture
        .store
        .run_memory_gardener(2)
        .expect("run deterministic gardener");
    let repeated = fixture
        .store
        .run_memory_gardener(2)
        .expect("repeat maintenance without duplicating pending proposals");
    assert_eq!(repeated.proposal_count, 0);
    let hot_proposal = report
        .proposals
        .iter()
        .find(|proposal| proposal.proposal_type == "demote_inactive_hot")
        .expect("propose HOT demotion");
    let promotion = report
        .proposals
        .iter()
        .find(|proposal| proposal.proposal_type == "promote_episodic")
        .expect("propose supported promotion");
    assert_eq!(hot_proposal.status, "proposed");
    assert_eq!(promotion.minimum_support, Some(2));
    assert_eq!(
        fixture
            .store
            .get_memory(hot.id)
            .unwrap()
            .unwrap()
            .temperature,
        "hot",
        "maintenance only proposes changes until reviewed"
    );

    fixture
        .store
        .review_memory_maintenance_proposal(hot_proposal.id, "accepted")
        .expect("accept HOT demotion");
    fixture
        .store
        .review_memory_maintenance_proposal(promotion.id, "accepted")
        .expect("accept supported promotion");
    let hot_after = fixture.store.get_memory(hot.id).unwrap().unwrap();
    let episodic_after = fixture.store.get_memory(episodic.id).unwrap().unwrap();
    assert_eq!(hot_after.id, hot.id);
    assert_eq!(hot_after.temperature, "warm");
    assert_eq!(hot_after.memory_class, "canonical");
    assert_eq!(episodic_after.id, episodic.id);
    assert_eq!(episodic_after.temperature, "warm");
    assert_eq!(episodic_after.memory_class, "canonical");
    assert_eq!(episodic_after.sources.len(), 2);
    assert!(fixture
        .store
        .review_memory_maintenance_proposal(promotion.id, "accepted")
        .is_err());
}

#[test]
fn gardener_groups_duplicate_episodes_without_replacing_their_source_records() {
    let fixture = Fixture::new("duplicate-memory-groups");
    let body = "The adapter retries must stop after the schema mismatch.";
    let mut historical_input = memory(
        "episodic",
        "warm",
        body,
        vec![source("run-12")],
        vec![],
        false,
    );
    historical_input.valid_until = Some(1);
    let historical = fixture
        .store
        .create_memory(&historical_input)
        .expect("create historical episode");
    fixture
        .store
        .review_memory_candidate(historical.id, "accepted")
        .expect("accept historical episode");
    let current = fixture
        .store
        .create_memory(&memory(
            "episodic",
            "warm",
            body,
            vec![source("run-31")],
            vec![],
            false,
        ))
        .expect("create repeated episode");
    fixture
        .store
        .review_memory_candidate(current.id, "accepted")
        .expect("accept repeated episode");

    let report = fixture
        .store
        .run_memory_gardener(2)
        .expect("detect duplicate episodes");
    let duplicate = report
        .proposals
        .iter()
        .find(|proposal| proposal.proposal_type == "duplicate_memories")
        .expect("propose duplicate memory consolidation");
    assert_eq!(duplicate.subject_ids, vec![historical.id, current.id]);
    assert_eq!(report.findings["duplicateMemoryGroups"], 1);
    assert_eq!(
        fixture.store.get_memory(historical.id).unwrap().unwrap().id,
        historical.id,
        "duplicate detection preserves the historical source record"
    );
    assert_eq!(
        fixture
            .store
            .get_memory(current.id)
            .unwrap()
            .unwrap()
            .sources
            .len(),
        1,
        "duplicate detection preserves the newer source provenance"
    );
}

#[test]
fn evidence_collection_is_disabled_by_default_and_archives_only_unreferenced_extracted_metadata() {
    let fixture = Fixture::new("gc");
    let file_to_preserve = fixture.root.join("kept.log");
    fs::write(&file_to_preserve, "artifact data remains on disk\n")
        .expect("write local evidence file");
    let file_uri = file_to_preserve.to_string_lossy().to_string();
    let collectable = fixture
        .store
        .record_evidence_artifact(None, "test_log", &file_uri, Some("sha256:fixture"), false)
        .expect("record local artifact");
    let protected = fixture
        .store
        .record_evidence_artifact(
            None,
            "incident",
            "artifact://local/protected",
            Some("sha256:protected"),
            true,
        )
        .expect("record protected artifact");
    let referenced = fixture
        .store
        .record_evidence_artifact(
            None,
            "contract",
            "artifact://local/referenced",
            Some("sha256:referenced"),
            false,
        )
        .expect("record referenced artifact");
    let held = fixture
        .store
        .record_evidence_artifact(
            None,
            "qa",
            "artifact://local/held",
            Some("sha256:held"),
            false,
        )
        .expect("record held artifact");
    let missing_receipt = fixture
        .store
        .record_evidence_artifact(
            None,
            "trace",
            "artifact://local/no-extraction",
            Some("sha256:missing"),
            false,
        )
        .expect("record artifact without receipt");
    let project_id = fixture.project();
    let active_run = fixture
        .store
        .start_run(project_id, None, None, None)
        .expect("start QA investigation run");
    let under_investigation = fixture
        .store
        .record_evidence_artifact(
            Some(active_run.id),
            "qa_trace",
            "artifact://local/active-investigation",
            Some("sha256:active"),
            false,
        )
        .expect("record evidence from active investigation");
    let legacy_event = fixture
        .store
        .record_evidence_artifact(
            None,
            "legacy_log",
            "artifact://local/legacy-event-uri",
            Some("sha256:legacy"),
            false,
        )
        .expect("record artifact for legacy event check");
    let raw =
        rusqlite::Connection::open(fixture.store.database_path()).expect("open test database");
    raw.execute(
        "INSERT INTO events (aggregate_type, aggregate_id, event_type, payload_json, created_at)
         VALUES ('evidence_artifact', ?1, 'legacy_artifact_recorded', ?2, 1)",
        rusqlite::params![
            legacy_event.id.to_string(),
            json!({"uri":"artifact://local/legacy-event-uri"}).to_string()
        ],
    )
    .expect("insert immutable legacy event fixture");
    drop(raw);

    let durable = fixture
        .store
        .create_memory(&memory(
            "canonical",
            "warm",
            "Contract evidence remains traceable by artifact identity.",
            vec![MemorySource {
                source_type: "artifact".to_string(),
                source_id: referenced.id.to_string(),
            }],
            vec![referenced.id],
            false,
        ))
        .expect("create referenced canonical memory");
    fixture
        .store
        .review_memory_candidate(durable.id, "accepted")
        .expect("accept referenced canonical memory");
    let held_id = fixture
        .store
        .hold_evidence(held.id, "Investigating QA failure")
        .expect("hold artifact");
    fixture
        .store
        .record_evidence_extraction(collectable.id, &[durable.id])
        .expect("record extraction receipt");
    fixture
        .store
        .record_evidence_extraction(under_investigation.id, &[durable.id])
        .expect("record durable extraction for QA evidence");
    fixture
        .store
        .record_evidence_extraction(legacy_event.id, &[durable.id])
        .expect("record extraction for legacy artifact");

    let default_preview = fixture
        .store
        .preview_evidence_collection()
        .expect("preview default policy");
    assert!(default_preview.iter().all(|candidate| !candidate.eligible));
    assert!(default_preview
        .iter()
        .find(|candidate| candidate.artifact_id == collectable.id)
        .unwrap()
        .reasons
        .contains(&"retention_policy_disabled".to_string()));

    fixture
        .store
        .set_evidence_retention_policy(Some(0), true)
        .expect("configure explicit retention");
    let preview = fixture
        .store
        .preview_evidence_collection()
        .expect("preview retention");
    assert!(
        preview
            .iter()
            .find(|candidate| candidate.artifact_id == collectable.id)
            .unwrap()
            .eligible
    );
    assert!(preview
        .iter()
        .find(|candidate| candidate.artifact_id == protected.id)
        .unwrap()
        .reasons
        .contains(&"protected".to_string()));
    assert!(preview
        .iter()
        .find(|candidate| candidate.artifact_id == referenced.id)
        .unwrap()
        .reasons
        .contains(&"memory_provenance_reference".to_string()));
    assert!(preview
        .iter()
        .find(|candidate| candidate.artifact_id == held.id)
        .unwrap()
        .reasons
        .contains(&"active_investigation_hold".to_string()));
    assert!(preview
        .iter()
        .find(|candidate| candidate.artifact_id == missing_receipt.id)
        .unwrap()
        .reasons
        .contains(&"verified_extraction_missing".to_string()));
    assert!(preview
        .iter()
        .find(|candidate| candidate.artifact_id == under_investigation.id)
        .unwrap()
        .reasons
        .contains(&"run_not_verified_success".to_string()));
    assert!(preview
        .iter()
        .find(|candidate| candidate.artifact_id == legacy_event.id)
        .unwrap()
        .reasons
        .contains(&"immutable_event_contains_uri".to_string()));

    assert!(fixture.store.release_evidence_hold(held_id).unwrap());
    let result = fixture
        .store
        .collect_eligible_evidence()
        .expect("archive eligible local reference");
    assert_eq!(result.archived_artifact_ids, vec![collectable.id]);
    assert!(
        file_to_preserve.exists(),
        "Cathulu archives the reference, never the file"
    );
    let archived = fixture.store.get_artifact(collectable.id).unwrap().unwrap();
    assert_eq!(archived.id, collectable.id);
    assert!(archived.archived_at.is_some());
    assert_eq!(archived.content_hash.as_deref(), Some("sha256:fixture"));
    assert!(archived.uri.starts_with("cathulu-archived://artifact/"));
    let snapshot = fixture
        .store
        .capture_storage_snapshot(None)
        .expect("capture storage health");
    assert!(snapshot.sqlite_bytes > 0);
    assert!(snapshot.bytes_collected >= result.logical_bytes_collected);
}

#[test]
fn policy_versions_are_immutable_and_experiment_metrics_are_compared_against_verified_runs() {
    let fixture = Fixture::new("policy");
    let project_id = fixture.project();
    let run_v1 = fixture.verified_run(project_id);
    let run_v2 = fixture.verified_run(project_id);
    let policy_v1 = fixture
        .store
        .create_memory_policy(&MemoryPolicyInput {
            policy_key: "memory_policy".to_string(),
            version: 1,
            retrieval_method: "sql_fts".to_string(),
            hot_budget_bytes: 8 * 1024,
            warm_candidate_limit: 10,
            minimum_promotion_support: 2,
            retention_minimum_age_seconds: None,
        })
        .expect("create policy version 1");
    let policy_v2 = fixture
        .store
        .create_memory_policy(&MemoryPolicyInput {
            policy_key: "memory_policy".to_string(),
            version: 2,
            retrieval_method: "sql_fts".to_string(),
            hot_budget_bytes: 4 * 1024,
            warm_candidate_limit: 8,
            minimum_promotion_support: 3,
            retention_minimum_age_seconds: Some(86_400),
        })
        .expect("create policy version 2");
    assert!(fixture
        .store
        .create_memory_policy(&MemoryPolicyInput {
            policy_key: "memory_policy".to_string(),
            version: 1,
            retrieval_method: "sql_fts".to_string(),
            hot_budget_bytes: 8 * 1024,
            warm_candidate_limit: 10,
            minimum_promotion_support: 2,
            retention_minimum_age_seconds: None,
        })
        .is_err());
    for (policy_id, run_id, context_bytes) in
        [(policy_v1.id, run_v1, 1_000), (policy_v2.id, run_v2, 500)]
    {
        fixture
            .store
            .record_memory_policy_evaluation(&MemoryPolicyEvaluationInput {
                policy_id,
                experiment_key: "same-checkpoint".to_string(),
                task_type: "windows-build".to_string(),
                run_id,
                missing_context_failures: 0,
                regressions: 0,
                rework_count: 1,
                context_bytes,
                reintroduced_bytes: 64,
                useful_retrievals: 3,
                retrieval_count: 4,
                latency_ms: Some(120),
                human_interventions: 1,
            })
            .expect("record policy result");
    }
    let comparison = fixture
        .store
        .compare_memory_policy_experiment("same-checkpoint")
        .expect("compare policies");
    assert_eq!(comparison.len(), 2);
    assert!(comparison
        .iter()
        .all(|result| result.run_count == 1 && result.verified_successes == 1));
    assert!(comparison
        .iter()
        .any(|result| result.average_context_bytes == 1_000.0));
    assert!(comparison
        .iter()
        .any(|result| result.average_context_bytes == 500.0));
    assert!(fixture
        .store
        .record_memory_policy_evaluation(&MemoryPolicyEvaluationInput {
            policy_id: policy_v1.id,
            experiment_key: "same-checkpoint".to_string(),
            task_type: "windows-build".to_string(),
            run_id: run_v1,
            missing_context_failures: 0,
            regressions: 0,
            rework_count: 0,
            context_bytes: 1,
            reintroduced_bytes: 0,
            useful_retrievals: 0,
            retrieval_count: 0,
            latency_ms: None,
            human_interventions: 0,
        })
        .is_err());
    let snapshot = fixture
        .store
        .latest_storage_snapshot()
        .unwrap()
        .expect("finish_run snapshots storage");
    assert_eq!(snapshot.run_id, Some(run_v2));
    assert_eq!(snapshot.project_id, Some(project_id));
    assert!(snapshot.sqlite_bytes > 0);
}
