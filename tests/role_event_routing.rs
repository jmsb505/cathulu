use cathulu_core::delegation::{DelegationTaskPacket, WorkerCheckResult, WorkerReport};
use cathulu_core::Store;
use serde_json::json;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_data_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    std::env::current_dir()
        .expect("resolve repository root")
        .join("target")
        .join(format!(
            "cathulu-role-events-{}-{stamp}",
            std::process::id()
        ))
}

fn task_packet(title: &str) -> DelegationTaskPacket {
    DelegationTaskPacket {
        schema_version: 1,
        title: title.to_string(),
        objective: "Exercise project-scoped event routing.".to_string(),
        acceptance_criteria: vec!["Actionable events reach only the assigned PM.".to_string()],
        constraints: vec!["Use only isolated fixture data.".to_string()],
        allowed_scope: vec!["src/**".to_string()],
        referenced_memory_ids: vec![],
        referenced_artifact_ids: vec![],
        requested_report_fields: vec!["summary".to_string(), "checks".to_string()],
    }
}

fn worker_report(summary: &str) -> WorkerReport {
    WorkerReport {
        schema_version: 1,
        summary: summary.to_string(),
        outcome: "completed".to_string(),
        changed_files: vec![],
        commits: vec![],
        check_results: vec![WorkerCheckResult {
            name: "fixture-check".to_string(),
            status: "passed".to_string(),
            summary: "Fixture-only validation passed.".to_string(),
        }],
        blockers: vec![],
        artifact_ids: vec![],
        quality_check_ids: vec![],
    }
}

fn create_project(store: &Store, connection: &rusqlite::Connection, name: &str) -> i64 {
    connection
        .execute(
            "INSERT INTO projects
             (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
             VALUES (?1, ?2, 'main', NULL, 0, '[]', 1, 1)",
            rusqlite::params![name, format!("C:/fixture/{name}")],
        )
        .expect("seed isolated fixture project");
    let project_id = connection.last_insert_rowid();
    let objective = store
        .create_objective(
            project_id,
            "Event routing",
            "Validate durable selective routing",
        )
        .expect("create fixture objective");
    let task = store
        .create_task(objective.id, "Exercise routing", "Use fixture-only state")
        .expect("create fixture task");
    let run = store
        .start_run(project_id, Some(objective.id), Some(task.id), None)
        .expect("start fixture run");
    store
        .record_role_work_plan(
            project_id,
            run.id,
            "main",
            None,
            &json!({
                "schema_version": 1,
                "desired_outcome": "Validate event routing.",
                "phases": ["Prepare", "Route", "Verify"],
                "project_boundary": [name],
                "dependencies": [],
                "acceptance_criteria": ["Events stay inside the assigned project."],
                "owner_approval_gates": ["No external or destructive operations."],
                "known_risks": []
            }),
            &format!("main-brief-{name}"),
        )
        .expect("record Main brief");
    let manager = store
        .prepare_delegation(
            project_id,
            task.id,
            run.id,
            "sub_overseer",
            None,
            &task_packet("Project Manager"),
            None,
            &format!("manager-{name}"),
            None,
            None,
            None,
        )
        .expect("prepare Project Manager");
    activate_assignment(
        store,
        manager.id,
        project_id,
        &format!("project-manager:{project_id}"),
    );
    store
        .mark_delegation_dispatched(manager.id)
        .expect("dispatch Project Manager fixture assignment");
    let manager_plan = json!({
        "schema_version": 1,
        "summary": "Delegate one isolated routing task.",
        "manager_actions": ["Review the Worker report."],
        "worker_tasks": [{"task_key":"worker-one", "task_packet":task_packet("Worker")}]
    });
    let plan = store
        .record_role_work_plan(
            project_id,
            run.id,
            "project_manager",
            Some(manager.id),
            &manager_plan,
            &format!("manager-plan-{name}"),
        )
        .expect("record PM plan");
    store
        .review_role_work_plan(
            plan.id,
            "main_overseer",
            "accepted",
            "Fixture plan is bounded to its project.",
            &format!("manager-plan-review-{name}"),
        )
        .expect("accept PM plan");
    let worker = store
        .prepare_delegation(
            project_id,
            task.id,
            run.id,
            "worker",
            Some(manager.id),
            &task_packet("Worker"),
            Some("worker-one"),
            &format!("worker-{name}"),
            None,
            None,
            None,
        )
        .expect("prepare Worker");
    activate_assignment(
        store,
        worker.id,
        project_id,
        &format!("worker:{}", worker.id),
    );
    store
        .mark_delegation_dispatched(worker.id)
        .expect("dispatch Worker fixture assignment");
    worker.id
}

fn activate_assignment(store: &Store, delegation_id: i64, project_id: i64, receiver: &str) {
    let scope_id = project_id.to_string();
    let candidates = store
        .search_memory_candidates(
            "project",
            Some(&scope_id),
            "event routing fixture",
            "scope",
            Some(receiver),
            10,
        )
        .expect("complete fixture memory search");
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
            Some(&scope_id),
            "scope",
            receiver,
            "search_memory_candidates",
            &references,
        )
        .expect("record memory retrieval");
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
            &format!("fixture-activation-{delegation_id}"),
        )
        .expect("record completed activation");
}

