//! # ADK MCP SDK
//!
//! Enterprise traits for MCP servers targeting the ADK-Rust Enterprise registry.
//!
//! Servers implement these traits to declare their capabilities, tools, risk
//! levels, and health checks — enabling automatic onboarding into the registry.

pub mod health;
pub mod manifest;
pub mod risk;
pub mod tools;

pub use health::{HealthCheck, HealthStatus};
pub use manifest::ServerManifest;
pub use risk::RiskClass;
pub use tools::ToolMeta;
