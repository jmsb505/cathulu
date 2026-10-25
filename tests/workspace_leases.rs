use cathulu_core::agent_provider::{
    fixtures::ScriptedProvider, AgentProvider, CredentialSource, PermissionManifest, ProviderId,
    WorkspaceAccess,
};
use cathulu_core::worktree::WorkspaceLeaseStatus;
use cathulu_core::Store;
use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

struct Fixture {
    _store_dir: PathBuf,
    store: Store,
    project_id: i64,
    repo: PathBuf,
    worker_one: PathBuf,
    worker_two: PathBuf,
    unrelated_worktree: PathBuf,
    worker_one_id: i64,
    worker_two_id: i64,
    manager_id: i64,
}

fn fixture() -> Fixture {
    let root = std::env::current_dir()
        .expect("resolve repository root")
        .join("target")
        .join("manual-dev")
        .join("codex")
        .join(format!("workspace-leases-{}", Uuid::new_v4()));
    let repo = root.join("project");
    let worker_one = root.join("worktrees/worker-one");
    let worker_two = root.join("worktrees/worker-two");
    let unrelated_repo = root.join("unrelated");
    let unrelated_worktree = root.join("unrelated-worktree");
    let store_dir = root.join("store");
    fs::create_dir_all(root.join("worktrees")).expect("create linked worktree parent");
    fs::create_dir_all(repo.join("src")).expect("create fixture repository");
    init_repo(&repo);
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "worker-one",
            path_arg(&worker_one),
            "HEAD",
        ],
    );
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "worker-two",
            path_arg(&worker_two),
            "HEAD",
        ],
    );
    fs::create_dir_all(unrelated_repo.join("src")).expect("create unrelated repository");
    init_repo(&unrelated_repo);
    run_git(
        &unrelated_repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "unrelated-worker",
            path_arg(&unrelated_worktree),
            "HEAD",
        ],
    );

    let store = Store::open(&store_dir).expect("open scratch Cathulu store");
    let connection = Connection::open(store.database_path()).expect("open scratch database");
    connection
        .execute(
            "INSERT INTO projects(name, path, registered_at, reconciled_at)
             VALUES ('lease fixture', ?1, 1, 1)",
            params![repo.to_string_lossy()],
        )
        .expect("register scratch project");
    let project_id = connection.last_insert_rowid();
    let manager_id = insert_delegation(&connection, project_id, "sub_overseer", None, "manager");
    let worker_one_id = insert_delegation(
        &connection,
        project_id,
        "worker",
        Some(manager_id),
        "worker-one",
    );
    let worker_two_id = insert_delegation(
        &connection,
        project_id,
        "worker",
        Some(manager_id),
        "worker-two",
    );
    drop(connection);

    Fixture {
        _store_dir: store_dir,
        store,
        project_id,
        repo,
        worker_one,
        worker_two,
        unrelated_worktree,
        worker_one_id,
        worker_two_id,
        manager_id,
    }
}

fn init_repo(path: &Path) {
    run_git(path, &["init", "--quiet", "--initial-branch=main"]);
    run_git(path, &["config", "user.name", "Cathulu lease fixture"]);
    run_git(
        path,
        &["config", "user.email", "lease-fixture@example.invalid"],
    );
    fs::write(path.join("src/lib.rs"), "pub fn fixture() {}\n").expect("write Git fixture");
    run_git(path, &["add", "--all"]);
    run_git(path, &["commit", "--quiet", "-m", "fixture base"]);
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("launch local Git fixture command");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn path_arg(path: &Path) -> &str {
    path.to_str().expect("fixture path is valid UTF-8")
}

fn insert_delegation(
    connection: &Connection,
    project_id: i64,
    role: &str,
    parent_id: Option<i64>,
    label: &str,
) -> i64 {
    connection
        .execute(
            "INSERT INTO objectives(project_id, title, description, created_at, updated_at)
             VALUES (?1, ?2, '', 1, 1)",
            params![project_id, format!("{label} objective")],
        )
        .expect("insert fixture objective");
    let objective_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO tasks(objective_id, title, description, created_at, updated_at)
             VALUES (?1, ?2, '', 1, 1)",
            params![objective_id, format!("{label} task")],
        )
        .expect("insert fixture task");
    let task_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO runs(project_id, objective_id, task_id, status, started_at)
             VALUES (?1, ?2, ?3, 'running', 1)",
            params![project_id, objective_id, task_id],
        )
        .expect("insert fixture run");
    let run_id = connection.last_insert_rowid();
    let hash = format!("packet-{label}");
    connection
        .execute(
            "INSERT INTO delegations
             (project_id, task_id, run_id, role, status, packet_schema_version, packet_json,
              packet_hash, request_hash, idempotency_key_hash, parent_delegation_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'prepared', 1, '{}', ?5, ?6, ?7, ?8, 1, 1)",
            params![
                project_id,
                task_id,
                run_id,
                role,
                hash,
                format!("request-{label}"),
                format!("idempotency-{label}"),
                parent_id
            ],
        )
        .expect("insert fixture delegation");
    connection.last_insert_rowid()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[test]
