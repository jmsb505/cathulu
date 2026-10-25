//! Codex app-server protocol adapter.
//!
//! The dispatcher should consume provider-neutral events and errors. Codex thread/turn methods,
//! notification names, JSON-RPC request identifiers, and response payloads stay in this module.

use crate::agent_provider::{
    AckState, AgentProvider, ApprovalDecision, ApprovalKind, AttemptKey, CredentialSource,
    ExternalId, ExternalSession, ExternalTurn, FeatureSupport, IsolationCapabilities,
    McpToolConfig, ProviderCapabilities, ProviderError, ProviderEvent, ProviderEventKind,
    ProviderId, ProviderOperation, ProviderStartAdmission, SessionConfig, SessionStatus,
    SubmissionAck, SubmitTurnRequest, TurnOutcome, UnavailableReason, WorkspaceAccess,
    CONTRACT_VERSION, MAX_PROMPT_BYTES,
};
use crate::app_server::{AppServerMessage, AppServerProcess};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

const MAX_TRACKED_ATTEMPTS: usize = 256;
const MAX_TRACKED_APPROVALS: usize = 256;

#[derive(Clone)]
struct CachedSubmission {
    request_hash: String,
    result: Result<SubmissionAck, ProviderError>,
}

struct PendingApproval {
    session: ExternalSession,
    protocol_id: Value,
    method: String,
}

/// A bounded synchronous wrapper around the existing local Codex app-server process.
///
/// This adapter owns only provider protocol translation and submit idempotency. Cathulu continues
/// to own queueing, role capabilities, approval decisions, task state and reconciliation.
pub struct CodexAdapter {
    process: Box<dyn AppServerTransport>,
    admission: ProviderStartAdmission,
    sessions: HashSet<ExternalSession>,
    submissions: HashMap<AttemptKey, CachedSubmission>,
    active_turns: HashMap<ExternalSession, ExternalTurn>,
    approvals: HashMap<ExternalId, PendingApproval>,
}

impl CodexAdapter {
    pub fn start(admission: ProviderStartAdmission) -> Result<Self, ProviderError> {
        AppServerProcess::start(&admission)
            .map(|process| Self::from_process(process, admission))
            .map_err(|message| unavailable(UnavailableReason::Transport, message))
    }

    pub fn from_process(process: AppServerProcess, admission: ProviderStartAdmission) -> Self {
        Self::from_transport(Box::new(process), admission)
    }

    fn from_transport(
        process: Box<dyn AppServerTransport>,
        admission: ProviderStartAdmission,
    ) -> Self {
        Self {
            process,
            admission,
            sessions: HashSet::new(),
            submissions: HashMap::new(),
            active_turns: HashMap::new(),
            approvals: HashMap::new(),
        }
    }

    /// Compatibility entry point for the existing dispatcher setup. Protocol options and
    /// responses remain inside this adapter while role policy stays with the dispatcher.
    pub fn start_role_thread(
        &mut self,
        cwd: &Path,
        model: Option<&str>,
        approval_policy: &str,
        sandbox: &str,
        config: Value,
    ) -> Result<ExternalSession, ProviderError> {
        self.admission
            .ensure_workspace_root(cwd)
            .map_err(|message| ProviderError::Rejected { message })?;
        let id = self
            .process
            .start_thread(cwd, model, approval_policy, sandbox, config)
            .map_err(|message| unavailable(UnavailableReason::Transport, message))?;
        let session = external_session(id)?;
        self.sessions.insert(session.clone());
        Ok(session)
    }

    pub fn resume_role_thread(
        &mut self,
        session: &ExternalSession,
        config: Value,
    ) -> Result<(), ProviderError> {
        self.admission
            .ensure_valid(crate::unix_time())
            .map_err(|message| ProviderError::Rejected { message })?;
        ensure_codex_session(session)?;
        self.process
            .resume_thread(session.session_id.as_str(), config)
            .map_err(|message| ProviderError::Uncertain { message })?;
        self.sessions.insert(session.clone());
        Ok(())
    }

