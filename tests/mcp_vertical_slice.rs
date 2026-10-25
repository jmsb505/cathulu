use cathulu_core::{telemetry::CodexHookEvent, Store};
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    std::env::current_dir()
        .expect("resolve repository root")
        .join("target")
        .join(format!("cathulu-{label}-{}-{stamp}", std::process::id()))
}

fn make_git_repo(path: &Path) {
    fs::create_dir_all(path).expect("create temp repository");
    run_git(path, &["init", "--quiet", "-b", "main"]);
    run_git(path, &["config", "user.name", "Cathulu Test"]);
    run_git(
        path,
        &["config", "user.email", "cathulu-test@example.invalid"],
    );
    fs::write(path.join("README.md"), "first commit\n").expect("write tracked file");
    run_git(path, &["add", "README.md"]);
    run_git(path, &["commit", "--quiet", "-m", "initial"]);
    fs::write(path.join("work.txt"), "untracked evidence\n").expect("write untracked file");
}

fn make_graphify_repo(path: &Path) -> String {
    fs::create_dir_all(path.join("src")).expect("create repository source folder");
    run_git(path, &["init", "--quiet", "-b", "main"]);
    run_git(path, &["config", "user.name", "Cathulu Test"]);
    run_git(
        path,
        &["config", "user.email", "cathulu-test@example.invalid"],
    );
    fs::write(
        path.join("src/lib.rs"),
        "pub fn caller() { middle(); }\npub fn middle() { callee(); }\npub fn callee() {}\n",
    )
    .expect("write source fixture");
    run_git(path, &["add", "src/lib.rs"]);
    run_git(path, &["commit", "--quiet", "-m", "source fixture"]);
    current_head(path)
}

fn graphify_document(head: &str) -> Value {
    json!({
        "directed": true,
        "multigraph": false,
        "graph": { "schema_version": 1, "graphify_version": "0.9.82" },
        "built_at_commit": head,
        "nodes": [
            { "id": "src_lib_caller", "label": "caller()", "file_type": "code", "source_file": "src/lib.rs", "source_location": "L1" },
            { "id": "src_lib_middle", "label": "middle()", "file_type": "code", "source_file": "src/lib.rs", "source_location": "L2" },
            { "id": "src_lib_callee", "label": "callee()", "file_type": "code", "source_file": "src/lib.rs", "source_location": "L3" }
        ],
        "links": [
            { "source": "src_lib_caller", "target": "src_lib_middle", "relation": "calls", "confidence": "EXTRACTED", "source_file": "src/lib.rs", "source_location": "L1" },
            { "source": "src_lib_middle", "target": "src_lib_callee", "relation": "calls", "confidence": "EXTRACTED", "source_file": "src/lib.rs", "source_location": "L2" }
        ]
    })
}

fn write_graphify_document(repo: &Path, document: &Value) {
    let graph_path = repo.join("graphify-out/graph.json");
    fs::create_dir_all(graph_path.parent().expect("Graphify output folder"))
        .expect("create Graphify output folder");
    fs::write(
        graph_path,
        serde_json::to_vec(document).expect("serialize Graphify fixture"),
    )
    .expect("write Graphify fixture");
}

fn trace_graphify_path(data_dir: &Path, project_id: i64) -> Value {
    tool_text(&call_tool(
        data_dir,
        "trace_code_path",
        json!({
            "project_id": project_id,
            "source_symbol": "caller",
            "target_symbol": "callee",
            "source_file": "src/lib.rs",
            "target_file": "src/lib.rs"
        }),
    ))
}

fn read_graphify_snapshot(data_dir: &Path, project_id: i64) -> Value {
    let responses = run_initialized_server(
        data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://projects/{project_id}/graphify-snapshot") }),
        )],
    );
    resource_text(response(&responses, 2))
}

fn run_git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("launch git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn run_server(data_dir: &Path, messages: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_cathulu-mcp"))
        .env("CATHULU_DATA_DIR", data_dir)
        .env("CATHULU_ROLE_DISPATCHER_ENABLED", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch Cathulu MCP server");
    {
        let stdin = child.stdin.as_mut().expect("child stdin");
        for message in messages {
            serde_json::to_writer(&mut *stdin, message).expect("serialize MCP request");
            stdin.write_all(b"\n").expect("terminate MCP request line");
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
        .map(|line| serde_json::from_str(line).expect("stdout contains only JSON-RPC lines"))
        .collect()
}

fn run_server_without_provider_credentials(data_dir: &Path, messages: &[Value]) -> Vec<Value> {
    let env_file = data_dir.join("provider-credentials-disabled.env");
    assert!(
        !env_file.exists(),
        "the isolated provider credential file must not exist"
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_cathulu-mcp"))
        .env("CATHULU_DATA_DIR", data_dir)
        .env("CATHULU_ROLE_DISPATCHER_ENABLED", "0")
        .env("CATHULU_ENV_FILE", env_file)
        .env_remove("CATHULU_DECISION_PROVIDER_API_KEY")
        .env_remove("CATHULU_CONTEXT_PROVIDER_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch Cathulu MCP server without provider credentials");
    {
        let stdin = child.stdin.as_mut().expect("child stdin");
        for message in messages {
            serde_json::to_writer(&mut *stdin, message).expect("serialize MCP request");
            stdin.write_all(b"\n").expect("terminate MCP request line");
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
        .map(|line| serde_json::from_str(line).expect("stdout contains only JSON-RPC lines"))
        .collect()
}

fn run_initialized_server(data_dir: &Path, requests: &[Value]) -> Vec<Value> {
    let mut messages = vec![
        request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "cathulu-test", "version": "0.1.0" }
            }),
        ),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ];
    messages.extend_from_slice(requests);
    run_server(data_dir, &messages)
}

fn run_initialized_server_without_provider_credentials(
    data_dir: &Path,
    requests: &[Value],
) -> Vec<Value> {
    let mut messages = vec![
        request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "cathulu-test", "version": "0.1.0" }
            }),
        ),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ];
    messages.extend_from_slice(requests);
    run_server_without_provider_credentials(data_dir, &messages)
}

fn response<'a>(responses: &'a [Value], id: u64) -> &'a Value {
    responses
        .iter()
        .find(|message| message.get("id") == Some(&json!(id)))
        .unwrap_or_else(|| panic!("missing response id {id}: {responses:?}"))
}

