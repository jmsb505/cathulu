//! Provider-neutral transport contract. Events report observations, never task acceptance.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

pub const CONTRACT_VERSION: u32 = 1;
pub const MAX_PROMPT_BYTES: usize = 64 * 1024;
pub const MAX_EVENT_BYTES: usize = 8 * 1024;
pub const MAX_UNKNOWN_EXCERPT_BYTES: usize = 512;
pub mod fixtures;

macro_rules! identifier {
    ($name:ident, $maximum:expr, $provider:expr) => {
        #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, String> {
                let value = value.into();
                if value.is_empty()
                    || value.len() > $maximum
                    || value.trim() != value
                    || value.chars().any(char::is_control)
                    || ($provider
                        && !value.bytes().all(|byte| {
                            byte.is_ascii_lowercase()
                                || byte.is_ascii_digit()
                                || matches!(byte, b'_' | b'-')
                        }))
                {
                    return Err(format!("invalid {}", stringify!($name)));
                }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = String;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> String {
                value.0
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

identifier!(ProviderId, 64, true);
identifier!(ExternalId, 256, false);
identifier!(AttemptKey, 200, false);

impl ProviderId {
    pub fn codex() -> Self {
        Self("codex".to_string())
    }
    pub fn claude() -> Self {
        Self("claude".to_string())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalSession {
    pub provider_id: ProviderId,
    pub session_id: ExternalId,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalTurn {
    pub session: ExternalSession,
    pub turn_id: ExternalId,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureSupport {
    Supported,
    Unsupported,
    Unverified,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceAccess {
    ReadOnly,
    WorkspaceWrite,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    Subscription,
    ApiKey,
    Other,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IsolationCapabilities {
    pub settings: FeatureSupport,
    pub plugins: FeatureSupport,
    pub mcp_servers: FeatureSupport,
    pub instructions: FeatureSupport,
    pub memory: FeatureSupport,
}

impl IsolationCapabilities {
    pub fn verified(&self) -> bool {
        [
            self.settings,
            self.plugins,
            self.mcp_servers,
            self.instructions,
            self.memory,
        ]
        .iter()
        .all(|value| *value == FeatureSupport::Supported)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderCapabilities {
    pub contract_version: u32,
    pub provider_id: ProviderId,
    pub adapter_version: String,
    pub harness_version: Option<String>,
    pub resume_session: FeatureSupport,
    pub interrupt: FeatureSupport,
    pub status_query: FeatureSupport,
    pub structured_result: FeatureSupport,
    pub per_turn_model: FeatureSupport,
    pub isolation: IsolationCapabilities,
    pub credential_source: CredentialSource,
    pub max_prompt_bytes: usize,
    pub notes: Vec<String>,
}

impl ProviderCapabilities {
    pub fn validate(&self) -> Result<(), String> {
        if self.contract_version != CONTRACT_VERSION
            || !(1..=MAX_PROMPT_BYTES).contains(&self.max_prompt_bytes)
        {
            return Err("unsupported capability contract or prompt bound".to_string());
        }
        bounded("adapter version", &self.adapter_version, 128, true)?;
        if let Some(version) = &self.harness_version {
            bounded("harness version", version, 128, true)?;
        }
        if self.notes.len() > 16 {
            return Err("too many capability notes".to_string());
        }
        for note in &self.notes {
            bounded("capability note", note, 512, false)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionManifest {
    pub schema_version: u32,
    pub workspace_root: PathBuf,
    pub access: WorkspaceAccess,
    pub allowed_scope: Vec<String>,
    pub network_access: bool,
    pub allowed_credential_sources: Vec<CredentialSource>,
}

impl PermissionManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || !self.workspace_root.is_absolute()
            || self.allowed_scope.len() > 64
        {
            return Err(
                "invalid permission manifest version, workspace or scope bound".to_string(),
            );
        }
        for scope in &self.allowed_scope {
            bounded("allowed scope", scope, 512, true)?;
            if scope.starts_with('/')
                || scope.starts_with('\\')
                || scope.contains(':')
                || scope.split(['/', '\\']).any(|part| part == "..")
            {
                return Err("permission scopes must remain relative to the workspace".to_string());
            }
        }
        if self.allowed_credential_sources.is_empty()
            || self.allowed_credential_sources.len() > 3
            || self
                .allowed_credential_sources
                .contains(&CredentialSource::Unknown)
        {
            return Err("credential sources must be explicitly permitted and known".to_string());
        }
        Ok(())
    }

    pub fn content_hash(&self) -> Result<String, String> {
        self.validate()?;
        Ok(crate::hash::content_hash(
            &serde_json::to_string(self).map_err(|error| error.to_string())?,
        ))
    }
}

/// Refuse provider execution unless the adapter reports a concrete credential source and the
/// permission envelope explicitly allows that source. This check does not inspect credentials.
pub fn validate_provider_credential_source(
    capabilities: &ProviderCapabilities,
    permissions: &PermissionManifest,
) -> Result<CredentialSource, String> {
    capabilities.validate()?;
    permissions.validate()?;
    if capabilities.credential_source == CredentialSource::Unknown {
        return Err("provider credential source is unknown; execution is not eligible".into());
    }
    if !permissions
        .allowed_credential_sources
        .contains(&capabilities.credential_source)
    {
        return Err("provider credential source is not permitted by the execution scope".into());
    }
    Ok(capabilities.credential_source)
}

/// A runtime-only, fail-closed proof that a provider spawn was admitted for one exact workspace
/// lease, permission manifest, and known credential source. The record is created only after the
/// Store validates the live lease and bound role capability. Its default value denies startup.
#[derive(Clone, Debug, Default)]
pub struct ProviderStartAdmission {
    grant: Option<ProviderStartGrant>,
}

#[derive(Clone, Debug)]
struct ProviderStartGrant {
    workspace_lease_id: i64,
    workspace_root: PathBuf,
    workspace_path_hash: String,
    permission_hash: String,
    credential_source: CredentialSource,
    lease_expires_at: i64,
}

impl ProviderStartAdmission {
    pub(crate) fn from_validated_workspace(
        workspace_lease_id: i64,
        workspace_root: PathBuf,
        workspace_path_hash: String,
        permission_hash: String,
        credential_source: CredentialSource,
        lease_expires_at: i64,
    ) -> Result<Self, String> {
        if workspace_lease_id <= 0
            || !workspace_root.is_absolute()
            || workspace_path_hash.len() != 64
            || !workspace_path_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || permission_hash.len() != 64
            || !permission_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            || credential_source == CredentialSource::Unknown
            || lease_expires_at <= 0
        {
            return Err("provider start admission identity is incomplete".into());
        }
        Ok(Self {
            grant: Some(ProviderStartGrant {
                workspace_lease_id,
                workspace_root,
                workspace_path_hash,
                permission_hash,
                credential_source,
                lease_expires_at,
            }),
        })
    }

    pub fn ensure_valid(&self, now: i64) -> Result<(), String> {
        let Some(grant) = &self.grant else {
            return Err("provider start has no validated workspace admission".into());
        };
        if now < 0 || grant.lease_expires_at <= now {
            return Err("provider start admission is expired".into());
        }
        if grant.workspace_lease_id <= 0
            || grant.workspace_path_hash.len() != 64
            || grant.permission_hash.len() != 64
            || grant.credential_source == CredentialSource::Unknown
        {
            return Err("provider start admission is invalid".into());
        }
        Ok(())
    }

    pub(crate) fn ensure_workspace_root(
        &self,
        workspace_root: &std::path::Path,
    ) -> Result<(), String> {
        self.ensure_valid(crate::unix_time())?;
        let Some(grant) = &self.grant else {
            return Err("provider start has no validated workspace admission".into());
        };
        if workspace_root != grant.workspace_root {
            return Err("provider workspace does not match its admitted lease path".into());
        }
        Ok(())
    }

    pub(crate) fn ensure_permissions(
        &self,
        permissions: &PermissionManifest,
    ) -> Result<(), String> {
        self.ensure_workspace_root(&permissions.workspace_root)?;
        let hash = permissions.content_hash()?;
        let Some(grant) = &self.grant else {
            return Err("provider start has no validated workspace admission".into());
        };
        if hash != grant.permission_hash {
            return Err("provider permission manifest differs from its admitted record".into());
        }
        Ok(())
    }

    pub fn is_admitted(&self) -> bool {
        self.grant.is_some()
    }
}

/// Runtime-only trusted MCP configuration. Deliberately neither Debug nor Serialize: env may contain capability tokens.
#[derive(Clone)]
pub struct McpToolConfig {
    pub server_name: String,
    pub command: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub required_tools: Vec<String>,
}

impl McpToolConfig {
    pub fn validate(&self) -> Result<(), String> {
        bounded("MCP server name", &self.server_name, 64, true)?;
        if !self.command.is_absolute()
            || self.args.len() > 64
            || self.env.len() > 64
            || self.required_tools.len() > 128
        {
            return Err("invalid trusted MCP configuration bounds".to_string());
        }
        for value in self
            .args
            .iter()
            .chain(self.env.keys())
            .chain(self.env.values())
            .chain(self.required_tools.iter())
        {
            bounded("MCP configuration value", value, 8 * 1024, false)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SessionConfig {
    pub logical_role_key: String,
    pub project_id: Option<i64>,
    pub permissions: PermissionManifest,
    pub tools: McpToolConfig,
    pub model: Option<String>,
}

impl SessionConfig {
    pub fn validate(&self) -> Result<(), String> {
        bounded("logical role key", &self.logical_role_key, 200, true)?;
        if self.project_id.is_some_and(|id| id <= 0) {
            return Err("invalid session project ID".to_string());
        }
        self.permissions.validate()?;
        self.tools.validate()?;
        if let Some(model) = &self.model {
            bounded("model", model, 200, true)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmitTurnRequest {
    pub session: ExternalSession,
    pub attempt_key: AttemptKey,
    pub text: String,
}

impl SubmitTurnRequest {
    pub fn validate(&self) -> Result<(), String> {
        bounded("work order text", &self.text, MAX_PROMPT_BYTES, true)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AckState {
    Acked,
    AckUnknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubmissionAck {
    pub acknowledgement: AckState,
    pub turn: Option<ExternalTurn>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOperation {
    OpenSession,
    ResumeSession,
    SubmitTurn,
    NextEvent,
    AnswerRequest,
    Interrupt,
    Status,
    CloseSession,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    NotInstalled,
    NotAuthenticated,
    AccountRestricted,
    Capacity,
    Transport,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderError {
    Unavailable {
        reason: UnavailableReason,
        retry_after_seconds: Option<u64>,
        message: String,
    },
    Rejected {
        message: String,
    },
    Uncertain {
        message: String,
    },
    Unsupported {
        operation: ProviderOperation,
    },
    SessionGone {
        session: ExternalSession,
    },
    Internal {
        message: String,
    },
}

impl fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ProviderError {}

impl ProviderError {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_EVENT_BYTES {
            return Err("provider error exceeds 8 KiB".to_string());
        }
        let error: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        error.validate()?;
        Ok(error)
    }
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Unavailable {
                reason,
                retry_after_seconds,
                message,
            } => {
                if retry_after_seconds.is_some() && *reason != UnavailableReason::Capacity {
                    return Err("only capacity errors carry a timed retry hint".to_string());
                }
                bounded("provider error", message, 1000, true)
            }
            Self::Rejected { message }
            | Self::Uncertain { message }
            | Self::Internal { message } => bounded("provider error", message, 1000, true),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TurnOutcome {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    Command,
    FileChange,
    Permission,
    Elicitation,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Decline,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderEventKind {
    TurnStarted {
        turn: ExternalTurn,
    },
    TurnCompleted {
        turn: ExternalTurn,
        outcome: TurnOutcome,
    },
    ApprovalRequested {
        session: ExternalSession,
        request_id: ExternalId,
        request_kind: ApprovalKind,
    },
    UsageObserved {
        session: ExternalSession,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
    CapacityObserved {
        retry_after_seconds: Option<u64>,
    },
    TransportLost {
        session: Option<ExternalSession>,
    },
    Unrecognized {
        event_kind: String,
        size_bytes: u64,
        excerpt: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderEvent {
    pub provider_id: ProviderId,
    pub event_id: Option<ExternalId>,
    pub sequence: Option<u64>,
    pub event: ProviderEventKind,
}

impl ProviderEvent {
    /// Stable namespace for the existing external-event ledger. Missing identity stays unknown.
    pub fn external_event_key(&self) -> Result<Option<String>, String> {
        self.validate()?;
        let identity = if let Some(id) = &self.event_id {
            format!("id:{id}")
        } else if let Some(sequence) = self.sequence {
            format!("sequence:{sequence}")
        } else {
            return Ok(None);
        };
        let (session, turn, kind) = match &self.event {
            ProviderEventKind::TurnStarted { turn } => (
                Some(&turn.session),
                Some(&turn.turn_id),
                "turn_started".to_string(),
            ),
            ProviderEventKind::TurnCompleted { turn, .. } => (
                Some(&turn.session),
                Some(&turn.turn_id),
                "turn_completed".to_string(),
            ),
            ProviderEventKind::ApprovalRequested { session, .. } => {
                (Some(session), None, "approval_requested".to_string())
            }
            ProviderEventKind::UsageObserved { session, .. } => {
                (Some(session), None, "usage_observed".to_string())
            }
            ProviderEventKind::TransportLost { session } => {
                (session.as_ref(), None, "transport_lost".to_string())
            }
            ProviderEventKind::CapacityObserved { .. } => {
                (None, None, "capacity_observed".to_string())
            }
            ProviderEventKind::Unrecognized { event_kind, .. } => {
                (None, None, format!("unrecognized:{event_kind}"))
            }
        };
        let encoded =
            serde_json::to_string(&(self.provider_id.clone(), session, turn, kind, identity))
                .map_err(|error| error.to_string())?;
        Ok(Some(format!(
            "provider-event:v1:{}",
            crate::hash::content_hash(&encoded)
        )))
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_EVENT_BYTES {
            return Err("provider event exceeds 8 KiB".to_string());
        }
        let event: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        event.validate()?;
        Ok(event)
    }

    pub fn validate(&self) -> Result<(), String> {
        if serde_json::to_vec(self)
            .map_err(|error| error.to_string())?
            .len()
            > MAX_EVENT_BYTES
        {
            return Err("provider event exceeds 8 KiB".to_string());
        }
        let session = match &self.event {
            ProviderEventKind::TurnStarted { turn }
            | ProviderEventKind::TurnCompleted { turn, .. } => Some(&turn.session),
            ProviderEventKind::ApprovalRequested { session, .. }
            | ProviderEventKind::UsageObserved { session, .. } => Some(session),
            ProviderEventKind::TransportLost { session } => session.as_ref(),
            ProviderEventKind::Unrecognized {
                event_kind,
                excerpt,
                ..
            } => {
                bounded("unknown event kind", event_kind, 128, true)?;
                if let Some(excerpt) = excerpt {
                    bounded(
                        "unknown event excerpt",
                        excerpt,
                        MAX_UNKNOWN_EXCERPT_BYTES,
                        false,
                    )?;
                }
                None
            }
            ProviderEventKind::CapacityObserved { .. } => None,
        };
        if session.is_some_and(|session| session.provider_id != self.provider_id) {
            return Err("provider event session namespace mismatch".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionStatus {
    Observed { active_turn: Option<ExternalTurn> },
    Gone {},
    Unknown {},
}

/// Bounded synchronous I/O matches the existing dispatcher thread and app-server client.
/// Implementations must deduplicate attempt keys and classify lost submit acknowledgements as uncertain.
pub trait AgentProvider: Send {
    fn capabilities(&self) -> ProviderCapabilities;
    fn open_session(&mut self, config: SessionConfig) -> Result<ExternalSession, ProviderError>;
    fn resume_session(
        &mut self,
        session: &ExternalSession,
        config: SessionConfig,
    ) -> Result<ExternalSession, ProviderError>;
    fn submit_turn(&mut self, request: SubmitTurnRequest) -> Result<SubmissionAck, ProviderError>;
    fn next_event(&mut self, timeout: Duration) -> Result<Option<ProviderEvent>, ProviderError>;
    fn answer_request(
        &mut self,
        session: &ExternalSession,
        request_id: &ExternalId,
        decision: ApprovalDecision,
    ) -> Result<(), ProviderError>;
    fn interrupt(&mut self, turn: &ExternalTurn) -> Result<(), ProviderError>;
    fn session_status(&mut self, session: &ExternalSession)
        -> Result<SessionStatus, ProviderError>;
    fn close_session(&mut self, session: &ExternalSession) -> Result<(), ProviderError>;
}

pub(crate) fn bounded(
    label: &str,
    value: &str,
    maximum: usize,
    required: bool,
) -> Result<(), String> {
    if value.len() > maximum || value.contains('\0') || (required && value.trim().is_empty()) {
        Err(format!("invalid {label} (maximum {maximum} bytes)"))
    } else {
        Ok(())
    }
}
