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

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn tool_call(id: u64, name: &str, arguments: Value) -> Value {
    request(
        id,
        "tools/call",
        json!({ "name": name, "arguments": arguments }),
    )
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_cathulu-mcp"))
        .env("CATHULU_DATA_DIR", data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch Cathulu MCP server");
    {
        let stdin = child.stdin.as_mut().expect("child stdin");
        for message in messages {
            serde_json::to_writer(&mut *stdin, &message).expect("serialize MCP request");
            stdin.write_all(b"\n").expect("terminate request line");
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

fn response(responses: &[Value], id: u64) -> &Value {
    responses
        .iter()
        .find(|message| message.get("id") == Some(&json!(id)))
        .unwrap_or_else(|| panic!("missing response id {id}: {responses:?}"))
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

fn preview_evidence(data_dir: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_cathulu-maintain"))
        .env("CATHULU_DATA_DIR", data_dir)
        .args(["evidence-gc", "preview"])
        .output()
        .expect("launch local evidence preview");
    assert!(
        output.status.success(),
        "evidence preview failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("evidence preview is JSON")
}

fn call_tool(data_dir: &Path, name: &str, arguments: Value) -> Value {
    let responses = run_initialized_server(data_dir, &[tool_call(2, name, arguments)]);
    tool_text(response(&responses, 2))
}

fn tool_error(data_dir: &Path, name: &str, arguments: Value) -> Value {
    let responses = run_initialized_server(data_dir, &[tool_call(2, name, arguments)]);
    let result = response(&responses, 2).clone();
    assert!(
        result.pointer("/result/isError") == Some(&json!(true)) || result.get("error").is_some(),
        "MCP tool call unexpectedly succeeded: {result}"
    );
    if result.get("error").is_some() {
        json!({ "result": { "isError": true }, "rpcError": result["error"] })
    } else {
        result
    }
}

fn create_task_and_run(data_dir: &Path, project_id: i64, label: &str) -> (i64, i64, i64) {
    let objective = call_tool(
        data_dir,
        "create_objective",
        json!({ "project_id": project_id, "title": format!("{label} objective") }),
    );
    let objective_id = objective["id"].as_i64().expect("objective ID");
    let task = call_tool(
        data_dir,
        "create_task",
        json!({ "objective_id": objective_id, "title": format!("{label} task") }),
    );
    let task_id = task["id"].as_i64().expect("task ID");
    let run = call_tool(
        data_dir,
        "start_run",
        json!({ "project_id": project_id, "objective_id": objective_id, "task_id": task_id }),
    );
    let run_id = run["id"].as_i64().expect("run ID");
    (objective_id, task_id, run_id)
}

#[test]
fn worker_reports_are_bounded_idempotent_reviewable_and_do_not_bypass_parent_quality() {
    let root = unique_temp_dir("worker-report-review");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    let mut project_ids = Vec::new();
    for (name, path) in [
        ("m7-main-fixture", "C:/fixture/m7-main"),
        ("m7-other-fixture", "C:/fixture/m7-other"),
    ] {
        database
            .execute(
                "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
                 VALUES (?1, ?2, 'main', NULL, 0, '[]', 1, 1)",
                rusqlite::params![name, path],
            )
            .expect("seed registered project");
        project_ids.push(database.last_insert_rowid());
    }
    drop(database);

    let (_objective_id, task_id, parent_run_id) =
        create_task_and_run(&data_dir, project_ids[0], "worker report");
    let (other_objective_id, other_task_id, other_run_id) =
        create_task_and_run(&data_dir, project_ids[1], "foreign evidence");
    let parent_artifact = call_tool(
        &data_dir,
        "record_evidence_artifact",
        json!({
            "run_id": parent_run_id,
            "artifact_kind": "test-log",
            "uri": "artifact://local/m7-parent-test-log",
            "content_hash": "sha256:m7-parent-test-log"
        }),
    )["id"]
        .as_i64()
        .expect("parent evidence artifact ID");
    let foreign_artifact = call_tool(
        &data_dir,
        "record_evidence_artifact",
        json!({
            "run_id": other_run_id,
            "artifact_kind": "test-log",
            "uri": "artifact://local/m7-foreign-test-log",
            "content_hash": "sha256:m7-foreign-test-log"
        }),
    )["id"]
        .as_i64()
        .expect("foreign evidence artifact ID");
    let parent_check = call_tool(
        &data_dir,
        "record_quality_check",
        json!({
            "run_id": parent_run_id,
            "check_key": "worker-unit-tests",
            "mandatory": false,
            "status": "passed",
            "details": { "source": "parent-recorded" }
        }),
    )["id"]
        .as_i64()
        .expect("parent quality-check ID");
    let foreign_check = call_tool(
        &data_dir,
        "record_quality_check",
        json!({
            "run_id": other_run_id,
            "check_key": "foreign-check",
            "mandatory": false,
            "status": "passed",
            "details": {}
        }),
    )["id"]
        .as_i64()
        .expect("foreign quality-check ID");
    assert!(other_objective_id > 0 && other_task_id > 0);

    let packet = json!({
        "schema_version": 1,
        "title": "Implement worker report evidence",
        "objective": "Submit an evidence-backed report through the local MCP boundary.",
        "acceptance_criteria": ["Keep worker claims distinct from parent QA"],
        "constraints": ["Do not spawn or control a worker"],
        "allowed_scope": ["src/**"],
        "referenced_memory_ids": [],
        "referenced_artifact_ids": [],
        "requested_report_fields": ["summary", "changed_paths", "commits", "checks", "blockers", "artifact_refs"]
    });
    let prepared = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": project_ids[0],
            "task_id": task_id,
            "run_id": parent_run_id,
            "role": "worker",
            "task_packet": packet,
            "idempotency_key": "m7-delegation-attempt"
        }),
    );
    let delegation_id = prepared["id"].as_i64().expect("delegation ID");
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": delegation_id }),
    );

    let worker_report = json!({
        "schemaVersion": 1,
        "summary": "The scoped change is complete and its worker checks passed.",
        "outcome": "completed",
        "changedFiles": ["src/delegation.rs"],
        "commits": ["0123456789abcdef0123456789abcdef01234567"],
        "checkResults": [{
            "name": "workspace tests",
            "status": "passed",
            "summary": "The worker reports all workspace tests passed."
        }],
        "blockers": [],
        "artifactIds": [parent_artifact],
        "qualityCheckIds": [parent_check]
    });

    let mut out_of_scope = worker_report.clone();
    out_of_scope["changedFiles"][0] = json!("tests/other.rs");
    assert_eq!(
        tool_error(
            &data_dir,
            "submit_delegation_report",
            json!({ "delegation_id": delegation_id, "worker_report": out_of_scope, "idempotency_key": "m7-out-of-scope" })
        )
        .pointer("/result/isError"),
        Some(&json!(true))
    );

    let mut traversal = worker_report.clone();
    traversal["changedFiles"][0] = json!("src/../../secrets.txt");
    assert_eq!(
        tool_error(
            &data_dir,
            "submit_delegation_report",
            json!({ "delegation_id": delegation_id, "worker_report": traversal, "idempotency_key": "m7-path-traversal" })
        )
        .pointer("/result/isError"),
        Some(&json!(true))
    );

    let mut duplicate_files = worker_report.clone();
    duplicate_files["changedFiles"] = json!(["src/delegation.rs", "src/delegation.rs"]);
    tool_error(
        &data_dir,
        "submit_delegation_report",
        json!({ "delegation_id": delegation_id, "worker_report": duplicate_files, "idempotency_key": "m7-duplicate-file" }),
    );

    let mut invalid_outcome = worker_report.clone();
    invalid_outcome["outcome"] = json!("verified_success");
    tool_error(
        &data_dir,
        "submit_delegation_report",
        json!({ "delegation_id": delegation_id, "worker_report": invalid_outcome, "idempotency_key": "m7-invalid-outcome" }),
    );

    let mut foreign_evidence = worker_report.clone();
    foreign_evidence["artifactIds"] = json!([foreign_artifact]);
    assert_eq!(
        tool_error(
            &data_dir,
            "submit_delegation_report",
            json!({ "delegation_id": delegation_id, "worker_report": foreign_evidence, "idempotency_key": "m7-foreign-artifact" })
        )
        .pointer("/result/isError"),
        Some(&json!(true))
    );

    let mut foreign_check_reference = worker_report.clone();
    foreign_check_reference["qualityCheckIds"] = json!([foreign_check]);
    assert_eq!(
        tool_error(
            &data_dir,
            "submit_delegation_report",
            json!({ "delegation_id": delegation_id, "worker_report": foreign_check_reference, "idempotency_key": "m7-foreign-check" })
        )
        .pointer("/result/isError"),
        Some(&json!(true))
    );

    let mut oversized = worker_report.clone();
    oversized["summary"] = json!("s".repeat(4_000));
    oversized["blockers"] = json!(vec!["b".repeat(1_000); 16]);
    assert_eq!(
        tool_error(
            &data_dir,
            "submit_delegation_report",
            json!({ "delegation_id": delegation_id, "worker_report": oversized, "idempotency_key": "m7-oversized-report" })
        )
        .pointer("/result/isError"),
        Some(&json!(true))
    );

    let submitted = run_initialized_server(
        &data_dir,
        &[
            request(2, "tools/list", json!({})),
            tool_call(
                3,
                "submit_delegation_report",
                json!({
                    "delegation_id": delegation_id,
                    "worker_report": worker_report,
                    "idempotency_key": "m7-report-attempt"
                }),
            ),
            tool_call(
                4,
                "submit_delegation_report",
                json!({
                    "delegation_id": delegation_id,
                    "worker_report": worker_report,
                    "idempotency_key": "m7-report-attempt"
                }),
            ),
        ],
    );
    let tool_names = response(&submitted, 2)
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("MCP tools are listed")
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"submit_delegation_report"));
    assert!(tool_names.contains(&"review_delegation_report"));
    assert!(!tool_names.iter().any(|name| {
        name.contains("spawn") || name.contains("execute") || name.contains("accept_changes")
    }));
    let first_report = tool_text(response(&submitted, 3));
    let duplicate_report = tool_text(response(&submitted, 4));
    let report_id = first_report["reportId"].as_i64().expect("report ID");
    assert_eq!(duplicate_report["reportId"], json!(report_id));
    assert_eq!(first_report["reviewStatus"], json!("reported"));
    assert_eq!(first_report["report"]["outcome"], json!("completed"));
    assert_eq!(first_report["reportHash"].as_str().map(str::len), Some(64));
    assert!(!serde_json::to_string(&first_report)
        .expect("serialize report")
        .contains("m7-report-attempt"));

    let mut conflicting_report = worker_report.clone();
    conflicting_report["summary"] = json!("Different worker report");
    assert_eq!(
        tool_error(
            &data_dir,
            "submit_delegation_report",
            json!({ "delegation_id": delegation_id, "worker_report": conflicting_report, "idempotency_key": "m7-report-attempt" })
        )
        .pointer("/result/isError"),
        Some(&json!(true))
    );

    let review_args = json!({
        "delegation_id": delegation_id,
        "decision": "needs_follow_up",
        "note": "Attach the parent verification result before accepting.",
        "idempotency_key": "m7-review-attempt-1"
    });
    let repeated_review = run_initialized_server(
        &data_dir,
        &[
            tool_call(2, "review_delegation_report", review_args.clone()),
            tool_call(3, "review_delegation_report", review_args),
        ],
    );
    let first_review = tool_text(response(&repeated_review, 2));
    let duplicate_review = tool_text(response(&repeated_review, 3));
    assert_eq!(first_review["reviewStatus"], json!("needs_follow_up"));
    assert_eq!(first_review["reviewId"], duplicate_review["reviewId"]);

    let accepted = call_tool(
        &data_dir,
        "review_delegation_report",
        json!({
            "delegation_id": delegation_id,
            "decision": "accepted",
            "note": "Worker evidence reviewed; parent QA remains authoritative.",
            "idempotency_key": "m7-review-attempt-2"
        }),
    );
    assert_eq!(accepted["reviewStatus"], json!("accepted"));

    let before_finish_responses = run_initialized_server(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://runs/{parent_run_id}") }),
            ),
            request(3, "resources/read", json!({ "uri": "cathulu://dashboard" })),
        ],
    );
    let before_finish = resource_text(response(&before_finish_responses, 2));
    assert_eq!(before_finish["status"], json!("running"));
    let before_finish_dashboard = resource_text(response(&before_finish_responses, 3));
    assert_eq!(
        before_finish_dashboard["delegations"][0]["report"]["reviewStatus"],
        json!("accepted")
    );
    let finished = call_tool(
        &data_dir,
        "finish_run",
        json!({ "run_id": parent_run_id, "summary": "Worker reported completion." }),
    );
    assert_eq!(finished["status"], json!("blocked"));

    let final_state = run_initialized_server(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://delegations/{delegation_id}") }),
            ),
            request(3, "resources/read", json!({ "uri": "cathulu://dashboard" })),
        ],
    );
    let persisted = resource_text(response(&final_state, 2));
    assert_eq!(persisted["report"]["reportId"], json!(report_id));
    assert_eq!(persisted["report"]["reviewStatus"], json!("accepted"));
    assert_eq!(
        persisted["report"]["qualityCheckRefs"][0]["qualityCheckId"],
        json!(parent_check)
    );
    assert_eq!(
        persisted["report"]["qualityCheckRefs"][0]["status"],
        json!("passed")
    );
    assert_eq!(
        persisted["report"]["reviewHistory"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        persisted["lifecycleEvents"].as_array().map(Vec::len),
        Some(5)
    );
    let dashboard = resource_text(response(&final_state, 3));
    let parent_run = dashboard["recentRuns"]
        .as_array()
        .and_then(|runs| runs.iter().find(|run| run["id"] == json!(parent_run_id)))
        .expect("dashboard includes the parent run");
    assert_eq!(parent_run["status"], json!("blocked"));
    assert_eq!(
        dashboard["delegations"][0]["report"]["reviewStatus"],
        json!("accepted")
    );

    let evidence_preview = preview_evidence(&data_dir);
    let protected_report_evidence = evidence_preview
        .as_array()
        .and_then(|candidates| {
            candidates
                .iter()
                .find(|candidate| candidate.get("artifactId") == Some(&json!(parent_artifact)))
        })
        .expect("worker report evidence appears in the evidence preview");
    assert!(protected_report_evidence["reasons"]
        .as_array()
        .expect("evidence preview reasons")
        .contains(&json!("worker_report_reference")));

    let reopened = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://delegations/{delegation_id}") }),
        )],
    );
    let after_restart = resource_text(response(&reopened, 2));
    assert_eq!(after_restart["report"]["reviewStatus"], json!("accepted"));
    assert_eq!(
        after_restart["report"]["reviewHistory"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn delegation_packet_has_idempotent_auditable_lifecycle_and_survives_restart() {
    let root = unique_temp_dir("delegation-lifecycle");
    let data_dir = root.join("data");

    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('m6-fixture', 'C:/fixture/m6', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed a local project without requiring a Git subprocess");
    drop(database);

    let objective = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "create_objective",
            json!({ "project_id": 1, "title": "M6 objective" }),
        )],
    );
    assert_eq!(
        tool_text(response(&objective, 2)).get("id"),
        Some(&json!(1))
    );
    let task = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "create_task",
            json!({ "objective_id": 1, "title": "M6 task" }),
        )],
    );
    assert_eq!(tool_text(response(&task, 2)).get("id"), Some(&json!(1)));
    let run = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "start_run",
            json!({ "project_id": 1, "objective_id": 1, "task_id": 1 }),
        )],
    );
    assert_eq!(tool_text(response(&run, 2)).get("id"), Some(&json!(1)));

    let artifact_response = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "record_evidence_artifact",
            json!({
                "run_id": 1,
                "artifact_kind": "m6_fixture",
                "uri": "artifact://local/m6-evidence",
                "content_hash": "sha256:m6-evidence"
            }),
        )],
    );
    let artifact_id = tool_text(response(&artifact_response, 2))["id"]
        .as_i64()
        .expect("registered evidence artifact ID");
    let memory_response = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "create_memory",
            json!({
                "scope_type": "project",
                "scope_id": "1",
                "memory_class": "canonical",
                "temperature": "warm",
                "kind": "lesson",
                "body": "The M6 packet contract is stored as structured data.",
                "sources": [{ "source_type": "run", "source_id": "1" }],
                "importance": 0.8,
                "confidence": 0.9
            }),
        )],
    );
    let memory_id = tool_text(response(&memory_response, 2))["id"]
        .as_i64()
        .expect("provenance-backed memory ID");
    let accepted_memory = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "review_memory_candidate",
            json!({ "memory_id": memory_id, "decision": "accepted" }),
        )],
    );
    assert_eq!(
        tool_text(response(&accepted_memory, 2)).get("reviewState"),
        Some(&json!("accepted"))
    );

    let task_packet = json!({
        "schema_version": 1,
        "title": "Implement typed delegation contract",
        "objective": "Add an auditable delegation record through the public MCP boundary.",
        "acceptance_criteria": ["Persist a stable packet hash", "Keep lifecycle history append-only"],
        "constraints": ["Do not spawn agents or edit Codex configuration"],
        "allowed_scope": ["src/**", "tests/**"],
        "referenced_memory_ids": [memory_id],
        "referenced_artifact_ids": [artifact_id],
        "requested_report_fields": ["summary", "changed_paths", "checks", "blockers"]
    });
    let prepared_args = json!({
        "project_id": 1,
        "task_id": 1,
        "run_id": 1,
        "role": "worker",
        "task_packet": task_packet,
        "idempotency_key": "m6-task-attempt-1",
        "requested_model": "example-model",
        "requested_effort": "high",
        "external_correlation": "native-codex-worker-handle"
    });
    let prepared = run_initialized_server(
        &data_dir,
        &[
            request(2, "tools/list", json!({})),
            tool_call(3, "prepare_delegation", prepared_args.clone()),
            tool_call(4, "prepare_delegation", prepared_args.clone()),
        ],
    );
    let tools = response(&prepared, 2)
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("MCP tools are listed");
    let tool_names = tools
        .iter()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"prepare_delegation"));
    assert!(tool_names.contains(&"mark_delegation_dispatched"));
    assert!(tool_names.contains(&"cancel_delegation"));
    assert!(tool_names.contains(&"reconcile_delegation_worktree"));
    assert!(tool_names.contains(&"associate_delegation_worker"));
    assert!(!tool_names.iter().any(|name| name.contains("spawn")));
    assert!(!tool_names.iter().any(|name| {
        name.contains("execute")
            || name.contains("config")
            || matches!(
                *name,
                "create_worktree" | "move_worktree" | "remove_worktree" | "merge_worktree"
            )
    }));
    let first = tool_text(response(&prepared, 3));
    let repeated = tool_text(response(&prepared, 4));
    let delegation_id = first
        .get("id")
        .and_then(Value::as_i64)
        .expect("prepared delegation has a stable numeric ID");
    assert_eq!(repeated.get("id"), Some(&json!(delegation_id)));
    assert_eq!(first.get("status"), Some(&json!("prepared")));
    assert_eq!(first.get("requestedModel"), Some(&json!("example-model")));
    assert_eq!(first.get("requestedEffort"), Some(&json!("high")));
    assert_eq!(
        first["taskPacket"]["referencedMemoryIds"],
        json!([memory_id])
    );
    assert_eq!(
        first["taskPacket"]["referencedArtifactIds"],
        json!([artifact_id])
    );
    assert!(first.get("effectiveModel").is_none());
    assert!(first.get("effectiveEffort").is_none());
    let serialized_first = serde_json::to_string(&first).expect("serialize delegation record");
    assert!(!serialized_first.contains("m6-task-attempt-1"));
    assert!(!serialized_first.contains("native-codex-worker-handle"));
    assert_eq!(
        first
            .get("externalCorrelationHash")
            .and_then(Value::as_str)
            .map(str::len),
        Some(64)
    );
    assert_eq!(
        first
            .get("packetHash")
            .and_then(Value::as_str)
            .map(str::len),
        Some(64)
    );

    let mut wrong_project = prepared_args.clone();
    wrong_project["project_id"] = json!(999);
    wrong_project["idempotency_key"] = json!("invalid-project-link");
    let invalid_parent = run_initialized_server(
        &data_dir,
        &[tool_call(2, "prepare_delegation", wrong_project)],
    );
    assert_eq!(
        response(&invalid_parent, 2).pointer("/result/isError"),
        Some(&json!(true))
    );

    let mut wrong_run = prepared_args.clone();
    wrong_run["run_id"] = json!(999);
    wrong_run["idempotency_key"] = json!("invalid-run-link");
    let invalid_run =
        run_initialized_server(&data_dir, &[tool_call(2, "prepare_delegation", wrong_run)]);
    assert_eq!(
        response(&invalid_run, 2).pointer("/result/isError"),
        Some(&json!(true))
    );

    let mut invalid_scope = prepared_args.clone();
    invalid_scope["task_packet"]["allowed_scope"][0] = json!("../outside-repository");
    invalid_scope["idempotency_key"] = json!("invalid-scope-path");
    let invalid_packet = run_initialized_server(
        &data_dir,
        &[tool_call(2, "prepare_delegation", invalid_scope)],
    );
    assert_eq!(
        response(&invalid_packet, 2).pointer("/result/isError"),
        Some(&json!(true))
    );

    let mut oversized_packet = prepared_args.clone();
    oversized_packet["task_packet"]["objective"] = json!("x".repeat(4_001));
    oversized_packet["idempotency_key"] = json!("oversized-objective");
    let oversized = run_initialized_server(
        &data_dir,
        &[tool_call(2, "prepare_delegation", oversized_packet)],
    );
    assert_eq!(
        response(&oversized, 2).pointer("/result/isError"),
        Some(&json!(true))
    );
    let evidence_preview = preview_evidence(&data_dir);
    let artifact_candidate = evidence_preview
        .as_array()
        .and_then(|candidates| {
            candidates
                .iter()
                .find(|candidate| candidate.get("artifactId") == Some(&json!(artifact_id)))
        })
        .expect("delegation artifact appears in the evidence preview");
    assert!(artifact_candidate["reasons"]
        .as_array()
        .expect("evidence preview reasons")
        .contains(&json!("delegation_reference")));

    let mut conflicting_args = prepared_args;
    conflicting_args["task_packet"]["title"] = json!("Different request under the same key");
    let conflict = run_initialized_server(
        &data_dir,
        &[tool_call(2, "prepare_delegation", conflicting_args)],
    );
    assert_eq!(
        response(&conflict, 2).pointer("/result/isError"),
        Some(&json!(true))
    );

    let dispatched = run_initialized_server(
        &data_dir,
        &[
            tool_call(
                2,
                "mark_delegation_dispatched",
                json!({ "delegation_id": delegation_id }),
            ),
            tool_call(
                3,
                "mark_delegation_dispatched",
                json!({ "delegation_id": delegation_id }),
            ),
        ],
    );
    assert_eq!(
        tool_text(response(&dispatched, 2)).get("status"),
        Some(&json!("dispatched"))
    );
    assert_eq!(
        tool_text(response(&dispatched, 3)).get("status"),
        Some(&json!("dispatched"))
    );
    let cancelled = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "cancel_delegation",
            json!({ "delegation_id": delegation_id }),
        )],
    );
    assert_eq!(
        tool_text(response(&cancelled, 2)).get("status"),
        Some(&json!("cancelled"))
    );
    let illegal_transition = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "mark_delegation_dispatched",
            json!({ "delegation_id": delegation_id }),
        )],
    );
    assert_eq!(
        response(&illegal_transition, 2).pointer("/result/isError"),
        Some(&json!(true))
    );

    let transition = run_initialized_server(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://delegations/{delegation_id}") }),
            ),
            request(3, "resources/read", json!({ "uri": "cathulu://runs/1" })),
            request(
                4,
                "resources/read",
                json!({ "uri": "cathulu://delegations/2" }),
            ),
        ],
    );
    let delegation = resource_text(response(&transition, 2));
    assert_eq!(delegation.get("status"), Some(&json!("cancelled")));
    let events = delegation
        .get("lifecycleEvents")
        .and_then(Value::as_array)
        .expect("delegation includes its append-only event history");
    assert_eq!(
        events.len(),
        3,
        "repeated dispatch does not append another event"
    );
    assert_eq!(
        events[0].get("eventType"),
        Some(&json!("delegation_prepared"))
    );
    assert_eq!(
        events[1].get("eventType"),
        Some(&json!("delegation_dispatched"))
    );
    assert_eq!(
        events[2].get("eventType"),
        Some(&json!("delegation_cancelled"))
    );
    assert_eq!(
        resource_text(response(&transition, 3)).get("status"),
        Some(&json!("running")),
        "delegation lifecycle does not change the parent run quality state"
    );
    assert!(
        response(&transition, 4).get("error").is_some(),
        "rejected requests do not leave extra delegation records"
    );

    let reopened = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://delegations/{delegation_id}") }),
        )],
    );
    let persisted = resource_text(response(&reopened, 2));
    assert_eq!(persisted.get("packetHash"), first.get("packetHash"));
    assert_eq!(persisted.get("status"), Some(&json!("cancelled")));
    assert_eq!(
        persisted["lifecycleEvents"].as_array().map(Vec::len),
        Some(3)
    );
    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn version_seven_migration_preserves_existing_version_five_delegations() {
    let root = unique_temp_dir("delegation-v5-migration");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('m7-v5-migration', 'C:/fixture/m7-v5-migration', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed registered project");
    drop(database);

    let (_objective_id, task_id, run_id) =
        create_task_and_run(&data_dir, 1, "preserve v5 delegation");
    let packet = json!({
        "schema_version": 1,
        "title": "Preserve this existing handoff",
        "objective": "Verify the additive M7 report migration keeps M6 records intact.",
        "acceptance_criteria": ["Keep the packet hash and dispatched state"],
        "constraints": ["Do not mutate the parent run"],
        "allowed_scope": ["src/**"],
        "referenced_memory_ids": [],
        "referenced_artifact_ids": [],
        "requested_report_fields": ["summary"]
    });
    let prepared = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1,
            "task_id": task_id,
            "run_id": run_id,
            "role": "worker",
            "task_packet": packet,
            "idempotency_key": "m7-v5-existing-delegation"
        }),
    );
    let delegation_id = prepared["id"].as_i64().expect("delegation ID");
    let packet_hash = prepared["packetHash"].clone();
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": delegation_id }),
    );

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open v5 database fixture");
    database
        .execute_batch(
            "DROP TABLE delegation_worker_event_refs;
             DROP TABLE delegation_worker_bindings;
             DROP TABLE codex_subagent_hook_events;
             DROP TABLE delegation_worktree_states;
             DROP TABLE worker_report_reviews;
             DROP TABLE worker_report_quality_check_refs;
             DROP TABLE worker_report_artifact_refs;
             DROP TABLE worker_reports;
             DELETE FROM schema_migrations WHERE version IN (6, 7);",
        )
        .expect("restore the populated version-five schema");
    drop(database);

    let migrated = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://delegations/{delegation_id}") }),
        )],
    );
    let delegation = resource_text(response(&migrated, 2));
    assert_eq!(delegation["packetHash"], packet_hash);
    assert_eq!(delegation["status"], json!("dispatched"));
    assert_eq!(delegation["report"], Value::Null);
    assert_eq!(delegation["worktree"], Value::Null);
    assert_eq!(
        delegation["lifecycleEvents"].as_array().map(Vec::len),
        Some(2)
    );

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open upgraded v7 database");
    let version: i64 = database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read schema version");
    let delegation_count: i64 = database
        .query_row("SELECT COUNT(*) FROM delegations", [], |row| row.get(0))
        .expect("count preserved v5 delegations");
    assert_eq!(version, 7);
    assert_eq!(delegation_count, 1);

    drop(database);
    fs::remove_dir_all(root).expect("remove migration fixture");
}

