use cathulu_core::{workflow::TaskPlanInput, Store};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    std::env::current_dir()
        .expect("repository root")
        .join("target/manual-dev/codex")
        .join(format!("m49-{label}-{}-{stamp}", std::process::id()))
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("launch fixture Git command");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_repo(path: &Path) {
    fs::create_dir_all(path).expect("create isolated repository");
    git(path, &["init", "--quiet", "-b", "main"]);
    git(path, &["config", "user.name", "Cathulu Test"]);
    git(
        path,
        &["config", "user.email", "cathulu-test@example.invalid"],
    );
    fs::write(path.join("README.md"), "M49 isolated repository fixture\n").unwrap();
    git(path, &["add", "README.md"]);
    git(path, &["commit", "--quiet", "-m", "fixture"]);
}

fn plan(title: &str) -> TaskPlanInput {
    TaskPlanInput {
        title: title.to_string(),
        description: String::new(),
        parent_task_id: None,
        priority: 0,
        due_date: None,
        effort_minutes: None,
        next_action: Some(format!("Next action for {title}")),
    }
}

fn remove_v20_for_populated_upgrade(database_path: &Path) {
    let database = Connection::open(database_path).expect("open populated v20 fixture");
    database
        .execute_batch(
            "DROP TRIGGER project_checkpoint_artifacts_sealed;
             DROP TRIGGER project_checkpoint_objectives_sealed;
             DROP TRIGGER project_checkpoint_tasks_sealed;
             DROP TRIGGER project_checkpoint_seals_no_update;
             DROP TRIGGER project_checkpoint_seals_no_delete;
             DROP TABLE project_checkpoint_seals;
             DROP TRIGGER project_checkpoint_artifacts_no_update;
             DROP TRIGGER project_checkpoint_artifacts_no_delete;
             DROP TRIGGER project_checkpoint_objectives_no_update;
             DROP TRIGGER project_checkpoint_objectives_no_delete;
             DROP TRIGGER project_checkpoint_tasks_no_update;
             DROP TRIGGER project_checkpoint_tasks_no_delete;
             DROP TRIGGER project_checkpoints_no_update;
             DROP TRIGGER project_checkpoints_no_delete;
             DROP TABLE project_checkpoint_artifacts;
             DROP TABLE project_checkpoint_objectives;
             DROP TABLE project_checkpoint_tasks;
             DROP TABLE project_checkpoints;
             DELETE FROM schema_migrations WHERE version = 20;",
        )
        .expect("simulate populated v19 database without v20 objects");
}