fn tool_call(id: u64, name: &str, arguments: Value) -> Value {
    request(
        id,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
}

fn tool_text(response: &Value) -> Value {
    assert_ne!(
        response.pointer("/result/isError"),
        Some(&json!(true)),
        "MCP tool call returned an error: {response}"
    );
    let text = response
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .expect("tool result has text content");
    serde_json::from_str(text).expect("tool result text is JSON")
}

fn resource_text(response: &Value) -> Value {
    let text = response
        .pointer("/result/contents/0/text")
        .and_then(Value::as_str)
        .expect("resource result has text content");
    serde_json::from_str(text).expect("resource text is JSON")
}

#[test]
fn register_reconcile_read_and_reopen_through_mcp() {
    let root = unique_temp_dir("round-trip");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);

    let discovery = run_initialized_server(
        &data_dir,
        &[
            request(2, "tools/list", json!({})),
            request(3, "resources/list", json!({})),
        ],
    );
    assert_eq!(
        response(&discovery, 1).pointer("/result/serverInfo/name"),
        Some(&json!("cathulu"))
    );
    let tools = response(&discovery, 2)
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("MCP tool list is an array");
    let tool_names = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(tool_names.len() >= 2);
    assert!(tool_names.contains(&"register_project"));
    assert!(tool_names.contains(&"reconcile_project"));
    assert!(!tool_names.iter().any(|name| name.contains("sql")));
    let register_tool = tools
        .iter()
        .find(|tool| tool.get("name") == Some(&json!("register_project")))
        .expect("register_project tool is discoverable");
    assert_eq!(
        register_tool.pointer("/inputSchema/properties/path/type"),
        Some(&json!("string"))
    );
    let reconcile_tool = tools
        .iter()
        .find(|tool| tool.get("name") == Some(&json!("reconcile_project")))
        .expect("reconcile_project tool is discoverable");
    assert_eq!(
        reconcile_tool.pointer("/inputSchema/properties/project_id/type"),
        Some(&json!("integer"))
    );
    let resource_uris = response(&discovery, 3)
        .pointer("/result/resources")
        .and_then(Value::as_array)
        .expect("MCP resource list is an array")
        .iter()
        .filter_map(|resource| resource["uri"].as_str())
        .collect::<Vec<_>>();
    for required in [
        "cathulu://projects",
        "cathulu://roles",
        "cathulu://dashboard",
        "cathulu://dispatches",
    ] {
        assert!(resource_uris.contains(&required), "missing {required}");
    }

    let registration = run_initialized_server(
        &data_dir,
        &[tool_call(2, "register_project", json!({ "path": repo }))],
    );
    let registered = tool_text(response(&registration, 2));
    assert_eq!(registered.get("branch"), Some(&json!("main")));
    assert_eq!(registered.get("dirty"), Some(&json!(true)));
    assert_eq!(registered.get("changedPaths"), Some(&json!(["work.txt"])));
    assert!(registered.get("head").and_then(Value::as_str).is_some());

    let project_id = registered
        .get("id")
        .and_then(Value::as_i64)
        .expect("registration returns a project ID");
    let role_catalog = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": "cathulu://roles" }),
        )],
    );
    let roles = resource_text(response(&role_catalog, 2));
    assert_eq!(roles.as_array().map(Vec::len), Some(2));
    assert_eq!(roles[0]["roleKey"], json!("main-overseer"));
    assert_eq!(
        roles[1]["roleKey"],
        json!(format!("project-manager:{project_id}"))
    );
    assert_eq!(
        roles[1]["memoryScopeKey"],
        json!(format!("project:{project_id}"))
    );

    let duplicate_registration = run_initialized_server(
        &data_dir,
        &[tool_call(2, "register_project", json!({ "path": repo }))],
    );
    let duplicate = tool_text(response(&duplicate_registration, 2));
    assert_eq!(duplicate.get("id"), Some(&json!(project_id)));
    assert_eq!(
        duplicate.get("registeredAt"),
        registered.get("registeredAt"),
        "duplicate registration preserves the original registration time"
    );

    let reconciliation = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "reconcile_project",
            json!({ "project_id": project_id }),
        )],
    );
    let reconciled = tool_text(response(&reconciliation, 2));
    assert_eq!(reconciled.get("branch"), Some(&json!("main")));
    assert_eq!(reconciled.get("dirty"), Some(&json!(true)));
    assert_eq!(reconciled.get("changedPaths"), Some(&json!(["work.txt"])));

    let resources = run_initialized_server(
        &data_dir,
        &[
            request(2, "resources/read", json!({ "uri": "cathulu://projects" })),
            request(
                3,
                "resources/read",
                json!({ "uri": format!("cathulu://projects/{project_id}") }),
            ),
        ],
    );
    let portfolio = resource_text(response(&resources, 2));
    assert_eq!(portfolio.as_array().map(Vec::len), Some(1));
    let project = resource_text(response(&resources, 3));
    assert_eq!(project.get("id"), Some(&json!(project_id)));
    assert_eq!(project.get("path"), Some(&json!(repo.to_string_lossy())));

    let restarted = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://projects/{project_id}") }),
        )],
    );
    let persisted = resource_text(response(&restarted, 2));
    assert_eq!(persisted.get("id"), Some(&json!(project_id)));
    assert_eq!(persisted.get("dirty"), Some(&json!(true)));
    assert_eq!(persisted.get("changedPaths"), Some(&json!(["work.txt"])));

    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn public_mcp_specialist_shadow_trials_skip_without_keys_and_preserve_run_state() {
    let root = unique_temp_dir("specialist-public-mcp");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);

    let project = tool_text(&call_tool_without_provider_credentials(
        &data_dir,
        "register_project",
        json!({ "path": repo }),
    ));
    let project_id = project["id"].as_i64().expect("registered project ID");
    let run = tool_text(&call_tool_without_provider_credentials(
        &data_dir,
        "start_run",
        json!({
            "project_id": project_id,
            "optimization_config": {
                "capabilities": {
                    "workflow_routing": {
                        "enabled": true,
                        "mode": "shadow",
                        "provider": "jev",
                        "model": "jev-test-model",
                        "providerLocality": "remote",
                        "remoteDataAllowed": true
                    },
                    "context_extract": {
                        "enabled": true,
                        "mode": "shadow",
                        "provider": "scaledown",
                        "model": "extract-test-model",
                        "providerLocality": "remote",
                        "remoteDataAllowed": true
                    }
                }
            }
        }),
    ));
    let run_id = run["id"].as_i64().expect("run ID");

    tool_text(&call_tool_without_provider_credentials(
        &data_dir,
        "record_quality_check",
        json!({
            "run_id": run_id,
            "check_key": "deterministic-qa",
            "mandatory": true,
            "status": "pending"
        }),
    ));

    let raw_log = "test harness output\nerror: fixture-only context extraction failure\n";
    let log_path = root.join("test-output.log");
    fs::write(&log_path, raw_log).expect("write local test artifact");
    let artifact = tool_text(&call_tool_without_provider_credentials(
        &data_dir,
        "record_evidence_artifact",
        json!({
            "run_id": run_id,
            "artifact_kind": "test_log",
            "uri": log_path
        }),
    ));
    let artifact_id = artifact["id"].as_i64().expect("artifact ID");

    let before_response = run_initialized_server_without_provider_credentials(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://runs/{run_id}") }),
        )],
    );
    let before = resource_text(response(&before_response, 2));
    assert_eq!(before["status"], json!("running"));
    assert_eq!(before["checks"][0]["status"], json!("pending"));

    let decision = tool_text(&call_tool_without_provider_credentials(
        &data_dir,
        "run_decision_shadow_trial",
        json!({
            "run_id": run_id,
            "invocation_id": "public-mcp-jev-missing-key",
            "capability": "workflow_routing"
        }),
    ));
    assert_eq!(decision["status"], json!("skipped"));

    let extraction = tool_text(&call_tool_without_provider_credentials(
        &data_dir,
        "run_context_extraction_shadow_trial",
        json!({
            "run_id": run_id,
            "invocation_id": "public-mcp-scaledown-missing-key",
            "artifact_ids": [artifact_id]
        }),
    ));
    assert_eq!(extraction["status"], json!("skipped"));

    let after_response = run_initialized_server_without_provider_credentials(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://runs/{run_id}") }),
            ),
            request(
                3,
                "resources/read",
                json!({ "uri": format!("cathulu://events/run/{run_id}") }),
            ),
        ],
    );
    let after = resource_text(response(&after_response, 2));
    assert_eq!(
        after, before,
        "shadow calls must leave run and QA state unchanged"
    );
    assert_eq!(
        fs::read(&log_path).expect("read original artifact"),
        raw_log.as_bytes()
    );

    let events = resource_text(response(&after_response, 3));
    let events = events.as_array().expect("run events are an array");
    let run_started = events
        .iter()
        .find(|event| event["eventType"] == json!("run_started"))
        .expect("run treatment is recorded");
    assert_eq!(
        run_started.pointer("/payload/optimizationTreatment/configuration/capabilities/workflow_routing/remoteDataAllowed"),
        Some(&json!(true))
    );
    let invocations = events
        .iter()
        .filter(|event| event["eventType"] == json!("optimization_invocation_recorded"))
        .collect::<Vec<_>>();
    assert_eq!(invocations.len(), 2);
    for (event, capability, provider) in [
        (&invocations[0], "workflow_routing", "jev"),
        (&invocations[1], "context_extract", "scaledown"),
    ] {
        assert_eq!(event["payload"]["capability"], json!(capability));
        assert_eq!(event["payload"]["provider"], json!(provider));
        assert_eq!(event["payload"]["providerLocality"], json!("remote"));
        assert_eq!(event["payload"]["remoteDataAllowed"], json!(true));
        assert_eq!(event["payload"]["mode"], json!("shadow"));
        assert_eq!(event["payload"]["status"], json!("skipped"));
        assert_eq!(event["payload"]["fallbackUsed"], json!(true));
        assert_eq!(event["payload"]["influencedExecution"], json!(false));
        assert_eq!(event["payload"]["payloadsPersisted"], json!(false));
    }
    assert!(!events
        .iter()
        .any(|event| event["eventType"] == json!("context_extraction_shadow_recorded")));

    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn graphify_path_returns_current_source_location_through_mcp() {
    let root = unique_temp_dir("graphify-path");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    let head = make_graphify_repo(&repo);
    write_graphify_document(&repo, &graphify_document(&head));

    let project_id = register_repo(&data_dir, &repo);
    let traced = trace_graphify_path(&data_dir, project_id);
    assert_eq!(traced["status"], json!("found"));
    assert_eq!(traced["sourceCommit"], json!(head));
    assert_eq!(traced["nodes"].as_array().map(Vec::len), Some(3));
    assert_eq!(traced["edges"].as_array().map(Vec::len), Some(2));
    assert_eq!(traced["edges"][0]["sourceFile"], json!("src/lib.rs"));
    assert_eq!(traced["edges"][0]["sourceLocation"], json!("L1"));
    assert_eq!(traced["edges"][0]["relation"], json!("calls"));
    assert_eq!(traced["edges"][0]["confidence"], json!("EXTRACTED"));
    let bounded = tool_text(&call_tool(
        &data_dir,
        "trace_code_path",
        json!({
            "project_id": project_id,
            "source_symbol": "caller",
            "target_symbol": "callee",
            "source_file": "src/lib.rs",
            "target_file": "src/lib.rs",
            "max_hops": 1
        }),
    ));
    assert_eq!(bounded["status"], json!("no_path"));
    let snapshot_id = traced["snapshotId"].as_str().expect("snapshot identity");
    assert_eq!(snapshot_id.len(), 64);

    let snapshot = read_graphify_snapshot(&data_dir, project_id);
    assert_eq!(snapshot["snapshotId"], json!(snapshot_id));
    assert_eq!(snapshot["status"], json!("available"));
    assert_eq!(
        trace_graphify_path(&data_dir, project_id)["snapshotId"],
        json!(snapshot_id)
    );

    fs::remove_dir_all(root).expect("remove Graphify test data");
}

#[test]
fn graphify_unavailable_and_stale_snapshots_fail_closed_through_mcp() {
    let root = unique_temp_dir("graphify-freshness");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    let head = make_graphify_repo(&repo);
    write_graphify_document(&repo, &graphify_document(&head));
    let project_id = register_repo(&data_dir, &repo);

    assert_eq!(
        trace_graphify_path(&data_dir, project_id)["status"],
        json!("found")
    );

    let mut stale = graphify_document(&head);
    stale["built_at_commit"] = json!("0".repeat(40));
    write_graphify_document(&repo, &stale);
    let stale_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(stale_result["status"], json!("unavailable"));
    assert_eq!(stale_result["reason"], json!("source_commit_mismatch"));
    assert!(stale_result["edges"]
        .as_array()
        .expect("edge list")
        .is_empty());
    assert_eq!(
        read_graphify_snapshot(&data_dir, project_id)["status"],
        json!("stale")
    );

    let mut unsupported = graphify_document(&head);
    unsupported["graph"]["schema_version"] = json!(2);
    write_graphify_document(&repo, &unsupported);
    let unsupported_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(unsupported_result["status"], json!("unavailable"));
    assert_eq!(
        unsupported_result["reason"],
        json!("unsupported_snapshot_version")
    );

    let mut unsupported_version = graphify_document(&head);
    unsupported_version["graph"]["graphify_version"] = json!("0.9.83");
    write_graphify_document(&repo, &unsupported_version);
    let unsupported_version_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(unsupported_version_result["status"], json!("unavailable"));
    assert_eq!(
        unsupported_version_result["reason"],
        json!("unsupported_snapshot_version")
    );

    write_graphify_document(&repo, &json!({ "not": "a Graphify snapshot" }));
    let malformed_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(malformed_result["status"], json!("unavailable"));
    assert_eq!(malformed_result["reason"], json!("malformed_snapshot"));
    let persisted = read_graphify_snapshot(&data_dir, project_id);
    assert_eq!(persisted["status"], json!("unavailable"));
    assert_eq!(persisted["statusReason"], json!("malformed_snapshot"));

    let mut empty = graphify_document(&head);
    empty["nodes"] = json!([]);
    empty["links"] = json!([]);
    write_graphify_document(&repo, &empty);
    let empty_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(empty_result["status"], json!("no_path"));
    assert!(empty_result["edges"]
        .as_array()
        .expect("edge list")
        .is_empty());
    assert_eq!(
        read_graphify_snapshot(&data_dir, project_id)["status"],
        json!("available")
    );

    let mut invalid_location = graphify_document(&head);
    invalid_location["nodes"][0]["source_location"] = json!("L99");
    write_graphify_document(&repo, &invalid_location);
    let location_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(location_result["status"], json!("unavailable"));
    assert_eq!(
        location_result["reason"],
        json!("source_location_out_of_range")
    );

    let mut outside_source = graphify_document(&head);
    outside_source["nodes"][0]["source_file"] = json!("../README.md");
    write_graphify_document(&repo, &outside_source);
    let outside_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(outside_result["status"], json!("unavailable"));
    assert_eq!(outside_result["reason"], json!("source_outside_repository"));

    write_graphify_document(&repo, &graphify_document(&head));
    assert_eq!(
        trace_graphify_path(&data_dir, project_id)["status"],
        json!("found")
    );
    fs::write(
        repo.join("src/lib.rs"),
        "pub fn caller() {}\npub fn middle() {}\npub fn callee() {}\n",
    )
    .expect("change indexed source");
    let changed_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(changed_result["status"], json!("unavailable"));
    assert_eq!(changed_result["reason"], json!("source_files_changed"));
    assert!(changed_result["edges"]
        .as_array()
        .expect("edge list")
        .is_empty());
    assert_eq!(
        read_graphify_snapshot(&data_dir, project_id)["status"],
        json!("stale")
    );

    run_git(&repo, &["checkout", "--", "src/lib.rs"]);
    assert_eq!(
        trace_graphify_path(&data_dir, project_id)["status"],
        json!("found")
    );
    fs::remove_file(repo.join("src/lib.rs")).expect("remove indexed source");
    let removed_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(removed_result["status"], json!("unavailable"));
    assert_eq!(removed_result["reason"], json!("source_files_changed"));
    assert!(removed_result["edges"]
        .as_array()
        .expect("edge list")
        .is_empty());

    fs::remove_file(repo.join("graphify-out/graph.json")).expect("remove Graphify snapshot");
    let missing_result = trace_graphify_path(&data_dir, project_id);
    assert_eq!(missing_result["status"], json!("unavailable"));
    assert_eq!(missing_result["reason"], json!("snapshot_missing"));

    fs::remove_dir_all(root).expect("remove Graphify freshness fixture");
}