    pub fn require_role_tools(
        &mut self,
        session: &ExternalSession,
        required_tools: &[&str],
    ) -> Result<(), ProviderError> {
        ensure_codex_session(session)?;
        self.process
            .require_cathulu_tools(session.session_id.as_str(), required_tools)
            .map_err(|message| ProviderError::Rejected { message })
    }

    pub fn submit_text(
        &mut self,
        request: SubmitTurnRequest,
    ) -> Result<SubmissionAck, ProviderError> {
        request
            .validate()
            .map_err(|message| ProviderError::Rejected { message })?;
        ensure_codex_session(&request.session)?;
        if !self.sessions.contains(&request.session) {
            return Err(ProviderError::SessionGone {
                session: request.session,
            });
        }
        let request_hash = submission_hash(&request)?;
        if let Some(cached) = self.submissions.get(&request.attempt_key) {
            if cached.request_hash != request_hash {
                return Err(ProviderError::Rejected {
                    message: "attempt key was already used for a different session or payload"
                        .into(),
                });
            }
            return cached.result.clone();
        }
        if self.submissions.len() >= MAX_TRACKED_ATTEMPTS {
            return Err(ProviderError::Rejected {
                message: "adapter attempt cache is full; reconcile before submitting more work"
                    .into(),
            });
        }

        // Once sent, a transport error cannot prove whether Codex accepted the turn. Preserve
        // that distinction so Cathulu can reconcile instead of retrying the same attempt.
        let response = match self
            .process
            .start_turn(request.session.session_id.as_str(), &request.text)
        {
            Ok(response) => response,
            Err(message) => {
                let error = ProviderError::Uncertain { message };
                self.submissions.insert(
                    request.attempt_key,
                    CachedSubmission {
                        request_hash,
                        result: Err(error.clone()),
                    },
                );
                return Err(error);
            }
        };
        let turn_id = response
            .pointer("/turn/id")
            .or_else(|| response.get("turnId"))
            .and_then(Value::as_str)
            .and_then(|id| ExternalId::new(id.to_string()).ok());
        let turn = turn_id.map(|turn_id| ExternalTurn {
            session: request.session.clone(),
            turn_id,
        });
        if let Some(turn) = &turn {
            self.active_turns
                .insert(request.session.clone(), turn.clone());
        }
        let acknowledgement = SubmissionAck {
            acknowledgement: if turn.is_some() {
                AckState::Acked
            } else {
                AckState::AckUnknown
            },
            turn,
        };
        self.submissions.insert(
            request.attempt_key,
            CachedSubmission {
                request_hash,
                result: Ok(acknowledgement.clone()),
            },
        );
        Ok(acknowledgement)
    }

