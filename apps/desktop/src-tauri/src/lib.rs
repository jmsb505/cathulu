use cathulu_core::{
    codex_config::{
        load_bundled_codex_catalog, CodexAgentPreference, CodexAgentPreferenceInput,
        CodexSelectorSnapshot,
    },
    codex_config_files::{
        apply_codex_config_preview as apply_config_preview, prepare_codex_config_preview,
        CodexConfigPreview, CodexConfigTarget, ConfigFileLocation, PreparedCodexConfigPreview,
    },
    dashboard::DashboardSnapshot,
    default_data_dir, ProjectSnapshot, Store,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{Manager, State};

struct DesktopState {
    store: Store,
    config_previews: Mutex<HashMap<String, PreparedCodexConfigPreview>>,
}

impl DesktopState {
    fn new(store: Store) -> Self {
        Self {
            store,
            config_previews: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexConfigApplyResult {
    target: String,
    preference_state: String,
    backup_count: usize,
    config_hash: String,
    runtime_effectiveness_verified: bool,
}

#[tauri::command]
fn project_snapshot(state: State<'_, DesktopState>) -> Result<Vec<ProjectSnapshot>, String> {
    state.store.list_projects()
}

#[tauri::command]
fn dashboard_snapshot(state: State<'_, DesktopState>) -> Result<DashboardSnapshot, String> {
    state.store.dashboard_snapshot()
}

#[tauri::command]
fn codex_selector_snapshot(
    state: State<'_, DesktopState>,
) -> Result<CodexSelectorSnapshot, String> {
    let catalog = load_bundled_codex_catalog()?;
    let preferences = state.store.codex_selector_preferences()?;
    let preference_state = if preferences
        .iter()
        .any(|preference| preference.last_applied_target.is_some())
    {
        "written_to_config"
    } else {
        "stored_in_cathulu"
    };
    Ok(CodexSelectorSnapshot {
        catalog,
        preferences,
        preference_state: preference_state.to_string(),
        runtime_effectiveness_verified: false,
    })
}

#[tauri::command]
fn save_codex_selector_preference(
    input: CodexAgentPreferenceInput,
    state: State<'_, DesktopState>,
) -> Result<CodexAgentPreference, String> {
    let catalog = load_bundled_codex_catalog()?;
    state.store.save_codex_selector_preference(&input, &catalog)
}

#[tauri::command]
fn preview_codex_config(
    target: CodexConfigTarget,
    state: State<'_, DesktopState>,
) -> Result<CodexConfigPreview, String> {
    let catalog = load_bundled_codex_catalog()?;
    let preferences = state.store.codex_selector_preferences()?;
    if preferences.is_empty() {
        return Err(
            "save at least one model preference before preparing a configuration preview".into(),
        );
    }
    if preferences.len() > 64 {
        return Err(
            "too many saved role preferences; remove unused roles before preparing a preview"
                .into(),
        );
    }
    for preference in &preferences {
        CodexAgentPreferenceInput {
            role_kind: preference.role_kind,
            agent_name: preference.agent_name.clone(),
            model_slug: preference.model_slug.clone(),
            reasoning_effort: preference.reasoning_effort.clone(),
        }
        .validate(&catalog)?;
    }

    let resolved = resolve_config_target(&state.store, target)?;
    let preview_id = new_preview_id();
    let preview = prepare_codex_config_preview(
        preview_id,
        resolved.label,
        resolved.key,
        resolved.config_file,
        resolved.agents_dir,
        &preferences,
        resolved.export_only,
    )?;
    let response = preview.public.clone();
    let mut pending = state
        .config_previews
        .lock()
        .map_err(|_| "Codex config preview state is unavailable".to_string())?;
    if response.can_apply {
        if pending.len() >= 4 {
            pending.clear();
        }
        pending.insert(response.preview_id.clone(), preview);
    }
    Ok(response)
}

#[tauri::command]
fn apply_codex_config_preview(
    preview_id: String,
    state: State<'_, DesktopState>,
) -> Result<CodexConfigApplyResult, String> {
    let preview = state
        .config_previews
        .lock()
        .map_err(|_| "Codex config preview state is unavailable".to_string())?
        .remove(&preview_id)
        .ok_or_else(|| "configuration preview expired; prepare a new preview".to_string())?;
    let receipt = apply_config_preview(&preview)?;
    state.store.mark_codex_selector_preferences_applied(
        &preview.role_ids,
        &receipt.target_key,
        &receipt.config_hash,
    )?;
    Ok(CodexConfigApplyResult {
        target: preview.public.target,
        preference_state: "written_to_config".into(),
        backup_count: receipt.backup_paths.len(),
        config_hash: receipt.config_hash,
        runtime_effectiveness_verified: false,
    })
}

#[tauri::command]
fn discard_codex_config_preview(
    preview_id: String,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state
        .config_previews
        .lock()
        .map_err(|_| "Codex config preview state is unavailable".to_string())?
        .remove(&preview_id);
    Ok(())
}

struct ResolvedConfigTarget {
    label: String,
    key: String,
    config_file: ConfigFileLocation,
    agents_dir: PathBuf,
    export_only: bool,
}

fn resolve_config_target(
    store: &Store,
    target: CodexConfigTarget,
) -> Result<ResolvedConfigTarget, String> {
    match target {
        CodexConfigTarget::Project { project_id } => {
            let project = store
                .get_project(project_id)?
                .ok_or_else(|| "select a repository already registered in Cathulu".to_string())?;
            let root = PathBuf::from(project.path)
                .canonicalize()
                .map_err(|error| format!("registered repository is unavailable: {error}"))?;
            if !root.is_dir() {
                return Err("registered repository path is not a directory".into());
            }
            let codex_dir = root.join(".codex");
            ensure_optional_child_is_safe(&root, &codex_dir)?;
            let agents_dir = codex_dir.join("agents");
            ensure_optional_child_is_safe(&root, &agents_dir)?;
            let config_path = codex_dir.join("config.toml");
            ensure_optional_file_is_safe(&root, &config_path)?;
            Ok(ResolvedConfigTarget {
                label: format!("Project · {} · .codex/config.toml", project.name),
                key: format!("project:{project_id}"),
                config_file: ConfigFileLocation {
                    path: config_path,
                    allowed_root: root,
                    display_path: ".codex/config.toml".into(),
                },
                agents_dir,
                export_only: false,
            })
        }
        CodexConfigTarget::UserBase => user_config_target(None, false),
        CodexConfigTarget::UserProfile { profile_name } => {
            if !is_safe_profile_name(&profile_name) {
                return Err(
                    "profile name must use 1–80 letters, numbers, hyphens, or underscores".into(),
                );
            }
            user_config_target(Some(profile_name), false)
        }
        CodexConfigTarget::ExportOnly => user_config_target(None, true),
    }
}

fn user_config_target(
    profile_name: Option<String>,
    export_only: bool,
) -> Result<ResolvedConfigTarget, String> {
    let config_home = codex_home()?;
    user_config_target_for_home(config_home, profile_name, export_only)
}

fn user_config_target_for_home(
    config_home: PathBuf,
    profile_name: Option<String>,
    export_only: bool,
) -> Result<ResolvedConfigTarget, String> {
    if profile_name
        .as_deref()
        .is_some_and(|profile| !is_safe_profile_name(profile))
    {
        return Err("profile name must use 1–80 letters, numbers, hyphens, or underscores".into());
    }
    if !config_home.is_absolute() {
        return Err("Codex configuration home must be an absolute path".into());
    }
    if let Ok(metadata) = std::fs::symlink_metadata(&config_home) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("Codex configuration home must be a regular local directory".into());
        }
    }
    let agents_dir = config_home.join("agents");
    ensure_optional_child_is_safe(&config_home, &agents_dir)?;
    let (label, key, filename, display_path) = match profile_name {
        Some(profile) => (
            format!("User profile · {profile}"),
            format!("user_profile:{profile}"),
            format!("{profile}.config.toml"),
            format!("{profile}.config.toml"),
        ),
        None if export_only => (
            "Export only · user config format".into(),
            "export_only".into(),
            "config.toml".into(),
            "config.toml".into(),
        ),
        None => (
            "User · config.toml".into(),
            "user_base".into(),
            "config.toml".into(),
            "config.toml".into(),
        ),
    };
    let config_path = config_home.join(filename);
    ensure_optional_file_is_safe(&config_home, &config_path)?;
    Ok(ResolvedConfigTarget {
        label,
        key,
        config_file: ConfigFileLocation {
            path: config_path,
            allowed_root: config_home,
            display_path,
        },
        agents_dir,
        export_only,
    })
}

fn codex_home() -> Result<PathBuf, String> {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|path| PathBuf::from(path).join(".codex")))
        .ok_or_else(|| {
            "CODEX_HOME or USERPROFILE must identify the local Codex configuration home".to_string()
        })?;
    if !home.is_absolute() {
        return Err("Codex configuration home must be an absolute path".into());
    }
    Ok(home)
}