#[test]
fn worker_events_route_once_to_their_project_manager_without_self_wake() {
    let data_dir = temporary_data_dir();
    let store = Store::open(&data_dir).expect("initialize fixture store");
    let connection = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open fixture database");
    let first_worker = create_project(&store, &connection, "routing-project-a");
    let second_worker = create_project(&store, &connection, "routing-project-b");
    drop(connection);

    store
        .record_delegation_update(
            first_worker,
            "working",
            Some("investigating"),
            "Routine progress is visible but should not wake the PM.",
            None,
            None,
            "project-a-routine-progress",
        )
        .expect("record routine progress");
    store
        .submit_delegation_report(
            first_worker,
            &WorkerReport {
                schema_version: 1,
                summary: "Fixture report awaiting PM review.".to_string(),
                outcome: "completed".to_string(),
                changed_files: vec![],
                commits: vec![],
                check_results: vec![WorkerCheckResult {
                    name: "fixture-check".to_string(),
                    status: "passed".to_string(),
                    summary: "Fixture-only validation passed.".to_string(),
                }],
                blockers: vec![],
                artifact_ids: vec![],
                quality_check_ids: vec![],
            },
            "project-a-report",
        )
        .expect("submit fixture Worker report");
    store
        .review_delegation_report(
            first_worker,
            "project_manager",
            "needs_follow_up",
            "Fixture review; do not route the PM's own review back to itself.",
            "project-a-pm-review",
        )
        .expect("record PM review");
    store
        .record_delegation_update(
            second_worker,
            "action_required",
            Some("investigating"),
            "Worker needs a bounded PM decision.",
            Some("worker-question"),
            Some("Choose the documented fixture behavior."),
            "project-b-action-required",
        )
        .expect("record actionable Worker update");

    assert_eq!(
        store.route_pending_role_events(100).expect("route events"),
        6
    );
    assert_eq!(
        store.route_pending_role_events(100).expect("retry routing"),
        0
    );

    let inbox = store.role_turn_inbox(100).expect("read routed inbox");
    assert_eq!(
        inbox.len(),
        6,
        "routine progress and the PM's own Worker review do not wake it"
    );
    let main_plan_messages = inbox
        .iter()
        .filter(|message| message.target_binding_key == "main-overseer")
        .collect::<Vec<_>>();
    assert_eq!(
        main_plan_messages.len(),
        2,
        "each PM plan reaches the single global Main role"
    );
    assert!(main_plan_messages
        .iter()
        .all(|message| message.event_type == "role_work_plan_recorded"));
    let plan_review_messages = inbox
        .iter()
        .filter(|message| message.event_type == "role_work_plan_reviewed")
        .collect::<Vec<_>>();
    assert_eq!(
        plan_review_messages.len(),
        2,
        "Main's review wakes each affected PM"
    );
    let report_message = inbox
        .iter()
        .find(|message| message.source_delegation_id == first_worker)
        .expect("project A report routed");
    assert_eq!(
        report_message.target_binding_key,
        format!("project-manager:{}", report_message.project_id)
    );
    assert_eq!(report_message.event_type, "delegation_report_submitted");
    let action_message = inbox
        .iter()
        .find(|message| message.source_delegation_id == second_worker)
        .expect("project B action routed");
    assert_eq!(
        action_message.target_binding_key,
        format!("project-manager:{}", action_message.project_id)
    );
    assert_eq!(action_message.event_type, "agent_action_required");
    assert_ne!(report_message.project_id, action_message.project_id);

    let active_project_manager = vec![
        report_message.target_binding_key.clone(),
        "main-overseer".to_string(),
    ];
    let claimed = store
        .claim_next_role_inbox_message(&active_project_manager)
        .expect("claim an independent project's event while another PM and Main are active")
        .expect("independent project can proceed");
    assert_ne!(
        claimed.target_binding_key,
        report_message.target_binding_key
    );
    assert_ne!(claimed.target_binding_key, "main-overseer");
    assert_eq!(claimed.status, "starting");
    store
        .set_role_inbox_status(
            claimed.id,
            "blocked",
            Some("fixture stopped before app-server dispatch"),
        )
        .expect("record safe fixture stop");
    let active_roles = inbox
        .iter()
        .map(|message| message.target_binding_key.clone())
        .collect::<Vec<_>>();
    assert!(store
        .claim_next_role_inbox_message(&active_roles)
        .expect("do not wake roles that already have an active turn")
        .is_none());
    let remaining = store
        .claim_next_role_inbox_message(&[])
        .expect("claim remaining project event")
        .expect("the global Main plan review remains queued");
    assert_eq!(remaining.target_binding_key, "main-overseer");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn pm_escalation_wakes_main_and_owner_decision_returns_only_to_that_pm() {
    let data_dir = temporary_data_dir();
    let store = Store::open(&data_dir).expect("initialize fixture store");
    let connection = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open fixture database");
    let worker_id = create_project(&store, &connection, "owner-relay-project");
    let (manager_id, project_id): (i64, i64) = connection
        .query_row(
            "SELECT parent_delegation_id, project_id FROM delegations WHERE id = ?1",
            rusqlite::params![worker_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .expect("resolve fixture Project Manager");
    connection
        .execute(
            "UPDATE role_event_cursor SET last_event_id = (SELECT COALESCE(MAX(id), 0) FROM events)",
            [],
        )
        .expect("start routing after fixture setup");
    drop(connection);

    store
        .record_delegation_update(
            manager_id,
            "action_required",
            Some("coordinating"),
            "This action crosses the Owner boundary.",
            Some("external-account-change"),
            Some("Change an external account setting."),
            "pm-escalation-action",
        )
        .expect("record PM escalation");
    let request = store
        .request_owner_approval(
            manager_id,
            "external-account-change",
            "An external account change requires Owner approval.",
            "Change the account setting.",
            "It changes access outside the project workspace.",
            "pm-owner-request",
        )
        .expect("create explicit Owner request");
    store
        .record_owner_approval_decision_from_desktop(request.id, &request.request_hash, "approved")
        .expect("record the local Owner decision");

    assert_eq!(
        store
            .route_pending_role_events(100)
            .expect("route PM and Owner events"),
        2
    );
    let inbox = store.role_turn_inbox(100).expect("read Main inbox");
    assert_eq!(inbox.len(), 2);
    assert!(inbox
        .iter()
        .all(|message| message.target_binding_key == "main-overseer"));
    assert!(inbox
        .iter()
        .any(|message| message.event_type == "agent_action_required"));
    assert!(inbox
        .iter()
        .any(|message| message.event_type == "owner_approval_decided"));
    assert!(inbox.iter().all(|message| message.project_id == project_id));

    let relay = store
        .relay_owner_approval_decision(request.id)
        .expect("Main relays explicit Owner decision");
    let duplicate = store
        .relay_owner_approval_decision(request.id)
        .expect("relay retry is idempotent");
    assert_eq!(relay["eventId"], duplicate["eventId"]);
    assert_eq!(
        store
            .route_pending_role_events(100)
            .expect("route decision to PM"),
        1
    );
    let routed = store.role_turn_inbox(100).expect("read PM inbox");
    let owner_response = routed
        .iter()
        .find(|message| message.event_type == "owner_decision_relayed")
        .expect("Owner decision reaches the affected PM");
    assert_eq!(
        owner_response.target_binding_key,
        format!("project-manager:{project_id}")
    );
    assert_eq!(owner_response.target_delegation_id, Some(manager_id));
    assert_eq!(owner_response.source_delegation_id, manager_id);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn pm_result_wakes_main_and_main_review_returns_to_only_that_pm() {
    let data_dir = temporary_data_dir();
    let store = Store::open(&data_dir).expect("initialize fixture store");
    let connection = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open fixture database");
    let worker_id = create_project(&store, &connection, "pm-result-project");
    let manager_id: i64 = connection
        .query_row(
            "SELECT parent_delegation_id FROM delegations WHERE id = ?1",
            rusqlite::params![worker_id],
            |row| row.get(0),
        )
        .expect("resolve fixture Project Manager");
    drop(connection);
    store
        .submit_delegation_report(
            worker_id,
            &worker_report("Worker completed the bounded task."),
            "pm-result-worker-report",
        )
        .expect("submit Worker report");
    store
        .review_delegation_report(
            worker_id,
            "project_manager",
            "accepted",
            "Worker evidence meets the fixture task packet.",
            "pm-result-worker-review",
        )
        .expect("PM accepts Worker report");
    let connection = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("reopen fixture database");
    connection
        .execute(
            "UPDATE role_event_cursor SET last_event_id = (SELECT COALESCE(MAX(id), 0) FROM events)",
            [],
        )
        .expect("start routing after Worker review");
    drop(connection);

    store
        .submit_delegation_report(
            manager_id,
            &worker_report("Project Manager reports a qualified project result."),
            "pm-result-manager-report",
        )
        .expect("submit PM result");
    assert_eq!(
        store
            .route_pending_role_events(100)
            .expect("route PM result to Main"),
        1
    );
    let main_message = store
        .role_turn_inbox(100)
        .expect("read Main inbox")
        .into_iter()
        .find(|message| message.target_binding_key == "main-overseer")
        .expect("PM result wakes Main");
    assert_eq!(main_message.event_type, "delegation_report_submitted");
    assert_eq!(main_message.source_delegation_id, manager_id);

    store
        .review_delegation_report(
            manager_id,
            "main_overseer",
            "needs_follow_up",
            "Fixture review requests one bounded follow-up.",
            "pm-result-main-review",
        )
        .expect("Main records qualified follow-up review");
    assert_eq!(
        store
            .route_pending_role_events(100)
            .expect("route Main review to PM"),
        1
    );
    let pm_message = store
        .role_turn_inbox(100)
        .expect("read role inbox")
        .into_iter()
        .find(|message| message.event_type == "delegation_report_reviewed")
        .expect("Main review wakes the affected PM");
    assert_eq!(
        pm_message.target_binding_key,
        format!("project-manager:{}", pm_message.project_id)
    );
    assert_eq!(pm_message.target_delegation_id, Some(manager_id));
    assert_eq!(pm_message.source_delegation_id, manager_id);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn main_resolved_pm_action_is_returned_to_that_pm_without_self_wake() {
    let data_dir = temporary_data_dir();
    let store = Store::open(&data_dir).expect("initialize fixture store");
    let connection = rusqlite::Connection::open(data_dir.join("cathulu.sqlite3"))
        .expect("open fixture database");
    let worker_id = create_project(&store, &connection, "main-resolution-project");
    let manager_id: i64 = connection
        .query_row(
            "SELECT parent_delegation_id FROM delegations WHERE id = ?1",
            rusqlite::params![worker_id],
            |row| row.get(0),
        )
        .expect("resolve fixture Project Manager");
    connection
        .execute(
            "UPDATE role_event_cursor SET last_event_id = (SELECT COALESCE(MAX(id), 0) FROM events)",
            [],
        )
        .expect("start routing after fixture setup");
    drop(connection);

    store
        .record_delegation_update(
            manager_id,
            "action_required",
            Some("coordinating"),
            "A reversible choice needs Main's coordination.",
            Some("bounded-choice"),
            Some("Choose the already documented option."),
            "main-resolution-request",
        )
        .expect("record PM escalation");
    assert_eq!(
        store
            .route_pending_role_events(100)
            .expect("route PM escalation"),
        1
    );
    let main_message = store
        .role_turn_inbox(100)
        .expect("read Main inbox")
        .into_iter()
        .find(|message| message.target_binding_key == "main-overseer")
        .expect("PM escalation wakes Main");
    assert_eq!(main_message.event_type, "agent_action_required");

    store
        .record_main_delegation_update(
            manager_id,
            "action_resolved",
            Some("coordinating"),
            "Use the documented option already accepted in the Main brief.",
            Some("bounded-choice"),
            None,
            "main-resolution-answer",
        )
        .expect("Main records its bounded decision");
    assert_eq!(
        store
            .route_pending_role_events(100)
            .expect("route answer to PM"),
        1
    );
    let answer = store
        .role_turn_inbox(100)
        .expect("read PM inbox")
        .into_iter()
        .find(|message| message.event_type == "agent_action_resolved")
        .expect("Main decision wakes the affected PM");
    assert_eq!(
        answer.target_binding_key,
        format!("project-manager:{}", answer.project_id)
    );
    assert_eq!(answer.target_delegation_id, Some(manager_id));

    let _ = std::fs::remove_dir_all(data_dir);
}
