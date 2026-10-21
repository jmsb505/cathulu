use crate::{codex_config::CodexAgentPreference, hash::content_hash};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{value, DocumentMut, Item, Table};

const MAX_CONFIG_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CodexConfigTarget {
    Project { project_id: i64 },
    UserBase,
    UserProfile { profile_name: String },
    ExportOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFieldChange {
    pub file: String,
    pub key: String,
    pub current_value: Option<String>,
    pub selected_value: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodexConfigPreview {
    pub preview_id: String,
    pub target: String,
    pub changes: Vec<ConfigFieldChange>,
    pub skipped_roles: Vec<ConfigSkippedRole>,
    pub export_text: String,
    pub can_apply: bool,
    pub export_only: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSkippedRole {
    pub role_id: String,
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct ConfigFileLocation {
    pub path: PathBuf,
    pub allowed_root: PathBuf,
    pub display_path: String,
}

#[derive(Clone, Debug)]
pub struct PreparedConfigFile {
    pub path: PathBuf,
    pub allowed_root: PathBuf,
    pub display_path: String,
    pub expected_hash: Option<String>,
    pub proposed_content: String,
}

#[derive(Clone, Debug)]
pub struct PreparedCodexConfigPreview {
    pub public: CodexConfigPreview,
    pub target_key: String,
    pub role_ids: Vec<String>,
    pub files: Vec<PreparedConfigFile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedConfigReceipt {
    pub target_key: String,
    pub config_hash: String,
    pub backup_paths: Vec<String>,
}

#[derive(Clone, Debug)]
struct PreviewFileState {
    location: ConfigFileLocation,
    original_hash: Option<String>,
    document: DocumentMut,
    changes: Vec<ConfigFieldChange>,
}

pub fn prepare_codex_config_preview(
    preview_id: String,
    target: String,
    target_key: String,
    config_file: ConfigFileLocation,
    agents_dir: PathBuf,
    preferences: &[CodexAgentPreference],
    export_only: bool,
) -> Result<PreparedCodexConfigPreview, String> {
    if preferences.is_empty() {
        return Err(
            "save at least one model preference before preparing a configuration preview".into(),
        );
    }

    let mut config_state = if export_only {
        PreviewFileState {
            location: config_file,
            original_hash: None,
            document: DocumentMut::new(),
            changes: Vec::new(),
        }
    } else {
        load_preview_file(config_file)?
    };
    let allowed_root = config_state.location.allowed_root.clone();
    let mut agent_states = Vec::new();
    let mut role_ids = Vec::with_capacity(preferences.len());
    let mut skipped_roles = Vec::new();

    for preference in preferences {
        match preference.role_kind {
            crate::codex_config::CodexRoleKind::MainOverseer => {
                role_ids.push(preference.role_id.clone());
                set_field(&mut config_state, "model", &preference.model_slug)?;
                set_field(
                    &mut config_state,
                    "model_reasoning_effort",
                    &preference.reasoning_effort,
                )?;
            }
            crate::codex_config::CodexRoleKind::SubagentDefault => {
                role_ids.push(preference.role_id.clone());
                set_agents_field(
                    &mut config_state,
                    "default_subagent_model",
                    &preference.model_slug,
                )?;
                set_agents_field(
                    &mut config_state,
                    "default_subagent_reasoning_effort",
                    &preference.reasoning_effort,
                )?;
            }
            crate::codex_config::CodexRoleKind::NamedAgent => {
                let name = preference.agent_name.as_deref().ok_or_else(|| {
                    "a named-agent preference is missing its agent name".to_string()
                })?;
                if !is_safe_agent_name(name) {
                    return Err("named-agent configuration contains an unsafe agent name".into());
                }
                let path = agents_dir.join(format!("{name}.toml"));
                let location = ConfigFileLocation {
                    path,
                    allowed_root: allowed_root.clone(),
                    display_path: format!("agents/{name}.toml"),
                };
                let state = if export_only {
                    PreviewFileState {
                        location,
                        original_hash: None,
                        document: DocumentMut::new(),
                        changes: Vec::new(),
                    }
                } else {
                    match load_preview_file(location) {
                        Ok(state) => state,
                        Err(reason) => {
                            skipped_roles.push(ConfigSkippedRole {
                                role_id: preference.role_id.clone(),
                                reason,
                            });
                            continue;
                        }
                    }
                };
                if state.original_hash.is_none() && !export_only {
                    skipped_roles.push(ConfigSkippedRole {
                        role_id: preference.role_id.clone(),
                        reason: format!(
                            "agents/{name}.toml does not exist in this target; Cathulu will not create an incomplete agent definition"
                        ),
                    });
                    continue;
                }
                let mut state = state;
                if !export_only {
                    let configured_name = state
                        .document
                        .get("name")
                        .and_then(Item::as_value)
                        .and_then(|value| value.as_str())
                        .map(str::to_string);
                    if configured_name.as_deref() != Some(name) {
                        skipped_roles.push(ConfigSkippedRole {
                            role_id: preference.role_id.clone(),
                            reason: format!(
                                "agents/{name}.toml does not declare the expected string `name`; this role was skipped"
                            ),
                        });
                        continue;
                    }
                }
                let mut candidate = state.clone();
                let update_result = (|| {
                    set_field(&mut candidate, "model", &preference.model_slug)?;
                    set_field(
                        &mut candidate,
                        "model_reasoning_effort",
                        &preference.reasoning_effort,
                    )?;
                    Ok::<(), String>(())
                })();
                if let Err(reason) = update_result {
                    skipped_roles.push(ConfigSkippedRole {
                        role_id: preference.role_id.clone(),
                        reason,
                    });
                    continue;
                }
                role_ids.push(preference.role_id.clone());
                state = candidate;
                agent_states.push(state);
            }
        }
    }

    let mut changes = config_state.changes.clone();
    let mut files = Vec::new();
    let mut export_parts = Vec::new();

    if !config_state.changes.is_empty() {
        export_parts.push(format!(
            "# {}\n{}",
            config_state.location.display_path,
            export_snippet(&config_state.changes, false)
        ));
        files.push(PreparedConfigFile {
            path: config_state.location.path.clone(),
            allowed_root: allowed_root.clone(),
            display_path: config_state.location.display_path.clone(),
            expected_hash: config_state.original_hash.clone(),
            proposed_content: config_state.document.to_string(),
        });
    }
    for state in agent_states {
        let content = state.document.to_string();
        changes.extend(state.changes.clone());
        if !state.changes.is_empty() {
            export_parts.push(format!(
                "# {}\n{}",
                state.location.display_path,
                export_snippet(&state.changes, true)
            ));
            files.push(PreparedConfigFile {
                path: state.location.path.clone(),
                allowed_root: state.location.allowed_root.clone(),
                display_path: state.location.display_path.clone(),
                expected_hash: state.original_hash.clone(),
                proposed_content: content,
            });
        }
    }

    let export_text = if export_parts.is_empty() {
        "All selected values already match the selected configuration.".to_string()
    } else {
        export_parts.join("\n\n")
    };
    let can_apply = !export_only && !files.is_empty();
    Ok(PreparedCodexConfigPreview {
        public: CodexConfigPreview {
            preview_id,
            target,
            changes,
            skipped_roles,
            export_text,
            can_apply,
            export_only,
        },
        target_key,
        role_ids,
        files,
    })
}

fn load_preview_file(location: ConfigFileLocation) -> Result<PreviewFileState, String> {
    if !location.path.is_absolute()
        || !location.allowed_root.is_absolute()
        || !location.path.starts_with(&location.allowed_root)
    {
        return Err("Codex configuration path is outside the selected target".into());
    }
    let (original_hash, original_content) = match fs::symlink_metadata(&location.path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "{} must be a regular file, not a link or directory",
                    location.display_path
                ));
            }
            if metadata.len() > MAX_CONFIG_BYTES {
                return Err(format!(
                    "{} exceeds the 1 MiB safety limit",
                    location.display_path
                ));
            }
            let content = fs::read_to_string(&location.path)
                .map_err(|error| format!("cannot read {}: {error}", location.display_path))?;
            (Some(content_hash(&content)), content)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (None, String::new()),
        Err(error) => return Err(format!("cannot inspect {}: {error}", location.display_path)),
    };
    let document = DocumentMut::from_str(&original_content).map_err(|_| {
        format!(
            "{} is not valid TOML; no changes were made",
            location.display_path
        )
    })?;
    Ok(PreviewFileState {
        location,
        original_hash,
        document,
        changes: Vec::new(),
    })
}

fn set_field(state: &mut PreviewFileState, key: &str, selected: &str) -> Result<(), String> {
    let current_value = read_string_field(state, key)?;
    if current_value.as_deref() == Some(selected) {
        return Ok(());
    }
    state
        .document
        .as_table_mut()
        .insert(key, value(selected.to_string()));
    state.changes.push(ConfigFieldChange {
        file: state.location.display_path.clone(),
        key: key.to_string(),
        current_value,
        selected_value: selected.to_string(),
    });
    Ok(())
}

fn read_string_field(state: &PreviewFileState, key: &str) -> Result<Option<String>, String> {
    state
        .document
        .as_table()
        .get(key)
        .filter(|item| !item.is_none())
        .map(|item| {
            item.as_value()
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .ok_or_else(|| {
                    format!(
                        "{} has a non-string `{key}` value; no changes were made",
                        state.location.display_path
                    )
                })
        })
        .transpose()
}

fn set_agents_field(state: &mut PreviewFileState, key: &str, selected: &str) -> Result<(), String> {
    if state.document.get("agents").is_none() {
        state
            .document
            .as_table_mut()
            .insert("agents", Item::Table(Table::new()));
    }
    let current_value = {
        let agents = state
            .document
            .as_table()
            .get("agents")
            .ok_or_else(|| "internal missing [agents] table".to_string())?
            .as_table()
            .ok_or_else(|| {
                "the existing [agents] Codex config value is not a table; no changes were made"
                    .to_string()
            })?;
        agents
            .get(key)
            .filter(|item| !item.is_none())
            .map(|item| {
                item.as_value()
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        format!(
                            "{} has a non-string `agents.{key}` value; no changes were made",
                            state.location.display_path
                        )
                    })
            })
            .transpose()?
    };
    if current_value.as_deref() == Some(selected) {
        return Ok(());
    }
    let agents_item = state
        .document
        .as_table_mut()
        .get_mut("agents")
        .expect("agents table was verified above");
    agents_item
        .as_table_mut()
        .expect("agents table was verified above")
        .insert(key, value(selected.to_string()));
    state.changes.push(ConfigFieldChange {
        file: state.location.display_path.clone(),
        key: format!("agents.{key}"),
        current_value,
        selected_value: selected.to_string(),
    });
    Ok(())
}

