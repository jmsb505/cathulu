use cathulu_core::Store;
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
        .env("CATHULU_ROLE_DISPATCHER_ENABLED", "0")
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
        .unwrap_or_else(|| panic!("resource result has no text content: {response}"));
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

fn rollback_schema_twelve(database: &rusqlite::Connection) {
    // These fixtures remove older tables to exercise upgrades with populated legacy data.
    // Remove later dependents and migration markers too, or Store would mistake the
    // partially downgraded fixture for an already current database.
    database
        .execute_batch(
            "DROP TRIGGER IF EXISTS role_capability_provider_attempt_insert_valid;
             DROP TRIGGER IF EXISTS main_capability_provider_attempt_insert_valid;
             DROP TRIGGER IF EXISTS role_capability_provider_attempt_valid;
             DROP TRIGGER IF EXISTS main_capability_provider_attempt_valid;
             DROP TABLE IF EXISTS provider_context_receipts;
             DROP TABLE IF EXISTS provider_attempts;
             DROP TABLE IF EXISTS provider_sessions;
             DELETE FROM schema_migrations WHERE version = 21;
             DROP TRIGGER IF EXISTS project_checkpoint_artifacts_sealed;
             DROP TRIGGER IF EXISTS project_checkpoint_objectives_sealed;
             DROP TRIGGER IF EXISTS project_checkpoint_tasks_sealed;
             DROP TRIGGER IF EXISTS project_checkpoint_seals_no_update;
             DROP TRIGGER IF EXISTS project_checkpoint_seals_no_delete;
             DROP TABLE IF EXISTS project_checkpoint_seals;
             DROP TRIGGER IF EXISTS project_checkpoint_artifacts_no_update;
             DROP TRIGGER IF EXISTS project_checkpoint_artifacts_no_delete;
             DROP TRIGGER IF EXISTS project_checkpoint_objectives_no_update;
             DROP TRIGGER IF EXISTS project_checkpoint_objectives_no_delete;
             DROP TRIGGER IF EXISTS project_checkpoint_tasks_no_update;
             DROP TRIGGER IF EXISTS project_checkpoint_tasks_no_delete;
             DROP TRIGGER IF EXISTS project_checkpoints_no_update;
             DROP TRIGGER IF EXISTS project_checkpoints_no_delete;
             DROP TABLE IF EXISTS project_checkpoint_artifacts;
             DROP TABLE IF EXISTS project_checkpoint_objectives;
             DROP TABLE IF EXISTS project_checkpoint_tasks;
             DROP TABLE IF EXISTS project_checkpoints;
             DELETE FROM schema_migrations WHERE version = 20;
             DROP TRIGGER IF EXISTS task_dependency_scope_insert;
             DROP TRIGGER IF EXISTS task_dependency_acyclic_insert;
             DROP TRIGGER IF EXISTS task_parent_scope_insert;
             DROP TRIGGER IF EXISTS task_parent_scope_update;
             DROP TRIGGER IF EXISTS task_parent_acyclic_insert;
             DROP TRIGGER IF EXISTS task_parent_acyclic_update;
             DROP TRIGGER IF EXISTS task_blocker_no_delete;
             DROP TRIGGER IF EXISTS task_completion_metadata_insert;
             DROP TRIGGER IF EXISTS task_completion_metadata_update;
             DROP TRIGGER IF EXISTS task_completion_run_scope_update;
             DROP TRIGGER IF EXISTS task_cancellation_metadata_insert;
             DROP TRIGGER IF EXISTS task_cancellation_metadata_update;
             DROP TABLE IF EXISTS task_blockers;
             DROP TABLE IF EXISTS task_dependencies;
             DELETE FROM schema_migrations WHERE version = 19;",
        )
        .expect("remove schema-nineteen planning triggers and tables");
    database
        .execute_batch(
            "ALTER TABLE tasks DROP COLUMN cancellation_reason;
             ALTER TABLE tasks DROP COLUMN cancelled_at;
             ALTER TABLE tasks DROP COLUMN completion_run_id;
             ALTER TABLE tasks DROP COLUMN completion_kind;
             ALTER TABLE tasks DROP COLUMN completed_at;
             ALTER TABLE tasks DROP COLUMN revision;
             ALTER TABLE tasks DROP COLUMN next_action;
             ALTER TABLE tasks DROP COLUMN effort_minutes;
             ALTER TABLE tasks DROP COLUMN due_date;
             ALTER TABLE tasks DROP COLUMN priority;
             ALTER TABLE tasks DROP COLUMN parent_task_id;
             ALTER TABLE projects DROP COLUMN revision;
             ALTER TABLE projects DROP COLUMN priority;
             ALTER TABLE projects DROP COLUMN status;",
        )
        .expect("restore project/task rows to the schema-eighteen shape");
    database
        .execute_batch(
            "DROP TABLE IF EXISTS owner_approval_relays;
             DROP TABLE IF EXISTS main_overseer_capability;
             DROP TABLE IF EXISTS role_turn_inbox_events;
             DROP TABLE IF EXISTS role_turn_inbox;
             DROP TABLE IF EXISTS role_event_cursor;
             DROP TABLE IF EXISTS role_thread_capabilities;
             DROP TABLE IF EXISTS role_dispatch_events;
             DROP TABLE IF EXISTS role_dispatch_outbox;
             DROP TABLE IF EXISTS role_thread_bindings;
             DROP TABLE IF EXISTS role_dispatcher_lease;
             DELETE FROM schema_migrations WHERE version >= 14;
             DROP TRIGGER IF EXISTS owner_approval_requests_no_update;
             DROP TRIGGER IF EXISTS owner_approval_requests_no_delete;
             DROP TRIGGER IF EXISTS owner_approval_decisions_no_update;
             DROP TRIGGER IF EXISTS owner_approval_decisions_no_delete;
             DROP TABLE IF EXISTS owner_approval_decisions;
             DROP TABLE IF EXISTS owner_approval_requests;
             DROP TRIGGER IF EXISTS role_work_plans_no_update;
             DROP TRIGGER IF EXISTS role_work_plans_no_delete;
             DROP TRIGGER IF EXISTS role_work_plan_reviews_no_update;
             DROP TRIGGER IF EXISTS role_work_plan_reviews_no_delete;
             DROP TABLE IF EXISTS role_work_plan_reviews;
             DROP TABLE IF EXISTS delegation_work_plan_refs;
             DROP TABLE IF EXISTS role_work_plans;
             ALTER TABLE worker_report_reviews DROP COLUMN reviewer_role;
             DELETE FROM schema_migrations WHERE version = 13;
             DROP TRIGGER IF EXISTS projects_create_logical_pm;
             DROP TRIGGER IF EXISTS logical_agent_role_identity_immutable;
             DROP TRIGGER IF EXISTS project_manager_assignment_role_valid;
             DROP TRIGGER IF EXISTS project_manager_assignment_role_immutable;
             DROP INDEX IF EXISTS project_manager_assignments_role_idx;",
        )
        .expect("remove schema-twelve role triggers and indexes");
    database
        .execute_batch("ALTER TABLE project_manager_assignments DROP COLUMN logical_role_key;")
        .expect("remove schema-twelve assignment binding for migration downgrade fixture");
    database
        .execute_batch(
            "DROP TABLE logical_agent_roles;
             DELETE FROM schema_migrations WHERE version = 12;",
        )
        .expect("remove schema-twelve role catalog");
}

fn hierarchy_packet() -> Value {
    json!({
        "schema_version": 1,
        "title": "Coordinate scoped project work",
        "objective": "Exercise hierarchy admission and lifecycle rules.",
        "acceptance_criteria": ["Persist auditable manager and worker relationships"],
        "constraints": [],
        "allowed_scope": ["src/**"],
        "referenced_memory_ids": [],
        "referenced_artifact_ids": [],
        "requested_report_fields": ["summary", "changed_paths", "checks", "blockers"]
    })
}

fn main_workflow_brief(label: &str) -> Value {
    json!({
        "schema_version": 1,
        "desired_outcome": format!("Complete the bounded {label} project task."),
        "phases": ["Inspect current state", "Perform scoped work", "Verify and report"],
        "project_boundary": ["Work only in the registered project repository"],
        "dependencies": [],
        "acceptance_criteria": ["Provide source-backed evidence and deterministic QA results"],
        "owner_approval_gates": ["Escalate credentials, destructive changes, and out-of-scope work"],
        "known_risks": []
    })
}

