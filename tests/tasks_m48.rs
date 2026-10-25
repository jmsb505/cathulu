use cathulu_core::{
    dispatcher::{role_allowed_tools, RoleCapabilityScope},
    memory::{FreshnessBasis, MemoryCreateInput, MemorySource},
    workflow::TaskPlanInput,
    Store,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::{SystemTime, UNIX_EPOCH};

fn scratch_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    std::env::current_dir()
        .expect("repository root")
        .join("target/manual-dev/codex")
        .join(format!("{label}-{}-{stamp}", std::process::id()))
}

fn create_git_repo(path: &Path) {
    fs::create_dir_all(path).expect("create fixture repository");
    git(path, &["init", "--quiet", "-b", "main"]);
    git(path, &["config", "user.name", "Cathulu Test"]);
    git(
        path,
        &["config", "user.email", "cathulu-test@example.invalid"],
    );
    fs::write(path.join("README.md"), "M48 isolated repository fixture\n")
        .expect("write repository fixture");
    git(path, &["add", "README.md"]);
    git(path, &["commit", "--quiet", "-m", "fixture"]);
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .expect("launch git fixture command");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn register_fixture_project(store: &Store, root: &Path, name: &str) -> i64 {
    let repo = root.join(name);
    create_git_repo(&repo);
    store
        .register_project(&repo)
        .expect("register isolated fixture project")
        .id
}

fn input(title: &str, priority: i64, due_date: Option<&str>) -> TaskPlanInput {
    TaskPlanInput {
        title: title.to_string(),
        description: String::new(),
        parent_task_id: None,
        priority,
        due_date: due_date.map(str::to_string),
        effort_minutes: None,
        next_action: None,
    }
}

fn create_task(
    store: &Store,
    objective_id: i64,
    plan: &TaskPlanInput,
) -> cathulu_core::workflow::Task {
    store
        .create_task_with_plan(objective_id, plan)
        .expect("create planning task")
}

#[test]
fn populated_v18_migration_preserves_ids_and_marks_legacy_unknowns() {
    let root = scratch_root("migrate-v18");
    let data_dir = root.join("data");
    fs::create_dir_all(&data_dir).expect("create isolated data directory");
    let database_path = data_dir.join("cathulu.sqlite3");
    let legacy = Connection::open(&database_path).expect("create v18 fixture database");
    legacy
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
             INSERT INTO schema_migrations(version, applied_at) VALUES (18, 100);
             CREATE TABLE projects (
               id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, path TEXT NOT NULL UNIQUE,
               git_branch TEXT, git_head TEXT, git_dirty INTEGER NOT NULL DEFAULT 0,
               changed_paths_json TEXT NOT NULL DEFAULT '[]', registered_at INTEGER NOT NULL,
               reconciled_at INTEGER NOT NULL
             );
             CREATE TABLE objectives (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
               title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
               status TEXT NOT NULL DEFAULT 'active', created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
             );
             CREATE TABLE tasks (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               objective_id INTEGER NOT NULL REFERENCES objectives(id) ON DELETE CASCADE,
               title TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', status TEXT NOT NULL,
               created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
             );
             CREATE TABLE runs (
               id INTEGER PRIMARY KEY, project_id INTEGER NOT NULL, task_id INTEGER,
               status TEXT NOT NULL
             );
             CREATE TABLE logical_agent_roles (role_key TEXT PRIMARY KEY);
             CREATE TABLE delegations (
               id INTEGER PRIMARY KEY, project_id INTEGER, role TEXT, parent_delegation_id INTEGER
             );
             CREATE TABLE role_thread_bindings (
               binding_key TEXT PRIMARY KEY, role_key TEXT, delegation_id INTEGER, project_id INTEGER,
               thread_id TEXT, status TEXT, last_turn_id TEXT, created_at INTEGER, updated_at INTEGER
             );
             CREATE TABLE role_dispatch_outbox (id INTEGER PRIMARY KEY, delegation_id INTEGER);
             CREATE TABLE role_turn_inbox (
               id INTEGER PRIMARY KEY, target_binding_key TEXT, target_delegation_id INTEGER, project_id INTEGER
             );
             CREATE TABLE role_thread_capabilities (binding_key TEXT PRIMARY KEY, delegation_id INTEGER);
             CREATE TABLE main_overseer_capability (singleton INTEGER PRIMARY KEY);
             CREATE TABLE memories (id INTEGER PRIMARY KEY);
             INSERT INTO projects(id, name, path, registered_at, reconciled_at)
               VALUES (41, 'legacy project', 'C:/fixture/legacy', 10, 20);
             INSERT INTO objectives(id, project_id, title, created_at, updated_at)
               VALUES (51, 41, 'legacy objective', 11, 21);
             INSERT INTO tasks(id, objective_id, title, status, created_at, updated_at) VALUES
               (61, 51, 'queued legacy', 'queued', 12, 22),
               (62, 51, 'blocked legacy', 'blocked', 13, 23),
               (63, 51, 'completed legacy', 'completed', 14, 24),
               (64, 51, 'failed legacy', 'failed', 15, 25),
               (65, 51, 'cancelled legacy', 'cancelled', 16, 26);",
        )
        .expect("create populated v18 fixture");
    drop(legacy);

    let store = Store::open(&data_dir).expect("upgrade populated v18 database");
    let queued = store
        .get_task(61)
        .expect("read queued legacy task")
        .unwrap();
    let blocked = store
        .get_task(62)
        .expect("read blocked legacy task")
        .unwrap();
    let completed = store
        .get_task(63)
        .expect("read completed legacy task")
        .unwrap();
    let failed = store
        .get_task(64)
        .expect("read failed legacy task")
        .unwrap();
    let cancelled = store
        .get_task(65)
        .expect("read cancelled legacy task")
        .unwrap();
    assert_eq!(queued.status, "queued");
    assert_eq!(queued.revision, 1);
    assert_eq!(queued.priority, 0);
    assert_eq!(queued.due_date, None);
    assert_eq!(blocked.status, "blocked");
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.completed_at, Some(24));
    assert_eq!(completed.completion_kind.as_deref(), Some("legacy_unknown"));
    assert_eq!(completed.completion_run_id, None);
    assert_eq!(failed.status, "failed");
    assert_eq!(
        cancelled.cancellation_reason.as_deref(),
        Some("legacy_unknown")
    );
    assert_eq!(cancelled.cancelled_at, Some(26));
    let legacy_blocker = store
        .list_tasks(Some(41), false, 100)
        .expect("read migrated blocker reasons")
        .into_iter()
        .find(|item| item.task.id == 62)
        .unwrap();
    assert!(legacy_blocker
        .blocking_reasons
        .iter()
        .any(|reason| reason.code == "manual_legacy_unknown"));
    let database = Connection::open(database_path).expect("read upgraded database");
    let version: i64 = database
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .expect("read upgraded migration version");
    assert_eq!(version, 22);
}

