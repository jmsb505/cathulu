use cathulu_core::delegation::DelegationTaskPacket;
use cathulu_core::dispatcher::role_allowed_tools;
use cathulu_core::Store;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_data_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    std::env::current_dir()
        .expect("resolve repository root")
        .join("target")
        .join("manual-dev")
        .join("codex")
        .join(format!(
            "cathulu-role-capability-{}-{stamp}",
            std::process::id()
        ))
}

fn insert_workspace_lease_fixture(
    store: &Store,
    project_id: i64,
    delegation_id: i64,
    role_kind: &str,
    binding_key: &str,
    access: &str,
) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_secs() as i64;
    let connection = Connection::open(store.database_path()).expect("open lease fixture DB");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable foreign keys");
    connection
        .execute(
            "INSERT INTO workspace_leases
             (project_id, delegation_id, role_kind, role_binding_key, access, owner_id,
              workspace_path, workspace_path_hash, common_dir_hash, base_commit, status,
              lease_expires_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'role-capability-fixture',
                     ?6, ?7, ?8, 'abcdef0', 'active', ?9, ?10, ?10)",
            rusqlite::params![
                project_id,
                delegation_id,
                role_kind,
                binding_key,
                access,
                format!("C:/fixture/role-capability-worktree/{delegation_id}"),
                format!("{delegation_id:064x}"),
                "b".repeat(64),
                now + 3600,
                now
            ],
        )
        .expect("insert active role workspace lease");
    connection.last_insert_rowid()
}

fn task_packet(title: &str) -> DelegationTaskPacket {
    DelegationTaskPacket {
        schema_version: 1,
        title: title.to_string(),
        objective: "Verify role-scoped Cathulu tool access.".to_string(),
        acceptance_criteria: vec![
            "Only the assigned project and delegation are accessible.".to_string()
        ],
        constraints: vec![],
        allowed_scope: vec!["src/**".to_string()],
        referenced_memory_ids: vec![],
        referenced_artifact_ids: vec![],
        requested_report_fields: vec!["summary".to_string(), "checks".to_string()],
    }
}

fn main_brief() -> Value {
    json!({
        "schema_version": 1,
        "desired_outcome": "Verify scoped role capabilities.",
        "phases": ["Prepare roles", "Check scope", "Record evidence"],
        "project_boundary": ["Use only the assigned fixture project."],
        "dependencies": [],
        "acceptance_criteria": ["Role tools enforce stored assignment scope."],
        "owner_approval_gates": ["Do not perform external or destructive work."],
        "known_risks": []
    })
}

fn activate_assignment(store: &Store, delegation_id: i64, project_id: i64, receiver: &str) {
    let project_scope_id = project_id.to_string();
    let candidates = store
        .search_memory_candidates(
            "project",
            Some(&project_scope_id),
            "scope role capability objective",
            "scope",
            Some(receiver),
            10,
        )
        .expect("search scoped project memory");
    let references = candidates
        .hot
        .iter()
        .chain(candidates.warm.iter())
        .chain(candidates.already_known.iter())
        .map(|candidate| (candidate.memory_id, candidate.content_hash.clone()))
        .collect::<Vec<_>>();
    store
        .record_delegation_memory_retrieval(
            delegation_id,
            "project",
            Some(&project_scope_id),
            "scope",
            receiver,
            "search_memory_candidates",
            &references,
        )
        .expect("record scoped memory retrieval");
    store
        .record_delegation_activation(
            delegation_id,
            "scope",
            receiver,
            if references.is_empty() {
                "no_relevant_memory"
            } else {
                "context_selected"
            },
            true,
            true,
            &references
                .iter()
                .map(|(memory_id, content_hash)| {
                    cathulu_core::delegation::ActivationMemoryReference {
                        memory_id: *memory_id,
                        content_hash: content_hash.clone(),
                        delivery: "already_known".to_string(),
                        transfer_id: None,
                    }
                })
                .collect::<Vec<_>>(),
            &format!("role-capability-activation-{delegation_id}"),
        )
        .expect("record complete memory activation");
}

