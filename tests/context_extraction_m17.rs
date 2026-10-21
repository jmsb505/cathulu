use cathulu_core::{
    context_extraction::{ContextExtractionCandidate, ContextExtractionCandidateKind},
    optimization::{
        CapabilitySettings, ContextModel, ContextModelFuture, ContextRequest, ContextResult,
        InvocationStatus, ModelCallFailure, ModelResponse, OptimizationCapability,
        OptimizationConfig, OptimizationMode, ProviderLocality,
    },
    Store,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    store: Store,
    data_dir: PathBuf,
    project_id: i64,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cathulu-context-m17-{label}-{}-{nonce}",
            std::process::id()
        ));
        let repo = root.join("repo");
        let data_dir = root.join("data");
        fs::create_dir_all(&repo).expect("create test repository");
        git(&repo, &["init", "--quiet", "-b", "main"]);
        git(&repo, &["config", "user.name", "Cathulu Test"]);
        git(
            &repo,
            &["config", "user.email", "cathulu-test@example.invalid"],
        );
        fs::write(repo.join("README.md"), "M17 fixture\n").expect("write fixture file");
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "--quiet", "-m", "fixture"]);
        let store = Store::open(&data_dir).expect("open local store");
        let project_id = store.register_project(&repo).expect("register project").id;
        Self {
            root,
            store,
            data_dir,
            project_id,
        }
    }

    fn run(&self, locality: ProviderLocality) -> i64 {
        self.store
            .start_run_with_optimization(
                self.project_id,
                None,
                None,
                None,
                &shadow_config(locality),
            )
            .expect("start run with M17 treatment")
            .id
    }

    fn artifact(&self, run_id: i64, filename: &str, content: &str) -> (i64, PathBuf) {
        let path = self.root.join(filename);
        fs::write(&path, content).expect("write local tool output");
        let hash = sha256(content);
        let artifact = self
            .store
            .record_evidence_artifact(
                Some(run_id),
                "test_log",
                &path.to_string_lossy(),
                Some(&format!("sha256:{hash}")),
                false,
            )
            .expect("register local evidence artifact");
        (artifact.id, path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone)]
enum ModelReply {
    Result(Option<ContextResult>),
    Error(ModelCallFailure),
}

struct TestContextModel {
    calls: AtomicUsize,
    reply: ModelReply,
    last_request: Mutex<Option<ContextRequest>>,
}

impl TestContextModel {
    fn result(result: ContextResult) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            reply: ModelReply::Result(Some(result)),
            last_request: Mutex::new(None),
        }
    }

    fn error(error: ModelCallFailure) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            reply: ModelReply::Error(error),
            last_request: Mutex::new(None),
        }
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ContextModel for TestContextModel {
    fn process<'a>(&'a self, request: &'a ContextRequest) -> ContextModelFuture<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last_request.lock().expect("request mutex") = Some(request.clone());
        let reply = self.reply.clone();
        Box::pin(async move {
            match reply {
                ModelReply::Result(result) => Ok(result.map(ModelResponse::new)),
                ModelReply::Error(error) => Err(error),
            }
        })
    }
}

fn shadow_config(locality: ProviderLocality) -> OptimizationConfig {
    let mut config = OptimizationConfig::default();
    config.capabilities.insert(
        OptimizationCapability::ContextExtract,
        CapabilitySettings {
            enabled: true,
            mode: OptimizationMode::Shadow,
            provider: Some("context_test_model".to_string()),
            model: Some("fixture-v1".to_string()),
            provider_locality: Some(locality),
            remote_data_allowed: locality == ProviderLocality::Remote,
        },
    );
    config
}

fn result_from_candidates(
    request: &ContextRequest,
    candidates: Vec<ContextExtractionCandidate>,
) -> ContextResult {
    let source_spans = candidates
        .iter()
        .map(|candidate| candidate.source_span.clone())
        .collect();
    ContextResult {
        schema_version: request.schema_version,
        content: json!({ "candidates": candidates }),
        source_artifact_ids: request
            .sources
            .iter()
            .map(|source| source.artifact_id)
            .collect(),
        source_content_hashes: request
            .sources
            .iter()
            .map(|source| source.content_hash.clone())
            .collect(),
        source_spans,
        model_confidence: Some(0.82),
    }
}

