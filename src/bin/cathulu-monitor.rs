use cathulu_core::{default_data_dir, terminal, Store};
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

fn main() {
    if let Err(error) = run() {
        eprintln!("Cathulu monitor: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mode = match args.as_slice() {
        [] => "once",
        [mode] if mode == "--once" => "once",
        [mode] if mode == "--watch" => "watch",
        [mode] if mode == "--json" => "json",
        _ => return Err("usage: cathulu-monitor [--once|--watch|--json]".into()),
    };
    let store = Store::open_existing_read_only(default_data_dir()?)?;
    if mode == "watch" && !io::stdout().is_terminal() {
        return Err(
            "--watch requires an interactive terminal; use --once or --json in scripts".into(),
        );
    }
    let width = std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(80);
    let mut previous_output = None;
    loop {
        let snapshot = store.dashboard_snapshot()?;
        let output = if mode == "json" {
            serde_json::to_string_pretty(&snapshot).map_err(|error| error.to_string())? + "\n"
        } else {
            terminal::render(&snapshot, width)
        };
        if mode == "watch" && previous_output.as_ref() == Some(&output) {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }
        if mode == "watch" {
            print!("\x1b[H\x1b[2J");
        }
        print!("{output}");
        io::stdout().flush().map_err(|error| error.to_string())?;
        previous_output = Some(output);
        if mode != "watch" {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Ok(())
}
