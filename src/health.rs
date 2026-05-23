use serde::{Deserialize, Serialize};

/// Health status reported by a server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: Option<String>,
    /// Latency of the last check in milliseconds.
    pub latency_ms: Option<u64>,
}

/// Trait that every ADK-compatible MCP server should implement.
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    /// Run connectivity, auth, and schema validation checks.
    async fn check_health(&self) -> HealthStatus;
}