fn representative_log() -> &'static str {
    "TOP_SECRET_CONTEXT_SHOULD_NOT_BE_SELECTED\n\
     routine output line 1\n\
     routine output line 2\n\
     running 1 test\n\
     test parser::rejects_bad_input ... FAILED\n\
     failures:\n\
     Error: expected token, received EOF\n\
     test result: FAILED. 0 passed; 1 failed\n"
}

fn sha256(content: &str) -> String {
    Sha256::digest(content.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
fn request_selects_hash_verified_spans_and_keeps_raw_artifact_unchanged() {
    let fixture = Fixture::new("selected-spans");
    let run_id = fixture.run(ProviderLocality::Local);
    let raw = representative_log();
    let (artifact_id, path) = fixture.artifact(run_id, "test-output.log", raw);

    let request = fixture
        .store
        .prepare_context_extraction_request(&[artifact_id])
        .expect("prepare minimized extraction input");
    let source = &request.sources[0];
    assert_eq!(source.artifact_id, artifact_id);
    assert_eq!(source.content_hash, sha256(raw));
    assert_eq!(
        source.source_ref,
        format!("cathulu://artifacts/{artifact_id}")
    );
    assert!(!source
        .source_ref
        .contains(&fixture.root.to_string_lossy().to_string()));
    assert!(!source
        .content
        .contains("TOP_SECRET_CONTEXT_SHOULD_NOT_BE_SELECTED"));
    assert!(!source.source_spans.is_empty());
    assert!(source
        .source_spans
        .iter()
        .all(|span| span.artifact_id == artifact_id));

    let reconstructed = source
        .source_spans
        .iter()
        .flat_map(|span| raw.as_bytes()[span.start_byte as usize..span.end_byte as usize].to_vec())
        .collect::<Vec<_>>();
    assert_eq!(reconstructed, source.content.as_bytes());
    let candidates = fixture
        .store
        .deterministic_context_extraction(&request)
        .expect("deterministic baseline extraction");
    assert!(candidates.len() >= 2);
    assert!(candidates.iter().all(|candidate| {
        &raw.as_bytes()
            [candidate.source_span.start_byte as usize..candidate.source_span.end_byte as usize]
            == candidate.quote.as_bytes()
    }));

    assert_eq!(
        fs::read(&path).expect("raw evidence remains retrievable"),
        raw.as_bytes()
    );
    let artifact = fixture.store.get_artifact(artifact_id).unwrap().unwrap();
    assert_eq!(
        artifact.content_hash.as_deref(),
        Some(format!("sha256:{}", sha256(raw)).as_str())
    );
}

#[tokio::test]
async fn local_shadow_records_provenance_without_changing_run_qa_or_raw_evidence() {
    let fixture = Fixture::new("local-shadow");
    let run_id = fixture.run(ProviderLocality::Local);
    let raw = representative_log();
    let (artifact_id, path) = fixture.artifact(run_id, "captured-output.log", raw);
    let request = fixture
        .store
        .prepare_context_extraction_request(&[artifact_id])
        .unwrap();
    let baseline = fixture
        .store
        .deterministic_context_extraction(&request)
        .unwrap();
    let result = result_from_candidates(&request, baseline);
    let model = TestContextModel::result(result);

    let receipt = fixture
        .store
        .run_context_extraction_shadow_trial(run_id, "m17-local-1", &request, &model)
        .await
        .expect("shadow trial should not block the run")
        .expect("enabled shadow invocation is recorded");

    assert_eq!(receipt.status, InvocationStatus::Completed);
    assert_eq!(model.call_count(), 1);
    let observed_request = model.last_request.lock().unwrap().clone().unwrap();
    assert_eq!(observed_request, request);
    assert!(!serde_json::to_string(&observed_request)
        .unwrap()
        .contains(&fixture.root.to_string_lossy().to_string()));

    let events = fixture
        .store
        .list_events("run", &run_id.to_string())
        .unwrap();
    let invocation = events
        .iter()
        .find(|event| event.event_type == "optimization_invocation_recorded")
        .expect("invocation telemetry is durable");
    assert_eq!(invocation.payload["mode"], "shadow");
    assert_eq!(invocation.payload["status"], "completed");
    assert_eq!(invocation.payload["influencedExecution"], false);
    assert_eq!(invocation.payload["payloadsPersisted"], false);

    let extraction = events
        .iter()
        .find(|event| event.event_type == "context_extraction_shadow_recorded")
        .expect("validated extraction result is retained for review");
    assert_eq!(extraction.payload["influencedExecution"], false);
    assert_eq!(extraction.payload["rawSourceOverwritten"], false);
    assert_eq!(
        extraction.payload["sourceArtifacts"][0]["artifactId"],
        artifact_id
    );
    assert_eq!(
        extraction.payload["sourceArtifacts"][0]["contentHash"],
        sha256(raw)
    );
    let selected_sources = serde_json::to_string(&request.sources).unwrap();
    assert_eq!(
        extraction.payload["selectedInputHash"],
        sha256(&selected_sources)
    );
    assert_eq!(
        extraction.payload["candidateCount"],
        extraction.payload["baselineCandidateCount"]
    );
    assert_eq!(
        extraction.payload["candidateSpanMatches"],
        extraction.payload["candidateCount"]
    );
    let encoded_event = serde_json::to_string(&extraction.payload).unwrap();
    assert!(!encoded_event.contains("TOP_SECRET_CONTEXT_SHOULD_NOT_BE_SELECTED"));
    assert!(!encoded_event.contains(&fixture.root.to_string_lossy().to_string()));
    assert_eq!(
        fixture.store.get_run(run_id).unwrap().unwrap().status,
        "running"
    );
    assert_eq!(fs::read(&path).unwrap(), raw.as_bytes());

    let reopened = Store::open(&fixture.data_dir).expect("reopen authoritative SQLite store");
    assert_eq!(fs::read(&path).unwrap(), raw.as_bytes());
    assert!(reopened
        .list_events("run", &run_id.to_string())
        .unwrap()
        .iter()
        .any(|event| event.event_type == "context_extraction_shadow_recorded"));
    assert_eq!(
        reopened.get_artifact(artifact_id).unwrap().unwrap().id,
        artifact_id
    );
}

#[tokio::test]
async fn malformed_or_failed_context_models_are_recorded_as_non_blocking_fallbacks() {
    let fixture = Fixture::new("fallbacks");
    for (label, reply, expected_status) in [
        (
            "timeout",
            ModelReply::Error(ModelCallFailure::Timeout),
            InvocationStatus::Timeout,
        ),
        (
            "unavailable",
            ModelReply::Error(ModelCallFailure::ProviderUnavailable),
            InvocationStatus::ProviderError,
        ),
    ] {
        let run_id = fixture.run(ProviderLocality::Local);
        let (artifact_id, _) =
            fixture.artifact(run_id, &format!("{label}.log"), representative_log());
        let request = fixture
            .store
            .prepare_context_extraction_request(&[artifact_id])
            .unwrap();
        let model = match reply {
            ModelReply::Error(error) => TestContextModel::error(error),
            ModelReply::Result(_) => unreachable!(),
        };
        let receipt = fixture
            .store
            .run_context_extraction_shadow_trial(run_id, &format!("m17-{label}"), &request, &model)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.status, expected_status);
        assert_eq!(model.call_count(), 1);
        assert_eq!(
            fixture.store.get_run(run_id).unwrap().unwrap().status,
            "running"
        );
        assert!(!fixture
            .store
            .list_events("run", &run_id.to_string())
            .unwrap()
            .iter()
            .any(|event| event.event_type == "context_extraction_shadow_recorded"));
    }

    let run_id = fixture.run(ProviderLocality::Local);
    let (artifact_id, _) = fixture.artifact(run_id, "malformed.log", representative_log());
    let request = fixture
        .store
        .prepare_context_extraction_request(&[artifact_id])
        .unwrap();
    let candidates = fixture
        .store
        .deterministic_context_extraction(&request)
        .unwrap();
    let mut invalid = result_from_candidates(&request, candidates);
    let mut invalid_candidates: Vec<ContextExtractionCandidate> =
        serde_json::from_value(invalid.content["candidates"].clone()).unwrap();
    invalid_candidates[0].quote.push_str(" fabricated");
    invalid.content = json!({ "candidates": invalid_candidates });
    let model = TestContextModel::result(invalid);
    let receipt = fixture
        .store
        .run_context_extraction_shadow_trial(run_id, "m17-invalid-span", &request, &model)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status, InvocationStatus::InvalidResponse);
    assert_eq!(
        fixture.store.get_run(run_id).unwrap().unwrap().status,
        "running"
    );
    let events = fixture
        .store
        .list_events("run", &run_id.to_string())
        .unwrap();
    let telemetry = events
        .iter()
        .find(|event| event.payload["invocationId"] == "m17-invalid-span")
        .unwrap();
    assert_eq!(telemetry.payload["status"], "invalid_response");
    assert_eq!(telemetry.payload["influencedExecution"], false);
    assert!(!events
        .iter()
        .any(|event| event.event_type == "context_extraction_shadow_recorded"));
}

