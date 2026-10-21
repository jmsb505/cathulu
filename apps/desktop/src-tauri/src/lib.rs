use cathulu_core::{dashboard::DashboardSnapshot, default_data_dir, ProjectSnapshot, Store};
use tauri::{Manager, State};

struct DesktopState {
    store: Store,
}

#[tauri::command]
fn project_snapshot(state: State<'_, DesktopState>) -> Result<Vec<ProjectSnapshot>, String> {
    state.store.list_projects()
}

#[tauri::command]
fn dashboard_snapshot(state: State<'_, DesktopState>) -> Result<DashboardSnapshot, String> {
    state.store.dashboard_snapshot()
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
            app.manage(DesktopState { store });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            project_snapshot,
            dashboard_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("Cathulu desktop runtime failed");
}

#[cfg(test)]
mod phase1_m4_integration_test;

#[cfg(test)]
mod tests {
    use super::{dashboard_snapshot, project_snapshot, DesktopState};
    use cathulu_core::{
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
            .manage(DesktopState { store })
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
}
