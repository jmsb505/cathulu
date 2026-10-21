use cathulu_core::{
    optimization::{
        CapabilitySettings, ContextCapability, ContextModel, ContextRequest, ContextSource,
        DecisionModel, DecisionRequest, DisabledContextModel, DisabledDecisionModel,
        EffectiveOptimizationMode, InvocationStatus, OptimizationCapability, OptimizationConfig,
        OptimizationInvocation, OptimizationMode, ProviderLocality, TreatmentConfigurationState,
    },
    Store,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
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
            "cathulu-optimization-{label}-{}-{nonce}",
            std::process::id()
        ));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).expect("create test repo");
        git(&repo, &["init", "--quiet", "-b", "main"]);
        git(&repo, &["config", "user.name", "Cathulu Test"]);
        git(
            &repo,
            &["config", "user.email", "cathulu-test@example.invalid"],
        );
        fs::write(repo.join("README.md"), "optimization fixture\n").expect("write fixture file");
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "--quiet", "-m", "fixture"]);
        let store = Store::open(root.join("data")).expect("open local store");
        let project_id = store.register_project(&repo).expect("register project").id;
        Self {
            root,
            store,
            project_id,
        }
    }

    fn workflow(&self, definition: Value) -> i64 {
        self.store
            .create_workflow_version("m15_test", "M15 test", 1, &definition)
            .expect("create workflow version")
            .id
    }

    fn run(&self, workflow_version_id: Option<i64>) -> i64 {
        self.store
            .start_run(self.project_id, None, None, workflow_version_id)
            .expect("start disabled-by-default run")
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
        .expect("run git command");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn local_enabled() -> CapabilitySettings {
    CapabilitySettings {
        enabled: true,
        provider: Some("jev".to_string()),
        provider_locality: Some(ProviderLocality::Local),
        ..CapabilitySettings::default()
    }
}

fn invocation() -> OptimizationInvocation {
    OptimizationInvocation {
        invocation_id: "shadow-run-1".to_string(),
        capability: OptimizationCapability::WorkflowRouting,
        provider: "jev".to_string(),
        model: Some("decision-test".to_string()),
        model_version: Some("test-version".to_string()),
        provider_locality: ProviderLocality::Local,
        remote_data_allowed: false,
        schema_version: 1,
        mode: EffectiveOptimizationMode::Shadow,
        input_bytes: 256,
        output_bytes: 32,
        input_tokens: Some(64),
        output_tokens: Some(8),
        latency_ms: 18,
        status: InvocationStatus::Completed,
        prediction: Some("bug_fix".to_string()),
        model_confidence: Some(0.84),
        probabilities: [("bug_fix".to_string(), 0.84)].into_iter().collect(),
        fallback_used: false,
        influenced_execution: false,
    }
}

#[tokio::test]
async fn every_capability_defaults_to_disabled_and_disabled_models_are_noops() {
    let fixture = Fixture::new("disabled-defaults");
    let config = OptimizationConfig::default();
    for capability in OptimizationCapability::ALL {
        assert_eq!(
            config.effective_mode(capability),
            EffectiveOptimizationMode::Disabled
        );
    }

    let decision = DecisionRequest {
        capability: OptimizationCapability::WorkflowRouting,
        schema_version: 1,
        state: json!({ "taskType": "maintenance" }),
        allowed_outcomes: vec!["bug_fix".to_string(), "needs_codex".to_string()],
    };
    assert!(decision.validate().is_ok());
    assert_eq!(DisabledDecisionModel.decide(&decision).await.unwrap(), None);
    let context = ContextRequest {
        capability: ContextCapability::Extract,
        schema_version: 1,
        task: "extract failures".to_string(),
        sources: vec![ContextSource {
            artifact_id: 1,
            content_hash: "a".repeat(64),
            source_ref: "artifact://test/output".to_string(),
            content: "raw output remains local".to_string(),
            source_spans: vec![],
        }],
        max_output_bytes: 1024,
    };
    assert!(context.validate().is_ok());
    assert_eq!(DisabledContextModel.process(&context).await.unwrap(), None);

    let run_id = fixture.run(None);
    let explicit_disabled_run = fixture
        .store
        .start_run_with_optimization(
            fixture.project_id,
            None,
            None,
            None,
            &OptimizationConfig::default(),
        )
        .expect("start run with explicit all-disabled configuration");
    let baseline_run = fixture.store.get_run(run_id).unwrap().unwrap();
    assert_eq!(baseline_run.project_id, explicit_disabled_run.project_id);
    assert_eq!(baseline_run.status, explicit_disabled_run.status);
    assert_eq!(
        baseline_run.workflow_version_id,
        explicit_disabled_run.workflow_version_id
    );
    let baseline_finished = fixture
        .store
        .finish_run(run_id, "Disabled baseline completed.")
        .expect("finish default run");
    let explicit_finished = fixture
        .store
        .finish_run(explicit_disabled_run.id, "Disabled baseline completed.")
        .expect("finish explicit disabled run");
    assert_eq!(baseline_finished.status, explicit_finished.status);
    assert_eq!(baseline_finished.checks, explicit_finished.checks);
    let events = fixture
        .store
        .list_events("run", &run_id.to_string())
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "optimization_invocation_recorded")
            .count(),
        0
    );
    let treatment = fixture
        .store
        .get_run_optimization_treatment(run_id)
        .unwrap()
        .expect("run treatment");
    assert_eq!(
        treatment.configuration_state,
        TreatmentConfigurationState::Valid
    );
    assert!(OptimizationCapability::ALL.iter().all(|capability| {
        treatment.configuration.effective_mode(*capability) == EffectiveOptimizationMode::Disabled
    }));
}

