pub mod app_server;
pub mod codex_config;
pub mod codex_config_files;
pub mod context_extraction;
pub mod dashboard;
pub mod delegation;
pub mod dispatcher;
mod events;
pub mod gardener;
pub mod graphify;
mod hash;
pub mod mcp;
pub mod memory;
mod migrations;
pub mod optimization;
pub mod provider_adapters;
mod provider_credentials;
pub mod roles;
mod sqlite;
pub mod telemetry;
pub mod terminal;
pub mod workflow;
pub mod worktree;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Prevent background CLI children from opening a console window on Windows.
#[cfg(windows)]
pub fn suppress_windows_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn suppress_windows_console(_: &mut Command) {}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub dirty: bool,
    pub changed_paths: Vec<String>,
    pub registered_at: i64,
    pub reconciled_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationFailure {
    pub project_id: i64,
    pub message: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationReport {
    pub checked: usize,
    pub refreshed: usize,
    pub failures: Vec<ReconciliationFailure>,
}

#[derive(Clone, Debug)]
pub struct Store {
    database_path: PathBuf,
    read_only: bool,
}

pub fn default_data_dir() -> Result<PathBuf, String> {
    std::env::var_os("CATHULU_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("Cathulu"))
        })
        .ok_or_else(|| {
            "CATHULU_DATA_DIR or LOCALAPPDATA must identify Cathulu's data directory".to_string()
        })
}