fn record_project_manager_plan(data_dir: &Path, manager_id: i64, tasks: &[(&str, Value)]) -> Value {
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open PM planning fixture");
    let (project_id, run_id): (i64, i64) = database
        .query_row(
            "SELECT project_id, run_id FROM delegations WHERE id = ?1",
            [manager_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read PM assignment scope");
    drop(database);
    let plan = json!({
        "schema_version": 1,
        "summary": "Decompose the project brief into reviewable work and report verified outcomes.",
        "manager_actions": ["Coordinate the project and review each Worker report"],
        "worker_tasks": tasks.iter().map(|(key, packet)| json!({
            "task_key": key,
            "task_packet": packet
        })).collect::<Vec<_>>()
    });
    let recorded = call_tool(
        data_dir,
        "record_role_work_plan",
        json!({
            "project_id": project_id,
            "run_id": run_id,
            "plan_level": "project_manager",
            "manager_delegation_id": manager_id,
            "plan": plan,
            "idempotency_key": format!("test-pm-plan-{manager_id}")
        }),
    );
    let plan_id = recorded["id"].as_i64().expect("PM work plan ID");
    call_tool(
        data_dir,
        "review_role_work_plan",
        json!({
            "plan_id": plan_id,
            "reviewer_role": "main_overseer",
            "decision": "accepted",
            "note": "The scoped task breakdown matches the Main project brief.",
            "idempotency_key": format!("test-main-plan-review-{manager_id}")
        }),
    );
    recorded
}

fn submit_and_accept_project_manager_report(data_dir: &Path, manager_id: i64) {
    call_tool(
        data_dir,
        "submit_delegation_report",
        json!({
            "delegation_id": manager_id,
            "worker_report": {
                "schema_version": 1,
                "summary": "The project plan was completed and reviewed against its acceptance criteria.",
                "outcome": "completed",
                "changed_files": [],
                "commits": [],
                "check_results": [],
                "blockers": [],
                "artifact_ids": [],
                "quality_check_ids": []
            },
            "idempotency_key": format!("test-pm-report-{manager_id}")
        }),
    );
    call_tool(
        data_dir,
        "review_delegation_report",
        json!({
            "delegation_id": manager_id,
            "reviewer_role": "main_overseer",
            "decision": "accepted",
            "note": "The Project Manager report is accepted as a report; deterministic QA remains authoritative.",
            "idempotency_key": format!("test-main-pm-review-{manager_id}")
        }),
    );
}

fn activation_args(
    delegation_id: i64,
    receiver_scope_id: &str,
    disposition: &str,
    memory_tools_available: bool,
    retrieval_completed: bool,
    memory_references: Vec<Value>,
    idempotency_key: &str,
) -> Value {
    json!({
        "delegation_id": delegation_id,
        "receiver_scope_type": "scope",
        "receiver_scope_id": receiver_scope_id,
        "disposition": disposition,
        "memory_tools_available": memory_tools_available,
        "retrieval_completed": retrieval_completed,
        "memory_references": memory_references,
        "idempotency_key": idempotency_key
    })
}

fn activate_with_no_relevant_memory(
    data_dir: &Path,
    delegation_id: i64,
    receiver_scope_id: &str,
    idempotency_key: &str,
) {
    call_tool(
        data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "task objective constraints acceptance",
            "receiver_scope_type": "scope", "receiver_scope_id": receiver_scope_id,
            "limit": 10, "delegation_id": delegation_id
        }),
    );
    let receipt = call_tool(
        data_dir,
        "record_delegation_activation",
        activation_args(
            delegation_id,
            receiver_scope_id,
            "no_relevant_memory",
            true,
            true,
            Vec::new(),
            idempotency_key,
        ),
    );
    assert_eq!(receipt["status"], json!("ready"));
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
    call_tool(
        data_dir,
        "record_role_work_plan",
        json!({
            "project_id": project_id,
            "run_id": run_id,
            "plan_level": "main",
            "plan": main_workflow_brief(label),
            "idempotency_key": format!("test-main-plan-{run_id}")
        }),
    );
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
        "reviewer_role": "main_overseer",
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
            "reviewer_role": "main_overseer",
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
    let project_summary = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project",
            "scope_id": project_ids[0].to_string(),
            "memory_class": "canonical",
            "temperature": "hot",
            "kind": "summary",
            "body": "Synthetic project context for the worker-report contract fixture.",
            "sources": [{ "source_type": "project", "source_id": project_ids[0].to_string() }],
            "artifact_ids": [],
            "importance": 0.9,
            "confidence": 0.95,
            "active": true,
            "protected": false
        }),
    );
    let project_summary_id = project_summary["id"].as_i64().expect("summary memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": project_summary_id, "decision": "accepted" }),
    );
    call_tool(
        &data_dir,
        "record_project_memory_review",
        json!({
            "run_id": parent_run_id,
            "disposition": "knowledge_saved",
            "reason_code": "project_profile_initialized",
            "memory_ids": [project_summary_id]
        }),
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
fn worker_parent_delegation_is_stored_and_returned_through_public_mcp() {
    let root = unique_temp_dir("hierarchy-parent");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-fixture', 'C:/fixture/hierarchy', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-other-fixture', 'C:/fixture/hierarchy-other', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed second project");
    drop(database);

    let (_, task_id, run_id) = create_task_and_run(&data_dir, 1, "hierarchy");
    let task_packet = json!({
        "schema_version": 1,
        "title": "Coordinate project work",
        "objective": "Record a hierarchy-aware delegation through the public MCP boundary.",
        "acceptance_criteria": ["Persist the manager relationship"],
        "constraints": [],
        "allowed_scope": ["src/**"],
        "referenced_memory_ids": [],
        "referenced_artifact_ids": [],
        "requested_report_fields": ["summary", "changed_paths", "checks", "blockers"]
    });
    let manager = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1,
            "task_id": task_id,
            "run_id": run_id,
            "role": "sub_overseer",
            "task_packet": task_packet,
            "idempotency_key": "hierarchy-manager"
        }),
    );
    let manager_id = manager["id"].as_i64().expect("manager delegation ID");
    activate_with_no_relevant_memory(
        &data_dir,
        manager_id,
        "project-manager:1",
        "hierarchy-manager-activation",
    );
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": manager_id }),
    );
    record_project_manager_plan(
        &data_dir,
        manager_id,
        &[("worker-one", task_packet.clone())],
    );

    let worker = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1,
            "task_id": task_id,
            "run_id": run_id,
            "role": "worker",
            "task_packet": task_packet,
            "plan_task_key": "worker-one",
            "idempotency_key": "hierarchy-worker",
            "parent_delegation_id": manager_id
        }),
    );
    let worker_id = worker["id"].as_i64().expect("worker delegation ID");
    assert_eq!(worker["parentDelegationId"], json!(manager_id));

    let read = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://delegations/{worker_id}") }),
        )],
    );
    let persisted = resource_text(response(&read, 2));
    assert_eq!(persisted["parentDelegationId"], json!(manager_id));
    assert_eq!(persisted["role"], json!("worker"));

    let wrong_role = tool_error(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "task_packet": task_packet,
            "idempotency_key": "hierarchy-worker-as-parent", "parent_delegation_id": worker_id
        }),
    );
    let wrong_role_text = wrong_role
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        wrong_role_text.contains("sub_overseer"),
        "unexpected wrong-parent rejection: {wrong_role}"
    );

    let (_, foreign_task_id, foreign_run_id) =
        create_task_and_run(&data_dir, 2, "foreign-hierarchy");
    let cross_project = tool_error(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 2, "task_id": foreign_task_id, "run_id": foreign_run_id,
            "role": "worker", "task_packet": task_packet,
            "idempotency_key": "hierarchy-cross-project-child", "parent_delegation_id": manager_id
        }),
    );
    let cross_project_text = cross_project
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        cross_project_text.contains("match project, task, and run"),
        "unexpected cross-project rejection: {cross_project}"
    );

    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn project_manager_slot_is_unique_auditable_and_released_on_completion() {
    let root = unique_temp_dir("hierarchy-manager-slot");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-slot-fixture', 'C:/fixture/hierarchy-slot', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    drop(database);

    let (_, first_task, first_run) = create_task_and_run(&data_dir, 1, "first-manager");
    let (_, second_task, second_run) = create_task_and_run(&data_dir, 1, "second-manager");
    let first = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": first_task, "run_id": first_run,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-slot-first-manager"
        }),
    );
    let first_id = first["id"].as_i64().expect("first manager ID");
    assert_eq!(first["projectManagerAssignmentStatus"], json!("active"));

    let rejected = tool_error(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": second_task, "run_id": second_run,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-slot-second-manager"
        }),
    );
    let rejected_again = tool_error(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": second_task, "run_id": second_run,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-slot-second-manager"
        }),
    );
    let rejection_text = rejected
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        rejection_text.contains("active Project Manager"),
        "unexpected rejection: {rejected}"
    );
    assert_eq!(
        rejected_again.pointer("/result/isError"),
        Some(&json!(true)),
        "a rejected idempotency key returns its original rejection"
    );
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open hierarchy database");
    let manager_count: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM project_manager_assignments WHERE project_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("count manager assignments");
    let rejection_count: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM events WHERE aggregate_type = 'run' AND aggregate_id = ?1 AND event_type = 'delegation_admission_rejected'",
            [second_run.to_string()],
            |row| row.get(0),
        )
        .expect("count admission rejection events");
    assert_eq!(
        manager_count, 1,
        "rejection leaves no partial manager assignment"
    );
    assert_eq!(rejection_count, 1, "rejection is auditable on its run");
    drop(database);

    activate_with_no_relevant_memory(
        &data_dir,
        first_id,
        "project-manager:1",
        "hierarchy-slot-first-manager-activation",
    );
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": first_id }),
    );
    record_project_manager_plan(&data_dir, first_id, &[]);
    submit_and_accept_project_manager_report(&data_dir, first_id);
    let completed = call_tool(
        &data_dir,
        "complete_project_manager_assignment",
        json!({ "delegation_id": first_id }),
    );
    assert_eq!(completed["status"], json!("completed"));
    let stale_parent = tool_error(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": first_task, "run_id": first_run,
            "role": "worker", "parent_delegation_id": first_id,
            "task_packet": hierarchy_packet(), "idempotency_key": "hierarchy-slot-stale-parent"
        }),
    );
    let stale_parent_text = stale_parent
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        stale_parent_text.contains("active Project Manager assignment"),
        "unexpected stale-parent rejection: {stale_parent}"
    );
    let second = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": second_task, "run_id": second_run,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-slot-second-manager-after-release"
        }),
    );
    assert!(
        second["id"].as_i64().is_some(),
        "released project slot accepts next manager"
    );
    let third_task_run = create_task_and_run(&data_dir, 1, "third-manager");
    let third_task = third_task_run.1;
    let third_run = third_task_run.2;
    let second_id = second["id"].as_i64().expect("second manager ID");
    call_tool(
        &data_dir,
        "cancel_delegation",
        json!({ "delegation_id": second_id }),
    );
    let third = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": third_task, "run_id": third_run,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-slot-third-manager-after-cancel"
        }),
    );
    assert!(
        third["id"].as_i64().is_some(),
        "cancelled assignment releases its project slot"
    );

    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn concurrent_project_manager_admissions_reserve_only_one_project_slot() {
    let root = unique_temp_dir("hierarchy-manager-race");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-race-fixture', 'C:/fixture/hierarchy-race', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    drop(database);
    let (_, task_id, run_id) = create_task_and_run(&data_dir, 1, "manager-race");
    let first_dir = data_dir.clone();
    let second_dir = data_dir.clone();
    let first = std::thread::spawn(move || {
        let output = run_initialized_server(
            &first_dir,
            &[tool_call(
                2,
                "prepare_delegation",
                json!({
                    "project_id": 1, "task_id": task_id, "run_id": run_id,
                    "role": "sub_overseer", "task_packet": hierarchy_packet(),
                    "idempotency_key": "hierarchy-manager-race-first"
                }),
            )],
        );
        response(&output, 2).clone()
    });
    let second = std::thread::spawn(move || {
        let output = run_initialized_server(
            &second_dir,
            &[tool_call(
                2,
                "prepare_delegation",
                json!({
                    "project_id": 1, "task_id": task_id, "run_id": run_id,
                    "role": "sub_overseer", "task_packet": hierarchy_packet(),
                    "idempotency_key": "hierarchy-manager-race-second"
                }),
            )],
        );
        response(&output, 2).clone()
    });
    let first_result = first.join().expect("first MCP admission completes");
    let second_result = second.join().expect("second MCP admission completes");
    let success_count = [&first_result, &second_result]
        .into_iter()
        .filter(|result| result.pointer("/result/isError") != Some(&json!(true)))
        .count();
    assert_eq!(
        success_count, 1,
        "exactly one concurrent manager admission succeeds"
    );

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open hierarchy database");
    let manager_count: i64 = database
        .query_row("SELECT COUNT(*) FROM project_manager_assignments WHERE project_id = 1 AND status = 'active'", [], |row| row.get(0))
        .expect("count active manager assignments");
    let rejection_count: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM events WHERE aggregate_type = 'run' AND aggregate_id = ?1 AND event_type = 'delegation_admission_rejected'",
            [run_id.to_string()],
            |row| row.get(0),
        )
        .expect("count admission rejection events");
    assert_eq!(
        manager_count, 1,
        "SQLite admits only one active Project Manager"
    );
    assert_eq!(
        rejection_count, 1,
        "the losing concurrent admission is auditable"
    );
    drop(database);

    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn worker_limit_counts_cancelled_replacements_and_rejects_a_third() {
    let root = unique_temp_dir("hierarchy-worker-cap");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-worker-fixture', 'C:/fixture/hierarchy-worker', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    drop(database);
    let (_, task_id, run_id) = create_task_and_run(&data_dir, 1, "worker-cap");
    let manager = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-worker-cap-manager"
        }),
    );
    let manager_id = manager["id"].as_i64().expect("manager ID");
    activate_with_no_relevant_memory(
        &data_dir,
        manager_id,
        "project-manager:1",
        "hierarchy-worker-cap-manager-activation",
    );
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": manager_id }),
    );
    let worker_packet = hierarchy_packet();
    record_project_manager_plan(
        &data_dir,
        manager_id,
        &[
            ("first", worker_packet.clone()),
            ("second", worker_packet.clone()),
            ("third", worker_packet.clone()),
        ],
    );

    let make_worker = |key: &str, task_key: &str| {
        call_tool(
            &data_dir,
            "prepare_delegation",
            json!({
                "project_id": 1, "task_id": task_id, "run_id": run_id,
                "role": "worker", "parent_delegation_id": manager_id,
                "task_packet": hierarchy_packet(), "plan_task_key": task_key, "idempotency_key": key
            }),
        )
    };
    let first = make_worker("hierarchy-worker-cap-first", "first");
    let first_id = first["id"].as_i64().expect("first worker ID");
    call_tool(
        &data_dir,
        "cancel_delegation",
        json!({ "delegation_id": first_id }),
    );
    let first_dir = data_dir.clone();
    let second_dir = data_dir.clone();
    let first_args = tool_call(
        2,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "parent_delegation_id": manager_id,
            "task_packet": hierarchy_packet(), "plan_task_key": "second", "idempotency_key": "hierarchy-worker-cap-second"
        }),
    );
    let second_args = tool_call(
        2,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "parent_delegation_id": manager_id,
            "task_packet": hierarchy_packet(), "plan_task_key": "third", "idempotency_key": "hierarchy-worker-cap-third"
        }),
    );
    let first = std::thread::spawn(move || {
        let output = run_initialized_server(&first_dir, &[first_args]);
        response(&output, 2).clone()
    });
    let second = std::thread::spawn(move || {
        let output = run_initialized_server(&second_dir, &[second_args]);
        response(&output, 2).clone()
    });
    let first_result = first.join().expect("first worker admission completes");
    let second_result = second.join().expect("second worker admission completes");
    let success_count = [&first_result, &second_result]
        .into_iter()
        .filter(|result| result.pointer("/result/isError") != Some(&json!(true)))
        .count();
    assert_eq!(
        success_count, 1,
        "exactly one worker can consume the final assignment slot"
    );
    let (accepted, rejected) = if first_result.pointer("/result/isError") == Some(&json!(true)) {
        (&second_result, &first_result)
    } else {
        (&first_result, &second_result)
    };
    let rejected = rejected.clone();
    let rejection_text = rejected
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        rejection_text.contains("two workers"),
        "unexpected rejection: {rejected}"
    );
    let rejected_key = if first_result.pointer("/result/isError") == Some(&json!(true)) {
        "hierarchy-worker-cap-second"
    } else {
        "hierarchy-worker-cap-third"
    };
    let rejected_again = tool_error(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "parent_delegation_id": manager_id,
            "task_packet": hierarchy_packet(),
            "plan_task_key": if rejected_key == "hierarchy-worker-cap-second" { "second" } else { "third" },
            "idempotency_key": rejected_key
        }),
    );
    assert_eq!(
        rejected_again.pointer("/result/isError"),
        Some(&json!(true)),
        "retrying the losing idempotency key preserves its rejected outcome"
    );
    let accepted_text = accepted
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .expect("accepted worker result has JSON text");
    let second =
        serde_json::from_str::<Value>(accepted_text).expect("accepted worker result parses");
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open hierarchy database");
    let worker_count: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM delegations WHERE parent_delegation_id = ?1",
            [manager_id],
            |row| row.get(0),
        )
        .expect("count worker assignments");
    let rejection_count: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM events WHERE aggregate_type = 'run' AND aggregate_id = ?1 AND event_type = 'delegation_admission_rejected'",
            [run_id.to_string()],
            |row| row.get(0),
        )
        .expect("count admission rejection events");
    assert_eq!(
        worker_count, 2,
        "cancelled worker still consumes the assignment's lifetime cap"
    );
    assert_eq!(
        rejection_count, 1,
        "concurrent third worker rejection is auditable"
    );
    drop(database);

    let unfinished = tool_error(
        &data_dir,
        "complete_project_manager_assignment",
        json!({ "delegation_id": manager_id }),
    );
    let unfinished_text = unfinished
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        unfinished_text.contains("unfinished"),
        "unexpected close rejection: {unfinished}"
    );
    let cancellation_blocked = tool_error(
        &data_dir,
        "cancel_delegation",
        json!({ "delegation_id": manager_id }),
    );
    let cancellation_text = cancellation_blocked
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        cancellation_text.contains("unfinished"),
        "PM cancellation must also preserve unfinished workers: {cancellation_blocked}"
    );
    let second_id = second["id"].as_i64().expect("second worker ID");
    call_tool(
        &data_dir,
        "cancel_delegation",
        json!({ "delegation_id": second_id }),
    );
    submit_and_accept_project_manager_report(&data_dir, manager_id);
    let completed = call_tool(
        &data_dir,
        "complete_project_manager_assignment",
        json!({ "delegation_id": manager_id }),
    );
    assert_eq!(completed["status"], json!("completed"));

    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn worker_task_cannot_be_prepared_until_main_accepts_the_dispatched_pm_plan() {
    let root = unique_temp_dir("hierarchy-parent-dispatch");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-dispatch-fixture', 'C:/fixture/hierarchy-dispatch', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    drop(database);
    let (_, task_id, run_id) = create_task_and_run(&data_dir, 1, "parent-dispatch");
    let manager = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-parent-dispatch-manager"
        }),
    );
    let manager_id = manager["id"].as_i64().expect("manager ID");
    let premature_worker = tool_error(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "parent_delegation_id": manager_id,
            "task_packet": hierarchy_packet(), "plan_task_key": "worker-one",
            "idempotency_key": "hierarchy-parent-dispatch-worker-premature"
        }),
    );
    assert!(premature_worker
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("record a work plan"));
    activate_with_no_relevant_memory(
        &data_dir,
        manager_id,
        "project-manager:1",
        "hierarchy-parent-dispatch-manager-activation",
    );
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": manager_id }),
    );
    record_project_manager_plan(&data_dir, manager_id, &[("worker-one", hierarchy_packet())]);
    let worker = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "parent_delegation_id": manager_id,
            "task_packet": hierarchy_packet(), "plan_task_key": "worker-one",
            "idempotency_key": "hierarchy-parent-dispatch-worker"
        }),
    );
    let worker_id = worker["id"].as_i64().expect("worker ID");
    activate_with_no_relevant_memory(
        &data_dir,
        worker_id,
        "agent:hierarchy-parent-worker",
        "hierarchy-parent-dispatch-worker-activation",
    );
    let dispatched = call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": worker_id }),
    );
    assert_eq!(dispatched["status"], json!("dispatched"));
    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn role_workflow_plans_and_parent_reviews_gate_the_hierarchy() {
    let root = unique_temp_dir("role-plan-review-gates");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized workflow-plan fixture");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('role-plan-fixture', 'C:/fixture/role-plan', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    drop(database);
    let objective = call_tool(
        &data_dir,
        "create_objective",
        json!({ "project_id": 1, "title": "Enforce role planning" }),
    );
    let objective_id = objective["id"].as_i64().expect("objective ID");
    let task = call_tool(
        &data_dir,
        "create_task",
        json!({ "objective_id": objective_id, "title": "Plan and review role work" }),
    );
    let task_id = task["id"].as_i64().expect("task ID");
    let run = call_tool(
        &data_dir,
        "start_run",
        json!({ "project_id": 1, "objective_id": objective_id, "task_id": task_id }),
    );
    let run_id = run["id"].as_i64().expect("run ID");
    let task_packet = hierarchy_packet();
    let prepare_pm = |key: &str| {
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "sub_overseer", "task_packet": task_packet,
            "idempotency_key": key
        })
    };
    let missing_main_brief = tool_error(
        &data_dir,
        "prepare_delegation",
        prepare_pm("role-plan-pm-before-main"),
    );
    assert!(missing_main_brief
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("Main workflow brief"));
    call_tool(
        &data_dir,
        "record_role_work_plan",
        json!({
            "project_id": 1, "run_id": run_id, "plan_level": "main",
            "plan": main_workflow_brief("role-plan enforcement"),
            "idempotency_key": "role-plan-main-brief"
        }),
    );
    let manager = call_tool(&data_dir, "prepare_delegation", prepare_pm("role-plan-pm"));
    let manager_id = manager["id"].as_i64().expect("Project Manager ID");
    activate_with_no_relevant_memory(
        &data_dir,
        manager_id,
        "project-manager:1",
        "role-plan-pm-activation",
    );
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": manager_id }),
    );
    call_tool(
        &data_dir,
        "record_delegation_update",
        json!({
            "delegation_id": manager_id,
            "update_type": "action_required",
            "phase": "coordinating",
            "summary": "An irreversible schema operation needs an authority decision.",
            "action_key": "schema-change-approval",
            "owner_action": "Approve the irreversible schema change",
            "idempotency_key": "role-plan-owner-action"
        }),
    );
    let owner_request = call_tool(
        &data_dir,
        "request_owner_approval",
        json!({
            "manager_delegation_id": manager_id,
            "action_key": "schema-change-approval",
            "request_summary": "The Project Manager escalated an irreversible schema operation.",
            "owner_action": "Approve or decline the schema change",
            "reason": "The action is irreversible and outside autonomous role authority.",
            "idempotency_key": "role-plan-owner-approval"
        }),
    );
    let owner_request_id = owner_request["id"]
        .as_i64()
        .expect("owner approval request ID");
    let owner_request_resource = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://owner-approvals/{owner_request_id}") }),
        )],
    );
    assert_eq!(
        resource_text(response(&owner_request_resource, 2))["decision"],
        Value::Null
    );
    let owner_dashboard = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": "cathulu://dashboard" }),
        )],
    );
    let owner_dashboard = resource_text(response(&owner_dashboard, 2));
    assert_eq!(
        owner_dashboard["pendingOwnerApprovalRequests"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        owner_dashboard["pendingAgentActions"][0]["routedToRole"],
        json!("main_overseer")
    );
    let premature_pm_report = tool_error(
        &data_dir,
        "submit_delegation_report",
        json!({
            "delegation_id": manager_id,
            "worker_report": {
                "schema_version": 1, "summary": "The PM cannot close before the owner responds.",
                "outcome": "completed", "changed_files": [], "commits": [],
                "check_results": [], "blockers": [], "artifact_ids": [], "quality_check_ids": []
            },
            "idempotency_key": "role-plan-owner-report-too-early"
        }),
    );
    assert!(premature_pm_report
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("owner request(s) remain unresolved"));
    let owner_decision_tool_list =
        run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let owner_tools = response(&owner_decision_tool_list, 2)["result"]["tools"]
        .as_array()
        .expect("MCP tool list");
    assert!(!owner_tools
        .iter()
        .any(|tool| { tool["name"] == json!("record_owner_approval_decision") }));
    let request_hash = owner_dashboard["pendingOwnerApprovalRequests"][0]["requestHash"]
        .as_str()
        .expect("snapshot request hash");
    let owner_decision = Store::open(&data_dir)
        .expect("open local owner-action store")
        .record_owner_approval_decision_from_desktop(owner_request_id, request_hash, "approved")
        .expect("record explicit desktop owner decision");
    assert_eq!(owner_decision.decision, "approved");
    assert_eq!(owner_decision.decision_origin, "desktop_owner_ui");
    let after_owner_response = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": "cathulu://dashboard" }),
        )],
    );
    let after_owner_response = resource_text(response(&after_owner_response, 2));
    assert_eq!(
        after_owner_response["pendingOwnerApprovalRequests"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        after_owner_response["pendingAgentActions"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    // Approval records human intent; the PM must still close the underlying action
    // before its result can be tested against the separate Worker-review gate.
    call_tool(
        &data_dir,
        "record_delegation_update",
        json!({
            "delegation_id": manager_id,
            "update_type": "action_resolved",
            "phase": "coordinating",
            "summary": "The approved action is handled in this synthetic fixture.",
            "action_key": "schema-change-approval",
            "idempotency_key": "role-plan-owner-action-resolved"
        }),
    );

    let manager_plan = call_tool(
        &data_dir,
        "record_role_work_plan",
        json!({
            "project_id": 1, "run_id": run_id, "plan_level": "project_manager",
            "manager_delegation_id": manager_id,
            "plan": {
                "schema_version": 1,
                "summary": "Split the brief into one bounded Worker task and review its evidence.",
                "manager_actions": ["Review Worker output before reporting to Main"],
                "worker_tasks": [{ "task_key": "worker-one", "task_packet": task_packet }]
            },
            "idempotency_key": "role-plan-pm-plan"
        }),
    );
    let plan_id = manager_plan["id"]
        .as_i64()
        .expect("Project Manager plan ID");
    let worker_args = json!({
        "project_id": 1, "task_id": task_id, "run_id": run_id,
        "role": "worker", "parent_delegation_id": manager_id,
        "task_packet": task_packet, "plan_task_key": "worker-one",
        "idempotency_key": "role-plan-worker"
    });
    let unreviewed_plan = tool_error(&data_dir, "prepare_delegation", worker_args.clone());
    assert!(unreviewed_plan
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("Main Overseer must accept"));
    let plan_review = call_tool(
        &data_dir,
        "review_role_work_plan",
        json!({
            "plan_id": plan_id, "reviewer_role": "main_overseer", "decision": "accepted",
            "note": "The task, project boundary, and acceptance evidence match the Main brief.",
            "idempotency_key": "role-plan-main-accepts-pm-plan"
        }),
    );
    assert_eq!(plan_review["decision"], json!("accepted"));
    let mut mismatched_packet = task_packet.clone();
    mismatched_packet["objective"] =
        json!("A different objective outside the reviewed task packet.");
    let mismatch = tool_error(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "parent_delegation_id": manager_id,
            "task_packet": mismatched_packet, "plan_task_key": "worker-one",
            "idempotency_key": "role-plan-worker-mismatch"
        }),
    );
    assert!(mismatch
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("does not match"));

    let worker = call_tool(&data_dir, "prepare_delegation", worker_args);
    let worker_id = worker["id"].as_i64().expect("Worker ID");
    activate_with_no_relevant_memory(
        &data_dir,
        worker_id,
        "agent:role-plan-worker",
        "role-plan-worker-activation",
    );
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": worker_id }),
    );
    call_tool(
        &data_dir,
        "submit_delegation_report",
        json!({
            "delegation_id": worker_id,
            "worker_report": {
                "schema_version": 1, "summary": "The planned Worker checks completed.",
                "outcome": "completed", "changed_files": [], "commits": [],
                "check_results": [], "blockers": [], "artifact_ids": [], "quality_check_ids": []
            },
            "idempotency_key": "role-plan-worker-report"
        }),
    );
    let pm_report_too_early = tool_error(
        &data_dir,
        "submit_delegation_report",
        json!({
            "delegation_id": manager_id,
            "worker_report": {
                "schema_version": 1, "summary": "Project work reported.",
                "outcome": "completed", "changed_files": [], "commits": [],
                "check_results": [], "blockers": [], "artifact_ids": [], "quality_check_ids": []
            },
            "idempotency_key": "role-plan-pm-report-too-early"
        }),
    );
    assert!(pm_report_too_early
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("Worker report(s) are accepted"));
    let wrong_worker_reviewer = tool_error(
        &data_dir,
        "review_delegation_report",
        json!({
            "delegation_id": worker_id, "reviewer_role": "main_overseer",
            "decision": "accepted", "note": "Wrong hierarchy level.",
            "idempotency_key": "role-plan-wrong-worker-reviewer"
        }),
    );
    assert!(wrong_worker_reviewer
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("project_manager"));
    call_tool(
        &data_dir,
        "review_delegation_report",
        json!({
            "delegation_id": worker_id, "reviewer_role": "project_manager",
            "decision": "accepted", "note": "Worker report satisfies its planned packet.",
            "idempotency_key": "role-plan-pm-accepts-worker"
        }),
    );
    call_tool(
        &data_dir,
        "submit_delegation_report",
        json!({
            "delegation_id": manager_id,
            "worker_report": {
                "schema_version": 1, "summary": "The Worker result was reviewed and the project brief is satisfied.",
                "outcome": "completed", "changed_files": [], "commits": [],
                "check_results": [], "blockers": [], "artifact_ids": [], "quality_check_ids": []
            },
            "idempotency_key": "role-plan-pm-result"
        }),
    );
    let completion_before_main_review = tool_error(
        &data_dir,
        "complete_project_manager_assignment",
        json!({ "delegation_id": manager_id }),
    );
    assert!(completion_before_main_review
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .contains("Main accepts"));
    call_tool(
        &data_dir,
        "review_delegation_report",
        json!({
            "delegation_id": manager_id, "reviewer_role": "main_overseer",
            "decision": "accepted", "note": "PM result reviewed against Main brief and Worker review.",
            "idempotency_key": "role-plan-main-accepts-pm-result"
        }),
    );
    let completed = call_tool(
        &data_dir,
        "complete_project_manager_assignment",
        json!({ "delegation_id": manager_id }),
    );
    assert_eq!(completed["status"], json!("completed"));

    let plan_resource = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://work-plans/{plan_id}") }),
        )],
    );
    let plan = resource_text(response(&plan_resource, 2));
    assert_eq!(plan["latestReview"]["decision"], json!("accepted"));
    fs::remove_dir_all(root).expect("remove workflow-plan fixture");
}