fn export_snippet(changes: &[ConfigFieldChange], agent_file: bool) -> String {
    changes
        .iter()
        .map(|change| {
            let key = if agent_file {
                change.key.as_str()
            } else {
                change.key.strip_prefix("agents.").unwrap_or(&change.key)
            };
            format!("{key} = {:?}", change.selected_value)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn apply_codex_config_preview(
    preview: &PreparedCodexConfigPreview,
) -> Result<AppliedConfigReceipt, String> {
    if !preview.public.can_apply {
        return Err("this preview has no configuration changes to apply".into());
    }
    for file in &preview.files {
        ensure_safe_parent_path(&file.allowed_root, &file.path, true)?;
        let current_hash = match fs::symlink_metadata(&file.path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(format!(
                        "{} is no longer a regular config file",
                        file.display_path
                    ));
                }
                let current = fs::read_to_string(&file.path)
                    .map_err(|error| format!("cannot recheck {}: {error}", file.display_path))?;
                Some(content_hash(&current))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("cannot recheck {}: {error}", file.display_path)),
        };
        if current_hash != file.expected_hash {
            return Err(format!(
                "{} changed after preview; prepare a fresh preview before applying",
                file.display_path
            ));
        }
    }

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    let mut staged = Vec::with_capacity(preview.files.len());
    for (index, file) in preview.files.iter().enumerate() {
        ensure_safe_parent_path(&file.allowed_root, &file.path, true)?;
        let parent = file
            .path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", file.display_path))?;
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "cannot create config directory for {}: {error}",
                file.display_path
            )
        })?;
        let name = file
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{} has an invalid filename", file.display_path))?;
        let temp_path = parent.join(format!(".{name}.cathulu-{nonce}-{index}.tmp"));
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| format!("cannot stage {}: {error}", file.display_path))?;
        if let Err(error) = temp_file
            .write_all(file.proposed_content.as_bytes())
            .and_then(|_| temp_file.sync_all())
        {
            let _ = fs::remove_file(&temp_path);
            cleanup_staged(&staged);
            return Err(format!("cannot stage {}: {error}", file.display_path));
        }
        staged.push((file, temp_path));
    }

    let mut completed: Vec<(PathBuf, Option<PathBuf>, String)> = Vec::new();
    let mut backups = Vec::new();
    for (index, (file, temp_path)) in staged.iter().enumerate() {
        let backup = if file.expected_hash.is_some() {
            let backup_path = file.path.with_file_name(format!(
                "{}.cathulu-{nonce}-{index}.bak",
                file.path.file_name().unwrap_or_default().to_string_lossy()
            ));
            if let Err(error) = fs::rename(&file.path, &backup_path) {
                rollback_applied(&completed);
                cleanup_staged(&staged);
                return Err(format!(
                    "cannot create backup for {}: {error}",
                    file.display_path
                ));
            }
            Some(backup_path)
        } else {
            None
        };
        if let Err(error) = fs::rename(temp_path, &file.path) {
            if let Some(backup_path) = &backup {
                let _ = fs::copy(backup_path, &file.path);
            }
            rollback_applied(&completed);
            cleanup_staged(&staged);
            return Err(format!("cannot install {}: {error}", file.display_path));
        }
        if let Some(backup_path) = &backup {
            backups.push(backup_path.to_string_lossy().to_string());
        }
        completed.push((file.path.clone(), backup, file.display_path.clone()));
    }

    let config_hash = content_hash(
        &preview
            .files
            .iter()
            .map(|file| format!("{}\0{}", file.display_path, file.proposed_content))
            .collect::<Vec<_>>()
            .join("\0"),
    );
    Ok(AppliedConfigReceipt {
        target_key: preview.target_key.clone(),
        config_hash,
        backup_paths: backups,
    })
}