#[test]
fn version_five_six_and_seven_migrations_preserve_version_four_project_run_and_memory_state() {
    let root = unique_temp_dir("delegation-v4-migration");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized database");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('m6-migration', 'C:/fixture/m6-migration', 'main', 'abc123', 0, '[]', 1, 1)",
            [],
        )
        .expect("seed pre-migration project state");
    drop(database);

    let objective = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "create_objective",
            json!({ "project_id": 1, "title": "Preserved objective" }),
        )],
    );
    assert_eq!(tool_text(response(&objective, 2))["id"], json!(1));
    let task = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "create_task",
            json!({ "objective_id": 1, "title": "Preserved task" }),
        )],
    );
    assert_eq!(tool_text(response(&task, 2))["id"], json!(1));
    let run = run_initialized_server(
        &data_dir,
        &[tool_call(
            2,
            "start_run",
            json!({ "project_id": 1, "objective_id": 1, "task_id": 1 }),
        )],
    );
    assert_eq!(tool_text(response(&run, 2))["id"], json!(1));

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open database to seed the version-four memory row");
    database
        .execute(
            "INSERT INTO memories
             (scope_key, scope_type, scope_id, memory_class, temperature, kind, body, content_hash,
              review_state, created_at, updated_at)
             VALUES ('project:1', 'project', '1', 'canonical', 'warm', 'lesson',
                     'Preserve this version-four memory', 'legacy-memory-hash', 'accepted', 1, 1)",
            [],
        )
        .expect("seed version-four memory state");
    database
        .execute_batch(
            "DROP TABLE delegation_worker_event_refs;
             DROP TABLE delegation_worker_bindings;
             DROP TABLE codex_subagent_hook_events;
             DROP TABLE delegation_worktree_states;
             DROP TRIGGER worker_report_reviews_no_update;
             DROP TRIGGER worker_report_reviews_no_delete;
             DROP TRIGGER worker_report_quality_check_refs_no_update;
             DROP TRIGGER worker_report_quality_check_refs_no_delete;
             DROP TRIGGER worker_report_artifact_refs_no_update;
             DROP TRIGGER worker_report_artifact_refs_no_delete;
             DROP TRIGGER worker_reports_no_update;
             DROP TRIGGER worker_reports_no_delete;
             DROP TABLE worker_report_reviews;
             DROP TABLE worker_report_quality_check_refs;
             DROP TABLE worker_report_artifact_refs;
             DROP TABLE worker_reports;
             DELETE FROM schema_migrations WHERE version IN (6, 7);
             DROP TRIGGER delegation_memory_refs_no_update;
             DROP TRIGGER delegation_memory_refs_no_delete;
             DROP TRIGGER delegation_artifact_refs_no_update;
             DROP TRIGGER delegation_artifact_refs_no_delete;
             DROP TRIGGER delegations_update_guard;
             DROP TRIGGER delegations_no_delete;
             DROP TABLE delegation_memory_refs;
             DROP TABLE delegation_artifact_refs;
             DROP TABLE delegations;
             DELETE FROM schema_migrations WHERE version = 5;",
        )
        .expect("restore a version-four schema fixture");
    drop(database);

    let migrated = run_initialized_server(
        &data_dir,
        &[
            request(
                2,
                "resources/read",
                json!({ "uri": "cathulu://projects/1" }),
            ),
            request(3, "resources/read", json!({ "uri": "cathulu://runs/1" })),
            request(
                4,
                "resources/read",
                json!({ "uri": "cathulu://memories/1" }),
            ),
        ],
    );
    assert_eq!(
        resource_text(response(&migrated, 2))["head"],
        json!("abc123")
    );
    assert_eq!(
        resource_text(response(&migrated, 3))["status"],
        json!("running")
    );
    assert_eq!(
        resource_text(response(&migrated, 4))["body"],
        json!("Preserve this version-four memory")
    );
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open migrated version-five database");
    let version: i64 = database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read schema version");
    let delegation_count: i64 = database
        .query_row("SELECT COUNT(*) FROM delegations", [], |row| row.get(0))
        .expect("query the additive delegation schema");
    assert_eq!(version, 7);
    assert_eq!(delegation_count, 0);
    drop(database);
    fs::remove_dir_all(root).expect("remove migration fixture");
}
