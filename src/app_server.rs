//! Local Codex app-server stdio client.
//!
//! The client only starts/resumes persistent threads and submits bounded turns. It does not
//! keep a model generation alive while Cathulu waits for project work.

use crate::agent_provider::ProviderStartAdmission;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;
use toml_edit::{DocumentMut, Value as TomlValue};

const MAX_CODEX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_CONFIGURED_MCP_SERVERS: usize = 128;

const MAX_RPC_LINE_BYTES: usize = 8 * 1024 * 1024;
const RPC_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug)]
pub enum AppServerMessage {
    Notification {
        method: String,
        params: Value,
    },
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    TransportClosed,
    TransportError(String),
}

#[derive(Debug)]
enum IncomingMessage {
    Response {
        id: u64,
        result: Option<Value>,
        error: Option<Value>,
    },
    Event(AppServerMessage),
}

pub struct AppServerProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    incoming: Receiver<IncomingMessage>,
    next_request_id: u64,
    deferred_responses: HashMap<u64, Result<Value, String>>,
    events: Vec<AppServerMessage>,
    admission: ProviderStartAdmission,
}

impl AppServerProcess {
    pub fn start(admission: &ProviderStartAdmission) -> Result<Self, String> {
        admission.ensure_valid(crate::unix_time())?;
        let mut command = app_server_command()?;
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // The protocol carries errors over stdout. Do not create or flash a console window.
            .stderr(Stdio::null());
        crate::suppress_windows_console(&mut command);

        let mut child = command
            .spawn()
            .map_err(|error| format!("cannot start the local Codex app-server: {error}"))?;
        let Some(stdin) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Codex app-server stdin pipe was not created".to_string());
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Codex app-server stdout pipe was not created".to_string());
        };
        let (sender, incoming) = mpsc::channel();
        thread::Builder::new()
            .name("cathulu-app-server-reader".into())
            .spawn(move || read_messages(BufReader::new(stdout), sender))
            .map_err(|error| format!("cannot start the app-server reader: {error}"))?;