fn rollback_applied(completed: &[(PathBuf, Option<PathBuf>, String)]) {
    for (path, backup, _) in completed.iter().rev() {
        let _ = fs::remove_file(path);
        if let Some(backup) = backup {
            let _ = fs::copy(backup, path);
        }
    }
}

fn ensure_safe_parent_path(
    root: &PathBuf,
    candidate: &PathBuf,
    create_missing: bool,
) -> Result<(), String> {
    if !root.is_absolute() || !candidate.is_absolute() || !candidate.starts_with(root) {
        return Err("Codex configuration path is outside the selected target".into());
    }
    if create_missing {
        fs::create_dir_all(root)
            .map_err(|error| format!("cannot create selected Codex configuration home: {error}"))?;
    }
    let root_metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("selected Codex configuration root is unavailable: {error}"))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err("selected Codex configuration root must be a regular directory".into());
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("cannot resolve selected Codex configuration root: {error}"))?;
    let relative_parent = candidate
        .parent()
        .and_then(|parent| parent.strip_prefix(root).ok())
        .ok_or_else(|| "Codex configuration parent is outside the selected target".to_string())?;
    let mut current = root.clone();
    for component in relative_parent.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err("Codex configuration directories cannot be links or files".into());
            }
            Ok(_) => {
                let canonical = current.canonicalize().map_err(|error| {
                    format!("cannot resolve Codex configuration directory: {error}")
                })?;
                if !canonical.starts_with(&canonical_root) {
                    return Err(
                        "Codex configuration directory resolves outside its selected root".into(),
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_missing => {
                fs::create_dir(&current).map_err(|create_error| {
                    format!("cannot create Codex configuration directory: {create_error}")
                })?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "cannot inspect Codex configuration directory: {error}"
                ))
            }
        }
    }
    Ok(())
}