#[test]
fn graphify_snapshot_changes_invalidate_only_explicitly_dependent_memory() {
    let root = unique_temp_dir("graphify-memory-freshness");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    let head = make_graphify_repo(&repo);
    write_graphify_document(&repo, &graphify_document(&head));
    let project_id = register_repo(&data_dir, &repo);
    let first_trace = trace_graphify_path(&data_dir, project_id);
    let old_snapshot_id = first_trace["snapshotId"]
        .as_str()
        .expect("initial Graphify snapshot ID");

    let dependent = create_memory(
        &data_dir,
        project_id,
        "Graph contract durability depends on source relationship evidence.",
        "lesson",
        "warm",
        vec![json!({ "source_type": "human_instruction", "source_id": "graphify-memory-test" })],
        json!({
            "repo_id": project_id,
            "git_commit": head,
            "graph_snapshot": old_snapshot_id
        }),
        0.9,
        false,
        None,
    );
    let dependent_id = dependent["id"]
        .as_i64()
        .expect("Graphify-dependent memory ID");
    let unchanged = create_memory(
        &data_dir,
        project_id,
        "Rollback procedure keeps database state recoverable.",
        "procedure",
        "warm",
        vec![json!({ "source_type": "human_instruction", "source_id": "non-graph-memory-test" })],
        json!({ "repo_id": project_id, "git_commit": head }),
        0.8,
        false,
        None,
    );
    let unchanged_id = unchanged["id"].as_i64().expect("non-Graphify memory ID");
    for memory_id in [dependent_id, unchanged_id] {
        tool_text(&call_tool(
            &data_dir,
            "review_memory_candidate",
            json!({ "memory_id": memory_id, "decision": "accepted" }),
        ));
    }

    let mut updated_snapshot = graphify_document(&head);
    updated_snapshot["pilot_revision"] = json!(2);
    write_graphify_document(&repo, &updated_snapshot);
    let second_trace = trace_graphify_path(&data_dir, project_id);
    assert_eq!(second_trace["status"], json!("found"));
    assert_ne!(second_trace["snapshotId"], json!(old_snapshot_id));

    let historical = read_memory(&data_dir, dependent_id);
    assert_eq!(
        historical["invalidationReason"],
        json!("graph_snapshot_changed")
    );
    assert!(historical["invalidatedAt"].as_i64().is_some());
    let unaffected = read_memory(&data_dir, unchanged_id);
    assert!(unaffected["invalidatedAt"].is_null());

    let old_search = tool_text(&call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project",
            "scope_id": project_id.to_string(),
            "query": "graph contract",
            "receiver_scope_type": "scope",
            "receiver_scope_id": "graphify-review",
            "limit": 10
        }),
    ));
    assert!(!old_search["warm"]
        .as_array()
        .expect("WARM candidates")
        .iter()
        .any(|candidate| candidate["memoryId"] == json!(dependent_id)));
    let current_search = tool_text(&call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project",
            "scope_id": project_id.to_string(),
            "query": "rollback procedure",
            "receiver_scope_type": "scope",
            "receiver_scope_id": "graphify-review",
            "limit": 10
        }),
    ));
    assert!(current_search["warm"]
        .as_array()
        .expect("WARM candidates")
        .iter()
        .any(|candidate| candidate["memoryId"] == json!(unchanged_id)));

    fs::remove_dir_all(root).expect("remove Graphify memory fixture");
}

#[test]
fn m1_project_rows_survive_additive_migrations_and_restart() {
    let root = unique_temp_dir("migration-preserves-project");
    let data_dir = root.join("data");
    fs::create_dir_all(&data_dir).expect("create data directory");
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("create version-one database");
    database
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
             INSERT INTO schema_migrations(version, applied_at) VALUES (1, 1);
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
             INSERT INTO projects(name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('legacy-project', 'C:/fixture/legacy-project', 'main', 'abc123', 1, '[\"src/lib.rs\"]', 10, 20);",
        )
        .expect("seed version-one project state");
    drop(database);

    let migrated = run_initialized_server(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": "cathulu://projects/1" }),
            ),
            request(3, "resources/read", json!({ "uri": "cathulu://dashboard" })),
            request(4, "resources/read", json!({ "uri": "cathulu://roles" })),
        ],
    );
    let project = resource_text(response(&migrated, 2));
    assert_eq!(project.get("id"), Some(&json!(1)));
    assert_eq!(project.get("name"), Some(&json!("legacy-project")));
    assert_eq!(project.get("head"), Some(&json!("abc123")));
    assert_eq!(project.get("changedPaths"), Some(&json!(["src/lib.rs"])));
    let dashboard = resource_text(response(&migrated, 3));
    assert_eq!(dashboard.pointer("/projects/0/id"), Some(&json!(1)));
    assert_eq!(dashboard.pointer("/memoryHealth/hotBytes"), Some(&json!(0)));
    let roles = resource_text(response(&migrated, 4));
    assert_eq!(roles.as_array().map(Vec::len), Some(2));
    assert_eq!(roles[0]["roleKey"], json!("main-overseer"));
    assert_eq!(roles[1]["roleKey"], json!("project-manager:1"));
    assert_eq!(dashboard["logicalRoles"], roles);
    let migrated_database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open migrated database");
    let schema_version: i64 = migrated_database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read migrated schema version");
    assert_eq!(schema_version, 20);
    migrated_database
        .prepare("SELECT external_event_id FROM events LIMIT 0")
        .expect("version-three external event identity column");
    migrated_database
        .prepare("SELECT run_id FROM codex_turn_runs LIMIT 0")
        .expect("version-three Codex turn mapping");
    migrated_database
        .prepare("SELECT rowid FROM memory_fts LIMIT 0")
        .expect("version-four FTS5 memory index");
    migrated_database
        .prepare("SELECT minimum_support FROM memory_maintenance_proposals LIMIT 0")
        .expect("deterministic gardener proposals");
    migrated_database
        .prepare("SELECT extraction_receipt_id FROM evidence_gc_ledger LIMIT 0")
        .expect("reference-aware evidence collection ledger");
    migrated_database
        .prepare("SELECT policy_id FROM memory_policy_evaluations LIMIT 0")
        .expect("versioned memory policy evaluations");
    migrated_database
        .prepare("SELECT packet_hash, idempotency_key_hash FROM delegations LIMIT 0")
        .expect("version-five delegation records");
    migrated_database
        .prepare("SELECT memory_id FROM delegation_memory_refs LIMIT 0")
        .expect("delegation memory references");
    migrated_database
        .prepare("SELECT artifact_id FROM delegation_artifact_refs LIMIT 0")
        .expect("delegation evidence references");
    migrated_database
        .prepare("SELECT base_commit, observed_head FROM delegation_worktree_states LIMIT 0")
        .expect("version-seven worktree baseline and reconciliation state");
    migrated_database
        .prepare("SELECT agent_id_hash, model_slug FROM codex_subagent_hook_events LIMIT 0")
        .expect("version-seven privacy-preserving subagent hook metadata");
    migrated_database
        .prepare("SELECT delegation_id, status FROM role_dispatch_outbox LIMIT 0")
        .expect("version-fourteen durable role dispatch queue");
    migrated_database
        .prepare("SELECT capability_hash FROM role_thread_capabilities LIMIT 0")
        .expect("version-sixteen scoped role capabilities");
    migrated_database
        .prepare("SELECT source_event_id, status FROM role_turn_inbox LIMIT 0")
        .expect("version-seventeen durable role event inbox");
    migrated_database
        .prepare("SELECT role_key, capability_hash FROM main_overseer_capability LIMIT 0")
        .expect("version-eighteen global Main capability");
    drop(migrated_database);
    fs::remove_dir_all(root).expect("remove migration fixture");
}