#[test]
fn hierarchy_dispatch_requires_a_ready_memory_activation_receipt() {
    let root = unique_temp_dir("hierarchy-memory-activation");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-memory-fixture', 'C:/fixture/hierarchy-memory', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    drop(database);
    let (_, task_id, run_id) = create_task_and_run(&data_dir, 1, "memory-activation");
    let manager = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-memory-manager"
        }),
    );
    let manager_id = manager["id"].as_i64().expect("manager ID");

    let wrong_scope_search = tool_error(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "project manager role memory",
            "receiver_scope_type": "scope", "receiver_scope_id": "replacement-chat",
            "limit": 10, "delegation_id": manager_id
        }),
    );
    assert!(
        wrong_scope_search
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("persistent role receiver scope"),
        "PM retrieval must use the persistent role scope: {wrong_scope_search}"
    );

    let missing = tool_error(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": manager_id }),
    );
    let missing_text = missing
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        missing_text.contains("memory activation receipt"),
        "unexpected rejection: {missing}"
    );

    let incomplete = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "memory_tools_unavailable",
            false,
            false,
            Vec::new(),
            "hierarchy-memory-manager-incomplete",
        ),
    );
    assert_eq!(incomplete["status"], json!("incomplete"));
    let blocked = tool_error(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": manager_id }),
    );
    let blocked_text = blocked
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        blocked_text.contains("incomplete"),
        "unexpected rejection: {blocked}"
    );

    let claimed_without_read = tool_error(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "no_relevant_memory",
            true,
            true,
            Vec::new(),
            "hierarchy-memory-manager-unverified-read",
        ),
    );
    assert!(
        claimed_without_read
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("successful bounded Cathulu memory search"),
        "a boolean claim alone cannot satisfy activation: {claimed_without_read}"
    );
    call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "hierarchy memory activation",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "limit": 10, "delegation_id": manager_id
        }),
    );

    let ready = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "no_relevant_memory",
            true,
            true,
            Vec::new(),
            "hierarchy-memory-manager-ready",
        ),
    );
    assert_eq!(ready["status"], json!("ready"));
    assert_eq!(ready["revision"], json!(2));
    assert!(ready["retrievalEventId"].as_i64().is_some());
    let dispatched_manager = call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": manager_id }),
    );
    assert_eq!(dispatched_manager["status"], json!("dispatched"));
    record_project_manager_plan(&data_dir, manager_id, &[("worker-one", hierarchy_packet())]);

    let worker = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "parent_delegation_id": manager_id,
            "task_packet": hierarchy_packet(), "plan_task_key": "worker-one",
            "idempotency_key": "hierarchy-memory-worker"
        }),
    );
    let worker_id = worker["id"].as_i64().expect("worker ID");
    let worker_blocked = tool_error(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": worker_id }),
    );
    assert!(
        worker_blocked
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("memory activation receipt"),
        "a Worker also needs activation evidence: {worker_blocked}"
    );
    call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "worker task objective constraints",
            "receiver_scope_type": "scope", "receiver_scope_id": "agent:m23-worker",
            "limit": 10, "delegation_id": worker_id
        }),
    );
    let worker_activation = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            worker_id,
            "agent:m23-worker",
            "no_relevant_memory",
            true,
            true,
            Vec::new(),
            "hierarchy-memory-worker-ready",
        ),
    );
    assert_eq!(worker_activation["status"], json!("ready"));
    assert_eq!(
        call_tool(
            &data_dir,
            "mark_delegation_dispatched",
            json!({ "delegation_id": worker_id }),
        )["status"],
        json!("dispatched")
    );

    let read = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://delegations/{manager_id}") }),
        )],
    );
    let manager_record = resource_text(response(&read, 2));
    assert_eq!(
        manager_record["activationReceipt"]["status"],
        json!("ready")
    );
    assert_eq!(
        manager_record["activationReceipt"]["disposition"],
        json!("no_relevant_memory")
    );
    assert_eq!(
        manager_record["activationReceipt"]["memoryReferences"],
        json!([])
    );

    fs::remove_dir_all(root).expect("remove temporary test data");
}