impl Store {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let data_dir = data_dir.as_ref();
        fs::create_dir_all(data_dir)
            .map_err(|error| format!("cannot create Cathulu data directory: {error}"))?;
        let store = Self {
            database_path: data_dir.join("cathulu.sqlite3"),
            read_only: false,
        };
        store.initialize()?;
        Ok(store)
    }

    /// Open an existing store for observation without migrations or writes.
    pub fn open_existing_read_only(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let database_path = data_dir.as_ref().join("cathulu.sqlite3");
        if !database_path.is_file() {
            return Err(format!(
                "Cathulu store does not exist at {}",
                database_path.display()
            ));
        }
        let store = Self {
            database_path,
            read_only: true,
        };
        store.connection()?;
        Ok(store)
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSnapshot>, String> {
        let connection = self.connection()?;
        connection
            .query(
                "SELECT id, name, path, git_branch, git_head, git_dirty, \
                 changed_paths_json, registered_at, reconciled_at \
                 FROM projects ORDER BY name COLLATE NOCASE, path",
            )?
            .iter()
            .map(project_from_row)
            .collect()
    }

    pub fn get_project(&self, id: i64) -> Result<Option<ProjectSnapshot>, String> {
        let connection = self.connection()?;
        let rows = connection.query(&format!(
            "SELECT id, name, path, git_branch, git_head, git_dirty, \
             changed_paths_json, registered_at, reconciled_at \
             FROM projects WHERE id = {id}"
        ))?;
        rows.first().map(project_from_row).transpose()
    }

    pub fn codex_selector_preferences(
        &self,
    ) -> Result<Vec<codex_config::CodexAgentPreference>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .raw()
            .prepare(
                "SELECT role_id, role_kind, agent_name, model_slug, reasoning_effort,
                        catalog_version, last_applied_target, last_applied_at,
                        last_applied_config_hash, updated_at
                 FROM codex_selector_preferences ORDER BY role_kind, role_id",
            )
            .map_err(|error| format!("cannot read Codex selector preferences: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })
            .map_err(|error| format!("cannot query Codex selector preferences: {error}"))?;
        let mut preferences = Vec::new();
        for row in rows {
            let (
                role_id,
                role_kind,
                agent_name,
                model_slug,
                reasoning_effort,
                catalog_version,
                last_applied_target,
                last_applied_at,
                last_applied_config_hash,
                updated_at,
            ) = row.map_err(|error| format!("cannot read Codex selector preference: {error}"))?;
            let role_kind = match role_kind.as_str() {
                "main_overseer" => codex_config::CodexRoleKind::MainOverseer,
                "subagent_default" => codex_config::CodexRoleKind::SubagentDefault,
                "named_agent" => codex_config::CodexRoleKind::NamedAgent,
                _ => return Err("database contains an unknown Codex selector role".to_string()),
            };
            preferences.push(codex_config::CodexAgentPreference {
                role_id,
                role_kind,
                agent_name,
                model_slug,
                reasoning_effort,
                catalog_version,
                last_applied_target,
                last_applied_at,
                last_applied_config_hash,
                updated_at,
            });
        }
        Ok(preferences)
    }

    pub fn save_codex_selector_preference(
        &self,
        input: &codex_config::CodexAgentPreferenceInput,
        catalog: &codex_config::CodexModelCatalog,
    ) -> Result<codex_config::CodexAgentPreference, String> {
        let (role_id, agent_name) = input.validate(catalog)?;
        let updated_at = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction()
            .map_err(|error| format!("cannot begin Codex preference update: {error}"))?;
        transaction
            .execute(
                "INSERT INTO codex_selector_preferences
                 (role_id, role_kind, agent_name, model_slug, reasoning_effort, catalog_version, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(role_id) DO UPDATE SET
                   role_kind = excluded.role_kind, agent_name = excluded.agent_name,
                   model_slug = excluded.model_slug, reasoning_effort = excluded.reasoning_effort,
                   catalog_version = excluded.catalog_version,
                   last_applied_target = NULL, last_applied_at = NULL,
                   last_applied_config_hash = NULL, updated_at = excluded.updated_at",
                params![
                    role_id,
                    input.role_kind.as_str(),
                    agent_name,
                    input.model_slug,
                    input.reasoning_effort,
                    catalog.cli_version,
                    updated_at,
                ],
            )
            .map_err(|error| format!("cannot save Codex selector preference: {error}"))?;
        crate::events::append_event(
            &transaction,
            "codex_selector_preference",
            &role_id,
            "preference_stored",
            &serde_json::json!({
                "roleKind": input.role_kind.as_str(),
                "agentName": agent_name,
                "modelSlug": input.model_slug,
                "reasoningEffort": input.reasoning_effort,
                "catalogVersion": catalog.cli_version,
                "configurationState": "stored_in_cathulu"
            }),
            updated_at,
        )?;
        transaction
            .commit()
            .map_err(|error| format!("cannot commit Codex selector preference: {error}"))?;
        self.codex_selector_preferences()?
            .into_iter()
            .find(|preference| preference.role_id == role_id)
            .ok_or_else(|| "saved Codex selector preference could not be read back".to_string())
    }

    pub fn mark_codex_selector_preferences_applied(
        &self,
        role_ids: &[String],
        target: &str,
        config_hash: &str,
    ) -> Result<(), String> {
        if role_ids.is_empty()
            || target.is_empty()
            || target.len() > 256
            || config_hash.len() != 64
            || !config_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("applied Codex selector status has invalid metadata".to_string());
        }
        let applied_at = unix_time();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction()
            .map_err(|error| format!("cannot begin Codex applied-status update: {error}"))?;
        for role_id in role_ids {
            let updated = transaction
                .execute(
                    "UPDATE codex_selector_preferences
                     SET last_applied_target = ?1, last_applied_at = ?2,
                         last_applied_config_hash = ?3
                     WHERE role_id = ?4",
                    params![target, applied_at, config_hash, role_id],
                )
                .map_err(|error| format!("cannot record applied Codex selector: {error}"))?;
            if updated != 1 {
                return Err("applied Codex selector preference no longer exists".to_string());
            }
            crate::events::append_event(
                &transaction,
                "codex_selector_preference",
                role_id,
                "preference_written_to_config",
                &serde_json::json!({
                    "target": target,
                    "configHash": config_hash,
                    "runtimeEffectivenessVerified": false
                }),
                applied_at,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| format!("cannot commit Codex applied-status update: {error}"))
    }

    pub fn register_project(&self, input_path: &Path) -> Result<ProjectSnapshot, String> {
        let git = inspect_git_repository(input_path)?;
        let now = unix_time();
        let changed_paths = serde_json::to_string(&git.changed_paths)
            .map_err(|error| format!("cannot serialize changed paths: {error}"))?;
        let dirty = !git.changed_paths.is_empty();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction()
            .map_err(|error| format!("cannot begin project registration: {error}"))?;
        let existed = transaction
            .query_row(
                "SELECT id FROM projects WHERE path = ?1",
                params![git.path],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| format!("cannot inspect existing project: {error}"))?
            .is_some();
        transaction
            .execute(
                "INSERT INTO projects
                 (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(path) DO UPDATE SET
                   name = excluded.name, git_branch = excluded.git_branch, git_head = excluded.git_head,
                   git_dirty = excluded.git_dirty, changed_paths_json = excluded.changed_paths_json,
                   reconciled_at = excluded.reconciled_at",
                params![git.name, git.path, git.branch, git.head, dirty, changed_paths, now],
            )
            .map_err(|error| format!("cannot save project registration: {error}"))?;
        let id = transaction
            .query_row(
                "SELECT id FROM projects WHERE path = ?1",
                params![git.path],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| format!("cannot read registered project id: {error}"))?;
        let invalidated =
            Self::invalidate_project_memories(&transaction, id, git.head.as_deref(), now)?;
        Self::invalidate_graphify_if_stale(
            &transaction,
            id,
            Path::new(&git.path),
            git.head.as_deref(),
            &git.changed_paths,
            now,
        )?;
        crate::events::append_event(
            &transaction,
            "project",
            id,
            if existed {
                "project_reconciled"
            } else {
                "project_registered"
            },
            &serde_json::json!({
                "head": git.head,
                "branch": git.branch,
                "dirty": dirty,
                "changedPathCount": git.changed_paths.len()
            }),
            now,
        )?;
        for memory_id in invalidated {
            crate::events::append_event(
                &transaction,
                "memory",
                memory_id,
                "memory_invalidated",
                &serde_json::json!({ "reason": "git_commit_changed", "repoId": id }),
                now,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| format!("cannot commit project registration: {error}"))?;
        self.get_project(id)?
            .ok_or_else(|| "registered project disappeared from the database".to_string())
    }

    pub fn reconcile_project(&self, id: i64) -> Result<ProjectSnapshot, String> {
        let existing = self
            .get_project(id)?
            .ok_or_else(|| format!("project {id} is not registered"))?;
        let git = inspect_git_repository(Path::new(&existing.path))?;
        let now = unix_time();
        let changed_paths = serde_json::to_string(&git.changed_paths)
            .map_err(|error| format!("cannot serialize changed paths: {error}"))?;
        let dirty = !git.changed_paths.is_empty();
        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction()
            .map_err(|error| format!("cannot begin project reconciliation: {error}"))?;
        transaction
            .execute(
                "UPDATE projects SET name = ?1, git_branch = ?2, git_head = ?3,
                   git_dirty = ?4, changed_paths_json = ?5, reconciled_at = ?6 WHERE id = ?7",
                params![
                    git.name,
                    git.branch,
                    git.head,
                    dirty,
                    changed_paths,
                    now,
                    id
                ],
            )
            .map_err(|error| format!("cannot update reconciled project: {error}"))?;
        let invalidated =
            Self::invalidate_project_memories(&transaction, id, git.head.as_deref(), now)?;
        Self::invalidate_graphify_if_stale(
            &transaction,
            id,
            Path::new(&git.path),
            git.head.as_deref(),
            &git.changed_paths,
            now,
        )?;
        crate::events::append_event(
            &transaction,
            "project",
            id,
            "project_reconciled",
            &serde_json::json!({
                "head": git.head,
                "branch": git.branch,
                "dirty": dirty,
                "changedPathCount": git.changed_paths.len()
            }),
            now,
        )?;
        for memory_id in invalidated {
            crate::events::append_event(
                &transaction,
                "memory",
                memory_id,
                "memory_invalidated",
                &serde_json::json!({ "reason": "git_commit_changed", "repoId": id }),
                now,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| format!("cannot commit project reconciliation: {error}"))?;
        self.get_project(id)?
            .ok_or_else(|| format!("project {id} disappeared during reconciliation"))
    }

    pub fn reconcile_registered_projects(&self) -> Result<ReconciliationReport, String> {
        let projects = self.list_projects()?;
        let mut report = ReconciliationReport {
            checked: projects.len(),
            ..ReconciliationReport::default()
        };
        for project in projects {
            match self.reconcile_project(project.id) {
                Ok(_) => report.refreshed += 1,
                Err(message) => report.failures.push(ReconciliationFailure {
                    project_id: project.id,
                    message,
                }),
            }
        }
        Ok(report)
    }

    pub(crate) fn registered_project_id_for_path(
        &self,
        input_path: &Path,
    ) -> Result<Option<i64>, String> {
        let Ok(canonical_input) = input_path.canonicalize() else {
            return Ok(None);
        };
        if !canonical_input.is_dir() {
            return Ok(None);
        }
        let Ok(root_output) = git(&canonical_input, &["rev-parse", "--show-toplevel"]) else {
            return Ok(None);
        };
        let root_text = String::from_utf8_lossy(&root_output).trim().to_string();
        let root = PathBuf::from(root_text)
            .canonicalize()
            .map_err(|_| "Git repository root could not be resolved".to_string())?;
        let root_path = display_path(&root);
        self.connection()?
            .raw()
            .query_row(
                "SELECT id FROM projects WHERE path = ?1",
                params![root_path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| {
                format!("cannot match hook directory to a registered project: {error}")
            })
    }

    fn initialize(&self) -> Result<(), String> {
        let connection = self.connection()?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations \
             (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL)",
        )?;
        let rows = connection
            .query("SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1")?;
        let version = rows
            .first()
            .map(|row| row_i64(Some(row), 0))
            .transpose()?
            .unwrap_or(0);
        if version > 18 {
            return Err(format!(
                "Cathulu database schema version {version} is newer than this application supports"
            ));
        }
        if version < 1 {
            connection.execute("BEGIN IMMEDIATE")?;
            let migration = connection.execute(
                "CREATE TABLE projects (\
                   id INTEGER PRIMARY KEY AUTOINCREMENT,\
                   name TEXT NOT NULL,\
                   path TEXT NOT NULL UNIQUE,\
                   git_branch TEXT,\
                   git_head TEXT,\
                   git_dirty INTEGER NOT NULL DEFAULT 0,\
                   changed_paths_json TEXT NOT NULL DEFAULT '[]',\
                   registered_at INTEGER NOT NULL,\
                   reconciled_at INTEGER NOT NULL\
                 ); \
                 INSERT INTO schema_migrations(version, applied_at) VALUES (1, \
                   CAST(strftime('%s','now') AS INTEGER));",
            );
            if let Err(error) = migration {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 2 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_2) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 3 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_3) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 4 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_4) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 5 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_5) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 6 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_6) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 7 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_7) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 8 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_8) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 9 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_9) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 10 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_10) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 11 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_11) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 12 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_12) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 13 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_13) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 14 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_14) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 15 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_15) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 16 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_16) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 17 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_17) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        if version < 18 {
            connection.execute("BEGIN IMMEDIATE")?;
            if let Err(error) = connection.execute(migrations::VERSION_18) {
                let _ = connection.execute("ROLLBACK");
                return Err(error);
            }
            connection.execute("COMMIT")?;
        }
        Ok(())
    }

    pub(crate) fn connection(&self) -> Result<sqlite::Connection, String> {
        if self.read_only {
            sqlite::Connection::open_read_only(&self.database_path)
        } else {
            sqlite::Connection::open(&self.database_path)
        }
    }
}