fn ensure_optional_child_is_safe(root: &Path, candidate: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(candidate) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("Codex configuration directories cannot be symbolic links".into())
        }
        Ok(metadata) if !metadata.is_dir() => {
            Err("Codex configuration path is not a directory".into())
        }
        Ok(_) => {
            let root = root
                .canonicalize()
                .map_err(|error| format!("cannot resolve configuration root: {error}"))?;
            let candidate = candidate
                .canonicalize()
                .map_err(|error| format!("cannot resolve configuration directory: {error}"))?;
            if candidate.starts_with(root) {
                Ok(())
            } else {
                Err("Codex configuration directory resolves outside its selected root".into())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot inspect Codex configuration directory: {error}"
        )),
    }
}

fn ensure_optional_file_is_safe(root: &Path, candidate: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(candidate) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("Codex configuration files cannot be symbolic links".into())
        }
        Ok(metadata) if !metadata.is_file() => {
            Err("Codex configuration target is not a regular file".into())
        }
        Ok(_) => {
            let root = root
                .canonicalize()
                .map_err(|error| format!("cannot resolve configuration root: {error}"))?;
            let candidate = candidate
                .canonicalize()
                .map_err(|error| format!("cannot resolve configuration file: {error}"))?;
            if candidate.starts_with(root) {
                Ok(())
            } else {
                Err("Codex configuration file resolves outside its selected root".into())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot inspect Codex configuration file: {error}")),
    }
}