    pub fn next_provider_event(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<ProviderEvent>, ProviderError> {
        let message = self
            .process
            .next_event(timeout)
            .map_err(|message| ProviderError::Uncertain { message })?;
        let Some(message) = message else {
            return Ok(None);
        };
        self.map_message(message)
    }

    fn map_message(
        &mut self,
        message: AppServerMessage,
    ) -> Result<Option<ProviderEvent>, ProviderError> {
        let mut mapped = match message {
            AppServerMessage::Notification { method, params } => {
                map_notification(&method, &params, &self.active_turns)?
            }
            AppServerMessage::Request { id, method, params } => {
                let Some(thread_id) = params.get("threadId").and_then(Value::as_str) else {
                    self.process
                        .reject_request(
                            id,
                            -32000,
                            "Cathulu cannot handle this request without a provider session; no approval was granted.",
                        )
                        .map_err(|message| ProviderError::Uncertain { message })?;
                    return Ok(Some(unrecognized("request_without_session", &params)));
                };
                if method.len() > 128 {
                    self.process
                        .reject_request(
                            id,
                            -32000,
                            "Cathulu cannot handle this oversized request method; no approval was granted.",
                        )
                        .map_err(|message| ProviderError::Uncertain { message })?;
                    return Ok(Some(unrecognized("oversized_request_method", &params)));
                }
                let session = external_session(thread_id.to_string())?;
                let request_id = approval_id(&session, &id)?;
                let request_kind = match method.as_str() {
                    "item/commandExecution/requestApproval" => ApprovalKind::Command,
                    "item/fileChange/requestApproval" => ApprovalKind::FileChange,
                    "item/permission/requestApproval" => ApprovalKind::Permission,
                    "item/tool/requestUserInput" => ApprovalKind::Elicitation,
                    _ => ApprovalKind::Other,
                };
                if self.approvals.len() >= MAX_TRACKED_APPROVALS {
                    return Err(ProviderError::Rejected {
                        message: "too many unanswered Codex requests".into(),
                    });
                }
                self.approvals.insert(
                    request_id.clone(),
                    PendingApproval {
                        session: session.clone(),
                        protocol_id: id,
                        method,
                    },
                );
                let (event, event_id) = ProviderEventKind::ApprovalRequested {
                    session,
                    request_id: request_id.clone(),
                    request_kind,
                }
                .with_event_id(request_id);
                Some(ProviderEvent {
                    provider_id: ProviderId::codex(),
                    event_id,
                    sequence: None,
                    event,
                })
            }
            AppServerMessage::TransportClosed => return Err(ProviderError::Uncertain {
                message:
                    "Codex app-server transport closed with active work requiring reconciliation"
                        .into(),
            }),
            AppServerMessage::TransportError(message) => {
                return Err(ProviderError::Uncertain { message })
            }
        };
        let Some(event) = mapped.as_ref() else {
            return Ok(None);
        };
        event
            .validate()
            .map_err(|message| ProviderError::Rejected { message })?;
        let event_session = match &event.event {
            ProviderEventKind::TurnStarted { turn }
            | ProviderEventKind::TurnCompleted { turn, .. } => Some(&turn.session),
            ProviderEventKind::ApprovalRequested { session, .. }
            | ProviderEventKind::UsageObserved { session, .. } => Some(session),
            ProviderEventKind::TransportLost { session } => session.as_ref(),
            ProviderEventKind::CapacityObserved { .. } | ProviderEventKind::Unrecognized { .. } => {
                None
            }
        };
        if event_session.is_some_and(|session| !self.sessions.contains(session)) {
            return Ok(Some(unrecognized("event_for_unknown_session", &json!({}))));
        }
        match &event.event {
            ProviderEventKind::TurnStarted { turn } => {
                self.active_turns.insert(turn.session.clone(), turn.clone());
            }
            ProviderEventKind::TurnCompleted { turn, .. } => {
                if self
                    .active_turns
                    .get(&turn.session)
                    .is_some_and(|active| active.turn_id == turn.turn_id)
                {
                    self.active_turns.remove(&turn.session);
                }
            }
            _ => {}
        }
        Ok(mapped.take())
    }
}

trait AppServerTransport: Send {
    fn start_thread(
        &mut self,
        cwd: &Path,
        model: Option<&str>,
        approval_policy: &str,
        sandbox: &str,
        config: Value,
    ) -> Result<String, String>;
    fn resume_thread(&mut self, thread_id: &str, config: Value) -> Result<(), String>;
    fn require_cathulu_tools(
        &mut self,
        thread_id: &str,
        required_tools: &[&str],
    ) -> Result<(), String>;
    fn start_turn(&mut self, thread_id: &str, prompt: &str) -> Result<Value, String>;
    fn next_event(&mut self, timeout: Duration) -> Result<Option<AppServerMessage>, String>;
    fn respond(&mut self, request_id: Value, result: Value) -> Result<(), String>;
    fn reject_request(&mut self, request_id: Value, code: i64, message: &str)
        -> Result<(), String>;
}

impl AppServerTransport for AppServerProcess {
    fn start_thread(
        &mut self,
        cwd: &Path,
        model: Option<&str>,
        approval_policy: &str,
        sandbox: &str,
        config: Value,
    ) -> Result<String, String> {
        AppServerProcess::start_thread(self, cwd, model, approval_policy, sandbox, config)
    }

    fn resume_thread(&mut self, thread_id: &str, config: Value) -> Result<(), String> {
        AppServerProcess::resume_thread(self, thread_id, config)
    }

    fn require_cathulu_tools(
        &mut self,
        thread_id: &str,
        required_tools: &[&str],
    ) -> Result<(), String> {
        AppServerProcess::require_cathulu_tools(self, thread_id, required_tools)
    }