#[derive(Debug)]
struct GitSnapshot {
    path: String,
    name: String,
    branch: Option<String>,
    head: Option<String>,
    changed_paths: Vec<String>,
}

fn inspect_git_repository(input_path: &Path) -> Result<GitSnapshot, String> {
    let canonical_input = input_path
        .canonicalize()
        .map_err(|error| format!("repository path cannot be opened: {error}"))?;
    if !canonical_input.is_dir() {
        return Err("repository path is not a directory".to_string());
    }
    let root_output = git(&canonical_input, &["rev-parse", "--show-toplevel"])?;
    let root_text = String::from_utf8_lossy(&root_output).trim().to_string();
    let git_root = PathBuf::from(root_text);
    let root = git_root
        .canonicalize()
        .map_err(|error| format!("Git repository root cannot be resolved: {error}"))?;
    let branch_output = git_optional(&git_root, &["branch", "--show-current"])?;
    let branch = branch_output
        .map(|output| String::from_utf8_lossy(&output).trim().to_string())
        .filter(|value| !value.is_empty());
    let head_output = git_optional(&git_root, &["rev-parse", "--verify", "HEAD"])?;
    let head = head_output
        .map(|output| String::from_utf8_lossy(&output).trim().to_string())
        .filter(|value| !value.is_empty());
    let status = git(
        &git_root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let changed_paths = parse_changed_paths(&status);
    let name = root
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    Ok(GitSnapshot {
        path: display_path(&root),
        name,
        branch,
        head,
        changed_paths,
    })
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(unc_path) = value.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc_path}");
        }
        if let Some(normal_path) = value.strip_prefix(r"\\?\") {
            return normal_path.to_string();
        }
    }
    value.into_owned()
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    suppress_windows_console(&mut command);
    let output = command
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| format!("cannot launch Git: {error}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if message.is_empty() {
            format!("Git command {:?} failed with {}", args, output.status)
        } else {
            message
        });
    }
    Ok(output.stdout)
}

