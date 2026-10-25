use crate::delegation::{ActivationMemoryReference, DelegationTaskPacket, WorkerReport};
use crate::dispatcher::{
    role_allowed_tools, RoleCapabilityScope, RoleDispatchAdmissionPolicy, RoleDispatcherService,
};
use crate::gardener::{MemoryPolicyEvaluationInput, MemoryPolicyInput};
use crate::memory::{FreshnessBasis, MemoryCreateInput, MemorySource};
use crate::optimization::{
    ContextModel, DecisionModel, DisabledContextModel, DisabledDecisionModel,
    OptimizationCapability, OptimizationConfig, ProviderLocality, RoutingDecisionSchema,
};
use crate::provider_adapters::{JevProvider, ScaleDownContextProvider};
use crate::telemetry::CodexHookEvent;
use crate::workflow::{EventRecord, TaskPlanInput};
use crate::worktree::WorkspaceAttemptResolution;
use crate::{default_data_dir, Store};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
    Implementation, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse, ReadResourceResult,
    Resource, ResourceContents, ResourceTemplate, ResultType, ServerCapabilities, ServerConfig,
    Tool,
};
use rmcp::service::RequestContext;
use rmcp::{
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
    ServiceExt,
};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

// Product dispatch stays off until the admission boundary is reviewed and the Owner explicitly
// approves enablement. Review is a prerequisite, not an activation decision. This gate also
// changes the default: an unset CATHULU_ROLE_DISPATCHER_ENABLED previously enabled dispatch.
const ROLE_DISPATCHER_RUNTIME_GATES_VALIDATED: bool = false;

