use super::{dashboard_snapshot, project_snapshot, DesktopState};
use cathulu_core::Store;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::WebviewWindowBuilder;

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params})
}

fn run_mcp(data_dir: &Path, rpc_request: Value) -> Vec<Value> {
    let executable = std::env::var_os("CATHULU_MCP_BIN")
        .map(PathBuf::from)
        .expect("set CATHULU_MCP_BIN to the locally built cathulu-mcp executable");
    let mut child = Command::new(executable)
        .env("CATHULU_DATA_DIR", data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch the local Cathulu MCP server");
    {
        let stdin = child.stdin.as_mut().expect("MCP stdin is piped");
        let messages = [
            request(
                1,
                "initialize",
                json!({
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {"name":"phase1-m4-integration-test", "version":"0.1.0"}
                }),
            ),
            json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
            rpc_request,
        ];
        for message in messages {
            serde_json::to_writer(&mut *stdin, &message).expect("serialize MCP message");
            stdin.write_all(b"\n").expect("terminate MCP message");
        }
    }
    let output = child.wait_with_output().expect("wait for MCP server");
    assert!(
        output.status.success(),
        "MCP server exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("MCP output is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("MCP stdout is JSON-RPC only"))
        .collect()
}

fn response(messages: &[Value], id: u64) -> &Value {
    messages
        .iter()
        .find(|message| message.get("id") == Some(&json!(id)))
        .unwrap_or_else(|| panic!("missing MCP response {id}: {messages:?}"))
}

fn mcp_tool(data_dir: &Path, name: &str, arguments: Value) -> Value {
    let messages = run_mcp(
        data_dir,
        request(2, "tools/call", json!({"name":name, "arguments":arguments})),
    );
    let result = response(&messages, 2);
    assert_ne!(
        result.pointer("/result/isError"),
        Some(&json!(true)),
        "MCP tool {name} failed: {result}"
    );
    let text = result
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .expect("MCP tool result contains JSON text");
    serde_json::from_str(text).expect("decode MCP tool result")
}

fn mcp_resource(data_dir: &Path, uri: &str) -> Value {
    let messages = run_mcp(data_dir, request(2, "resources/read", json!({"uri":uri})));
    let text = response(&messages, 2)
        .pointer("/result/contents/0/text")
        .and_then(Value::as_str)
        .expect("MCP resource result contains JSON text");
    serde_json::from_str(text).expect("decode MCP resource result")
}

fn run_git(repo: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(arguments)
        .output()
        .expect("launch git for the disposable fixture");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn invoke_dashboard(data_dir: &Path) -> cathulu_core::dashboard::DashboardSnapshot {
    let app = mock_builder()
        .manage(DesktopState {
            store: Store::open(data_dir).expect("open the MCP database from the Tauri boundary"),
        })
        .invoke_handler(tauri::generate_handler![
            project_snapshot,
            dashboard_snapshot
        ])
        .build(mock_context(noop_assets()))
        .expect("build mock Tauri app");
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build mock Tauri window");
    let response = tauri::test::get_ipc_response(
        &window,
        InvokeRequest {
            cmd: "dashboard_snapshot".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .expect("invoke the read-only dashboard command");
    response
        .deserialize::<cathulu_core::dashboard::DashboardSnapshot>()
        .expect("decode dashboard snapshot")
}

#[test]
#[ignore = "run from scripts/phase1-m4-integration-smoke.ps1 with isolated MCP and desktop binaries"]
fn phase1_m4_mcp_state_is_visible_through_tauri_and_survives_restarts() {
    let root = PathBuf::from(
        std::env::var_os("CATHULU_SMOKE_ROOT")
            .expect("set CATHULU_SMOKE_ROOT to a disposable directory"),
    );
    assert!(root.is_absolute(), "smoke root must be an absolute path");
    assert!(
        root.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("cathulu-phase1-m4-")),
        "smoke root must use the dedicated temporary fixture prefix"
    );
    let temp_dir = std::env::temp_dir()
        .canonicalize()
        .expect("canonicalize system temporary directory");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    fs::create_dir_all(&repo).expect("create disposable Git repository");
    assert!(
        root.canonicalize()
            .expect("canonicalize disposable smoke root")
            .starts_with(&temp_dir),
        "smoke data must stay under the system temporary directory"
    );
    run_git(&repo, &["init", "--quiet", "-b", "main"]);
    run_git(&repo, &["config", "user.name", "Cathulu Integration"]);
    run_git(
        &repo,
        &[
            "config",
            "user.email",
            "cathulu-integration@example.invalid",
        ],
    );
    fs::write(repo.join("README.md"), "Cathulu integration fixture\n")
        .expect("write tracked fixture file");
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "--quiet", "-m", "integration fixture"]);

    let project = mcp_tool(&data_dir, "register_project", json!({"path":repo}));
    let project_id = project["id"].as_i64().expect("project ID");
    fs::write(
        repo.join("changed-after-registration.txt"),
        "dirty fixture\n",
    )
    .expect("create a post-registration Git change");

    let objective = mcp_tool(
        &data_dir,
        "create_objective",
        json!({"project_id":project_id, "title":"Phase 1–4 integration validation", "description":"Prove the local MCP and read-only desktop share durable state."}),
    );
    let objective_id = objective["id"].as_i64().expect("objective ID");
    mcp_tool(
        &data_dir,
        "set_objective_status",
        json!({"id":objective_id, "status":"active"}),
    );
    let decision = mcp_tool(
        &data_dir,
        "record_decision",
        json!({"project_id":project_id, "objective_id":objective_id, "title":"Use one local store", "body":"The MCP and desktop boundaries read the same SQLite database."}),
    );
    let decision_id = decision["id"].as_i64().expect("decision ID");
    let workflow = mcp_tool(
        &data_dir,
        "create_workflow_version",
        json!({"workflow_key":"phase1_m4_integration", "name":"Phase 1–4 integration", "version":1, "definition":{"mandatory_checks":["format", "tests"]}}),
    );
    let workflow_id = workflow["id"].as_i64().expect("workflow version ID");
    let run = mcp_tool(
        &data_dir,
        "start_run",
        json!({"project_id":project_id, "objective_id":objective_id, "workflow_version_id":workflow_id}),
    );
    let run_id = run["id"].as_i64().expect("run ID");
    for check in ["format", "tests"] {
        mcp_tool(
            &data_dir,
            "record_quality_check",
            json!({"run_id":run_id, "check_key":check, "mandatory":true, "status":"passed", "details":{"fixture":"isolated phase 1–4 smoke"}}),
        );
    }
    let evidence_dir = root.join("evidence");
    fs::create_dir_all(&evidence_dir).expect("create external evidence fixture directory");
    let artifact_path = evidence_dir.join("integration-evidence.txt");
    fs::write(&artifact_path, "All required integration checks passed.\n")
        .expect("write local evidence fixture");
    let artifact = mcp_tool(
        &data_dir,
        "record_evidence_artifact",
        json!({"run_id":run_id, "artifact_kind":"test_result", "uri":artifact_path, "protected":true}),
    );
    let artifact_id = artifact["id"].as_i64().expect("artifact ID");
    mcp_tool(
        &data_dir,
        "set_memory_hot_budget",
        json!({
            "scope_type":"project",
            "scope_id":project_id.to_string(),
            "hot_budget_bytes":1024
        }),
    );
    let memory = mcp_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type":"project",
            "scope_id":project_id.to_string(),
            "memory_class":"canonical",
            "temperature":"hot",
            "kind":"decision",
            "body":"The Phase 1–4 integration smoke proved MCP and the dashboard share local SQLite state.",
            "sources":[{"source_type":"decision", "source_id":decision_id.to_string()}],
            "artifact_ids":[artifact_id],
            "valid_from":null,
            "valid_until":null,
            "supersedes_id":null,
            "freshness_basis":{},
            "importance":0.9,
            "confidence":1.0,
            "active":true,
            "protected":false
        }),
    );
    let memory_id = memory["id"].as_i64().expect("memory ID");
    assert_eq!(memory["memoryClass"], json!("canonical"));
    assert_eq!(memory["temperature"], json!("hot"));
    assert_eq!(memory["artifactIds"], json!([artifact_id]));
    mcp_tool(
        &data_dir,
        "review_memory_candidate",
        json!({"memory_id":memory_id, "decision":"accepted"}),
    );
    let first_transfer = mcp_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id":memory_id,
            "sender_scope_type":"project",
            "sender_scope_id":project_id.to_string(),
            "receiver_scope_type":"scope",
            "receiver_scope_id":"phase1-m4-integration",
            "run_id":run_id,
            "retrieval_method":"hot"
        }),
    );
    assert_eq!(first_transfer["action"], json!("full"));
    assert_eq!(first_transfer["firstTimeForReceiver"], json!(true));
    let transfer_id = first_transfer["transferId"].as_i64().expect("transfer ID");
    let repeated_transfer = mcp_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id":memory_id,
            "sender_scope_type":"project",
            "sender_scope_id":project_id.to_string(),
            "receiver_scope_type":"scope",
            "receiver_scope_id":"phase1-m4-integration",
            "run_id":run_id,
            "retrieval_method":"hot"
        }),
    );
    assert_eq!(repeated_transfer["action"], json!("omitted_known"));
    assert!(repeated_transfer["transferId"].is_null());
    let finished = mcp_tool(
        &data_dir,
        "finish_run",
        json!({"run_id":run_id, "summary":"The combined MCP and Tauri state check passed."}),
    );
    assert_eq!(finished["status"], json!("verified_success"));

    let dashboard = invoke_dashboard(&data_dir);
    assert_eq!(dashboard.projects.len(), 1);
    assert_eq!(dashboard.projects[0].id, project_id);
    assert!(dashboard.projects[0].dirty);
    assert_eq!(
        dashboard.projects[0].changed_paths,
        vec!["changed-after-registration.txt".to_string()]
    );
    assert_eq!(dashboard.objectives[0].id, objective_id);
    assert_eq!(dashboard.recent_runs[0].id, run_id);
    assert_eq!(dashboard.recent_runs[0].status, "verified_success");
    assert!(dashboard.memory_health.hot_bytes > 0);
    assert!(dashboard.memory_health.hot_bytes <= 1024);
    assert!(dashboard.memory_health.largest_scope_hot_bytes <= 1024);
    assert_eq!(dashboard.memory_health.canonical_count, 1);
    assert!(dashboard.memory_health.context_transfer_bytes > 0);

    let project_after_restart =
        mcp_resource(&data_dir, &format!("cathulu://projects/{project_id}"));
    assert_eq!(project_after_restart["branch"], json!("main"));
    assert_eq!(project_after_restart["dirty"], json!(true));
    assert_eq!(
        project_after_restart["changedPaths"],
        json!(["changed-after-registration.txt"])
    );
    let run_after_restart = mcp_resource(&data_dir, &format!("cathulu://runs/{run_id}"));
    assert_eq!(run_after_restart["status"], json!("verified_success"));
    assert_eq!(
        run_after_restart["checks"].as_array().map(Vec::len),
        Some(2)
    );
    let memory_after_restart = mcp_resource(&data_dir, &format!("cathulu://memories/{memory_id}"));
    assert_eq!(
        memory_after_restart["sources"][0]["sourceId"],
        json!(decision_id.to_string())
    );
    assert_eq!(memory_after_restart["artifactIds"], json!([artifact_id]));
    let transfer_after_restart =
        mcp_resource(&data_dir, &format!("cathulu://transfers/{transfer_id}"));
    assert_eq!(transfer_after_restart["memoryId"], json!(memory_id));
    assert_eq!(transfer_after_restart["runId"], json!(run_id));
    assert_eq!(transfer_after_restart["firstTimeForReceiver"], json!(true));
    assert!(transfer_after_restart.get("body").is_none());

    println!("MCP registered project {project_id}, verified run {run_id}, memory {memory_id}, transfer {transfer_id}; the read-only Tauri dashboard returned the same SQLite state.");
}