fn writer_leases_require_clean_linked_worktrees_and_serialize_project_writes() {
    let fixture = fixture();
    let lease = fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_one_id,
            "worker-session-one",
            &fixture.worker_one,
            WorkspaceAccess::WorkspaceWrite,
            100,
            60,
        )
        .expect("claim the clean linked worker worktree");
    assert_eq!(lease.status, WorkspaceLeaseStatus::Active);
    assert_eq!(lease.access, WorkspaceAccess::WorkspaceWrite);
    assert_eq!(lease.project_id, fixture.project_id);

    let conflict = fixture.store.claim_delegation_workspace(
        fixture.worker_two_id,
        "worker-session-two",
        &fixture.worker_two,
        WorkspaceAccess::WorkspaceWrite,
        101,
        60,
    );
    assert!(conflict.unwrap_err().contains("already leased"));

    let coordinator = fixture
        .store
        .claim_delegation_workspace(
            fixture.manager_id,
            "manager-session",
            &fixture.repo,
            WorkspaceAccess::ReadOnly,
            102,
            60,
        )
        .expect("read-only coordinator may inspect the registered root");
    assert_eq!(coordinator.access, WorkspaceAccess::ReadOnly);

    assert!(fixture
        .store
        .claim_delegation_workspace(
            fixture.manager_id,
            "manager-writer",
            &fixture.repo,
            WorkspaceAccess::WorkspaceWrite,
            103,
            60,
        )
        .unwrap_err()
        .contains("read-only"));
    assert!(fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_two_id,
            "root-writer",
            &fixture.repo,
            WorkspaceAccess::WorkspaceWrite,
            104,
            60,
        )
        .unwrap_err()
        .contains("registered project root"));

    fs::write(
        fixture.worker_one.join("owner-note.txt"),
        "preserve this local file",
    )
    .expect("make the scratch worktree dirty");
    let released = fixture
        .store
        .release_workspace_lease(lease.id, "worker-session-one", 105, "turn ended")
        .expect("release without cleaning the workspace");
    assert_eq!(released.status, WorkspaceLeaseStatus::Released);
    assert_eq!(
        fs::read_to_string(fixture.worker_one.join("owner-note.txt")).unwrap(),
        "preserve this local file"
    );
    assert!(fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_one_id,
            "worker-session-one-retry",
            &fixture.worker_one,
            WorkspaceAccess::WorkspaceWrite,
            106,
            60,
        )
        .unwrap_err()
        .contains("must be clean"));

    fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_two_id,
            "worker-session-two",
            &fixture.worker_two,
            WorkspaceAccess::WorkspaceWrite,
            107,
            60,
        )
        .expect("next writer can use a separate clean worktree after release");
}

#[test]
fn workspace_path_aliases_and_wrong_git_common_directories_are_rejected() {
    let fixture = fixture();
    let alias = fixture.worker_one.join(".");
    assert!(fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_one_id,
            "alias-session",
            &alias,
            WorkspaceAccess::WorkspaceWrite,
            100,
            60,
        )
        .unwrap_err()
        .contains("aliases and symlink"));

    assert!(fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_one_id,
            "unrelated-session",
            &fixture.unrelated_worktree,
            WorkspaceAccess::WorkspaceWrite,
            101,
            60,
        )
        .unwrap_err()
        .contains("not an exact linked worktree"));
}