fn role_mcp_responses(
    data_dir: &PathBuf,
    capability: &str,
    extra_requests: &[Value],
) -> Vec<Value> {
    let mut requests = vec![
        json!({
            "jsonrpc":"2.0", "id":1, "method":"initialize",
            "params":{
                "protocolVersion":"2025-11-25", "capabilities":{},
                "clientInfo":{"name":"role-capability-test", "version":"0.1.0"}
            }
        }),
        json!({ "jsonrpc":"2.0", "method":"notifications/initialized" }),
        json!({ "jsonrpc":"2.0", "id":2, "method":"tools/list", "params":{} }),
        json!({
            "jsonrpc":"2.0", "id":3, "method":"tools/call",
            "params":{"name":"review_role_work_plan", "arguments":{}}
        }),
    ];
    requests.extend_from_slice(extra_requests);
    let mut command = Command::new(env!("CARGO_BIN_EXE_cathulu-mcp"));
    command
        .env("CATHULU_DATA_DIR", data_dir)
        .env("CATHULU_ROLE_CAPABILITY", capability)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cathulu_core::suppress_windows_console(&mut command);
    let mut child = command.spawn().expect("start isolated role MCP server");
    {
        let stdin = child.stdin.as_mut().expect("child stdin");
        for request in requests {
            serde_json::to_writer(&mut *stdin, &request).expect("serialize MCP request");
            stdin.write_all(b"\n").expect("terminate MCP request line");
        }
    }
    let output = child
        .wait_with_output()
        .expect("wait for isolated role MCP server");
    assert!(
        output.status.success(),
        "role MCP server failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("MCP output is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("MCP stdout is JSON-RPC"))
        .collect()
}