#[test]
fn populated_v19_upgrade_preserves_tasks_and_checkpoint_revisions_are_immutable() {
    let root = scratch_root("migration-resume");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    create_repo(&repo);
    let store = Store::open(&data_dir).expect("open isolated store");
    let project = store.register_project(&repo).expect("register project");
    let objective = store
        .create_objective(project.id, "First slice", "Persist project progress")
        .expect("create objective");
    let first = store
        .create_task_with_plan(objective.id, &plan("Prerequisite"))
        .expect("create first task");
    let dependent = store
        .create_task_with_plan(objective.id, &plan("Dependent"))
        .expect("create dependent task");
    store
        .add_task_dependency(dependent.id, first.id, dependent.revision)
        .expect("add prerequisite");
    drop(store);

    remove_v20_for_populated_upgrade(&data_dir.join("cathulu.sqlite3"));
    let store = Store::open(&data_dir).expect("upgrade populated v19 to v20");
    assert_eq!(
        Connection::open(data_dir.join("cathulu.sqlite3"))
            .unwrap()
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        20
    );
    assert_eq!(
        store.get_task(first.id).unwrap().unwrap().title,
        "Prerequisite"
    );
    assert_eq!(store.get_task(dependent.id).unwrap().unwrap().revision, 2);

    let before = store
        .resume_project(project.id, 10)
        .expect("resume before checkpoint");
    assert!(before.checkpoint.is_none());
    assert!(before.checkpoint_age_seconds.is_none());
    assert!(before.next_eligible.iter().any(|task| task.id == first.id));
    assert!(before.blocked.iter().any(|task| {
        task.id == dependent.id
            && task
                .reasons
                .iter()
                .any(|reason| reason.code == "prerequisite_not_completed")
    }));
    let checkpoint = store
        .create_project_checkpoint(
            project.id,
            before.source_event_id,
            "First accepted snapshot",
            &[],
        )
        .expect("create first checkpoint");
    assert_eq!(checkpoint.revision, 1);
    assert_eq!(checkpoint.source_event_id, before.source_event_id);
    assert_eq!(checkpoint.project_revision, 1);
    assert_eq!(
        store
            .latest_project_checkpoint(project.id)
            .unwrap()
            .unwrap()
            .id,
        checkpoint.id
    );
    assert!(store
        .create_project_checkpoint(
            project.id,
            before.source_event_id,
            "Stale second snapshot",
            &[]
        )
        .unwrap_err()
        .contains("stale source event revision"));
    let database = Connection::open(data_dir.join("cathulu.sqlite3")).unwrap();
    assert!(database
        .execute(
            "UPDATE project_checkpoints SET summary_text = 'rewritten' WHERE id = ?1",
            [checkpoint.id]
        )
        .is_err());
    assert!(database
        .execute(
            "DELETE FROM project_checkpoints WHERE id = ?1",
            [checkpoint.id]
        )
        .is_err());
    assert!(database
        .execute(
            "UPDATE project_checkpoint_tasks SET status = 'completed' WHERE checkpoint_id = ?1",
            [checkpoint.id]
        )
        .is_err());
    assert!(database
        .execute(
            "INSERT INTO project_checkpoint_tasks(checkpoint_id, task_id, objective_id, revision, status, title)
             VALUES (?1, 999999, 1, 1, 'queued', 'late append')",
            [checkpoint.id],
        )
        .is_err());
    drop(database);

    store
        .complete_task_administratively(first.id, first.revision, "Ordinary prerequisite finished")
        .expect("complete first task");
    let added = store
        .create_task_with_plan(objective.id, &plan("Added later"))
        .expect("add task after checkpoint");
    store
        .record_decision(
            project.id,
            Some(objective.id),
            "Accepted choice",
            "Use the first slice",
            None,
        )
        .expect("record accepted decision");
    let changed = store
        .resume_project(project.id, 10)
        .expect("read current state");
    assert!(changed.checkpoint_age_seconds.is_some_and(|age| age >= 0));
    assert!(changed
        .next_eligible
        .iter()
        .any(|task| task.id == dependent.id));
    assert!(changed.next_eligible.iter().any(|task| task.id == added.id));
    assert!(!changed.remaining.iter().any(|task| task.id == first.id));
    assert!(changed.delta_since_checkpoint.iter().any(|delta| {
        delta.task_id == first.id
            && delta.from_status.as_deref() == Some("queued")
            && delta.to_status.as_deref() == Some("completed")
            && delta.event_id.is_some()
    }));
    assert!(changed
        .delta_since_checkpoint
        .iter()
        .any(|delta| { delta.task_id == added.id && delta.change == "added" }));
    assert_eq!(changed.repository.freshness, "same");
    assert_eq!(changed.recent_decisions[0].title, "Accepted choice");

    fs::write(repo.join("CHANGE.md"), "New committed source\n").unwrap();
    git(&repo, &["add", "CHANGE.md"]);
    git(&repo, &["commit", "--quiet", "-m", "change"]);
    store
        .reconcile_project(project.id)
        .expect("explicit Git refresh");
    let refreshed = store
        .resume_project(project.id, 10)
        .expect("resume after refresh");
    assert_eq!(refreshed.repository.freshness, "changed_since_checkpoint");
    assert!(refreshed.repository.observed_at.is_some());
    let missing_repo = root.join("repo-unavailable");
    assert!(repo.starts_with(&root) && missing_repo.starts_with(&root));
    fs::rename(&repo, &missing_repo).expect("make fixture repository unavailable");
    let unavailable = store
        .resume_project(project.id, 10)
        .expect("resume with missing repository");
    assert_eq!(unavailable.repository.availability, "unavailable");
    assert_eq!(unavailable.repository.freshness, "unavailable");
    assert_eq!(
        unavailable.repository.stored_head,
        refreshed.repository.stored_head
    );
    assert!(unavailable
        .next_eligible
        .iter()
        .any(|task| task.id == dependent.id));
    let second = store
        .create_project_checkpoint(
            project.id,
            unavailable.source_event_id,
            "Second historical snapshot",
            &[],
        )
        .expect("create checkpoint without implicit Git refresh");
    assert_eq!(second.revision, 2);
    assert_eq!(
        store
            .get_project_checkpoint(checkpoint.id)
            .unwrap()
            .unwrap(),
        checkpoint
    );
    drop(store);
    let reopened = Store::open(&data_dir).expect("restart v20 store");
    assert_eq!(
        reopened
            .latest_project_checkpoint(project.id)
            .unwrap()
            .unwrap()
            .id,
        second.id
    );
    assert_eq!(
        reopened
            .resume_project(project.id, 10)
            .unwrap()
            .repository
            .availability,
        "unavailable"
    );
    drop(reopened);
    fs::remove_dir_all(root).expect("remove isolated fixture");
}

