use cathulu_core::{
    agent_provider::WorkspaceAccess,
    delegation::DelegationTaskPacket,
    memory::{FreshnessBasis, MemoryCreateInput, MemorySource},
    workflow::TaskPlanInput,
    ProjectKind, ProjectVisibility, Store,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

fn scratch_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    let root = std::env::current_dir()
        .expect("repository root")
        .join("target/manual-dev/codex/cx06-m50")
        .join(format!("{label}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&root).expect("create isolated M50 scratch folder");
    root
}

fn mcp_responses(data_dir: &Path, capability: Option<&str>, requests: &[Value]) -> Vec<Value> {
    let mut all_requests = vec![
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "m50-isolated-test", "version": "0.1.0" }
            }
        }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ];
    all_requests.extend_from_slice(requests);

    let mut command = Command::new(env!("CARGO_BIN_EXE_cathulu-mcp"));
    command
        .env("CATHULU_DATA_DIR", data_dir)
        .env("CATHULU_ROLE_DISPATCHER_ENABLED", "0")
        .env("CATHULU_ROLE_DISPATCHER_CLIENT_ENABLED", "0")
        .env("CATHULU_ROLE_DISPATCHER_ALLOWED_CREDENTIAL_SOURCES", "")
        .env("CATHULU_ROLE_DISPATCHER_NETWORK_ACCESS", "0")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_BASE_URL")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(capability) = capability {
        command.env("CATHULU_ROLE_CAPABILITY", capability);
    } else {
        command.env_remove("CATHULU_ROLE_CAPABILITY");
    }
    cathulu_core::suppress_windows_console(&mut command);
    let mut child = command.spawn().expect("start isolated M50 MCP process");
    {
        let stdin = child.stdin.as_mut().expect("child stdin");
        for request in all_requests {
            serde_json::to_writer(&mut *stdin, &request).expect("serialize MCP request");
            stdin.write_all(b"\n").expect("terminate MCP request line");
        }
    }
    let output = child
        .wait_with_output()
        .expect("wait for isolated M50 MCP process");
    assert!(
        output.status.success(),
        "isolated MCP process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("MCP output is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("MCP stdout is JSON-RPC"))
        .collect()
}

fn response(responses: &[Value], id: i64) -> &Value {
    responses
        .iter()
        .find(|message| message.get("id") == Some(&json!(id)))
        .unwrap_or_else(|| panic!("missing response id {id}: {responses:?}"))
}