#[test]
fn per_thread_capabilities_are_hashed_revocable_and_argument_scoped() {
    let data_dir = temporary_data_dir();
    let store = Store::open(&data_dir).expect("initialize fixture store");
    let connection = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open fixture database");
    connection
        .execute(
            "INSERT INTO projects
             (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('role-capability-fixture', 'C:/fixture/role-capability', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed project");
    let project_id = connection.last_insert_rowid();
    drop(connection);

    let objective = store
        .create_objective(
            project_id,
            "Capability scope",
            "Verify stored role boundaries",
        )
        .expect("create objective");
    let task = store
        .create_task(
            objective.id,
            "Exercise role boundaries",
            "Use a local fixture only",
        )
        .expect("create task");
    let run = store
        .start_run(project_id, Some(objective.id), Some(task.id), None)
        .expect("start fixture run");
    store
        .record_role_work_plan(
            project_id,
            run.id,
            "main",
            None,
            &main_brief(),
            "capability-main-plan",
        )
        .expect("record Main project brief");

    let manager = store
        .prepare_delegation(
            project_id,
            task.id,
            run.id,
            "sub_overseer",
            None,
            &task_packet("Project Manager"),
            None,
            "capability-manager-assignment",
            None,
            None,
            None,
        )
        .expect("prepare Project Manager");
    activate_assignment(
        &store,
        manager.id,
        project_id,
        &format!("project-manager:{project_id}"),
    );
    store
        .mark_delegation_dispatched(manager.id)
        .expect("dispatch Project Manager assignment");
    let manager_binding = format!("project-manager:{project_id}");
    let manager_lease_id = insert_workspace_lease_fixture(
        &store,
        project_id,
        manager.id,
        "sub_overseer",
        &manager_binding,
        "read_only",
    );
    let manager_capability = store
        .issue_role_thread_capability_for_workspace(
            &manager_binding,
            "sub_overseer",
            project_id,
            manager.id,
            manager_lease_id,
        )
        .expect("issue Project Manager capability");
    assert_eq!(manager_capability.token.len(), 32);
    let manager_scope = store
        .resolve_role_thread_capability(&manager_capability.token)
        .expect("resolve Project Manager capability");
    assert_eq!(manager_scope.role_kind, "sub_overseer");
    assert_eq!(manager_scope.project_id, Some(project_id));
    assert_eq!(manager_scope.delegation_id, Some(manager.id));
    assert_eq!(manager_scope.capability_hash.len(), 64);
    let stored_capability_hash = Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open capability hash database")
        .query_row(
            "SELECT capability_hash FROM role_thread_capabilities WHERE binding_key = ?1",
            [&manager_binding],
            |row| row.get::<_, String>(0),
        )
        .expect("read stored capability hash");
    assert_eq!(manager_scope.capability_hash, stored_capability_hash);
    assert_ne!(manager_scope.capability_hash, manager_capability.token);

    store
        .authorize_role_tool_call(
            &manager_scope,
            "read_memories",
            &json!({"scope_type":"project", "scope_id":project_id.to_string()}),
        )
        .expect("Project Manager can read its project memory");
    assert!(store
        .authorize_role_tool_call(
            &manager_scope,
            "read_memories",
            &json!({"scope_type":"global", "scope_id":null}),
        )
        .is_err());
    assert!(store
        .authorize_role_tool_call(&manager_scope, "review_role_work_plan", &json!({}))
        .is_err());
    assert!(!role_allowed_tools("sub_overseer").contains(&"review_role_work_plan"));
    let role_responses = role_mcp_responses(
        &data_dir,
        &manager_capability.token,
        &[
            json!({
                "jsonrpc": "2.0", "id": 4, "method": "resources/read",
                "params": {"uri": format!("cathulu://projects/{project_id}/resume")}
            }),
            json!({
                "jsonrpc": "2.0", "id": 5, "method": "resources/read",
                "params": {"uri": format!("cathulu://projects/{}/resume", project_id + 1)}
            }),
            json!({
                "jsonrpc": "2.0", "id": 6, "method": "resources/read",
                "params": {"uri": format!("cathulu://projects/{project_id}/checkpoints/latest")}
            }),
            json!({
                "jsonrpc": "2.0", "id": 7, "method": "resources/read",
                "params": {"uri": format!("cathulu://projects/{}/checkpoints/latest", project_id + 1)}
            }),
        ],
    );
    let listed_tools = role_responses
        .iter()
        .find(|message| message.get("id") == Some(&json!(2)))
        .expect("tool list response")
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("role tool catalog");
    assert!(listed_tools
        .iter()
        .any(|tool| tool["name"] == "prepare_delegation"));
    assert!(!listed_tools
        .iter()
        .any(|tool| tool["name"] == "review_role_work_plan"));
    let denied_call = role_responses
        .iter()
        .find(|message| message.get("id") == Some(&json!(3)))
        .expect("restricted tool response");
    assert!(denied_call.get("error").is_some());
    for id in [4, 6] {
        assert!(role_responses
            .iter()
            .find(|message| message.get("id") == Some(&json!(id)))
            .unwrap()
            .pointer("/result/contents/0/text")
            .is_some());
    }
    for id in [5, 7] {
        assert!(role_responses
            .iter()
            .find(|message| message.get("id") == Some(&json!(id)))
            .unwrap()
            .get("error")
            .is_some());
    }

    let manager_plan = json!({
        "schema_version": 1,
        "summary": "Delegate one scoped verification task.",
        "manager_actions": ["Review the Worker result."],
        "worker_tasks": [{"task_key":"worker-one", "task_packet":task_packet("Worker") }]
    });
    let plan = store
        .record_role_work_plan(
            project_id,
            run.id,
            "project_manager",
            Some(manager.id),
            &manager_plan,
            "capability-manager-plan",
        )
        .expect("record Project Manager plan");
    store
        .review_role_work_plan(
            plan.id,
            "main_overseer",
            "accepted",
            "Fixture acceptance for scoped worker delegation.",
            "capability-main-plan-review",
        )
        .expect("accept Project Manager plan");
    let worker = store
        .prepare_delegation(
            project_id,
            task.id,
            run.id,
            "worker",
            Some(manager.id),
            &task_packet("Worker"),
            Some("worker-one"),
            "capability-worker-assignment",
            None,
            None,
            None,
        )
        .expect("prepare Worker");
    activate_assignment(
        &store,
        worker.id,
        project_id,
        &format!("worker:{}", worker.id),
    );
    store
        .mark_delegation_dispatched(worker.id)
        .expect("dispatch Worker assignment");
    let worker_binding = format!("worker:{}", worker.id);
    let worker_lease_id = insert_workspace_lease_fixture(
        &store,
        project_id,
        worker.id,
        "worker",
        &worker_binding,
        "workspace_write",
    );
    let worker_capability = store
        .issue_role_thread_capability_for_workspace(
            &worker_binding,
            "worker",
            project_id,
            worker.id,
            worker_lease_id,
        )
        .expect("issue Worker capability");
    let worker_scope = store
        .resolve_role_thread_capability(&worker_capability.token)
        .expect("resolve Worker capability");
    assert_eq!(worker_scope.role_kind, "worker");
    store
        .authorize_role_tool_call(
            &worker_scope,
            "record_delegation_update",
            &json!({"delegation_id":worker.id}),
        )
        .expect("Worker can update its own assignment");
    assert!(store
        .authorize_role_tool_call(
            &worker_scope,
            "record_delegation_update",
            &json!({"delegation_id":manager.id}),
        )
        .is_err());
    assert!(store
        .authorize_role_tool_call(&worker_scope, "dispatch_prepared_delegations", &json!({}))
        .is_err());

    store
        .revoke_role_thread_capability(&worker_binding)
        .expect("revoke Worker capability");
    assert!(store
        .resolve_role_thread_capability(&worker_capability.token)
        .is_err());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn global_main_capability_can_review_and_escalate_but_cannot_implement_or_dispatch() {
    let data_dir = temporary_data_dir();
    let store = Store::open(&data_dir).expect("initialize fixture store");
    let connection = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open fixture database");
    connection
        .execute(
            "INSERT INTO projects
             (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES ('main-capability-fixture', 'C:/fixture/main-capability', 'main', NULL, 0, '[]', 1, 1)",
            [],
        )
        .expect("seed Main role fixture project");
    let project_id = connection.last_insert_rowid();
    drop(connection);
    let objective = store
        .create_objective(
            project_id,
            "Main capability",
            "Exercise the global Main scope",
        )
        .expect("create objective");
    let task = store
        .create_task(objective.id, "Review PM plan", "Use fixture-only state")
        .expect("create task");
    let run = store
        .start_run(project_id, Some(objective.id), Some(task.id), None)
        .expect("start fixture run");
    store
        .record_role_work_plan(
            project_id,
            run.id,
            "main",
            None,
            &main_brief(),
            "main-capability-brief",
        )
        .expect("record Main brief");
    let manager = store
        .prepare_delegation(
            project_id,
            task.id,
            run.id,
            "sub_overseer",
            None,
            &task_packet("PM"),
            None,
            "main-capability-manager",
            None,
            None,
            None,
        )
        .expect("prepare PM");
    activate_assignment(
        &store,
        manager.id,
        project_id,
        &format!("project-manager:{project_id}"),
    );
    store
        .mark_delegation_dispatched(manager.id)
        .expect("dispatch PM");
    let plan = store
        .record_role_work_plan(
            project_id,
            run.id,
            "project_manager",
            Some(manager.id),
            &json!({
                "schema_version": 1,
                "summary": "A bounded fixture plan.",
                "manager_actions": ["Review only this fixture."],
                "worker_tasks": []
            }),
            "main-capability-pm-plan",
        )
        .expect("record PM plan");

    let capability = store
        .issue_main_overseer_capability()
        .expect("issue global Main capability");
    let scope = store
        .resolve_role_thread_capability(&capability.token)
        .expect("resolve global Main capability");
    assert_eq!(scope.role_kind, "main_overseer");
    assert_eq!(scope.project_id, None);
    assert_eq!(scope.delegation_id, None);
    store
        .authorize_role_tool_call(
            &scope,
            "read_memories",
            &json!({"scope_type":"project", "scope_id":project_id.to_string()}),
        )
        .expect("Main can read any registered project's memory");
    store
        .authorize_role_tool_call(
            &scope,
            "review_role_work_plan",
            &json!({"plan_id":plan.id, "reviewer_role":"main_overseer"}),
        )
        .expect("Main can review the active PM's plan");
    assert!(store
        .authorize_role_tool_call(
            &scope,
            "read_memories",
            &json!({"scope_type":"project", "scope_id":"999999"}),
        )
        .is_err());
    assert!(store
        .authorize_role_tool_call(
            &scope,
            "read_memories",
            &json!({"scope_type":"global", "scope_id":null, "include_cold":true}),
        )
        .is_err());
    assert!(store
        .authorize_role_tool_call(&scope, "prepare_delegation", &json!({}))
        .is_err());
    assert!(store
        .authorize_role_tool_call(&scope, "dispatch_prepared_delegations", &json!({}))
        .is_err());

    let role_responses = role_mcp_responses(
        &data_dir,
        &capability.token,
        &[
            json!({
                "jsonrpc": "2.0", "id": 4, "method": "resources/read",
                "params": {"uri": "cathulu://portfolio/tasks"}
            }),
            json!({
                "jsonrpc": "2.0", "id": 5, "method": "resources/read",
                "params": {"uri": format!("cathulu://projects/{project_id}/planning")}
            }),
            json!({
                "jsonrpc": "2.0", "id": 6, "method": "resources/read",
                "params": {"uri": format!("cathulu://projects/{project_id}/resume")}
            }),
        ],
    );
    let listed_tools = role_responses
        .iter()
        .find(|message| message.get("id") == Some(&json!(2)))
        .expect("Main tool list response")
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("Main tool catalog");
    assert!(listed_tools
        .iter()
        .any(|tool| tool["name"] == "review_role_work_plan"));
    assert!(listed_tools
        .iter()
        .any(|tool| tool["name"] == "relay_owner_decision_to_project_manager"));
    assert!(!listed_tools
        .iter()
        .any(|tool| tool["name"] == "prepare_delegation"));
    for id in [4, 5, 6] {
        assert!(role_responses
            .iter()
            .find(|message| message.get("id") == Some(&json!(id)))
            .unwrap()
            .pointer("/result/contents/0/text")
            .is_some());
    }
    store
        .revoke_role_thread_capability("main-overseer")
        .expect("revoke global Main capability");
    assert!(store
        .resolve_role_thread_capability(&capability.token)
        .is_err());

    let _ = std::fs::remove_dir_all(data_dir);
}
