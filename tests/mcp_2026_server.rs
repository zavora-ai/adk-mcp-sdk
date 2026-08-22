use adk_mcp_sdk::mcp_2026_server;
use rmcp::{
    ClientHandler, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{
        CacheScope, CallToolResponse, ClientCapabilities, ClientInfo, ClientJsonRpcMessage,
        ClientRequest, ElicitRequestParams, ElicitResult, ElicitationAction, GetTaskParams,
        Implementation, ListToolsRequest, ProtocolVersion, RequestId, RequestMetaObject,
        ServerJsonRpcMessage, TaskPayload,
    },
    schemars, tool, tool_router,
    transport::{IntoTransport, Transport},
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

    async fn create_elicitation(
        &self,
        _request: ElicitRequestParams,
        _context: rmcp::service::RequestContext<rmcp::service::RoleClient>,
    ) -> Result<ElicitResult, rmcp::ErrorData> {
        Ok(ElicitResult::new(ElicitationAction::Accept)
            .with_content(serde_json::json!({"action": "accept"})))
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

    #[tool(description = "Protected identity test")]
    async fn danger(&self) -> String {
        adk_mcp_sdk::current_caller_identity().unwrap_or_else(|| "missing".into())
    }

    #[tool(description = "Return a legacy JSON text result")]
    async fn json_status(&self) -> String {
        serde_json::json!({"ok": true, "status": "ready"}).to_string()
    }

    #[tool(description = "Return a legacy JSON text error")]
    async fn json_error(&self) -> String {
        serde_json::json!({"ok": false, "error": "expected failure"}).to_string()
    }
}

mcp_2026_server! {
    server: TestServer,
    task_tools: ["echo"],
    task_ttl_overrides: [("echo", 120_000)],
    approval_tools: ["danger"],
    mutating_tools: ["danger"],
    destructive_tools: ["danger"],
    idempotent_tools: [],
    cache_ttl_ms: 60_000,
    instructions: "Test server instructions",
}

#[derive(Clone)]
struct NoTaskServer;

#[tool_router]
impl NoTaskServer {
    #[tool(description = "Read a value")]
    async fn read(&self) -> String {
        "value".into()
    }
}

mcp_2026_server! {
    server: NoTaskServer,
    task_tools: [],
    approval_tools: [],
    cache_ttl_ms: 86_400_000,
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
    assert!(tools.tools.iter().all(|tool| tool.output_schema.is_some()));
    assert!(tools.tools.iter().all(|tool| tool.annotations.is_some()));
    let danger = tools
        .tools
        .iter()
        .find(|tool| tool.name == "danger")
        .unwrap();
    let annotations = danger.annotations.as_ref().unwrap();
    assert_eq!(annotations.read_only_hint, Some(false));
    assert_eq!(annotations.destructive_hint, Some(true));

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
    assert_eq!(created.task.ttl_ms, Some(120_000));
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

#[tokio::test]
async fn legacy_json_text_is_promoted_to_structured_content_and_errors() {
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

    let success = client
        .call_tool(rmcp::model::CallToolRequestParams::new("json_status"))
        .await
        .unwrap();
    assert_eq!(success.structured_content.unwrap()["status"], "ready");
    assert_eq!(success.is_error, Some(false));

    let error = client
        .call_tool(rmcp::model::CallToolRequestParams::new("json_error"))
        .await
        .unwrap();
    assert_eq!(
        error.structured_content.unwrap()["error"],
        "expected failure"
    );
    assert_eq!(error.is_error, Some(true));

    client.cancel().await.unwrap();
    server.await.unwrap();
}

#[test]
fn tasks_capability_is_only_advertised_when_used() {
    assert!(TestServer.get_info().capabilities.supports_tasks());
    assert!(!NoTaskServer.get_info().capabilities.supports_tasks());
}

#[tokio::test]
async fn protected_call_completes_sealed_mrtr_and_binds_identity() {
    // SAFETY: this integration-test process sets the key before making its only
    // protected call; the SDK snapshots it on first use.
    unsafe {
        std::env::set_var(
            "MCP_REQUEST_STATE_KEY",
            "integration-test-signing-key-at-least-32-bytes",
        );
    }
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
    let result = client
        .call_tool(rmcp::model::CallToolRequestParams::new("danger"))
        .await
        .unwrap();
    assert_eq!(result.content[0].as_text().unwrap().text, "current-test");
    client.cancel().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn stateless_request_lists_tools_without_initialize() {
    let (server_transport, client_transport) = tokio::io::duplex(8_192);
    let server = tokio::spawn(async move { TestServer.serve(server_transport).await.unwrap() });
    let mut client = IntoTransport::<rmcp::RoleClient, _, _>::into_transport(client_transport);
    let mut meta = RequestMetaObject::new();
    meta.set_protocol_version(ProtocolVersion::V_2026_07_28);
    meta.set_client_info(Implementation::new("handshakeless-test", "1"));
    meta.set_client_capabilities(ClientCapabilities::default());
    let mut request = ListToolsRequest {
        method: Default::default(),
        params: None,
        extensions: Default::default(),
    };
    request.extensions.insert(meta);
    client
        .send(ClientJsonRpcMessage::request(
            ClientRequest::ListToolsRequest(request),
            RequestId::Number(1),
        ))
        .await
        .unwrap();
    assert!(matches!(
        client.receive().await,
        Some(ServerJsonRpcMessage::Response(_))
    ));
    server.await.unwrap().cancel().await.unwrap();
}