#[test]
fn v1_project_upgrades_through_m19_with_deterministic_defaults() {
    let root = scratch_root("migrate-v1");
    let data_dir = root.join("data");
    fs::create_dir_all(&data_dir).expect("create isolated data directory");
    let database_path = data_dir.join("cathulu.sqlite3");
    let legacy = Connection::open(&database_path).expect("create v1 fixture database");
    legacy
        .execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
             INSERT INTO schema_migrations(version, applied_at) VALUES (1, 100);
             CREATE TABLE projects (
               id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, path TEXT NOT NULL UNIQUE,
               git_branch TEXT, git_head TEXT, git_dirty INTEGER NOT NULL DEFAULT 0,
               changed_paths_json TEXT NOT NULL DEFAULT '[]', registered_at INTEGER NOT NULL,
               reconciled_at INTEGER NOT NULL
             );
             INSERT INTO projects(id, name, path, registered_at, reconciled_at)
               VALUES (7, 'v1 project', 'C:/fixture/v1', 10, 20);",
        )
        .expect("create v1 fixture");
    drop(legacy);

    let store = Store::open(&data_dir).expect("upgrade v1 database through v19");
    assert_eq!(
        store
            .get_project(7)
            .expect("read migrated project")
            .unwrap()
            .id,
        7
    );
    assert_eq!(
        store
            .get_project_planning(7)
            .expect("read project planning defaults")
            .unwrap(),
        cathulu_core::workflow::ProjectPlanning {
            project_id: 7,
            status: "active".to_string(),
            priority: 0,
            revision: 1,
        }
    );
}

#[test]
fn task_run_completion_requires_finished_verified_success_and_task_revision() {
    let root = scratch_root("run-completion");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    let objective = store
        .create_objective(project_id, "Run-linked task", "must pass QA")
        .expect("create objective");
    let task = create_task(&store, objective.id, &input("Run-linked task", 0, None));
    let summary = store
        .create_memory(&MemoryCreateInput {
            scope_type: "project".to_string(),
            scope_id: Some(project_id.to_string()),
            memory_class: "canonical".to_string(),
            temperature: "hot".to_string(),
            kind: "summary".to_string(),
            body: "This isolated repository fixture verifies task linked QA completion."
                .to_string(),
            sources: vec![
                MemorySource {
                    source_type: "project".to_string(),
                    source_id: project_id.to_string(),
                },
                MemorySource {
                    source_type: "source_file".to_string(),
                    source_id: "README.md".to_string(),
                },
            ],
            artifact_ids: Vec::new(),
            valid_from: None,
            valid_until: None,
            supersedes_id: None,
            freshness_basis: FreshnessBasis::default(),
            importance: 0.9,
            confidence: 0.95,
            active: true,
            protected: false,
        })
        .expect("create source-backed fixture summary");
    store
        .review_memory_candidate(summary.id, "accepted")
        .expect("accept fixture project summary");
    let blocked_run = store
        .start_run(project_id, Some(objective.id), Some(task.id), None)
        .expect("start task run without required checks");
    store
        .record_project_memory_review(
            blocked_run.id,
            "no_durable_change",
            "task_produced_no_reusable_knowledge",
            &[],
        )
        .expect("record memory review for blocked run");
    let blocked = store
        .finish_run(blocked_run.id, "no required checks were declared")
        .expect("finish blocked run");
    assert_eq!(blocked.status, "blocked");
    let workflow = store
        .create_workflow_version(
            "m48_verified_task",
            "M48 verified task",
            1,
            &json!({ "mandatory_checks": ["unit"] }),
        )
        .expect("create QA workflow");
    let run = store
        .start_run(
            project_id,
            Some(objective.id),
            Some(task.id),
            Some(workflow.id),
        )
        .expect("start task run");
    let active_task = store.get_task(task.id).unwrap().unwrap();
    assert_eq!(active_task.status, "active");
    assert_eq!(active_task.revision, task.revision + 1);
    assert!(store
        .accept_task_run(task.id, run.id, active_task.revision)
        .unwrap_err()
        .contains("verified-success"));
    store
        .record_quality_check(run.id, "unit", true, "passed", &json!({ "count": 1 }))
        .expect("record passing required check");
    store
        .record_project_memory_review(
            run.id,
            "no_durable_change",
            "task_produced_no_reusable_knowledge",
            &[],
        )
        .expect("record required project memory review");
    let finished = store
        .finish_run(run.id, "required check passed")
        .expect("finish verified run");
    assert_eq!(finished.status, "verified_success");
    let accepted = store
        .accept_task_run(task.id, run.id, active_task.revision)
        .expect("accept verified run as task completion");
    assert_eq!(accepted.status, "completed");
    assert_eq!(accepted.completion_kind.as_deref(), Some("verified_run"));
    assert_eq!(accepted.completion_run_id, Some(run.id));
    assert!(store
        .transition_task_status(task.id, accepted.revision, "queued")
        .unwrap_err()
        .contains("cannot transition"));
}