#[tokio::test]
async fn remote_opt_in_runs_shadow_and_empty_inputs_skip_without_changing_baseline() {
    let fixture = Fixture::new("skip");
    let remote_run = fixture.run(ProviderLocality::Remote);
    let (artifact_id, _) =
        fixture.artifact(remote_run, "remote-candidate.log", representative_log());
    let remote_request = fixture
        .store
        .prepare_context_extraction_request(&[artifact_id])
        .unwrap();
    let candidates = fixture
        .store
        .deterministic_context_extraction(&remote_request)
        .unwrap();
    let remote_model =
        TestContextModel::result(result_from_candidates(&remote_request, candidates));
    let remote_receipt = fixture
        .store
        .run_context_extraction_shadow_trial(
            remote_run,
            "m17-remote-skip",
            &remote_request,
            &remote_model,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(remote_receipt.status, InvocationStatus::Completed);
    assert_eq!(remote_model.call_count(), 1);

    let empty_run = fixture.run(ProviderLocality::Local);
    let (empty_artifact_id, _) =
        fixture.artifact(empty_run, "no-errors.log", "all checks passed\n");
    let empty_request = fixture
        .store
        .prepare_context_extraction_request(&[empty_artifact_id])
        .unwrap();
    assert!(empty_request.sources[0].content.is_empty());
    assert!(empty_request.sources[0].source_spans.is_empty());
    let empty_model = TestContextModel::error(ModelCallFailure::ProviderUnavailable);
    let empty_receipt = fixture
        .store
        .run_context_extraction_shadow_trial(
            empty_run,
            "m17-empty-skip",
            &empty_request,
            &empty_model,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(empty_receipt.status, InvocationStatus::Skipped);
    assert_eq!(empty_model.call_count(), 0);

    for (run_id, expected_status) in [(remote_run, "completed"), (empty_run, "skipped")] {
        assert_eq!(
            fixture.store.get_run(run_id).unwrap().unwrap().status,
            "running"
        );
        let events = fixture
            .store
            .list_events("run", &run_id.to_string())
            .unwrap();
        let telemetry = events
            .iter()
            .find(|event| event.event_type == "optimization_invocation_recorded")
            .unwrap();
        assert_eq!(telemetry.payload["status"], expected_status);
        assert_eq!(telemetry.payload["influencedExecution"], false);
        assert_eq!(
            events
                .iter()
                .any(|event| event.event_type == "context_extraction_shadow_recorded"),
            run_id == remote_run
        );
    }
}

#[test]
fn unsupported_or_changed_artifacts_are_rejected_before_model_execution() {
    let fixture = Fixture::new("artifact-validation");
    let run_id = fixture.run(ProviderLocality::Local);
    let (artifact_id, path) = fixture.artifact(run_id, "source.log", representative_log());
    let model = TestContextModel::error(ModelCallFailure::ProviderUnavailable);

    fs::write(&path, "changed evidence\n").expect("change artifact after registration");
    assert!(fixture
        .store
        .prepare_context_extraction_request(&[artifact_id])
        .is_err());
    assert_eq!(model.call_count(), 0);

    let unsupported_path = fixture.root.join("unsupported-source.txt");
    fs::write(&unsupported_path, representative_log()).expect("write non-tool evidence");
    let source_artifact = fixture
        .store
        .record_evidence_artifact(
            Some(run_id),
            "source_tree",
            &unsupported_path.to_string_lossy(),
            None,
            false,
        )
        .expect("register non-tool evidence kind");
    assert!(fixture
        .store
        .prepare_context_extraction_request(&[source_artifact.id])
        .is_err());
}

#[test]
fn captured_local_test_harness_output_matches_the_deterministic_baseline() {
    let fixture = Fixture::new("captured-test-output");
    let run_id = fixture.run(ProviderLocality::Local);
    let test_source = fixture.root.join("deliberate_failure.rs");
    let test_binary = fixture.root.join(if cfg!(windows) {
        "deliberate_failure.exe"
    } else {
        "deliberate_failure"
    });
    fs::write(
        &test_source,
        "#[test]\nfn captured_failure() { assert_eq!(2, 1); }\n",
    )
    .expect("write local Rust test fixture");
    let compile = Command::new("rustc")
        .arg("--test")
        .arg(&test_source)
        .arg("-o")
        .arg(&test_binary)
        .output()
        .expect("compile local test fixture");
    assert!(
        compile.status.success(),
        "rustc fixture compilation failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let captured = Command::new(&test_binary)
        .output()
        .expect("capture local test-harness output");
    assert!(
        !captured.status.success(),
        "fixture should fail intentionally"
    );
    let mut captured_output = String::from_utf8_lossy(&captured.stdout).into_owned();
    captured_output.push_str(&String::from_utf8_lossy(&captured.stderr));
    assert!(captured_output.contains("FAILED"));

    let (artifact_id, raw_path) =
        fixture.artifact(run_id, "captured-rust-test.log", &captured_output);
    let request = fixture
        .store
        .prepare_context_extraction_request(&[artifact_id])
        .expect("prepare extraction from real local tool output");
    let candidates = fixture
        .store
        .deterministic_context_extraction(&request)
        .expect("extract deterministic baseline candidates");
    assert!(!candidates.is_empty());
    assert!(candidates
        .iter()
        .any(|candidate| { candidate.kind == ContextExtractionCandidateKind::TestFailure }));
    assert_eq!(
        fs::read(raw_path).expect("captured raw output remains available"),
        captured_output.as_bytes()
    );
}

#[test]
fn candidate_kinds_remain_closed_and_versioned() {
    assert!(
        serde_json::from_value::<ContextExtractionCandidateKind>(json!("security_override"))
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(ContextExtractionCandidateKind::Panic).unwrap(),
        Value::String("panic".to_string())
    );
}

#[tokio::test]
async fn shadow_extraction_rejects_artifacts_from_a_different_run_before_model_use() {
    let fixture = Fixture::new("artifact-run-scope");
    let trial_run_id = fixture.run(ProviderLocality::Local);
    let evidence_run_id = fixture.run(ProviderLocality::Local);
    let (artifact_id, _) = fixture.artifact(
        evidence_run_id,
        "other-run.log",
        "error: evidence belongs to another run\n",
    );
    let request = fixture
        .store
        .prepare_context_extraction_request(&[artifact_id])
        .expect("prepare registered artifact request");
    let model = TestContextModel::error(ModelCallFailure::ProviderUnavailable);

    let error = fixture
        .store
        .run_context_extraction_shadow_trial(
            trial_run_id,
            "m17-cross-run-artifact",
            &request,
            &model,
        )
        .await
        .expect_err("cross-run evidence is rejected");

    assert!(error.contains("same run"));
    assert_eq!(model.call_count(), 0);
    assert_eq!(
        fixture.store.get_run(trial_run_id).unwrap().unwrap().status,
        "running"
    );
}