        let mut process = Self {
            child,
            stdin: Some(stdin),
            incoming,
            next_request_id: 1,
            deferred_responses: HashMap::new(),
            events: Vec::new(),
            admission: admission.clone(),
        };
        process.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "cathulu",
                    "title": "Cathulu Local Role Dispatcher",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": { "experimentalApi": true }
            }),
        )?;
        process.notify("initialized", json!({}))?;
        Ok(process)
    }

    pub(crate) fn set_admission(
        &mut self,
        admission: ProviderStartAdmission,
    ) -> Result<(), String> {
        admission.ensure_valid(crate::unix_time())?;
        self.admission = admission;
        Ok(())
    }

    pub fn start_thread(
        &mut self,
        cwd: &Path,
        model: Option<&str>,
        approval_policy: &str,
        sandbox: &str,
        config: Value,
    ) -> Result<String, String> {
        self.admission.ensure_workspace_root(cwd)?;
        let mut params = json!({
            "cwd": cwd,
            "approvalPolicy": approval_policy,
            "sandbox": sandbox,
            "config": config
        });
        if let Some(model) = model {
            params["model"] = Value::String(model.to_string());
        }
        let result = self.request("thread/start", params)?;
        result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= 256)
            .map(str::to_string)
            .ok_or_else(|| "Codex app-server returned no persistent thread ID".to_string())
    }

    pub fn resume_thread(&mut self, thread_id: &str, config: Value) -> Result<(), String> {
        self.admission.ensure_valid(crate::unix_time())?;
        if thread_id.is_empty() || thread_id.len() > 256 {
            return Err("stored Codex thread ID is invalid".to_string());
        }
        let result = self.request(
            "thread/resume",
            json!({ "threadId": thread_id, "config": config }),
        )?;
        let resumed_id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| "Codex app-server returned no resumed thread ID".to_string())?;
        if resumed_id != thread_id {
            return Err("Codex app-server resumed a different thread".to_string());
        }
        Ok(())
    }

    /// Verify that this specific thread has a connected Cathulu MCP catalog before it is asked
    /// to act. A global server inventory is insufficient because tool access can differ by CWD.
    pub fn require_cathulu_tools(
        &mut self,
        thread_id: &str,
        required_tools: &[&str],
    ) -> Result<(), String> {
        let result = self.request(
            "mcpServerStatus/list",
            json!({
                "threadId": thread_id,
                "serverName": "cathulu",
                "detail": "full"
            }),
        )?;
        validate_cathulu_tools(&result, required_tools)
    }

    /// Submit one turn and return on the app-server acknowledgement, never on turn completion.
    pub fn start_turn(&mut self, thread_id: &str, prompt: &str) -> Result<Value, String> {
        self.admission.ensure_valid(crate::unix_time())?;
        if thread_id.is_empty() || thread_id.len() > 256 {
            return Err("stored Codex thread ID is invalid".to_string());
        }
        if prompt.trim().is_empty() || prompt.len() > 64 * 1024 {
            return Err("role assignment prompt must be non-empty and at most 64 KiB".to_string());
        }
        self.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{ "type": "text", "text": prompt }]
            }),
        )
    }

    pub fn next_event(&mut self, timeout: Duration) -> Result<Option<AppServerMessage>, String> {
        if !self.events.is_empty() {
            return Ok(Some(self.events.remove(0)));
        }
        match self.incoming.recv_timeout(timeout) {
            Ok(IncomingMessage::Event(event)) => Ok(Some(event)),
            Ok(IncomingMessage::Response { id, result, error }) => {
                let response = decode_response(result, error);
                self.deferred_responses.insert(id, response);
                Ok(None)
            }
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err("Codex app-server output stream closed".to_string())
            }
        }
    }

    /// Respond to a server-initiated request. Callers must apply an explicit policy; this client
    /// intentionally never auto-approves tool, file, permission, or elicitation requests.
    pub fn respond(&mut self, request_id: Value, result: Value) -> Result<(), String> {
        self.write_message(&json!({ "id": request_id, "result": result }))
    }

    /// Reject an unsupported or unapproved server request without interpreting it as consent.
    pub fn reject_request(
        &mut self,
        request_id: Value,
        code: i64,
        message: &str,
    ) -> Result<(), String> {
        self.write_message(&json!({
            "id": request_id,
            "error": { "code": code, "message": message }
        }))
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or_else(|| "Codex app-server request ID exhausted".to_string())?;
        if let Some(response) = self.deferred_responses.remove(&id) {
            return response;
        }
        self.write_message(&json!({ "id": id, "method": method, "params": params }))?;
        loop {
            match self.incoming.recv_timeout(RPC_TIMEOUT) {
                Ok(IncomingMessage::Response {
                    id: response_id,
                    result,
                    error,
                }) if response_id == id => return decode_response(result, error),
                Ok(IncomingMessage::Response {
                    id: response_id,
                    result,
                    error,
                }) => {
                    self.deferred_responses
                        .insert(response_id, decode_response(result, error));
                }
                Ok(IncomingMessage::Event(event)) => self.events.push(event),
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("Codex app-server request `{method}` timed out"));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(format!("Codex app-server closed during `{method}`"));
                }
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.write_message(&json!({ "method": method, "params": params }))
    }

    fn write_message(&mut self, value: &Value) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "Codex app-server stdin is closed".to_string())?;
        serde_json::to_writer(&mut *stdin, value)
            .map_err(|error| format!("cannot encode Codex app-server message: {error}"))?;
        stdin
            .write_all(b"\n")
            .and_then(|_| stdin.flush())
            .map_err(|error| format!("cannot write to Codex app-server: {error}"))
    }
}

impl Drop for AppServerProcess {
    fn drop(&mut self) {
        // Closing stdin is the graceful shutdown signal. A short-lived kill is the fallback so
        // an MCP shutdown can never leave a hidden app-server process behind.
        drop(self.stdin.take());
        for _ in 0..20 {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn app_server_command() -> Result<Command, String> {
    // Use Codex's executable directly. A shell wrapper can flash a console and makes config
    // overrides depend on shell quoting. The Windows Codex installation supplies codex.exe.
    #[cfg(windows)]
    let mut command = Command::new("codex.exe");
    #[cfg(not(windows))]
    let mut command = Command::new("codex");

    command.args(["app-server", "--listen", "stdio://"]);
    for override_value in configured_mcp_disable_overrides()? {
        command.arg("--config").arg(override_value);
    }
    Ok(command)
}

fn configured_mcp_disable_overrides() -> Result<Vec<String>, String> {
    let codex_home = env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            env::var_os("USERPROFILE").map(|path| std::path::PathBuf::from(path).join(".codex"))
        })
        .or_else(|| env::var_os("HOME").map(|path| std::path::PathBuf::from(path).join(".codex")));
    let Some(config_path) = codex_home.map(|path| path.join("config.toml")) else {
        return Ok(Vec::new());
    };
    let metadata = match fs::metadata(&config_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "cannot inspect Codex config for MCP isolation: {error}"
            ))
        }
    };
    if !metadata.is_file() || metadata.len() > MAX_CODEX_CONFIG_BYTES {
        return Err(
            "Codex config for MCP isolation must be a regular file no larger than 1 MiB"
                .to_string(),
        );
    }
    let contents = fs::read_to_string(&config_path)
        .map_err(|error| format!("cannot read Codex config for MCP isolation: {error}"))?;
    mcp_server_disable_overrides(&contents)
}

