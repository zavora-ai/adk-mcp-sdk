use serde::{Deserialize, Serialize};

use crate::risk::RiskClass;

/// Metadata for a single tool exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMeta {
    pub name: String,
    pub description: String,
    pub risk_class: RiskClass,
    /// Whether this tool requires approval before execution in production.
    #[serde(default)]
    pub requires_approval: bool,
    /// Credential IDs this tool needs at runtime.
    #[serde(default)]
    pub credential_bindings: Vec<String>,
}
