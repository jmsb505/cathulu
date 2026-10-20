use crate::memory::{FreshnessBasis, MemoryCreateInput, MemorySource};
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

fn empty_object() -> Value {
    serde_json::json!({})
}
fn default_retrieval_method() -> String {
    "explicit".to_string()
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
        self.store
            .start_run(
                a.project_id,
                a.objective_id,
                a.task_id,
                a.workflow_version_id,
            )
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
            Resource::new("cathulu://dashboard", "Cathulu dashboard snapshot")
                .with_description(
                    "Read-only project, workflow, quality, and memory-health summary.",
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
            ResourceTemplate::new("cathulu://runs/{id}", "Cathulu run")
                .with_description("One run and its quality check results.")
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
        } else if request.uri == "cathulu://dashboard" {
            serde_json::to_string(&self.store.dashboard_snapshot().map_err(internal_error)?)
                .map_err(internal_error)?
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
    report_startup_reconciliation(&store);
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

fn report_startup_reconciliation(store: &Store) {
    match store.reconcile_registered_projects() {
        Ok(report) => {
            for failure in report.failures {
                eprintln!(
                    "Cathulu startup Git reconciliation skipped project {}: {}",
                    failure.project_id, failure.message
                );
            }
        }
        Err(error) => eprintln!("Cathulu startup Git reconciliation failed: {error}"),
    }
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
