//! Deterministic no-I/O fixture available to standalone adapter contract tests.

use super::*;
use std::collections::{BTreeMap, VecDeque};

pub struct ScriptedProvider {
    capabilities: ProviderCapabilities,
    session: Option<ExternalSession>,
    binding: Option<(String, Option<i64>, String)>,
    session_counter: u64,
    submissions: BTreeMap<AttemptKey, (String, Result<SubmissionAck, ProviderError>)>,
    scripted_submissions: VecDeque<Result<SubmissionAck, ProviderError>>,
    events: VecDeque<Result<ProviderEvent, ProviderError>>,
    pub unique_submit_count: usize,
    pub declined_requests: Vec<(ExternalSession, ExternalId)>,
}

impl ScriptedProvider {
    pub fn new(provider_id: ProviderId) -> Self {
        Self {
            capabilities: ProviderCapabilities {
                contract_version: CONTRACT_VERSION,
                provider_id,
                adapter_version: "fixture-v1".to_string(),
                harness_version: Some("synthetic-no-io".to_string()),
                resume_session: FeatureSupport::Supported,
                interrupt: FeatureSupport::Unsupported,
                status_query: FeatureSupport::Unsupported,
                structured_result: FeatureSupport::Supported,
                per_turn_model: FeatureSupport::Unsupported,
                isolation: IsolationCapabilities {
                    settings: FeatureSupport::Supported,
                    plugins: FeatureSupport::Supported,
                    mcp_servers: FeatureSupport::Supported,
                    instructions: FeatureSupport::Supported,
                    memory: FeatureSupport::Supported,
                },
                credential_source: CredentialSource::Subscription,
                max_prompt_bytes: MAX_PROMPT_BYTES,
                notes: vec![
                    "Fixture mechanics only; no live provider capability claim.".to_string()
                ],
            },
            session: None,
            binding: None,
            session_counter: 0,
            submissions: BTreeMap::new(),
            scripted_submissions: VecDeque::new(),
            events: VecDeque::new(),
            unique_submit_count: 0,
            declined_requests: Vec::new(),
        }
    }

    pub fn queue_submission(&mut self, result: Result<SubmissionAck, ProviderError>) {
        self.scripted_submissions.push_back(result);
    }

    pub fn queue_event(&mut self, event: ProviderEvent) -> Result<(), String> {
        event.validate()?;
        self.events.push_back(Ok(event));
        Ok(())
    }

    pub fn queue_raw_event(&mut self, bytes: &[u8]) {
        self.events.push_back(
            ProviderEvent::parse(bytes).map_err(|message| ProviderError::Rejected { message }),
        );
    }

    fn ensure_session(&self, session: &ExternalSession) -> Result<(), ProviderError> {
        if self.session.as_ref() == Some(session) {
            Ok(())
        } else {
            Err(ProviderError::SessionGone {
                session: session.clone(),
            })
        }
    }

    fn validate_config(&self, config: &SessionConfig) -> Result<(), ProviderError> {
        config
            .validate()
            .map_err(|message| ProviderError::Rejected { message })?;
        if !config
            .permissions
            .allowed_credential_sources
            .contains(&self.capabilities.credential_source)
        {
            return Err(ProviderError::Unavailable {
                reason: UnavailableReason::NotAuthenticated,
                retry_after_seconds: None,
                message: "fixture credential source is not approved".to_string(),
            });
        }
        Ok(())
    }
}