#[test]
fn resume_bounds_every_list_and_reports_qa_and_scoped_artifacts() {
    let root = scratch_root("bounds-evidence");
    let repo = root.join("repo-one");
    let other_repo = root.join("repo-two");
    let data_dir = root.join("data");
    create_repo(&repo);
    create_repo(&other_repo);
    let store = Store::open(&data_dir).unwrap();
    let project = store.register_project(&repo).unwrap();
    let other = store.register_project(&other_repo).unwrap();
    let objective = store
        .create_objective(project.id, "Bounded resume", "Fixture")
        .unwrap();
    for index in 0..28 {
        let title = format!("Task {index:02} {}", "x".repeat(180));
        let task = store
            .create_task_with_plan(objective.id, &plan(&title))
            .unwrap();
        if index % 3 == 0 {
            store
                .add_task_blocker(task.id, task.revision, "waiting", &"b".repeat(900))
                .unwrap();
        }
    }
    let first_workflow = store
        .create_workflow_version(
            "m49-bounded-qa",
            "M49 bounded QA",
            1,
            &json!({"mandatory_checks": ["build"]}),
        )
        .unwrap();
    let database = Connection::open(data_dir.join("cathulu.sqlite3")).unwrap();
    database
        .execute(
            "INSERT INTO runs (project_id, objective_id, workflow_version_id, status, summary, started_at, finished_at)
             VALUES (?1, ?2, ?3, 'blocked', 'mandatory build failed', 100, 101)",
            rusqlite::params![project.id, objective.id, first_workflow.id],
        )
        .unwrap();
    let run_id = database.last_insert_rowid();
    database
        .execute(
            "INSERT INTO quality_checks (run_id, check_key, mandatory, status, details_json, updated_at)
             VALUES (?1, 'build', 1, 'failed', '{}', 101)",
            [run_id],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO evidence_artifacts (run_id, artifact_kind, uri, content_hash, created_at)
             VALUES (?1, 'test-log', 'fixture://m49/build-failure', 'hash-m49', 101)",
            [run_id],
        )
        .unwrap();
    let valid_artifact_id = database.last_insert_rowid();
    database
        .execute(
            "INSERT INTO runs (project_id, status, started_at, finished_at)
             VALUES (?1, 'inconclusive', 100, 101)",
            [other.id],
        )
        .unwrap();
    let other_run_id = database.last_insert_rowid();
    database
        .execute(
            "INSERT INTO evidence_artifacts (run_id, artifact_kind, uri, created_at)
             VALUES (?1, 'test-log', 'fixture://m49/other-project', 101)",
            [other_run_id],
        )
        .unwrap();
    let wrong_artifact_id = database.last_insert_rowid();
    drop(database);
    store
        .create_workflow_version(
            "m49-bounded-qa",
            "M49 bounded QA",
            2,
            &json!({"mandatory_checks": ["build", "security"]}),
        )
        .unwrap();
    let packet = store.resume_project(project.id, 10).unwrap();
    assert_eq!(packet.limits.remaining_total, 28);
    assert_eq!(packet.remaining.len(), 10);
    assert_eq!(packet.limits.remaining_omitted, 18);
    assert_eq!(packet.limits.blocked_total, 10);
    assert_eq!(packet.limits.next_eligible_total, 18);
    assert!(packet.blocked.iter().all(|task| task.reasons.len() <= 3));
    assert!(packet
        .remaining
        .iter()
        .all(|task| task.due_date.is_none() && task.effort_minutes.is_none()));
    assert!(serde_json::to_vec(&packet).unwrap().len() <= 32_000);
    let verification = packet.verification.as_ref().unwrap();
    assert_eq!(verification.run_id, run_id);
    assert_eq!(verification.status, "blocked");
    assert_eq!(verification.non_passing_mandatory_checks, 1);
    assert_eq!(verification.mandatory_checks_changed_since, Some(true));
    assert!(store.resume_project(project.id, 11).is_err());
    assert!(store.resume_project(-999, 10).is_err());
    assert!(store
        .create_project_checkpoint(
            project.id,
            packet.source_event_id,
            "Bad artifact",
            &[wrong_artifact_id]
        )
        .unwrap_err()
        .contains("not linked to a run in project"));
    assert!(store
        .latest_project_checkpoint(project.id)
        .unwrap()
        .is_none());
    let checkpoint = store
        .create_project_checkpoint(
            project.id,
            packet.source_event_id,
            "Evidence attached to this immutable revision",
            &[valid_artifact_id],
        )
        .unwrap();
    assert_eq!(checkpoint.artifacts[0].artifact_id, valid_artifact_id);
    assert_eq!(
        checkpoint.artifacts[0].content_hash.as_deref(),
        Some("hash-m49")
    );
    assert!(store
        .resume_project(other.id, 10)
        .unwrap()
        .remaining
        .is_empty());
    drop(store);
    fs::remove_dir_all(root).expect("remove isolated fixture");
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

fn initialized_messages(requests: &[Value]) -> Vec<Value> {
    let mut messages = vec![
        request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "cathulu-m49-test", "version": "0.1.0" }
            }),
        ),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ];
    messages.extend_from_slice(requests);
    messages
}

