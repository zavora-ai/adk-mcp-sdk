use serde::{Deserialize, Serialize};

use crate::risk::RiskLevel;
use crate::tools::ToolMeta;

/// Transport protocol supported by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Stdio,
    Sse,
    StreamableHttp,
}

/// Write permission level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritesAllowed {
    None,
    Gated,
    Approved,
}

/// The server manifest — the contract between an MCP server and the registry.
///
/// Can be loaded from `mcp-server.toml` or constructed in code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerManifest {
    pub server_id: String,
    pub display_name: String,
    pub version: String,
    pub domain: String,
    pub transports: Vec<Transport>,
    pub risk_level: RiskLevel,
    pub writes_allowed: WritesAllowed,
    /// Minimum SDK version this server was built against.
    pub sdk_version: String,
    /// Tools this server exposes.
    #[serde(default)]
    pub tools: Vec<ToolMeta>,
    /// Credential IDs required from the vault.
    #[serde(default)]
    pub credentials: Vec<String>,
    /// Governance gate labels.
    #[serde(default)]
    pub governance_gates: Vec<String>,
    /// Environments this server is deployed to.
    #[serde(default)]
    pub environments: Vec<String>,
}

impl ServerManifest {
    /// Load manifest from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Load manifest from a file path.
    pub fn from_file(path: &std::path::Path) -> Result<Self, ManifestError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ManifestError::Io(e.to_string()))?;
        Self::from_toml(&content).map_err(|e| ManifestError::Parse(e.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Parse error: {0}")]
    Parse(String),
}
