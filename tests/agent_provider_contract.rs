use cathulu_core::agent_provider::{
    fixtures::{sample_artifacts, ScriptedProvider},
    *,
};
use cathulu_core::execution_artifacts::{WorkOrder, WorkResult};
use cathulu_core::provider_routing::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Duration;

fn config() -> SessionConfig {
    let root = std::env::current_dir().unwrap();
    SessionConfig {
        logical_role_key: "worker:1".to_string(),
        project_id: Some(1),
        permissions: PermissionManifest {
            schema_version: 1,
            workspace_root: root.clone(),
            access: WorkspaceAccess::ReadOnly,
            allowed_scope: vec!["src/**".to_string()],
            network_access: false,
            allowed_credential_sources: vec![CredentialSource::Subscription],
        },
        tools: McpToolConfig {
            server_name: "cathulu".to_string(),
            command: root.join("fixture-mcp.exe"),
            args: Vec::new(),
            env: BTreeMap::new(),
            required_tools: vec!["read_memories".to_string()],
        },
        model: None,
    }
}

fn submission(session: &ExternalSession, key: &str) -> SubmitTurnRequest {
    SubmitTurnRequest {
        session: session.clone(),
        attempt_key: AttemptKey::new(key).unwrap(),
        text: "Synthetic work order; no external execution".to_string(),
    }
}

#[test]
fn identifiers_are_validated_during_deserialization_and_namespaced() {
    assert!(ProviderId::new("Codex").is_err());
    assert!(ExternalId::new("x".repeat(257)).is_err());
    assert!(AttemptKey::new("bad\nkey").is_err());
    assert!(serde_json::from_value::<ProviderId>(json!("../provider")).is_err());
    let first = ExternalSession {
        provider_id: ProviderId::codex(),
        session_id: ExternalId::new("same-id").unwrap(),
    };
    let second = ExternalSession {
        provider_id: ProviderId::claude(),
        session_id: first.session_id.clone(),
    };
    assert_ne!(first, second);
    let keys: Vec<_> = [first, second.clone()]
        .into_iter()
        .map(|session| {
            ProviderEvent {
                provider_id: session.provider_id.clone(),
                event_id: Some(ExternalId::new("same-event-id").unwrap()),
                sequence: Some(1),
                event: ProviderEventKind::TurnStarted {
                    turn: ExternalTurn {
                        session,
                        turn_id: ExternalId::new("same-turn-id").unwrap(),
                    },
                },
            }
            .external_event_key()
            .unwrap()
            .unwrap()
        })
        .collect();
    assert_ne!(keys[0], keys[1]);
    let event = ProviderEvent {
        provider_id: ProviderId::codex(),
        event_id: None,
        sequence: None,
        event: ProviderEventKind::TurnStarted {
            turn: ExternalTurn {
                session: second,
                turn_id: ExternalId::new("turn").unwrap(),
            },
        },
    };
    assert!(event.validate().unwrap_err().contains("namespace"));
}

#[test]
fn lost_acknowledgement_never_creates_a_second_submission_for_the_attempt() {
    let mut provider = ScriptedProvider::new(ProviderId::codex());
    let session = provider.open_session(config()).unwrap();
    provider.queue_submission(Err(ProviderError::Uncertain {
        message: "lost ack".to_string(),
    }));
    let request = submission(&session, "attempt-1");
    assert!(matches!(
        provider.submit_turn(request.clone()),
        Err(ProviderError::Uncertain { .. })
    ));
    assert!(matches!(
        provider.submit_turn(request.clone()),
        Err(ProviderError::Uncertain { .. })
    ));
    assert_eq!(provider.unique_submit_count, 1);
    let mut changed = request.clone();
    changed.text.push_str(" changed");
    assert!(matches!(
        provider.submit_turn(changed),
        Err(ProviderError::Rejected { .. })
    ));
    provider.close_session(&session).unwrap();
    let replacement = provider.open_session(config()).unwrap();
    let mut rebound = request;
    rebound.session = replacement;
    assert!(matches!(
        provider.submit_turn(rebound),
        Err(ProviderError::Rejected { .. })
    ));
    assert_eq!(provider.unique_submit_count, 1);
}