    fn start_turn(&mut self, thread_id: &str, prompt: &str) -> Result<Value, String> {
        AppServerProcess::start_turn(self, thread_id, prompt)
    }

    fn next_event(&mut self, timeout: Duration) -> Result<Option<AppServerMessage>, String> {
        AppServerProcess::next_event(self, timeout)
    }

    fn respond(&mut self, request_id: Value, result: Value) -> Result<(), String> {
        AppServerProcess::respond(self, request_id, result)
    }

    fn reject_request(
        &mut self,
        request_id: Value,
        code: i64,
        message: &str,
    ) -> Result<(), String> {
        AppServerProcess::reject_request(self, request_id, code, message)
    }
}

impl AgentProvider for CodexAdapter {
    fn capabilities(&self) -> ProviderCapabilities {
        declared_capabilities()
    }

    fn open_session(&mut self, config: SessionConfig) -> Result<ExternalSession, ProviderError> {
        config
            .validate()
            .map_err(|message| ProviderError::Rejected { message })?;
        self.admission
            .ensure_permissions(&config.permissions)
            .map_err(|message| ProviderError::Rejected { message })?;
        enforce_unactivated_runtime_gates(&config.permissions)?;
        let provider_config = codex_config(&config.tools)?;
        let sandbox = match config.permissions.access {
            WorkspaceAccess::ReadOnly => "read-only",
            WorkspaceAccess::WorkspaceWrite => "workspace-write",
        };
        let session = self.start_role_thread(
            &config.permissions.workspace_root,
            config.model.as_deref(),
            "on-request",
            sandbox,
            provider_config,
        )?;
        self.require_role_tools(
            &session,
            &config
                .tools
                .required_tools
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        )?;
        Ok(session)
    }

    fn resume_session(
        &mut self,
        session: &ExternalSession,
        config: SessionConfig,
    ) -> Result<ExternalSession, ProviderError> {
        config
            .validate()
            .map_err(|message| ProviderError::Rejected { message })?;
        self.admission
            .ensure_permissions(&config.permissions)
            .map_err(|message| ProviderError::Rejected { message })?;
        enforce_unactivated_runtime_gates(&config.permissions)?;
        let provider_config = codex_config(&config.tools)?;
        self.resume_role_thread(session, provider_config)?;
        self.require_role_tools(
            session,
            &config
                .tools
                .required_tools
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        )?;
        Ok(session.clone())
    }

    fn submit_turn(&mut self, request: SubmitTurnRequest) -> Result<SubmissionAck, ProviderError> {
        self.submit_text(request)
    }

    fn next_event(&mut self, timeout: Duration) -> Result<Option<ProviderEvent>, ProviderError> {
        self.next_provider_event(timeout)
    }

    fn answer_request(
        &mut self,
        session: &ExternalSession,
        request_id: &ExternalId,
        decision: ApprovalDecision,
    ) -> Result<(), ProviderError> {
        let pending = self
            .approvals
            .remove(request_id)
            .ok_or_else(|| ProviderError::Rejected {
                message: "Codex approval request is unknown or already answered".into(),
            })?;
        if &pending.session != session {
            self.approvals.insert(request_id.clone(), pending);
            return Err(ProviderError::Rejected {
                message: "Codex approval request belongs to another session".into(),
            });
        }
        match decision {
            ApprovalDecision::Decline
                if matches!(
                    pending.method.as_str(),
                    "item/commandExecution/requestApproval"
                        | "item/fileChange/requestApproval"
                ) => self
                .process
                .respond(pending.protocol_id, json!({ "decision": "decline" }))
                .map_err(|message| ProviderError::Uncertain { message }),
            ApprovalDecision::Decline => self
                .process
                .reject_request(
                    pending.protocol_id,
                    -32000,
                    "Cathulu has no approval policy for this Codex request; no approval was granted.",
                )
                .map_err(|message| ProviderError::Uncertain { message }),
        }
    }

    fn interrupt(&mut self, _turn: &ExternalTurn) -> Result<(), ProviderError> {
        Err(ProviderError::Unsupported {
            operation: ProviderOperation::Interrupt,
        })
    }

