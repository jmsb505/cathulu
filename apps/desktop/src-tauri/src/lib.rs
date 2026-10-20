use cathulu_core::{default_data_dir, ProjectSnapshot, Store};
use tauri::{Manager, State};

struct DesktopState {
    store: Store,
}

#[tauri::command]
fn project_snapshot(state: State<'_, DesktopState>) -> Result<Vec<ProjectSnapshot>, String> {
    state.store.list_projects()
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = default_data_dir().map_err(std::io::Error::other)?;
            let store = Store::open(data_dir).map_err(std::io::Error::other)?;
            app.manage(DesktopState { store });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![project_snapshot])
        .run(tauri::generate_context!())
        .expect("Cathulu desktop runtime failed");
}

#[cfg(test)]
mod tests {
    use super::{project_snapshot, DesktopState};
    use cathulu_core::Store;
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
        let app = mock_builder()
            .manage(DesktopState { store })
            .invoke_handler(tauri::generate_handler![project_snapshot])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let response = tauri::test::get_ipc_response(
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
        let displayed = response
            .deserialize::<Vec<cathulu_core::ProjectSnapshot>>()
            .unwrap();
        assert_eq!(displayed, vec![registered]);
        let _ = std::fs::remove_dir_all(temp);
    }
}
