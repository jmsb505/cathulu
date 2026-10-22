// Cathulu's desktop is a GUI in both development and release builds. Keeping
// the Windows subsystem consistent prevents debug launches from opening a
// console window beside the dashboard.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    cathulu_desktop_lib::run();
}