fn run_no_provider_server(data_dir: &Path, env_file: &Path, messages: &[Value]) -> Vec<Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cathulu-mcp"));
    command
        .env("CATHULU_DATA_DIR", data_dir)
        .env("CATHULU_ROLE_DISPATCHER_ENABLED", "0")
        .env("CATHULU_ENV_FILE", env_file)
        .env_remove("CATHULU_DECISION_PROVIDER_API_KEY")
        .env_remove("CATHULU_CONTEXT_PROVIDER_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cathulu_core::suppress_windows_console(&mut command);
    let mut child = command.spawn().expect("start isolated MCP server");
    {
        let stdin = child.stdin.as_mut().expect("MCP stdin");
        for message in messages {
            serde_json::to_writer(&mut *stdin, message).expect("write JSON-RPC request");
            stdin.write_all(b"\n").expect("end JSON-RPC line");
        }
    }
    let output = child.wait_with_output().expect("wait for MCP server");
    assert!(
        output.status.success(),
        "MCP server failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("MCP stdout is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("MCP emits JSON-RPC lines"))
        .collect()
}

fn response(responses: &[Value], id: u64) -> &Value {
    responses
        .iter()
        .find(|value| value.get("id") == Some(&json!(id)))
        .unwrap_or_else(|| panic!("missing MCP response {id}: {responses:?}"))
}

