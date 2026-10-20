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
    assert_eq!(tool_names.len(), 2);
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
        Some(1)
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
