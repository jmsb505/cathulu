#[tokio::main]
async fn main() {
    if let Err(error) = cathulu_core::mcp::run_stdio().await {
        eprintln!("Cathulu MCP server stopped: {error}");
        std::process::exit(1);
    }
}
