use cathulu_core::{
    delegation::DelegationTaskPacket,
    memory::{FreshnessBasis, MemoryCreateInput, MemorySource},
    optimization::{
        parse_decision_result, CapabilitySettings, DecisionModel, DecisionModelFuture,
        DecisionRequest, DecisionResult, InvocationStatus, ModelCallFailure, ModelResponse,
        OptimizationCapability, OptimizationConfig, OptimizationMode, OptimizationShadowReview,
        ProviderLocality, RoutingDecisionSchema, ShadowOutcomeAssessment,
    },
    Store,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    store: Store,
    project_id: i64,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cathulu-optimization-m16-{label}-{}-{nonce}",
            std::process::id()
        ));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).expect("create test repository");
        git(&repo, &["init", "--quiet", "-b", "main"]);
        git(&repo, &["config", "user.name", "Cathulu Test"]);
        git(
            &repo,
            &["config", "user.email", "cathulu-test@example.invalid"],
        );
        fs::write(repo.join("README.md"), "M16 fixture\n").expect("write fixture file");
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "--quiet", "-m", "fixture"]);
        let store = Store::open(root.join("data")).expect("open local store");
        let project_id = store.register_project(&repo).expect("register project").id;
        let profile = store
            .create_memory(&MemoryCreateInput {
                scope_type: "project".to_string(),
                scope_id: Some(project_id.to_string()),
                memory_class: "canonical".to_string(),
                temperature: "hot".to_string(),
                kind: "summary".to_string(),
                body: "This local repository is a fixture for DecisionModel shadow workflow tests."
                    .to_string(),
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
            .expect("create project summary");
        store
            .review_memory_candidate(profile.id, "accepted")
            .expect("accept project summary");
        Self {
            root,
            store,
            project_id,
        }
    }

    fn workflow(&self, definition: Value) -> i64 {
        self.store
            .create_workflow_version("m16_test", "M16 test", 1, &definition)
            .expect("create workflow version")
            .id
    }

    fn run(
        &self,
        config: &OptimizationConfig,
        task_id: Option<i64>,
        objective_id: Option<i64>,
    ) -> i64 {
        self.store
            .start_run_with_optimization(self.project_id, objective_id, task_id, None, config)
            .expect("start run")
            .id
    }

    fn review_memory(&self, run_id: i64) {
        self.store
            .record_project_memory_review(
                run_id,
                "no_durable_change",
                "task_produced_no_reusable_knowledge",
                &[],
            )
            .expect("review project memory for run");
    }

    fn objective_and_task(&self) -> (i64, i64) {
        let objective = self
            .store
            .create_objective(
                self.project_id,
                "OBJECTIVE_PRIVATE_MARKER",
                "OBJECTIVE_DESCRIPTION_PRIVATE_MARKER",
            )
            .expect("create objective");
        let task = self
            .store
            .create_task(
                objective.id,
                "TASK_TITLE_PRIVATE_MARKER",
                "TASK_DESCRIPTION_PRIVATE_MARKER",
            )
            .expect("create task");
        self.store
            .set_task_status(task.id, "active")
            .expect("activate task");
        (objective.id, task.id)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone)]
enum ModelReply {
    Result(Option<DecisionResult>),
    Error(ModelCallFailure),
}

struct TestDecisionModel {
    calls: AtomicUsize,
    reply: ModelReply,
}

impl TestDecisionModel {
    fn result(result: DecisionResult) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            reply: ModelReply::Result(Some(result)),
        }
    }

    fn error(error: ModelCallFailure) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            reply: ModelReply::Error(error),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl DecisionModel for TestDecisionModel {
    fn decide<'a>(&'a self, _request: &'a DecisionRequest) -> DecisionModelFuture<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let reply = self.reply.clone();
        Box::pin(async move {
            match reply {
                ModelReply::Result(result) => Ok(result.map(ModelResponse::new)),
                ModelReply::Error(error) => Err(error),
            }
        })
    }
}

fn local_shadow_config(capability: OptimizationCapability) -> OptimizationConfig {
    let mut config = OptimizationConfig::default();
    config.capabilities.insert(
        capability,
        CapabilitySettings {
            enabled: true,
            mode: OptimizationMode::Shadow,
            provider: Some("local_test_model".to_string()),
            model: Some("fixture-v1".to_string()),
            provider_locality: Some(ProviderLocality::Local),
            remote_data_allowed: false,
        },
    );
    config
}

