//! Claude Code harness adapter core (PLAN-021 CC-04). Pure: builds the `claude -p` invocation,
//! scrubs the environment, and maps stream-json lines onto the CX-03 seam. It spawns nothing and
//! reads nothing from the process environment; the caller supplies the environment, the
//! transport and the Cathulu-owned config directory. Live use stays gated by the P-Q1 isolation
//! decision (`docs/parallel/claude/CC-04-adapter-evidence.md`) and by M51b/M52.

use crate::agent_provider::{
    ApprovalKind, CredentialSource, ExternalId, ExternalSession, ExternalTurn, FeatureSupport,
    IsolationCapabilities, McpToolConfig, ProviderCapabilities, ProviderError, ProviderEvent,
    ProviderEventKind, ProviderId, SessionConfig, TurnOutcome, UnavailableReason, WorkspaceAccess,
    CONTRACT_VERSION, MAX_PROMPT_BYTES, MAX_UNKNOWN_EXCERPT_BYTES,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::BufRead;

pub const ADAPTER_VERSION: &str = "claude-code-core-1";

/// The same list is passed as `--tools` (which built-in tools exist) and `--allowedTools` (which calls
/// run without a prompt). Read-only attempts get no write tools. Bash is never granted here: widening
/// it needs an Owner decision.
const READ_TOOLS: &str = "Read,Grep,Glob";
const WRITE_TOOLS: &str = "Read,Grep,Glob,Edit,Write";

/// Provider-selecting variables removed from the child environment. Every `ANTHROPIC_*` variable
/// is removed too, so an inherited API key cannot silently replace the subscription. The
/// `CLAUDE_CODE_USE_*` selectors route the child to another provider, so they are removed as well.
/// Re-check this list against the official environment-variable reference when the CLI changes.
const PROVIDER_ENV_KEYS: &[&str] = &[
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDE_CODE_USE_ANTHROPIC_AWS",
    "CLAUDE_CODE_USE_MANTLE",
];

/// Exit status Claude Code reports for SIGTERM. SIGTERM records no result (headless docs).
const SIGTERM_EXIT: i32 = 143;

/// Largest stream-json line the adapter accepts (CC-03 T9: the same 8 MiB bound as app-server RPC
/// lines). `read_bounded_line` enforces it before a line is buffered, and `map_line` checks it again.
pub const MAX_STREAM_LINE_BYTES: usize = 8 * 1024 * 1024;

pub struct Invocation {
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    /// Contains capability tokens in the server environment. The caller writes it to a
    /// owner-only file at `mcp_config_path` and never logs it.
    pub mcp_config_json: String,
}

/// Builds one `claude -p` invocation. The prompt is written to stdin by the caller.
/// `config_dir` is the Cathulu-owned `CLAUDE_CONFIG_DIR`; the login lives there, not in the user profile.
pub fn build_invocation(
    session: &SessionConfig,
    session_id: &ExternalId,
    config_dir: &str,
    mcp_config_path: &str,
    parent_env: &BTreeMap<String, String>,
) -> Result<Invocation, String> {
    session.validate()?;
    validate_uuid(session_id.as_str())?;
    if config_dir.is_empty() || mcp_config_path.is_empty() {
        return Err("config directory and MCP config path are required".to_string());
    }
    let tools = match session.permissions.access {
        WorkspaceAccess::ReadOnly => READ_TOOLS,
        WorkspaceAccess::WorkspaceWrite => WRITE_TOOLS,
    };
    let mut args: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--session-id",
        session_id.as_str(),
        "--permission-mode",
        "dontAsk",
        "--permission-prompts",
        "none",
        "--allowedTools",
        tools,
        "--tools",
        tools,
        "--disable-slash-commands",
        "--strict-mcp-config",
        "--mcp-config",
        mcp_config_path,
    ]
    .iter()
    .map(|value| value.to_string())
    .collect();
    if let Some(model) = &session.model {
        args.extend(["--model".to_string(), model.clone()]);
    }
    let mut env = scrub_environment(parent_env);
    env.insert("CLAUDE_CONFIG_DIR".to_string(), config_dir.to_string());
    env.insert(
        "CLAUDE_CODE_DISABLE_AUTO_MEMORY".to_string(),
        "1".to_string(),
    );
    env.insert(
        "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS".to_string(),
        "1".to_string(),
    );
    Ok(Invocation {
        args,
        env,
        mcp_config_json: mcp_config_json(&session.tools)?,
    })
}