#[test]
fn double_preserves_event_identity_denies_approval_and_exposes_unsupported_operations() {
    let mut provider = ScriptedProvider::new(ProviderId::codex());
    provider.capabilities().validate().unwrap();
    let session = provider.open_session(config()).unwrap();
    assert_eq!(
        provider.resume_session(&session, config()).unwrap(),
        session
    );
    let ack = provider
        .submit_turn(submission(&session, "attempt-events"))
        .unwrap();
    let turn = ack.turn.unwrap();
    let completed = ProviderEvent {
        provider_id: ProviderId::codex(),
        event_id: Some(ExternalId::new("completion-1").unwrap()),
        sequence: Some(2),
        event: ProviderEventKind::TurnCompleted {
            turn: turn.clone(),
            outcome: TurnOutcome::Completed,
        },
    };
    let started = ProviderEvent {
        provider_id: ProviderId::codex(),
        event_id: Some(ExternalId::new("start-1").unwrap()),
        sequence: Some(1),
        event: ProviderEventKind::TurnStarted { turn: turn.clone() },
    };
    assert_eq!(
        completed.external_event_key().unwrap(),
        completed.clone().external_event_key().unwrap()
    );
    assert_ne!(
        completed.external_event_key().unwrap(),
        started.external_event_key().unwrap()
    );
    let mut unidentified = completed.clone();
    unidentified.event_id = None;
    unidentified.sequence = None;
    assert_eq!(unidentified.external_event_key().unwrap(), None);
    for event in [completed.clone(), completed.clone(), started.clone()] {
        provider.queue_event(event).unwrap();
    }
    assert_eq!(
        provider.next_event(Duration::ZERO).unwrap(),
        Some(completed.clone())
    );
    assert_eq!(
        provider.next_event(Duration::ZERO).unwrap(),
        Some(completed)
    );
    assert_eq!(provider.next_event(Duration::ZERO).unwrap(), Some(started));
    let request_id = ExternalId::new("approval-1").unwrap();
    provider
        .answer_request(&session, &request_id, ApprovalDecision::Decline)
        .unwrap();
    assert_eq!(
        provider.declined_requests,
        vec![(session.clone(), request_id)]
    );
    assert_eq!(
        provider.interrupt(&turn),
        Err(ProviderError::Unsupported {
            operation: ProviderOperation::Interrupt
        })
    );
    assert_eq!(
        provider.session_status(&session),
        Err(ProviderError::Unsupported {
            operation: ProviderOperation::Status
        })
    );
    let unknown = ExternalSession {
        provider_id: ProviderId::codex(),
        session_id: ExternalId::new("gone").unwrap(),
    };
    assert!(matches!(
        provider.resume_session(&unknown, config()),
        Err(ProviderError::SessionGone { .. })
    ));
    let mut other_config = config();
    other_config.project_id = Some(2);
    assert!(matches!(
        provider.resume_session(&session, other_config),
        Err(ProviderError::Rejected { .. })
    ));
    let encoded = serde_json::to_value(&turn).unwrap();
    assert!(encoded.get("success").is_none());
}

#[test]
fn auth_capacity_malformed_and_oversized_inputs_remain_distinct() {
    let mut provider = ScriptedProvider::new(ProviderId::claude());
    let session = provider.open_session(config()).unwrap();
    for (index, reason, retry) in [
        (0, UnavailableReason::NotAuthenticated, None),
        (1, UnavailableReason::Capacity, Some(60)),
    ] {
        let error = ProviderError::Unavailable {
            reason,
            retry_after_seconds: retry,
            message: "synthetic unavailable".to_string(),
        };
        error.validate().unwrap();
        assert_eq!(
            ProviderError::parse(&serde_json::to_vec(&error).unwrap()).unwrap(),
            error
        );
        provider.queue_submission(Err(error.clone()));
        assert_eq!(
            provider.submit_turn(submission(&session, &format!("attempt-{index}"))),
            Err(error)
        );
    }
    provider.queue_raw_event(b"{truncated");
    assert!(matches!(
        provider.next_event(Duration::ZERO),
        Err(ProviderError::Rejected { .. })
    ));
    assert!(ProviderEvent::parse(&vec![b'x'; MAX_EVENT_BYTES + 1]).is_err());
    assert!(ProviderError::Unavailable {
        reason: UnavailableReason::AccountRestricted,
        retry_after_seconds: Some(60),
        message: "restricted".to_string()
    }
    .validate()
    .is_err());
    let event = ProviderEvent {
        provider_id: ProviderId::claude(),
        event_id: None,
        sequence: None,
        event: ProviderEventKind::Unrecognized {
            event_kind: "future".to_string(),
            size_bytes: 900,
            excerpt: Some("x".repeat(513)),
        },
    };
    assert!(event.validate().is_err());
}

