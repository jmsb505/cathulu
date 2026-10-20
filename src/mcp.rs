use crate::{default_data_dir, ProjectSnapshot, Store};
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
        result.resources = vec![Resource::new("cathulu://projects", "Cathulu projects")
            .with_description("Registered repositories and their latest Git snapshots.")
            .with_mime_type("application/json")];
        Ok(result)
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let mut result = ListResourceTemplatesResult::default();
        result.resource_templates =
            vec![
                ResourceTemplate::new("cathulu://projects/{id}", "Cathulu project")
                    .with_description("One registered repository and its latest Git snapshot.")
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
        } else if let Some(project_id) = request.uri.strip_prefix("cathulu://projects/") {
            let project_id = project_id.parse::<i64>().map_err(|_| {
                McpError::resource_not_found(
                    "project_not_found",
                    Some(serde_json::json!({ "uri": request.uri })),
                )
            })?;
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

fn success(project: ProjectSnapshot) -> CallToolResult {
    match serde_json::to_string(&project) {
        Ok(json) => CallToolResult::success(vec![ContentBlock::text(json)]),
        Err(error) => tool_error(format!("cannot serialize project snapshot: {error}")),
    }
}

fn tool_error(error: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error)])
}

fn internal_error(error: impl std::fmt::Display) -> McpError {
    McpError::internal_error(error.to_string(), None)
}
