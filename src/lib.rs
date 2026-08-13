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
pub use protocol::{Mcp2026Policy, approval_gate, caller_identity};
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
        approval_tools: [$($approval_tool:literal),* $(,)?],
        cache_ttl_ms: $cache_ttl_ms:expr $(,)?
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
                    );

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
                                .with_ttl_ms(policy.task_ttl_ms)
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
                                    ::tokio::select! {
                                        _ = task_context.cancelled() => {
                                            Err(::rmcp::task_manager::TaskExit::Cancelled)
                                        }
                                        result = call => match result {
                                            Ok(::rmcp::model::CallToolResponse::Complete(result)) => Ok(result),
                                            Ok(_) => Err(::rmcp::task_manager::TaskExit::Error(
                                                ::rmcp::ErrorData::internal_error(
                                                    "nested task or input-required response is not supported",
                                                    None,
                                                ),
                                            )),
                                            Err(error) => Err(::rmcp::task_manager::TaskExit::Error(error)),
                                        }
                                    }
                                })
                            },
                        );
                        return Ok(::rmcp::model::CallToolResponse::Task(
                            ::rmcp::model::CreateTaskResult::new(task),
                        ));
                    }

                    <$server>::tool_router()
                        .call(::rmcp::handler::server::tool::ToolCallContext::new(
                            self, request, context,
                        ))
                        .await
                }

                async fn list_tools(
                    &self,
                    _request: Option<::rmcp::model::PaginatedRequestParams>,
                    _context: ::rmcp::service::RequestContext<::rmcp::service::RoleServer>,
                ) -> ::std::result::Result<::rmcp::model::ListToolsResult, ::rmcp::ErrorData> {
                    Ok(::rmcp::model::ListToolsResult::with_all_items(
                        <$server>::tool_router().list_all(),
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
                    ::rmcp::model::ServerInfo::new(
                        ::rmcp::model::ServerCapabilities::builder()
                            .enable_tools()
                            .enable_tasks()
                            .build(),
                    )
                    .with_server_info(::rmcp::model::Implementation::from_build_env())
                    .with_instructions(
                        "Stateless MCP 2026 server with Tasks, sealed MRTR approvals, per-request identity, and cache hints. Legacy initialization remains supported."
                            .to_string(),
                    )
                }
            }
        };
    };
}