#[test]
fn logical_roles_and_known_memory_survive_project_manager_reassignment() {
    let root = unique_temp_dir("logical-agent-roles");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized role registry");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('role-project-a', 'C:/fixture/role-project-a', 'main', NULL, 0, '[]', 1, 1),
                    ('role-project-b', 'C:/fixture/role-project-b', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("register projects through the normal project table");
    drop(database);

    let roles_response = run_initialized_server(
        &data_dir,
        &[
            request(2, "resources/read", json!({ "uri": "cathulu://roles" })),
            request(3, "resources/read", json!({ "uri": "cathulu://dashboard" })),
        ],
    );
    let roles = resource_text(response(&roles_response, 2));
    assert_eq!(roles.as_array().map(Vec::len), Some(3));
    assert_eq!(
        roles
            .as_array()
            .expect("role catalog")
            .iter()
            .filter(|role| role["roleKind"] == "main_overseer")
            .count(),
        1,
        "only one persistent Main Overseer role exists"
    );
    assert_eq!(
        roles
            .as_array()
            .expect("role catalog")
            .iter()
            .filter(|role| role["roleKind"] == "project_manager")
            .count(),
        2,
        "each registered project has one persistent Project Manager role"
    );
    assert_eq!(roles[0]["roleKey"], json!("main-overseer"));
    assert_eq!(roles[1]["roleKey"], json!("project-manager:1"));
    assert_eq!(roles[1]["memoryScopeKey"], json!("project:1"));
    assert_eq!(
        roles[1]["receiverScopeKey"],
        json!("scope:project-manager:1")
    );
    assert_eq!(roles[1]["toolProfileKey"], json!("project-manager-v1"));
    assert_eq!(
        resource_text(response(&roles_response, 3))["logicalRoles"],
        roles,
        "the dashboard snapshot uses the same authoritative role catalog"
    );

    let main_memory = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "global", "scope_id": null,
            "memory_class": "canonical", "temperature": "hot", "kind": "constraint",
            "body": "Main Overseer state belongs to the one persistent logical role.",
            "sources": [{ "source_type": "human_instruction", "source_id": "main-role-test" }],
            "importance": 0.9, "confidence": 1.0, "active": true, "protected": false
        }),
    );
    let main_memory_id = main_memory["id"].as_i64().expect("Main role memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": main_memory_id, "decision": "accepted" }),
    );
    let first_main_transfer = call_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": main_memory_id,
            "sender_scope_type": "global", "sender_scope_id": null,
            "receiver_scope_type": "scope", "receiver_scope_id": "main-overseer",
            "retrieval_method": "hot"
        }),
    );
    assert_eq!(first_main_transfer["action"], json!("full"));
    let replacement_main_transfer = call_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": main_memory_id,
            "sender_scope_type": "global", "sender_scope_id": null,
            "receiver_scope_type": "scope", "receiver_scope_id": "main-overseer",
            "retrieval_method": "hot"
        }),
    );
    assert_eq!(
        replacement_main_transfer["action"],
        json!("omitted_known"),
        "a replacement Main chat reuses the same logical receiver scope"
    );

    let (_, first_task_id, first_run_id) =
        create_task_and_run(&data_dir, 1, "first role activation");
    let first_manager = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": first_task_id, "run_id": first_run_id,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "persistent-role-first-assignment"
        }),
    );
    let first_id = first_manager["id"].as_i64().expect("first PM assignment");
    let memory = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": "1",
            "memory_class": "canonical", "temperature": "warm", "kind": "lesson",
            "body": "This verified lesson remains known to the persistent Project Manager role.",
            "sources": [{ "source_type": "human_instruction", "source_id": "logical-role-test" }],
            "importance": 0.8, "confidence": 0.95, "active": true, "protected": false
        }),
    );
    let memory_id = memory["id"].as_i64().expect("canonical memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": memory_id, "decision": "accepted" }),
    );
    let first_search = call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "verified lesson persistent project manager",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "limit": 10, "delegation_id": first_id
        }),
    );
    assert!(first_search["warm"]
        .as_array()
        .unwrap()
        .iter()
        .any(|candidate| candidate["memoryId"] == json!(memory_id)));
    let transfer = call_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": memory_id, "sender_scope_type": "project", "sender_scope_id": "1",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "run_id": first_run_id, "retrieval_method": "explicit", "token_count": 16
        }),
    );
    let first_activation = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            first_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": memory_id,
                "content_hash": memory["contentHash"],
                "delivery": "transferred",
                "transfer_id": transfer["transferId"]
            })],
            "persistent-role-first-activation",
        ),
    );
    assert_eq!(first_activation["status"], json!("ready"));
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": first_id }),
    );
    record_project_manager_plan(&data_dir, first_id, &[]);
    submit_and_accept_project_manager_report(&data_dir, first_id);
    let completed = call_tool(
        &data_dir,
        "complete_project_manager_assignment",
        json!({ "delegation_id": first_id }),
    );
    assert_eq!(completed["logicalRoleKey"], json!("project-manager:1"));

    let (_, second_task_id, second_run_id) =
        create_task_and_run(&data_dir, 1, "replacement role activation");
    let second_manager = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": second_task_id, "run_id": second_run_id,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "persistent-role-second-assignment"
        }),
    );
    let second_id = second_manager["id"]
        .as_i64()
        .expect("replacement PM assignment");
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open reassigned PM role state");
    let role_links: (i64, i64) = database
        .query_row(
            "SELECT COUNT(*), COUNT(DISTINCT logical_role_key)
             FROM project_manager_assignments WHERE project_id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read repeated assignment role links");
    drop(database);
    assert_eq!(
        role_links,
        (2, 1),
        "replacement chat assignment reuses the same logical role"
    );

    let second_search = call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "verified lesson persistent project manager",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "limit": 10, "delegation_id": second_id
        }),
    );
    assert!(second_search["alreadyKnown"]
        .as_array()
        .unwrap()
        .iter()
        .any(|candidate| candidate["memoryId"] == json!(memory_id)));
    let second_activation = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            second_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": memory_id,
                "content_hash": memory["contentHash"],
                "delivery": "already_known"
            })],
            "persistent-role-second-activation",
        ),
    );
    assert_eq!(second_activation["status"], json!("ready"));
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": second_id }),
    );
    let final_snapshot = run_initialized_server(
        &data_dir,
        &[
            request(2, "resources/read", json!({ "uri": "cathulu://roles" })),
            request(3, "resources/read", json!({ "uri": "cathulu://dashboard" })),
        ],
    );
    assert_eq!(resource_text(response(&final_snapshot, 2)), roles);
    let dashboard = resource_text(response(&final_snapshot, 3));
    let assignment = dashboard["delegations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|delegation| delegation["id"] == json!(second_id))
        .expect("active PM assignment appears in the dashboard role projection");
    assert_eq!(assignment["logicalRoleKey"], json!("project-manager:1"));
    assert_eq!(
        assignment["projectManagerAssignmentStatus"],
        json!("active")
    );
    fs::remove_dir_all(root).expect("remove logical role fixture");
}