#[test]
fn workflow_runs_require_every_declared_mandatory_check_and_keep_an_event_journal() {
    let root = unique_temp_dir("workflow-quality-gates");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    let registered = run_initialized_server(
        &data_dir,
        &[tool_call(2, "register_project", json!({ "path": repo }))],
    );
    let project_id = tool_text(response(&registered, 2))["id"]
        .as_i64()
        .expect("registered project ID");
    let atomicity_database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open database for atomic objective assertion");
    atomicity_database
        .execute_batch(
            "CREATE TRIGGER reject_objective_event BEFORE INSERT ON events
             WHEN NEW.event_type = 'objective_created'
             BEGIN SELECT RAISE(ABORT, 'forced event failure'); END;",
        )
        .expect("install temporary event failure trigger");
    let rolled_back_objective = call_tool(
        &data_dir,
        "create_objective",
        json!({ "project_id": project_id, "title": "must roll back" }),
    );
    assert_eq!(
        rolled_back_objective.pointer("/result/isError"),
        Some(&json!(true))
    );
    let objective_count: i64 = atomicity_database
        .query_row(
            "SELECT COUNT(*) FROM objectives WHERE title = 'must roll back'",
            [],
            |row| row.get(0),
        )
        .expect("query rolled back objective state");
    assert_eq!(
        objective_count, 0,
        "state and event insertion roll back together"
    );
    atomicity_database
        .execute_batch("DROP TRIGGER reject_objective_event")
        .expect("remove temporary event failure trigger");
    drop(atomicity_database);
    let objective_result = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "create_objective",
            json!({
                "project_id": project_id,
                "title": "Ship a verified slice",
                "description": "Keep the work auditable."
            }),
        )],
    );
    let objective_id = tool_text(response(&objective_result, 2))["id"]
        .as_i64()
        .expect("objective ID");
    let decision_result = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "record_decision",
            json!({
                "project_id": project_id,
                "objective_id": objective_id,
                "title": "Keep SQLite authoritative",
                "body": "Memory projections retain provenance and do not replace domain records."
            }),
        )],
    );
    let decision_id = tool_text(response(&decision_result, 2))["id"]
        .as_i64()
        .expect("decision ID");
    let decision_resource = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://decisions/{decision_id}") }),
        )],
    );
    assert_eq!(
        resource_text(response(&decision_resource, 2))["title"],
        json!("Keep SQLite authoritative")
    );
    let task_result = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "create_task",
            json!({
                "objective_id": objective_id,
                "title": "Validate the release",
                "description": "Run the required checks."
            }),
        )],
    );
    let task_id = tool_text(response(&task_result, 2))["id"]
        .as_i64()
        .expect("task ID");
    let task_resource = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://tasks/{task_id}") }),
        )],
    );
    assert_eq!(
        resource_text(response(&task_resource, 2))["status"],
        json!("queued")
    );
    let workflow_result = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "create_workflow_version",
            json!({
                "workflow_key": "windows_release",
                "name": "Windows release",
                "version": 1,
                "definition": { "mandatory_checks": ["format", "tests"] }
            }),
        )],
    );
    let workflow_version_id = tool_text(response(&workflow_result, 2))["id"]
        .as_i64()
        .expect("workflow version ID");
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open migration store for immutability assertions");
    assert!(database
        .execute(
            "UPDATE decisions SET body = 'changed' WHERE id = ?1",
            [decision_id]
        )
        .is_err());
    assert!(database
        .execute(
            "UPDATE workflow_versions SET definition_json = '{}' WHERE id = ?1",
            [workflow_version_id]
        )
        .is_err());

    let blocked_run = start_test_run(
        &data_dir,
        project_id,
        objective_id,
        task_id,
        workflow_version_id,
    );
    record_check(&data_dir, blocked_run, "format", true, "passed");
    record_project_memory_review_for_test(&data_dir, blocked_run);
    database
        .execute_batch(
            "CREATE TRIGGER reject_finish_event BEFORE INSERT ON events
             WHEN NEW.event_type = 'run_finished'
             BEGIN SELECT RAISE(ABORT, 'forced finish event failure'); END;",
        )
        .expect("install temporary run event failure trigger");
    let failed_finish = call_tool(
        &data_dir,
        "finish_run",
        json!({ "run_id": blocked_run, "summary": "This finish must roll back." }),
    );
    assert_eq!(failed_finish.pointer("/result/isError"), Some(&json!(true)));
    let uncommitted_run = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://runs/{blocked_run}") }),
        )],
    );
    assert_eq!(
        resource_text(response(&uncommitted_run, 2))["status"],
        json!("running")
    );
    database
        .execute_batch("DROP TRIGGER reject_finish_event")
        .expect("remove temporary run event failure trigger");
    let blocked = finish_test_run(&data_dir, blocked_run);
    assert_eq!(blocked.get("status"), Some(&json!("blocked")));
    let blocked_events = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://events/run/{blocked_run}") }),
        )],
    );
    let events = resource_text(response(&blocked_events, 2));
    let event_types = events
        .as_array()
        .expect("run event list")
        .iter()
        .filter_map(|event| event["eventType"].as_str())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"run_started"));
    assert!(event_types.contains(&"quality_check_recorded"));
    assert!(event_types.contains(&"run_finished"));
    let event_id = events.as_array().unwrap()[0]["id"].as_i64().unwrap();
    assert!(database
        .execute(
            "UPDATE events SET event_type = 'forged' WHERE id = ?1",
            [event_id]
        )
        .is_err());
    assert!(database
        .execute("DELETE FROM events WHERE id = ?1", [event_id])
        .is_err());
    drop(database);

    let failed_run = start_test_run(
        &data_dir,
        project_id,
        objective_id,
        task_id,
        workflow_version_id,
    );
    record_check(&data_dir, failed_run, "format", true, "passed");
    record_check(&data_dir, failed_run, "tests", true, "failed");
    assert_eq!(
        finish_test_run(&data_dir, failed_run).get("status"),
        Some(&json!("failed"))
    );

    let verified_run = start_test_run(
        &data_dir,
        project_id,
        objective_id,
        task_id,
        workflow_version_id,
    );
    record_check(&data_dir, verified_run, "format", true, "passed");
    record_check(&data_dir, verified_run, "tests", true, "passed");
    assert_eq!(
        finish_test_run(&data_dir, verified_run).get("status"),
        Some(&json!("verified_success"))
    );

    let dashboard = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": "cathulu://dashboard" }),
        )],
    );
    let snapshot = resource_text(response(&dashboard, 2));
    assert_eq!(
        snapshot.pointer("/objectives/0/title"),
        Some(&json!("Ship a verified slice"))
    );
    assert_eq!(
        snapshot.pointer("/recentRuns/0/status"),
        Some(&json!("verified_success"))
    );
    fs::remove_dir_all(root).expect("remove workflow fixture");
}

#[test]
fn memory_identity_provenance_known_by_freshness_and_hot_budget_are_preserved() {
    let root = unique_temp_dir("memory-foundation");
    let repo = root.join("repo");
    let unrelated_repo = root.join("unrelated");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    make_git_repo(&unrelated_repo);
    let first_project = register_repo(&data_dir, &repo);
    let second_project = register_repo(&data_dir, &unrelated_repo);
    let dependency = tool_text(&call_tool(
        &data_dir,
        "record_dependency",
        json!({ "from_project_id": first_project, "to_project_id": second_project, "relation": "consumes" }),
    ));
    let dependency_id = dependency["id"].as_i64().expect("dependency ID");
    let dependency_resource = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://dependencies/{dependency_id}") }),
        )],
    );
    assert_eq!(
        resource_text(response(&dependency_resource, 2))["relation"],
        json!("consumes")
    );

    let missing_provenance = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": first_project,
            "memory_class": "canonical", "temperature": "hot", "kind": "contract",
            "body": "C consumes Contract V3", "importance": 0.9, "confidence": 0.9
        }),
    );
    assert_eq!(
        missing_provenance.pointer("/result/isError"),
        Some(&json!(true))
    );

    let initial = create_memory(
        &data_dir,
        first_project,
        "C consumes Contract V2",
        "contract",
        "hot",
        vec![json!({ "source_type": "human_instruction", "source_id": "owner-decision-1" })],
        json!({ "repo_id": first_project, "git_commit": current_head(&repo) }),
        0.9,
        false,
        None,
    );
    let initial_id = initial["id"].as_i64().expect("stable memory ID");
    let initial_hash = initial["contentHash"]
        .as_str()
        .expect("content hash")
        .to_string();
    assert_eq!(initial_hash.len(), 64);
    assert_eq!(initial["reviewState"], json!("candidate"));
    assert_eq!(initial["temperature"], json!("hot"));
    let accepted = call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": initial_id, "decision": "accepted" }),
    );
    assert_eq!(tool_text(&accepted)["reviewState"], json!("accepted"));

    let first_transfer = transfer_memory(&data_dir, initial_id, first_project, "scope_A-C", "hot");
    assert_eq!(first_transfer["action"], json!("full"));
    assert_eq!(first_transfer["firstTimeForReceiver"], json!(true));
    let transfer_id = first_transfer["transferId"]
        .as_i64()
        .expect("transfer ledger ID");
    let transfer_resource = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://transfers/{transfer_id}") }),
        )],
    );
    let transfer_record = resource_text(response(&transfer_resource, 2));
    assert_eq!(transfer_record["memoryId"], json!(initial_id));
    assert_eq!(transfer_record["contentHash"], json!(initial_hash));
    assert_eq!(transfer_record["firstTimeForReceiver"], json!(true));
    assert_eq!(
        transfer_record["receiverScopeKey"],
        json!("scope:scope_A-C")
    );
    assert!(
        transfer_record.get("body").is_none(),
        "transfer ledger stores metadata, not context bodies"
    );
    let known_transfer = transfer_memory(&data_dir, initial_id, first_project, "scope_A-C", "hot");
    assert_eq!(known_transfer["action"], json!("omitted_known"));
    assert!(known_transfer["transferId"].is_null());

    let warmed = call_tool(
        &data_dir,
        "set_memory_temperature",
        json!({ "memory_id": initial_id, "temperature": "warm" }),
    );
    let warmed = tool_text(&warmed);
    assert_eq!(warmed["id"], json!(initial_id));
    assert_eq!(warmed["memoryClass"], json!("canonical"));
    assert_eq!(warmed["contentHash"], json!(initial_hash));
    assert_eq!(warmed["temperature"], json!("warm"));

    let successor = create_memory(
        &data_dir,
        first_project,
        "C consumes Contract V3",
        "contract",
        "warm",
        vec![json!({ "source_type": "human_instruction", "source_id": "owner-decision-2" })],
        json!({ "repo_id": first_project, "git_commit": current_head(&repo) }),
        0.95,
        false,
        Some(initial_id),
    );
    let successor_id = successor["id"].as_i64().expect("successor memory ID");
    assert_eq!(successor["supersedesId"], json!(initial_id));
    let historical_predecessor = read_memory(&data_dir, initial_id);
    assert_eq!(
        historical_predecessor["supersededById"],
        json!(successor_id)
    );
    assert!(historical_predecessor["validUntil"].as_i64().is_some());
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": successor_id, "decision": "accepted" }),
    );
    let delta = transfer_memory(&data_dir, successor_id, first_project, "scope_A-C", "sql");
    assert_eq!(delta["action"], json!("delta"));
    assert_eq!(delta["replacesMemoryId"], json!(initial_id));
    let current_records = tool_text(&call_tool(
        &data_dir,
        "read_memories",
        json!({ "scope_type": "project", "scope_id": first_project.to_string() }),
    ));
    assert!(current_records
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["id"] == json!(successor_id)));
    assert!(!current_records
        .as_array()
        .unwrap()
        .iter()
        .any(|record| record["id"] == json!(initial_id)));
    let historical_records = tool_text(&call_tool(
        &data_dir,
        "read_memories",
        json!({ "scope_type": "project", "scope_id": first_project.to_string(), "include_history": true, "include_cold": true }),
    ));
    let historical_ids = historical_records
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|record| record["id"].as_i64())
        .collect::<Vec<_>>();
    assert!(historical_ids.contains(&initial_id));
    assert!(historical_ids.contains(&successor_id));

    let duplicate = create_memory(
        &data_dir,
        first_project,
        "C consumes Contract V3",
        "contract",
        "cold",
        vec![json!({ "source_type": "human_instruction", "source_id": "owner-decision-2b" })],
        json!({}),
        0.8,
        false,
        None,
    );
    assert_eq!(duplicate["id"], json!(successor_id));
    assert_eq!(
        duplicate["temperature"],
        json!("warm"),
        "a duplicate does not mutate temperature or identity"
    );

    let second_memory = create_memory(
        &data_dir,
        first_project,
        "A low-priority note that can move to WARM when the scope budget is full.",
        "lesson",
        "hot",
        vec![json!({ "source_type": "human_instruction", "source_id": "budget-note-1" })],
        json!({}),
        0.1,
        false,
        None,
    );
    let second_memory_id = second_memory["id"].as_i64().expect("memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": second_memory_id, "decision": "accepted" }),
    );
    call_tool(
        &data_dir,
        "set_memory_hot_budget",
        json!({ "scope_type": "project", "scope_id": first_project.to_string(), "hot_budget_bytes": 100 }),
    );
    let low = create_memory(
        &data_dir,
        first_project,
        &"l".repeat(80),
        "summary",
        "hot",
        vec![json!({ "source_type": "human_instruction", "source_id": "budget-note-2" })],
        json!({}),
        0.1,
        false,
        None,
    );
    let low_id = low["id"].as_i64().expect("low-priority ID");
    let high = create_memory(
        &data_dir,
        first_project,
        &"h".repeat(80),
        "summary",
        "hot",
        vec![json!({ "source_type": "human_instruction", "source_id": "budget-note-3" })],
        json!({}),
        0.9,
        false,
        None,
    );
    let high_id = high["id"].as_i64().expect("high-priority ID");
    let low_after_budget = read_memory(&data_dir, low_id);
    let high_after_budget = read_memory(&data_dir, high_id);
    assert_eq!(low_after_budget["temperature"], json!("warm"));
    assert_eq!(high_after_budget["temperature"], json!("hot"));
    assert_eq!(
        low_after_budget["id"],
        json!(low_id),
        "demotion preserves durable identity"
    );

    let remote_artifact = call_tool(
        &data_dir,
        "record_evidence_artifact",
        json!({ "artifact_kind": "remote", "uri": "https://example.invalid/log.txt" }),
    );
    assert_eq!(
        remote_artifact.pointer("/result/isError"),
        Some(&json!(true))
    );
    let artifact_result = call_tool(
        &data_dir,
        "record_evidence_artifact",
        json!({
            "artifact_kind": "failure_log",
            "uri": "artifact://incident-9/test.log",
            "content_hash": "sha256:incident9",
            "protected": true
        }),
    );
    let artifact = tool_text(&artifact_result);
    let artifact_id = artifact["id"].as_i64().expect("artifact ID");
    assert_eq!(artifact["protected"], json!(true));
    let artifact_resource = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://artifacts/{artifact_id}") }),
        )],
    );
    assert_eq!(
        resource_text(response(&artifact_resource, 2))["protected"],
        json!(true)
    );
    let cold_result = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": first_project.to_string(),
            "memory_class": "canonical", "temperature": "cold", "kind": "failure_pattern",
            "body": "A protected failure artifact remains cold until an audit asks for it.",
            "importance": 1.0, "confidence": 0.9, "active": false, "protected": true,
            "sources": [{ "source_type": "human_instruction", "source_id": "incident-note-9" }],
            "artifact_ids": [artifact_id]
        }),
    );
    let cold = tool_text(&cold_result);
    let cold_id = cold["id"].as_i64().expect("cold memory ID");
    assert_eq!(cold["artifactIds"], json!([artifact_id]));
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": cold_id, "decision": "accepted" }),
    );
    assert_eq!(
        transfer_memory(&data_dir, cold_id, first_project, "scope_A-C", "manual")["action"],
        json!("excluded_cold")
    );
    assert!(database_artifact_unprotect_is_rejected(
        &data_dir,
        artifact_id
    ));

    let unrelated = create_memory(
        &data_dir,
        second_project,
        "Unrelated repository is pinned to Contract V1.",
        "contract",
        "warm",
        vec![json!({ "source_type": "human_instruction", "source_id": "owner-decision-3" })],
        json!({ "repo_id": second_project, "git_commit": current_head(&unrelated_repo) }),
        0.7,
        false,
        None,
    );
    let unrelated_id = unrelated["id"].as_i64().expect("unrelated memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": unrelated_id, "decision": "accepted" }),
    );
    fs::write(repo.join("README.md"), "second commit\n").expect("change tracked file");
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "--quiet", "-m", "second"]);
    let reconciliation = call_tool(
        &data_dir,
        "reconcile_project",
        json!({ "project_id": first_project }),
    );
    assert_eq!(tool_text(&reconciliation)["id"], json!(first_project));
    assert_eq!(
        read_memory(&data_dir, successor_id)["invalidationReason"],
        json!("git_commit_changed")
    );
    assert!(read_memory(&data_dir, unrelated_id)["invalidatedAt"].is_null());

    let current = call_tool(
        &data_dir,
        "read_memories",
        json!({ "scope_type": "project", "scope_id": first_project.to_string(), "include_history": false, "include_cold": false }),
    );
    let current_records = tool_text(&current);
    let current_ids = current_records
        .as_array()
        .expect("current memory list")
        .iter()
        .filter_map(|record| record["id"].as_i64())
        .collect::<Vec<_>>();
    assert!(
        !current_ids.contains(&successor_id),
        "stale or superseded records are not current"
    );
    assert!(
        !current_ids.contains(&cold_id),
        "cold memory is not automatically retrieved"
    );
    assert!(
        !current_ids.contains(&low_id),
        "HOT budget demotion preserves WARM memory but candidates stay out of activation"
    );

    fs::remove_dir_all(root).expect("remove memory fixture");
}

