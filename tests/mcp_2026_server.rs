use adk_mcp_sdk::mcp_2026_server;
use rmcp::{
    ClientHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{
        CacheScope, CallToolResponse, ClientCapabilities, ClientInfo, GetTaskParams,
        Implementation, ProtocolVersion, TaskPayload,
    },
    schemars, tool, tool_router,
};

#[derive(Clone)]
struct LegacyClient;

impl ClientHandler for LegacyClient {
    fn get_info(&self) -> ClientInfo {
        let mut info = ClientInfo::new(
            ClientCapabilities::default(),
            Implementation::new("legacy-test", "1"),
        );
        info.protocol_version = ProtocolVersion::V_2025_11_25;
        info
    }
}

#[derive(Clone)]
struct CurrentClient;

impl ClientHandler for CurrentClient {
    fn get_info(&self) -> ClientInfo {
        let mut info = ClientInfo::new(
            ClientCapabilities::builder().enable_tasks().build(),
            Implementation::new("current-test", "1"),
        );
        info.protocol_version = ProtocolVersion::V_2026_07_28;
        info
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EchoInput {
    value: String,
}

#[derive(Clone)]
struct TestServer;

#[tool_router]
impl TestServer {
    #[tool(description = "Echo a value")]
    async fn echo(&self, Parameters(input): Parameters<EchoInput>) -> String {
        input.value
    }
}

mcp_2026_server! {
    server: TestServer,
    task_tools: ["echo"],
    approval_tools: [],
    cache_ttl_ms: 60_000,
}

#[tokio::test]
async fn legacy_tool_call_still_works() {
    let (server_transport, client_transport) = tokio::io::duplex(8_192);
    let server = tokio::spawn(async move {
        TestServer
            .serve(server_transport)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });

    let client = LegacyClient.serve(client_transport).await.unwrap();
    let result = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("echo").with_arguments(
                serde_json::json!({"value": "ok"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(result.content[0].as_text().unwrap().text, "ok");
    client.cancel().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn current_client_receives_cache_hints_and_task_lifecycle() {
    let (server_transport, client_transport) = tokio::io::duplex(8_192);
    let server = tokio::spawn(async move {
        TestServer
            .serve(server_transport)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });
    let client = CurrentClient.serve(client_transport).await.unwrap();

    let tools = client.list_tools(None).await.unwrap();
    assert_eq!(tools.ttl_ms, Some(60_000));
    assert_eq!(tools.cache_scope, Some(CacheScope::Public));

    let response = client
        .call_tool_once(
            rmcp::model::CallToolRequestParams::new("echo").with_arguments(
                serde_json::json!({"value": "task-ok"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let created = match response {
        CallToolResponse::Task(created) => created,
        other => panic!("expected task, got {other:?}"),
    };
    loop {
        let task = client
            .peer()
            .get_task(GetTaskParams::new(created.task.task_id.clone()))
            .await
            .unwrap()
            .task;
        if task.status().is_terminal() {
            match task.payload {
                TaskPayload::Completed { result } => {
                    assert_eq!(result["content"][0]["text"], "task-ok");
                }
                other => panic!("unexpected terminal payload: {other:?}"),
            }
            break;
        }
        tokio::task::yield_now().await;
    }

    client.cancel().await.unwrap();
    server.await.unwrap();
}
