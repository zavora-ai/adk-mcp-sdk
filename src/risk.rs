use serde::{Deserialize, Serialize};

/// Tool risk classification per ADK-Rust Enterprise governance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    ReadOnly,
    GeneratedWriteDraft,
    InternalWrite,
    ExternalWrite,
    FinancialAction,
    IdentitySecurityAction,
    ProductionDeployment,
    FileCodeWrite,
}

/// Server-level risk designation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