#[test]
fn task_portfolio_enforces_revisions_scope_blockers_order_and_completion_guards() {
    let root = scratch_root("rules");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "primary");
    let objective = store
        .create_objective(project_id, "M48", "task planning rules")
        .expect("create fixture objective");
    let project = store
        .get_project_planning(project_id)
        .expect("read project metadata")
        .unwrap();
    let updated_project = store
        .update_project_planning(project_id, project.revision, "active", 4)
        .expect("update explicit project priority");
    assert_eq!(updated_project.priority, 4);

    let prerequisite = create_task(&store, objective.id, &input("Prerequisite", 0, None));
    let dependent = create_task(
        &store,
        objective.id,
        &input("Dependent", 4, Some("2026-10-26")),
    );
    store
        .add_task_dependency(dependent.id, prerequisite.id, dependent.revision)
        .expect("add prerequisite edge");
    let dependent = store.get_task(dependent.id).unwrap().unwrap();
    let blocked = store
        .list_tasks(Some(project_id), false, 100)
        .expect("query task portfolio")
        .into_iter()
        .find(|item| item.task.id == dependent.id)
        .unwrap();
    assert!(!blocked.eligible);
    assert!(blocked.blocking_reasons.iter().any(|reason| {
        reason.code == "prerequisite_not_completed"
            && reason.related_task_id == Some(prerequisite.id)
    }));
    assert!(store
        .list_tasks(Some(project_id), true, 100)
        .unwrap()
        .iter()
        .all(|item| item.task.id != dependent.id));

    let unrelated = create_task(&store, objective.id, &input("Unrelated", 4, None));
    let tie_first = create_task(
        &store,
        objective.id,
        &input("Tie first", 2, Some("2026-11-01")),
    );
    let tie_second = create_task(
        &store,
        objective.id,
        &input("Tie second", 2, Some("2026-11-01")),
    );
    assert!(store
        .add_task_dependency(prerequisite.id, dependent.id, prerequisite.revision)
        .unwrap_err()
        .contains("cycle"));
    let stale_input = TaskPlanInput {
        title: "Updated dependent".into(),
        description: "edited".into(),
        parent_task_id: None,
        priority: 5,
        due_date: Some("2026-10-25".into()),
        effort_minutes: Some(45),
        next_action: Some("Run checks".into()),
    };
    let updated = store
        .update_task_plan(dependent.id, dependent.revision, &stale_input)
        .expect("revisioned task update");
    assert_eq!(updated.revision, dependent.revision + 1);
    assert_eq!(updated.effort_minutes, Some(45));
    assert!(store
        .update_task_plan(dependent.id, dependent.revision, &stale_input)
        .unwrap_err()
        .contains("stale task revision"));
    assert!(store
        .update_task_plan(
            unrelated.id,
            unrelated.revision,
            &TaskPlanInput {
                due_date: Some("2026-02-30".into()),
                ..input("Invalid date", 0, None)
            }
        )
        .unwrap_err()
        .contains("calendar date"));
    let sorted = store.list_tasks(Some(project_id), false, 100).unwrap();
    assert_eq!(
        sorted
            .iter()
            .take(5)
            .map(|item| item.task.id)
            .collect::<Vec<_>>(),
        vec![
            dependent.id,
            unrelated.id,
            tie_first.id,
            tie_second.id,
            prerequisite.id
        ]
    );
    assert_eq!(sorted[0].task.due_date.as_deref(), Some("2026-10-25"));
    assert_eq!(sorted[1].task.due_date, None);
    assert_eq!(sorted[1].task.effort_minutes, None);
    let tie_positions = sorted
        .iter()
        .enumerate()
        .filter(|(_, item)| item.task.id == tie_first.id || item.task.id == tie_second.id)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(tie_positions.len(), 2);
    assert_eq!(tie_positions[1], tie_positions[0] + 1);
    assert!(tie_first.id < tie_second.id);
    assert!(sorted[tie_positions[0]].task.id < sorted[tie_positions[1]].task.id);

    let blocker = store
        .add_task_blocker(
            updated.id,
            updated.revision,
            "decision",
            "await Owner decision",
        )
        .expect("add structured blocker");
    assert_eq!(blocker.status, "open");
    let blocked_again = store.get_task(updated.id).unwrap().unwrap();
    let item = store
        .list_tasks(Some(project_id), false, 100)
        .unwrap()
        .into_iter()
        .find(|item| item.task.id == updated.id)
        .unwrap();
    assert!(item
        .blocking_reasons
        .iter()
        .any(|reason| reason.code == "manual_decision"));
    let resolved = store
        .resolve_task_blocker(blocker.id, blocked_again.revision, "decision recorded")
        .expect("resolve blocker with audit text");
    assert_eq!(resolved.status, "resolved");
    let dependent_after_resolution = store.get_task(updated.id).unwrap().unwrap();
    let still_blocked = store
        .list_tasks(Some(project_id), false, 100)
        .unwrap()
        .into_iter()
        .find(|item| item.task.id == updated.id)
        .unwrap();
    assert!(
        !still_blocked.eligible,
        "dependency remains after manual unblock"
    );
    let completed_prerequisite = store
        .complete_task_administratively(prerequisite.id, prerequisite.revision, "prerequisite done")
        .expect("complete prerequisite administratively");
    assert_eq!(completed_prerequisite.status, "completed");
    let now_eligible = store
        .list_tasks(Some(project_id), true, 100)
        .unwrap()
        .into_iter()
        .find(|item| item.task.id == updated.id)
        .expect("completed prerequisite unblocks dependent task");
    assert!(now_eligible.eligible);

    let foreign_project = register_fixture_project(&store, &root.join("repos"), "foreign");
    let foreign_objective = store
        .create_objective(foreign_project, "Foreign project", "scope boundary")
        .expect("create foreign project objective");
    let foreign_task = create_task(
        &store,
        foreign_objective.id,
        &input("Foreign task", 0, None),
    );
    assert!(store
        .add_task_dependency(
            dependent_after_resolution.id,
            foreign_task.id,
            dependent_after_resolution.revision,
        )
        .unwrap_err()
        .contains("same project"));
    assert!(store
        .update_task_plan(
            dependent_after_resolution.id,
            dependent_after_resolution.revision,
            &TaskPlanInput {
                parent_task_id: Some(foreign_task.id),
                ..input("Updated dependent", 5, Some("2026-10-25"))
            }
        )
        .unwrap_err()
        .contains("same project"));
    assert_eq!(
        store
            .get_task(dependent_after_resolution.id)
            .unwrap()
            .unwrap()
            .revision,
        dependent_after_resolution.revision,
        "rejected cross-project writes must leave the task unchanged"
    );

    let failed_prerequisite =
        create_task(&store, objective.id, &input("Failed prerequisite", 0, None));
    let failed_dependent = create_task(&store, objective.id, &input("Failed dependent", 1, None));
    store
        .add_task_dependency(
            failed_dependent.id,
            failed_prerequisite.id,
            failed_dependent.revision,
        )
        .expect("add failed prerequisite edge");
    store
        .transition_task_status(
            failed_prerequisite.id,
            failed_prerequisite.revision,
            "failed",
        )
        .expect("mark prerequisite failed");
    let failed_prerequisite = store.get_task(failed_prerequisite.id).unwrap().unwrap();
    let failed_view = store
        .list_tasks(Some(project_id), false, 100)
        .unwrap()
        .into_iter()
        .find(|item| item.task.id == failed_dependent.id)
        .unwrap();
    assert!(failed_view.blocking_reasons.iter().any(|reason| {
        reason.code == "prerequisite_not_completed" && reason.message.contains("failed")
    }));
    let retried_prerequisite = store
        .reopen_task(
            failed_prerequisite.id,
            failed_prerequisite.revision,
            "retry after correcting the failed setup",
        )
        .expect("reopen failed prerequisite with reason");
    assert_eq!(retried_prerequisite.status, "queued");
    assert!(retried_prerequisite.revision > failed_prerequisite.revision);
    let retried_view = store
        .list_tasks(Some(project_id), false, 100)
        .unwrap()
        .into_iter()
        .find(|item| item.task.id == failed_dependent.id)
        .unwrap();
    assert!(retried_view.blocking_reasons.iter().any(|reason| {
        reason.code == "prerequisite_not_completed" && reason.message.contains("queued")
    }));

    let parent = create_task(&store, objective.id, &input("Parent", 0, None));
    let child = create_task(&store, objective.id, &input("Child", 0, None));
    let child_plan = TaskPlanInput {
        parent_task_id: Some(parent.id),
        ..input("Child", 0, None)
    };
    let child = store
        .update_task_plan(child.id, child.revision, &child_plan)
        .expect("assign same-project parent");
    assert!(store
        .complete_task_administratively(parent.id, parent.revision, "finish parent")
        .unwrap_err()
        .contains("child task"));
    assert!(store
        .update_task_plan(
            parent.id,
            parent.revision,
            &TaskPlanInput {
                parent_task_id: Some(child.id),
                ..input("Parent", 0, None)
            }
        )
        .unwrap_err()
        .contains("cycle"));
    store
        .transition_task_status(child.id, child.revision, "cancelled")
        .expect_err("generic task status must require reasoned cancellation");
    let child = store.get_task(child.id).unwrap().unwrap();
    assert!(store
        .cancel_task(child.id, child.revision, "legacy_unknown")
        .unwrap_err()
        .contains("reserved"));
    let cancelled_child = store
        .cancel_task(
            child.id,
            child.revision,
            "waive this child because the work is no longer required",
        )
        .expect("cancel leaf child with explicit waiver reason");
    assert_eq!(cancelled_child.status, "cancelled");
    assert_eq!(
        cancelled_child.cancellation_reason.as_deref(),
        Some("waive this child because the work is no longer required")
    );
    let reopened_child = store
        .reopen_task(
            cancelled_child.id,
            cancelled_child.revision,
            "reopen after confirming the child work is needed",
        )
        .expect("reopen cancelled child with reason");
    assert_eq!(reopened_child.status, "queued");
    assert!(store
        .complete_task_administratively(parent.id, parent.revision, "finish parent")
        .unwrap_err()
        .contains("child task"));
    let cancelled_child = store
        .cancel_task(
            reopened_child.id,
            reopened_child.revision,
            "waive the child after rechecking scope",
        )
        .expect("record final child waiver");
    assert_eq!(
        cancelled_child.cancellation_reason.as_deref(),
        Some("waive the child after rechecking scope")
    );
    let parent = store.get_task(parent.id).unwrap().unwrap();
    let completed_parent = store
        .complete_task_administratively(parent.id, parent.revision, "ordinary todo finished")
        .expect("complete parent after child is terminal");
    assert_eq!(
        completed_parent.completion_kind.as_deref(),
        Some("administrative")
    );
    assert!(store
        .set_task_status(unrelated.id, "completed")
        .unwrap_err()
        .contains("completion commands"));

    let all_tasks = store.list_tasks(Some(project_id), false, 100).unwrap();
    let eligible_tasks = store.list_tasks(Some(project_id), true, 100).unwrap();
    let expected_eligible_ids = all_tasks
        .iter()
        .filter(|item| item.eligible)
        .map(|item| item.task.id)
        .collect::<Vec<_>>();
    let actual_eligible_ids = eligible_tasks
        .iter()
        .map(|item| item.task.id)
        .collect::<Vec<_>>();
    assert_eq!(actual_eligible_ids, expected_eligible_ids);

    let project_events = Connection::open(store.database_path())
        .expect("open audit database")
        .query_row(
            "SELECT COUNT(*) FROM events WHERE aggregate_type = 'project'
             AND aggregate_id = ?1 AND event_type = 'project_priority_changed'",
            [project_id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .expect("count audited project priority edits");
    let task_priority_events = Connection::open(store.database_path())
        .expect("open audit database for task events")
        .query_row(
            "SELECT COUNT(*) FROM events WHERE aggregate_type = 'task'
             AND aggregate_id = ?1 AND event_type = 'task_priority_changed'",
            [dependent.id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .expect("count audited task priority edits");
    assert_eq!(project_events, 1);
    assert_eq!(task_priority_events, 1);

    let capability = RoleCapabilityScope {
        role_kind: "worker".to_string(),
        project_id: Some(project_id),
        delegation_id: Some(1),
        capability_hash: "0".repeat(64),
        workspace_lease_id: None,
    };
    assert!(role_allowed_tools("worker").iter().all(|tool| {
        !matches!(
            tool,
            &"update_task_plan" | &"add_task_dependency" | &"complete_task_administratively"
        )
    }));
    assert!(store
        .authorize_role_tool_call(
            &capability,
            "update_task_plan",
            &json!({ "task_id": dependent.id, "expected_revision": updated.revision })
        )
        .unwrap_err()
        .contains("cannot call"));
}

#[test]
fn concurrent_task_plan_edits_accept_one_revision_and_reject_the_stale_writer() {
    let root = scratch_root("concurrent-revision");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    let objective = store
        .create_objective(project_id, "Concurrent edit", "optimistic revision")
        .expect("create objective");
    let task = create_task(&store, objective.id, &input("Concurrent task", 0, None));

    let barrier = Arc::new(Barrier::new(3));
    let first_store = store.clone();
    let first_barrier = Arc::clone(&barrier);
    let first_task = task.clone();
    let first = std::thread::spawn(move || {
        first_barrier.wait();
        first_store.update_task_plan(
            first_task.id,
            first_task.revision,
            &TaskPlanInput {
                title: "Concurrent winner A".to_string(),
                ..input("Concurrent winner A", 1, None)
            },
        )
    });

    let second_store = store.clone();
    let second_barrier = Arc::clone(&barrier);
    let second_task = task.clone();
    let second = std::thread::spawn(move || {
        second_barrier.wait();
        second_store.update_task_plan(
            second_task.id,
            second_task.revision,
            &TaskPlanInput {
                title: "Concurrent winner B".to_string(),
                ..input("Concurrent winner B", 2, None)
            },
        )
    });

    barrier.wait();
    let first_result = first.join().expect("first concurrent edit returns");
    let second_result = second.join().expect("second concurrent edit returns");
    assert_ne!(first_result.is_ok(), second_result.is_ok());
    let stale_error = first_result
        .as_ref()
        .err()
        .or_else(|| second_result.as_ref().err())
        .expect("one writer loses the expected revision");
    assert!(stale_error.contains("stale task revision"), "{stale_error}");
    let updated = store.get_task(task.id).unwrap().unwrap();
    assert_eq!(updated.revision, task.revision + 1);
    assert!(matches!(
        updated.title.as_str(),
        "Concurrent winner A" | "Concurrent winner B"
    ));
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

fn run_no_provider_server(data_dir: &Path, env_file: &Path, messages: &[Value]) -> Vec<Value> {
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
        .expect("launch isolated MCP server with dispatch/providers disabled");
    {
        let stdin = child.stdin.as_mut().expect("MCP stdin");
        for message in messages {
            serde_json::to_writer(&mut *stdin, message).expect("write JSON-RPC message");
            stdin.write_all(b"\n").expect("terminate JSON-RPC line");
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

fn initialized_messages(requests: &[Value]) -> Vec<Value> {
    let mut messages = vec![
        request(
            1,
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "cathulu-m48-test", "version": "0.1.0" }
            }),
        ),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    ];
    messages.extend_from_slice(requests);
    messages
}

fn tool_value(response: &Value) -> Value {
    assert_ne!(
        response.pointer("/result/isError"),
        Some(&json!(true)),
        "MCP tool call failed: {response}"
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
fn public_mcp_task_flow_survives_restart_with_dispatch_and_provider_calls_disabled() {
    let root = scratch_root("mcp");
    let repo = root.join("repo");
    let data_dir = root.join("data");
    let env_file = root.join("provider-credentials-disabled.env");
    create_git_repo(&repo);
    assert!(!env_file.exists(), "provider env file must not exist");

    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(2, "register_project", json!({ "path": repo }))]),
    );
    let project = tool_value(
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap(),
    );
    let project_id = project["id"].as_i64().unwrap();

    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "create_objective",
            json!({ "project_id": project_id, "title": "M48 provider-free flow" }),
        )]),
    );
    let objective = tool_value(
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap(),
    );
    let objective_id = objective["id"].as_i64().unwrap();

    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[
            tool_call(
                2,
                "create_task",
                json!({
                    "objective_id": objective_id,
                    "title": "Prerequisite",
                    "priority": 1,
                    "effort_minutes": 30
                }),
            ),
            tool_call(
                3,
                "create_task",
                json!({
                    "objective_id": objective_id,
                    "title": "Blocked task",
                    "priority": 4,
                    "due_date": "2026-10-26"
                }),
            ),
        ]),
    );
    let first_task = tool_value(
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap(),
    );
    let second_task = tool_value(
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(3)))
            .unwrap(),
    );
    let first_id = first_task["id"].as_i64().unwrap();
    let second_id = second_task["id"].as_i64().unwrap();

    let linked = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "add_task_dependency",
            json!({
                "task_id": second_id,
                "prerequisite_task_id": first_id,
                "expected_revision": 1
            }),
        )]),
    );
    let linked_task = tool_value(
        linked
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap(),
    );
    assert_eq!(linked_task["revision"], json!(2));

    let responses = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[
            tool_call(
                2,
                "set_task_status",
                json!({ "task_id": second_id, "expected_revision": 1, "status": "active" }),
            ),
            tool_call(3, "list_tasks", json!({ "eligible_only": true })),
            request(
                4,
                "resources/read",
                json!({ "uri": format!("cathulu://projects/{project_id}/tasks") }),
            ),
        ]),
    );
    let eligible = tool_value(
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(3)))
            .unwrap(),
    );
    assert_eq!(
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap()
            .pointer("/result/isError"),
        Some(&json!(true)),
        "stale MCP status edit must be rejected"
    );
    assert_eq!(eligible.as_array().unwrap().len(), 1);
    assert_eq!(eligible[0]["task"]["id"], json!(first_id));
    let project_tasks = resource_value(
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(4)))
            .unwrap(),
    );
    let dependent = project_tasks
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["task"]["id"] == json!(second_id))
        .unwrap();
    assert_eq!(dependent["eligible"], json!(false));
    assert_eq!(dependent["task"]["status"], json!("queued"));
    assert_eq!(dependent["task"]["revision"], json!(2));
    assert!(dependent["blockingReasons"][0]["code"] == json!("prerequisite_not_completed"));

    let restarted = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "list_tasks",
            json!({ "project_id": project_id, "eligible_only": false }),
        )]),
    );
    let persisted = tool_value(
        restarted
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap(),
    );
    assert_eq!(persisted.as_array().unwrap().len(), 2);
    assert_eq!(
        persisted
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["task"]["id"] == json!(second_id))
            .unwrap()["blockingReasons"][0]["relatedTaskId"],
        json!(first_id)
    );

    let created = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "create_task",
            json!({ "objective_id": objective_id, "title": "Reasoned cancellation" }),
        )]),
    );
    let created_task = tool_value(
        created
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap(),
    );
    let cancelled = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "cancel_task",
            json!({
                "task_id": created_task["id"],
                "expected_revision": created_task["revision"],
                "reason": "MCP fixture no longer needs this task"
            }),
        )]),
    );
    let cancelled_task = tool_value(
        cancelled
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap(),
    );
    assert_eq!(cancelled_task["status"], json!("cancelled"));
    let reopened = run_no_provider_server(
        &data_dir,
        &env_file,
        &initialized_messages(&[tool_call(
            2,
            "reopen_task",
            json!({
                "task_id": cancelled_task["id"],
                "expected_revision": cancelled_task["revision"],
                "reason": "MCP fixture reopens this task to verify the command"
            }),
        )]),
    );
    let reopened_task = tool_value(
        reopened
            .iter()
            .find(|value| value.get("id") == Some(&json!(2)))
            .unwrap(),
    );
    assert_eq!(reopened_task["status"], json!("queued"));
    assert_eq!(reopened_task["cancellationReason"], Value::Null);
}