#[test]
fn memory_retrieval_prefers_hot_filters_history_and_marks_known_warm_candidates() {
    let root = unique_temp_dir("memory-retrieval");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    let project_id = register_repo(&data_dir, &repo);

    let hot = tool_text(&call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": project_id.to_string(),
            "memory_class": "canonical", "temperature": "hot", "kind": "contract",
            "body": "Current state: C depends on A.ContractV3.",
            "sources": [{ "source_type": "human_instruction", "source_id": "m4-hot" }],
            "importance": 1.0, "confidence": 1.0, "active": true
        }),
    ));
    let hot_id = hot["id"].as_i64().expect("HOT memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": hot_id, "decision": "accepted" }),
    );

    let warm = create_memory(
        &data_dir,
        project_id,
        "Migration timeout follows schema mismatch in migration tools.",
        "lesson",
        "warm",
        vec![json!({ "source_type": "human_instruction", "source_id": "m4-warm" })],
        json!({}),
        0.8,
        false,
        None,
    );
    let warm_id = warm["id"].as_i64().expect("WARM memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": warm_id, "decision": "accepted" }),
    );

    let cold = create_memory(
        &data_dir,
        project_id,
        "A cold migration timeout record mentions schema mismatch.",
        "lesson",
        "cold",
        vec![json!({ "source_type": "human_instruction", "source_id": "m4-cold" })],
        json!({}),
        0.7,
        false,
        None,
    );
    let cold_id = cold["id"].as_i64().expect("COLD memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": cold_id, "decision": "accepted" }),
    );

    let stale = tool_text(&call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": project_id.to_string(),
            "memory_class": "episodic", "temperature": "warm", "kind": "lesson",
            "body": "Historical migration timeout followed a schema mismatch.",
            "sources": [{ "source_type": "human_instruction", "source_id": "m4-stale" }],
            "valid_until": 1, "importance": 0.5, "confidence": 0.7
        }),
    ));
    let stale_id = stale["id"].as_i64().expect("stale memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": stale_id, "decision": "accepted" }),
    );

    let search = tool_text(&call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": project_id.to_string(),
            "query": "migration timeout", "receiver_scope_type": "scope",
            "receiver_scope_id": "Scope_M4", "limit": 10
        }),
    ));
    let hot_ids = search["hot"].as_array().expect("HOT candidate list");
    let warm_ids = search["warm"].as_array().expect("WARM candidate list");
    assert_eq!(
        hot_ids.first().and_then(|row| row["memoryId"].as_i64()),
        Some(hot_id)
    );
    assert_eq!(
        warm_ids.first().and_then(|row| row["memoryId"].as_i64()),
        Some(warm_id)
    );
    assert!(!warm_ids.iter().any(|row| row["memoryId"] == json!(cold_id)));
    assert!(!warm_ids
        .iter()
        .any(|row| row["memoryId"] == json!(stale_id)));
    assert!(warm_ids.first().and_then(|row| row.get("body")).is_none());

    let transfer = transfer_memory(&data_dir, warm_id, project_id, "Scope_M4", "fts");
    assert_eq!(transfer["action"], json!("full"));
    let refreshed_search = tool_text(&call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": project_id.to_string(),
            "query": "migration timeout", "receiver_scope_type": "scope",
            "receiver_scope_id": "Scope_M4", "limit": 10
        }),
    ));
    assert!(refreshed_search["alreadyKnown"]
        .as_array()
        .expect("known memories")
        .iter()
        .any(|row| row["memoryId"] == json!(warm_id)));
    assert!(!refreshed_search["warm"]
        .as_array()
        .expect("WARM candidates")
        .iter()
        .any(|row| row["memoryId"] == json!(warm_id)));

    fs::remove_dir_all(root).expect("remove retrieval fixture");
}

#[test]
fn memory_consolidation_creates_a_provenance_linked_candidate_without_removing_episodes() {
    let root = unique_temp_dir("memory-consolidation");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    let project_id = register_repo(&data_dir, &repo);

    let first = tool_text(&call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": project_id.to_string(),
            "memory_class": "episodic", "temperature": "warm", "kind": "failure_pattern",
            "body": "Migration run 1 timed out after Contract V2 changed.",
            "sources": [{ "source_type": "test_result", "source_id": "run-1-check" }],
            "importance": 0.6, "confidence": 0.8
        }),
    ));
    let first_id = first["id"].as_i64().expect("first episode ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": first_id, "decision": "accepted" }),
    );
    let second = tool_text(&call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": project_id.to_string(),
            "memory_class": "episodic", "temperature": "warm", "kind": "failure_pattern",
            "body": "Migration run 2 timed out after Contract V2 changed.",
            "sources": [{ "source_type": "test_result", "source_id": "run-2-check" }],
            "importance": 0.7, "confidence": 0.85
        }),
    ));
    let second_id = second["id"].as_i64().expect("second episode ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": second_id, "decision": "accepted" }),
    );

    let consolidated = tool_text(&call_tool(
        &data_dir,
        "consolidate_memories",
        json!({
            "scope_type": "project", "scope_id": project_id.to_string(),
            "source_memory_ids": [first_id, second_id],
            "body": "Contract V2 changes can cause the migration to time out.",
            "kind": "failure_pattern", "importance": 0.9, "confidence": 0.9
        }),
    ));
    let consolidated_id = consolidated["id"].as_i64().expect("consolidated memory ID");
    assert_eq!(consolidated["memoryClass"], json!("canonical"));
    assert_eq!(consolidated["reviewState"], json!("candidate"));
    let sources = consolidated["sources"]
        .as_array()
        .expect("consolidated provenance");
    assert!(sources
        .iter()
        .any(|source| source["sourceType"] == json!("memory")
            && source["sourceId"] == json!(first_id.to_string())));
    assert!(sources
        .iter()
        .any(|source| source["sourceType"] == json!("memory")
            && source["sourceId"] == json!(second_id.to_string())));
    assert!(sources
        .iter()
        .any(|source| source["sourceId"] == json!("run-1-check")));
    assert!(sources
        .iter()
        .any(|source| source["sourceId"] == json!("run-2-check")));
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": consolidated_id, "decision": "accepted" }),
    );
    assert_eq!(read_memory(&data_dir, first_id)["id"], json!(first_id));
    assert_eq!(read_memory(&data_dir, second_id)["id"], json!(second_id));
    assert_eq!(
        read_memory(&data_dir, consolidated_id)["reviewState"],
        json!("accepted")
    );

    fs::remove_dir_all(root).expect("remove consolidation fixture");
}

fn start_test_run(
    data_dir: &Path,
    project_id: i64,
    objective_id: i64,
    task_id: i64,
    workflow_version_id: i64,
) -> i64 {
    let response = call_tool(
        data_dir,
        "start_run",
        json!({
            "project_id": project_id,
            "objective_id": objective_id,
            "task_id": task_id,
            "workflow_version_id": workflow_version_id
        }),
    );
    tool_text(&response)["id"].as_i64().expect("run ID")
}