fn decision_result(request: &DecisionRequest, outcome: &str, confidence: f64) -> DecisionResult {
    let mut probabilities = BTreeMap::new();
    for allowed in &request.allowed_outcomes {
        probabilities.insert(allowed.clone(), if allowed == outcome { 1.0 } else { 0.0 });
    }
    DecisionResult {
        schema_version: request.schema_version,
        outcome: outcome.to_string(),
        model_confidence: Some(confidence),
        probabilities,
    }
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("run git command");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn routing_schemas_have_fixed_outcomes_and_explicit_escape_choices() {
    let fixture = Fixture::new("schemas");
    let run_id = fixture.run(&OptimizationConfig::default(), None, None);
    let state = fixture.store.routing_decision_state(run_id).unwrap();

    for schema in [
        RoutingDecisionSchema::WorkflowClass,
        RoutingDecisionSchema::DelegationStrategy,
    ] {
        let request = schema.request(&state).expect("bounded routing request");
        request.validate().expect("valid schema request");
        for escape in [
            "unknown",
            "insufficient_context",
            "needs_codex",
            "needs_more_evidence",
            "not_applicable",
        ] {
            assert!(request
                .allowed_outcomes
                .iter()
                .any(|outcome| outcome == escape));
        }
    }

    let mut duplicate = RoutingDecisionSchema::WorkflowClass
        .request(&state)
        .unwrap();
    duplicate.allowed_outcomes.push("bug_fix".to_string());
    assert!(duplicate.validate().is_err());
}

#[test]
fn routing_state_is_derived_from_sqlite_and_excludes_free_form_or_repository_content() {
    let fixture = Fixture::new("state-projection");
    let (objective_id, task_id) = fixture.objective_and_task();
    let repo = fixture.root.join("repo");
    fs::write(
        repo.join("secret-file-name.txt"),
        "FILE_CONTENT_PRIVATE_MARKER",
    )
    .expect("make repository dirty");
    fixture
        .store
        .reconcile_project(fixture.project_id)
        .expect("reconcile dirty project");
    let workflow_id = fixture.workflow(json!({ "mandatory_checks": ["compile", "test"] }));
    let run_id = fixture
        .store
        .start_run_with_optimization(
            fixture.project_id,
            Some(objective_id),
            Some(task_id),
            Some(workflow_id),
            &OptimizationConfig::default(),
        )
        .expect("start run with workflow version")
        .id;
    fixture
        .store
        .record_quality_check(
            run_id,
            "compile",
            true,
            "failed",
            &json!({"detail":"QA_PRIVATE_MARKER"}),
        )
        .expect("record failed mandatory check");
    fixture
        .store
        .record_quality_check(run_id, "test", true, "pending", &json!({}))
        .expect("record pending mandatory check");
    fixture
        .store
        .record_quality_check(run_id, "format", false, "failed", &json!({}))
        .expect("record non-mandatory check");

    let packet = DelegationTaskPacket {
        schema_version: 1,
        title: "Bounded worker fixture".to_string(),
        objective: "Validate local delegation counts".to_string(),
        acceptance_criteria: vec!["Record a prepared worker".to_string()],
        constraints: Vec::new(),
        allowed_scope: vec!["src/**".to_string()],
        referenced_memory_ids: Vec::new(),
        referenced_artifact_ids: Vec::new(),
        requested_report_fields: vec!["summary".to_string()],
    };
    fixture
        .store
        .prepare_delegation(
            fixture.project_id,
            task_id,
            run_id,
            "worker",
            None,
            &packet,
            None,
            "m16-worker-idempotency",
            None,
            None,
            None,
        )
        .expect("prepare worker delegation");

    let state = fixture.store.routing_decision_state(run_id).unwrap();
    assert!(state.task_record_present);
    assert_eq!(state.task_status.as_deref(), Some("active"));
    assert!(state.objective_record_present);
    assert!(state.workflow_version_present);
    assert!(state.project_dirty);
    assert_eq!(state.changed_path_count, 1);
    assert_eq!(state.mandatory_check_count, 2);
    assert_eq!(state.mandatory_failed_count, 1);
    assert_eq!(state.mandatory_pending_count, 1);
    assert_eq!(state.active_worker_count, 1);
    assert_eq!(state.active_sub_overseer_count, 0);

    let encoded = serde_json::to_string(&state).unwrap();
    for private_value in [
        "TASK_TITLE_PRIVATE_MARKER",
        "TASK_DESCRIPTION_PRIVATE_MARKER",
        "OBJECTIVE_PRIVATE_MARKER",
        "OBJECTIVE_DESCRIPTION_PRIVATE_MARKER",
        "secret-file-name.txt",
        "FILE_CONTENT_PRIVATE_MARKER",
        "QA_PRIVATE_MARKER",
        fixture.root.to_string_lossy().as_ref(),
    ] {
        assert!(
            !encoded.contains(private_value),
            "state leaked {private_value}"
        );
    }
}

#[test]
fn decision_response_parser_is_bounded_strict_and_normalizes_probabilities() {
    let fixture = Fixture::new("parse-response");
    let run_id = fixture.run(&OptimizationConfig::default(), None, None);
    let state = fixture.store.routing_decision_state(run_id).unwrap();
    let request = RoutingDecisionSchema::WorkflowClass
        .request(&state)
        .unwrap();
    let mut response = decision_result(&request, "bug_fix", 0.95);
    response.probabilities.insert("bug_fix".to_string(), 0.5);
    response.probabilities.insert("feature".to_string(), 0.5);
    response
        .probabilities
        .values_mut()
        .for_each(|value| *value *= 1.01);
    let bytes = serde_json::to_vec(&response).unwrap();
    let parsed = parse_decision_result(&bytes, &request).expect("parse and normalize response");
    let total: f64 = parsed.probabilities.values().sum();
    assert!((total - 1.0).abs() < 1e-9);

    let mut out_of_schema = response.clone();
    out_of_schema.outcome = "run_tests_and_mark_success".to_string();
    assert!(out_of_schema.validate_for(&request).is_err());

    let mut inconsistent_choice = decision_result(&request, "bug_fix", 0.8);
    inconsistent_choice
        .probabilities
        .insert("bug_fix".to_string(), 0.1);
    inconsistent_choice
        .probabilities
        .insert("feature".to_string(), 0.9);
    assert!(inconsistent_choice.validate_for(&request).is_err());

    let with_unknown = br#"{"schemaVersion":1,"outcome":"bug_fix","unexpectedAction":"execute"}"#;
    assert!(parse_decision_result(with_unknown, &request).is_err());
    assert!(parse_decision_result(&vec![b' '; 16 * 1024 + 1], &request).is_err());
}

#[tokio::test]
async fn shadow_prediction_is_recorded_without_changing_run_or_delegation_state() {
    let fixture = Fixture::new("shadow-no-influence");
    let config = local_shadow_config(OptimizationCapability::WorkflowRouting);
    let run_id = fixture.run(&config, None, None);
    let state = fixture.store.routing_decision_state(run_id).unwrap();
    let request = RoutingDecisionSchema::WorkflowClass
        .request(&state)
        .unwrap();
    let model = TestDecisionModel::result(decision_result(&request, "bug_fix", 0.99));
    let mut unsafe_request = request.clone();
    unsafe_request.state["taskText"] = json!("UNMINIMIZED_TASK_PRIVATE_MARKER");
    assert!(fixture
        .store
        .run_decision_shadow_trial(run_id, "m16-unminimized", &unsafe_request, &model)
        .await
        .is_err());
    assert_eq!(model.call_count(), 0);

    let receipt = fixture
        .store
        .run_decision_shadow_trial(run_id, "m16-shadow-1", &request, &model)
        .await
        .expect("run shadow trial")
        .expect("shadow invocation recorded");

    assert_eq!(model.call_count(), 1);
    assert!(receipt.event_id > 0);
    assert_eq!(receipt.status, InvocationStatus::Completed);
    let run = fixture.store.get_run(run_id).unwrap().unwrap();
    assert_eq!(run.status, "running");
    let after = fixture.store.routing_decision_state(run_id).unwrap();
    assert_eq!(after.active_worker_count, 0);
    assert_eq!(after.active_sub_overseer_count, 0);
    let events = fixture
        .store
        .list_events("run", &run_id.to_string())
        .unwrap();
    let telemetry = events
        .iter()
        .find(|event| event.event_type == "optimization_invocation_recorded")
        .expect("append-only invocation telemetry");
    assert_eq!(telemetry.payload["mode"], "shadow");
    assert_eq!(telemetry.payload["status"], "completed");
    assert_eq!(telemetry.payload["influencedExecution"], false);
    assert_eq!(telemetry.payload["prediction"], "bug_fix");
    assert_eq!(telemetry.payload["modelConfidence"], 0.99);
    assert_eq!(telemetry.payload["payloadsPersisted"], false);
}

#[tokio::test]
async fn post_run_shadow_review_links_prediction_to_qa_and_final_outcome_events() {
    let fixture = Fixture::new("shadow-review");
    let config = local_shadow_config(OptimizationCapability::WorkflowRouting);
    let workflow_id = fixture.workflow(json!({ "mandatory_checks": ["test"] }));
    let run_id = fixture
        .store
        .start_run_with_optimization(fixture.project_id, None, None, Some(workflow_id), &config)
        .expect("start treatment-attributed run")
        .id;
    let state = fixture.store.routing_decision_state(run_id).unwrap();
    let request = RoutingDecisionSchema::WorkflowClass
        .request(&state)
        .unwrap();
    let model = TestDecisionModel::result(decision_result(&request, "verification", 0.97));
    fixture
        .store
        .run_decision_shadow_trial(run_id, "m16-review-1", &request, &model)
        .await
        .expect("record shadow prediction");
    fixture
        .store
        .record_quality_check(run_id, "test", true, "passed", &json!({}))
        .expect("record deterministic QA");
    fixture.review_memory(run_id);
    let finished = fixture
        .store
        .finish_run(run_id, "Mandatory QA passed.")
        .expect("finish run");
    assert_eq!(finished.status, "verified_success");
    let run_events = fixture
        .store
        .list_events("run", &run_id.to_string())
        .unwrap();
    let quality_event_id = run_events
        .iter()
        .find(|event| event.event_type == "quality_check_recorded")
        .expect("quality event")
        .id;
    let run_finished_event_id = run_events
        .iter()
        .find(|event| event.event_type == "run_finished")
        .expect("final run event")
        .id;
    let review = OptimizationShadowReview {
        invocation_id: "m16-review-1".to_string(),
        assessment: ShadowOutcomeAssessment::NoMaterialDifference,
        human_corrected_prediction: Some(false),
        evidence_event_ids: vec![quality_event_id, run_finished_event_id],
    };

    let first = fixture
        .store
        .record_optimization_shadow_review(run_id, &review)
        .expect("append human-reviewed counterfactual assessment");
    let replay = fixture
        .store
        .record_optimization_shadow_review(run_id, &review)
        .expect("idempotent review replay");
    assert_eq!(first.id, replay.id);
    assert_eq!(first.event_type, "optimization_shadow_review_recorded");
    assert_eq!(first.payload["assessment"], "no_material_difference");
    assert_eq!(first.payload["finalRunStatus"], "verified_success");
    assert_eq!(first.payload["humanCorrectedPrediction"], false);
    assert_eq!(
        first.payload["evidenceEventIds"].as_array().unwrap().len(),
        2
    );
    let reopened = Store::open(fixture.root.join("data")).expect("reopen SQLite store");
    let persisted_review = reopened
        .list_events("run", &run_id.to_string())
        .unwrap()
        .into_iter()
        .find(|event| event.event_type == "optimization_shadow_review_recorded")
        .expect("persisted review event");
    assert_eq!(persisted_review.id, first.id);

    let mut conflicting = review;
    conflicting.assessment = ShadowOutcomeAssessment::WouldHaveHelped;
    assert!(fixture
        .store
        .record_optimization_shadow_review(run_id, &conflicting)
        .is_err());
}

#[tokio::test]
async fn disabled_remote_and_failed_shadow_trials_never_block_the_baseline() {
    let fixture = Fixture::new("safe-fallbacks");
    let state_config = OptimizationConfig::default();
    let disabled_run = fixture.run(&state_config, None, None);
    let disabled_request = RoutingDecisionSchema::WorkflowClass
        .request(&fixture.store.routing_decision_state(disabled_run).unwrap())
        .unwrap();
    let disabled_model = TestDecisionModel::result(decision_result(
        &disabled_request,
        "insufficient_context",
        0.99,
    ));
    assert!(fixture
        .store
        .run_decision_shadow_trial(
            disabled_run,
            "m16-disabled",
            &disabled_request,
            &disabled_model,
        )
        .await
        .unwrap()
        .is_none());
    assert_eq!(disabled_model.call_count(), 0);

    let mut remote_config = OptimizationConfig::default();
    remote_config.capabilities.insert(
        OptimizationCapability::WorkflowRouting,
        CapabilitySettings {
            enabled: true,
            mode: OptimizationMode::Shadow,
            provider: Some("remote_candidate".to_string()),
            model: Some("candidate-v1".to_string()),
            provider_locality: Some(ProviderLocality::Remote),
            remote_data_allowed: true,
        },
    );
    let remote_run = fixture.run(&remote_config, None, None);
    let remote_request = RoutingDecisionSchema::WorkflowClass
        .request(&fixture.store.routing_decision_state(remote_run).unwrap())
        .unwrap();
    let remote_model =
        TestDecisionModel::result(decision_result(&&remote_request, "unknown", 0.99));
    let remote_receipt = fixture
        .store
        .run_decision_shadow_trial(remote_run, "m16-remote", &remote_request, &remote_model)
        .await
        .expect("remote shadow call is permitted by the immutable opt-in")
        .expect("record remote invocation");
    assert_eq!(remote_receipt.status, InvocationStatus::Completed);
    assert_eq!(remote_model.call_count(), 1);
    let remote_event = fixture
        .store
        .list_events("run", &remote_run.to_string())
        .unwrap()
        .into_iter()
        .find(|event| event.event_type == "optimization_invocation_recorded")
        .expect("record the remote invocation metadata");
    assert_eq!(remote_event.payload["status"], "completed");
    assert_eq!(remote_event.payload["providerLocality"], "remote");
    assert_eq!(remote_event.payload["remoteDataAllowed"], true);
    assert_eq!(remote_event.payload["fallbackUsed"], false);
    assert_eq!(remote_event.payload["payloadsPersisted"], false);

    for (suffix, failure, expected_status) in [
        (
            "timeout",
            ModelCallFailure::Timeout,
            InvocationStatus::Timeout,
        ),
        (
            "unavailable",
            ModelCallFailure::ProviderUnavailable,
            InvocationStatus::ProviderError,
        ),
        (
            "malformed",
            ModelCallFailure::InvalidResponse,
            InvocationStatus::InvalidResponse,
        ),
    ] {
        let run_id = fixture.run(
            &local_shadow_config(OptimizationCapability::WorkflowRouting),
            None,
            None,
        );
        let request = RoutingDecisionSchema::WorkflowClass
            .request(&fixture.store.routing_decision_state(run_id).unwrap())
            .unwrap();
        let model = TestDecisionModel::error(failure);
        let invocation_id = format!("m16-{suffix}");
        let receipt = fixture
            .store
            .run_decision_shadow_trial(run_id, &invocation_id, &request, &model)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.status, expected_status);
        let event = fixture
            .store
            .list_events("run", &run_id.to_string())
            .unwrap()
            .into_iter()
            .find(|event| event.payload["invocationId"] == invocation_id)
            .expect("failure telemetry");
        assert_eq!(event.payload["fallbackUsed"], true);
        assert_eq!(event.payload["influencedExecution"], false);
        assert_eq!(
            fixture.store.get_run(run_id).unwrap().unwrap().status,
            "running"
        );
        assert_eq!(model.call_count(), 1);
    }

    let invalid_run = fixture.run(
        &local_shadow_config(OptimizationCapability::WorkflowRouting),
        None,
        None,
    );
    let invalid_request = RoutingDecisionSchema::WorkflowClass
        .request(&fixture.store.routing_decision_state(invalid_run).unwrap())
        .unwrap();
    let invalid_model = TestDecisionModel::result(decision_result(
        &invalid_request,
        "attempted_action_outside_schema",
        1.0,
    ));
    let invalid_receipt = fixture
        .store
        .run_decision_shadow_trial(
            invalid_run,
            "m16-invalid-outcome",
            &invalid_request,
            &invalid_model,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(invalid_receipt.status, InvocationStatus::InvalidResponse);
    let invalid_event = fixture
        .store
        .list_events("run", &invalid_run.to_string())
        .unwrap()
        .into_iter()
        .find(|event| event.payload["invocationId"] == "m16-invalid-outcome")
        .expect("invalid response telemetry");
    assert_eq!(invalid_event.payload["prediction"], Value::Null);
    assert_eq!(invalid_event.payload["fallbackUsed"], true);
    assert_eq!(
        fixture.store.get_run(invalid_run).unwrap().unwrap().status,
        "running"
    );
}
