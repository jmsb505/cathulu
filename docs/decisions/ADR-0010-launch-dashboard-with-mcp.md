# ADR-0010 — Launch the desktop dashboard with MCP

- **Status:** Accepted
- **Date:** 2026-10-22
- **Decision owners:** Cathulu project owner

## Context

Codex starts the local Cathulu MCP process as a stdio server. That process and the Tauri dashboard are separate executables. A working MCP connection therefore does not make an observability window visible. The configured MCP executable is installed in a dedicated release directory, so a dashboard built from another checkout or target directory could read the same database with incompatible schema assumptions.

## Decision

The MCP executable looks for `cathulu-desktop` beside itself and schedules its launch on a named background thread while RMCP stdio starts immediately. Desktop/WebView startup and the bounded Windows readiness wait must never delay MCP initialization: Codex may restart an MCP process that has not completed its stdio handshake, which can otherwise make the dashboard repeatedly appear and disappear. On Windows the launch thread serializes MCP-triggered launches with a short-lived named mutex and uses `ShellExecuteW` to open the GUI independently of MCP pipe handles. Other desktop targets launch with null standard streams. The dashboard inherits the MCP environment, including any `CATHULU_DATA_DIR` override. If scheduling, the sibling lookup, startup synchronization, or the app launch fails, MCP reports the issue to stderr and continues normally.

The desktop registers Tauri's single-instance plugin with a no-op callback so a repeated MCP start cannot raise the existing window over the user's active application. On Windows, the MCP checks for the registered single-instance window while holding the launch mutex and returns immediately if it already exists. Otherwise it uses `ShellExecuteW` with `SW_SHOWNOACTIVATE` to open the first dashboard in the background. The main Tauri window also sets `focus: false`. The user can bring the dashboard forward from the taskbar when needed. The release MCP and desktop executables are built from the same source tree and placed together in the configured directory. Direct Cargo release builds enable the desktop crate's `custom-protocol` feature so Tauri embeds `frontendDist` rather than trying to load the development server URL.

The Tauri setup hook must not synchronously reconcile repositories before the window is created. It manages the local store first, then schedules the Git scan on Tauri's blocking worker pool. MCP startup does not repeat that full scan. On Windows, background Git and Codex CLI child processes use `CREATE_NO_WINDOW` so repository inspection does not open terminal windows.

### Release refresh path

Codex uses the stable `target/mcp-current/cathulu-mcp.exe` path. `scripts/build-current.ps1` stages the MCP and desktop from one source fingerprint, verifies both hashes, stores the pair in a versioned directory, and advances the Windows junction at that stable path. The desktop remains beside its matching MCP binary. This avoids replacing an executable currently open in Windows and avoids editing Codex configuration for every build. Running MCP processes retain their loaded code; a new build takes effect in a session after its Cathulu MCP process is restarted. The build script never restarts Codex or MCP.

## Consequences

- Loading the configured local MCP also displays Cathulu's read-only dashboard.
- MCP protocol stdout remains isolated from the desktop process, which is launched independently from its stdio pipes on Windows.
- Desktop startup failure is not a dependency of MCP availability.
- MCP stdio initialization is not delayed by dashboard or WebView startup.
- The local desktop uses the same data directory and source revision as MCP.
- Multiple Codex sessions reuse one dashboard window without activating, showing, restoring, or focusing it on every MCP start.
- Near-simultaneous MCP restarts do not race the first Tauri single-instance registration.
- A standalone MCP binary without its sibling desktop continues to work with a stderr diagnostic.
- Slow repository inspection cannot block initial dashboard rendering or MCP stdio initialization, and background Git commands do not create visible console windows on Windows.

## Alternatives considered

- **Keep MCP and desktop fully separate:** rejected because the user needs the dashboard to appear automatically for workflow observability.
- **Launch the older debug executable:** rejected because its source revision may not support the current local schema.
- **Make dashboard startup mandatory:** rejected because optional UI failure must not block the MCP protocol.
- **Start another desktop window for every Codex session:** rejected because it creates duplicate windows and obscures the single local observability surface.
