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
    std::env::temp_dir().join(format!("cathulu-{label}-{}-{stamp}", std::process::id()))
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
    assert_eq!(
        response(&discovery, 3)
            .pointer("/result/resources")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );

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
fn m1_project_rows_survive_additive_m2_migration_and_restart() {
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
    let response = call_tool(
        data_dir,
        "finish_run",
        json!({ "run_id": run_id, "summary": "Finished test run." }),
    );
    tool_text(&response)
}

fn register_repo(data_dir: &Path, repo: &Path) -> i64 {
    let response = call_tool(data_dir, "register_project", json!({ "path": repo }));
    tool_text(&response)["id"].as_i64().expect("project ID")
}

fn call_tool(data_dir: &Path, name: &str, arguments: Value) -> Value {
    let responses = run_initialized_server(data_dir, &[tool_call(2, name, arguments)]);
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
    let root = unique_temp_dir("invalid-repo");
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