#[test]
fn capability_flags_are_independent_and_workflow_overrides_are_versioned_in_run_treatment() {
    let fixture = Fixture::new("workflow-overrides");
    let mut config = OptimizationConfig::default();
    config
        .capabilities
        .insert(OptimizationCapability::WorkflowRouting, local_enabled());
    config.validate().expect("valid local shadow configuration");
    assert_eq!(
        config.effective_mode(OptimizationCapability::WorkflowRouting),
        EffectiveOptimizationMode::Shadow
    );
    assert_eq!(
        config.effective_mode(OptimizationCapability::DelegationRouting),
        EffectiveOptimizationMode::Disabled
    );

    let workflow_id = fixture.workflow(json!({
        "mandatory_checks": ["test"],
        "optimization": {
            "capabilities": {
                "context_extract": {
                    "enabled": true,
                    "mode": "shadow",
                    "provider": "scaledown",
                    "providerLocality": "remote",
                    "remoteDataAllowed": true
                }
            }
        }
    }));
    let run_id = fixture
        .store
        .start_run_with_optimization(fixture.project_id, None, None, Some(workflow_id), &config)
        .expect("start run with workflow treatment")
        .id;
    let treatment = fixture
        .store
        .get_run_optimization_treatment(run_id)
        .unwrap()
        .expect("treatment snapshot");
    assert_eq!(treatment.workflow_version_id, Some(workflow_id));
    assert_eq!(
        treatment.workflow_content_hash.as_deref().unwrap().len(),
        64
    );
    assert_eq!(treatment.configuration_hash.len(), 64);
    assert_eq!(
        treatment
            .configuration
            .effective_mode(OptimizationCapability::WorkflowRouting),
        EffectiveOptimizationMode::Shadow
    );
    assert_eq!(
        treatment
            .configuration
            .effective_mode(OptimizationCapability::ContextExtract),
        EffectiveOptimizationMode::Shadow
    );
    assert_eq!(
        treatment
            .configuration
            .effective_mode(OptimizationCapability::ContextCompress),
        EffectiveOptimizationMode::Disabled
    );
    assert!(
        treatment
            .configuration
            .settings(OptimizationCapability::ContextExtract)
            .remote_data_allowed
    );
}

#[test]
fn invalid_configuration_fails_closed_without_blocking_run_creation() {
    let fixture = Fixture::new("invalid-config");
    let mut invalid = OptimizationConfig::default();
    let mut remote = local_enabled();
    remote.provider_locality = Some(ProviderLocality::Remote);
    remote.remote_data_allowed = false;
    invalid
        .capabilities
        .insert(OptimizationCapability::ContextExtract, remote);
    assert!(invalid.validate().is_err());
    let mut active = OptimizationConfig::default();
    let mut active_routing = local_enabled();
    active_routing.mode = OptimizationMode::Active;
    active
        .capabilities
        .insert(OptimizationCapability::WorkflowRouting, active_routing);
    assert!(active.validate().is_err());

    let workflow_id = fixture.workflow(json!({
        "mandatory_checks": ["test"],
        "optimization": {
            "capabilities": {
                "context_extract": { "enabled": true, "mode": "active" }
            }
        }
    }));
    let run_id = fixture
        .store
        .start_run_with_optimization(fixture.project_id, None, None, Some(workflow_id), &invalid)
        .expect("invalid optional treatment must not prevent a run")
        .id;
    let treatment = fixture
        .store
        .get_run_optimization_treatment(run_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        treatment.configuration_state,
        TreatmentConfigurationState::InvalidDisabledFallback
    );
    assert!(OptimizationCapability::ALL.iter().all(|capability| {
        treatment.configuration.effective_mode(*capability) == EffectiveOptimizationMode::Disabled
    }));
}