const SERVER_NAME: &str = "cathulu";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct CathuluServer {
    store: Store,
    dispatcher_enabled: bool,
    role_capability: Option<RoleCapabilityScope>,
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
    #[serde(default)]
    parent_task_id: Option<i64>,
    #[serde(default)]
    priority: i64,
    #[serde(default)]
    due_date: Option<String>,
    #[serde(default)]
    effort_minutes: Option<i64>,
    #[serde(default)]
    next_action: Option<String>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UpdateProjectPlanningArgs {
    project_id: i64,
    expected_revision: i64,
    status: String,
    priority: i64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct UpdateTaskPlanArgs {
    task_id: i64,
    expected_revision: i64,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    parent_task_id: Option<i64>,
    priority: i64,
    #[serde(default)]
    due_date: Option<String>,
    #[serde(default)]
    effort_minutes: Option<i64>,
    #[serde(default)]
    next_action: Option<String>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TaskStatusRevisionArgs {
    task_id: i64,
    expected_revision: i64,
    status: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TaskRevisionReasonArgs {
    task_id: i64,
    expected_revision: i64,
    reason: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TaskDependencyArgs {
    task_id: i64,
    prerequisite_task_id: i64,
    expected_revision: i64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddTaskBlockerArgs {
    task_id: i64,
    expected_revision: i64,
    reason_code: String,
    reason: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ResolveTaskBlockerArgs {
    blocker_id: i64,
    expected_task_revision: i64,
    resolution: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CompleteTaskArgs {
    task_id: i64,
    expected_revision: i64,
    reason: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AcceptTaskRunArgs {
    task_id: i64,
    run_id: i64,
    expected_revision: i64,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct TaskQueryArgs {
    #[serde(default)]
    project_id: Option<i64>,
    #[serde(default)]
    eligible_only: bool,
    #[serde(default = "default_task_query_limit")]
    limit: usize,
}

fn default_task_query_limit() -> usize {
    100
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct CreateProjectCheckpointArgs {
    project_id: i64,
    expected_source_event_id: i64,
    summary_text: String,
    #[serde(default)]
    artifact_ids: Vec<i64>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ResumeProjectArgs {
    project_id: i64,
    #[serde(default = "default_resume_limit")]
    limit: usize,
}
fn default_resume_limit() -> usize {
    10
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
    #[schemars(schema_with = "json_object_input_schema")]
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
    #[serde(default)]
    plan_task_key: Option<String>,
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
struct DispatchPreparedDelegationsArgs {
    delegation_ids: Vec<i64>,
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
    reviewer_role: String,
    decision: String,
    #[serde(default)]
    note: String,
    idempotency_key: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RecordRoleWorkPlanArgs {
    project_id: i64,
    run_id: i64,
    plan_level: String,
    #[serde(default)]
    manager_delegation_id: Option<i64>,
    #[schemars(schema_with = "json_object_input_schema")]
    plan: Value,
    idempotency_key: String,
}

fn json_object_input_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "object",
        "properties": {},
        "additionalProperties": true
    })
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReviewRoleWorkPlanArgs {
    plan_id: i64,
    reviewer_role: String,
    decision: String,
    note: String,
    idempotency_key: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RequestOwnerApprovalArgs {
    manager_delegation_id: i64,
    action_key: String,
    request_summary: String,
    owner_action: String,
    reason: String,
    idempotency_key: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RelayOwnerApprovalArgs {
    request_id: i64,
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
struct ListWorkspaceRecoveryCasesArgs {
    /// Maximum quarantined leases to return (1–100; defaults to 50).
    #[serde(default)]
    limit: Option<usize>,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct WorkspaceAttemptResolutionArgs {
    attempt_key: String,
    terminal_state: String,
    reason: String,
}
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReconcileWorkspaceLeaseArgs {
    lease_id: i64,
    attempt_resolutions: Vec<WorkspaceAttemptResolutionArgs>,
    resolution: String,
    /// Must be true only after confirming the provider execution has stopped.
    provider_stopped_confirmed: bool,
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
#[serde(deny_unknown_fields)]
struct ProjectMemoryReviewArgs {
    run_id: i64,
    disposition: String,
    reason_code: String,
    #[serde(default)]
    memory_ids: Vec<i64>,
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

    #[tool(
        description = "Change an objective status to active, blocked, completed, cancelled, or archived."
    )]
    fn set_objective_status(&self, Parameters(a): Parameters<SetStatusArgs>) -> CallToolResult {
        self.store
            .set_objective_status(a.id, &a.status)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Set a project's planning status and priority using its expected revision."
    )]
    fn update_project_planning(
        &self,
        Parameters(a): Parameters<UpdateProjectPlanningArgs>,
    ) -> CallToolResult {
        self.store
            .update_project_planning(a.project_id, a.expected_revision, &a.status, a.priority)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Create a task inside an objective.")]
    fn create_task(&self, Parameters(a): Parameters<CreateTaskArgs>) -> CallToolResult {
        self.store
            .create_task_with_plan(
                a.objective_id,
                &TaskPlanInput {
                    title: a.title,
                    description: a.description,
                    parent_task_id: a.parent_task_id,
                    priority: a.priority,
                    due_date: a.due_date,
                    effort_minutes: a.effort_minutes,
                    next_action: a.next_action,
                },
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Change a task to queued, active, or failed with an expected revision. Use cancel_task for a reasoned cancellation, structured blockers for blocked, and completion commands for completed."
    )]
    fn set_task_status(&self, Parameters(a): Parameters<TaskStatusRevisionArgs>) -> CallToolResult {
        self.store
            .transition_task_status(a.task_id, a.expected_revision, &a.status)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Cancel a task with an explicit reason and expected revision; parents require every child completed or explicitly waived."
    )]
    fn cancel_task(&self, Parameters(a): Parameters<TaskRevisionReasonArgs>) -> CallToolResult {
        self.store
            .cancel_task(a.task_id, a.expected_revision, &a.reason)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Reopen a failed or cancelled task with an audit reason and expected revision."
    )]
    fn reopen_task(&self, Parameters(a): Parameters<TaskRevisionReasonArgs>) -> CallToolResult {
        self.store
            .reopen_task(a.task_id, a.expected_revision, &a.reason)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Replace a task's planning fields with an optimistic revision check.")]
    fn update_task_plan(&self, Parameters(a): Parameters<UpdateTaskPlanArgs>) -> CallToolResult {
        self.store
            .update_task_plan(
                a.task_id,
                a.expected_revision,
                &TaskPlanInput {
                    title: a.title,
                    description: a.description,
                    parent_task_id: a.parent_task_id,
                    priority: a.priority,
                    due_date: a.due_date,
                    effort_minutes: a.effort_minutes,
                    next_action: a.next_action,
                },
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Add a same-project prerequisite edge. Cycles and stale task revisions are rejected."
    )]
    fn add_task_dependency(&self, Parameters(a): Parameters<TaskDependencyArgs>) -> CallToolResult {
        self.store
            .add_task_dependency(a.task_id, a.prerequisite_task_id, a.expected_revision)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Remove a same-project prerequisite edge with a task revision check.")]
    fn remove_task_dependency(
        &self,
        Parameters(a): Parameters<TaskDependencyArgs>,
    ) -> CallToolResult {
        self.store
            .remove_task_dependency(a.task_id, a.prerequisite_task_id, a.expected_revision)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Record a structured open blocker and move the task into blocked status.")]
    fn add_task_blocker(&self, Parameters(a): Parameters<AddTaskBlockerArgs>) -> CallToolResult {
        self.store
            .add_task_blocker(a.task_id, a.expected_revision, &a.reason_code, &a.reason)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Resolve one blocker with an audit reason; the task returns to queued after its final manual blocker is resolved."
    )]
    fn resolve_task_blocker(
        &self,
        Parameters(a): Parameters<ResolveTaskBlockerArgs>,
    ) -> CallToolResult {
        self.store
            .resolve_task_blocker(a.blocker_id, a.expected_task_revision, &a.resolution)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Complete an ordinary task administratively with a reason, only when dependency, blocker, run, and child constraints pass."
    )]
    fn complete_task_administratively(
        &self,
        Parameters(a): Parameters<CompleteTaskArgs>,
    ) -> CallToolResult {
        self.store
            .complete_task_administratively(a.task_id, a.expected_revision, &a.reason)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Accept a linked run as task completion only after finish_run recorded verified_success and task constraints still pass."
    )]
    fn accept_task_run(&self, Parameters(a): Parameters<AcceptTaskRunArgs>) -> CallToolResult {
        self.store
            .accept_task_run(a.task_id, a.run_id, a.expected_revision)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "List bounded project tasks with deterministic eligibility reasons and stable priority/due-date ordering. Unknown effort and due dates remain null."
    )]
    fn list_tasks(&self, Parameters(a): Parameters<TaskQueryArgs>) -> CallToolResult {
        self.store
            .list_tasks(a.project_id, a.eligible_only, a.limit)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Create an immutable project checkpoint from the current task/objective state. Supply the source event revision from resume_project; artifact IDs must refer to runs in this project."
    )]
    fn create_project_checkpoint(
        &self,
        Parameters(a): Parameters<CreateProjectCheckpointArgs>,
    ) -> CallToolResult {
        self.store
            .create_project_checkpoint(
                a.project_id,
                a.expected_source_event_id,
                &a.summary_text,
                &a.artifact_ids,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Read a bounded, source-backed project resume packet without refreshing Git or invoking a provider."
    )]
    fn resume_project(&self, Parameters(a): Parameters<ResumeProjectArgs>) -> CallToolResult {
        self.store
            .resume_project(a.project_id, a.limit)
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(description = "Read the latest immutable checkpoint revision for one project.")]
    fn latest_project_checkpoint(
        &self,
        Parameters(a): Parameters<ReconcileProjectArgs>,
    ) -> CallToolResult {
        self.store
            .latest_project_checkpoint(a.project_id)
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
        description = "Prepare a bounded native-Codex assignment. A Project Manager assignment requires a recorded Main workflow brief for that project/run. A Worker under a Project Manager requires an accepted PM plan, its exact plan_task_key, and a task packet matching that plan. Preparation records state only; it does not spawn a chat."
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
                a.plan_task_key.as_deref(),
                &a.idempotency_key,
                a.requested_model.as_deref(),
                a.requested_effort.as_deref(),
                a.external_correlation.as_deref(),
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Record an immutable, versioned Main workflow brief or Project Manager task plan. Main briefs are required before preparing a PM assignment. PM plans require Main review before any Worker assignment can be prepared. Keep each project brief independent unless the owner declared a dependency."
    )]
    fn record_role_work_plan(
        &self,
        Parameters(a): Parameters<RecordRoleWorkPlanArgs>,
    ) -> CallToolResult {
        self.store
            .record_role_work_plan(
                a.project_id,
                a.run_id,
                &a.plan_level,
                a.manager_delegation_id,
                &a.plan,
                &a.idempotency_key,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Record Main Overseer's review of a Project Manager work plan. Worker assignments stay blocked until the latest plan review is accepted. This records review evidence; it does not dispatch work."
    )]
    fn review_role_work_plan(
        &self,
        Parameters(a): Parameters<ReviewRoleWorkPlanArgs>,
    ) -> CallToolResult {
        self.store
            .review_role_work_plan(
                a.plan_id,
                &a.reviewer_role,
                &a.decision,
                &a.note,
                &a.idempotency_key,
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
        description = "Queue up to 32 prepared PM or Worker assignments for Cathulu's local asynchronous Codex dispatcher. Returns after durable queue acceptance; it never waits for a model turn. Currently unavailable: M52 workspace-lease, exact-path, permission, and credential-source admission is not integrated, so environment flags cannot activate dispatch. Planning, memory, scope, and QA gates still apply. A queued item is not success; inspect cathulu://dashboard for queued, running, blocked, or reconciliation-required state."
    )]
    fn dispatch_prepared_delegations(
        &self,
        Parameters(a): Parameters<DispatchPreparedDelegationsArgs>,
    ) -> CallToolResult {
        if !self.dispatcher_enabled {
            return tool_error(
                "local role dispatch is unavailable: M52 workspace-lease, exact-path, permission, and credential-source admission is incomplete; no delegation was queued or started"
                    .to_string(),
            );
        }
        self.store
            .enqueue_role_dispatches(&a.delegation_ids)
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
        let record = if self
            .role_capability
            .as_ref()
            .is_some_and(|capability| capability.role_kind == "main_overseer")
        {
            self.store.record_main_delegation_update(
                a.delegation_id,
                &a.update_type,
                a.phase.as_deref(),
                &a.summary,
                a.action_key.as_deref(),
                a.owner_action.as_deref(),
                &a.idempotency_key,
            )
        } else {
            self.store.record_delegation_update(
                a.delegation_id,
                &a.update_type,
                a.phase.as_deref(),
                &a.summary,
                a.action_key.as_deref(),
                a.owner_action.as_deref(),
                &a.idempotency_key,
            )
        };
        record.map(success).unwrap_or_else(tool_error)
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
        description = "Submit one bounded, idempotent report for a dispatched Worker or Project Manager assignment. A PM may report its project result only after its Worker reports have been accepted. Reports remain self-reported until the parent role reviews them; deterministic QA remains authoritative."
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
        description = "Append the parent role's review of a Worker or Project Manager report. reviewer_role must be project_manager for a Worker and main_overseer for a PM. PM completion is blocked until Main accepts its report; this never marks code or a run verified."
    )]
    fn review_delegation_report(
        &self,
        Parameters(a): Parameters<ReviewDelegationReportArgs>,
    ) -> CallToolResult {
        self.store
            .review_delegation_report(
                a.delegation_id,
                &a.reviewer_role,
                &a.decision,
                &a.note,
                &a.idempotency_key,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Create an explicit request for owner input only after an active Project Manager has escalated an unresolved action and Main determines neither PM nor Main can safely resolve it. The request appears in Cathulu's local dashboard for an explicit Owner decision. Model tools cannot record that decision, and this request does not execute the action or pause any other project."
    )]
    fn request_owner_approval(
        &self,
        Parameters(a): Parameters<RequestOwnerApprovalArgs>,
    ) -> CallToolResult {
        self.store
            .request_owner_approval(
                a.manager_delegation_id,
                &a.action_key,
                &a.request_summary,
                &a.owner_action,
                &a.reason,
                &a.idempotency_key,
            )
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Relay a completed local Owner decision through Main to the affected active Project Manager. This records a durable event for that project only; it does not execute the requested action or mark it complete."
    )]
    fn relay_owner_decision_to_project_manager(
        &self,
        Parameters(a): Parameters<RelayOwnerApprovalArgs>,
    ) -> CallToolResult {
        self.store
            .relay_owner_approval_decision(a.request_id)
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
        description = "List quarantined workspace leases and their unresolved provider attempts for local operator review. Returns path hashes, never canonical workspace paths or prompt text. This unscoped local MCP does not authenticate an Owner and must not be remotely exposed; scoped role agents cannot call this tool."
    )]
    fn list_workspace_recovery_cases(
        &self,
        Parameters(a): Parameters<ListWorkspaceRecoveryCasesArgs>,
    ) -> CallToolResult {
        if self.role_capability.is_some() {
            return tool_error(
                "workspace recovery is available only through the unscoped local MCP".into(),
            );
        }
        self.store
            .list_workspace_recovery_cases(a.limit.unwrap_or(50))
            .map(success)
            .unwrap_or_else(tool_error)
    }

    #[tool(
        description = "Manually reconcile one quarantined workspace lease through the unscoped local MCP. Before calling, independently confirm the provider execution has stopped and review its result. Supply every unresolved attempt for the exact lease, a terminal state (completed, failed, or blocked) and a reason for each, plus an overall reason. Cathulu revalidates the linked-worktree identity and atomically records the attempt dispositions and lease resolution. It never deletes, moves, or cleans a worktree and never starts or retries a provider. This endpoint does not authenticate an Owner, must not be remotely exposed, and is denied to scoped role agents."
    )]
    fn reconcile_workspace_lease(
        &self,
        Parameters(a): Parameters<ReconcileWorkspaceLeaseArgs>,
    ) -> CallToolResult {
        if self.role_capability.is_some() {
            return tool_error(
                "workspace recovery is available only through the unscoped local MCP".into(),
            );
        }
        let attempt_resolutions = a
            .attempt_resolutions
            .into_iter()
            .map(|item| WorkspaceAttemptResolution {
                attempt_key: item.attempt_key,
                terminal_state: item.terminal_state,
                reason: item.reason,
            })
            .collect::<Vec<_>>();
        self.store
            .reconcile_workspace_lease_with_attempts(
                a.lease_id,
                &attempt_resolutions,
                crate::worktree::unix_time(),
                &a.resolution,
                a.provider_stopped_confirmed,
            )
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
        description = "Finish a run after recording its project memory review with record_project_memory_review. Verified success also requires all mandatory workflow checks to pass."
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
        description = "Create a concise, distilled memory candidate with provenance, freshness, temporal validity, and stable identity. Use this to establish a source-backed canonical project summary during first project analysis and to preserve later verified reusable knowledge. Memory is project knowledge, never a chat transcript, private reasoning, prompt, or raw tool history. Review canonical claims against their cited sources before accepting them."
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

    #[tool(
        description = "Accept or reject a memory candidate after checking its distilled claims against the cited project sources. Never accept transcripts, private reasoning, or raw tool histories as memory."
    )]
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
        description = "Record the required compact memory review before finishing a project run. Use knowledge_saved with accepted, current project memory IDs, or no_durable_change only if a current canonical project summary already existed before this run. The reason_code is a fixed enum; this receipt stores no free text, memory body, transcript, prompt, or tool output."
    )]
    fn record_project_memory_review(
        &self,
        Parameters(a): Parameters<ProjectMemoryReviewArgs>,
    ) -> CallToolResult {
        self.store
            .record_project_memory_review(a.run_id, &a.disposition, &a.reason_code, &a.memory_ids)
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
        match self.store.list_memories(
            &a.scope_type,
            a.scope_id.as_deref(),
            a.include_history,
            a.include_cold,
        ) {
            Ok(memories) => {
                if let Some(capability) = &self.role_capability {
                    if let Err(error) = self
                        .store
                        .record_session_context_read(&capability.capability_hash, &memories)
                    {
                        return tool_error(error);
                    }
                }
                success(memories)
            }
            Err(error) => tool_error(error),
        }
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
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if let Some(capability) = &self.role_capability {
            let arguments = Value::Object(request.arguments.clone().unwrap_or_default());
            self.store
                .authorize_role_tool_call(capability, request.name.as_ref(), &arguments)
                .map_err(|message| McpError::invalid_params(message, None))?;
        }
        let tool_context =
            rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        CathuluServer::tool_router().call(tool_context).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut tools = CathuluServer::tool_router().list_all();
        if let Some(capability) = &self.role_capability {
            tools.retain(|tool| {
                role_allowed_tools(&capability.role_kind).contains(&tool.name.as_ref())
            });
        }
        let supports_cache_hints = context
            .protocol_version()
            .is_some_and(|version| version >= rmcp::model::ProtocolVersion::V_2026_07_28);
        Ok(ListToolsResult {
            result_type: Some(ResultType::COMPLETE),
            tools,
            meta: None,
            next_cursor: None,
            ttl_ms: supports_cache_hints.then_some(0),
            cache_scope: supports_cache_hints.then_some(CacheScope::Private),
        })
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        if self
            .role_capability
            .as_ref()
            .is_some_and(|capability| !role_allowed_tools(&capability.role_kind).contains(&name))
        {
            return None;
        }
        CathuluServer::tool_router().get(name).cloned()
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new(SERVER_NAME, SERVER_VERSION))
        .with_instructions(
            "Cathulu stores local project evidence. Register or reconcile repositories with tools; read current snapshots through resources. The desktop dashboard is observational except for its explicit local Owner-action decision control; it never executes the requested action.",
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
            Resource::new("cathulu://portfolio/tasks", "Cathulu task portfolio")
                .with_description("The first 100 current tasks with deterministic eligibility and blocker reasons.")
                .with_mime_type("application/json"),
            Resource::new("cathulu://roles", "Cathulu logical agent roles")
                .with_description(
                    "Persistent Main Overseer and per-project Project Manager identities, memory scopes, and versioned tool profiles. Task assignments and Codex chats are separate.",
                )
                .with_mime_type("application/json"),
            Resource::new("cathulu://dashboard", "Cathulu dashboard snapshot")
                .with_description(
                    "Read-only project and agent graph with recent progress, role dispatch queue state, unresolved owner actions, blockers, quality failures, and memory health.",
                )
                .with_mime_type("application/json"),
            Resource::new("cathulu://dispatches", "Cathulu role dispatch queue")
                .with_description("Recent asynchronous role dispatches. Queue state is execution metadata, not proof of task correctness.")
                .with_mime_type("application/json"),
        ];
        if self.role_capability.is_some() {
            result.resources.retain(|resource| {
                resource.uri == "cathulu://projects"
                    || (self
                        .role_capability
                        .as_ref()
                        .is_some_and(|capability| capability.role_kind == "main_overseer")
                        && resource.uri == "cathulu://portfolio/tasks")
            });
        }
        Ok(result)
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let mut result = ListResourceTemplatesResult::default();
        result.resource_templates = vec![
            ResourceTemplate::new("cathulu://dispatches", "Cathulu role dispatch queue")
                .with_description("Recent asynchronous role dispatches. Queue state is execution metadata, not proof of task correctness.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://projects/{id}", "Cathulu project")
                .with_description("One registered repository and its latest Git snapshot.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://projects/{id}/planning", "Cathulu project planning")
                .with_description("One project's planning status, priority, and optimistic revision.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://projects/{id}/tasks", "Cathulu project tasks")
                .with_description("The first 100 tasks for one project with eligibility and blocker reasons.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://projects/{id}/checkpoints/latest", "Cathulu latest project checkpoint")
                .with_description("The latest immutable checkpoint revision for one project, or null when none exists.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://projects/{id}/resume", "Cathulu project resume")
                .with_description("A bounded, source-backed current-state packet with checkpoint deltas and stored repository freshness.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://portfolio/tasks", "Cathulu task portfolio")
                .with_description("The first 100 tasks across projects with eligibility and blocker reasons.")
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
            ResourceTemplate::new("cathulu://work-plans/{id}", "Cathulu role work plan")
                .with_description("One immutable Main workflow brief or Project Manager task plan, including the latest parent review when present.")
                .with_mime_type("application/json"),
            ResourceTemplate::new("cathulu://owner-approvals/{id}", "Cathulu owner approval request")
                .with_description("One explicit owner request and its recorded response, if any.")
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
        if self.role_capability.is_some() {
            result.resource_templates.retain(|template| {
                if self
                    .role_capability
                    .as_ref()
                    .is_some_and(|capability| capability.role_kind == "main_overseer")
                {
                    matches!(
                        template.uri_template.as_str(),
                        "cathulu://projects/{id}"
                            | "cathulu://projects/{id}/planning"
                            | "cathulu://projects/{id}/tasks"
                            | "cathulu://projects/{id}/checkpoints/latest"
                            | "cathulu://projects/{id}/resume"
                            | "cathulu://portfolio/tasks"
                            | "cathulu://delegations/{id}"
                            | "cathulu://work-plans/{id}"
                            | "cathulu://owner-approvals/{id}"
                            | "cathulu://runs/{id}"
                            | "cathulu://events/run/{id}"
                    )
                } else {
                    matches!(
                        template.uri_template.as_str(),
                        "cathulu://projects/{id}"
                            | "cathulu://projects/{id}/planning"
                            | "cathulu://projects/{id}/tasks"
                            | "cathulu://projects/{id}/checkpoints/latest"
                            | "cathulu://projects/{id}/resume"
                            | "cathulu://delegations/{id}"
                    )
                }
            });
        }
        Ok(result)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if let Some(capability) = &self.role_capability {
            let allowed = if capability.role_kind == "main_overseer" {
                matches!(
                    request.uri.as_str(),
                    "cathulu://projects"
                        | "cathulu://roles"
                        | "cathulu://dashboard"
                        | "cathulu://dispatches"
                ) || request.uri == "cathulu://portfolio/tasks"
                    || request.uri.starts_with("cathulu://projects/")
                    || request.uri.starts_with("cathulu://delegations/")
                    || request.uri.starts_with("cathulu://work-plans/")
                    || request.uri.starts_with("cathulu://owner-approvals/")
                    || request.uri.starts_with("cathulu://runs/")
                    || request.uri.starts_with("cathulu://events/run/")
            } else if request.uri == "cathulu://projects" {
                true
            } else if role_can_read_task_resource(
                &capability.role_kind,
                capability.project_id,
                &request.uri,
            ) {
                true
            } else if let Some(project_id) = request.uri.strip_prefix("cathulu://projects/") {
                project_id.parse::<i64>().ok() == capability.project_id
            } else if let Some(delegation_id) = request.uri.strip_prefix("cathulu://delegations/") {
                delegation_id.parse::<i64>().ok().is_some_and(|id| {
                    self.store
                        .role_capability_can_read_delegation(capability, id)
                        .unwrap_or(false)
                })
            } else {
                false
            };
            if !allowed {
                return Err(McpError::invalid_params(
                    "role capability cannot read this resource",
                    None,
                ));
            }
        }
        let data = if request.uri == "cathulu://projects" {
            let mut projects = self.store.list_projects().map_err(internal_error)?;
            if let Some(capability) = &self.role_capability {
                if let Some(project_id) = capability.project_id {
                    projects.retain(|project| project.id == project_id);
                }
            }
            serde_json::to_string(&projects).map_err(internal_error)?
        } else if request.uri == "cathulu://portfolio/tasks" {
            serde_json::to_string(
                &self
                    .store
                    .list_tasks(None, false, 100)
                    .map_err(internal_error)?,
            )
            .map_err(internal_error)?
        } else if request.uri == "cathulu://roles" {
            serde_json::to_string(&self.store.logical_agent_roles().map_err(internal_error)?)
                .map_err(internal_error)?
        } else if request.uri == "cathulu://dashboard" {
            serde_json::to_string(&self.store.dashboard_snapshot().map_err(internal_error)?)
                .map_err(internal_error)?
        } else if request.uri == "cathulu://dispatches" {
            serde_json::to_string(&self.store.role_dispatches(100).map_err(internal_error)?)
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
        } else if let Some(plan_id) = request.uri.strip_prefix("cathulu://work-plans/") {
            let plan_id = parse_resource_id(plan_id, &request.uri)?;
            let plan = self
                .store
                .get_role_work_plan(plan_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "work_plan_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&plan).map_err(internal_error)?
        } else if let Some(request_id) = request.uri.strip_prefix("cathulu://owner-approvals/") {
            let request_id = parse_resource_id(request_id, &request.uri)?;
            let approval = self
                .store
                .get_owner_approval_request(request_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "owner_approval_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&approval).map_err(internal_error)?
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
            .and_then(|path| path.strip_suffix("/planning"))
        {
            let project_id = parse_resource_id(project_id, &request.uri)?;
            let project = self
                .store
                .get_project_planning(project_id)
                .map_err(internal_error)?
                .ok_or_else(|| {
                    McpError::resource_not_found(
                        "project_not_found",
                        Some(serde_json::json!({ "uri": request.uri })),
                    )
                })?;
            serde_json::to_string(&project).map_err(internal_error)?
        } else if let Some(project_id) = request
            .uri
            .strip_prefix("cathulu://projects/")
            .and_then(|path| path.strip_suffix("/tasks"))
        {
            let project_id = parse_resource_id(project_id, &request.uri)?;
            serde_json::to_string(
                &self
                    .store
                    .list_tasks(Some(project_id), false, 100)
                    .map_err(internal_error)?,
            )
            .map_err(internal_error)?
        } else if let Some(project_id) = request
            .uri
            .strip_prefix("cathulu://projects/")
            .and_then(|path| path.strip_suffix("/checkpoints/latest"))
        {
            let project_id = parse_resource_id(project_id, &request.uri)?;
            serde_json::to_string(
                &self
                    .store
                    .latest_project_checkpoint(project_id)
                    .map_err(internal_error)?,
            )
            .map_err(internal_error)?
        } else if let Some(project_id) = request
            .uri
            .strip_prefix("cathulu://projects/")
            .and_then(|path| path.strip_suffix("/resume"))
        {
            let project_id = parse_resource_id(project_id, &request.uri)?;
            serde_json::to_string(
                &self
                    .store
                    .resume_project(project_id, 10)
                    .map_err(internal_error)?,
            )
            .map_err(internal_error)?
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
    let role_capability = match std::env::var("CATHULU_ROLE_CAPABILITY") {
        Ok(token) => Some(store.resolve_role_thread_capability(&token)?),
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => {
            return Err("Cathulu role capability environment is not valid Unicode".to_string())
        }
    };
    let dispatcher_requested = role_dispatcher_requested(
        std::env::var("CATHULU_ROLE_DISPATCHER_ENABLED")
            .ok()
            .as_deref(),
    );
    let dispatcher_client_requested = std::env::var("CATHULU_ROLE_DISPATCHER_CLIENT_ENABLED")
        .is_ok_and(|value| matches!(value.trim(), "1" | "true" | "yes"));
    let dispatcher_enabled =
        role_dispatcher_runtime_enabled(dispatcher_requested, dispatcher_client_requested);
    let dispatcher =
        if role_dispatcher_service_enabled(dispatcher_requested, role_capability.is_some()) {
            Some(RoleDispatcherService::start(
                store.clone(),
                RoleDispatchAdmissionPolicy::from_environment()?,
            )?)
        } else {
            None
        };
    let service = CathuluServer {
        store,
        dispatcher_enabled,
        role_capability,
    }
    .serve(rmcp::transport::stdio())
    .await
    .map_err(|error| format!("cannot start MCP stdio service: {error}"))?;
    service
        .waiting()
        .await
        .map_err(|error| format!("MCP stdio service stopped unexpectedly: {error}"))?;
    drop(dispatcher);
    Ok(())
}

fn role_dispatcher_runtime_enabled(dispatcher_requested: bool, client_requested: bool) -> bool {
    (dispatcher_requested || client_requested) && ROLE_DISPATCHER_RUNTIME_GATES_VALIDATED
}

fn role_dispatcher_requested(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
}

fn role_dispatcher_service_enabled(dispatcher_requested: bool, has_role_capability: bool) -> bool {
    dispatcher_requested && !has_role_capability && ROLE_DISPATCHER_RUNTIME_GATES_VALIDATED
}

fn role_can_read_task_resource(role_kind: &str, project_id: Option<i64>, uri: &str) -> bool {
    if role_kind != "sub_overseer" {
        return false;
    }
    uri.strip_prefix("cathulu://projects/")
        .and_then(|path| path.split_once('/'))
        .is_some_and(|(resource_project_id, resource)| {
            resource_project_id.parse::<i64>().ok() == project_id
                && matches!(
                    resource,
                    "planning" | "tasks" | "checkpoints/latest" | "resume"
                )
        })
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

#[cfg(test)]
mod schema_tests {
    use super::{
        role_can_read_task_resource, role_dispatcher_requested, role_dispatcher_runtime_enabled,
        role_dispatcher_service_enabled, CathuluServer,
    };

    #[test]
    fn m52_admission_gate_keeps_product_dispatch_off_even_when_requested() {
        assert!(!role_dispatcher_requested(None));
        assert!(!role_dispatcher_requested(Some("")));
        assert!(!role_dispatcher_requested(Some("invalid")));
        assert!(role_dispatcher_requested(Some("true")));
        assert!(role_dispatcher_requested(Some("1")));
        assert!(!role_dispatcher_runtime_enabled(true, false));
        assert!(!role_dispatcher_runtime_enabled(false, true));
        assert!(!role_dispatcher_runtime_enabled(true, true));
        assert!(!role_dispatcher_runtime_enabled(false, false));
        assert!(!role_dispatcher_service_enabled(true, false));
        assert!(!role_dispatcher_service_enabled(true, true));
        assert!(!role_dispatcher_service_enabled(false, false));
    }

    #[test]
    fn structured_json_tool_inputs_have_explicit_object_schemas() {
        let tools = CathuluServer::tool_router().list_all();
        for (tool_name, field_name) in [
            ("create_workflow_version", "definition"),
            ("record_role_work_plan", "plan"),
        ] {
            let tool = tools
                .iter()
                .find(|tool| tool.name.as_ref() == tool_name)
                .expect("tool is registered");
            let input = serde_json::to_value(&tool.input_schema).unwrap();
            let field = input
                .pointer(&format!("/properties/{field_name}"))
                .expect("field schema exists");
            assert_eq!(
                field.get("type").and_then(serde_json::Value::as_str),
                Some("object")
            );
            assert!(field
                .get("properties")
                .is_some_and(serde_json::Value::is_object));
        }
    }

    #[test]
    fn workspace_recovery_tools_are_registered_with_explicit_operator_confirmation() {
        let tools = CathuluServer::tool_router().list_all();
        let list_tool = tools
            .iter()
            .find(|tool| tool.name.as_ref() == "list_workspace_recovery_cases")
            .expect("quarantined lease inspection tool is registered");
        let reconcile_tool = tools
            .iter()
            .find(|tool| tool.name.as_ref() == "reconcile_workspace_lease")
            .expect("manual reconciliation tool is registered");
        let schema = serde_json::to_value(&reconcile_tool.input_schema).unwrap();
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("reconciliation tool has an object input schema");
        for field in [
            "lease_id",
            "attempt_resolutions",
            "resolution",
            "provider_stopped_confirmed",
        ] {
            assert!(
                properties.contains_key(field),
                "missing required field {field}"
            );
        }
        assert!(list_tool
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("must not be remotely exposed"));
        assert!(reconcile_tool
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("independently confirm the provider execution has stopped"));
    }

    #[test]
    fn task_resources_are_visible_only_to_main_or_the_matching_project_manager() {
        assert!(role_can_read_task_resource(
            "sub_overseer",
            Some(7),
            "cathulu://projects/7/tasks"
        ));
        assert!(role_can_read_task_resource(
            "sub_overseer",
            Some(7),
            "cathulu://projects/7/planning"
        ));
        assert!(!role_can_read_task_resource(
            "sub_overseer",
            Some(7),
            "cathulu://projects/8/tasks"
        ));
        assert!(!role_can_read_task_resource(
            "worker",
            Some(7),
            "cathulu://projects/7/tasks"
        ));
        assert!(!role_can_read_task_resource(
            "sub_overseer",
            Some(7),
            "cathulu://projects/7/tasks/other"
        ));
    }
}