fn mcp_server_disable_overrides(config: &str) -> Result<Vec<String>, String> {
    let document = DocumentMut::from_str(config).map_err(|_| {
        "Codex config is invalid TOML; refusing to start the role dispatcher".to_string()
    })?;
    let Some(servers) = document.as_table().get("mcp_servers") else {
        return Ok(Vec::new());
    };
    let names = if let Some(table) = servers.as_table() {
        table
            .iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>()
    } else if let Some(table) = servers.as_value().and_then(TomlValue::as_inline_table) {
        table
            .iter()
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>()
    } else {
        return Err(
            "Codex mcp_servers config is not a table; refusing to start the role dispatcher"
                .to_string(),
        );
    };
    if names.len() > MAX_CONFIGURED_MCP_SERVERS {
        return Err(
            "Codex config lists too many MCP servers for safe role-dispatch isolation".to_string(),
        );
    }

    names
        .into_iter()
        .map(|name| {
            if name.is_empty() || name.len() > 256 {
                return Err("Codex config contains an invalid MCP server name".to_string());
            }
            // TOML-quote the key segment. Command::arg passes this as one argument; no shell
            // interprets server names or values from the user's config.
            let quoted_name = TomlValue::from(name).to_string();
            Ok(format!("mcp_servers.{quoted_name}.enabled=false"))
        })
        .collect()
}

fn decode_response(result: Option<Value>, error: Option<Value>) -> Result<Value, String> {
    if let Some(error) = error {
        let code = error
            .get("code")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Codex app-server returned an RPC error");
        return Err(format!("Codex app-server RPC {code}: {message}"));
    }
    result.ok_or_else(|| "Codex app-server returned an empty RPC result".to_string())
}

fn validate_cathulu_tools(result: &Value, required_tools: &[&str]) -> Result<(), String> {
    let servers = result
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "Codex app-server returned an invalid MCP status catalog".to_string())?;
    let server = servers
        .iter()
        .find(|server| server.get("name").and_then(Value::as_str) == Some("cathulu"))
        .ok_or_else(|| "the role thread does not have the Cathulu MCP server".to_string())?;
    if server.get("runtimeStatus").and_then(Value::as_str) != Some("connected") {
        return Err("the role thread's Cathulu MCP server is not connected".to_string());
    }
    let tools = server
        .get("tools")
        .and_then(Value::as_object)
        .ok_or_else(|| "the role thread's Cathulu MCP catalog is unavailable".to_string())?;
    if let Some(missing) = required_tools
        .iter()
        .find(|tool| !tools.contains_key(**tool))
    {
        return Err(format!(
            "the role thread is missing required Cathulu tool `{missing}`"
        ));
    }
    Ok(())
}

fn read_messages<R: BufRead>(mut reader: R, sender: Sender<IncomingMessage>) {
    loop {
        let mut line = Vec::new();
        match read_bounded_line(&mut reader, &mut line) {
            Ok(0) => {
                let _ = sender.send(IncomingMessage::Event(AppServerMessage::TransportClosed));
                break;
            }
            Ok(_) => match parse_incoming(&line) {
                Ok(Some(message)) => {
                    if sender.send(message).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = sender.send(IncomingMessage::Event(AppServerMessage::TransportError(
                        error,
                    )));
                    break;
                }
            },
            Err(error) => {
                let _ = sender.send(IncomingMessage::Event(AppServerMessage::TransportError(
                    error.to_string(),
                )));
                break;
            }
        }
    }
}

fn read_bounded_line<R: BufRead>(reader: &mut R, line: &mut Vec<u8>) -> io::Result<usize> {
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(line.len());
        }
        let count = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|position| position + 1)
            .unwrap_or(available.len());
        if line.len().saturating_add(count) > MAX_RPC_LINE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Codex app-server message exceeds the 8 MiB limit",
            ));
        }
        line.extend_from_slice(&available[..count]);
        reader.consume(count);
        if line.last() == Some(&b'\n') {
            return Ok(line.len());
        }
    }
}