fn is_safe_profile_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

static PREVIEW_COUNTER: AtomicU64 = AtomicU64::new(1);

fn new_preview_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!(
        "codex-config-{now}-{}",
        PREVIEW_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = default_data_dir().map_err(std::io::Error::other)?;
            let store = Store::open(data_dir).map_err(std::io::Error::other)?;
            let report = store
                .reconcile_registered_projects()
                .map_err(std::io::Error::other)?;
            for failure in report.failures {
                eprintln!(
                    "Cathulu startup Git reconciliation skipped project {}: {}",
                    failure.project_id, failure.message
                );
            }
            app.manage(DesktopState::new(store));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            project_snapshot,
            dashboard_snapshot,
            codex_selector_snapshot,
            save_codex_selector_preference,
            preview_codex_config,
            apply_codex_config_preview,
            discard_codex_config_preview
        ])
        .run(tauri::generate_context!())
        .expect("Cathulu desktop runtime failed");
}

#[cfg(test)]
mod phase1_m4_integration_test;

#[cfg(test)]
mod tests {
    use super::{
        apply_codex_config_preview, codex_selector_snapshot, dashboard_snapshot,
        preview_codex_config, project_snapshot, save_codex_selector_preference,
        user_config_target_for_home, DesktopState,
    };
    use cathulu_core::{
        codex_config::{load_bundled_codex_catalog, CodexAgentPreferenceInput, CodexRoleKind},
        codex_config_files::CodexConfigPreview,
        memory::{FreshnessBasis, MemoryCreateInput, MemorySource},
        Store,
    };
    use std::path::Path;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::WebviewWindowBuilder;