fn cleanup_staged(staged: &[(&PreparedConfigFile, PathBuf)]) {
    for (_, path) in staged {
        let _ = fs::remove_file(path);
    }
}

fn is_safe_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 80
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::{
        apply_codex_config_preview, prepare_codex_config_preview, CodexConfigTarget,
        ConfigFileLocation,
    };
    use crate::codex_config::{CodexAgentPreference, CodexRoleKind};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("cathulu-config-{label}-{suffix}"))
    }

    fn preference(kind: CodexRoleKind, name: Option<&str>, model: &str) -> CodexAgentPreference {
        CodexAgentPreference {
            role_id: name.map_or_else(|| kind.as_str().to_string(), |name| format!("agent:{name}")),
            role_kind: kind,
            agent_name: name.map(str::to_string),
            model_slug: model.into(),
            reasoning_effort: "high".into(),
            catalog_version: "test".into(),
            last_applied_target: None,
            last_applied_at: None,
            last_applied_config_hash: None,
            updated_at: 1,
        }
    }

    fn preview(
        root: &std::path::Path,
        preferences: &[CodexAgentPreference],
        export_only: bool,
    ) -> super::PreparedCodexConfigPreview {
        prepare_codex_config_preview(
            "preview-1".into(),
            "Project: demo".into(),
            "project:1".into(),
            ConfigFileLocation {
                path: root.join("config.toml"),
                allowed_root: root.to_path_buf(),
                display_path: ".codex/config.toml".into(),
            },
            root.join("agents"),
            preferences,
            export_only,
        )
        .unwrap()
    }

    #[test]
    fn preview_preserves_comments_and_unrelated_values_and_cancel_has_no_side_effects() {
        let root = temp_dir("preview");
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        let original = "# keep this comment\nmodel = \"old-model\"\n[unrelated]\nsecret_label = \"private-value\"\n";
        fs::write(&config, original).unwrap();
        let prepared = preview(
            &root,
            &[preference(CodexRoleKind::MainOverseer, None, "new-model")],
            false,
        );
        assert_eq!(prepared.public.changes.len(), 2);
        assert!(prepared
            .public
            .export_text
            .contains("model = \"new-model\""));
        assert!(!prepared.public.export_text.contains("# keep this comment"));
        assert!(!prepared.public.export_text.contains("private-value"));
        assert_eq!(fs::read_to_string(&config).unwrap(), original);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn apply_creates_backup_and_stale_preview_is_rejected() {
        let root = temp_dir("apply");
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        fs::write(&config, "# keep\nmodel = \"old-model\"\n").unwrap();
        let prepared = preview(
            &root,
            &[preference(CodexRoleKind::MainOverseer, None, "new-model")],
            false,
        );
        let receipt = apply_codex_config_preview(&prepared).unwrap();
        assert_eq!(receipt.backup_paths.len(), 1);
        assert!(fs::read_to_string(&config)
            .unwrap()
            .contains("model = \"new-model\""));
        assert!(std::path::Path::new(&receipt.backup_paths[0]).exists());

        let stale = preview(
            &root,
            &[preference(CodexRoleKind::MainOverseer, None, "third-model")],
            false,
        );
        fs::write(&config, "model = \"external-change\"\n").unwrap();
        assert!(apply_codex_config_preview(&stale)
            .unwrap_err()
            .contains("changed after preview"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_only_builds_a_copyable_config_without_writing() {
        let root = temp_dir("export");
        fs::create_dir_all(&root).unwrap();
        let config = root.join("config.toml");
        fs::write(&config, "# existing\n[unrelated]\nvalue = 4\n").unwrap();
        let before = fs::read_to_string(&config).unwrap();
        let prepared = preview(
            &root,
            &[
                preference(CodexRoleKind::SubagentDefault, None, "model-b"),
                preference(CodexRoleKind::NamedAgent, Some("reviewer"), "agent-model"),
            ],
            true,
        );
        assert!(!prepared.public.can_apply);
        assert!(prepared
            .public
            .export_text
            .contains("default_subagent_model"));
        assert!(prepared.public.export_text.contains("agents/reviewer.toml"));
        assert!(prepared
            .public
            .export_text
            .contains("model = \"agent-model\""));
        assert_eq!(prepared.public.changes.len(), 4);
        assert_eq!(fs::read_to_string(&config).unwrap(), before);
        let _target = CodexConfigTarget::ExportOnly;
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_toml_and_missing_named_agent_are_rejected_without_writes() {
        let root = temp_dir("invalid");
        fs::create_dir_all(root.join("agents")).unwrap();
        let config = root.join("config.toml");
        fs::write(&config, "model = [\n").unwrap();
        let invalid = prepare_codex_config_preview(
            "preview-1".into(),
            "Project".into(),
            "project:1".into(),
            ConfigFileLocation {
                path: config.clone(),
                allowed_root: root.clone(),
                display_path: ".codex/config.toml".into(),
            },
            root.join("agents"),
            &[preference(CodexRoleKind::MainOverseer, None, "model")],
            false,
        )
        .unwrap_err();
        assert!(invalid.contains("not valid TOML"));

        fs::write(&config, "model = \"old\"\n").unwrap();
        let missing_agent = prepare_codex_config_preview(
            "preview-2".into(),
            "Project".into(),
            "project:1".into(),
            ConfigFileLocation {
                path: config.clone(),
                allowed_root: root.clone(),
                display_path: ".codex/config.toml".into(),
            },
            root.join("agents"),
            &[preference(
                CodexRoleKind::NamedAgent,
                Some("reviewer"),
                "model",
            )],
            false,
        )
        .unwrap();
        assert!(!missing_agent.public.can_apply);
        assert_eq!(missing_agent.public.skipped_roles.len(), 1);
        assert!(missing_agent.public.skipped_roles[0]
            .reason
            .contains("will not create an incomplete agent definition"));
        assert_eq!(fs::read_to_string(&config).unwrap(), "model = \"old\"\n");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn default_subagent_and_existing_named_agent_overrides_preserve_their_own_files() {
        let root = temp_dir("roles");
        let agents = root.join("agents");
        fs::create_dir_all(&agents).unwrap();
        let config = root.join("config.toml");
        let config_before = "# main config comment\n[agents]\ndefault_subagent_model = \"old-default\"\n[unrelated]\nkeep = true\n";
        fs::write(&config, config_before).unwrap();
        let agent_path = agents.join("reviewer.toml");
        let agent_before = "# agent instructions are retained\nname = \"reviewer\"\ndescription = \"Code review specialist\"\ninstructions = \"Review carefully\"\nmodel = \"old-agent\"\n";
        fs::write(&agent_path, agent_before).unwrap();

        let prepared = prepare_codex_config_preview(
            "preview-roles".into(),
            "Project · fixture".into(),
            "project:1".into(),
            ConfigFileLocation {
                path: config.clone(),
                allowed_root: root.clone(),
                display_path: ".codex/config.toml".into(),
            },
            agents,
            &[
                preference(CodexRoleKind::SubagentDefault, None, "new-default"),
                preference(CodexRoleKind::NamedAgent, Some("reviewer"), "new-agent"),
            ],
            false,
        )
        .unwrap();
        assert_eq!(prepared.public.changes.len(), 4);
        assert_eq!(fs::read_to_string(&config).unwrap(), config_before);
        assert_eq!(fs::read_to_string(&agent_path).unwrap(), agent_before);

        apply_codex_config_preview(&prepared).unwrap();
        let config_after = fs::read_to_string(config).unwrap();
        assert!(config_after.contains("# main config comment"));
        assert!(config_after.contains("default_subagent_model = \"new-default\""));
        assert!(config_after.contains("keep = true"));
        let agent_after = fs::read_to_string(agent_path).unwrap();
        assert!(agent_after.contains("# agent instructions are retained"));
        assert!(agent_after.contains("instructions = \"Review carefully\""));
        assert!(agent_after.contains("model = \"new-agent\""));
        assert!(agent_after.contains("model_reasoning_effort = \"high\""));
        let _ = fs::remove_dir_all(root);
    }
}