fn git_optional(repo: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, String> {
    let mut command = Command::new("git");
    suppress_windows_console(&mut command);
    let output = command
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|error| format!("cannot launch Git: {error}"))?;
    if output.status.success() {
        Ok(Some(output.stdout))
    } else {
        Ok(None)
    }
}

fn parse_changed_paths(output: &[u8]) -> Vec<String> {
    let fields = output.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut paths = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let record = fields[index];
        index += 1;
        if record.len() < 4 {
            continue;
        }
        let status = &record[..2];
        paths.push(String::from_utf8_lossy(&record[3..]).into_owned());
        if status.contains(&b'R') || status.contains(&b'C') {
            index += 1;
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn project_from_row(row: &Vec<Option<String>>) -> Result<ProjectSnapshot, String> {
    let changed_paths = row
        .get(6)
        .and_then(Option::as_deref)
        .ok_or_else(|| "database returned no changed path list".to_string())?;
    Ok(ProjectSnapshot {
        id: row_i64(Some(row), 0)?,
        name: row_string(row, 1)?,
        path: row_string(row, 2)?,
        branch: row.get(3).cloned().flatten(),
        head: row.get(4).cloned().flatten(),
        dirty: row_i64(Some(row), 5)? != 0,
        changed_paths: serde_json::from_str(changed_paths)
            .map_err(|error| format!("stored changed paths are invalid: {error}"))?,
        registered_at: row_i64(Some(row), 7)?,
        reconciled_at: row_i64(Some(row), 8)?,
    })
}

fn row_string(row: &Vec<Option<String>>, index: usize) -> Result<String, String> {
    row.get(index)
        .cloned()
        .flatten()
        .ok_or_else(|| format!("database row is missing column {index}"))
}

fn row_i64(row: Option<&Vec<Option<String>>>, index: usize) -> Result<i64, String> {
    row.and_then(|row| row.get(index))
        .and_then(Option::as_deref)
        .ok_or_else(|| format!("database row is missing integer column {index}"))?
        .parse::<i64>()
        .map_err(|error| format!("database integer column {index} is invalid: {error}"))
}

fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::parse_changed_paths;

    #[test]
    fn parses_porcelain_paths_without_treating_header_as_a_path() {
        let output = b"?? new.txt\0 M changed.txt\0R  moved.txt\0old.txt\0";
        assert_eq!(
            parse_changed_paths(output),
            vec!["changed.txt", "moved.txt", "new.txt"]
        );
    }
}