// F1 gate (CC-01-review-CX-01-final.md §3). Administrative completion is refused while a linked
// run is blocked, failed, inconclusive or cancelled, or while a verified run is linked. The
// verified-run path stays open, and reasoned cancellation remains the child waiver.

fn finish_linked_run(
    store: &Store,
    project_id: i64,
    objective_id: i64,
    task_id: i64,
    workflow_id: Option<i64>,
    check_status: Option<&str>,
) -> cathulu_core::workflow::RunRecord {
    let run = store
        .start_run(project_id, Some(objective_id), Some(task_id), workflow_id)
        .expect("start run");
    if let Some(check_status) = check_status {
        store
            .record_quality_check(run.id, "unit", true, check_status, &json!({ "count": 1 }))
            .expect("record mandatory check");
    }
    store
        .record_project_memory_review(
            run.id,
            "no_durable_change",
            "task_produced_no_reusable_knowledge",
            &[],
        )
        .expect("record memory review");
    store
        .finish_run(run.id, "F1 gate fixture")
        .expect("finish run")
}

fn accepted_project_summary(store: &Store, project_id: i64) {
    let summary = store
        .create_memory(&MemoryCreateInput {
            scope_type: "project".to_string(),
            scope_id: Some(project_id.to_string()),
            memory_class: "canonical".to_string(),
            temperature: "hot".to_string(),
            kind: "summary".to_string(),
            body: "Fixture summary that existed before the F1 gate runs.".to_string(),
            sources: vec![
                MemorySource {
                    source_type: "project".to_string(),
                    source_id: project_id.to_string(),
                },
                MemorySource {
                    source_type: "source_file".to_string(),
                    source_id: "README.md".to_string(),
                },
            ],
            artifact_ids: Vec::new(),
            valid_from: None,
            valid_until: None,
            supersedes_id: None,
            freshness_basis: FreshnessBasis::default(),
            importance: 0.9,
            confidence: 0.95,
            active: true,
            protected: false,
        })
        .expect("create project summary");
    store
        .review_memory_candidate(summary.id, "accepted")
        .expect("accept project summary");
}