fn parse_incoming(line: &[u8]) -> Result<Option<IncomingMessage>, String> {
    let value: Value = serde_json::from_slice(line)
        .map_err(|error| format!("Codex app-server sent invalid JSON: {error}"))?;
    if let Some(method) = value.get("method").and_then(Value::as_str) {
        let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
        if let Some(id) = value.get("id") {
            return Ok(Some(IncomingMessage::Event(AppServerMessage::Request {
                id: id.clone(),
                method: method.to_string(),
                params,
            })));
        }
        return Ok(Some(IncomingMessage::Event(
            AppServerMessage::Notification {
                method: method.to_string(),
                params,
            },
        )));
    }
    let id = value.get("id").and_then(Value::as_u64).ok_or_else(|| {
        "Codex app-server returned a message without a numeric request ID".to_string()
    })?;
    Ok(Some(IncomingMessage::Response {
        id,
        result: value.get("result").cloned(),
        error: value.get("error").cloned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        mcp_server_disable_overrides, parse_incoming, validate_cathulu_tools, AppServerMessage,
        IncomingMessage,
    };
    use crate::agent_provider::ProviderStartAdmission;
    use serde_json::json;

    #[test]
    fn app_server_spawn_rejects_default_denied_admission_before_startup() {
        let error = match super::AppServerProcess::start(&ProviderStartAdmission::default()) {
            Ok(_) => panic!("default admission must not spawn a provider"),
            Err(error) => error,
        };
        assert!(error.contains("no validated workspace admission"));
    }

    #[test]
    fn parses_app_server_notifications_and_approval_requests_separately() {
        let notification = parse_incoming(
            br#"{"method":"turn/completed","params":{"threadId":"thr_1","turn":{"status":"completed"}}}"#,
        )
        .unwrap()
        .unwrap();
        assert!(matches!(
            notification,
            IncomingMessage::Event(AppServerMessage::Notification { method, .. })
                if method == "turn/completed"
        ));

        let approval = parse_incoming(
            br#"{"id":17,"method":"item/fileChange/requestApproval","params":{"threadId":"thr_1"}}"#,
        )
        .unwrap()
        .unwrap();
        assert!(matches!(
            approval,
            IncomingMessage::Event(AppServerMessage::Request { id, method, .. })
                if id == json!(17) && method == "item/fileChange/requestApproval"
        ));
    }

    #[test]
    fn parses_rpc_success_and_error_responses() {
        let success = parse_incoming(br#"{"id":4,"result":{"thread":{"id":"thr_4"}}}"#)
            .unwrap()
            .unwrap();
        assert!(matches!(success, IncomingMessage::Response { id: 4, .. }));

        let error = parse_incoming(br#"{"id":5,"error":{"code":-32600,"message":"no rollout"}}"#)
            .unwrap()
            .unwrap();
        assert!(matches!(error, IncomingMessage::Response { id: 5, .. }));
    }

    #[test]
    fn rejects_unbounded_protocol_messages() {
        let line = vec![b'a'; super::MAX_RPC_LINE_BYTES + 1];
        let error =
            super::read_bounded_line(&mut std::io::Cursor::new(line), &mut Vec::new()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn requires_thread_scoped_connected_cathulu_tools_before_dispatch() {
        let status = json!({
            "data": [{
                "name": "cathulu",
                "runtimeStatus": "connected",
                "tools": {"read_memories": {}, "submit_delegation_report": {}}
            }]
        });
        assert!(
            validate_cathulu_tools(&status, &["read_memories", "submit_delegation_report"]).is_ok()
        );
        assert!(
            validate_cathulu_tools(&status, &["dispatch_prepared_delegations"])
                .unwrap_err()
                .contains("dispatch_prepared_delegations")
        );

        let disconnected = json!({
            "data": [{"name":"cathulu","runtimeStatus":"failed","tools":{"read_memories":{}}}]
        });
        assert!(validate_cathulu_tools(&disconnected, &["read_memories"]).is_err());
    }

    #[test]
    fn disables_global_mcp_servers_without_copying_config_values() {
        let config = r#"
            model = "configured-model"
            [mcp_servers.cathulu]
            command = "C:/private/path/cathulu-mcp.exe"
            [mcp_servers.github]
            command = "node"
            args = ["server.js"]
        "#;
        let overrides = mcp_server_disable_overrides(config).unwrap();
        assert_eq!(
            overrides,
            vec![
                "mcp_servers.\"cathulu\".enabled=false",
                "mcp_servers.\"github\".enabled=false"
            ]
        );
        assert!(overrides
            .iter()
            .all(|value| !value.contains("private/path")));
    }

    #[test]
    fn quotes_unusual_mcp_names_and_rejects_malformed_config() {
        let config = r#"mcp_servers = { "custom server" = { command = "node" } }"#;
        assert_eq!(
            mcp_server_disable_overrides(config).unwrap(),
            vec!["mcp_servers.\"custom server\".enabled=false"]
        );
        assert!(mcp_server_disable_overrides("mcp_servers = [\n").is_err());
    }
}
