//! Compact, evidence-backed terminal projection. This never owns orchestration state.

use crate::dashboard::DashboardSnapshot;

fn compact(value: &str, limit: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let short = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}

fn indicator(status: &str) -> &'static str {
    match status {
        "running" | "active" | "dispatched" | "in_progress" => ">",
        "queued" | "prepared" | "pending" => "·",
        "blocked" | "failed" | "needs_reconciliation" => "!",
        "completed" | "verified_success" | "accepted" => "✓",
        _ => "-",
    }
}

/// A bounded snapshot for a terminal 60 columns wide or wider. Data is labelled as recorded;
/// absence of a checkpoint never implies that an agent is currently working or has stopped.
pub fn render(snapshot: &DashboardSnapshot, width: usize) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    render_at(snapshot, width, now)
}

fn render_at(snapshot: &DashboardSnapshot, width: usize, now: i64) -> String {
    let width = width.max(60);
    let detail = width.saturating_sub(34).min(80);
    let mut lines = vec!["CATHULU  •  recorded state".to_string()];
    let queued = snapshot
        .role_dispatches
        .iter()
        .filter(|item| item.status == "queued")
        .count();
    let running = snapshot
        .role_dispatches
        .iter()
        .filter(|item| item.status == "running")
        .count();
    let attention =
        snapshot.pending_agent_actions.len() + snapshot.pending_owner_approval_requests.len();
    lines.push(format!(
        "{} projects  |  {} queued  |  {} role turns  |  {} attention",
        snapshot.projects.len(),
        queued,
        running,
        attention
    ));
    lines.push(String::new());
    lines.push("MAIN OVERSEER  •  logical role; turn not observed".to_string());
    lines.push("PROJECTS".to_string());

    for project in &snapshot.projects {
        let current = snapshot
            .delegations
            .iter()
            .filter(|item| {
                item.project_id == project.id
                    && item.parent_run_status == "running"
                    && now.saturating_sub(item.updated_at) <= 900
                    && !matches!(item.status.as_str(), "completed" | "cancelled" | "failed")
            })
            .max_by_key(|item| item.updated_at);
        let dispatch = snapshot
            .role_dispatches
            .iter()
            .filter(|item| {
                item.project_id == project.id
                    && matches!(item.status.as_str(), "queued" | "running")
                    && now.saturating_sub(item.updated_at) <= 900
            })
            .max_by_key(|item| item.updated_at);
        let last_run = snapshot
            .recent_runs
            .iter()
            .filter(|item| item.project_name == project.name)
            .max_by_key(|item| item.started_at);
        let update = snapshot
            .agent_updates
            .iter()
            .filter(|item| item.project_id == project.id)
            .max_by_key(|item| item.created_at);
        let state_code = dispatch
            .map(|item| item.status.as_str())
            .or_else(|| current.map(|item| item.status.as_str()))
            .or_else(|| last_run.map(|item| item.status.as_str()))
            .unwrap_or("");
        let state = dispatch
            .map(|item| format!("role turn {} (recorded)", item.status))
            .or_else(|| current.map(|item| format!("assignment {} (unconfirmed)", item.status)))
            .or_else(|| last_run.map(|item| format!("last run {}", item.status)))
            .unwrap_or_else(|| "no recorded assignment".to_string());
        lines.push(format!(
            "{} {}  {}",
            indicator(state_code),
            compact(&project.name, 22),
            state
        ));
        if let Some(item) = current {
            lines.push(format!(
                "    {} #{}: {}",
                if item.role == "worker" {
                    "Worker"
                } else {
                    "PM"
                },
                item.id,
                compact(&item.task_title, detail)
            ));
        }
        if let Some(item) = update {
            let label = if now.saturating_sub(item.created_at) > 900 {
                "last note (historical)"
            } else {
                "latest note"
            };
            lines.push(format!("    {label}: {}", compact(&item.summary, detail)));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "MEMORY  hot {} B  warm {} B  cold {} B",
        snapshot.memory_health.hot_bytes,
        snapshot.memory_health.warm_bytes,
        snapshot.memory_health.cold_bytes
    ));
    lines.push(format!(
        "        canonical {}  episodic {}  evidence {}",
        snapshot.memory_health.canonical_count,
        snapshot.memory_health.episodic_count,
        snapshot.memory_health.evidence_count
    ));
    if attention > 0 {
        lines.push(String::new());
        lines.push("ATTENTION".to_string());
        for action in snapshot.pending_owner_approval_requests.iter().take(3) {
            lines.push(format!(
                "! {}: {}",
                compact(&action.project_name, 20),
                compact(&action.owner_action, detail)
            ));
        }
        for action in snapshot.pending_agent_actions.iter().take(3) {
            lines.push(format!(
                "! {}: {}",
                compact(&action.project_name, 20),
                compact(&action.summary, detail)
            ));
        }
    }
    if !snapshot.failed_checks.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "QA  {} failed/inconclusive checks recorded",
            snapshot.failed_checks.len()
        ));
    }
    lines.push(String::new());
    let recent = snapshot
        .agent_updates
        .iter()
        .filter(|item| now.saturating_sub(item.created_at) <= 900)
        .take(3)
        .collect::<Vec<_>>();
    lines.push("RECENT AGENT CHECKPOINTS".to_string());
    if recent.is_empty() {
        lines.push("No checkpoint recorded in the last 15 min".to_string());
    } else {
        for item in recent {
            lines.push(format!(
                "• {}: {}",
                compact(&item.project_name, 16),
                compact(&item.summary, detail)
            ));
        }
    }
    lines.push(String::new());
    lines.push("Recorded state only • --json for full evidence".to_string());
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegation::DashboardDelegationSummary;
    use crate::{dashboard::MemoryHealthSummary, ProjectSnapshot};

    #[test]
    fn render_projects_and_memory_without_claiming_unobserved_work() {
        let mut snapshot = DashboardSnapshot {
            logical_roles: vec![],
            projects: vec![ProjectSnapshot {
                id: 1,
                name: "PULSO".into(),
                path: "C:/pulso".into(),
                branch: None,
                head: None,
                dirty: false,
                changed_paths: vec![],
                registered_at: 0,
                reconciled_at: 0,
            }],
            objectives: vec![],
            blockers: vec![],
            recent_runs: vec![],
            agent_activity: vec![],
            agent_updates: vec![],
            pending_agent_actions: vec![],
            pending_owner_approval_requests: vec![],
            recent_agent_update_count: 0,
            failed_checks: vec![],
            delegations: vec![],
            role_dispatches: vec![],
            role_inbox: vec![],
            memory_health: MemoryHealthSummary {
                hot_bytes: 42,
                ..Default::default()
            },
        };
        let output = render(&snapshot, 60);
        assert!(output.contains("PULSO  no recorded assignment"));
        assert!(output.contains("hot 42 B"));
        assert!(!output.contains("working"));
        assert!(output.lines().all(|line| line.chars().count() <= 60));

        snapshot.delegations.push(DashboardDelegationSummary {
            id: 7,
            project_id: 1,
            project_name: "PULSO".into(),
            task_title: "Old audit".into(),
            run_id: 3,
            role: "worker".into(),
            status: "dispatched".into(),
            parent_delegation_id: None,
            logical_role_key: None,
            project_manager_assignment_status: None,
            activation_status: None,
            parent_run_status: "inconclusive".into(),
            created_at: 10,
            updated_at: 10,
            report: None,
            worktree: None,
            worker_telemetry: None,
        });
        let historical = render_at(&snapshot, 60, 20);
        assert!(historical.contains("no recorded assignment"));
        assert!(!historical.contains("Old audit"));
    }
}