fn record_check(data_dir: &Path, run_id: i64, key: &str, mandatory: bool, status: &str) {
    let response = call_tool(
        data_dir,
        "record_quality_check",
        json!({
            "run_id": run_id,
            "check_key": key,
            "mandatory": mandatory,
            "status": status,
            "details": { "source": "test" }
        }),
    );
    tool_text(&response);
}

fn finish_test_run(data_dir: &Path, run_id: i64) -> Value {
    record_project_memory_review_for_test(data_dir, run_id);
    let response = call_tool(
        data_dir,
        "finish_run",
        json!({ "run_id": run_id, "summary": "Finished test run." }),
    );
    tool_text(&response)
}

fn record_project_memory_review_for_test(data_dir: &Path, run_id: i64) {
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open test store for project memory review fixture");
    let existing_review: Option<String> = database
        .query_row(
            "SELECT payload_json FROM events
             WHERE aggregate_type = 'run' AND aggregate_id = ?1
               AND event_type = 'project_memory_reviewed' ORDER BY id LIMIT 1",
            [run_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .expect("query existing memory review");
    if let Some(payload) = existing_review {
        let review: Value = serde_json::from_str(&payload).expect("decode existing review");
        tool_text(&call_tool(
            data_dir,
            "record_project_memory_review",
            json!({
                "run_id": run_id,
                "disposition": review["disposition"],
                "reason_code": review["reasonCode"],
                "memory_ids": review["memoryIds"]
            }),
        ));
        return;
    }
    let project_id: i64 = database
        .query_row(
            "SELECT project_id FROM runs WHERE id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .expect("read run project ID");
    let project_scope = format!("project:{project_id}");
    let existing_profile: Option<i64> = database
        .query_row(
            "SELECT id FROM memories
             WHERE scope_key = ?1 AND memory_class = 'canonical' AND kind = 'summary'
               AND review_state = 'accepted' AND temperature IN ('hot', 'warm')
               AND superseded_by_id IS NULL AND invalidated_at IS NULL
             ORDER BY id DESC LIMIT 1",
            [&project_scope],
            |row| row.get(0),
        )
        .optional()
        .expect("query project summary");
    let (disposition, reason_code, memory_ids) = if existing_profile.is_some() {
        (
            "no_durable_change",
            "task_produced_no_reusable_knowledge",
            Vec::new(),
        )
    } else {
        let created = tool_text(&call_tool(
            data_dir,
            "create_memory",
            json!({
                "scope_type": "project",
                "scope_id": project_id.to_string(),
                "memory_class": "canonical",
                "temperature": "hot",
                "kind": "summary",
                "body": "This local repository is a fixture for the Cathulu MCP vertical-slice tests.",
                "sources": [
                    { "source_type": "project", "source_id": project_id.to_string() },
                    { "source_type": "source_file", "source_id": "README.md" }
                ],
                "artifact_ids": [],
                "importance": 0.9,
                "confidence": 0.95,
                "active": true,
                "protected": false
            }),
        ));
        let memory_id = created["id"].as_i64().expect("created profile ID");
        tool_text(&call_tool(
            data_dir,
            "review_memory_candidate",
            json!({ "memory_id": memory_id, "decision": "accepted" }),
        ));
        (
            "knowledge_saved",
            "project_profile_initialized",
            vec![memory_id],
        )
    };
    tool_text(&call_tool(
        data_dir,
        "record_project_memory_review",
        json!({
            "run_id": run_id,
            "disposition": disposition,
            "reason_code": reason_code,
            "memory_ids": memory_ids
        }),
    ));
}

fn register_repo(data_dir: &Path, repo: &Path) -> i64 {
    let response = call_tool(data_dir, "register_project", json!({ "path": repo }));
    tool_text(&response)["id"].as_i64().expect("project ID")
}

fn create_task_for_project(data_dir: &Path, project_id: i64, label: &str) -> (i64, i64) {
    let objective = tool_text(&call_tool(
        data_dir,
        "create_objective",
        json!({ "project_id": project_id, "title": format!("{label} objective") }),
    ));
    let objective_id = objective["id"].as_i64().expect("objective ID");
    let task = tool_text(&call_tool(
        data_dir,
        "create_task",
        json!({ "objective_id": objective_id, "title": format!("{label} task") }),
    ));
    (objective_id, task["id"].as_i64().expect("task ID"))
}

fn prepare_and_dispatch_delegation(
    data_dir: &Path,
    project_id: i64,
    task_id: i64,
    run_id: i64,
    label: &str,
    allowed_scope: &[&str],
) -> i64 {
    let prepared = tool_text(&call_tool(
        data_dir,
        "prepare_delegation",
        json!({
            "project_id": project_id,
            "task_id": task_id,
            "run_id": run_id,
            "role": "worker",
            "task_packet": {
                "schema_version": 1,
                "title": format!("{label} worker"),
                "objective": "Complete the bounded worker task and return auditable evidence.",
                "acceptance_criteria": ["Keep the worker result auditable"],
                "constraints": ["Do not change unrelated project files"],
                "allowed_scope": allowed_scope,
                "referenced_memory_ids": [],
                "referenced_artifact_ids": [],
                "requested_report_fields": ["summary", "changed_paths", "commits"]
            },
            "idempotency_key": format!("{label}-delegation")
        }),
    ));
    let delegation_id = prepared["id"].as_i64().expect("delegation ID");
    call_tool(
        data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": delegation_id }),
    );
    delegation_id
}

fn call_tool(data_dir: &Path, name: &str, arguments: Value) -> Value {
    let responses = run_initialized_server(data_dir, &[tool_call(2, name, arguments)]);
    response(&responses, 2).clone()
}

fn call_tool_without_provider_credentials(data_dir: &Path, name: &str, arguments: Value) -> Value {
    let responses = run_initialized_server_without_provider_credentials(
        data_dir,
        &[tool_call(2, name, arguments)],
    );
    response(&responses, 2).clone()
}

fn create_memory(
    data_dir: &Path,
    project_id: i64,
    body: &str,
    kind: &str,
    temperature: &str,
    sources: Vec<Value>,
    freshness_basis: Value,
    importance: f64,
    protected: bool,
    supersedes_id: Option<i64>,
) -> Value {
    let result = call_tool(
        data_dir,
        "create_memory",
        json!({
            "scope_type": "project",
            "scope_id": project_id.to_string(),
            "memory_class": "canonical",
            "temperature": temperature,
            "kind": kind,
            "body": body,
            "sources": sources,
            "artifact_ids": [],
            "valid_from": null,
            "valid_until": null,
            "supersedes_id": supersedes_id,
            "freshness_basis": freshness_basis,
            "importance": importance,
            "confidence": 0.9,
            "active": false,
            "protected": protected
        }),
    );
    tool_text(&result)
}

fn read_memory(data_dir: &Path, memory_id: i64) -> Value {
    let responses = run_initialized_server(
        data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://memories/{memory_id}") }),
        )],
    );
    resource_text(response(&responses, 2))
}

fn database_artifact_unprotect_is_rejected(data_dir: &Path, artifact_id: i64) -> bool {
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open artifact database for trigger assertion");
    database
        .execute(
            "UPDATE evidence_artifacts SET protected = 0 WHERE id = ?1",
            [artifact_id],
        )
        .is_err()
}

fn transfer_memory(
    data_dir: &Path,
    memory_id: i64,
    sender_project_id: i64,
    receiver_scope_id: &str,
    retrieval_method: &str,
) -> Value {
    let result = call_tool(
        data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": memory_id,
            "sender_scope_type": "project",
            "sender_scope_id": sender_project_id.to_string(),
            "receiver_scope_type": "scope",
            "receiver_scope_id": receiver_scope_id,
            "run_id": null,
            "retrieval_method": retrieval_method,
            "token_count": null
        }),
    );
    tool_text(&result)
}