fn qa_workflow(store: &Store) -> i64 {
    store
        .create_workflow_version(
            "m48_f1_gate",
            "M48 F1 gate",
            1,
            &json!({ "mandatory_checks": ["unit"] }),
        )
        .expect("create QA workflow")
        .id
}

fn assert_administrative_completion_refused(store: &Store, task_id: i64, named: &[&str]) {
    let before = store
        .get_task(task_id)
        .expect("read task")
        .expect("task exists");
    let event_count = store
        .list_events("task", &task_id.to_string())
        .expect("read task events before refusal")
        .len();
    let error = store
        .complete_task_administratively(task_id, before.revision, "ship anyway")
        .expect_err("the F1 gate must refuse administrative completion");
    for fragment in named {
        assert!(
            error.contains(fragment),
            "error should name {fragment:?}: {error}"
        );
    }
    let after = store
        .get_task(task_id)
        .expect("read task")
        .expect("task exists");
    assert_eq!(after.status, before.status);
    assert_eq!(after.revision, before.revision);
    assert!(after.completion_kind.is_none());
    assert_eq!(
        store
            .list_events("task", &task_id.to_string())
            .expect("read task events after refusal")
            .len(),
        event_count
    );
}

fn parent_and_child(
    store: &Store,
    objective_id: i64,
) -> (cathulu_core::workflow::Task, cathulu_core::workflow::Task) {
    let parent = create_task(store, objective_id, &input("Parent", 0, None));
    let child = create_task(store, objective_id, &input("Child", 0, None));
    let child = store
        .update_task_plan(
            child.id,
            child.revision,
            &TaskPlanInput {
                parent_task_id: Some(parent.id),
                ..input("Child", 0, None)
            },
        )
        .expect("assign same-project parent");
    (parent, child)
}

