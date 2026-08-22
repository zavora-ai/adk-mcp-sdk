//! Shared policy for the MCP 2026-07-28 stateless protocol.

use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use rmcp::{
    ErrorData,
    model::{
        CallToolRequestParams, CallToolResponse, ElicitRequest, ElicitRequestParams, InputRequest,
        InputRequests, InputRequiredResult, ProtocolVersion, RequestStateCodec, SealOptions, Tool,
        ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
    task_manager::TaskContext,
};
use serde::{Deserialize, Serialize};

tokio::task_local! {
    static CALLER_IDENTITY: Option<String>;
    static TASK_CONTEXT: Option<TaskContext>;
}

/// Run a tool under its request-scoped caller identity. Server implementations
/// can read this with [`current_caller_identity`] for audit attribution without
/// accepting a spoofable actor field from tool arguments.
pub async fn scope_caller_identity<F>(identity: Option<String>, future: F) -> F::Output
where
    F: std::future::Future,
{
    CALLER_IDENTITY.scope(identity, future).await
}

/// Identity bound by the shared server handler for the current tool call.
pub fn current_caller_identity() -> Option<String> {
    CALLER_IDENTITY.try_with(Clone::clone).ok().flatten()
}

/// Run a tool with access to its protocol-native task context.
pub async fn scope_task_context<F>(context: Option<TaskContext>, future: F) -> F::Output
where
    F: std::future::Future,
{
    TASK_CONTEXT.scope(context, future).await
}

/// Return the active task context when a tool was materialized as a Task.
pub fn current_task_context() -> Option<TaskContext> {
    TASK_CONTEXT.try_with(Clone::clone).ok().flatten()
}

/// Update the status of the current Task. Direct and legacy tool calls are a no-op.
pub fn set_current_task_status(message: impl Into<String>) {
    if let Some(context) = current_task_context() {
        context.set_status_message(message);
    }
}

/// Shared defaults for tools-only MCP servers.
#[derive(Debug, Clone, Copy)]
pub struct Mcp2026Policy {
    task_tools: &'static [&'static str],
    task_ttl_overrides: &'static [(&'static str, u64)],
    approval_tools: &'static [&'static str],
    mutating_tools: &'static [&'static str],
    destructive_tools: &'static [&'static str],
    idempotent_tools: &'static [&'static str],
    pub cache_ttl_ms: u64,
    pub task_ttl_ms: u64,
    pub task_poll_interval_ms: u64,
}

impl Mcp2026Policy {
    pub const fn new(
        task_tools: &'static [&'static str],
        approval_tools: &'static [&'static str],
        cache_ttl_ms: u64,
    ) -> Self {
        Self {
            task_tools,
            task_ttl_overrides: &[],
            approval_tools,
            mutating_tools: approval_tools,
            destructive_tools: &[],
            idempotent_tools: &[],
            cache_ttl_ms,
            task_ttl_ms: 15 * 60 * 1_000,
            task_poll_interval_ms: 250,
        }
    }

    pub const fn with_tool_policies(
        mut self,
        task_ttl_overrides: &'static [(&'static str, u64)],
        mutating_tools: &'static [&'static str],
        destructive_tools: &'static [&'static str],
        idempotent_tools: &'static [&'static str],
    ) -> Self {
        self.task_ttl_overrides = task_ttl_overrides;
        self.mutating_tools = mutating_tools;
        self.destructive_tools = destructive_tools;
        self.idempotent_tools = idempotent_tools;
        self
    }

    pub fn has_task_tools(&self) -> bool {
        !self.task_tools.is_empty()
    }

    pub fn task_ttl_ms(&self, name: &str) -> u64 {
        self.task_ttl_overrides
            .iter()
            .find_map(|(tool, ttl)| (*tool == name).then_some(*ttl))
            .unwrap_or(self.task_ttl_ms)
    }

    pub fn is_task_tool(&self, name: &str) -> bool {
        self.task_tools.contains(&name)
    }

    pub fn requires_approval(&self, name: &str) -> bool {
        self.approval_tools.contains(&name)
    }

    pub fn is_mutating_tool(&self, name: &str) -> bool {
        self.mutating_tools.contains(&name)
    }

    pub fn is_destructive_tool(&self, name: &str) -> bool {
        self.destructive_tools.contains(&name)
    }

    pub fn is_idempotent_tool(&self, name: &str) -> bool {
        !self.is_mutating_tool(name) || self.idempotent_tools.contains(&name)
    }
}

/// Add a generic JSON object schema and MCP behavior annotations to a tool catalog.
///
/// Server handlers may still declare a more precise output schema; this only fills
/// schemas and annotation fields that the tool macro left empty.
pub fn enrich_tools(mut tools: Vec<Tool>, policy: &Mcp2026Policy) -> Vec<Tool> {
    let output_schema = Arc::new(
        serde_json::json!({
            "type": "object",
            "description": "Structured JSON result. Tool-specific fields are documented in the tool description and server README.",
            "additionalProperties": true
        })
        .as_object()
        .expect("object schema")
        .clone(),
    );
    for tool in &mut tools {
        if tool.output_schema.is_none() {
            tool.output_schema = Some(output_schema.clone());
        }
        let name = tool.name.as_ref();
        let title = name
            .split('_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        let mut annotations = tool.annotations.take().unwrap_or_default();
        annotations.title.get_or_insert(title.clone());
        annotations
            .read_only_hint
            .get_or_insert(!policy.is_mutating_tool(name));
        annotations
            .destructive_hint
            .get_or_insert(policy.is_destructive_tool(name));
        annotations
            .idempotent_hint
            .get_or_insert(policy.is_idempotent_tool(name));
        annotations.open_world_hint.get_or_insert(false);
        tool.title.get_or_insert(title);
        tool.annotations = Some(ToolAnnotations::from_raw(
            annotations.title,
            annotations.read_only_hint,
            annotations.destructive_hint,
            annotations.idempotent_hint,
            annotations.open_world_hint,
        ));
    }
    tools
}

/// Promote legacy JSON text results to MCP structured content and mark
/// `{ "ok": false }` responses as tool-level errors.
pub fn normalize_call_response(response: CallToolResponse) -> CallToolResponse {
    let CallToolResponse::Complete(mut result) = response else {
        return response;
    };
    if result.structured_content.is_none()
        && let Some(text) = result.content.first().and_then(|content| content.as_text())
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text.text)
    {
        if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
            result.is_error = Some(true);
        }
        result.structured_content = Some(value);
    }
    CallToolResponse::Complete(result)
}

