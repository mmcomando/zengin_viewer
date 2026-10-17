use rmcp::{
    ServiceExt, handler::server::wrapper::Parameters, schemars, tool, tool_router, transport::stdio,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddParams {
    a: i32,
    b: i32,
}

#[derive(Clone)]
struct MyServer;

#[tool_router]
impl MyServer {
    #[tool(description = "Add two numbers")]
    fn add(&self, Parameters(params): Parameters<AddParams>) -> String {
        (params.a + params.b).to_string()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = MyServer;

    let service = server.serve(stdio()).await?;

    service.waiting().await?;

    Ok(())
}