#[test]
fn f1_refuses_administrative_completion_after_failed_run() {
    let root = scratch_root("f1-failed");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    accepted_project_summary(&store, project_id);
    let objective = store
        .create_objective(project_id, "F1 gate", "administrative completion")
        .expect("create objective");
    let task = create_task(&store, objective.id, &input("Failed QA", 0, None));
    let workflow = qa_workflow(&store);
    let run = finish_linked_run(
        &store,
        project_id,
        objective.id,
        task.id,
        Some(workflow),
        Some("failed"),
    );
    assert_eq!(run.status, "failed");
    let run_id = run.id.to_string();
    assert_administrative_completion_refused(&store, task.id, &[run_id.as_str(), "failed"]);
}

#[test]
fn f1_refuses_administrative_completion_after_blocked_run() {
    let root = scratch_root("f1-blocked");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    accepted_project_summary(&store, project_id);
    let objective = store
        .create_objective(project_id, "F1 gate", "administrative completion")
        .expect("create objective");
    let task = create_task(&store, objective.id, &input("No checks", 0, None));
    let run = finish_linked_run(&store, project_id, objective.id, task.id, None, None);
    assert_eq!(run.status, "blocked");
    let run_id = run.id.to_string();
    assert_administrative_completion_refused(&store, task.id, &[run_id.as_str(), "blocked"]);
}

