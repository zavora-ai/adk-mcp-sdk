# ADK MCP SDK

[![Crates.io](https://img.shields.io/crates/v/adk-mcp-sdk.svg)](https://crates.io/crates/adk-mcp-sdk)
[![Docs.rs](https://docs.rs/adk-mcp-sdk/badge.svg)](https://docs.rs/adk-mcp-sdk)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![ADK-Rust Enterprise](https://img.shields.io/badge/ADK--Rust-Enterprise-purple.svg)](https://enterprise.adk-rust.com)

Enterprise SDK traits for MCP servers targeting the [ADK-Rust Enterprise](https://enterprise.adk-rust.com) registry. Provides the shared contract that enables automatic server onboarding, health monitoring, tool governance, and risk classification.

## What This Crate Provides

| Type | Purpose |
|------|---------|
| `ServerManifest` | TOML-loadable server identity, capabilities, and tool declarations |
| `HealthCheck` | Async trait for connectivity, auth, and schema validation |
| `ToolMeta` | Per-tool metadata with risk class and credential bindings |
| `RiskClass` | Tool risk classification (read-only → production deployment) |
| `RiskLevel` | Server-level risk designation (low → critical) |

## Quick Start

```toml
[dependencies]
adk-mcp-sdk = "0.1"
```

```rust
use adk_mcp_sdk::{HealthCheck, HealthStatus, RiskClass, ServerManifest, ToolMeta};

struct MyServer;

#[async_trait::async_trait]
impl HealthCheck for MyServer {
    async fn check_health(&self) -> HealthStatus {
        HealthStatus {
            healthy: true,
            message: Some("All systems operational".into()),
            latency_ms: Some(5),
        }
    }
}
```

## Server Manifest

Every ADK-compatible MCP server ships a `mcp-server.toml`:

```toml
server_id = "mcp_my_server"
display_name = "My MCP Server"
version = "1.0.0"
domain = "platform"
sdk_version = "0.1.0"
risk_level = "medium"
writes_allowed = "gated"
transports = ["stdio", "streamable_http"]
credentials = ["vault://my-api-key"]
governance_gates = []
environments = ["development", "staging", "production"]

[[tools]]
name = "list_items"
description = "List items with optional filters"
risk_class = "read_only"
requires_approval = false
credential_bindings = []
```

Load it in code:

```rust
let manifest = ServerManifest::from_file(Path::new("mcp-server.toml"))?;
```

## Risk Classes

| Class | Examples | Default Policy |
|-------|----------|----------------|
| `ReadOnly` | search, get, list | Allowed if scoped |
| `GeneratedWriteDraft` | create draft, proposal | Allowed in staging |
| `InternalWrite` | add note, update ticket | Gated by owner policy |
| `ExternalWrite` | send email, post Slack | Approval required |
| `FinancialAction` | capture, refund, payout | Always through intent + approval |
| `IdentitySecurityAction` | reset password, revoke | Identity verification + approver |
| `ProductionDeployment` | promote, deploy, rollback | Release governance required |
| `FileCodeWrite` | write file, create PR | Workspace trust + repo policy |

## Servers Using This SDK

| Server | Version | Description |
|--------|---------|-------------|
| [mcp-credentials-vault](https://crates.io/crates/mcp-credentials-vault) | 1.0.0 | Scoped credential access with 6 vault backends |
| [mcp-artifact-store](https://github.com/zavora-ai/mcp-artifact-store) | 1.0.0 | Governed artifact registry with provenance |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Contributors

<!-- ALL-CONTRIBUTORS-LIST:START -->
| [<img src="https://github.com/jkmaina.png" width="80px;" alt=""/><br /><sub><b>James Karanja Maina</b></sub>](https://github.com/jkmaina) |
|:---:|
<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.

---

Part of the [ADK-Rust Enterprise](https://enterprise.adk-rust.com) MCP server ecosystem.

Built with ❤️ by [Zavora AI](https://zavora.ai)
