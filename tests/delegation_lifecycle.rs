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
    assert!(!tool_names.iter().any(|name| name.contains("spawn")));
    assert!(!tool_names.iter().any(|name| {
        name.contains("execute") || name.contains("config") || name.contains("worktree")
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
fn version_five_migration_preserves_version_four_project_run_and_memory_state() {
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
            "DROP TRIGGER delegation_memory_refs_no_update;
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
    assert_eq!(version, 5);
    assert_eq!(delegation_count, 0);
    drop(database);
    fs::remove_dir_all(root).expect("remove migration fixture");
}
