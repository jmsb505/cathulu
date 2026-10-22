use std::path::Path;
#[cfg(not(windows))]
use std::process::{Command, Stdio};

#[cfg(windows)]
mod windows_dashboard_launch {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_ABANDONED: u32 = 0x80;
    const WAIT_FAILED: u32 = u32::MAX;
    const WAIT_TIMEOUT: u32 = 0x102;
    const STARTUP_MUTEX_TIMEOUT_MS: u32 = 15_000;
    const WINDOW_READY_TIMEOUT: Duration = Duration::from_secs(8);
    const SW_SHOWNOACTIVATE: i32 = 4;

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateMutexW(
            attributes: *const std::ffi::c_void,
            initial_owner: i32,
            name: *const u16,
        ) -> *mut std::ffi::c_void;
        fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
        fn ReleaseMutex(handle: *mut std::ffi::c_void) -> i32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut std::ffi::c_void;
    }

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            window: *mut std::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_command: i32,
        ) -> isize;
    }

    struct LaunchMutex(*mut std::ffi::c_void);

    impl LaunchMutex {
        fn acquire() -> Result<Self, String> {
            let name = wide(OsStr::new("Local\\CathuluDashboardStartup"));
            let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
            if handle.is_null() {
                return Err(format!(
                    "CreateMutexW failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            match unsafe { WaitForSingleObject(handle, STARTUP_MUTEX_TIMEOUT_MS) } {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self(handle)),
                WAIT_TIMEOUT => {
                    unsafe { CloseHandle(handle) };
                    Err("timed out waiting to serialize Cathulu dashboard startup".into())
                }
                WAIT_FAILED => {
                    let error = std::io::Error::last_os_error();
                    unsafe { CloseHandle(handle) };
                    Err(format!("WaitForSingleObject failed: {error}"))
                }
                result => {
                    unsafe { CloseHandle(handle) };
                    Err(format!(
                        "WaitForSingleObject returned unexpected result {result}"
                    ))
                }
            }
        }
    }

    impl Drop for LaunchMutex {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0);
                CloseHandle(self.0);
            }
        }
    }

    fn single_instance_window_ready() -> bool {
        // These stable labels come from tauri-plugin-single-instance's documented target window
        // naming, using Cathulu's application identifier from tauri.conf.json.
        let class_name = wide(OsStr::new("app.cathulu.desktop-sic"));
        let window_name = wide(OsStr::new("app.cathulu.desktop-siw"));
        !unsafe { FindWindowW(class_name.as_ptr(), window_name.as_ptr()) }.is_null()
    }

    pub fn launch(executable: &Path, directory: &Path) -> Result<(), String> {
        let _lock = LaunchMutex::acquire()?;
        // Repeated MCP starts are common while Codex reloads a server. Leave an
        // existing dashboard exactly as the user left it instead of raising it.
        if single_instance_window_ready() {
            return Ok(());
        }
        let operation = wide(OsStr::new("open"));
        let executable = wide(executable.as_os_str());
        let directory = wide(directory.as_os_str());
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                executable.as_ptr(),
                std::ptr::null(),
                directory.as_ptr(),
                SW_SHOWNOACTIVATE,
            )
        };
        if result <= 32 {
            return Err(format!("Windows shell launch returned error {result}"));
        }

        let started_at = Instant::now();
        while started_at.elapsed() < WINDOW_READY_TIMEOUT {
            if single_instance_window_ready() {
                return Ok(());
            }
            sleep(Duration::from_millis(50));
        }
        Err("dashboard did not register its single-instance window within 8 seconds".into())
    }
}

#[cfg(windows)]
fn launch_dashboard_process(executable: &Path, directory: &Path) -> Result<(), String> {
    windows_dashboard_launch::launch(executable, directory)
}

#[cfg(not(windows))]
fn launch_dashboard_process(executable: &Path, directory: &Path) -> Result<(), String> {
    Command::new(executable)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn launch_dashboard() {
    let mcp_executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Cathulu MCP could not locate its executable for dashboard startup: {error}");
            return;
        }
    };
    let Some(directory) = mcp_executable.parent() else {
        eprintln!("Cathulu MCP executable has no parent directory; dashboard startup skipped");
        return;
    };

    #[cfg(windows)]
    let dashboard_name = "cathulu-desktop.exe";
    #[cfg(not(windows))]
    let dashboard_name = "cathulu-desktop";
    let dashboard_executable = directory.join(dashboard_name);
    if !dashboard_executable.is_file() {
        eprintln!(
            "Cathulu desktop executable was not found beside the MCP server at {}; MCP will continue without opening the dashboard",
            dashboard_executable.display()
        );
        return;
    }

    if let Err(error) = launch_dashboard_process(&dashboard_executable, directory) {
        eprintln!(
            "Cathulu MCP could not launch the desktop dashboard at {}: {error}",
            dashboard_executable.display()
        );
    }
}

#[tokio::main]
async fn main() {
    // MCP stdio must become ready independently of WebView or desktop startup.
    // Codex can restart an MCP process that has not completed initialization;
    // waiting here for the dashboard window can otherwise create a launch loop.
    if let Err(error) = std::thread::Builder::new()
        .name("cathulu-dashboard-launch".into())
        .spawn(launch_dashboard)
    {
        eprintln!("Cathulu MCP could not schedule dashboard startup: {error}");
    }

    if let Err(error) = cathulu_core::mcp::run_stdio().await {
        eprintln!("Cathulu MCP server stopped: {error}");
        std::process::exit(1);
    }
}