#[test]
fn artifact_envelopes_reject_tampering_scope_changes_and_accepted_state() {
    let (order, result) = sample_artifacts().unwrap();
    let bytes = serde_json::to_vec(&order).unwrap();
    assert_eq!(
        WorkOrder::parse(&bytes, &order.envelope.scope).unwrap(),
        order
    );
    assert_eq!(
        WorkResult::parse(&serde_json::to_vec(&result).unwrap(), &order).unwrap(),
        result
    );
    let mut changed = serde_json::to_value(&order).unwrap();
    changed["payload"]["taskPacket"]["title"] = json!("tampered title");
    assert!(WorkOrder::parse(
        &serde_json::to_vec(&changed).unwrap(),
        &order.envelope.scope
    )
    .unwrap_err()
    .contains("hash"));
    changed = serde_json::to_value(&order).unwrap();
    changed["unexpected"] = json!(true);
    assert!(WorkOrder::parse(
        &serde_json::to_vec(&changed).unwrap(),
        &order.envelope.scope
    )
    .is_err());
    for field in ["projectId", "taskRevision", "runId"] {
        let mut changed = serde_json::to_value(&result).unwrap();
        changed["envelope"]["scope"][field] = json!(999);
        assert!(WorkResult::parse(&serde_json::to_vec(&changed).unwrap(), &order).is_err());
    }
    let mut changed_result = result.clone();
    changed_result.envelope.parent_artifact_ids.clear();
    assert!(changed_result.validate_against(&order).is_err());
    let mut changed = serde_json::to_value(&result).unwrap();
    changed["envelope"]["verification"] = json!("accepted");
    assert!(WorkResult::parse(&serde_json::to_vec(&changed).unwrap(), &order).is_err());
    let mut out_of_scope = result.report.clone();
    out_of_scope.changed_files = vec!["other/private.rs".to_string()];
    let out_of_scope = WorkResult::new(result.envelope.clone(), out_of_scope).unwrap();
    assert!(out_of_scope.validate_against(&order).is_err());
    assert!(WorkResult::parse(&vec![b'x'; 16 * 1024 + 1], &order).is_err());
}

#[test]
fn permission_scope_and_unknown_availability_cannot_silently_widen() {
    let config = config();
    config.validate().unwrap();
    assert_eq!(
        config.permissions.content_hash().unwrap(),
        config.permissions.content_hash().unwrap()
    );
    let mut invalid = config.clone();
    invalid.permissions.allowed_scope = vec!["../outside/**".to_string()];
    assert!(invalid.validate().is_err());
    invalid = config;
    invalid.permissions.allowed_credential_sources = vec![CredentialSource::Unknown];
    assert!(invalid.validate().is_err());
    let observation = AvailabilityObservation {
        provider_id: ProviderId::codex(),
        state: AvailabilityState::Available {
            quota: Some(QuotaObservation {
                remaining: 0,
                unit: "observed_calls".to_string(),
                resets_at: None,
            }),
        },
        observed_at: Some(100),
        source: ObservationSource::Preflight,
    };
    assert!(matches!(
        observation.effective_state(100, 60),
        AvailabilityState::Available {
            quota: Some(QuotaObservation { remaining: 0, .. })
        }
    ));
    assert_eq!(
        observation.effective_state(200, 60),
        AvailabilityState::Unknown {}
    );
    assert!(serde_json::from_value::<AvailabilityState>(
        json!({"kind":"unknown", "quota":{"remaining":0}})
    )
    .is_err());
    assert!(serde_json::from_value::<TaskShape>(
        json!({"kind":"administrative", "provider":"codex"})
    )
    .is_err());
    assert!(
        serde_json::from_value::<SessionStatus>(json!({"kind":"unknown", "active_turn":null}))
            .is_err()
    );
    assert!(serde_json::from_value::<RoutingOutcome>(
        json!({"kind":"no_llm", "provider_id":"codex"})
    )
    .is_err());
    let mut candidate = RoutingCandidate {
        capabilities: ScriptedProvider::new(ProviderId::codex()).capabilities(),
        availability: observation,
        all_required_disclosures_granted: false,
    };
    candidate.validate().unwrap();
    candidate.availability.provider_id = ProviderId::claude();
    assert!(candidate.validate().is_err());
}

#[test]
fn published_payloads_validate_against_the_actual_exported_types() {
    let examples: Value = serde_json::from_str(include_str!(
        "../docs/parallel/codex/F1-sample-payloads.json"
    ))
    .unwrap();
    let (order, result) = sample_artifacts().unwrap();
    assert_eq!(examples["workOrder"], serde_json::to_value(&order).unwrap());
    assert_eq!(
        examples["workResult"],
        serde_json::to_value(&result).unwrap()
    );
    WorkOrder::parse(
        &serde_json::to_vec(&examples["workOrder"]).unwrap(),
        &order.envelope.scope,
    )
    .unwrap();
    WorkResult::parse(
        &serde_json::to_vec(&examples["workResult"]).unwrap(),
        &order,
    )
    .unwrap();
    ProviderEvent::parse(&serde_json::to_vec(&examples["event"]).unwrap()).unwrap();
    assert!(matches!(
        ProviderError::parse(&serde_json::to_vec(&examples["error"]).unwrap()).unwrap(),
        ProviderError::Uncertain { .. }
    ));
    serde_json::from_value::<RoutingRequest>(examples["routingRequest"].clone()).unwrap();
    let candidate: RoutingCandidate =
        serde_json::from_value(examples["routingCandidate"].clone()).unwrap();
    candidate.validate().unwrap();
    assert!(!candidate.all_required_disclosures_granted);
    assert_eq!(
        serde_json::from_value::<AvailabilityState>(examples["unknownAvailability"].clone())
            .unwrap(),
        AvailabilityState::Unknown {}
    );
    let _: Box<dyn AgentProvider> = Box::new(ScriptedProvider::new(ProviderId::codex()));
}