#[derive(Debug, Serialize, Deserialize)]
struct ApprovalState {
    client_name: String,
    tool_name: String,
}

/// Return the request-scoped client identity. Stateless MCP requests must carry
/// this in `_meta`; legacy sessions use the identity negotiated at initialize.
pub fn caller_identity(context: &RequestContext<RoleServer>) -> Result<String, ErrorData> {
    context
        .client_info()
        .map(|client| client.name)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            ErrorData::invalid_request("client identity is required for protected operations", None)
        })
}

fn request_state_codec() -> Result<&'static RequestStateCodec, ErrorData> {
    static CODEC: OnceLock<Result<RequestStateCodec, String>> = OnceLock::new();
    match CODEC.get_or_init(|| {
        let key = std::env::var("MCP_REQUEST_STATE_KEY")
            .map_err(|_| "MCP_REQUEST_STATE_KEY must be configured".to_string())?;
        if key.as_bytes().len() < 32 {
            return Err("MCP_REQUEST_STATE_KEY must contain at least 32 bytes".to_string());
        }
        Ok(RequestStateCodec::new(key.into_bytes()))
    }) {
        Ok(codec) => Ok(codec),
        Err(message) => Err(ErrorData::invalid_request(message.clone(), None)),
    }
}

fn approval_binding(
    request: &CallToolRequestParams,
    client_name: &str,
) -> Result<Vec<u8>, ErrorData> {
    serde_json::to_vec(&(
        "tools/call",
        request.name.as_ref(),
        request.arguments.as_ref(),
        client_name,
    ))
    .map_err(|error| ErrorData::internal_error(error.to_string(), None))
}

fn response_accepted(request: &CallToolRequestParams) -> bool {
    request
        .input_responses
        .as_ref()
        .and_then(|responses| responses.get("approval"))
        .and_then(|value| value.get("action"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|action| action.eq_ignore_ascii_case("accept"))
}

/// Apply a fail-closed, integrity-protected MRTR approval gate.
///
/// `Ok(Some(_))` is the first-round `input_required` response. `Ok(None)`
/// means the request has a valid approval and can be dispatched. Legacy peers
/// cannot execute protected tools because they cannot prove MRTR approval.
pub fn approval_gate(
    request: &CallToolRequestParams,
    context: &RequestContext<RoleServer>,
    policy: &Mcp2026Policy,
) -> Result<Option<CallToolResponse>, ErrorData> {
    if !policy.requires_approval(request.name.as_ref()) {
        return Ok(None);
    }

    if context.protocol_version() != Some(ProtocolVersion::V_2026_07_28) {
        return Err(ErrorData::invalid_request(
            "this protected tool requires MCP 2026-07-28 MRTR approval",
            None,
        ));
    }

    let client_name = caller_identity(context)?;
    let binding = approval_binding(request, &client_name)?;
    let codec = request_state_codec()?;

    if let Some(sealed) = request.request_state.as_deref() {
        let state: ApprovalState = codec
            .open_json_with(sealed, &binding)
            .map_err(|error| ErrorData::invalid_request(error.to_string(), None))?;
        if state.client_name != client_name || state.tool_name != request.name {
            return Err(ErrorData::invalid_request(
                "approval state does not match this caller or tool",
                None,
            ));
        }
        if !response_accepted(request) {
            return Err(ErrorData::invalid_request(
                "protected operation was not approved",
                None,
            ));
        }
        return Ok(None);
    }

    let state = ApprovalState {
        client_name,
        tool_name: request.name.to_string(),
    };
    let sealed = codec
        .seal_json_with(
            &state,
            &SealOptions::new()
                .associated_data(&binding)
                .ttl(Duration::from_secs(120)),
        )
        .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    let schema = serde_json::from_value(serde_json::json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["accept", "decline"]
            }
        },
        "required": ["action"]
    }))
    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
    let request_message = format!(
        "Approve protected tool '{}' for client '{}'?",
        request.name, state.client_name
    );
    let mut input_requests = InputRequests::new();
    input_requests.insert(
        "approval".to_string(),
        InputRequest::Elicitation(ElicitRequest::new(
            ElicitRequestParams::FormElicitationParams {
                meta: None,
                message: request_message,
                requested_schema: schema,
            },
        )),
    );
    Ok(Some(
        InputRequiredResult::new(Some(input_requests), Some(sealed)).into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_classifies_tools() {
        let policy = Mcp2026Policy::new(&["export"], &["delete"], 60_000);
        assert!(policy.is_task_tool("export"));
        assert!(policy.requires_approval("delete"));
        assert!(!policy.requires_approval("read"));
    }
}
