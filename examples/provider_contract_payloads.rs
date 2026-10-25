use cathulu_core::agent_provider::{fixtures::sample_artifacts, *};
use cathulu_core::provider_routing::*;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (order, result) = sample_artifacts()?;
    let fixture =
        cathulu_core::agent_provider::fixtures::ScriptedProvider::new(ProviderId::codex());
    let routing_candidate = RoutingCandidate {
        capabilities: fixture.capabilities(),
        availability: AvailabilityObservation {
            provider_id: ProviderId::codex(),
            state: AvailabilityState::Unknown {},
            observed_at: None,
            source: ObservationSource::Unknown,
        },
        all_required_disclosures_granted: false,
    };
    routing_candidate.validate()?;
    let event = ProviderEvent {
        provider_id: ProviderId::codex(),
        event_id: Some(ExternalId::new("sample-event")?),
        sequence: Some(1),
        event: ProviderEventKind::TurnCompleted {
            turn: result.envelope.turn.clone().expect("sample turn"),
            outcome: TurnOutcome::Completed,
        },
    };
    event.validate()?;
    let error = ProviderError::Uncertain {
        message: "Submission acknowledgement was lost; reconcile before another attempt."
            .to_string(),
    };
    let request = RoutingRequest {
        task_shape: TaskShape::Bounded {},
        risk: Risk::Low,
        requires_write: true,
        required_review: true,
        owner_pin: None,
        owner_excluded: Vec::new(),
        owner_preference: Vec::new(),
        now: 1,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "contractVersion": CONTRACT_VERSION,
            "workOrder": order,
            "workResult": result,
            "event": event,
            "error": error,
            "routingRequest": request,
            "routingCandidate": routing_candidate,
            "unknownAvailability": AvailabilityState::Unknown {}
        }))?
    );
    Ok(())
}
