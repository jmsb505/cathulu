# CC-04 — Claude Code adapter evidence

Agent: Claude Code lane. Status: **§1–§6 are the historical feasibility snapshot, written before F1 existed and kept for the record. §7 is the current pure-core state: the adapter core exists, is fixture-tested in the scratch crate, and is registered in `src/lib.rs` as a non-activating module.** Nothing in §1–§6 is observed behaviour of an installed CLI.
Baseline: branch `codex/manual-dual-m48`, HEAD `5fc5459` (CX-01 committed). CX-02 edits were uncommitted in the live tree at update time and were not reviewed here.
Documentation checked 2026-10-25: code.claude.com `headless`, `cli-reference`, `settings`, `memory`, `authentication`. The `env-vars` fetch was aborted, so environment-variable facts below come only from the other pages and are marked as such.

## 1. Local evidence

| Observation | Result |
| --- | --- |
| `claude` on PATH of this machine's Bash environment | **Not found** (`claude --version` fails). The running assistant is not evidence of an installed CLI usable by Cathulu. |
| Installed version, auth mode, subscription status | **Unknown.** Not inspected; installing or logging in is not authorized by this packet and is not done. |
| Everything below | **Documentation only.** The docs describe the latest release; the version an Owner has installed may be older. Each capability must be feature-detected at preflight (`system/init` carries version and capabilities). |

## 2. Documented facts relevant to an adapter

Credentials (authentication page, "Authentication precedence"):
- Order, highest first: cloud-provider variables (`CLAUDE_CODE_USE_BEDROCK`/`VERTEX`/`FOUNDRY`); `ANTHROPIC_AUTH_TOKEN`; `ANTHROPIC_API_KEY`; `apiKeyHelper` output; `CLAUDE_CODE_OAUTH_TOKEN`; profile/federation credentials; subscription OAuth from `/login`.
- **"In non-interactive mode (`-p`), the key is always used when present."** Interactive mode asks once; print mode does not. A subscription login does not override a key that is present.
- Named profile (`ANTHROPIC_PROFILE`) and federation variables rank above `/login`. An active profile in the Anthropic configuration directory ranks above `/login` only when it is `oidc_federation`; a `user_oauth` profile ranks below a working login. Profiles are not read in bare mode, Claude Desktop or cloud sessions, so non-bare `-p` runs do read them.
- `--bare`: never reads OAuth credentials, the keychain or `CLAUDE_CODE_OAUTH_TOKEN`; it needs `ANTHROPIC_API_KEY`, an `apiKeyHelper`, or a cloud-provider credential.
- `CLAUDE_CONFIG_DIR` (settings page and authentication page): sets a different home directory for settings, session history, plugins and `.credentials.json`. Each directory has its own login. A dedicated directory is therefore subscription-compatible, but its first use needs one interactive login by the Owner.