fn current_head(repo: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("read fixture HEAD");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn invalid_repository_is_reported_without_creating_a_project() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cathulu-invalid-repo-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create temp directory");
    let data_dir = root.join("data");
    let not_a_repo = root.join("not-a-repo");
    fs::create_dir_all(&not_a_repo).expect("create non-repository path");
    let failed_registration = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "register_project",
            json!({ "path": not_a_repo }),
        )],
    );
    assert_eq!(
        response(&failed_registration, 2).pointer("/result/isError"),
        Some(&json!(true))
    );
    let state_after_failure = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": "cathulu://projects" }),
        )],
    );
    assert_eq!(
        resource_text(response(&state_after_failure, 2))
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn codex_hook_events_are_sanitized_idempotent_and_recover_after_restart() {
    let root = unique_temp_dir("codex-hooks");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    let project_id = register_repo(&data_dir, &repo);

    let discovery = run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let telemetry_tool = response(&discovery, 2)
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool.get("name") == Some(&json!("record_codex_hook_event")))
        })
        .expect("safe Codex hook ingestion tool is discoverable");
    let properties = telemetry_tool
        .pointer("/inputSchema/properties")
        .and_then(Value::as_object)
        .expect("telemetry tool has a typed input schema");
    for sensitive_field in ["prompt", "tool_input", "tool_response", "transcript_path"] {
        assert!(
            !properties.contains_key(sensitive_field),
            "telemetry input must not accept {sensitive_field}"
        );
    }

    let rejected_sensitive_input = call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "synthetic-session-rejected",
            "turn_id": "synthetic-turn-rejected",
            "cwd": repo.to_string_lossy().into_owned(),
            "prompt": "PROMPT_FIELD_MUST_BE_REJECTED"
        }),
    );
    assert_eq!(
        rejected_sensitive_input.pointer("/result/isError"),
        Some(&json!(true))
    );

    let started = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "synthetic-session-01",
            "turn_id": "synthetic-turn-01",
            "cwd": repo.to_string_lossy().into_owned()
        }),
    ));
    let run_id = started
        .get("runId")
        .and_then(Value::as_i64)
        .expect("hook event maps to an observed run");
    assert_eq!(started.get("recorded"), Some(&json!(true)));

    fs::write(
        repo.join("README.md"),
        "second commit after Cathulu restart\n",
    )
    .expect("update tracked fixture file");
    run_git(&repo, &["add", "README.md"]);
    run_git(&repo, &["commit", "--quiet", "-m", "second"]);
    let expected_head = current_head(&repo);
    let reconciliation = tool_text(&call_tool(
        &data_dir,
        "reconcile_project",
        json!({ "project_id": project_id }),
    ));
    assert_eq!(reconciliation.get("head"), Some(&json!(expected_head)));

    let tool_event = json!({
        "hook_event_name": "PostToolUse",
        "session_id": "synthetic-session-01",
        "turn_id": "synthetic-turn-01",
        "cwd": repo.to_string_lossy().into_owned(),
        "tool_name": "Bash",
        "tool_use_id": "synthetic-tool-01"
    });
    let recorded_tool = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        tool_event.clone(),
    ));
    assert_eq!(recorded_tool.get("runId"), Some(&json!(run_id)));
    assert_eq!(recorded_tool.get("recorded"), Some(&json!(true)));
    let duplicate_tool = tool_text(&call_tool(&data_dir, "record_codex_hook_event", tool_event));
    assert_eq!(duplicate_tool.get("duplicate"), Some(&json!(true)));

    let stopped = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "Stop",
            "session_id": "synthetic-session-01",
            "turn_id": "synthetic-turn-01",
            "cwd": repo.to_string_lossy().into_owned()
        }),
    ));
    assert_eq!(stopped.get("runId"), Some(&json!(run_id)));
    assert_eq!(stopped.get("recorded"), Some(&json!(true)));

    let refreshed_project = run_initialized_server(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://projects/{project_id}") }),
            ),
            request(
                3,
                "resources/read",
                json!({ "uri": format!("cathulu://events/run/{run_id}") }),
            ),
            request(4, "resources/read", json!({ "uri": "cathulu://dashboard" })),
        ],
    );
    assert_eq!(
        resource_text(response(&refreshed_project, 2)).get("head"),
        Some(&json!(expected_head))
    );
    let events = resource_text(response(&refreshed_project, 3));
    let events_json = events.to_string();
    assert_eq!(events.as_array().map(Vec::len), Some(5));
    assert!(!events_json.contains("synthetic-session-01"));
    assert!(!events_json.contains("synthetic-turn-01"));
    assert!(events_json.contains("sessionHash"));
    assert!(events_json.contains("toolCategory"));
    let dashboard = resource_text(response(&refreshed_project, 4));
    assert_eq!(
        dashboard
            .pointer("/recentRuns/0/status")
            .and_then(Value::as_str),
        Some("inconclusive")
    );
    assert_eq!(
        dashboard
            .pointer("/recentRuns/0/summary")
            .and_then(Value::as_str),
        Some("Codex turn ended; Cathulu verification was not recorded.")
    );

    let duplicate_stop = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "Stop",
            "session_id": "synthetic-session-01",
            "turn_id": "synthetic-turn-01",
            "cwd": repo.to_string_lossy().into_owned()
        }),
    ));
    assert_eq!(duplicate_stop.get("duplicate"), Some(&json!(true)));
    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn delegation_worktree_reconciliation_is_scoped_and_verifies_report_claims() {
    let root = unique_temp_dir("delegation-worktree");
    let repo = root.join("repo");
    let worker = root.join("worker");
    let foreign = root.join("foreign");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    make_git_repo(&foreign);
    let project_id = register_repo(&data_dir, &repo);
    let (objective_id, task_id) = create_task_for_project(&data_dir, project_id, "m8 git evidence");
    let run = tool_text(&call_tool(
        &data_dir,
        "start_run",
        json!({ "project_id": project_id, "objective_id": objective_id, "task_id": task_id }),
    ));
    let run_id = run["id"].as_i64().expect("parent run ID");
    let delegation_id = prepare_and_dispatch_delegation(
        &data_dir,
        project_id,
        task_id,
        run_id,
        "m8-git-evidence",
        &["src/**"],
    );
    let base = current_head(&repo);

    let rejected_paths = [
        root.join("missing"),
        foreign.clone(),
        repo.join("nested"),
        repo.clone(),
    ];
    fs::create_dir_all(repo.join("nested")).expect("create non-worktree nested directory");
    for (index, rejected_path) in rejected_paths.iter().enumerate() {
        let result = call_tool(
            &data_dir,
            "reconcile_delegation_worktree",
            json!({
                "delegation_id": delegation_id,
                "worktree_path": rejected_path.to_string_lossy()
            }),
        );
        assert_eq!(
            result.pointer("/result/isError"),
            Some(&json!(true)),
            "rejected worktree path {index} unexpectedly reconciled: {result}"
        );
    }
    let unreconciled = resource_text(&response(
        &run_initialized_server(
            &data_dir,
            &[request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://delegations/{delegation_id}") }),
            )],
        ),
        2,
    ));
    assert_eq!(unreconciled["worktree"]["baseCommit"], json!(base));
    assert_eq!(unreconciled["worktree"]["lastReconciledAt"], Value::Null);

    let add_worktree = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["worktree", "add", "--quiet", "-b", "codex/m8-worker"])
        .arg(&worker)
        .arg(&base)
        .output()
        .expect("launch git to create disposable linked worktree fixture");
    assert!(
        add_worktree.status.success(),
        "cannot create Git worktree fixture: {}",
        String::from_utf8_lossy(&add_worktree.stderr)
    );
    run_git(&worker, &["config", "user.name", "Cathulu Test"]);
    run_git(
        &worker,
        &["config", "user.email", "cathulu-test@example.invalid"],
    );
    fs::create_dir_all(worker.join("src")).expect("create worker source directory");
    fs::write(worker.join("src/worker.rs"), "pub fn worker_change() {}\n")
        .expect("write worker change");
    run_git(&worker, &["add", "src/worker.rs"]);
    run_git(&worker, &["commit", "--quiet", "-m", "worker change"]);
    let worker_head = current_head(&worker);

    let report = call_tool(
        &data_dir,
        "submit_delegation_report",
        json!({
            "delegation_id": delegation_id,
            "idempotency_key": "m8-worker-report",
            "worker_report": {
                "schemaVersion": 1,
                "summary": "Added the requested worker file.",
                "outcome": "completed",
                "changedFiles": ["src/worker.rs"],
                "commits": [worker_head],
                "checkResults": [],
                "blockers": [],
                "artifactIds": [],
                "qualityCheckIds": []
            }
        }),
    );
    tool_text(&report);
    let reconciled = tool_text(&call_tool(
        &data_dir,
        "reconcile_delegation_worktree",
        json!({
            "delegation_id": delegation_id,
            "worktree_path": worker.to_string_lossy()
        }),
    ));
    assert_eq!(reconciled["baseCommit"], json!(base));
    assert_eq!(reconciled["observedHead"], json!(worker_head));
    assert_eq!(reconciled["branch"], json!("codex/m8-worker"));
    assert_eq!(reconciled["isClean"], json!(true));
    assert_eq!(reconciled["changedPaths"], json!(["src/worker.rs"]));
    assert_eq!(
        reconciled.pointer("/reportVerification/status"),
        Some(&json!("verified"))
    );
    assert_eq!(
        reconciled["reportVerification"]["verifiedCommits"],
        json!([worker_head])
    );

    fs::write(
        worker.join("src/pending.tmp"),
        "uncommitted worker evidence\n",
    )
    .expect("write untracked worker evidence");
    let dirty = tool_text(&call_tool(
        &data_dir,
        "reconcile_delegation_worktree",
        json!({
            "delegation_id": delegation_id,
            "worktree_path": worker.to_string_lossy()
        }),
    ));
    assert_eq!(dirty["isClean"], json!(false));
    assert_eq!(
        dirty["changedPaths"],
        json!(["src/pending.tmp", "src/worker.rs"])
    );
    assert_eq!(
        dirty.pointer("/reportVerification/status"),
        Some(&json!("verified"))
    );

    let (mismatch_objective, mismatch_task) =
        create_task_for_project(&data_dir, project_id, "m8 git mismatch");
    let mismatch_run = tool_text(&call_tool(
        &data_dir,
        "start_run",
        json!({
            "project_id": project_id,
            "objective_id": mismatch_objective,
            "task_id": mismatch_task
        }),
    ));
    let mismatch_delegation_id = prepare_and_dispatch_delegation(
        &data_dir,
        project_id,
        mismatch_task,
        mismatch_run["id"].as_i64().expect("mismatch parent run ID"),
        "m8-git-mismatch",
        &["src/**"],
    );
    fs::write(
        foreign.join("foreign-only.txt"),
        "not in the registered repository\n",
    )
    .expect("write foreign-only commit");
    run_git(&foreign, &["add", "foreign-only.txt"]);
    run_git(
        &foreign,
        &["commit", "--quiet", "-m", "foreign-only commit"],
    );
    let foreign_commit = current_head(&foreign);
    tool_text(&call_tool(
        &data_dir,
        "submit_delegation_report",
        json!({
            "delegation_id": mismatch_delegation_id,
            "idempotency_key": "m8-mismatch-worker-report",
            "worker_report": {
                "schemaVersion": 1,
                "summary": "This report intentionally claims unrelated Git evidence.",
                "outcome": "completed",
                "changedFiles": ["src/not-actually-changed.rs"],
                "commits": [foreign_commit],
                "checkResults": [],
                "blockers": [],
                "artifactIds": [],
                "qualityCheckIds": []
            }
        }),
    ));
    let mismatch = tool_text(&call_tool(
        &data_dir,
        "reconcile_delegation_worktree",
        json!({
            "delegation_id": mismatch_delegation_id,
            "worktree_path": worker.to_string_lossy()
        }),
    ));
    assert_eq!(
        mismatch.pointer("/reportVerification/status"),
        Some(&json!("mismatch"))
    );
    assert_eq!(
        mismatch["reportVerification"]["unverifiedCommits"],
        json!([foreign_commit])
    );
    assert_eq!(
        mismatch["reportVerification"]["unverifiedPaths"],
        json!(["src/not-actually-changed.rs"])
    );
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open worktree snapshot count");
    let snapshot_count: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM delegation_worktree_states WHERE delegation_id = ?1",
            rusqlite::params![delegation_id],
            |row| row.get(0),
        )
        .expect("count worktree projections");
    assert_eq!(
        snapshot_count, 1,
        "reconciliation updates one state row per delegation"
    );
    drop(database);

    let reopened = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://delegations/{delegation_id}") }),
        )],
    );
    let delegation = resource_text(response(&reopened, 2));
    assert_eq!(
        delegation.pointer("/worktree/reportVerification/status"),
        Some(&json!("verified"))
    );
    assert_eq!(
        delegation.pointer("/worktree/observedHead"),
        Some(&json!(worker_head))
    );
    assert_eq!(delegation.pointer("/worktree/isClean"), Some(&json!(false)));
    fs::remove_dir_all(root).expect("remove temporary test data and linked worktree fixture");
}