    #[test]
    fn dashboard_command_boundary_returns_the_shared_project_snapshot() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp = std::env::temp_dir().join(format!("cathulu-tauri-command-{suffix}"));
        let repo = temp.join("repo");
        let data = temp.join("data");
        std::fs::create_dir_all(&repo).unwrap();
        let status = Command::new("git")
            .args([
                "-C",
                repo.to_str().unwrap(),
                "init",
                "--quiet",
                "-b",
                "main",
            ])
            .status()
            .unwrap();
        assert!(status.success());
        let store = Store::open(&data).unwrap();
        let registered = store.register_project(Path::new(&repo)).unwrap();
        let first_body = "First scope HOT memory";
        let second_body = "Second scope has longer HOT memory";
        for (scope_id, body, source_id) in [
            ("scope-1", first_body, "decision-1"),
            ("scope-2", second_body, "decision-2"),
        ] {
            store
                .create_memory(&MemoryCreateInput {
                    scope_type: "scope".into(),
                    scope_id: Some(scope_id.into()),
                    memory_class: "canonical".into(),
                    temperature: "hot".into(),
                    kind: "fact".into(),
                    body: body.into(),
                    sources: vec![MemorySource {
                        source_type: "human_instruction".into(),
                        source_id: source_id.into(),
                    }],
                    artifact_ids: vec![],
                    valid_from: None,
                    valid_until: None,
                    supersedes_id: None,
                    freshness_basis: FreshnessBasis::default(),
                    importance: 0.8,
                    confidence: 1.0,
                    active: true,
                    protected: false,
                })
                .unwrap();
        }
        let app = mock_builder()
            .manage(DesktopState::new(store))
            .invoke_handler(tauri::generate_handler![
                project_snapshot,
                dashboard_snapshot
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let response = tauri::test::get_ipc_response(
            &window,
            InvokeRequest {
                cmd: "dashboard_snapshot".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .unwrap();
        let displayed = response
            .deserialize::<cathulu_core::dashboard::DashboardSnapshot>()
            .unwrap();
        assert_eq!(displayed.projects, vec![registered]);
        assert_eq!(
            displayed.memory_health.hot_bytes,
            (first_body.len() + second_body.len()) as i64
        );
        assert_eq!(displayed.memory_health.hot_scope_count, 2);
        assert_eq!(
            displayed.memory_health.largest_scope_hot_bytes,
            second_body.len() as i64
        );

        let compatibility_response = tauri::test::get_ipc_response(
            &window,
            InvokeRequest {
                cmd: "project_snapshot".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .unwrap();
        let projects = compatibility_response
            .deserialize::<Vec<cathulu_core::ProjectSnapshot>>()
            .unwrap();
        assert_eq!(projects, displayed.projects);
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn config_preview_apply_is_explicit_preserves_other_toml_and_keeps_runtime_unknown() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp = std::env::temp_dir().join(format!("cathulu-config-command-{suffix}"));
        let repo = temp.join("repo");
        let codex_dir = repo.join(".codex");
        let data = temp.join("data");
        std::fs::create_dir_all(&codex_dir).unwrap();
        let git_status = Command::new("git")
            .args([
                "-C",
                repo.to_str().unwrap(),
                "init",
                "--quiet",
                "-b",
                "main",
            ])
            .status()
            .unwrap();
        assert!(git_status.success());
        let config_path = codex_dir.join("config.toml");
        let original = "# preserved comment\n[unrelated]\nkeep = \"unchanged\"\n";
        std::fs::write(&config_path, original).unwrap();

        let store = Store::open(&data).unwrap();
        let project = store.register_project(Path::new(&repo)).unwrap();
        let catalog = load_bundled_codex_catalog().unwrap();
        let model = catalog.models.first().unwrap();
        let effort = model
            .default_reasoning_effort
            .as_deref()
            .unwrap_or_else(|| model.reasoning_efforts.first().unwrap());
        store
            .save_codex_selector_preference(
                &CodexAgentPreferenceInput {
                    role_kind: CodexRoleKind::MainOverseer,
                    agent_name: None,
                    model_slug: model.model_slug.clone(),
                    reasoning_effort: effort.to_string(),
                },
                &catalog,
            )
            .unwrap();

        let app = mock_builder()
            .manage(DesktopState::new(store.clone()))
            .invoke_handler(tauri::generate_handler![
                preview_codex_config,
                apply_codex_config_preview,
                codex_selector_snapshot,
                save_codex_selector_preference
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let preview_response = tauri::test::get_ipc_response(
            &window,
            InvokeRequest {
                cmd: "preview_codex_config".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(serde_json::json!({
                    "target": { "kind": "project", "projectId": project.id }
                })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .unwrap();
        let preview = preview_response
            .deserialize::<CodexConfigPreview>()
            .unwrap();
        assert!(preview.can_apply);
        assert_eq!(preview.changes.len(), 2);
        assert_eq!(std::fs::read_to_string(&config_path).unwrap(), original);

        let apply_response = tauri::test::get_ipc_response(
            &window,
            InvokeRequest {
                cmd: "apply_codex_config_preview".into(),
                callback: CallbackFn(2),
                error: CallbackFn(3),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::Json(serde_json::json!({ "previewId": preview.preview_id })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .unwrap();
        let apply_result = apply_response.deserialize::<serde_json::Value>().unwrap();
        assert_eq!(apply_result["preferenceState"], "written_to_config");
        assert_eq!(apply_result["runtimeEffectivenessVerified"], false);

        let applied = std::fs::read_to_string(&config_path).unwrap();
        assert!(applied.contains("# preserved comment"));
        assert!(applied.contains("keep = \"unchanged\""));
        assert!(applied.contains(&format!("model = \"{}\"", model.model_slug)));
        let backup = std::fs::read_dir(&codex_dir)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().ends_with(".bak"))
            .unwrap();
        assert_eq!(std::fs::read_to_string(backup.path()).unwrap(), original);
        let preferences = store.codex_selector_preferences().unwrap();
        assert_eq!(
            preferences[0].last_applied_target.as_deref(),
            Some("project:1")
        );
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn user_base_profile_and_export_targets_resolve_to_distinct_files_without_writing() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home = std::env::temp_dir().join(format!("cathulu-codex-home-{suffix}"));
        let base = user_config_target_for_home(home.clone(), None, false).unwrap();
        let profile =
            user_config_target_for_home(home.clone(), Some("work".into()), false).unwrap();
        let export = user_config_target_for_home(home.clone(), None, true).unwrap();
        assert_eq!(base.config_file.path, home.join("config.toml"));
        assert_eq!(profile.config_file.path, home.join("work.config.toml"));
        assert_eq!(export.config_file.path, home.join("config.toml"));
        assert!(export.export_only);
        assert_ne!(base.key, profile.key);
        assert!(
            user_config_target_for_home(home.clone(), Some("../outside".into()), false).is_err()
        );
        assert!(!home.exists(), "target resolution is read-only");
    }
}