Settings and ambient configuration:
- Precedence (settings page): managed settings > `--settings` > `.claude/settings.local.json` > `.claude/settings.json` > `~/.claude/settings.json`. Environment variables resolve per pair, not by level. Project files are read from the session's primary working directory.
- `--setting-sources user,project,local` is a documented CLI flag. Managed settings are not listed as a source that can be excluded. Behaviour with an empty list is not documented.
- Without `--bare`, `-p` loads hooks in `.claude/settings.json`, `.mcp.json` servers, skills, subagents, installed plugins, auto memory and CLAUDE.md. It shows no trust dialog for those (headless page).
- CLAUDE.md: `~/.claude/CLAUDE.md` and `~/.claude/rules/` load without a trust prompt (memory page). `claudeMdExcludes` skips listed CLAUDE.md files by path or glob; a managed CLAUDE.md cannot be excluded. Whether `CLAUDE_CONFIG_DIR` relocates `~/.claude/CLAUDE.md` is not stated.
- Auto memory: `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`, or `"autoMemoryEnabled": false` in settings (memory page). Storage location: `autoMemoryDirectory`.
- MCP and commands: `--strict-mcp-config --mcp-config <file>` (only the supplied servers); `--disable-slash-commands`.
- Plugins: beyond "`CLAUDE_CONFIG_DIR` stores plugins there", no disabling control was found in the pages read. Repository files can declare `enabledPlugins` (cloud section of the settings page). The non-bare behaviour is **not verified**.
- Background work: a `-p` run can wait up to 10 minutes of idle time for background commands, subagents, monitors or wakeups (`CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS`). `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS` turns background features off (headless page, bare-mode list; the variable's own page was not fetched).

Process and output (headless and CLI reference pages):
- Exit 0 on success, non-zero on failure. In-run failures such as missing authentication are printed as the result on stdout, so classify by content as well as exit code.
- `--output-format stream-json --verbose`: the last line is a `result` message. `system/init` is first and carries model, tools, MCP servers, plugins, `capabilities`, `plugin_errors`, `mcp_server_errors`. `system/api_retry` carries `error` categories (`authentication_failed`, `oauth_org_not_allowed`, `account_on_hold`, `billing_error`, `rate_limit`, `overloaded`, `invalid_request`, `model_not_found`, `server_error`, `max_output_tokens`, `cloud_credential_error`, `unknown`).
- SIGTERM exits 143, leaves the turn unfinished and records no result. SIGINT ends the turn cleanly.
- Permissions: `--permission-prompts none` (v2.1.259+) denies requests that would prompt, without waiting. `--permission-mode dontAsk` denies anything that would prompt. `--allowedTools` uses rule syntax with prefix matching. `--dangerously-skip-permissions` is never used by this plan. A `-p` run starts in Manual mode from v2.1.302 when nothing sets a mode.
- Sessions: `--session-id <uuid>` lets the caller choose the id before submission. `--resume` and `--continue` exist; `--no-session-persistence` makes resume impossible.

## 3. What the documented facts change for P-Q1 (subscription-safe isolation)

1. **Credential risk is documented, not hypothetical.** An inherited `ANTHROPIC_API_KEY` is always used in `-p` mode, which bills the API rather than the subscription. The adapter must not pass these into the child environment: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_USE_BEDROCK`/`VERTEX`/`FOUNDRY`, `ANTHROPIC_PROFILE`, `ANTHROPIC_FEDERATION_RULE_ID`, `ANTHROPIC_ORGANIZATION_ID`, `ANTHROPIC_BASE_URL` (unless the Owner names an endpoint), and `CLAUDE_CODE_OAUTH_TOKEN` (unless intended). Two sources cannot be removed by environment scrubbing alone: an `apiKeyHelper` set in a settings file (its allowed scope in repository files was not verified), and an active profile file in the Anthropic configuration directory (`%APPDATA%\Anthropic` on Windows). Both need a preflight check or a documented exclusion.
2. **A candidate recipe built only from documented pieces** (not verified on an install):
   - `CLAUDE_CONFIG_DIR` set to a Cathulu-owned directory, where the Owner logs in once with the subscription;
   - the scrubbed credential environment from item 1;
   - `--strict-mcp-config --mcp-config <Cathulu-only file>` and `--disable-slash-commands`;
   - `--settings` JSON with `claudeMdExcludes` covering the repository's CLAUDE.md and AGENTS.md files, and `"autoMemoryEnabled": false`, plus `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`;
   - `--permission-prompts none` with an explicit `--allowedTools` allowlist; no bypass flag;
   - `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS` so a turn cannot wait on background work;
   - a `--session-id` chosen by Cathulu before submission.
3. **Still open after that recipe:** plugins under the dedicated directory and any repository-declared `enabledPlugins`; whether `CLAUDE_CONFIG_DIR` moves `~/.claude/CLAUDE.md`; the behaviour of an empty `--setting-sources`; repository-level `.claude/settings.json` hooks, which the headless page says run under `-p` without a trust prompt. Only `--bare` is documented to skip hooks outright, and `--bare` is excluded for subscription use.
4. **Verdict.** Isolation stays **Unverified**. Under CC-03 T5 the adapter is ineligible for write or high-risk work until a real install shows that the recipe removes plugin, CLAUDE.md, hook and project-config influence and that the credential source is the subscription. Read-only review use is a separate Owner decision once that risk is accepted.
5. **Owner actions for P-Q1** (not performed here): install the Claude Code CLI on this machine, and log in once inside the dedicated `CLAUDE_CONFIG_DIR`. Both need Owner approval.
6. **Lost acknowledgement.** Because Cathulu can choose the session id, a lost acknowledgement can be resolved by observing whether a transcript exists for that id, rather than by guessing. Transcript location stability is not verified. The `--resume` documentation accepts a `.jsonl` transcript path, so the location is at least documented for resume.

## 4. Mapping into the CC-03 seam (to be coded against real F1 types)

| Claude Code signal | Seam value |
| --- | --- |
| `system/init` with `mcp_server_errors` non-empty, or the Cathulu server absent or not connected | `Unavailable{Transport}` before any work (equivalent of `require_cathulu_tools`, `app_server.rs:157-171`) |
| `system/init` `plugin_errors`, or unexpected `plugins` or `mcp_servers` | isolation not met ⇒ refuse the attempt (`Rejected`) |
| `api_retry.error` = `authentication_failed`, `oauth_org_not_allowed`, `cloud_credential_error` | `Unavailable{NotAuthenticated}` |
| `account_on_hold`, `billing_error` | `Unavailable{AccountRestricted}`: not fixed by waiting, never queued for retry. Amendment already applied to CC-03 §3. |
| `rate_limit`, `overloaded` | `Unavailable{Capacity{retry_after = retry_delay_ms}}` |
| `server_error`, no-response retries | `Unavailable{Transport}` |
| `invalid_request`, `model_not_found`, `max_output_tokens` | `Rejected{reason}` |
| `result` with `permission_denials` | approval-equivalent event: Decline recorded; attempt `blocked` |
| missing `result`, exit 143, stream ends mid-turn | `Uncertain` ⇒ `needs_reconcile`; never retry under the same attempt key |
| exit 0 with `result` | `TurnCompleted(Completed)`: result available only; success still needs Cathulu's report, QA and review |
| unknown JSON event type | `Unrecognized{kind,size}` |
| credential source other than the subscription (detected or unverifiable) | `Unavailable{NotAuthenticated}` for routing; a scrubbed environment is the control until the source can be observed |

## 5. Fixtures (synthetic, no live call; implemented in the scratch crate)

Success stream (init, assistant, result); truncated stream (no `result`); malformed JSON line; oversized line; `api_retry` for each error category; authentication failure delivered as a stdout result with non-zero exit; `permission_denials` populated; MCP server skipped (`mcp_server_errors`); plugin load error; SIGTERM-style end (no result, 143); unknown session on resume; duplicate or out-of-order result. Each asserts the mapping above and that no fixture can yield task success. Add one fixture where an `ANTHROPIC_API_KEY` is present in the parent environment and the adapter must strip it.

Status (2026-10-25): implemented, and 27 of 27 pass (§7). The unknown session on resume is covered as a mismatched init identity. An authentication failure delivered as a stdout result is `Failed` by its exit and `is_error`; the result text is not classified (§7).

## 6. Not done and unverified

- No adapter code and no adapter compile. The only Rust built from `target/manual-dev/claude/` is the CX-01 review reproduction in `target/manual-dev/claude/scratch/`, which runs against the committed `5fc5459` snapshot. It is not an adapter.
- No installed `claude`, no preflight, no live task. The M53 activation prerequisites (M51b, M52, P-Q1, disclosure, scope) are all open.
- Unverified: plugin isolation in non-bare mode; whether `CLAUDE_CONFIG_DIR` moves `~/.claude/CLAUDE.md`; empty `--setting-sources`; managed settings that cannot be excluded; the allowed scope of `apiKeyHelper` in repository files; how to observe the active credential source; repository hooks under `-p` (documented, not observed); Windows process-tree and console behaviour for `claude.exe`; whether `stream-json` input gives a durable submit receipt.
- The `env-vars` page fetch was aborted. Environment-variable facts come from the other pages only.
- Documentation can change; this record is dated 2026-10-25.
- `--restricted` (installed 2.1.302 help) removes Bash, the other code-running tools and WebFetch unless `--tools` names them, ignores user, project and local settings files, and confines file tools to the working directories. It is not adopted here. Isolation stays `Unverified` until its effect is observed.
- Whether `dontAsk` lets the role call the Cathulu MCP tools is unverified. `--allowedTools` names only the built-in tools, not `mcp__cathulu__*`. Not observed, not changed.

## 7. CC-04 core status (2026-10-25): pure adapter core, fixture-tested, registered as non-activating

Source: `src/claude_code.rs` (Claude-owned; registered in `lib.rs` as a pure, non-activating module in Codex's uncommitted candidate, with its tests in `tests/claude_adapter.rs`; Codex owns registration and commit). Scratch consumer: `target/manual-dev/claude/scratch`, which includes the file in place by `#[path]` and depends on the checkout's `cathulu-core` by path (`../../../..`). It was last verified at HEAD `0d33295`; the accepted seam is `09f192b`.

What it does, and nothing more:
- `build_invocation`: a `claude -p` argv with stream-json output, a UUID session ID, `dontAsk` plus `--permission-prompts none`, read-only or write tool allowlists (no Bash) passed as both `--tools` (which built-in tools exist; `--tools` is in the installed 2.1.302 help, checked with `claude --help`, no model call) and `--allowedTools` (which calls run without a prompt), `--strict-mcp-config` (MCP servers only; it does not prove broader configuration or plugin isolation), `--disable-slash-commands`, and a Cathulu-only MCP config returned as JSON for an owner-only file. Refuses `--bare` and any bypass by construction.
- `scrub_environment`: removes every `ANTHROPIC_*` variable and the provider selectors, case-insensitively. Sets `CLAUDE_CONFIG_DIR` to the Cathulu-owned directory, plus auto-memory and background-task disables.
- `StreamMapper`: maps stream-json lines to `ProviderEvent`s. Init refuses MCP errors, plugin errors, loaded plugins and an unconnected Cathulu server. Capacity retries become observations. Auth, billing, server and request errors become terminal, with distinct reasons. Permission denials become approval requests. Truncated lines are `Uncertain`, and a missing `is_error` counts as failed.
- Turn outcome is buffered (review of 2026-10-25, Codex candidate findings). `map_line` never returns `TurnCompleted`. `finish(exit_code)` releases it: `Completed` needs a success result and exit 0. A failed result with a known exit is `Failed`. No result, SIGTERM (143), a success result with a nonzero or unknown exit, and a second result line are `Uncertain`. A nonzero exit therefore cannot be accepted as completion.
- Bounds (CC-03 T9). `MAX_STREAM_LINE_BYTES` is 8 MiB. `read_bounded_line` refuses an oversized line before it is buffered past the cap, and `map_line` refuses it before parsing. Invalid UTF-8 is `Uncertain`.
- Identity (same review). A session that is not Claude is refused at construction. `init` must name the selected session, and any line that names a session must name it; a mismatch is `Rejected`. Every event the mapper generates passes `ProviderEvent::validate` before it leaves.
- Stable identity for state-mutating events (review of 2026-10-25). A result takes its line's `uuid`. A permission denial takes its line's `uuid` or, failing that, `permission-denied:<tool_use_id>`. A result with no stable identity, and a denial with no `tool_use_id`, return `Uncertain`, so no outcome is dropped while `finish` reports success. Non-mutating observations, such as capacity retries and unrecognized lines, may stay unidentified.
- `adapter_capabilities`: reports every isolation class `Unverified` and interrupt/status `Unsupported`. Nothing is claimed beyond what is documented.

Not done, deliberately:
- No process spawn, transport, stdin prompt write, or `AgentProvider` implementation. Those belong to the live adapter after the P-Q1 decision, M51b and M52.
- The turn ID is the Cathulu attempt key, not the session ID. One `claude -p` process is one turn, but a resumed session runs many turns under one session ID, and `provider_attempts` requires turn IDs to be unique per session. Using the session ID would make the second turn collide with the first. The attempt key is Cathulu-issued, not provider-issued. (Corrected 2026-10-25 after Codex's v21 schema review.)
- The allowlist is a proposed default. Bash stays excluded until the Owner decides.
- Authentication by result text. §2 says in-run authentication failures reach stdout as the result, but the evidence does not pin that text or a status field. A failed result is `Failed` by its exit and `is_error`, and no text matching is implemented. Authentication observations come from `api_retry` lines or preflight.

Verification (2026-10-25, after Codex's CX-07 review, at HEAD `0d33295`): `cargo test --locked --offline --manifest-path target/manual-dev/claude/scratch/Cargo.toml --tests` passes the adapter suite 27 of 27 with no warnings, against the checkout's `cathulu-core` at HEAD `0d33295`. The current-head log is `target/manual-dev/claude/cx07-current-head-0d33295.log`. The new fixture checks that `--tools` carries the approved list in both modes, once, without Bash. The fixtures cover: a nonzero or unknown exit never completing a success result; an authentication failure printed as a stdout result, with exit 1 or 0, as `Failed`; SIGTERM after a failed result; a second result line; a result with no stable identity; a denial with no tool-use identifier; a permission-denied line taking its identity from its own `uuid` or else from its tool use; an oversized line refused by `map_line` and by `read_bounded_line`, with a line exactly at the cap accepted; the reader joining split chunks and stripping `\r`; a non-Claude session refused at construction; a mismatched or unnamed init identity and a mismatched result identity, each `Rejected`; and generated events passing `ProviderEvent::validate`. `rustfmt --check` is clean on `src/claude_code.rs` and the adapter test. No provider was called, nothing was installed, and no credential or configuration was changed. Registration candidate (2026-10-25, Codex, uncommitted, based on `646cb03`): `cargo fmt --all -- --check`, `cargo check --offline --all-targets` and `cargo test --offline --all-targets` pass (236/236) with both dispatch flags false. The scratch suites pass 66/66 (adapter 27/27, policy 31/31, existing suites 2/2 and 6/6). Neither module is called by dispatcher, persistence, MCP routing or provider-start code. Dispatch stays off, isolation stays `Unverified`, and credential source stays `Unknown`.

## 8. Install record (2026-10-25, Owner-approved install)

- Installed Claude Code **2.1.302** on this Windows machine with `winget install Anthropic.ClaudeCode`. WinGet verified the installer hash. Chosen over the PowerShell script because it doesn't pipe a downloaded script into PowerShell. WinGet doesn't auto-update, so upgrades are manual (`winget upgrade Anthropic.ClaudeCode`).
- Binary: `C:\Users\sjuan\AppData\Local\Microsoft\WinGet\Packages\Anthropic.ClaudeCode_Microsoft.Winget.Source_8wekyb3d8bbwe\claude.exe`. The PATH entry takes effect in a new shell.
- Created an empty Cathulu-owned config folder outside the repo: `C:\Users\sjuan\AppData\Local\Cathulu\claude-config`. `claude doctor` against it reports no installation issues, and it isn't signed in (expected).
- Not done: login. That stays with the Owner, in a fresh PowerShell window, with `ANTHROPIC_BASE_URL` removed. This session's environment sets `ANTHROPIC_BASE_URL` and several `CLAUDE_CODE_*` variables, so the login must not run from it.
- Isolation is still unverified. No model call has been made.