#[test]
fn run_attribution_and_deduplicated_telemetry_are_append_only_and_survive_reopen() {
    let fixture = Fixture::new("telemetry-reopen");
    let workflow_id = fixture.workflow(json!({ "mandatory_checks": ["test"] }));
    let run_id = fixture.run(Some(workflow_id));
    let first = fixture
        .store
        .record_optimization_invocation(run_id, &invocation())
        .expect("record shadow telemetry");
    let duplicate = fixture
        .store
        .record_optimization_invocation(run_id, &invocation())
        .expect("deduplicate replayed telemetry");
    assert_eq!(first.id, duplicate.id);

    let mut conflicting = invocation();
    conflicting.latency_ms += 1;
    assert!(fixture
        .store
        .record_optimization_invocation(run_id, &conflicting)
        .is_err());

    let persisted = Store::open(fixture.root.join("data")).expect("reopen SQLite store");
    let treatment = persisted
        .get_run_optimization_treatment(run_id)
        .unwrap()
        .expect("persisted treatment");
    assert_eq!(treatment.workflow_version_id, Some(workflow_id));
    let events = persisted.list_events("run", &run_id.to_string()).unwrap();
    let invocations = events
        .iter()
        .filter(|event| event.event_type == "optimization_invocation_recorded")
        .collect::<Vec<_>>();
    assert_eq!(invocations.len(), 1);
    let payload = &invocations[0].payload;
    assert_eq!(payload["telemetrySchemaVersion"], 1);
    assert_eq!(payload["workflowVersionId"], workflow_id);
    assert_eq!(payload["configurationHash"], treatment.configuration_hash);
    assert_eq!(payload["payloadsPersisted"], false);
    let serialized = serde_json::to_string(payload).unwrap();
    assert!(!serialized.contains("raw prompt"));
    assert!(!serialized.contains("raw response"));
    assert!(!serialized.contains("credential"));

    let database = Connection::open(persisted.database_path()).unwrap();
    assert!(database
        .execute(
            "UPDATE events SET payload_json = '{}' WHERE id = ?1",
            [first.id]
        )
        .is_err());
    assert!(database
        .execute("DELETE FROM events WHERE id = ?1", [first.id])
        .is_err());
    let schema_version: i64 = database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(
        schema_version, 11,
        "M15 should not add a duplicate schema authority"
    );
}

#[test]
fn shadow_telemetry_cannot_influence_execution_and_remote_use_requires_egress_permission() {
    let fixture = Fixture::new("telemetry-guards");
    let run_id = fixture.run(None);
    let mut influencing = invocation();
    influencing.influenced_execution = true;
    assert!(fixture
        .store
        .record_optimization_invocation(run_id, &influencing)
        .is_err());

    let mut remote = invocation();
    remote.invocation_id = "remote-without-egress".to_string();
    remote.provider_locality = ProviderLocality::Remote;
    remote.remote_data_allowed = false;
    assert!(fixture
        .store
        .record_optimization_invocation(run_id, &remote)
        .is_err());

    let serialized: Value = serde_json::from_str(
        r#"{"capabilities":{"workflow_routing":{"enabled":true,"mode":"shadow","provider":"jev","providerLocality":"local","remoteDataAllowed":false,"extra":"unexpected"}}}"#,
    )
    .unwrap();
    assert!(serde_json::from_value::<OptimizationConfig>(serialized).is_err());
    assert_ne!(
        ContextCapability::Extract,
        ContextCapability::Classify,
        "context operations retain distinct contracts"
    );
    assert_ne!(
        OptimizationCapability::WorkflowRouting,
        OptimizationCapability::MemoryReranking
    );
    assert_eq!(OptimizationMode::default(), OptimizationMode::Shadow);
}