#[test]
fn subagent_hook_telemetry_requires_explicit_run_scoped_binding_and_stays_private() {
    let root = unique_temp_dir("subagent-hook-telemetry");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    let project_id = register_repo(&data_dir, &repo);
    let (objective_id, task_id) =
        create_task_for_project(&data_dir, project_id, "m8 worker telemetry");

    let parent_hook = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "m8-private-parent-session",
            "turn_id": "m8-private-parent-turn",
            "cwd": repo.to_string_lossy()
        }),
    ));
    let run_id = parent_hook["runId"]
        .as_i64()
        .expect("observed parent run ID");
    let database_path = data_dir.join("cathulu.sqlite3");
    let database =
        rusqlite::Connection::open(&database_path).expect("open test state to attach task");
    database
        .execute(
            "UPDATE runs SET objective_id = ?1, task_id = ?2 WHERE id = ?3",
            rusqlite::params![objective_id, task_id, run_id],
        )
        .expect("attach synthetic observed run to its task for delegation setup");
    drop(database);

    let delegation_id = prepare_and_dispatch_delegation(
        &data_dir,
        project_id,
        task_id,
        run_id,
        "m8-worker-telemetry",
        &["src/**"],
    );
    let schema = run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let hook_tool = response(&schema, 2)
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools
                .iter()
                .find(|tool| tool["name"] == "record_codex_hook_event")
        })
        .expect("Codex hook tool is discoverable");
    let hook_properties = hook_tool
        .pointer("/inputSchema/properties")
        .and_then(Value::as_object)
        .expect("Codex hook input has typed properties");
    for unsupported in [
        "reasoning_effort",
        "transcript_path",
        "last_assistant_message",
        "prompt",
    ] {
        assert!(!hook_properties.contains_key(unsupported));
    }

    let unknown_parent = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": "unmapped-session",
            "turn_id": "unmapped-turn",
            "agent_id": "unmapped-agent",
            "agent_type": "worker",
            "model": "gpt-5-codex",
            "cwd": repo.to_string_lossy()
        }),
    ));
    assert_eq!(unknown_parent["reason"], json!("unknown_parent_turn"));
    assert_eq!(unknown_parent["runId"], Value::Null);

    let start = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": "m8-private-parent-session",
            "turn_id": "m8-private-parent-turn",
            "agent_id": "m8-private-agent-id",
            "agent_type": "worker",
            "model": "gpt-5-codex"
        }),
    ));
    let hook_event_id = start["eventId"].as_i64().expect("SubagentStart event ID");
    assert_eq!(start["runId"], json!(run_id));
    assert_eq!(start["recorded"], json!(true));
    let duplicate = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "SubagentStart",
            "session_id": "m8-private-parent-session",
            "turn_id": "m8-private-parent-turn",
            "agent_id": "m8-private-agent-id",
            "agent_type": "worker",
            "model": "gpt-5-codex"
        }),
    ));
    assert_eq!(duplicate["duplicate"], json!(true));
    assert_eq!(duplicate["eventId"], json!(hook_event_id));

    let binding = tool_text(&call_tool(
        &data_dir,
        "associate_delegation_worker",
        json!({
            "delegation_id": delegation_id,
            "hook_event_id": hook_event_id,
            "idempotency_key": "m8-worker-binding"
        }),
    ));
    assert_eq!(binding["effectiveModel"], json!("gpt-5-codex"));
    assert_eq!(binding["effectiveEffort"], Value::Null);
    assert_eq!(binding["events"].as_array().map(Vec::len), Some(1));
    let duplicate_binding = tool_text(&call_tool(
        &data_dir,
        "associate_delegation_worker",
        json!({
            "delegation_id": delegation_id,
            "hook_event_id": hook_event_id,
            "idempotency_key": "m8-worker-binding"
        }),
    ));
    assert_eq!(duplicate_binding["hookEventId"], json!(hook_event_id));

    let stop = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "SubagentStop",
            "session_id": "m8-private-parent-session",
            "turn_id": "m8-private-parent-turn",
            "agent_id": "m8-private-agent-id",
            "agent_type": "worker",
            "model": "gpt-5-codex"
        }),
    ));
    assert_eq!(stop["recorded"], json!(true));

    let reopened = run_initialized_server(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://delegations/{delegation_id}") }),
            ),
            request(
                3,
                "resources/read",
                json!({ "uri": format!("cathulu://events/run/{run_id}") }),
            ),
        ],
    );
    let delegation = resource_text(response(&reopened, 2));
    assert_eq!(
        delegation.pointer("/workerTelemetry/effectiveModel"),
        Some(&json!("gpt-5-codex"))
    );
    assert_eq!(
        delegation.pointer("/workerTelemetry/effectiveEffort"),
        Some(&Value::Null)
    );
    assert_eq!(
        delegation.pointer("/workerTelemetry/events/1/eventKind"),
        Some(&json!("stopped"))
    );
    let history = resource_text(response(&reopened, 3)).to_string();
    for private_value in [
        "m8-private-parent-session",
        "m8-private-parent-turn",
        "m8-private-agent-id",
    ] {
        assert!(
            !history.contains(private_value),
            "raw hook identifier persisted: {private_value}"
        );
    }
    assert!(history.contains("agentIdHash"));

    let database = rusqlite::Connection::open(&database_path).expect("open telemetry persistence");
    let metadata: String = database
        .query_row(
            "SELECT session_hash || turn_hash || agent_id_hash || agent_type || COALESCE(model_slug, '')
             FROM codex_subagent_hook_events ORDER BY created_at LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("read persisted coarse metadata");
    assert!(!metadata.contains("m8-private-parent-session"));
    assert!(!metadata.contains("m8-private-parent-turn"));
    assert!(!metadata.contains("m8-private-agent-id"));
    drop(database);
    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn unsupported_or_unregistered_hook_events_are_ignored_without_partial_runs() {
    let root = unique_temp_dir("ignored-hooks");
    let repo = root.join("repo");
    let unregistered_repo = root.join("unregistered");
    let removed_repo = root.join("removed");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    make_git_repo(&unregistered_repo);
    make_git_repo(&removed_repo);
    let project_id = register_repo(&data_dir, &repo);
    register_repo(&data_dir, &removed_repo);

    let unsupported = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "SessionEnd",
            "session_id": "synthetic-session-02",
            "turn_id": "synthetic-turn-02",
            "cwd": repo.to_string_lossy().into_owned()
        }),
    ));
    assert_eq!(unsupported.get("recorded"), Some(&json!(false)));
    assert_eq!(unsupported.get("reason"), Some(&json!("unsupported_event")));

    let malformed = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "PostToolUse",
            "session_id": "synthetic-session-malformed",
            "turn_id": "synthetic-turn-malformed",
            "cwd": repo.to_string_lossy().into_owned()
        }),
    ));
    assert_eq!(malformed.get("recorded"), Some(&json!(false)));
    assert_eq!(
        malformed.get("reason"),
        Some(&json!("missing_tool_metadata"))
    );

    let unregistered = tool_text(&call_tool(
        &data_dir,
        "record_codex_hook_event",
        json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "synthetic-session-03",
            "turn_id": "synthetic-turn-03",
            "cwd": unregistered_repo.to_string_lossy().into_owned()
        }),
    ));
    assert_eq!(unregistered.get("recorded"), Some(&json!(false)));
    assert_eq!(
        unregistered.get("reason"),
        Some(&json!("unregistered_project"))
    );

    fs::write(repo.join("README.md"), "updated after restart\n")
        .expect("update tracked fixture file");
    run_git(&repo, &["add", "README.md"]);
    run_git(
        &repo,
        &["commit", "--quiet", "-m", "restart reconciliation"],
    );
    let expected_head = current_head(&repo);
    let reconciliation = tool_text(&call_tool(
        &data_dir,
        "reconcile_project",
        json!({ "project_id": project_id }),
    ));
    assert_eq!(reconciliation.get("head"), Some(&json!(expected_head)));
    fs::remove_dir_all(&removed_repo).expect("remove one registered repository");

    let dashboard = run_initialized_server(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://projects/{project_id}") }),
            ),
            request(3, "resources/read", json!({ "uri": "cathulu://dashboard" })),
        ],
    );
    assert_eq!(
        resource_text(response(&dashboard, 2)).get("head"),
        Some(&json!(expected_head))
    );
    assert_eq!(
        resource_text(response(&dashboard, 3))
            .get("recentRuns")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn synthetic_hook_payload_discards_prompt_and_tool_bodies_before_persistence() {
    let root = unique_temp_dir("hook-privacy");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    let store = Store::open(&data_dir).expect("open isolated SQLite store");
    let project = store
        .register_project(&repo)
        .expect("register synthetic Git repository");
    let input: CodexHookEvent = serde_json::from_value(json!({
        "hook_event_name": "PostToolUse",
        "session_id": "private-session-id",
        "turn_id": "private-turn-id",
        "cwd": repo.to_string_lossy(),
        "tool_name": "Bash",
        "tool_use_id": "private-tool-use-id",
        "prompt": "PROMPT_BODY_MUST_NOT_PERSIST",
        "transcript_path": "TRANSCRIPT_PATH_MUST_NOT_PERSIST",
        "tool_input": { "command": "TOOL_INPUT_MUST_NOT_PERSIST" },
        "tool_response": "TOOL_OUTPUT_MUST_NOT_PERSIST"
    }))
    .expect("deserialize only the allowlisted hook fields");

    let result = store
        .record_codex_hook_event(input)
        .expect("record sanitized hook metadata");
    let run_id = result.run_id.expect("observed run ID");
    assert_eq!(result.recorded, true);
    let events = store
        .list_events("run", &run_id.to_string())
        .expect("read sanitized run events");
    let serialized = serde_json::to_string(&events).expect("serialize event history");
    for secret in [
        "PROMPT_BODY_MUST_NOT_PERSIST",
        "TRANSCRIPT_PATH_MUST_NOT_PERSIST",
        "TOOL_INPUT_MUST_NOT_PERSIST",
        "TOOL_OUTPUT_MUST_NOT_PERSIST",
        "private-session-id",
        "private-turn-id",
        "private-tool-use-id",
    ] {
        assert!(
            !serialized.contains(secret),
            "event history contains {secret}"
        );
    }
    assert!(!serialized.contains(&repo.to_string_lossy().to_string()));
    assert_eq!(store.get_project(project.id).unwrap().unwrap(), project);
    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn codex_event_ingestion_rolls_back_run_mapping_when_event_append_fails() {
    let root = unique_temp_dir("hook-atomicity");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    make_git_repo(&repo);
    register_repo(&data_dir, &repo);

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open database for synthetic partial failure");
    database
        .execute_batch(
            "CREATE TRIGGER reject_codex_event BEFORE INSERT ON events
             WHEN NEW.event_type LIKE 'codex_%'
             BEGIN SELECT RAISE(ABORT, 'forced hook event failure'); END;",
        )
        .expect("install event append failure trigger");
    drop(database);

    let arguments = json!({
        "hook_event_name": "UserPromptSubmit",
        "session_id": "atomic-session",
        "turn_id": "atomic-turn",
        "cwd": repo.to_string_lossy().into_owned()
    });
    let failed = call_tool(&data_dir, "record_codex_hook_event", arguments.clone());
    assert_eq!(failed.pointer("/result/isError"), Some(&json!(true)));

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("reopen database after failed event append");
    let run_count: i64 = database
        .query_row("SELECT COUNT(*) FROM runs", [], |row| row.get(0))
        .expect("query rolled-back run state");
    let mapping_count: i64 = database
        .query_row("SELECT COUNT(*) FROM codex_turn_runs", [], |row| row.get(0))
        .expect("query rolled-back turn mapping");
    assert_eq!(run_count, 0);
    assert_eq!(mapping_count, 0);
    database
        .execute_batch("DROP TRIGGER reject_codex_event")
        .expect("remove synthetic failure trigger");
    drop(database);

    let recovered = tool_text(&call_tool(&data_dir, "record_codex_hook_event", arguments));
    assert_eq!(recovered.get("recorded"), Some(&json!(true)));
    fs::remove_dir_all(root).expect("remove temporary test data");
}
