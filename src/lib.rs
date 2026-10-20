pub mod mcp;
mod sqlite;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Clone, Debug)]
pub struct Store {
    database_path: PathBuf,
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
        };
        store.initialize()?;
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

    pub fn register_project(&self, input_path: &Path) -> Result<ProjectSnapshot, String> {
        let git = inspect_git_repository(input_path)?;
        let now = unix_time();
        let connection = self.connection()?;
        connection.execute("BEGIN IMMEDIATE")?;
        let result = (|| {
            let path = sql_string(&git.path)?;
            let name = sql_string(&git.name)?;
            let branch = sql_optional_string(git.branch.as_deref())?;
            let head = sql_optional_string(git.head.as_deref())?;
            let changed_paths = sql_string(
                &serde_json::to_string(&git.changed_paths).map_err(|error| error.to_string())?,
            )?;
            let dirty = if git.changed_paths.is_empty() { 0 } else { 1 };
            connection.execute(&format!(
                "INSERT INTO projects \
                 (name, path, git_branch, git_head, git_dirty, changed_paths_json, registered_at, reconciled_at) \
                 VALUES ({name}, {path}, {branch}, {head}, {dirty}, {changed_paths}, {now}, {now}) \
                 ON CONFLICT(path) DO UPDATE SET \
                 name = excluded.name, git_branch = excluded.git_branch, git_head = excluded.git_head, \
                 git_dirty = excluded.git_dirty, changed_paths_json = excluded.changed_paths_json, \
                 reconciled_at = excluded.reconciled_at"
            ))?;
            let rows = connection.query(&format!("SELECT id FROM projects WHERE path = {path}"))?;
            let id = row_i64(rows.first(), 0)?;
            connection.execute("COMMIT")?;
            self.get_project(id)?
                .ok_or_else(|| "registered project disappeared from the database".to_string())
        })();
        if result.is_err() {
            let _ = connection.execute("ROLLBACK");
        }
        result
    }

    pub fn reconcile_project(&self, id: i64) -> Result<ProjectSnapshot, String> {
        let existing = self
            .get_project(id)?
            .ok_or_else(|| format!("project {id} is not registered"))?;
        let git = inspect_git_repository(Path::new(&existing.path))?;
        let now = unix_time();
        let connection = self.connection()?;
        let branch = sql_optional_string(git.branch.as_deref())?;
        let head = sql_optional_string(git.head.as_deref())?;
        let changed_paths = sql_string(
            &serde_json::to_string(&git.changed_paths).map_err(|error| error.to_string())?,
        )?;
        let dirty = if git.changed_paths.is_empty() { 0 } else { 1 };
        connection.execute(&format!(
            "UPDATE projects SET name = {}, git_branch = {branch}, git_head = {head}, \
             git_dirty = {dirty}, changed_paths_json = {changed_paths}, reconciled_at = {now} \
             WHERE id = {id}",
            sql_string(&git.name)?
        ))?;
        self.get_project(id)?
            .ok_or_else(|| format!("project {id} disappeared during reconciliation"))
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
        Ok(())
    }

    fn connection(&self) -> Result<sqlite::Connection, String> {
        sqlite::Connection::open(&self.database_path)
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
    let output = Command::new("git")
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
    let output = Command::new("git")
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

fn sql_string(value: &str) -> Result<String, String> {
    if value.contains('\0') {
        return Err("SQLite text values cannot contain a NUL character".to_string());
    }
    Ok(format!("'{}'", value.replace('\'', "''")))
}

fn sql_optional_string(value: Option<&str>) -> Result<String, String> {
    value
        .map(sql_string)
        .transpose()
        .map(|value| value.unwrap_or_else(|| "NULL".to_string()))
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