#[test]
fn workspace_lease_recovery_quarantines_attempt_before_cross_provider_replacement() {
    let fixture = fixture();
    let started = now();
    let lease = fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_one_id,
            "worker-session-one",
            &fixture.worker_one,
            WorkspaceAccess::WorkspaceWrite,
            started,
            60,
        )
        .expect("claim lease for provider attempt fixture");

    let binding_key = format!("worker:{}", fixture.worker_one_id);
    let codex_session_id = "replacement-codex-session";
    let connection = Connection::open(fixture.store.database_path()).unwrap();
    connection
        .execute(
            "INSERT INTO role_thread_bindings
             (binding_key, role_key, delegation_id, project_id, thread_id, status,
              last_turn_id, created_at, updated_at)
             VALUES (?1, NULL, ?2, ?3, ?4, 'active', NULL, ?5, ?5)",
            params![
                binding_key,
                fixture.worker_one_id,
                fixture.project_id,
                codex_session_id,
                started
            ],
        )
        .expect("insert worker compatibility binding");
    connection
        .execute(
            "INSERT INTO role_dispatch_outbox
             (delegation_id, idempotency_key_hash, status, created_at, updated_at)
             VALUES (?1, ?2, 'queued', ?3, ?3)",
            params![fixture.worker_one_id, "a".repeat(64), started],
        )
        .expect("insert queued dispatch fixture");
    let dispatch_id = connection.last_insert_rowid();
    drop(connection);

    let session = cathulu_core::agent_provider::ExternalSession {
        provider_id: ProviderId::claude(),
        session_id: cathulu_core::agent_provider::ExternalId::new("initial-claude-session")
            .unwrap(),
    };
    fixture
        .store
        .record_provider_session(&session, &binding_key, "active", None, None)
        .expect("persist scoped provider session");
    let capability = fixture
        .store
        .issue_role_thread_capability_for_workspace(
            &binding_key,
            "worker",
            fixture.project_id,
            fixture.worker_one_id,
            lease.id,
        )
        .expect("issue a worker capability bound to its exact lease");
    let attempt_key = cathulu_core::agent_provider::AttemptKey::new("lease-attempt-one").unwrap();
    let attempt = fixture
        .store
        .begin_provider_attempt_under_workspace_lease(
            &attempt_key,
            &session,
            Some(dispatch_id),
            None,
            fixture.worker_one_id,
            &"b".repeat(64),
            lease.id,
        )
        .expect("persist a provider attempt under its exact lease");
    assert_eq!(attempt.workspace_lease_id, Some(lease.id));
    fixture
        .store
        .bind_capability_to_provider_attempt(&capability.token, &session, &attempt_key)
        .expect("bind capability to an attempt under the same lease");
    assert_eq!(
        fixture
            .store
            .resolve_role_thread_capability(&capability.token)
            .unwrap()
            .workspace_lease_id,
        Some(lease.id)
    );

    let renewal_time = started + 30;
    let renewed = fixture
        .store
        .renew_workspace_lease(lease.id, "worker-session-one", renewal_time, 60)
        .expect("a live provider attempt keeps its exact workspace lease renewable");
    assert_eq!(renewed.status, WorkspaceLeaseStatus::Active);
    assert!(renewed.lease_expires_at > lease.lease_expires_at);

    let codex_session = cathulu_core::agent_provider::ExternalSession {
        provider_id: ProviderId::codex(),
        session_id: cathulu_core::agent_provider::ExternalId::new(codex_session_id).unwrap(),
    };
    assert!(fixture
        .store
        .record_provider_session(&codex_session, &binding_key, "active", None, None)
        .unwrap_err()
        .contains("unresolved provider attempt"));
    let claude_status_before_reconciliation: String =
        Connection::open(fixture.store.database_path())
            .unwrap()
            .query_row(
                "SELECT status FROM provider_sessions
             WHERE provider_id = 'claude' AND session_id = 'initial-claude-session'",
                [],
                |row| row.get(0),
            )
            .expect("keep the Claude session active while its attempt is unresolved");
    assert_eq!(claude_status_before_reconciliation, "active");

    let future = renewed.lease_expires_at + 1;
    assert_eq!(
        fixture.store.expire_stale_workspace_leases(future).unwrap(),
        1
    );
    assert!(fixture
        .store
        .begin_provider_attempt_under_workspace_lease(
            &attempt_key,
            &session,
            Some(dispatch_id),
            None,
            fixture.worker_one_id,
            &"b".repeat(64),
            lease.id,
        )
        .unwrap_err()
        .contains("lease is inactive"));
    let expired = fixture
        .store
        .release_workspace_lease(
            lease.id,
            "worker-session-one",
            future + 1,
            "lease expiry observed",
        )
        .expect("expiry with a live attempt remains quarantined");
    assert_eq!(expired.status, WorkspaceLeaseStatus::NeedsReconcile);
    assert!(fixture
        .store
        .resolve_workspace_lease(
            lease.id,
            "worker-session-one",
            future + 2,
            "attempt still uncertain",
        )
        .unwrap_err()
        .contains("attempt remains uncertain"));
    assert!(fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_two_id,
            "worker-session-two",
            &fixture.worker_two,
            WorkspaceAccess::WorkspaceWrite,
            future + 3,
            60,
        )
        .is_err());

    Connection::open(fixture.store.database_path())
        .unwrap()
        .execute(
            "UPDATE provider_attempts SET status = 'completed', updated_at = ?1
             WHERE attempt_key = ?2",
            params![future + 4, attempt_key.as_str()],
        )
        .expect("fixture represents an externally reconciled terminal attempt");
    let resolved = fixture
        .store
        .resolve_workspace_lease(
            lease.id,
            "worker-session-one",
            future + 5,
            "provider result reconciled as completed",
        )
        .expect("explicit terminal reconciliation releases the lock");
    assert_eq!(resolved.status, WorkspaceLeaseStatus::Released);
    fixture
        .store
        .record_provider_session(&codex_session, &binding_key, "active", None, None)
        .expect("replace the provider session after reconciliation");
    assert!(fixture
        .store
        .resolve_role_thread_capability(&capability.token)
        .is_err());
    let active_provider_sessions: i64 = Connection::open(fixture.store.database_path())
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM provider_sessions
             WHERE binding_key = ?1 AND status = 'active'",
            params![binding_key],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active_provider_sessions, 1);

    let connection = Connection::open(fixture.store.database_path()).unwrap();
    let claude_status: String = connection
        .query_row(
            "SELECT status FROM provider_sessions
             WHERE provider_id = 'claude' AND session_id = 'initial-claude-session'",
            [],
            |row| row.get(0),
        )
        .expect("keep the replaced Claude session in provider history");
    let codex_status: String = connection
        .query_row(
            "SELECT status FROM provider_sessions
             WHERE provider_id = 'codex' AND session_id = ?1",
            params![codex_session_id],
            |row| row.get(0),
        )
        .expect("persist the replacement Codex session");
    let (attempt_provider, attempt_session, attempt_status): (String, String, String) = connection
        .query_row(
            "SELECT provider_id, session_id, status FROM provider_attempts
             WHERE attempt_key = ?1",
            params![attempt_key.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("retain the reconciled Claude attempt");
    assert_eq!(claude_status, "inactive");
    assert_eq!(codex_status, "active");
    assert_eq!(attempt_provider, "claude");
    assert_eq!(attempt_session, "initial-claude-session");
    assert_eq!(attempt_status, "completed");
}

#[test]
fn provider_workspace_requires_matching_lease_and_verified_credential_source() {
    let fixture = fixture();
    let now = now();
    let lease = fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_one_id,
            "worker-session-one",
            &fixture.worker_one,
            WorkspaceAccess::WorkspaceWrite,
            now,
            60,
        )
        .expect("claim a clean linked worktree");
    let provider = ScriptedProvider::new(ProviderId::codex());
    let mut capabilities = provider.capabilities();
    capabilities.credential_source = CredentialSource::Subscription;
    let permissions = PermissionManifest {
        schema_version: 1,
        workspace_root: fixture.worker_one.clone(),
        access: WorkspaceAccess::WorkspaceWrite,
        allowed_scope: vec!["src/**".into()],
        network_access: true,
        allowed_credential_sources: vec![CredentialSource::Subscription],
    };
    let validated = fixture
        .store
        .validate_provider_workspace(lease.id, &permissions, &capabilities, now)
        .expect("current credential and exact workspace match");
    assert_eq!(validated.record.id, lease.id);
    assert_eq!(validated.credential_source, CredentialSource::Subscription);

    capabilities.credential_source = CredentialSource::Unknown;
    assert!(fixture
        .store
        .validate_provider_workspace(lease.id, &permissions, &capabilities, now)
        .unwrap_err()
        .contains("unknown"));
    capabilities.credential_source = CredentialSource::Subscription;
    permissions_with_wrong_path(&fixture, lease.id, &capabilities, now);
}

