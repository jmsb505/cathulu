use crate::delegation::{ActivationMemoryReference, DelegationTaskPacket, WorkerReport};
use crate::gardener::{MemoryPolicyEvaluationInput, MemoryPolicyInput};
use crate::memory::{FreshnessBasis, MemoryCreateInput, MemorySource};
use crate::optimization::{
    ContextModel, DecisionModel, DisabledContextModel, DisabledDecisionModel,
    OptimizationCapability, OptimizationConfig, ProviderLocality, RoutingDecisionSchema,
};
use crate::provider_adapters::{JevProvider, ScaleDownContextProvider};
use crate::telemetry::CodexHookEvent;
use crate::workflow::EventRecord;
use crate::{default_data_dir, Store};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ListResourceTemplatesResult, ListResourcesResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult,
    Resource, ResourceContents, ResourceTemplate, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
    ServiceExt,
};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const SERVER_NAME: &str = "cathulu";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct CathuluServer {
    store: Store,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RegisterProjectArgs {
    /// Absolute or relative path to a local Git repository.
    path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ReconcileProjectArgs {
    /// Cathulu project ID returned by register_project.
    project_id: i64,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TraceCodePathArgs {
    project_id: i64,
    source_symbol: String,
    target_symbol: String,
    #[serde(default)]
    source_file: Option<String>,
    #[serde(default)]
    target_file: Option<String>,
    #[serde(default)]
    max_hops: Option<usize>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ProjectDependencyArgs {
    from_project_id: i64,
    to_project_id: i64,
    relation: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateObjectiveArgs {
    project_id: i64,
    title: String,
    #[serde(default)]
    description: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetStatusArgs {
    id: i64,
    status: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateTaskArgs {
    objective_id: i64,
    title: String,
    #[serde(default)]
    description: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RecordDecisionArgs {
    project_id: i64,
    #[serde(default)]
    objective_id: Option<i64>,
    title: String,
    body: String,
    #[serde(default)]
    supersedes_decision_id: Option<i64>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateWorkflowVersionArgs {
    workflow_key: String,
    name: String,
    version: i64,
    definition: Value,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct StartRunArgs {
    project_id: i64,
    #[serde(default)]
    objective_id: Option<i64>,
    #[serde(default)]
    task_id: Option<i64>,
    #[serde(default)]
    workflow_version_id: Option<i64>,
    /// Optional per-run specialist treatment configuration. All capabilities
    /// remain disabled unless explicitly enabled with shadow mode.
    #[serde(default)]
    optimization_config: Option<Value>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DecisionShadowTrialArgs {
    run_id: i64,
    invocation_id: String,
    capability: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ContextExtractionShadowTrialArgs {
    run_id: i64,
    invocation_id: String,
    artifact_ids: Vec<i64>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PrepareDelegationArgs {
    project_id: i64,
    task_id: i64,
    run_id: i64,
    role: String,
    #[serde(default)]
    parent_delegation_id: Option<i64>,
    task_packet: DelegationTaskPacket,
    idempotency_key: String,
    #[serde(default)]
    requested_model: Option<String>,
    #[serde(default)]
    requested_effort: Option<String>,
    #[serde(default)]
    external_correlation: Option<String>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RecordDelegationActivationArgs {
    delegation_id: i64,
    receiver_scope_type: String,
    receiver_scope_id: String,
    disposition: String,
    memory_tools_available: bool,
    retrieval_completed: bool,
    #[serde(default)]
    memory_references: Vec<ActivationMemoryReference>,
    idempotency_key: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct DelegationIdArgs {
    delegation_id: i64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RecordDelegationUpdateArgs {
    delegation_id: i64,
    update_type: String,
    #[serde(default)]
    phase: Option<String>,
    summary: String,
    #[serde(default)]
    action_key: Option<String>,
    #[serde(default)]
    owner_action: Option<String>,
    idempotency_key: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SubmitDelegationReportArgs {
    delegation_id: i64,
    worker_report: WorkerReport,
    idempotency_key: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReviewDelegationReportArgs {
    delegation_id: i64,
    decision: String,
    #[serde(default)]
    note: String,
    idempotency_key: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReconcileDelegationWorktreeArgs {
    delegation_id: i64,
    /// Absolute or relative path to an already-existing Git worktree belonging to the registered project.
    worktree_path: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AssociateDelegationWorkerArgs {
    delegation_id: i64,
    hook_event_id: i64,
    idempotency_key: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CodexHookEventArgs {
    hook_event_name: String,
    session_id: String,
    turn_id: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_use_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    agent_type: Option<String>,
    #[serde(default)]
    model: Option<String>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RecordQualityCheckArgs {
    run_id: i64,
    check_key: String,
    mandatory: bool,
    status: String,
    #[serde(default = "empty_object")]
    details: Value,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct FinishRunArgs {
    run_id: i64,
    summary: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RecordArtifactArgs {
    #[serde(default)]
    run_id: Option<i64>,
    artifact_kind: String,
    uri: String,
    #[serde(default)]
    content_hash: Option<String>,
    #[serde(default)]
    protected: bool,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateMemoryArgs {
    scope_type: String,
    #[serde(default)]
    scope_id: Option<String>,
    memory_class: String,
    temperature: String,
    kind: String,
    body: String,
    #[serde(default)]
    sources: Vec<MemorySource>,
    #[serde(default)]
    artifact_ids: Vec<i64>,
    #[serde(default)]
    valid_from: Option<i64>,
    #[serde(default)]
    valid_until: Option<i64>,
    #[serde(default)]
    supersedes_id: Option<i64>,
    #[serde(default)]
    freshness_basis: FreshnessBasis,
    importance: f64,
    confidence: f64,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    protected: bool,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ReviewMemoryArgs {
    memory_id: i64,
    decision: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetMemoryTemperatureArgs {
    memory_id: i64,
    temperature: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetMemoryActiveArgs {
    memory_id: i64,
    active: bool,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SetMemoryBudgetArgs {
    scope_type: String,
    #[serde(default)]
    scope_id: Option<String>,
    hot_budget_bytes: i64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PrepareTransferArgs {
    memory_id: i64,
    sender_scope_type: String,
    #[serde(default)]
    sender_scope_id: Option<String>,
    receiver_scope_type: String,
    #[serde(default)]
    receiver_scope_id: Option<String>,
    #[serde(default)]
    run_id: Option<i64>,
    #[serde(default = "default_retrieval_method")]
    retrieval_method: String,
    #[serde(default)]
    token_count: Option<i64>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ReadMemoriesArgs {
    scope_type: String,
    #[serde(default)]
    scope_id: Option<String>,
    #[serde(default)]
    include_history: bool,
    #[serde(default)]
    include_cold: bool,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchMemoryArgs {
    scope_type: String,
    #[serde(default)]
    scope_id: Option<String>,
    query: String,
    receiver_scope_type: String,
    #[serde(default)]
    receiver_scope_id: Option<String>,
    #[serde(default = "default_memory_search_limit")]
    limit: i64,
    #[serde(default)]
    delegation_id: Option<i64>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ConsolidateMemoriesArgs {
    scope_type: String,
    #[serde(default)]
    scope_id: Option<String>,
    source_memory_ids: Vec<i64>,
    body: String,
    kind: String,
    importance: f64,
    confidence: f64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RunMemoryGardenerArgs {
    #[serde(default = "default_promotion_support")]
    minimum_promotion_support: i64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ReviewMaintenanceProposalArgs {
    proposal_id: i64,
    decision: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateMemoryPolicyArgs {
    policy_key: String,
    version: i64,
    retrieval_method: String,
    hot_budget_bytes: i64,
    warm_candidate_limit: i64,
    minimum_promotion_support: i64,
    #[serde(default)]
    retention_minimum_age_seconds: Option<i64>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RecordMemoryPolicyEvaluationArgs {
    policy_id: i64,
    experiment_key: String,
    task_type: String,
    run_id: i64,
    missing_context_failures: i64,
    regressions: i64,
    rework_count: i64,
    context_bytes: i64,
    reintroduced_bytes: i64,
    useful_retrievals: i64,
    retrieval_count: i64,
    #[serde(default)]
    latency_ms: Option<i64>,
    human_interventions: i64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CompareMemoryPolicyArgs {
    experiment_key: String,
}

fn empty_object() -> Value {
    serde_json::json!({})
}
fn default_retrieval_method() -> String {
    "explicit".to_string()
}
fn default_memory_search_limit() -> i64 {
    10
}
fn default_promotion_support() -> i64 {
    2
}

#[tool_router]
impl CathuluServer {
    #[tool(
        description = "Register a local Git repository and save its current branch, HEAD, and changed paths."
    )]
    fn register_project(
        &self,
        Parameters(arguments): Parameters<RegisterProjectArgs>,
    ) -> CallToolResult {
        match self
            .store
            .register_project(std::path::Path::new(&arguments.path))
        {
            Ok(project) => success(project),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "Refresh a registered repository's branch, HEAD, dirty state, and changed paths."
    )]
    fn reconcile_project(
        &self,
        Parameters(arguments): Parameters<ReconcileProjectArgs>,
    ) -> CallToolResult {
        match self.store.reconcile_project(arguments.project_id) {
            Ok(project) => success(project),
            Err(error) => tool_error(error),
        }
    }

    #[tool(
        description = "Trace a bounded directed path between code symbols in an optional local Graphify snapshot. Results include current source locations; unavailable snapshots return no candidates and leave SQLite/FTS as authority."
    )]
    fn trace_code_path(
        &self,
        Parameters(arguments): Parameters<TraceCodePathArgs>,
    ) -> CallToolResult {
        self.store
            .trace_code_path(
                arguments.project_id,
                &arguments.source_symbol,
                &arguments.target_symbol,
                arguments.source_file.as_deref(),
                arguments.target_file.as_deref(),
                arguments
                    .max_hops
                    .unwrap_or(crate::graphify::DEFAULT_MAX_HOPS),
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Record a directed relationship between two registered local projects.")]
    fn record_dependency(
        &self,
        Parameters(a): Parameters<ProjectDependencyArgs>,
    ) -> CallToolResult {
        self.store
            .record_dependency(a.from_project_id, a.to_project_id, &a.relation)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Create a durable project objective.")]
    fn create_objective(&self, Parameters(a): Parameters<CreateObjectiveArgs>) -> CallToolResult {
        self.store
            .create_objective(a.project_id, &a.title, &a.description)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Change an objective status to active, blocked, completed, or cancelled.")]
    fn set_objective_status(&self, Parameters(a): Parameters<SetStatusArgs>) -> CallToolResult {
        self.store
            .set_objective_status(a.id, &a.status)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Create a task inside an objective.")]
    fn create_task(&self, Parameters(a): Parameters<CreateTaskArgs>) -> CallToolResult {
        self.store
            .create_task(a.objective_id, &a.title, &a.description)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Change a task status to pending, active, blocked, completed, or cancelled."
    )]
    fn set_task_status(&self, Parameters(a): Parameters<SetStatusArgs>) -> CallToolResult {
        self.store
            .set_task_status(a.id, &a.status)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Record an immutable, provenance-bearing project decision.")]
    fn record_decision(&self, Parameters(a): Parameters<RecordDecisionArgs>) -> CallToolResult {
        self.store
            .record_decision(
                a.project_id,
                a.objective_id,
                &a.title,
                &a.body,
                a.supersedes_decision_id,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Create or retrieve an immutable version of a quality workflow.")]
    fn create_workflow_version(
        &self,
        Parameters(a): Parameters<CreateWorkflowVersionArgs>,
    ) -> CallToolResult {
        self.store
            .create_workflow_version(&a.workflow_key, &a.name, a.version, &a.definition)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Start an auditable run for a project, objective, task, and workflow version."
    )]
    fn start_run(&self, Parameters(a): Parameters<StartRunArgs>) -> CallToolResult {
        let optimization_config = match a.optimization_config {
            Some(value) => match serde_json::from_value::<OptimizationConfig>(value) {
                Ok(config) => config,
                Err(error) => return tool_error(format!("invalid optimization_config: {error}")),
            },
            None => OptimizationConfig::default(),
        };
        self.store
            .start_run_with_optimization(
                a.project_id,
                a.objective_id,
                a.task_id,
                a.workflow_version_id,
                &optimization_config,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Run a Jev workflow/delegation prediction in shadow mode. This makes a remote call only when the run's immutable treatment explicitly enables the Jev capability, sets providerLocality=remote and remoteDataAllowed=true, and a local API key is available. It never changes execution."
    )]
    async fn run_decision_shadow_trial(
        &self,
        Parameters(a): Parameters<DecisionShadowTrialArgs>,
    ) -> CallToolResult {
        let result = async {
            let schema = match a.capability.as_str() {
                "workflow_routing" => RoutingDecisionSchema::WorkflowClass,
                "delegation_routing" => RoutingDecisionSchema::DelegationStrategy,
                _ => {
                    return Err(
                        "capability must be workflow_routing or delegation_routing".to_string()
                    )
                }
            };
            let request = schema.request(&self.store.routing_decision_state(a.run_id)?)?;
            let treatment = self
                .store
                .get_run_optimization_treatment(a.run_id)?
                .ok_or_else(|| {
                    format!("run {} has no optimization treatment snapshot", a.run_id)
                })?;
            let settings = treatment.configuration.settings(schema.capability());
            let model: Box<dyn DecisionModel> = if settings.enabled
                && settings.provider.as_deref() == Some("jev")
                && settings.provider_locality == Some(ProviderLocality::Remote)
                && settings.remote_data_allowed
            {
                JevProvider::from_environment(settings.model.as_deref())
                    .map(|provider| Box::new(provider) as Box<dyn DecisionModel>)
                    .unwrap_or_else(|| Box::new(DisabledDecisionModel))
            } else {
                Box::new(DisabledDecisionModel)
            };
            self.store
                .run_decision_shadow_trial(a.run_id, &a.invocation_id, &request, model.as_ref())
                .await
        }
        .await;
        result.map(success).unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Run ScaleDown extraction over selected spans from registered test/compiler/CI/tool-output artifacts in shadow mode. This makes a remote call only when the run's immutable context_extract treatment explicitly enables provider=scaledown, providerLocality=remote and remoteDataAllowed=true, and a local API key is available. Cathulu sends selected excerpts only; raw evidence and workflow execution remain unchanged."
    )]
    async fn run_context_extraction_shadow_trial(
        &self,
        Parameters(a): Parameters<ContextExtractionShadowTrialArgs>,
    ) -> CallToolResult {
        let result = async {
            let request = self
                .store
                .prepare_context_extraction_request(&a.artifact_ids)?;
            let treatment = self
                .store
                .get_run_optimization_treatment(a.run_id)?
                .ok_or_else(|| {
                    format!("run {} has no optimization treatment snapshot", a.run_id)
                })?;
            let settings = treatment
                .configuration
                .settings(OptimizationCapability::ContextExtract);
            let model: Box<dyn ContextModel> = if settings.enabled
                && settings.provider.as_deref() == Some("scaledown")
                && settings.provider_locality == Some(ProviderLocality::Remote)
                && settings.remote_data_allowed
            {
                ScaleDownContextProvider::from_environment(settings.model.as_deref())
                    .map(|provider| Box::new(provider) as Box<dyn ContextModel>)
                    .unwrap_or_else(|| Box::new(DisabledContextModel))
            } else {
                Box::new(DisabledContextModel)
            };
            self.store
                .run_context_extraction_shadow_trial(
                    a.run_id,
                    &a.invocation_id,
                    &request,
                    model.as_ref(),
                )
                .await
        }
        .await;
        result.map(success).unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Prepare an auditable native-Codex delegation packet for an existing task and run. This records the request only; it does not spawn a worker or change Codex configuration."
    )]
    fn prepare_delegation(
        &self,
        Parameters(a): Parameters<PrepareDelegationArgs>,
    ) -> CallToolResult {
        self.store
            .prepare_delegation(
                a.project_id,
                a.task_id,
                a.run_id,
                &a.role,
                a.parent_delegation_id,
                &a.task_packet,
                &a.idempotency_key,
                a.requested_model.as_deref(),
                a.requested_effort.as_deref(),
                a.external_correlation.as_deref(),
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Record an append-only memory activation receipt for a hierarchy-aware Project Manager or Worker before dispatch. Each referenced memory must be current, in scope, and either transferred to or already known by the exact logical receiver. Use no_relevant_memory only after memory retrieval completed successfully and returned no candidates."
    )]
    fn record_delegation_activation(
        &self,
        Parameters(a): Parameters<RecordDelegationActivationArgs>,
    ) -> CallToolResult {
        self.store
            .record_delegation_activation(
                a.delegation_id,
                &a.receiver_scope_type,
                &a.receiver_scope_id,
                &a.disposition,
                a.memory_tools_available,
                a.retrieval_completed,
                &a.memory_references,
                &a.idempotency_key,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Record that the owner dispatched a prepared delegation using native Codex. This does not start or control a worker."
    )]
    fn mark_delegation_dispatched(
        &self,
        Parameters(a): Parameters<DelegationIdArgs>,
    ) -> CallToolResult {
        self.store
            .mark_delegation_dispatched(a.delegation_id)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Mark a prepared or dispatched delegation as cancelled in Cathulu's audit record."
    )]
    fn cancel_delegation(&self, Parameters(a): Parameters<DelegationIdArgs>) -> CallToolResult {
        self.store
            .cancel_delegation(a.delegation_id)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Record a short, self-reported progress checkpoint or owner action for a dispatched Project Manager/Worker delegation. Use at meaningful state changes, not per tool call; during sustained work, report a brief checkpoint about every 10 minutes so the dashboard can distinguish recent activity from an unconfirmed signal. update_type is working, waiting, action_required, or action_resolved. For action_required, provide a stable action_key and the exact owner_action/question; clear it with action_resolved using the same key after the owner responds. This event is not proof of code changes or successful QA."
    )]
    fn record_delegation_update(
        &self,
        Parameters(a): Parameters<RecordDelegationUpdateArgs>,
    ) -> CallToolResult {
        self.store
            .record_delegation_update(
                a.delegation_id,
                &a.update_type,
                a.phase.as_deref(),
                &a.summary,
                a.action_key.as_deref(),
                a.owner_action.as_deref(),
                &a.idempotency_key,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Complete a dispatched Project Manager assignment after every child worker has reported or been cancelled. This releases the project's active-manager slot and records an audit event."
    )]
    fn complete_project_manager_assignment(
        &self,
        Parameters(a): Parameters<DelegationIdArgs>,
    ) -> CallToolResult {
        self.store
            .complete_project_manager_assignment(a.delegation_id)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Submit one bounded, idempotent worker report for a dispatched delegation. Reported claims and referenced evidence remain separate from parent-run verification."
    )]
    fn submit_delegation_report(
        &self,
        Parameters(a): Parameters<SubmitDelegationReportArgs>,
    ) -> CallToolResult {
        self.store
            .submit_delegation_report(a.delegation_id, &a.worker_report, &a.idempotency_key)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Append an explicit review decision for a worker report: accepted, rejected, or needs_follow_up. This does not accept code or change parent-run quality state."
    )]
    fn review_delegation_report(
        &self,
        Parameters(a): Parameters<ReviewDelegationReportArgs>,
    ) -> CallToolResult {
        self.store
            .review_delegation_report(a.delegation_id, &a.decision, &a.note, &a.idempotency_key)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Read and reconcile Git facts from an existing worktree that belongs to the registered project. Cathulu does not create, move, remove, or merge worktrees."
    )]
    fn reconcile_delegation_worktree(
        &self,
        Parameters(a): Parameters<ReconcileDelegationWorktreeArgs>,
    ) -> CallToolResult {
        if a.worktree_path.len() > 4_096 || a.worktree_path.chars().any(char::is_control) {
            return tool_error(
                "worktree path must be bounded and contain no control characters".to_string(),
            );
        }
        self.store
            .reconcile_delegation_worktree(a.delegation_id, Path::new(&a.worktree_path))
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Explicitly associate a recorded SubagentStart event with a dispatched delegation. Matching uses the parent run and the selected hook event; no worker is inferred or started."
    )]
    fn associate_delegation_worker(
        &self,
        Parameters(a): Parameters<AssociateDelegationWorkerArgs>,
    ) -> CallToolResult {
        self.store
            .associate_delegation_worker(a.delegation_id, a.hook_event_id, &a.idempotency_key)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Record or update a named quality check for an open run.")]
    fn record_quality_check(
        &self,
        Parameters(a): Parameters<RecordQualityCheckArgs>,
    ) -> CallToolResult {
        self.store
            .record_quality_check(a.run_id, &a.check_key, a.mandatory, &a.status, &a.details)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Finish a run. Verified success requires all mandatory workflow checks to pass."
    )]
    fn finish_run(&self, Parameters(a): Parameters<FinishRunArgs>) -> CallToolResult {
        self.store
            .finish_run(a.run_id, &a.summary)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Record allowlisted Codex hook metadata for a registered repository. Never pass prompt text, transcripts, tool inputs, or tool outputs."
    )]
    fn record_codex_hook_event(
        &self,
        Parameters(a): Parameters<CodexHookEventArgs>,
    ) -> CallToolResult {
        self.store
            .record_codex_hook_event(CodexHookEvent {
                hook_event_name: a.hook_event_name,
                session_id: a.session_id,
                turn_id: a.turn_id,
                cwd: a.cwd,
                tool_name: a.tool_name,
                tool_use_id: a.tool_use_id,
                agent_id: a.agent_id,
                agent_type: a.agent_type,
                model: a.model,
            })
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Record a local evidence artifact associated with a run.")]
    fn record_evidence_artifact(
        &self,
        Parameters(a): Parameters<RecordArtifactArgs>,
    ) -> CallToolResult {
        self.store
            .record_evidence_artifact(
                a.run_id,
                &a.artifact_kind,
                &a.uri,
                a.content_hash.as_deref(),
                a.protected,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Create a durable memory candidate with provenance, freshness, temporal validity, and stable identity."
    )]
    fn create_memory(&self, Parameters(a): Parameters<CreateMemoryArgs>) -> CallToolResult {
        let input = MemoryCreateInput {
            scope_type: a.scope_type,
            scope_id: a.scope_id,
            memory_class: a.memory_class,
            temperature: a.temperature,
            kind: a.kind,
            body: a.body,
            sources: a.sources,
            artifact_ids: a.artifact_ids,
            valid_from: a.valid_from,
            valid_until: a.valid_until,
            supersedes_id: a.supersedes_id,
            freshness_basis: a.freshness_basis,
            importance: a.importance,
            confidence: a.confidence,
            active: a.active,
            protected: a.protected,
        };
        self.store
            .create_memory(&input)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Accept or reject a memory candidate after review.")]
    fn review_memory_candidate(
        &self,
        Parameters(a): Parameters<ReviewMemoryArgs>,
    ) -> CallToolResult {
        self.store
            .review_memory_candidate(a.memory_id, &a.decision)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Change memory temperature without changing its identity or memory class."
    )]
    fn set_memory_temperature(
        &self,
        Parameters(a): Parameters<SetMemoryTemperatureArgs>,
    ) -> CallToolResult {
        self.store
            .set_memory_temperature(a.memory_id, &a.temperature)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Mark a memory as active or inactive for its logical scope.")]
    fn set_memory_active(&self, Parameters(a): Parameters<SetMemoryActiveArgs>) -> CallToolResult {
        self.store
            .set_memory_active(a.memory_id, a.active)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Set the bounded HOT-memory byte budget for a logical scope.")]
    fn set_memory_hot_budget(
        &self,
        Parameters(a): Parameters<SetMemoryBudgetArgs>,
    ) -> CallToolResult {
        self.store
            .set_memory_hot_budget(&a.scope_type, a.scope_id.as_deref(), a.hot_budget_bytes)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Prepare an auditable context transfer, omitting unchanged known memory and excluding stale or COLD memory."
    )]
    fn prepare_context_transfer(
        &self,
        Parameters(a): Parameters<PrepareTransferArgs>,
    ) -> CallToolResult {
        self.store
            .prepare_context_transfer(
                a.memory_id,
                &a.sender_scope_type,
                a.sender_scope_id.as_deref(),
                &a.receiver_scope_type,
                a.receiver_scope_id.as_deref(),
                a.run_id,
                &a.retrieval_method,
                a.token_count,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Read current accepted memory for a scope, optionally including history or COLD records."
    )]
    fn read_memories(&self, Parameters(a): Parameters<ReadMemoriesArgs>) -> CallToolResult {
        self.store
            .list_memories(
                &a.scope_type,
                a.scope_id.as_deref(),
                a.include_history,
                a.include_cold,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Find compact HOT and selective WARM memory candidates. Results contain IDs and hashes only; use prepare_context_transfer to send an auditable body or delta."
    )]
    fn search_memory_candidates(
        &self,
        Parameters(a): Parameters<SearchMemoryArgs>,
    ) -> CallToolResult {
        let result = match self.store.search_memory_candidates(
            &a.scope_type,
            a.scope_id.as_deref(),
            &a.query,
            &a.receiver_scope_type,
            a.receiver_scope_id.as_deref(),
            a.limit,
        ) {
            Ok(result) => result,
            Err(error) => return tool_error(error),
        };
        if let Some(delegation_id) = a.delegation_id {
            let references = result
                .hot
                .iter()
                .chain(result.warm.iter())
                .chain(result.already_known.iter())
                .map(|memory| (memory.memory_id, memory.content_hash.clone()))
                .collect::<Vec<_>>();
            if let Err(error) = self.store.record_delegation_memory_retrieval(
                delegation_id,
                &a.scope_type,
                a.scope_id.as_deref(),
                &a.receiver_scope_type,
                a.receiver_scope_id.as_deref().unwrap_or_default(),
                "search_memory_candidates",
                &references,
            ) {
                return tool_error(error);
            }
        }
        success(result)
    }

    #[tool(
        description = "Create a reviewable canonical memory candidate from accepted episodic/evidence memories, retaining their memory IDs, provenance, and artifact references."
    )]
    fn consolidate_memories(
        &self,
        Parameters(a): Parameters<ConsolidateMemoriesArgs>,
    ) -> CallToolResult {
        self.store
            .consolidate_memories(
                &a.scope_type,
                a.scope_id.as_deref(),
                &a.source_memory_ids,
                &a.body,
                &a.kind,
                a.importance,
                a.confidence,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Run deterministic local memory maintenance checks and return auditable proposals. This never deletes evidence."
    )]
    fn run_memory_gardener(
        &self,
        Parameters(a): Parameters<RunMemoryGardenerArgs>,
    ) -> CallToolResult {
        self.store
            .run_memory_gardener(a.minimum_promotion_support)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Review a Memory Gardener proposal. Accepted proposals can promote supported episodic memory or change temperature; evidence deletion is not available through MCP."
    )]
    fn review_memory_maintenance_proposal(
        &self,
        Parameters(a): Parameters<ReviewMaintenanceProposalArgs>,
    ) -> CallToolResult {
        self.store
            .review_memory_maintenance_proposal(a.proposal_id, &a.decision)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Create an immutable local memory-policy version for measured experiments. Does not change model or effort selectors."
    )]
    fn create_memory_policy(
        &self,
        Parameters(a): Parameters<CreateMemoryPolicyArgs>,
    ) -> CallToolResult {
        self.store
            .create_memory_policy(&MemoryPolicyInput {
                policy_key: a.policy_key,
                version: a.version,
                retrieval_method: a.retrieval_method,
                hot_budget_bytes: a.hot_budget_bytes,
                warm_candidate_limit: a.warm_candidate_limit,
                minimum_promotion_support: a.minimum_promotion_support,
                retention_minimum_age_seconds: a.retention_minimum_age_seconds,
            })
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Record memory-policy measurements against a completed, quality-classified Cathulu run."
    )]
    fn record_memory_policy_evaluation(
        &self,
        Parameters(a): Parameters<RecordMemoryPolicyEvaluationArgs>,
    ) -> CallToolResult {
        self.store
            .record_memory_policy_evaluation(&MemoryPolicyEvaluationInput {
                policy_id: a.policy_id,
                experiment_key: a.experiment_key,
                task_type: a.task_type,
                run_id: a.run_id,
                missing_context_failures: a.missing_context_failures,
                regressions: a.regressions,
                rework_count: a.rework_count,
                context_bytes: a.context_bytes,
                reintroduced_bytes: a.reintroduced_bytes,
                useful_retrievals: a.useful_retrievals,
                retrieval_count: a.retrieval_count,
                latency_ms: a.latency_ms,
                human_interventions: a.human_interventions,
            })
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Compare verified outcomes and resource measures for the policies recorded under one experiment key."
    )]
    fn compare_memory_policy_experiment(
        &self,
        Parameters(a): Parameters<CompareMemoryPolicyArgs>,
    ) -> CallToolResult {
        self.store
            .compare_memory_policy_experiment(&a.experiment_key)
            .map(success)
            .unwrap_or_else(tool_error)
    }
}

#[tool_handler]
impl ServerHandler for CathuluServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new(SERVER_NAME, SERVER_VERSION))
        .with_instructions(
            "Cathulu stores local project evidence. Register or reconcile repositories with tools; read current snapshots through resources. The dashboard is read-only.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let mut result = ListResourcesResult::default();
        result.resources = vec![
            Resource::new("cathulu://projects", "Cathulu projects")
                .with_description("Registered repositories and their latest Git snapshots.")
                .with_mime_type("application/json"),
            Resource::new("cathulu://roles", "Cathulu logical agent roles")
                .with_description(
                    "Persistent Main Overseer and per-project Project Manager identities, memory scopes, and versioned tool profiles. Task assignments and Codex chats are separate.",
                )
                .with_mime_type("application/json"),
            Resource::new("cathulu://dashboard", "Cathulu dashboard snapshot")
                .with_description(
                    "Read-only project and agent graph with recent self-reported progress checkpoints, unresolved owner actions, blockers, required quality failures, and memory-health summary.",
                )
                .with_mime_type("application/json"),
        ];
        Ok(result)
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let mut result = ListResourceTemplatesResult::default();
        result.resource_templates = vec![
            ResourceTemplate::new("cathulu://projects/{id}", "Cathulu project")
                .with_description("One registered repository and its latest Git snapshot.")
                .with_mime_type("application/json"),
            ResourceTemplate::new(
                "cathulu://projects/{id}/graphify-snapshot",
                "Cathulu Graphify snapshot",
            )
            .with_description("Current local Graphify snapshot identity and freshness state, if available.")
            .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://runs/{id}", "Cathulu run")
                .with_description("One run and its quality check results.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://delegations/{id}", "Cathulu delegation")
                .with_description("One bounded task packet, requested settings, append-only lifecycle history, and any worker report with review history.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://decisions/{id}", "Cathulu decision")
                .with_description("One immutable project decision and its supersession link.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://objectives/{id}", "Cathulu objective")
                .with_description("One current project objective.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://tasks/{id}", "Cathulu task")
                .with_description("One current objective task.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://dependencies/{id}", "Cathulu project dependency")
                .with_description("One recorded relationship between local projects.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://artifacts/{id}", "Cathulu evidence artifact")
                .with_description("One local artifact reference and its protection state.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://memories/{id}", "Cathulu memory")
                .with_description(
                    "One memory record with provenance, temporal validity, and freshness.",
                )
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://transfers/{id}", "Cathulu context transfer")
                .with_description("Auditable metadata for one Cathulu-controlled context transfer.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://events/run/{id}", "Cathulu run events")
                .with_description("Append-only event history for a run.")
                .with_mime_type("application/json"),
        ];
        Ok(result)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let data = if request.uri == "cathulu://projects" {
            serde_json::to_string(&self.store.list_projects().map_err(internal_error)?)
                .map_err(internal_error)?
        } else if request.uri == "cathulu://roles" {
            serde_json::to_string(&self.store.logical_agent_roles().map_err(internal_error)?)
                .map_err(internal_error)?
        } else if request.uri == "cathulu://dashboard" {
            serde_json::to_string(&self.store.dashboard_snapshot().map_err(internal_error)?)
                .map_err(internal_error)?
        } else if let Some(delegation_id) = request.uri.strip_prefix("cathulu://delegations/") {
            let delegation_id = parse_resource_id(delegation_id, &request.uri)?;
            let delegation = self
                .store
                .get_delegation(delegation_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "delegation_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&delegation).map_err(internal_error)?
        } else if let Some(run_id) = request.uri.strip_prefix("cathulu://runs/") {
            let run_id = parse_resource_id(run_id, &request.uri)?;
            let run = self
                .store
                .get_run(run_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "run_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&run).map_err(internal_error)?
        } else if let Some(decision_id) = request.uri.strip_prefix("cathulu://decisions/") {
            let decision_id = parse_resource_id(decision_id, &request.uri)?;
            let decision = self
                .store
                .get_decision(decision_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "decision_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&decision).map_err(internal_error)?
        } else if let Some(objective_id) = request.uri.strip_prefix("cathulu://objectives/") {
            let objective_id = parse_resource_id(objective_id, &request.uri)?;
            let objective = self
                .store
                .get_objective(objective_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "objective_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&objective).map_err(internal_error)?
        } else if let Some(task_id) = request.uri.strip_prefix("cathulu://tasks/") {
            let task_id = parse_resource_id(task_id, &request.uri)?;
            let task = self
                .store
                .get_task(task_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "task_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&task).map_err(internal_error)?
        } else if let Some(dependency_id) = request.uri.strip_prefix("cathulu://dependencies/") {
            let dependency_id = parse_resource_id(dependency_id, &request.uri)?;
            let dependency = self
                .store
                .get_dependency(dependency_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "dependency_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&dependency).map_err(internal_error)?
        } else if let Some(artifact_id) = request.uri.strip_prefix("cathulu://artifacts/") {
            let artifact_id = parse_resource_id(artifact_id, &request.uri)?;
            let artifact = self
                .store
                .get_artifact(artifact_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "artifact_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&artifact).map_err(internal_error)?
        } else if let Some(memory_id) = request.uri.strip_prefix("cathulu://memories/") {
            let memory_id = parse_resource_id(memory_id, &request.uri)?;
            let memory = self
                .store
                .get_memory(memory_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "memory_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&memory).map_err(internal_error)?
        } else if let Some(transfer_id) = request.uri.strip_prefix("cathulu://transfers/") {
            let transfer_id = parse_resource_id(transfer_id, &request.uri)?;
            let transfer = self
                .store
                .get_context_transfer(transfer_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "transfer_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&transfer).map_err(internal_error)?
        } else if let Some(run_id) = request.uri.strip_prefix("cathulu://events/run/") {
            let run_id = parse_resource_id(run_id, &request.uri)?;
            let events: Vec<EventRecord> = self
                .store
                .list_events("run", &run_id.to_string())
                .map_err(internal_error)?;
            serde_json::to_string(&events).map_err(internal_error)?
        } else if let Some(project_id) = request
            .uri
            .strip_prefix("cathulu://projects/")
            .and_then(|path| path.strip_suffix("/graphify-snapshot"))
        {
            let project_id = parse_resource_id(project_id, &request.uri)?;
            let snapshot = self
                .store
                .get_graphify_snapshot(project_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "graphify_snapshot_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&snapshot).map_err(internal_error)?
        } else if let Some(project_id) = request.uri.strip_prefix("cathulu://projects/") {
            let project_id = parse_resource_id(project_id, &request.uri)?;
            let project = self
                .store
                .get_project(project_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "project_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&project).map_err(internal_error)?
        } else {
            return Err(McpError::resource_not_found(
                "resource_not_found",
                Some(serde_json::json!({ "uri": request.uri })),
            ));
        };
        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(data, &request.uri).with_mime_type("application/json")
        ])
        .into())
    }
}

pub async fn run_stdio() -> Result<(), String> {
    let store = Store::open(default_data_dir()?)?;
    let service = CathuluServer { store }
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| format!("cannot start MCP stdio service: {error}"))?;
    service
        .waiting()
        .await
        .map_err(|error| format!("MCP stdio service stopped unexpectedly: {error}"))?;
    Ok(())
}

fn parse_resource_id(id: &str, uri: &str) -> Result<i64, McpError> {
    id.parse::<i64>().map_err(|_| {
        McpError::resource_not_found(
            "resource_not_found",
            Some(serde_json::json!({ "uri": uri })),
        )
    })
}

fn success<T: Serialize>(value: T) -> CallToolResult {
    match serde_json::to_string(&value) {
        Ok(json) => CallToolResult::success(vec![ContentBlock::text(json)]),
        Err(error) => tool_error(format!("cannot serialize tool result: {error}")),
    }
}

fn tool_error(error: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error)])
}

fn internal_error(error: impl std::fmt::Display) -> McpError {
    McpError::internal_error(error.to_string(), None)
}
