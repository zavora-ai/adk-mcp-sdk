//! # ADK MCP SDK
//!
//! Enterprise traits for MCP servers targeting the ADK-Rust Enterprise registry.
//!
//! Servers implement these traits to declare their capabilities, tools, risk
//! levels, and health checks — enabling automatic onboarding into the registry.

pub mod health;
pub mod manifest;
pub mod protocol;
pub mod risk;
pub mod tools;

pub use health::{HealthCheck, HealthStatus};
pub use manifest::ServerManifest;
pub use protocol::{
    Mcp2026Policy, approval_gate, caller_identity, current_caller_identity, current_task_context,
    enrich_tools, normalize_call_response, scope_caller_identity, scope_task_context,
    set_current_task_status,
};
pub use risk::RiskClass;
pub use tools::ToolMeta;

/// Generate a tools-only `ServerHandler` with the MCP 2026 stateless features
/// used across Zavora servers.
///
/// The server's tool impl must use `#[rmcp::tool_router]` (without
/// `server_handler`). Protected tools are held behind an integrity-protected
/// MRTR approval, and selected tools become protocol-native MCP tasks when the client
/// advertises task support. Legacy clients continue to execute ordinary tools,
/// but protected calls fail closed because they cannot complete MRTR approval.
#[macro_export]
macro_rules! mcp_2026_server {
    (
        server: $server:ty,
        task_tools: [$($task_tool:literal),* $(,)?],
        task_ttl_overrides: [$(($ttl_tool:literal, $ttl_ms:expr)),* $(,)?],
        approval_tools: [$($approval_tool:literal),* $(,)?],
        mutating_tools: [$($mutating_tool:literal),* $(,)?],
        destructive_tools: [$($destructive_tool:literal),* $(,)?],
        idempotent_tools: [$($idempotent_tool:literal),* $(,)?],
        cache_ttl_ms: $cache_ttl_ms:expr,
        instructions: $instructions:expr $(,)?
    ) => {
        $crate::mcp_2026_server! {
            @impl
            server: $server,
            task_tools: [$($task_tool),*],
            task_ttl_overrides: [$(($ttl_tool, $ttl_ms)),*],
            approval_tools: [$($approval_tool),*],
            mutating_tools: [$($mutating_tool),*],
            destructive_tools: [$($destructive_tool),*],
            idempotent_tools: [$($idempotent_tool),*],
            cache_ttl_ms: $cache_ttl_ms,
            instructions: $instructions,
        }
    };
    (
        server: $server:ty,
        task_tools: [$($task_tool:literal),* $(,)?],
        approval_tools: [$($approval_tool:literal),* $(,)?],
        cache_ttl_ms: $cache_ttl_ms:expr $(,)?
    ) => {
        $crate::mcp_2026_server! {
            @impl
            server: $server,
            task_tools: [$($task_tool),*],
            task_ttl_overrides: [],
            approval_tools: [$($approval_tool),*],
            mutating_tools: [$($approval_tool),*],
            destructive_tools: [],
            idempotent_tools: [],
            cache_ttl_ms: $cache_ttl_ms,
            instructions: "Stateless MCP server with structured results, risk annotations, Tasks when applicable, sealed MRTR approvals, per-request identity, and cache hints. Legacy initialization remains supported.",
        }
    };
    (
        @impl
        server: $server:ty,
        task_tools: [$($task_tool:literal),* $(,)?],
        task_ttl_overrides: [$(($ttl_tool:literal, $ttl_ms:expr)),* $(,)?],
        approval_tools: [$($approval_tool:literal),* $(,)?],
        mutating_tools: [$($mutating_tool:literal),* $(,)?],
        destructive_tools: [$($destructive_tool:literal),* $(,)?],
        idempotent_tools: [$($idempotent_tool:literal),* $(,)?],
        cache_ttl_ms: $cache_ttl_ms:expr,
        instructions: $instructions:expr $(,)?
    ) => {
        const _: () = {
            fn __adk_tasks() -> &'static ::rmcp::task_manager::TaskManager {
                static TASKS: ::std::sync::OnceLock<::rmcp::task_manager::TaskManager> =
                    ::std::sync::OnceLock::new();
                TASKS.get_or_init(::rmcp::task_manager::TaskManager::new)
            }

            impl ::rmcp::ServerHandler for $server {
                async fn call_tool(
                    &self,
                    request: ::rmcp::model::CallToolRequestParams,
                    context: ::rmcp::service::RequestContext<::rmcp::service::RoleServer>,
                ) -> ::std::result::Result<
                    ::rmcp::model::CallToolResponse,
                    ::rmcp::ErrorData,
                > {
                    let policy = $crate::Mcp2026Policy::new(
                        &[$($task_tool),*],
                        &[$($approval_tool),*],
                        $cache_ttl_ms,
                    ).with_tool_policies(
                        &[$(($ttl_tool, $ttl_ms)),*],
                        &[$($mutating_tool),*],
                        &[$($destructive_tool),*],
                        &[$($idempotent_tool),*],
                    );
                    let caller = context.client_info().map(|client| client.name);

                    if let Some(response) = $crate::approval_gate(&request, &context, &policy)? {
                        return Ok(response);
                    }

                    let client_supports_tasks = context
                        .client_capabilities()
                        .is_some_and(|capabilities| capabilities.supports_tasks());
                    if client_supports_tasks && policy.is_task_tool(request.name.as_ref()) {
                        let owned_server = self.clone();
                        let owned_context = context.clone();
                        let task = __adk_tasks().spawn(
                            ::rmcp::task_manager::TaskOptions::new()
                                .with_ttl_ms(policy.task_ttl_ms(request.name.as_ref()))
                                .with_poll_interval_ms(policy.task_poll_interval_ms)
                                .with_status_message(format!("Running {}", request.name)),
                            move |task_context| {
                                Box::pin(async move {
                                    let router = <$server>::tool_router();
                                    let call = router.call(
                                        ::rmcp::handler::server::tool::ToolCallContext::new(
                                            &owned_server,
                                            request,
                                            owned_context,
                                        ),
                                    );
                                    let cancellation = task_context.clone();
                                    $crate::scope_task_context(Some(task_context),
                                        $crate::scope_caller_identity(caller, async move { ::tokio::select! {
                                            _ = cancellation.cancelled() => {
                                                Err(::rmcp::task_manager::TaskExit::Cancelled)
                                            }
                                            result = call => match result.map($crate::normalize_call_response) {
                                                Ok(::rmcp::model::CallToolResponse::Complete(result)) => Ok(result),
                                                Ok(_) => Err(::rmcp::task_manager::TaskExit::Error(
                                                    ::rmcp::ErrorData::internal_error(
                                                        "nested task or input-required response is not supported",
                                                        None,
                                                    ),
                                                )),
                                                Err(error) => Err(::rmcp::task_manager::TaskExit::Error(error)),
                                            }
                                        }})
                                    ).await
                                })
                            },
                        );
                        return Ok(::rmcp::model::CallToolResponse::Task(
                            ::rmcp::model::CreateTaskResult::new(task),
                        ));
                    }

                    $crate::scope_caller_identity(caller, <$server>::tool_router()
                        .call(::rmcp::handler::server::tool::ToolCallContext::new(
                            self, request, context,
                        )))
                        .await
                        .map($crate::normalize_call_response)
                }

                async fn list_tools(
                    &self,
                    _request: Option<::rmcp::model::PaginatedRequestParams>,
                    _context: ::rmcp::service::RequestContext<::rmcp::service::RoleServer>,
                ) -> ::std::result::Result<::rmcp::model::ListToolsResult, ::rmcp::ErrorData> {
                    let policy = $crate::Mcp2026Policy::new(
                        &[$($task_tool),*],
                        &[$($approval_tool),*],
                        $cache_ttl_ms,
                    ).with_tool_policies(
                        &[$(($ttl_tool, $ttl_ms)),*],
                        &[$($mutating_tool),*],
                        &[$($destructive_tool),*],
                        &[$($idempotent_tool),*],
                    );
                    Ok(::rmcp::model::ListToolsResult::with_all_items(
                        $crate::enrich_tools(<$server>::tool_router().list_all(), &policy),
                    )
                    .with_ttl_ms($cache_ttl_ms)
                    .with_cache_scope(::rmcp::model::CacheScope::Public))
                }

                async fn get_task(
                    &self,
                    request: ::rmcp::model::GetTaskParams,
                    _context: ::rmcp::service::RequestContext<::rmcp::service::RoleServer>,
                ) -> ::std::result::Result<::rmcp::model::GetTaskResult, ::rmcp::ErrorData> {
                    Ok(::rmcp::model::GetTaskResult::new(
                        __adk_tasks().get_task(&request.task_id)?,
                    ))
                }

                async fn update_task(
                    &self,
                    request: ::rmcp::model::UpdateTaskParams,
                    _context: ::rmcp::service::RequestContext<::rmcp::service::RoleServer>,
                ) -> ::std::result::Result<(), ::rmcp::ErrorData> {
                    __adk_tasks().update_task(&request.task_id, request.input_responses)
                }

                async fn cancel_task(
                    &self,
                    request: ::rmcp::model::CancelTaskParams,
                    _context: ::rmcp::service::RequestContext<::rmcp::service::RoleServer>,
                ) -> ::std::result::Result<(), ::rmcp::ErrorData> {
                    __adk_tasks().cancel_task(&request.task_id)
                }

                fn get_info(&self) -> ::rmcp::model::ServerInfo {
                    let policy = $crate::Mcp2026Policy::new(
                        &[$($task_tool),*],
                        &[$($approval_tool),*],
                        $cache_ttl_ms,
                    );
                    let capabilities = if policy.has_task_tools() {
                        ::rmcp::model::ServerCapabilities::builder()
                            .enable_tools()
                            .enable_tasks()
                            .build()
                    } else {
                        ::rmcp::model::ServerCapabilities::builder()
                            .enable_tools()
                            .build()
                    };
                    ::rmcp::model::ServerInfo::new(
                        capabilities,
                    )
                    .with_server_info(::rmcp::model::Implementation::from_build_env())
                    .with_instructions($instructions.to_string())
                }
            }
        };
    };
}