#[test]
fn provider_start_admission_requires_a_bound_lease_and_explicit_source() {
    let fixture = fixture();
    let now = now();
    let lease = fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_one_id,
            "worker-admission-test",
            &fixture.worker_one,
            WorkspaceAccess::WorkspaceWrite,
            now,
            60,
        )
        .expect("claim a clean linked worktree");
    let provider = ScriptedProvider::new(ProviderId::codex());
    let mut capabilities = provider.capabilities();
    capabilities.credential_source = CredentialSource::Subscription;
    let permissions = PermissionManifest {
        schema_version: 1,
        workspace_root: fixture.worker_one.clone(),
        access: WorkspaceAccess::WorkspaceWrite,
        allowed_scope: vec!["src/**".into()],
        network_access: true,
        allowed_credential_sources: vec![CredentialSource::Subscription],
    };
    let binding_key = format!("worker:{}", fixture.worker_one_id);
    let unbound = fixture
        .store
        .issue_role_thread_capability(
            &binding_key,
            "worker",
            fixture.project_id,
            fixture.worker_one_id,
        )
        .expect("issue a legacy unbound fixture capability");
    assert!(fixture
        .store
        .validate_role_provider_start(&unbound.token, lease.id, &permissions, &capabilities, now,)
        .unwrap_err()
        .contains("require an active workspace lease"));

    let bound = fixture
        .store
        .issue_role_thread_capability_for_workspace(
            &binding_key,
            "worker",
            fixture.project_id,
            fixture.worker_one_id,
            lease.id,
        )
        .expect("bind capability to exact active lease");
    let admission = fixture
        .store
        .validate_role_provider_start(&bound.token, lease.id, &permissions, &capabilities, now)
        .expect("admit exact lease and known allowed source");
    assert!(admission.is_admitted());

    capabilities.credential_source = CredentialSource::Unknown;
    assert!(fixture
        .store
        .validate_role_provider_start(&bound.token, lease.id, &permissions, &capabilities, now,)
        .unwrap_err()
        .contains("unknown"));
    capabilities.credential_source = CredentialSource::ApiKey;
    assert!(fixture
        .store
        .validate_role_provider_start(&bound.token, lease.id, &permissions, &capabilities, now,)
        .unwrap_err()
        .contains("not permitted"));
}