    fn session_status(
        &mut self,
        _session: &ExternalSession,
    ) -> Result<SessionStatus, ProviderError> {
        Err(ProviderError::Unsupported {
            operation: ProviderOperation::Status,
        })
    }

    fn close_session(&mut self, _session: &ExternalSession) -> Result<(), ProviderError> {
        Err(ProviderError::Unsupported {
            operation: ProviderOperation::CloseSession,
        })
    }
}

pub(crate) fn declared_capabilities() -> ProviderCapabilities {
    ProviderCapabilities {
            contract_version: CONTRACT_VERSION,
            provider_id: ProviderId::codex(),
            adapter_version: env!("CARGO_PKG_VERSION").to_string(),
            harness_version: None,
            resume_session: FeatureSupport::Unverified,
            interrupt: FeatureSupport::Unsupported,
            status_query: FeatureSupport::Unsupported,
            structured_result: FeatureSupport::Unverified,
            per_turn_model: FeatureSupport::Unsupported,
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
                "Codex app-server session/turn protocol is adapted; workspace and credential isolation remain unverified.".into(),
                "Turn completion is an observation only; Cathulu retains report, QA and review authority.".into(),
            ],
    }
}

fn enforce_unactivated_runtime_gates(
    permissions: &crate::agent_provider::PermissionManifest,
) -> Result<(), ProviderError> {
    if permissions.access == WorkspaceAccess::WorkspaceWrite {
        return Err(ProviderError::Rejected {
            message: "Codex writer sessions remain gated until M52 workspace leases are integrated"
                .into(),
        });
    }
    if !permissions.network_access {
        return Err(ProviderError::Rejected {
            message: "Codex network isolation cannot be enforced by the current app-server adapter"
                .into(),
        });
    }
    Err(unavailable(
        UnavailableReason::NotAuthenticated,
        "Codex credential source is unverified and is not eligible for provider execution".into(),
    ))
}

fn codex_config(tools: &McpToolConfig) -> Result<Value, ProviderError> {
    tools
        .validate()
        .map_err(|message| ProviderError::Rejected { message })?;
    let mut servers = serde_json::Map::new();
    servers.insert(
        tools.server_name.clone(),
        json!({
            "command": tools.command,
            "args": tools.args,
            "env": tools.env,
            "enabled": true,
            "startup_timeout_ms": 20000
        }),
    );
    Ok(json!({ "mcp_servers": servers }))
}

fn map_notification(
    method: &str,
    params: &Value,
    active_turns: &HashMap<ExternalSession, ExternalTurn>,
) -> Result<Option<ProviderEvent>, ProviderError> {
    let thread_id = params.get("threadId").and_then(Value::as_str);
    let kind = match method {
        "turn/started" => {
            let (Some(thread_id), Some(turn_id)) = (thread_id, notification_turn_id(params)) else {
                return Ok(None);
            };
            let session = external_session(thread_id.to_string())?;
            let turn = external_turn(&session, turn_id)?;
            ProviderEventKind::TurnStarted { turn }.with_event_id(stable_event_id(
                "turn_started",
                &session,
                &turn_id,
            )?)
        }
        "turn/completed" => {
            let (Some(thread_id), Some(turn_id)) = (thread_id, notification_turn_id(params)) else {
                return Ok(None);
            };
            let session = external_session(thread_id.to_string())?;
            if active_turns
                .get(&session)
                .is_some_and(|active| active.turn_id.as_str() != turn_id)
            {
                return Ok(None);
            }
            let turn = external_turn(&session, turn_id)?;
            let status = params
                .pointer("/turn/status")
                .or_else(|| params.get("status"))
                .and_then(Value::as_str);
            let outcome = match status {
                Some("completed") => TurnOutcome::Completed,
                Some("interrupted" | "cancelled") => TurnOutcome::Interrupted,
                _ => TurnOutcome::Failed,
            };
            ProviderEventKind::TurnCompleted { turn, outcome }.with_event_id(stable_event_id(
                "turn_completed",
                &session,
                &turn_id,
            )?)
        }
        _ => return Ok(Some(unrecognized(method, params))),
    };
    Ok(Some(ProviderEvent {
        provider_id: ProviderId::codex(),
        event_id: kind.1,
        sequence: None,
        event: kind.0,
    }))
}

