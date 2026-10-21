use crate::{events::append_event, hash::content_hash, Store};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeReportVerification {
    pub status: String,
    pub verified_commits: Vec<String>,
    pub unverified_commits: Vec<String>,
    pub verified_paths: Vec<String>,
    pub unverified_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DelegationWorktreeState {
    pub worktree_path_hash: Option<String>,
    pub branch: Option<String>,
    pub base_commit: Option<String>,
    pub observed_head: Option<String>,
    pub is_clean: Option<bool>,
    pub changed_paths: Vec<String>,
    pub report_verification: Option<WorktreeReportVerification>,
    pub last_reconciled_at: Option<i64>,
}

#[derive(Debug)]
struct GitState {
    path: PathBuf,
    branch: Option<String>,
    head: Option<String>,
    clean: bool,
    changed_paths: Vec<String>,
}

impl Store {
    pub fn reconcile_delegation_worktree(
        &self,
        delegation_id: i64,
        worktree_path: &Path,
    ) -> Result<DelegationWorktreeState, String> {
        if delegation_id <= 0 {
            return Err("delegation ID must be positive".to_string());
        }
        let (project_path, base_commit, report_json) = {
            let connection = self.connection()?;
            connection
                .raw()
                .query_row(
                    "SELECT p.path, state.base_commit, report.report_json
                     FROM delegations d
                     JOIN projects p ON p.id = d.project_id
                     LEFT JOIN delegation_worktree_states state ON state.delegation_id = d.id
                     LEFT JOIN worker_reports report ON report.delegation_id = d.id
                     WHERE d.id = ?1",
                    params![delegation_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(sqlite_error)?
                .ok_or_else(|| format!("delegation {delegation_id} was not found"))?
        };

        let registered_path = PathBuf::from(project_path)
            .canonicalize()
            .map_err(|error| format!("registered project path is unavailable: {error}"))?;
        let registered_root = git_text(&registered_path, &["rev-parse", "--show-toplevel"])?;
        let registered_root = canonical_path(PathBuf::from(registered_root.trim()))?;
        if !same_path(&registered_root, &registered_path) {
            return Err("registered project path no longer resolves to its Git root".to_string());
        }

        let requested = worktree_path
            .canonicalize()
            .map_err(|error| format!("worktree path is unavailable: {error}"))?;
        if !requested.is_dir() {
            return Err("worktree path is not a directory".to_string());
        }
        let worktree_root = git_text(&requested, &["rev-parse", "--show-toplevel"])?;
        let worktree_root = canonical_path(PathBuf::from(worktree_root.trim()))?;
        if !listed_worktree(&registered_root, &worktree_root)? {
            return Err("path is not an exact Git worktree registered to this project".to_string());
        }
        if same_path(&registered_root, &worktree_root) {
            return Err("delegation reconciliation requires a linked worktree distinct from the registered project root".to_string());
        }
        let registered_common = common_dir(&registered_root)?;
        let worktree_common = common_dir(&worktree_root)?;
        if !same_path(&registered_common, &worktree_common) {
            return Err(
                "worktree does not share the registered project's Git common directory".to_string(),
            );
        }

        let observed = inspect_worktree(&worktree_root, base_commit.as_deref())?;
        let verification = verify_report(
            &worktree_root,
            base_commit.as_deref(),
            observed.head.as_deref(),
            &observed.changed_paths,
            report_json.as_deref(),
        );
        let now = unix_time();
        let path = display_path(&observed.path);
        let path_hash = content_hash(&format!("delegation-worktree-path-v1\0{path}"));
        let changed_json = serde_json::to_string(&observed.changed_paths)
            .map_err(|error| format!("cannot serialize Git changed paths: {error}"))?;
        let verification_json = serde_json::to_string(&verification)
            .map_err(|error| format!("cannot serialize worker report verification: {error}"))?;

        let mut connection = self.connection()?;
        let transaction = connection
            .raw_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(sqlite_error)?;
        let exists = transaction
            .query_row(
                "SELECT 1 FROM delegation_worktree_states WHERE delegation_id = ?1",
                params![delegation_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(sqlite_error)?
            .is_some();
        if exists {
            transaction
                .execute(
                    "UPDATE delegation_worktree_states SET worktree_path = ?1, worktree_path_hash = ?2,
                     branch = ?3, observed_head = ?4, is_clean = ?5, changed_paths_json = ?6,
                     report_verification_json = ?7, last_reconciled_at = ?8 WHERE delegation_id = ?9",
                    params![
                        path,
                        path_hash,
                        observed.branch,
                        observed.head,
                        observed.clean,
                        changed_json,
                        verification_json,
                        now,
                        delegation_id
                    ],
                )
                .map_err(sqlite_error)?;
        } else {
            transaction
                .execute(
                    "INSERT INTO delegation_worktree_states
                     (delegation_id, base_commit, worktree_path, worktree_path_hash, branch, observed_head,
                      is_clean, changed_paths_json, report_verification_json, last_reconciled_at, created_at)
                     VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                    params![
                        delegation_id,
                        path,
                        path_hash,
                        observed.branch,
                        observed.head,
                        observed.clean,
                        changed_json,
                        verification_json,
                        now
                    ],
                )
                .map_err(sqlite_error)?;
        }
        append_event(
            &transaction,
            "delegation",
            delegation_id,
            "delegation_worktree_reconciled",
            &json!({
                "worktreePathHash": path_hash,
                "branch": observed.branch,
                "observedHead": observed.head,
                "isClean": observed.clean,
                "changedPathCount": observed.changed_paths.len(),
                "reportVerification": verification.status,
            }),
            now,
        )?;
        transaction.commit().map_err(sqlite_error)?;

        Ok(DelegationWorktreeState {
            worktree_path_hash: Some(path_hash),
            branch: observed.branch,
            base_commit,
            observed_head: observed.head,
            is_clean: Some(observed.clean),
            changed_paths: observed.changed_paths,
            report_verification: Some(verification),
            last_reconciled_at: Some(now),
        })
    }

    pub(crate) fn delegation_worktree_state(
        &self,
        delegation_id: i64,
    ) -> Result<Option<DelegationWorktreeState>, String> {
        let connection = self.connection()?;
        let row = connection
            .raw()
            .query_row(
                "SELECT worktree_path_hash, branch, base_commit, observed_head, is_clean,
                        changed_paths_json, report_verification_json, last_reconciled_at
                 FROM delegation_worktree_states WHERE delegation_id = ?1",
                params![delegation_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<bool>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(sqlite_error)?;
        row.map(
            |(path_hash, branch, base, head, clean, changed, verification, reconciled_at)| {
                Ok(DelegationWorktreeState {
                    worktree_path_hash: path_hash,
                    branch,
                    base_commit: base,
                    observed_head: head,
                    is_clean: clean,
                    changed_paths: changed
                        .map(|changed| serde_json::from_str(&changed))
                        .transpose()
                        .map_err(|error| format!("stored Git changed paths are invalid: {error}"))?
                        .unwrap_or_default(),
                    report_verification: verification
                        .map(|verification| serde_json::from_str(&verification))
                        .transpose()
                        .map_err(|error| {
                            format!("stored report verification is invalid: {error}")
                        })?,
                    last_reconciled_at: reconciled_at,
                })
            },
        )
        .transpose()
    }
}

fn inspect_worktree(root: &Path, base: Option<&str>) -> Result<GitState, String> {
    let branch = git_optional(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
        .filter(|value| !value.is_empty());
    let head = git_optional(root, &["rev-parse", "--verify", "HEAD"])?
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
        .filter(|value| !value.is_empty());
    let status = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let clean = status.is_empty();
    let mut changed = BTreeSet::new();
    if let Some(base) = base {
        let output = git(root, &["diff", base, "--name-only", "-z", "--"])?;
        add_nul_paths(&mut changed, &output);
    }
    let untracked = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    add_nul_paths(&mut changed, &untracked);
    if base.is_none() {
        add_status_paths(&mut changed, &status);
    }
    Ok(GitState {
        path: root.to_path_buf(),
        branch,
        head,
        clean,
        changed_paths: changed.into_iter().collect(),
    })
}

fn verify_report(
    root: &Path,
    base: Option<&str>,
    head: Option<&str>,
    changed_paths: &[String],
    report_json: Option<&str>,
) -> WorktreeReportVerification {
    let mut result = WorktreeReportVerification {
        status: "no_report".to_string(),
        verified_commits: Vec::new(),
        unverified_commits: Vec::new(),
        verified_paths: Vec::new(),
        unverified_paths: Vec::new(),
    };
    let Some(report_json) = report_json else {
        return result;
    };
    let Ok(report) = serde_json::from_str::<Value>(report_json) else {
        result.status = "unavailable".to_string();
        return result;
    };
    let commits = report
        .get("commits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let paths = report
        .get("changedFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(normalize_report_path)
        .collect::<Vec<_>>();
    if commits.is_empty() && paths.is_empty() {
        result.status = if base.is_some() && head.is_some() {
            "no_claims"
        } else {
            "unavailable"
        }
        .to_string();
        return result;
    }
    let (Some(base), Some(head)) = (base, head) else {
        result.status = "unavailable".to_string();
        result.unverified_commits = commits;
        result.unverified_paths = paths;
        return result;
    };
    if !is_commit(root, base) || !is_commit(root, head) || !is_ancestor(root, base, head) {
        result.status = "unavailable".to_string();
        result.unverified_commits = commits;
        result.unverified_paths = paths;
        return result;
    }
    for commit in commits {
        if is_commit(root, &commit)
            && is_ancestor(root, base, &commit)
            && is_ancestor(root, &commit, head)
        {
            result.verified_commits.push(commit);
        } else {
            result.unverified_commits.push(commit);
        }
    }
    let changed = changed_paths.iter().cloned().collect::<BTreeSet<_>>();
    for path in paths {
        if changed.contains(&path) {
            result.verified_paths.push(path);
        } else {
            result.unverified_paths.push(path);
        }
    }
    let mismatch = !result.unverified_commits.is_empty() || !result.unverified_paths.is_empty();
    result.status = if mismatch { "mismatch" } else { "verified" }.to_string();
    result
}

fn listed_worktree(registered_root: &Path, target_root: &Path) -> Result<bool, String> {
    let bytes = git(registered_root, &["worktree", "list", "--porcelain", "-z"])?;
    let mut current_path = None;
    for field in bytes.split(|byte| *byte == 0) {
        if field.is_empty() {
            if let Some(path) = current_path.take() {
                let path = canonical_path(path)?;
                if same_path(&path, target_root) {
                    return Ok(true);
                }
            }
        } else if let Some(path) = field.strip_prefix(b"worktree ") {
            current_path = Some(PathBuf::from(String::from_utf8_lossy(path).into_owned()));
        }
    }
    if let Some(path) = current_path {
        return Ok(same_path(&canonical_path(path)?, target_root));
    }
    Ok(false)
}

fn common_dir(root: &Path) -> Result<PathBuf, String> {
    let value = git_text(root, &["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(value.trim());
    canonical_path(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

fn canonical_path(path: PathBuf) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("Git path could not be resolved: {error}"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let bytes = git(root, args)?;
    String::from_utf8(bytes).map_err(|_| "Git returned a non-UTF-8 metadata value".to_string())
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("cannot launch Git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_error(&output, args))
    }
}

fn git_optional(root: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|error| format!("cannot launch Git: {error}"))?;
    if output.status.success() {
        Ok(Some(output.stdout))
    } else {
        Ok(None)
    }
}

fn is_commit(root: &Path, commit: &str) -> bool {
    !commit.is_empty()
        && !commit.chars().any(char::is_whitespace)
        && git(root, &["cat-file", "-e", &format!("{commit}^{{commit}}")]).is_ok()
}

fn is_ancestor(root: &Path, ancestor: &str, descendant: &str) -> bool {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .output();
    matches!(output, Ok(output) if output.status.success())
}

fn add_nul_paths(paths: &mut BTreeSet<String>, bytes: &[u8]) {
    for path in bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        paths.insert(String::from_utf8_lossy(path).into_owned());
    }
}

fn add_status_paths(paths: &mut BTreeSet<String>, bytes: &[u8]) {
    let fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut index = 0;
    while index < fields.len() {
        let record = fields[index];
        index += 1;
        if record.len() < 4 {
            continue;
        }
        let status = &record[..2];
        paths.insert(String::from_utf8_lossy(&record[3..]).into_owned());
        if status.contains(&b'R') || status.contains(&b'C') {
            if let Some(path) = fields.get(index).filter(|path| !path.is_empty()) {
                paths.insert(String::from_utf8_lossy(path).into_owned());
            }
            index += 1;
        }
    }
}

fn normalize_report_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
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

fn git_error(output: &Output, args: &[&str]) -> String {
    let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if message.is_empty() {
        format!("Git command {:?} failed with {}", args, output.status)
    } else {
        message
    }
}

fn unix_time() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn sqlite_error(error: rusqlite::Error) -> String {
    format!("SQLite operation failed: {error}")
}