fn tool_value(response: &Value) -> Value {
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

fn resource_value(response: &Value) -> Value {
    let text = response
        .pointer("/result/contents/0/text")
        .and_then(Value::as_str)
        .expect("resource result has text content");
    serde_json::from_str(text).expect("resource text is JSON")
}

#[test]
fn non_code_project_supports_provider_free_tasks_checkpoints_and_resume() {
    let root = scratch_root("provider-free");
    let data_dir = root.join("data");
    let store = Store::open(&data_dir).expect("open isolated store");

    let project = store
        .create_non_code_project("Personal planning")
        .expect("create pathless non-code project");
    assert_eq!(project.path, None);
    assert_eq!(project.kind, ProjectKind::NonCode);
    assert_eq!(project.visibility, ProjectVisibility::Private);

    let related_private = store
        .create_non_code_project("Another private project")
        .expect("create second local-only project");
    assert_eq!(
        store
            .record_dependency(project.id, related_private.id, "depends_on")
            .unwrap_err(),
        "private_project_dependency_requires_owner_grant"
    );
    let dependency_count: i64 = Connection::open(store.database_path())
        .expect("open dependency fixture")
        .query_row("SELECT COUNT(*) FROM project_dependencies", [], |row| {
            row.get(0)
        })
        .expect("read dependency fixture count");
    assert_eq!(dependency_count, 0, "private edge was recorded");

    let roles = store.logical_agent_roles().expect("read logical roles");
    let project_role = roles
        .iter()
        .find(|role| role.role_key == format!("project-manager:{}", project.id))
        .expect("non-code project keeps its stable PM role");
    assert_eq!(
        project_role.memory_scope_key,
        format!("project:{}", project.id)
    );

    let objective = store
        .create_objective(project.id, "Provider-free work", "Local planning only")
        .expect("create objective without Git");
    let task = store
        .create_task_with_plan(
            objective.id,
            &TaskPlanInput {
                title: "Review upcoming commitments".into(),
                description: String::new(),
                parent_task_id: None,
                priority: 0,
                due_date: None,
                effort_minutes: None,
                next_action: Some("List the next action".into()),
            },
        )
        .expect("create task without Git");
    let resume = store
        .resume_project(project.id, 10)
        .expect("resume without Git");
    assert_eq!(resume.repository.availability, "not_applicable");
    assert_eq!(resume.repository.freshness, "not_applicable");
    assert!(resume.remaining.iter().any(|item| item.id == task.id));

    let checkpoint = store
        .create_project_checkpoint(
            project.id,
            resume.source_event_id,
            "Provider-free checkpoint",
            &[],
        )
        .expect("checkpoint without Git");
    assert_eq!(checkpoint.repository_head, None);
    assert!(!checkpoint.repository_dirty);
    assert_eq!(
        store
            .latest_project_checkpoint(project.id)
            .expect("read latest checkpoint")
            .expect("checkpoint exists")
            .id,
        checkpoint.id
    );

    let graphify = store
        .trace_code_path(project.id, "source", "target", None, None, 2)
        .expect("Graphify refuses a project with no repository");
    assert_eq!(graphify.status, "unavailable");
    assert_eq!(
        graphify.reason.as_deref(),
        Some("project_has_no_repository")
    );

    let run = store
        .start_run(project.id, Some(objective.id), Some(task.id), None)
        .expect("record provider-free local run state");
    let packet = DelegationTaskPacket {
        schema_version: 1,
        title: "Must stay local".into(),
        objective: "No provider start for a private non-code project".into(),
        acceptance_criteria: vec!["No provider is started".into()],
        constraints: vec![],
        allowed_scope: vec!["local-only".into()],
        referenced_memory_ids: vec![],
        referenced_artifact_ids: vec![],
        requested_report_fields: vec!["summary".into()],
    };
    assert_eq!(
        store
            .prepare_delegation(
                project.id,
                task.id,
                run.id,
                "worker",
                None,
                &packet,
                None,
                "m50-pathless-delegation",
                None,
                None,
                None,
            )
            .unwrap_err(),
        "project_has_no_repository"
    );

    // Seed a prepared legacy assignment to exercise queue and workspace admission's independent
    // path checks. Normal preparation above already refuses before creating this row.
    let database = Connection::open(store.database_path()).expect("open isolated dispatch fixture");
    let packet_json = serde_json::to_string(&packet).expect("serialize dispatch fixture packet");
    database
        .execute(
            "INSERT INTO delegations
             (project_id, task_id, run_id, role, status, packet_schema_version, packet_json,
              packet_hash, request_hash, idempotency_key_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'worker', 'prepared', 1, ?4, ?5, ?6, ?7, 1, 1)",
            params![
                project.id,
                task.id,
                run.id,
                packet_json,
                "a".repeat(64),
                "b".repeat(64),
                "c".repeat(64),
            ],
        )
        .expect("insert isolated pathless admission fixture");
    let delegation_id = database.last_insert_rowid();
    drop(database);
    assert_eq!(
        store.enqueue_role_dispatch(delegation_id).unwrap_err(),
        "project_has_no_repository"
    );
    assert_eq!(
        store
            .claim_delegation_workspace(
                delegation_id,
                "cx06-m50-test",
                std::path::Path::new("target/manual-dev/codex/cx06-m50/not-a-worktree"),
                WorkspaceAccess::ReadOnly,
                1_800_000_000,
                60,
            )
            .unwrap_err(),
        "project_has_no_repository"
    );
    let admission_state =
        Connection::open(store.database_path()).expect("inspect isolated admission state");
    for table in [
        "role_dispatch_outbox",
        "workspace_leases",
        "provider_attempts",
    ] {
        let rows: i64 = admission_state
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count provider admission side effects");
        assert_eq!(rows, 0, "pathless project reached {table}");
    }
    drop(admission_state);

    assert_eq!(
        store.reconcile_project(project.id).unwrap_err(),
        "not_applicable:project_has_no_repository"
    );
    let report = store
        .reconcile_registered_projects()
        .expect("bulk reconciliation skips no-repository projects");
    assert_eq!(report.checked, 2);
    assert_eq!(report.refreshed, 0);
    assert_eq!(report.skipped, 2);
    assert!(report.failures.is_empty());

    let main_capability = store
        .issue_main_overseer_capability()
        .expect("issue isolated Main capability");
    let main_scope = store
        .resolve_role_thread_capability(&main_capability.token)
        .expect("resolve Main capability");
    let private_memory_read = store.authorize_role_tool_call(
        &main_scope,
        "read_memories",
        &json!({
            "scope_type": "project",
            "scope_id": project.id.to_string(),
            "include_cold": false
        }),
    );
    assert!(private_memory_read.is_err());

    drop(store);
    fs::remove_dir_all(root).expect("remove isolated M50 fixture");
}

#[test]
fn populated_v22_project_rows_migrate_without_losing_ids_roles_memory_or_children() {
    let root = scratch_root("populated-migration");
    let data_dir = root.join("data");
    let store = Store::open(&data_dir).expect("open isolated v23 store");
    let database_path = store.database_path().to_path_buf();

    let database = Connection::open(&database_path).expect("open fixture database");
    let registered_at = 1_700_000_000_i64;
    let mut project_ids = Vec::new();
    for index in 0..3 {
        database
            .execute(
                "INSERT INTO projects
                 (name, path, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
                 VALUES (?1, ?2, ?3, 0, '[]', ?4, ?4)",
                params![
                    format!("Legacy repo {index}"),
                    format!("/fixture/legacy-{index}"),
                    format!("commit-{index}"),
                    registered_at + index
                ],
            )
            .expect("insert legacy repository project");
        project_ids.push(database.last_insert_rowid());
    }
    let first_project_id = project_ids[0];
    drop(database);

    store
        .record_dependency(project_ids[0], project_ids[1], "depends_on")
        .expect("record shared repository dependency");
    let objective = store
        .create_objective(first_project_id, "Legacy objective", "Keep this record")
        .expect("create legacy objective");
    let task = store
        .create_task_with_plan(
            objective.id,
            &TaskPlanInput {
                title: "Legacy task".into(),
                description: String::new(),
                parent_task_id: None,
                priority: 4,
                due_date: None,
                effort_minutes: None,
                next_action: None,
            },
        )
        .expect("create legacy task");
    let memory = store
        .create_memory(&MemoryCreateInput {
            scope_type: "project".into(),
            scope_id: Some(first_project_id.to_string()),
            memory_class: "canonical".into(),
            temperature: "warm".into(),
            kind: "fact".into(),
            body: "migrationproof retained searchable memory".into(),
            sources: vec![MemorySource {
                source_type: "project".into(),
                source_id: first_project_id.to_string(),
            }],
            artifact_ids: vec![],
            valid_from: None,
            valid_until: None,
            supersedes_id: None,
            freshness_basis: FreshnessBasis {
                repo_id: Some(first_project_id),
                git_commit: Some("legacy-commit".into()),
                ..FreshnessBasis::default()
            },
            importance: 0.9,
            confidence: 0.95,
            active: true,
            protected: false,
        })
        .expect("create existing project memory");
    store
        .review_memory_candidate(memory.id, "accepted")
        .expect("accept existing memory");
    let resume = store
        .resume_project(first_project_id, 10)
        .expect("read populated project state");
    store
        .create_project_checkpoint(
            first_project_id,
            resume.source_event_id,
            "Legacy checkpoint",
            &[],
        )
        .expect("create legacy checkpoint");

    let sequence_high_water = 500_i64;
    let downgrade = Connection::open(&database_path).expect("open database for v22 fixture");
    downgrade
        .execute_batch(&format!(
            "PRAGMA foreign_keys = OFF;
             BEGIN IMMEDIATE;
             DROP INDEX IF EXISTS projects_repository_path_unique;
             DROP TRIGGER IF EXISTS projects_create_logical_pm;
             CREATE TABLE projects_v22 (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               name TEXT NOT NULL,
               path TEXT NOT NULL UNIQUE,
               git_branch TEXT,
               git_head TEXT,
               git_dirty INTEGER NOT NULL DEFAULT 0,
               changed_paths_json TEXT NOT NULL DEFAULT '[]',
               registered_at INTEGER NOT NULL,
               reconciled_at INTEGER NOT NULL,
               status TEXT NOT NULL DEFAULT 'active'
                 CHECK(status IN ('active', 'paused', 'completed', 'archived')),
               priority INTEGER NOT NULL DEFAULT 0 CHECK(priority BETWEEN -100 AND 100),
               revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0)
             );
             INSERT INTO projects_v22
               (id, name, path, git_branch, git_head, git_dirty, changed_paths_json,
                registered_at, reconciled_at, status, priority, revision)
             SELECT id, name, path, git_branch, git_head, git_dirty, changed_paths_json,
                    registered_at, reconciled_at, status, priority, revision FROM projects;
             DROP TABLE projects;
             ALTER TABLE projects_v22 RENAME TO projects;
             CREATE TRIGGER projects_create_logical_pm AFTER INSERT ON projects
             BEGIN
               INSERT INTO logical_agent_roles
                 (role_key, role_kind, project_id, memory_scope_key, receiver_scope_key,
                  tool_profile_key, created_at)
               VALUES ('project-manager:' || NEW.id, 'project_manager', NEW.id,
                       'project:' || NEW.id, 'scope:project-manager:' || NEW.id,
                       'project-manager-v1', NEW.registered_at);
             END;
             UPDATE sqlite_sequence SET seq = MAX(seq, {sequence_high_water})
               WHERE name = 'projects';
             DELETE FROM schema_migrations WHERE version = 23;
             COMMIT;"
        ))
        .expect("downgrade populated fixture to v22 shape");
    drop(downgrade);
    drop(store);

    let upgraded = Store::open(&data_dir).expect("upgrade populated v22 fixture to v23");
    let projects = upgraded.list_projects().expect("read migrated projects");
    assert_eq!(projects.len(), 3);
    for (project, expected_id) in projects.iter().zip(&project_ids) {
        assert_eq!(project.id, *expected_id);
        assert_eq!(project.kind, ProjectKind::Repository);
        assert_eq!(project.visibility, ProjectVisibility::Shared);
        assert!(project.path.is_some());
    }
    let roles = upgraded
        .logical_agent_roles()
        .expect("read preserved roles");
    for project_id in &project_ids {
        let role = roles
            .iter()
            .find(|role| role.role_key == format!("project-manager:{project_id}"))
            .expect("stable PM role preserved");
        assert_eq!(role.memory_scope_key, format!("project:{project_id}"));
    }
    assert!(upgraded
        .list_tasks(Some(first_project_id), false, 10)
        .expect("read preserved tasks")
        .iter()
        .any(|item| item.task.id == task.id));
    assert_eq!(
        upgraded
            .latest_project_checkpoint(first_project_id)
            .expect("read preserved checkpoint")
            .expect("checkpoint preserved")
            .summary_text,
        "Legacy checkpoint"
    );
    let after_migration = Connection::open(&database_path).expect("inspect upgraded database");
    let fts_count: i64 = after_migration
        .query_row(
            "SELECT COUNT(*) FROM memory_fts WHERE memory_fts MATCH 'migrationproof'",
            [],
            |row| row.get(0),
        )
        .expect("query preserved FTS index");
    assert_eq!(fts_count, 1);
    let foreign_key_violations: i64 = after_migration
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("check migrated foreign keys");
    assert_eq!(foreign_key_violations, 0);
    drop(after_migration);

    let next_project = upgraded
        .create_non_code_project("After migration")
        .expect("project trigger and sequence work after migration");
    assert!(next_project.id > sequence_high_water);
    let new_pm = upgraded
        .logical_agent_roles()
        .expect("read post-migration role")
        .into_iter()
        .find(|role| role.role_key == format!("project-manager:{}", next_project.id))
        .expect("project insert trigger was recreated");
    assert_eq!(
        new_pm.memory_scope_key,
        format!("project:{}", next_project.id)
    );

    drop(upgraded);
    fs::remove_dir_all(root).expect("remove isolated migration fixture");
}

#[test]
fn main_workspace_lookup_skips_a_pathless_project_with_the_same_name() {
    let root = scratch_root("main-workspace-path");
    let data_dir = root.join("data");
    let store = Store::open(&data_dir).expect("open isolated workspace lookup store");
    let pathless = store
        .create_non_code_project("cathulu")
        .expect("create earlier pathless project with the repository name");
    assert_eq!(pathless.kind, ProjectKind::NonCode);

    let repository_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("canonicalize repository workspace");
    let repository = store
        .register_project(&repository_path)
        .expect("register repository project with the same name");
    assert_eq!(repository.kind, ProjectKind::Repository);
    assert_eq!(repository.name.to_ascii_lowercase(), "cathulu");
    let resolved_path = store
        .cathulu_workspace_path()
        .expect("find repository path while an earlier pathless project exists");
    assert_eq!(
        resolved_path
            .canonicalize()
            .expect("canonicalize resolved workspace path"),
        repository_path
    );

    drop(store);
    fs::remove_dir_all(root).expect("remove isolated workspace lookup fixture");
}

#[test]
fn mcp_creates_private_non_code_project_and_withholds_it_from_main_context() {
    let root = scratch_root("mcp-privacy");
    let data_dir = root.join("data");
    let store = Store::open(&data_dir).expect("open isolated store");
    let main_capability = store
        .issue_main_overseer_capability()
        .expect("issue isolated Main capability");
    drop(store);

    let created = mcp_responses(
        &data_dir,
        None,
        &[
            json!({ "jsonrpc":"2.0", "id":2, "method":"tools/list", "params":{} }),
            json!({
                "jsonrpc":"2.0", "id":3, "method":"tools/call",
                "params":{
                    "name":"create_non_code_project",
                    "arguments":{"name":"Private planning fixture"}
                }
            }),
        ],
    );
    let tools = response(&created, 2)
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("unscoped local MCP tool catalog");
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "create_non_code_project"));
    let project = tool_value(response(&created, 3));
    let project_id = project["id"].as_i64().expect("created project ID");
    assert_eq!(project["path"], Value::Null);
    assert_eq!(project["kind"], "non_code");
    assert_eq!(project["visibility"], "private");

    let reconciliation = mcp_responses(
        &data_dir,
        None,
        &[json!({
            "jsonrpc":"2.0", "id":2, "method":"tools/call",
            "params":{
                "name":"reconcile_project",
                "arguments":{"project_id":project_id}
            }
        })],
    );
    let reconciliation = tool_value(response(&reconciliation, 2));
    assert_eq!(reconciliation["status"], "not_applicable");
    assert_eq!(reconciliation["reason"], "project_has_no_repository");

    let store = Store::open(&data_dir).expect("reopen isolated store");
    let objective = store
        .create_objective(project_id, "Private objective text", "Private details")
        .expect("local owner can manage private project");
    let task = store
        .create_task(
            objective.id,
            "Private task text",
            "Not available to Main context",
        )
        .expect("local owner can create private task");
    drop(store);

    let owner_view = mcp_responses(
        &data_dir,
        None,
        &[json!({
            "jsonrpc":"2.0", "id":2, "method":"resources/read",
            "params":{"uri":format!("cathulu://projects/{project_id}/tasks")}
        })],
    );
    let owner_tasks = resource_value(response(&owner_view, 2));
    assert!(owner_tasks
        .as_array()
        .expect("local project task list")
        .iter()
        .any(|item| item["task"]["id"] == task.id));

    let main_view = mcp_responses(
        &data_dir,
        Some(&main_capability.token),
        &[
            json!({
                "jsonrpc":"2.0", "id":2, "method":"resources/read",
                "params":{"uri":"cathulu://projects"}
            }),
            json!({
                "jsonrpc":"2.0", "id":3, "method":"resources/read",
                "params":{"uri":"cathulu://portfolio/tasks"}
            }),
            json!({
                "jsonrpc":"2.0", "id":4, "method":"resources/read",
                "params":{"uri":"cathulu://roles"}
            }),
            json!({
                "jsonrpc":"2.0", "id":5, "method":"resources/read",
                "params":{"uri":"cathulu://dashboard"}
            }),
            json!({
                "jsonrpc":"2.0", "id":6, "method":"resources/read",
                "params":{"uri":format!("cathulu://projects/{project_id}/tasks")}
            }),
            json!({
                "jsonrpc":"2.0", "id":7, "method":"resources/read",
                "params":{"uri":format!("cathulu://projects/{project_id}/resume")}
            }),
        ],
    );
    let projects = resource_value(response(&main_view, 2));
    assert!(projects
        .as_array()
        .expect("Main's shared project list")
        .iter()
        .all(|project| project["id"] != project_id));
    assert!(!projects.to_string().contains("Private planning fixture"));
    assert!(!projects.to_string().contains("Private objective text"));
    assert!(!projects.to_string().contains("Private task text"));
    let tasks = resource_value(response(&main_view, 3));
    assert!(!tasks.to_string().contains("Private task text"));
    let roles = resource_value(response(&main_view, 4));
    assert!(!roles
        .to_string()
        .contains(&format!("project-manager:{project_id}")));
    for id in [5, 6, 7] {
        assert!(response(&main_view, id).get("error").is_some());
    }

    drop(main_capability);
    fs::remove_dir_all(root).expect("remove isolated MCP fixture");
}