#[test]
fn activation_receipts_require_current_transfers_for_the_exact_receiver() {
    let root = unique_temp_dir("hierarchy-memory-transfer");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-transfer-fixture', 'C:/fixture/hierarchy-transfer', 'main', NULL, 0, '[]', 1, 1),
                    ('hierarchy-transfer-foreign', 'C:/fixture/hierarchy-transfer-foreign', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    drop(database);
    let (_, task_id, run_id) = create_task_and_run(&data_dir, 1, "memory-transfer");
    let manager = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "hierarchy-transfer-manager"
        }),
    );
    let manager_id = manager["id"].as_i64().expect("manager ID");
    let memory = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": "1",
            "memory_class": "canonical", "temperature": "warm", "kind": "contract",
            "body": "The adapter contract requires stable request IDs.",
            "sources": [{ "source_type": "human_instruction", "source_id": "m23-test" }],
            "importance": 0.8, "confidence": 0.95, "active": true, "protected": false
        }),
    );
    let memory_id = memory["id"].as_i64().expect("memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": memory_id, "decision": "accepted" }),
    );
    let foreign_transfer = call_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": memory_id, "sender_scope_type": "project", "sender_scope_id": "1",
            "receiver_scope_type": "scope", "receiver_scope_id": "agent:someone-else",
            "run_id": run_id, "retrieval_method": "explicit", "token_count": 12
        }),
    );
    call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "adapter contract stable request IDs",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "limit": 10, "delegation_id": manager_id
        }),
    );
    let foreign_rejected = tool_error(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": memory_id,
                "content_hash": memory["contentHash"],
                "delivery": "transferred",
                "transfer_id": foreign_transfer["transferId"]
            })],
            "hierarchy-transfer-wrong-receiver",
        ),
    );
    assert!(
        foreign_rejected
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("receiver scope"),
        "foreign transfer cannot activate this delegation: {foreign_rejected}"
    );

    let transfer = call_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": memory_id, "sender_scope_type": "project", "sender_scope_id": "1",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "run_id": run_id, "retrieval_method": "explicit", "token_count": 12
        }),
    );
    let receipt = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": memory_id,
                "content_hash": memory["contentHash"],
                "delivery": "transferred",
                "transfer_id": transfer["transferId"]
            })],
            "hierarchy-transfer-valid",
        ),
    );
    assert_eq!(receipt["status"], json!("ready"));
    assert_eq!(
        receipt["memoryReferences"][0]["contentHash"],
        memory["contentHash"]
    );
    let idempotent_receipt = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": memory_id,
                "content_hash": memory["contentHash"],
                "delivery": "transferred",
                "transfer_id": transfer["transferId"]
            })],
            "hierarchy-transfer-valid",
        ),
    );
    assert_eq!(idempotent_receipt["id"], receipt["id"]);

    let repeated_transfer = call_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": memory_id, "sender_scope_type": "project", "sender_scope_id": "1",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "run_id": run_id, "retrieval_method": "explicit", "token_count": 12
        }),
    );
    assert_eq!(repeated_transfer["action"], json!("omitted_known"));
    call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "adapter contract stable request IDs",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "limit": 10, "delegation_id": manager_id
        }),
    );
    let known_receipt = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": memory_id,
                "content_hash": memory["contentHash"],
                "delivery": "already_known"
            })],
            "hierarchy-transfer-known",
        ),
    );
    assert_eq!(known_receipt["revision"], json!(2));
    assert_eq!(
        known_receipt["memoryReferences"][0]["delivery"],
        json!("already_known")
    );

    let successor = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": "1",
            "memory_class": "canonical", "temperature": "warm", "kind": "contract",
            "body": "The adapter contract requires stable request IDs and bounded retries.",
            "sources": [{ "source_type": "human_instruction", "source_id": "m23-test-successor" }],
            "importance": 0.9, "confidence": 0.96, "active": true, "protected": false,
            "supersedes_id": memory_id
        }),
    );
    let successor_id = successor["id"].as_i64().expect("successor memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": successor_id, "decision": "accepted" }),
    );
    let delta_transfer = call_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": successor_id, "sender_scope_type": "project", "sender_scope_id": "1",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "run_id": run_id, "retrieval_method": "explicit", "token_count": 15
        }),
    );
    assert_eq!(delta_transfer["action"], json!("delta"));
    call_tool(
        &data_dir,
        "search_memory_candidates",
        json!({
            "scope_type": "project", "scope_id": "1",
            "query": "adapter contract stable request IDs bounded retries",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "limit": 10, "delegation_id": manager_id
        }),
    );
    let delta_receipt = call_tool(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": successor_id,
                "content_hash": successor["contentHash"],
                "delivery": "transferred",
                "transfer_id": delta_transfer["transferId"]
            })],
            "hierarchy-transfer-delta",
        ),
    );
    assert_eq!(delta_receipt["revision"], json!(3));
    let superseded_rejected = tool_error(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": memory_id,
                "content_hash": memory["contentHash"],
                "delivery": "transferred",
                "transfer_id": transfer["transferId"]
            })],
            "hierarchy-transfer-superseded",
        ),
    );
    assert!(
        superseded_rejected
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("stale"),
        "superseded knowledge cannot be activated as current: {superseded_rejected}"
    );

    let foreign_memory = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": "2",
            "memory_class": "canonical", "temperature": "warm", "kind": "contract",
            "body": "This contract belongs to the independent foreign project.",
            "sources": [{ "source_type": "human_instruction", "source_id": "m23-foreign" }],
            "importance": 0.7, "confidence": 0.9, "active": true, "protected": false
        }),
    );
    let foreign_memory_id = foreign_memory["id"].as_i64().expect("foreign memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": foreign_memory_id, "decision": "accepted" }),
    );
    let foreign_scope_rejected = tool_error(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": foreign_memory_id,
                "content_hash": foreign_memory["contentHash"],
                "delivery": "already_known"
            })],
            "hierarchy-transfer-foreign-memory",
        ),
    );
    assert!(
        foreign_scope_rejected
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("outside the delegation project/task scope"),
        "foreign project memory cannot be activated: {foreign_scope_rejected}"
    );

    let hot_memory = call_tool(
        &data_dir,
        "create_memory",
        json!({
            "scope_type": "project", "scope_id": "1",
            "memory_class": "working", "temperature": "hot", "kind": "summary",
            "body": "Active HOT context for budget validation.",
            "sources": [], "importance": 0.5, "confidence": 0.8,
            "active": true, "protected": false
        }),
    );
    let hot_memory_id = hot_memory["id"].as_i64().expect("HOT memory ID");
    call_tool(
        &data_dir,
        "review_memory_candidate",
        json!({ "memory_id": hot_memory_id, "decision": "accepted" }),
    );
    let hot_transfer = call_tool(
        &data_dir,
        "prepare_context_transfer",
        json!({
            "memory_id": hot_memory_id, "sender_scope_type": "project", "sender_scope_id": "1",
            "receiver_scope_type": "scope", "receiver_scope_id": "project-manager:1",
            "run_id": run_id, "retrieval_method": "explicit", "token_count": 8
        }),
    );
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open activation budget fixture");
    database
        .execute(
            "UPDATE memory_scope_budgets SET hot_budget_bytes = 0 WHERE scope_key = 'project:1'",
            [],
        )
        .expect("lower HOT budget for validation");
    drop(database);
    let over_budget_rejected = tool_error(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": hot_memory_id,
                "content_hash": hot_memory["contentHash"],
                "delivery": "transferred",
                "transfer_id": hot_transfer["transferId"]
            })],
            "hierarchy-transfer-over-budget",
        ),
    );
    assert!(
        over_budget_rejected
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("exceeds its 0-byte budget"),
        "over-budget HOT memory cannot be activated: {over_budget_rejected}"
    );
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("restore HOT memory budget fixture");
    database
        .execute(
            "UPDATE memory_scope_budgets SET hot_budget_bytes = 8192 WHERE scope_key = 'project:1'",
            [],
        )
        .expect("restore HOT budget");
    database
        .execute(
            "UPDATE memories SET temperature = 'cold' WHERE id = ?1",
            [hot_memory_id],
        )
        .expect("demote HOT memory for validation");
    drop(database);
    let cold_rejected = tool_error(
        &data_dir,
        "record_delegation_activation",
        activation_args(
            manager_id,
            "project-manager:1",
            "context_selected",
            true,
            true,
            vec![json!({
                "memory_id": hot_memory_id,
                "content_hash": hot_memory["contentHash"],
                "delivery": "transferred",
                "transfer_id": hot_transfer["transferId"]
            })],
            "hierarchy-transfer-cold",
        ),
    );
    assert!(
        cold_rejected
            .pointer("/result/content/0/text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("COLD activation memory"),
        "COLD memory cannot be activated automatically: {cold_rejected}"
    );
    assert_eq!(
        call_tool(
            &data_dir,
            "mark_delegation_dispatched",
            json!({ "delegation_id": manager_id }),
        )["status"],
        json!("dispatched")
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
    assert!(tool_names.contains(&"record_delegation_activation"));
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
fn version_eleven_migration_preserves_existing_version_five_delegations() {
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
    rollback_schema_twelve(&database);
    database
        .execute_batch(
            "DROP TABLE delegation_activation_memory_refs;
             DROP TABLE delegation_activation_receipts;
             DROP TABLE delegation_worker_event_refs;
             DROP TABLE delegation_worker_bindings;
             DROP TABLE codex_subagent_hook_events;
             DROP TABLE delegation_worktree_states;
             DROP TABLE worker_report_reviews;
             DROP TABLE worker_report_quality_check_refs;
             DROP TABLE worker_report_artifact_refs;
             DROP TABLE worker_reports;
             DROP TABLE codex_selector_preferences;
             DROP TABLE project_graphify_snapshots;
             DROP TRIGGER project_manager_assignment_identity_immutable;
             DROP TABLE project_manager_assignments;
             DROP TRIGGER delegations_parent_immutable;
             DROP INDEX delegations_parent_idx;
             ALTER TABLE delegations DROP COLUMN parent_delegation_id;
             DELETE FROM schema_migrations WHERE version IN (6, 7, 8, 9, 10, 11);",
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
    assert_eq!(delegation["parentDelegationId"], Value::Null);
    assert_eq!(delegation["projectManagerAssignmentStatus"], Value::Null);
    assert_eq!(delegation["activationReceipt"], Value::Null);
    assert_eq!(
        delegation["lifecycleEvents"].as_array().map(Vec::len),
        Some(2)
    );

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open upgraded version-eleven database");
    let version: i64 = database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read schema version");
    let delegation_count: i64 = database
        .query_row("SELECT COUNT(*) FROM delegations", [], |row| row.get(0))
        .expect("count preserved v5 delegations");
    assert_eq!(version, 21);
    assert_eq!(delegation_count, 1);
    let assigned_count: i64 = database
        .query_row(
            "SELECT COUNT(*) FROM project_manager_assignments",
            [],
            |row| row.get(0),
        )
        .expect("count hierarchy-aware assignments");
    assert_eq!(
        assigned_count, 0,
        "migration does not infer legacy manager assignments"
    );

    drop(database);
    fs::remove_dir_all(root).expect("remove migration fixture");
}

#[test]
fn version_eleven_migration_preserves_existing_hierarchy_links() {
    let root = unique_temp_dir("delegation-v10-migration");
    let data_dir = root.join("data");
    run_initialized_server(&data_dir, &[request(2, "tools/list", json!({}))]);
    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open initialized fixture store");
    database
        .execute(
            "INSERT INTO projects (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('hierarchy-v10-fixture', 'C:/fixture/hierarchy-v10', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    drop(database);
    let (_, task_id, run_id) = create_task_and_run(&data_dir, 1, "v10 hierarchy preservation");
    let manager = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "sub_overseer", "task_packet": hierarchy_packet(),
            "idempotency_key": "v10-preserve-manager"
        }),
    );
    let manager_id = manager["id"].as_i64().expect("manager ID");
    activate_with_no_relevant_memory(
        &data_dir,
        manager_id,
        "project-manager:1",
        "v10-preserve-manager-activation",
    );
    call_tool(
        &data_dir,
        "mark_delegation_dispatched",
        json!({ "delegation_id": manager_id }),
    );
    record_project_manager_plan(
        &data_dir,
        manager_id,
        &[("preserved-worker", hierarchy_packet())],
    );
    let worker = call_tool(
        &data_dir,
        "prepare_delegation",
        json!({
            "project_id": 1, "task_id": task_id, "run_id": run_id,
            "role": "worker", "parent_delegation_id": manager_id,
            "task_packet": hierarchy_packet(), "plan_task_key": "preserved-worker",
            "idempotency_key": "v10-preserve-worker"
        }),
    );
    let worker_id = worker["id"].as_i64().expect("worker ID");

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open version-ten fixture");
    rollback_schema_twelve(&database);
    database
        .execute_batch(
            "DROP TABLE delegation_activation_memory_refs;
             DROP TABLE delegation_activation_receipts;
             DELETE FROM schema_migrations WHERE version = 11;",
        )
        .expect("restore the populated version-ten schema");
    drop(database);

    let migrated = run_initialized_server(
        &data_dir,
        &[request(
            2,
            "resources/read",
            json!({ "uri": format!("cathulu://delegations/{worker_id}") }),
        )],
    );
    let worker_record = resource_text(response(&migrated, 2));
    assert_eq!(worker_record["parentDelegationId"], json!(manager_id));
    assert_eq!(worker_record["activationReceipt"], Value::Null);
    assert_eq!(worker_record["projectManagerAssignmentStatus"], Value::Null);

    let database = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open upgraded version-eleven fixture");
    let version: i64 = database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read schema version");
    assert_eq!(version, 21);
    let assignment_status: String = database
        .query_row(
            "SELECT status FROM project_manager_assignments WHERE manager_delegation_id = ?1",
            [manager_id],
            |row| row.get(0),
        )
        .expect("read preserved manager assignment");
    assert_eq!(assignment_status, "active");

    drop(database);
    fs::remove_dir_all(root).expect("remove migration fixture");
}

#[test]
fn version_five_through_eleven_migrations_preserve_version_four_project_run_and_memory_state() {
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
    rollback_schema_twelve(&database);
    database
        .execute_batch(
            "DROP TABLE delegation_activation_memory_refs;
             DROP TABLE delegation_activation_receipts;
             DROP TABLE delegation_worker_event_refs;
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
             DROP TABLE codex_selector_preferences;
             DROP TABLE project_graphify_snapshots;
             DROP TRIGGER project_manager_assignment_identity_immutable;
             DROP TABLE project_manager_assignments;
             DROP TRIGGER delegations_parent_immutable;
             DROP INDEX delegations_parent_idx;
             DELETE FROM schema_migrations WHERE version IN (6, 7, 8, 9, 10, 11);
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
        .expect("open migrated version-ten database");
    let version: i64 = database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read schema version");
    let delegation_count: i64 = database
        .query_row("SELECT COUNT(*) FROM delegations", [], |row| row.get(0))
        .expect("query the additive delegation schema");
    assert_eq!(version, 21);
    assert_eq!(delegation_count, 0);
    drop(database);
    fs::remove_dir_all(root).expect("remove migration fixture");
}