#[test]
fn provider_workspace_git_identity_failure_quarantines_lease_and_revokes_capability() {
    let fixture = fixture();
    let now = now();
    let lease = fixture
        .store
        .claim_delegation_workspace(
            fixture.worker_one_id,
            "worker-identity-mismatch-test",
            &fixture.worker_one,
            WorkspaceAccess::WorkspaceWrite,
            now,
            60,
        )
        .expect("claim a clean linked worktree");
    let binding_key = format!("worker:{}", fixture.worker_one_id);
    let capability = fixture
        .store
        .issue_role_thread_capability_for_workspace(
            &binding_key,
            "worker",
            fixture.project_id,
            fixture.worker_one_id,
            lease.id,
        )
        .expect("bind capability to exact active lease");
    let provider = ScriptedProvider::new(ProviderId::codex());
    let mut capabilities = provider.capabilities();
    capabilities.credential_source = CredentialSource::Subscription;
    let permissions = PermissionManifest {
        schema_version: 1,
        workspace_root: fixture.worker_one.clone(),
        access: WorkspaceAccess::WorkspaceWrite,
        allowed_scope: vec!["src/**".into()],
        network_access: true,
        allowed_credential_sources: vec![CredentialSource::Subscription],
    };
    fs::write(
        fixture.worker_one.join(".git"),
        "gitdir: missing-worktree-metadata\n",
    )
    .expect("change linked worktree identity");

    assert!(fixture
        .store
        .validate_role_provider_start(
            &capability.token,
            lease.id,
            &permissions,
            &capabilities,
            now,
        )
        .unwrap_err()
        .contains("verification failed during provider admission"));
    let connection = Connection::open(fixture.store.database_path()).unwrap();
    let status: String = connection
        .query_row(
            "SELECT status FROM workspace_leases WHERE id = ?1",
            params![lease.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "needs_reconcile");
    assert!(fixture
        .store
        .resolve_role_thread_capability(&capability.token)
        .unwrap_err()
        .contains("revoked"));
}

#[test]
fn provider_workspace_identity_hash_mismatch_has_a_distinct_quarantine_reason() {
    let fixture = fixture();
    let now = now();
    let binding_key = format!("project-manager:{}", fixture.project_id);
    let connection = Connection::open(fixture.store.database_path()).unwrap();
    connection
        .execute(
            "INSERT INTO workspace_leases
             (project_id, delegation_id, role_kind, role_binding_key, access, owner_id,
              workspace_path, workspace_path_hash, common_dir_hash, base_commit, status,
              lease_expires_at, created_at, updated_at)
             VALUES (?1, ?2, 'sub_overseer', ?3, 'read_only', 'identity-mismatch-test',
                     ?4, ?5, ?6, 'abcdef0', 'active', ?7, ?8, ?8)",
            params![
                fixture.project_id,
                fixture.manager_id,
                binding_key,
                fixture.repo.to_string_lossy(),
                "0".repeat(64),
                "f".repeat(64),
                now + 3600,
                now
            ],
        )
        .expect("insert isolated lease with validly shaped but stale identity hashes");
    let lease_id = connection.last_insert_rowid();
    drop(connection);

    let provider = ScriptedProvider::new(ProviderId::codex());
    let mut capabilities = provider.capabilities();
    capabilities.credential_source = CredentialSource::Subscription;
    let permissions = PermissionManifest {
        schema_version: 1,
        workspace_root: fixture.repo.clone(),
        access: WorkspaceAccess::ReadOnly,
        allowed_scope: vec![],
        network_access: true,
        allowed_credential_sources: vec![CredentialSource::Subscription],
    };

    assert!(fixture
        .store
        .validate_provider_workspace(lease_id, &permissions, &capabilities, now)
        .unwrap_err()
        .contains("identity changed during provider admission"));

    let connection = Connection::open(fixture.store.database_path()).unwrap();
    let (status, details): (String, String) = connection
        .query_row(
            "SELECT lease.status, event.details_json
             FROM workspace_leases lease
             JOIN workspace_lease_events event ON event.lease_id = lease.id
             WHERE lease.id = ?1 AND event.event_type = 'needs_reconcile'
             ORDER BY event.id DESC LIMIT 1",
            params![lease_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "needs_reconcile");
    assert!(details.contains("identity changed during provider admission"));
}

#[test]
fn role_capability_is_revoked_with_its_workspace_lease() {
    let fixture = fixture();
    let current_time = now();
    let capability = fixture
        .store
        .issue_role_thread_capability(
            &format!("project-manager:{}", fixture.project_id),
            "sub_overseer",
            fixture.project_id,
            fixture.manager_id,
        )
        .expect("issue coordinator capability");
    let lease = fixture
        .store
        .claim_delegation_workspace(
            fixture.manager_id,
            "manager-session",
            &fixture.repo,
            WorkspaceAccess::ReadOnly,
            current_time,
            60,
        )
        .expect("claim read-only coordinator lease");
    fixture
        .store
        .bind_role_thread_capability_to_workspace_lease(&capability.token, lease.id)
        .expect("bind capability to exact lease");
    assert_eq!(
        fixture
            .store
            .resolve_role_thread_capability(&capability.token)
            .unwrap()
            .workspace_lease_id,
        Some(lease.id)
    );
    fixture
        .store
        .release_workspace_lease(
            lease.id,
            "manager-session",
            current_time + 1,
            "review finished",
        )
        .expect("release coordinator lease");
    assert!(fixture
        .store
        .resolve_role_thread_capability(&capability.token)
        .unwrap_err()
        .contains("revoked"));
}

fn permissions_with_wrong_path(
    fixture: &Fixture,
    lease_id: i64,
    capabilities: &cathulu_core::agent_provider::ProviderCapabilities,
    now: i64,
) {
    let permissions = PermissionManifest {
        schema_version: 1,
        workspace_root: fixture.repo.clone(),
        access: WorkspaceAccess::ReadOnly,
        allowed_scope: vec!["src/**".into()],
        network_access: true,
        allowed_credential_sources: vec![CredentialSource::Subscription],
    };
    assert!(fixture
        .store
        .validate_provider_workspace(lease_id, &permissions, capabilities, now)
        .unwrap_err()
        .contains("does not match the exact leased path"));
}