fn tool_value(response: &Value) -> Value {
    assert_ne!(
        response.pointer("/result/isError"),
        Some(&json!(true)),
        "{response}"
    );
    let text = response
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .expect("tool result text");
    serde_json::from_str(text).expect("tool result is JSON")
}

fn resource_value(response: &Value) -> Value {
    let text = response
        .pointer("/result/contents/0/text")
        .and_then(Value::as_str)
        .expect("resource result text");
    serde_json::from_str(text).expect("resource result is JSON")
}

#[test]
fn public_mcp_checkpoint_resume_survives_restart_without_providers() {
    let root = scratch_root("mcp-no-provider");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    let env_file = root.join("absent-provider-credentials.env");
    create_repo(&repo);
    assert!(!env_file.exists());
    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(2, "register_project", json!({ "path": repo }))]),
    );
    let project_id = tool_value(response(&responses, 2))["id"].as_i64().unwrap();
    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "create_objective",
            json!({ "project_id": project_id, "title": "M49 public flow" }),
        )]),
    );
    let objective_id = tool_value(response(&responses, 2))["id"].as_i64().unwrap();
    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "create_task",
            json!({ "objective_id": objective_id, "title": "Finish first slice" }),
        )]),
    );
    let task_id = tool_value(response(&responses, 2))["id"].as_i64().unwrap();
    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "resume_project",
            json!({ "project_id": project_id }),
        )]),
    );
    let before = tool_value(response(&responses, 2));
    assert!(before["checkpoint"].is_null());
    assert_eq!(before["nextEligible"][0]["id"], json!(task_id));
    let source_event_id = before["sourceEventId"].as_i64().unwrap();
    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "create_project_checkpoint",
            json!({
                "project_id": project_id,
                "expected_source_event_id": source_event_id,
                "summary_text": "First public checkpoint"
            }),
        )]),
    );
    let checkpoint = tool_value(response(&responses, 2));
    assert_eq!(checkpoint["revision"], json!(1));
    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "create_project_checkpoint",
            json!({
                "project_id": project_id,
                "expected_source_event_id": source_event_id,
                "summary_text": "Stale checkpoint"
            }),
        )]),
    );
    assert_eq!(
        response(&responses, 2).pointer("/result/isError"),
        Some(&json!(true))
    );
    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[
            tool_call(
                2,
                "complete_task_administratively",
                json!({
                    "task_id": task_id,
                    "expected_revision": 1,
                    "reason": "Ordinary task done"
                }),
            ),
            tool_call(
                3,
                "create_task",
                json!({ "objective_id": objective_id, "title": "Continue with next task" }),
            ),
        ]),
    );
    let completed = tool_value(response(&responses, 2));
    let next = tool_value(response(&responses, 3));
    assert_eq!(completed["completionKind"], json!("administrative"));
    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[
            request(
                2,
                "resources/read",
                json!({ "uri": format!("cathulu://projects/{project_id}/resume") }),
            ),
            request(
                3,
                "resources/read",
                json!({ "uri": format!("cathulu://projects/{project_id}/checkpoints/latest") }),
            ),
            tool_call(4, "resume_project", json!({ "project_id": project_id })),
        ]),
    );
    let resumed = resource_value(response(&responses, 2));
    let latest = resource_value(response(&responses, 3));
    let tool_resume = tool_value(response(&responses, 4));
    assert_eq!(latest["id"], checkpoint["id"]);
    assert_eq!(resumed["checkpoint"]["revision"], json!(1));
    assert_eq!(resumed["nextEligible"][0]["id"], next["id"]);
    assert_eq!(resumed["deltaSinceCheckpoint"].as_array().unwrap().len(), 2);
    assert_eq!(resumed["sourceEventId"], tool_resume["sourceEventId"]);
    assert_eq!(resumed["repository"]["availability"], json!("available"));
    drop(responses);
    fs::remove_dir_all(root).expect("remove isolated fixture");
}