/// Removes provider-selecting variables. Matching is case-insensitive because Windows env names are.
pub fn scrub_environment(parent: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    parent
        .iter()
        .filter(|(key, _)| {
            let upper = key.to_ascii_uppercase();
            !upper.starts_with("ANTHROPIC_") && !PROVIDER_ENV_KEYS.contains(&upper.as_str())
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn mcp_config_json(tools: &McpToolConfig) -> Result<String, String> {
    tools.validate()?;
    let mut server = serde_json::Map::new();
    server.insert("type".to_string(), json!("stdio"));
    server.insert("command".to_string(), json!(tools.command));
    server.insert("args".to_string(), json!(tools.args));
    server.insert("env".to_string(), json!(tools.env));
    serde_json::to_string(&json!({
        "mcpServers": { tools.server_name.clone(): Value::Object(server) }
    }))
    .map_err(|error| error.to_string())
}

fn validate_uuid(value: &str) -> Result<(), String> {
    let valid = value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        });
    if valid {
        Ok(())
    } else {
        Err("Claude session ID must be a UUID".to_string())
    }
}

/// Capabilities this core can honestly report before any preflight. Nothing is verified here:
/// isolation stays `Unverified` until a real install proves each class (CC-04 §3).
pub fn adapter_capabilities(harness_version: Option<&str>) -> ProviderCapabilities {
    ProviderCapabilities {
        contract_version: CONTRACT_VERSION,
        provider_id: ProviderId::claude(),
        adapter_version: ADAPTER_VERSION.to_string(),
        harness_version: harness_version.map(str::to_string),
        resume_session: FeatureSupport::Unverified,
        interrupt: FeatureSupport::Unsupported,
        status_query: FeatureSupport::Unsupported,
        structured_result: FeatureSupport::Supported,
        per_turn_model: FeatureSupport::Supported,
        isolation: IsolationCapabilities {
            settings: FeatureSupport::Unverified,
            plugins: FeatureSupport::Unverified,
            mcp_servers: FeatureSupport::Unverified,
            instructions: FeatureSupport::Unverified,
            memory: FeatureSupport::Unverified,
        },
        credential_source: CredentialSource::Unknown,
        max_prompt_bytes: MAX_PROMPT_BYTES,
        notes: vec![
            "isolation unverified: plugins, CLAUDE.md and auto memory have no documented off-switch without --bare".to_string(),
            "--bare is excluded: it requires an API key, which is silent billing for this adapter".to_string(),
        ],
    }
}

/// Maps the stream-json lines of one `claude -p` run. One process is one turn. A resumed session
/// runs many turns under the same session ID, so the turn ID is the attempt key (unique per attempt,
/// as `provider_attempts` requires). The attempt key is Cathulu-issued, not provider-issued.
pub struct StreamMapper {
    session: ExternalSession,
    turn_id: ExternalId,
    cathulu_server: String,
    initialized: bool,
    result: Option<StreamResult>,
}

/// The `result` line, held until `finish` so that the process exit status can still refuse it.
struct StreamResult {
    event_id: ExternalId,
    success: bool,
}

impl StreamMapper {
    /// Refuses any session that is not a Claude session, so no other provider's stream is mapped here.
    pub fn new(
        session: ExternalSession,
        turn_id: ExternalId,
        cathulu_server: &str,
    ) -> Result<Self, ProviderError> {
        if session.provider_id != ProviderId::claude() {
            return Err(ProviderError::Rejected {
                message: "the Claude stream mapper needs a Claude session".to_string(),
            });
        }
        Ok(Self {
            session,
            turn_id,
            cathulu_server: cathulu_server.to_string(),
            initialized: false,
            result: None,
        })
    }

    /// Maps one stream line. An `Err` is terminal for the attempt. The turn outcome is never returned
    /// here: `finish` releases it once the exit status is known.
    pub fn map_line(&mut self, line: &str) -> Result<Vec<ProviderEvent>, ProviderError> {
        if line.len() > MAX_STREAM_LINE_BYTES {
            return Err(oversized_line());
        }
        let value: Value =
            serde_json::from_str(line).map_err(|error| ProviderError::Uncertain {
                message: format!("truncated or malformed stream line: {error}"),
            })?;
        let event_id = value
            .get("uuid")
            .and_then(Value::as_str)
            .and_then(|id| ExternalId::new(id).ok());
        let events = match value.get("type").and_then(Value::as_str) {
            Some("system") => self.system(&value, event_id)?,
            Some("result") => self.result(&value, event_id)?,
            Some("assistant" | "user" | "stream_event") => Vec::new(),
            other => vec![self.event(
                event_id,
                ProviderEventKind::Unrecognized {
                    event_kind: other.unwrap_or("missing").chars().take(64).collect(),
                    size_bytes: line.len() as u64,
                    excerpt: excerpt(line),
                },
            )],
        };
        validated(events)
    }

    fn system(
        &mut self,
        value: &Value,
        event_id: Option<ExternalId>,
    ) -> Result<Vec<ProviderEvent>, ProviderError> {
        let subtype = value.get("subtype").and_then(Value::as_str);
        self.check_session(value, subtype == Some("init"))?;
        match subtype {
            Some("init") => self.init(value).map(|()| Vec::new()),
            Some("api_retry") => self.api_retry(value, event_id),
            Some("permission_denied") => Ok(vec![self.approval(value, event_id)?]),
            // The denial itself carries the tool-use identifier, so each one is read on its own.
            _ => Ok(Vec::new()),
        }
    }

    fn init(&mut self, value: &Value) -> Result<(), ProviderError> {
        if non_empty(value, "mcp_server_errors") {
            return Err(unavailable(
                UnavailableReason::Transport,
                None,
                "a --mcp-config entry was skipped; the role cannot use Cathulu tools",
            ));
        }
        if non_empty(value, "plugin_errors") {
            return Err(ProviderError::Rejected {
                message: "a plugin failed to load; isolation is not met".to_string(),
            });
        }
        if non_empty(value, "plugins") {
            return Err(ProviderError::Rejected {
                message: "plugins loaded; isolation is not met (CC-04 §3)".to_string(),
            });
        }
        let connected = value
            .get("mcp_servers")
            .and_then(Value::as_array)
            .is_some_and(|servers| {
                servers.iter().any(|server| {
                    server.get("name").and_then(Value::as_str) == Some(self.cathulu_server.as_str())
                        && server.get("status").and_then(Value::as_str) == Some("connected")
                })
            });
        if !connected {
            return Err(unavailable(
                UnavailableReason::Transport,
                None,
                "the Cathulu MCP server is not connected in this session",
            ));
        }
        self.initialized = true;
        Ok(())
    }

    /// A line that names a session must name the selected one. The init line must name one.
    fn check_session(&self, value: &Value, required: bool) -> Result<(), ProviderError> {
        match value.get("session_id").and_then(Value::as_str) {
            Some(id) if id == self.session.session_id.as_str() => Ok(()),
            Some(_) => Err(ProviderError::Rejected {
                message: "claude named a session other than the one selected".to_string(),
            }),
            None if required => Err(ProviderError::Rejected {
                message: "claude init line does not name its session".to_string(),
            }),
            None => Ok(()),
        }
    }

    fn api_retry(
        &mut self,
        value: &Value,
        event_id: Option<ExternalId>,
    ) -> Result<Vec<ProviderEvent>, ProviderError> {
        let category = value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let retry_after = value
            .get("retry_delay_ms")
            .and_then(Value::as_u64)
            .map(|ms| ms.div_ceil(1000));
        let message = format!("claude api error: {category}");
        match category {
            "rate_limit" | "overloaded" => Ok(vec![self.event(
                event_id,
                ProviderEventKind::CapacityObserved {
                    retry_after_seconds: retry_after,
                },
            )]),
            "authentication_failed" | "oauth_org_not_allowed" | "cloud_credential_error" => Err(
                unavailable(UnavailableReason::NotAuthenticated, None, &message),
            ),
            "account_on_hold" | "billing_error" => Err(unavailable(
                UnavailableReason::AccountRestricted,
                None,
                &message,
            )),
            "invalid_request" | "model_not_found" | "max_output_tokens" => {
                Err(ProviderError::Rejected { message })
            }
            "server_error" => Err(unavailable(UnavailableReason::Transport, None, &message)),
            _ => Ok(vec![self.event(
                event_id,
                ProviderEventKind::Unrecognized {
                    event_kind: "api_retry".to_string(),
                    size_bytes: value.to_string().len() as u64,
                    excerpt: None,
                },
            )]),
        }
    }

    fn result(
        &mut self,
        value: &Value,
        event_id: Option<ExternalId>,
    ) -> Result<Vec<ProviderEvent>, ProviderError> {
        if !self.initialized {
            return Err(uncertain("result arrived before a clean init line"));
        }
        if self.result.is_some() {
            return Err(uncertain(
                "a second result line arrived; the turn outcome is ambiguous",
            ));
        }
        self.check_session(value, false)?;
        // The outcome is state-mutating, so it needs a stable identity. Without one the turn cannot
        // be recorded reliably, and the attempt stays uncertain rather than dropping the result.
        let event_id = event_id.ok_or_else(|| {
            uncertain("result line has no stable identity; the turn outcome cannot be recorded")
        })?;
        let mut events = Vec::new();
        if let Some(denials) = value.get("permission_denials").and_then(Value::as_array) {
            for denial in denials {
                events.push(self.approval(denial, None)?);
            }
        }
        // A missing `is_error` counts as failure: the outcome must be proven, not assumed.
        let success = value.get("is_error").and_then(Value::as_bool) == Some(false)
            && value.get("subtype").and_then(Value::as_str) == Some("success");
        self.result = Some(StreamResult { event_id, success });
        Ok(events)
    }

    fn approval(
        &self,
        denial: &Value,
        event_id: Option<ExternalId>,
    ) -> Result<ProviderEvent, ProviderError> {
        // A denial must name the tool use it refused. Without that identifier neither the approval
        // nor its event identity is stable, so the attempt stays uncertain.
        let tool_use_id = denial
            .get("tool_use_id")
            .and_then(Value::as_str)
            .ok_or_else(|| uncertain("permission denial has no tool-use identifier"))?;
        let request_id =
            ExternalId::new(tool_use_id).map_err(|message| ProviderError::Uncertain {
                message: format!("permission denial has an invalid identifier: {message}"),
            })?;
        // The line's own identity wins. Otherwise the event takes its identity from the tool use,
        // so a replayed denial is recognised as the same event.
        let event_id =
            match event_id {
                Some(id) => id,
                None => ExternalId::new(format!("permission-denied:{tool_use_id}")).map_err(
                    |message| ProviderError::Uncertain {
                        message: format!("permission denial has an invalid identifier: {message}"),
                    },
                )?,
            };
        Ok(self.event(
            Some(event_id),
            ProviderEventKind::ApprovalRequested {
                session: self.session.clone(),
                request_id,
                request_kind: ApprovalKind::Other,
            },
        ))
    }

    /// Called once the process has ended, and the only place the turn outcome is released. Completion
    /// needs a success result and exit 0. A failed result with a known exit status is a failed turn.
    /// Everything else is `Uncertain`: no result, SIGTERM, or a result the exit status contradicts.
    pub fn finish(&self, exit_code: Option<i32>) -> Result<Vec<ProviderEvent>, ProviderError> {
        let Some(result) = &self.result else {
            return Err(uncertain(
                "claude ended without a result line; reconcile before any retry",
            ));
        };
        if exit_code == Some(SIGTERM_EXIT) {
            return Err(uncertain(
                "claude was terminated by SIGTERM; reconcile before any retry",
            ));
        }
        let outcome = match (result.success, exit_code) {
            (true, Some(0)) => TurnOutcome::Completed,
            (false, Some(_)) => TurnOutcome::Failed,
            _ => return Err(uncertain(
                "the result and exit status do not prove an outcome; reconcile before any retry",
            )),
        };
        validated(vec![self.event(
            Some(result.event_id.clone()),
            ProviderEventKind::TurnCompleted {
                turn: self.turn(),
                outcome,
            },
        )])
    }

    fn turn(&self) -> ExternalTurn {
        ExternalTurn {
            session: self.session.clone(),
            turn_id: self.turn_id.clone(),
        }
    }

    fn event(&self, event_id: Option<ExternalId>, event: ProviderEventKind) -> ProviderEvent {
        ProviderEvent {
            provider_id: ProviderId::claude(),
            event_id,
            sequence: None,
            event,
        }
    }
}

/// Reads one stream line from the child's stdout, refusing it as soon as it passes the cap. The cap
/// is checked before each chunk is appended, so an oversized line never grows the buffer past it.
/// Returns `Ok(None)` at the end of the stream.
pub fn read_bounded_line<R: BufRead>(reader: &mut R) -> Result<Option<String>, ProviderError> {
    let mut line: Vec<u8> = Vec::new();
    let mut ended = false;
    loop {
        let chunk = reader
            .fill_buf()
            .map_err(|error| uncertain(&format!("claude stream read failed: {error}")))?;
        if chunk.is_empty() {
            ended = true;
            break;
        }
        let newline = chunk.iter().position(|&byte| byte == b'\n');
        let take = newline.unwrap_or(chunk.len());
        if line.len() + take > MAX_STREAM_LINE_BYTES {
            return Err(oversized_line());
        }
        line.extend_from_slice(&chunk[..take]);
        reader.consume(take + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    if ended && line.is_empty() {
        return Ok(None);
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|_| uncertain("claude stream line is not valid UTF-8"))
}

/// Every event this adapter generates must pass the seam's own validation before it leaves the mapper.
fn validated(events: Vec<ProviderEvent>) -> Result<Vec<ProviderEvent>, ProviderError> {
    for event in &events {
        event.validate().map_err(|message| {
            uncertain(&format!(
                "generated claude event failed validation: {message}"
            ))
        })?;
    }
    Ok(events)
}

fn oversized_line() -> ProviderError {
    uncertain("claude stream line exceeds 8 MiB; the turn cannot be recorded")
}

fn non_empty(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
}

fn uncertain(message: &str) -> ProviderError {
    ProviderError::Uncertain {
        message: message.to_string(),
    }
}

fn unavailable(
    reason: UnavailableReason,
    retry_after: Option<u64>,
    message: &str,
) -> ProviderError {
    ProviderError::Unavailable {
        reason,
        retry_after_seconds: retry_after,
        message: message.to_string(),
    }
}

fn excerpt(line: &str) -> Option<String> {
    let mut end = line.len().min(MAX_UNKNOWN_EXCERPT_BYTES);
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    Some(line[..end].to_string())
}