trait WithEventId {
    fn with_event_id(self, id: ExternalId) -> (ProviderEventKind, Option<ExternalId>);
}

impl WithEventId for ProviderEventKind {
    fn with_event_id(self, id: ExternalId) -> (ProviderEventKind, Option<ExternalId>) {
        (self, Some(id))
    }
}

fn unrecognized(kind: &str, params: &Value) -> ProviderEvent {
    let encoded = serde_json::to_vec(params).unwrap_or_default();
    ProviderEvent {
        provider_id: ProviderId::codex(),
        event_id: None,
        sequence: None,
        event: ProviderEventKind::Unrecognized {
            event_kind: bounded_event_kind(kind),
            size_bytes: encoded.len().min(u64::MAX as usize) as u64,
            excerpt: None,
        },
    }
}

fn bounded_event_kind(kind: &str) -> String {
    let mut value = kind
        .chars()
        .filter(|character| !character.is_control())
        .take(128)
        .collect::<String>();
    if value.is_empty() {
        value = "unknown".into();
    }
    value
}

fn notification_turn_id(params: &Value) -> Option<&str> {
    params
        .pointer("/turn/id")
        .or_else(|| params.get("turnId"))
        .and_then(Value::as_str)
}

fn external_session(session_id: String) -> Result<ExternalSession, ProviderError> {
    Ok(ExternalSession {
        provider_id: ProviderId::codex(),
        session_id: external_id(session_id)?,
    })
}

fn external_turn(session: &ExternalSession, turn_id: &str) -> Result<ExternalTurn, ProviderError> {
    Ok(ExternalTurn {
        session: session.clone(),
        turn_id: external_id(turn_id.to_string())?,
    })
}

fn external_id(value: String) -> Result<ExternalId, ProviderError> {
    ExternalId::new(value).map_err(|message| ProviderError::Rejected { message })
}

fn ensure_codex_session(session: &ExternalSession) -> Result<(), ProviderError> {
    if session.provider_id == ProviderId::codex() {
        Ok(())
    } else {
        Err(ProviderError::Rejected {
            message: "session belongs to a different provider namespace".into(),
        })
    }
}

fn approval_id(
    session: &ExternalSession,
    protocol_id: &Value,
) -> Result<ExternalId, ProviderError> {
    let encoded = serde_json::to_string(protocol_id).map_err(|error| ProviderError::Internal {
        message: error.to_string(),
    })?;
    external_id(format!(
        "request:{}",
        digest_hex(format!("{}:{encoded}", session.session_id).as_bytes())
    ))
}

fn stable_event_id(
    event_kind: &str,
    session: &ExternalSession,
    turn_id: &str,
) -> Result<ExternalId, ProviderError> {
    external_id(format!(
        "event:{}",
        digest_hex(format!("codex:{event_kind}:{}:{turn_id}", session.session_id).as_bytes())
    ))
}