impl AgentProvider for ScriptedProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities.clone()
    }

    fn open_session(&mut self, config: SessionConfig) -> Result<ExternalSession, ProviderError> {
        self.validate_config(&config)?;
        self.session_counter += 1;
        let session = ExternalSession {
            provider_id: self.capabilities.provider_id.clone(),
            session_id: ExternalId::new(format!("fixture-session-{}", self.session_counter))
                .expect("bounded fixture ID"),
        };
        self.binding = Some((
            config.logical_role_key,
            config.project_id,
            config
                .permissions
                .content_hash()
                .map_err(|message| ProviderError::Rejected { message })?,
        ));
        self.session = Some(session.clone());
        Ok(session)
    }

    fn resume_session(
        &mut self,
        session: &ExternalSession,
        config: SessionConfig,
    ) -> Result<ExternalSession, ProviderError> {
        self.ensure_session(session)?;
        self.validate_config(&config)?;
        let binding = (
            config.logical_role_key,
            config.project_id,
            config
                .permissions
                .content_hash()
                .map_err(|message| ProviderError::Rejected { message })?,
        );
        if self.binding.as_ref() != Some(&binding) {
            return Err(ProviderError::Rejected {
                message: "session role/project/permission binding changed".to_string(),
            });
        }
        Ok(session.clone())
    }

    fn submit_turn(&mut self, request: SubmitTurnRequest) -> Result<SubmissionAck, ProviderError> {
        self.ensure_session(&request.session)?;
        request
            .validate()
            .map_err(|message| ProviderError::Rejected { message })?;
        let request_hash =
            crate::hash::content_hash(&serde_json::to_string(&request).map_err(|error| {
                ProviderError::Internal {
                    message: error.to_string(),
                }
            })?);
        if let Some((previous_hash, result)) = self.submissions.get(&request.attempt_key) {
            if previous_hash != &request_hash {
                return Err(ProviderError::Rejected {
                    message: "attempt key reused with changed session or content".to_string(),
                });
            }
            return result.clone();
        }
        self.unique_submit_count += 1;
        let mut result = self.scripted_submissions.pop_front().unwrap_or_else(|| {
            Ok(SubmissionAck {
                acknowledgement: AckState::Acked,
                turn: Some(ExternalTurn {
                    session: request.session,
                    turn_id: ExternalId::new(format!("fixture-turn-{}", self.unique_submit_count))
                        .expect("bounded fixture ID"),
                }),
            })
        });
        if result
            .as_ref()
            .ok()
            .and_then(|ack| ack.turn.as_ref())
            .is_some_and(|turn| Some(&turn.session) != self.session.as_ref())
        {
            result = Err(ProviderError::Rejected {
                message: "scripted acknowledgement namespace mismatch".to_string(),
            });
        }
        self.submissions
            .insert(request.attempt_key, (request_hash, result.clone()));
        result
    }

    fn next_event(&mut self, _timeout: Duration) -> Result<Option<ProviderEvent>, ProviderError> {
        let event = self.events.pop_front().transpose()?;
        if event
            .as_ref()
            .is_some_and(|event| event.provider_id != self.capabilities.provider_id)
        {
            return Err(ProviderError::Rejected {
                message: "event provider namespace mismatch".to_string(),
            });
        }
        Ok(event)
    }

    fn answer_request(
        &mut self,
        session: &ExternalSession,
        request_id: &ExternalId,
        _decision: ApprovalDecision,
    ) -> Result<(), ProviderError> {
        self.ensure_session(session)?;
        self.declined_requests
            .push((session.clone(), request_id.clone()));
        Ok(())
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

    fn close_session(&mut self, session: &ExternalSession) -> Result<(), ProviderError> {
        self.ensure_session(session)?;
        self.session = None;
        self.binding = None;
        Ok(())
    }
}

/// Synthetic envelopes for shared contract examples. They contain no real project or account data.
pub fn sample_artifacts() -> Result<
    (
        crate::execution_artifacts::WorkOrder,
        crate::execution_artifacts::WorkResult,
    ),
    String,
> {
    use crate::delegation::{DelegationTaskPacket, WorkerCheckResult, WorkerReport};
    use crate::execution_artifacts::*;
    let session = ExternalSession {
        provider_id: ProviderId::codex(),
        session_id: ExternalId::new("sample-session")?,
    };
    let envelope = ArtifactEnvelope {
        schema_version: 1,
        artifact_id: 1,
        scope: ExecutionScope {
            project_id: 1,
            task_id: 1,
            task_revision: 1,
            run_id: 1,
            delegation_id: 1,
            attempt_key: AttemptKey::new("sample-attempt-1")?,
        },
        parent_artifact_ids: Vec::new(),
        role: "worker".to_string(),
        session: Some(session.clone()),
        turn: None,
        observed_model: None,
        observed_harness_version: None,
        source_event_id: Some(1),
        repository: RepositoryObservation::default(),
        workspace_lease_id: None,
        created_at: 1,
        permission_manifest_hash: "b".repeat(64),
        verification: SubmissionVerification::Unverified,
        payload_hash: "0".repeat(64),
        evidence_artifact_ids: Vec::new(),
    };
    let order = WorkOrder::new(
        envelope.clone(),
        DelegationTaskPacket {
            schema_version: 1,
            title: "Synthetic contract task".to_string(),
            objective: "Demonstrate a bounded typed handoff".to_string(),
            acceptance_criteria: vec!["Contract fixture round-trips".to_string()],
            constraints: vec!["No provider or live data".to_string()],
            allowed_scope: vec!["src/**".to_string()],
            referenced_memory_ids: vec![1],
            referenced_artifact_ids: Vec::new(),
            requested_report_fields: vec![
                "summary".to_string(),
                "changed_paths".to_string(),
                "checks".to_string(),
            ],
        },
        ContextManifest {
            entries: vec![ContextEntry {
                memory_id: 1,
                content_hash: "a".repeat(64),
                delivery: ContextDelivery::Full,
            }],
            excluded_private_count: 0,
        },
    )?;
    let mut result_envelope = envelope;
    result_envelope.artifact_id = 2;
    result_envelope.parent_artifact_ids = vec![1];
    result_envelope.turn = Some(ExternalTurn {
        session,
        turn_id: ExternalId::new("sample-turn")?,
    });
    result_envelope.verification = SubmissionVerification::Reported;
    let result = WorkResult::new(
        result_envelope,
        WorkerReport {
            schema_version: 1,
            summary: "Synthetic result available for Cathulu review".to_string(),
            outcome: "completed".to_string(),
            changed_files: vec!["src/example.rs".to_string()],
            commits: Vec::new(),
            check_results: vec![WorkerCheckResult {
                name: "fixture".to_string(),
                status: "passed".to_string(),
                summary: "Synthetic check only".to_string(),
            }],
            blockers: Vec::new(),
            artifact_ids: Vec::new(),
            quality_check_ids: Vec::new(),
        },
    )?;
    result.validate_against(&order)?;
    Ok((order, result))
}