#[test]
fn f1_refuses_administrative_completion_after_inconclusive_run() {
    let root = scratch_root("f1-inconclusive");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    accepted_project_summary(&store, project_id);
    let objective = store
        .create_objective(project_id, "F1 gate", "administrative completion")
        .expect("create objective");
    let task = create_task(&store, objective.id, &input("Inconclusive QA", 0, None));
    let workflow = qa_workflow(&store);
    let run = finish_linked_run(
        &store,
        project_id,
        objective.id,
        task.id,
        Some(workflow),
        Some("inconclusive"),
    );
    assert_eq!(run.status, "inconclusive");
    let run_id = run.id.to_string();
    assert_administrative_completion_refused(&store, task.id, &[run_id.as_str(), "inconclusive"]);
}

#[test]
fn f1_refuses_administrative_completion_after_cancelled_run() {
    let root = scratch_root("f1-cancelled");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    accepted_project_summary(&store, project_id);
    let objective = store
        .create_objective(project_id, "F1 gate", "administrative completion")
        .expect("create objective");
    let task = create_task(&store, objective.id, &input("Cancelled run", 0, None));
    let run = finish_linked_run(&store, project_id, objective.id, task.id, None, None);
    // No public path sets runs.status to 'cancelled'; this fixture pins the gate's coverage.
    let connection = Connection::open(store.database_path()).expect("open fixture database");
    connection
        .execute(
            "UPDATE runs SET status = 'cancelled' WHERE id = ?1",
            [run.id],
        )
        .expect("fixture: cancel run");
    drop(connection);
    let run_id = run.id.to_string();
    assert_administrative_completion_refused(&store, task.id, &[run_id.as_str(), "cancelled"]);
}