fn submission_hash(request: &SubmitTurnRequest) -> Result<String, ProviderError> {
    let encoded = serde_json::to_vec(&(&request.session, &request.text)).map_err(|error| {
        ProviderError::Internal {
            message: error.to_string(),
        }
    })?;
    Ok(digest_hex(&encoded))
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unavailable(reason: UnavailableReason, message: String) -> ProviderError {
    ProviderError::Unavailable {
        reason,
        retry_after_seconds: None,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    fn codex_session() -> ExternalSession {
        external_session("thread-1".to_string()).unwrap()
    }

    #[test]
    fn completed_event_requires_a_stable_thread_and_turn_identity() {
        let session = codex_session();
        let active = ExternalTurn {
            session: session.clone(),
            turn_id: external_id("turn-2".into()).unwrap(),
        };
        let active_turns = HashMap::from([(session, active)]);
        let missing_turn = map_notification(
            "turn/completed",
            &json!({ "threadId": "thread-1", "status": "completed" }),
            &active_turns,
        )
        .unwrap();
        assert!(missing_turn.is_none());
        let stale_turn = map_notification(
            "turn/completed",
            &json!({ "threadId": "thread-1", "turn": { "id": "turn-old", "status": "completed" } }),
            &active_turns,
        )
        .unwrap();
        assert!(stale_turn.is_none());
    }

    #[test]
    fn turn_events_have_session_scoped_stable_event_ids() {
        let active_turns = HashMap::new();
        let message = json!({
            "threadId": "thread-1",
            "turn": { "id": "turn-1", "status": "completed" }
        });
        let first = map_notification("turn/completed", &message, &active_turns)
            .unwrap()
            .unwrap();
        let second = map_notification("turn/completed", &message, &active_turns)
            .unwrap()
            .unwrap();
        assert_eq!(first.event_id, second.event_id);
        assert!(first.external_event_key().unwrap().is_some());
    }

    #[test]
    fn unknown_notifications_are_non_mutating_observations() {
        let event = map_notification("experimental/new", &json!({ "value": 1 }), &HashMap::new())
            .unwrap()
            .unwrap();
        assert!(event.event_id.is_none());
        assert!(matches!(
            event.event,
            ProviderEventKind::Unrecognized { .. }
        ));
    }

    #[test]
    fn capabilities_do_not_claim_unverified_isolation_or_credential_source() {
        // The capability contract is independent of a spawned process. Constructing a process
        // is deliberately avoided by testing the conservative declared capability shape.
        let capabilities = declared_capabilities();
        assert!(capabilities.validate().is_ok());
        assert_eq!(capabilities.credential_source, CredentialSource::Unknown);
        assert_eq!(capabilities.isolation.settings, FeatureSupport::Unverified);
    }

    #[test]
    fn unverified_credentials_and_missing_m52_workspace_leases_block_session_open() {
        let writer = crate::agent_provider::PermissionManifest {
            schema_version: 1,
            workspace_root: std::env::current_dir().unwrap(),
            access: WorkspaceAccess::WorkspaceWrite,
            allowed_scope: Vec::new(),
            network_access: true,
            allowed_credential_sources: vec![CredentialSource::Subscription],
        };
        assert!(matches!(
            enforce_unactivated_runtime_gates(&writer),
            Err(ProviderError::Rejected { .. })
        ));

        let reader = crate::agent_provider::PermissionManifest {
            access: WorkspaceAccess::ReadOnly,
            ..writer
        };
        assert!(matches!(
            enforce_unactivated_runtime_gates(&reader),
            Err(ProviderError::Unavailable {
                reason: UnavailableReason::NotAuthenticated,
                ..
            })
        ));
    }

    #[derive(Default)]
    struct FakeState {
        next_events: VecDeque<Result<Option<AppServerMessage>, String>>,
        start_turn_results: VecDeque<Result<Value, String>>,
        start_turn_calls: usize,
        responses: Vec<(Value, Value)>,
        rejections: Vec<(Value, i64, String)>,
    }

    struct FakeTransport(Arc<Mutex<FakeState>>);

    impl AppServerTransport for FakeTransport {
        fn start_thread(
            &mut self,
            _cwd: &Path,
            _model: Option<&str>,
            _approval_policy: &str,
            _sandbox: &str,
            _config: Value,
        ) -> Result<String, String> {
            Ok("thread-1".into())
        }

        fn resume_thread(&mut self, _thread_id: &str, _config: Value) -> Result<(), String> {
            Ok(())
        }

        fn require_cathulu_tools(
            &mut self,
            _thread_id: &str,
            _required_tools: &[&str],
        ) -> Result<(), String> {
            Ok(())
        }

        fn start_turn(&mut self, _thread_id: &str, _prompt: &str) -> Result<Value, String> {
            let mut state = self.0.lock().unwrap();
            state.start_turn_calls += 1;
            state
                .start_turn_results
                .pop_front()
                .unwrap_or_else(|| Ok(json!({ "turn": { "id": "turn-1" } })))
        }

        fn next_event(&mut self, _timeout: Duration) -> Result<Option<AppServerMessage>, String> {
            self.0
                .lock()
                .unwrap()
                .next_events
                .pop_front()
                .unwrap_or(Ok(None))
        }

        fn respond(&mut self, request_id: Value, result: Value) -> Result<(), String> {
            self.0.lock().unwrap().responses.push((request_id, result));
            Ok(())
        }

        fn reject_request(
            &mut self,
            request_id: Value,
            code: i64,
            message: &str,
        ) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .rejections
                .push((request_id, code, message.to_string()));
            Ok(())
        }
    }

    fn fake_adapter() -> (CodexAdapter, Arc<Mutex<FakeState>>) {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let admission = ProviderStartAdmission::from_validated_workspace(
            1,
            std::path::PathBuf::from("C:\\workspace"),
            "a".repeat(64),
            "b".repeat(64),
            CredentialSource::Subscription,
            i64::MAX,
        )
        .unwrap();
        let adapter =
            CodexAdapter::from_transport(Box::new(FakeTransport(Arc::clone(&state))), admission);
        (adapter, state)
    }

    fn open_fake_session(adapter: &mut CodexAdapter) -> ExternalSession {
        adapter
            .start_role_thread(
                Path::new("C:\\workspace"),
                None,
                "on-request",
                "read-only",
                json!({}),
            )
            .unwrap()
    }

    #[test]
    fn duplicate_attempt_key_replays_ack_without_a_second_submit() {
        let (mut adapter, state) = fake_adapter();
        let session = open_fake_session(&mut adapter);
        let request = SubmitTurnRequest {
            session,
            attempt_key: AttemptKey::new("attempt-1").unwrap(),
            text: "bounded task".into(),
        };
        let first = adapter.submit_text(request.clone()).unwrap();
        let second = adapter.submit_text(request).unwrap();
        assert_eq!(first, second);
        assert_eq!(state.lock().unwrap().start_turn_calls, 1);
    }

    #[test]
    fn uncertain_submit_is_cached_and_cannot_be_retried_under_the_same_key() {
        let (mut adapter, state) = fake_adapter();
        let session = open_fake_session(&mut adapter);
        state
            .lock()
            .unwrap()
            .start_turn_results
            .push_back(Err("ack timed out".into()));
        let request = SubmitTurnRequest {
            session,
            attempt_key: AttemptKey::new("attempt-uncertain").unwrap(),
            text: "bounded task".into(),
        };
        assert!(matches!(
            adapter.submit_text(request.clone()),
            Err(ProviderError::Uncertain { .. })
        ));
        assert!(matches!(
            adapter.submit_text(request),
            Err(ProviderError::Uncertain { .. })
        ));
        assert_eq!(state.lock().unwrap().start_turn_calls, 1);
    }

    #[test]
    fn approval_requests_are_declined_and_never_auto_approved() {
        let (mut adapter, state) = fake_adapter();
        let _session = open_fake_session(&mut adapter);
        state
            .lock()
            .unwrap()
            .next_events
            .push_back(Ok(Some(AppServerMessage::Request {
                id: json!(41),
                method: "item/commandExecution/requestApproval".into(),
                params: json!({ "threadId": "thread-1", "command": "hidden from mapper" }),
            })));
        let event = adapter
            .next_provider_event(Duration::ZERO)
            .unwrap()
            .unwrap();
        let ProviderEventKind::ApprovalRequested {
            session: owner,
            request_id,
            ..
        } = event.event
        else {
            panic!("expected a typed approval request")
        };
        adapter
            .answer_request(&owner, &request_id, ApprovalDecision::Decline)
            .unwrap();
        let state = state.lock().unwrap();
        assert_eq!(
            state.responses,
            vec![(json!(41), json!({ "decision": "decline" }))]
        );
        assert!(state.rejections.is_empty());
    }

    #[test]
    fn unscoped_requests_are_rejected_and_unknown_events_cannot_mutate() {
        let (mut adapter, state) = fake_adapter();
        open_fake_session(&mut adapter);
        state
            .lock()
            .unwrap()
            .next_events
            .push_back(Ok(Some(AppServerMessage::Request {
                id: json!(41),
                method: "item/commandExecution/requestApproval".into(),
                params: json!({ "command": "not included in observation" }),
            })));
        let event = adapter
            .next_provider_event(Duration::ZERO)
            .unwrap()
            .unwrap();
        assert!(event.event_id.is_none());
        assert!(matches!(
            event.event,
            ProviderEventKind::Unrecognized { .. }
        ));
        assert_eq!(state.lock().unwrap().rejections.len(), 1);
    }
}