#[test]
fn f1_refuses_administrative_completion_when_failed_run_precedes_verified_run() {
    let root = scratch_root("f1-failed-then-verified");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    accepted_project_summary(&store, project_id);
    let objective = store
        .create_objective(project_id, "F1 gate", "administrative completion")
        .expect("create objective");
    let task = create_task(
        &store,
        objective.id,
        &input("Failed then verified", 0, None),
    );
    let workflow = qa_workflow(&store);
    let failed = finish_linked_run(
        &store,
        project_id,
        objective.id,
        task.id,
        Some(workflow),
        Some("failed"),
    );
    let verified = finish_linked_run(
        &store,
        project_id,
        objective.id,
        task.id,
        Some(workflow),
        Some("passed"),
    );
    assert_eq!(verified.status, "verified_success");
    let failed_id = failed.id.to_string();
    assert_administrative_completion_refused(&store, task.id, &[failed_id.as_str(), "failed"]);
}

#[test]
fn f1_refuses_administrative_completion_while_verified_run_is_unaccepted() {
    let root = scratch_root("f1-verified-unaccepted");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    accepted_project_summary(&store, project_id);
    let objective = store
        .create_objective(project_id, "F1 gate", "administrative completion")
        .expect("create objective");
    let task = create_task(
        &store,
        objective.id,
        &input("Verified, unaccepted", 0, None),
    );
    let workflow = qa_workflow(&store);
    let verified = finish_linked_run(
        &store,
        project_id,
        objective.id,
        task.id,
        Some(workflow),
        Some("passed"),
    );
    assert_eq!(verified.status, "verified_success");
    let verified_id = verified.id.to_string();
    assert_administrative_completion_refused(
        &store,
        task.id,
        &[verified_id.as_str(), "accept_task_run"],
    );
}

#[test]
fn f1_verified_completion_still_works_after_earlier_failed_run() {
    let root = scratch_root("f1-verified-guard");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    accepted_project_summary(&store, project_id);
    let objective = store
        .create_objective(project_id, "F1 gate", "verified path")
        .expect("create objective");
    let task = create_task(
        &store,
        objective.id,
        &input("Failed then verified", 0, None),
    );
    let workflow = qa_workflow(&store);
    let failed = finish_linked_run(
        &store,
        project_id,
        objective.id,
        task.id,
        Some(workflow),
        Some("failed"),
    );
    let verified = finish_linked_run(
        &store,
        project_id,
        objective.id,
        task.id,
        Some(workflow),
        Some("passed"),
    );
    assert_eq!(failed.status, "failed");
    assert_eq!(verified.status, "verified_success");
    let current = store
        .get_task(task.id)
        .expect("read task")
        .expect("task exists");
    let accepted = store
        .accept_task_run(task.id, verified.id, current.revision)
        .expect("a verified run must still complete the task");
    assert_eq!(accepted.status, "completed");
    assert_eq!(accepted.completion_kind.as_deref(), Some("verified_run"));
    assert_eq!(accepted.completion_run_id, Some(verified.id));
}

#[test]
fn f1_failed_child_keeps_parent_open_until_cancelled_with_reason() {
    let root = scratch_root("f1-failed-child");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    let objective = store
        .create_objective(project_id, "F1 gate", "parent and child")
        .expect("create objective");
    let (parent, child) = parent_and_child(&store, objective.id);
    let child = store
        .set_task_status(child.id, "failed")
        .expect("mark child failed");
    let parent = store
        .get_task(parent.id)
        .expect("read parent")
        .expect("parent exists");
    let error = store
        .complete_task_administratively(parent.id, parent.revision, "finish parent")
        .expect_err("a failed child keeps the parent open");
    assert!(error.contains("child task"), "{error}");
    let child = store
        .cancel_task(child.id, child.revision, "descoped after failed QA")
        .expect("cancel failed child with a reason");
    assert_eq!(child.status, "cancelled");
    let parent = store
        .get_task(parent.id)
        .expect("read parent")
        .expect("parent exists");
    let completed = store
        .complete_task_administratively(parent.id, parent.revision, "finish parent")
        .expect("reasoned cancellation is the child waiver");
    assert_eq!(completed.status, "completed");
}

#[test]
fn f1_legacy_cancelled_child_keeps_parent_open() {
    let root = scratch_root("f1-legacy-child");
    let store = Store::open(root.join("data")).expect("open isolated M48 store");
    let project_id = register_fixture_project(&store, &root.join("repos"), "project");
    let objective = store
        .create_objective(project_id, "F1 gate", "legacy child")
        .expect("create objective");
    let (parent, child) = parent_and_child(&store, objective.id);
    // Migrated cancellations carry the reserved reason, which is not waiver evidence.
    let connection = Connection::open(store.database_path()).expect("open fixture database");
    connection
        .execute(
            "UPDATE tasks SET status = 'cancelled', cancellation_reason = 'legacy_unknown',
               cancelled_at = updated_at WHERE id = ?1",
            [child.id],
        )
        .expect("fixture: legacy cancellation");
    drop(connection);
    let parent = store
        .get_task(parent.id)
        .expect("read parent")
        .expect("parent exists");
    let error = store
        .complete_task_administratively(parent.id, parent.revision, "finish parent")
        .expect_err("legacy cancellation is not waiver evidence");
    assert!(error.contains("child task"), "{error}");
}
