# ADK MCP SDK

[![Crates.io](https://img.shields.io/crates/v/adk-mcp-sdk.svg)](https://crates.io/crates/adk-mcp-sdk)
[![Docs.rs](https://docs.rs/adk-mcp-sdk/badge.svg)](https://docs.rs/adk-mcp-sdk)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![ADK-Rust Enterprise](https://img.shields.io/badge/ADK--Rust-Enterprise-purple.svg)](https://enterprise.adk-rust.com)

The shared contract between MCP servers and the [ADK-Rust Enterprise](https://enterprise.adk-rust.com) registry. Every server in the ecosystem implements these traits to enable automatic onboarding, health monitoring, tool governance, and risk classification.

## Architecture

<p align="center">
  <img src="https://raw.githubusercontent.com/zavora-ai/adk-mcp-sdk/main/docs/assets/architecture.svg" alt="ADK MCP SDK Architecture" width="800"/>
</p>

## What This Crate Provides

| Export | Type | Purpose |
|--------|------|---------|
| `ServerManifest` | Struct | TOML-loadable server identity, capabilities, and tool declarations |
| `HealthCheck` | Trait | Async health probe for registry monitoring |
| `HealthStatus` | Struct | Health response with status, message, and latency |
| `ToolMeta` | Struct | Per-tool metadata with risk class and credential bindings |
| `RiskClass` | Enum | Tool-level risk classification (8 levels) |
| `RiskLevel` | Enum | Server-level risk designation (4 levels) |
| `Transport` | Enum | Supported transport protocols (stdio, SSE, HTTP) |
| `WritesAllowed` | Enum | Write permission level (none, gated, approved) |
| `ManifestError` | Error | Typed errors for manifest loading |

## Installation

```toml
[dependencies]
adk-mcp-sdk = "0.1"
```

## Quick Start

### 1. Implement HealthCheck

```rust
use adk_mcp_sdk::{HealthCheck, HealthStatus};

#[derive(Clone)]
struct MyServer;

#[async_trait::async_trait]
impl HealthCheck for MyServer {
    async fn check_health(&self) -> HealthStatus {
        // Check your backend connectivity, auth, schema, etc.
        HealthStatus {
            healthy: true,
            message: Some("All systems operational".into()),
            latency_ms: Some(5),
        }
    }
}
```

### 2. Create `mcp-server.toml`

```toml
server_id = "mcp_my_server"
display_name = "My MCP Server"
version = "1.0.0"
domain = "platform"
sdk_version = "0.1.0"
risk_level = "medium"
writes_allowed = "gated"
transports = ["stdio"]
credentials = ["vault://my-api-key"]
governance_gates = []
environments = ["development", "staging", "production"]

[[tools]]
name = "list_items"
description = "List items with optional filters"
risk_class = "read_only"
requires_approval = false
credential_bindings = []

[[tools]]
name = "create_item"
description = "Create a new item (gated write)"
risk_class = "internal_write"
requires_approval = true
credential_bindings = ["vault://my-api-key"]
```

### 3. Load Manifest in Code

```rust
use adk_mcp_sdk::ServerManifest;
use std::path::Path;

// From file
let manifest = ServerManifest::from_file(Path::new("mcp-server.toml"))?;

// From string
let manifest = ServerManifest::from_toml(include_str!("../mcp-server.toml"))?;

// Programmatic
let manifest = ServerManifest {
    server_id: "mcp_my_server".into(),
    display_name: "My MCP Server".into(),
    version: env!("CARGO_PKG_VERSION").into(),
    domain: "platform".into(),
    transports: vec![adk_mcp_sdk::manifest::Transport::Stdio],
    risk_level: adk_mcp_sdk::risk::RiskLevel::Medium,
    writes_allowed: adk_mcp_sdk::manifest::WritesAllowed::Gated,
    sdk_version: "0.1.0".into(),
    tools: vec![],
    credentials: vec![],
    governance_gates: vec![],
    environments: vec!["development".into(), "production".into()],
};
```

### 4. Declare Tool Metadata

```rust
use adk_mcp_sdk::{RiskClass, ToolMeta};

let tools = vec![
    ToolMeta {
        name: "search_items".into(),
        description: "Search items by query".into(),
        risk_class: RiskClass::ReadOnly,
        requires_approval: false,
        credential_bindings: vec![],
    },
    ToolMeta {
        name: "delete_item".into(),
        description: "Delete an item permanently".into(),
        risk_class: RiskClass::ExternalWrite,
        requires_approval: true,
        credential_bindings: vec!["vault://admin-key".into()],
    },
];
```

## Risk Classes

The governance engine uses risk classes to determine approval requirements, audit depth, and environment restrictions.

| Class | Enum Variant | Examples | Default Policy |
|-------|-------------|----------|----------------|
| Read-only | `ReadOnly` | search, get, list, inspect | Allowed if scoped |
| Generated draft | `GeneratedWriteDraft` | create draft, proposal | Allowed in staging |
| Internal write | `InternalWrite` | add note, update ticket, assign | Gated by owner policy |
| External write | `ExternalWrite` | send email, post Slack, create PR | Approval required |
| Financial | `FinancialAction` | capture payment, refund, payout | Intent + approval |
| Identity/Security | `IdentitySecurityAction` | reset password, revoke access | Identity verification + approver |
| Production deploy | `ProductionDeployment` | promote, deploy, rollback | Release governance required |
| File/Code write | `FileCodeWrite` | write file, create branch | Workspace trust + repo policy |

## Risk Levels (Server-Level)

| Level | When to Use | Registry Behavior |
|-------|-------------|-------------------|
| `Low` | Read-only servers, search, analytics | Auto-approved onboarding |
| `Medium` | Servers with gated writes | Standard review |
| `High` | Servers with external writes or sensitive data | Security review required |
| `Critical` | Credential vaults, identity, financial | Full governance audit |

## Server Manifest Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `server_id` | String | ✅ | Unique identifier (snake_case) |
| `display_name` | String | ✅ | Human-readable name |
| `version` | String | ✅ | SemVer version |
| `domain` | String | ✅ | Domain category (platform, security, devops, etc.) |
| `sdk_version` | String | ✅ | Minimum SDK version |
| `risk_level` | Enum | ✅ | low, medium, high, critical |
| `writes_allowed` | Enum | ✅ | none, gated, approved |
| `transports` | Array | ✅ | stdio, sse, streamable_http |
| `credentials` | Array | ❌ | Vault credential URIs needed |
| `governance_gates` | Array | ❌ | Required governance checks |
| `environments` | Array | ❌ | Target deployment environments |
| `tools` | Array | ❌ | Tool declarations (see ToolMeta) |

## Complete Example: Building an MCP Server

```rust
use adk_mcp_sdk::{HealthCheck, HealthStatus};
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router, ServiceExt, transport::stdio};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchInput {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}
fn default_limit() -> u32 { 20 }

#[derive(Clone)]
pub struct MyServer;

#[tool_router(server_handler)]
impl MyServer {
    #[tool(description = "Search items by query")]
    async fn search(&self, Parameters(input): Parameters<SearchInput>) -> String {
        format!("Found results for '{}' (limit {})", input.query, input.limit)
    }
}

#[async_trait::async_trait]
impl HealthCheck for MyServer {
    async fn check_health(&self) -> HealthStatus {
        HealthStatus { healthy: true, message: None, latency_ms: Some(1) }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let service = MyServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

## Servers Using This SDK

| Server | Version | Domain | Description |
|--------|---------|--------|-------------|
| [mcp-credentials-vault](https://crates.io/crates/mcp-credentials-vault) | 1.1.0 | security | Scoped credential access, 5 vault backends |
| [mcp-artifact-store](https://crates.io/crates/mcp-artifact-store) | 1.1.0 | platform | Governed artifact registry with provenance |
| [mcp-session-memory](https://crates.io/crates/mcp-session-memory) | 1.1.0 | platform | Session and memory persistence |
| [mcp-a2a](https://crates.io/crates/mcp-a2a) | 1.2.0 | agents | A2A remote agent protocol |
| [mcp-acp-workspace](https://crates.io/crates/mcp-acp-workspace) | 1.2.0 | agents | ACP workspace control plane |
| [mcp-governance-policy](https://crates.io/crates/mcp-governance-policy) | 1.1.0 | governance | Policy evaluation and approvals |
| [mcp-environment](https://crates.io/crates/mcp-environment) | 1.2.0 | platform | Environment registry and deployments |
| [mcp-registry](https://crates.io/crates/mcp-registry) | 1.1.0 | platform | MCP server registration and discovery |
| [mcp-identity](https://crates.io/crates/mcp-identity) | 1.1.0 | security | Identity, access, and entitlements |
| [mcp-device-management](https://crates.io/crates/mcp-device-management) | 1.5.0 | endpoint-security | Device posture and remediation |
| [adk-mcp-github](https://crates.io/crates/adk-mcp-github) | 1.3.0 | developer-tools | GitHub repos, PRs, issues, workflows |
| [mcp-cicd](https://crates.io/crates/mcp-cicd) | 1.1.0 | devops | CI/CD pipelines and deployments |
| [mcp-code-search](https://crates.io/crates/mcp-code-search) | 1.1.0 | developer-tools | Semantic code search and call graphs |
| [mcp-itsm](https://crates.io/crates/mcp-itsm) | 1.1.0 | operations | Tickets, incidents, change requests |
| [mcp-knowledge-base](https://crates.io/crates/mcp-knowledge-base) | 1.2.0 | operations | Articles, search, feedback loops |
| [mcp-security-advisory](https://crates.io/crates/mcp-security-advisory) | 1.1.0 | security | CVE/GHSA/OSV advisory search and audit |
| [mcp-package-registry](https://crates.io/crates/mcp-package-registry) | 1.1.0 | developer-tools | Dependency metadata and upgrade plans |
| [mcp-test-runner](https://crates.io/crates/mcp-test-runner) | 1.1.0 | developer-tools | Test execution and coverage |

## Creating a New Server

```bash
# Clone the template
git clone https://github.com/zavora-ai/mcp-server-template my-server
cd my-server

# Edit Cargo.toml — rename package, update description
# Edit mcp-server.toml — set server_id, tools, risk level
# Edit src/main.rs — implement your tools

# Build and test
cargo build
cargo test

# Publish
cargo publish
```

The template includes a working `#[tool_router(server_handler)]` example with HealthCheck, tracing, and stdio transport.

## Integration with rmcp

This SDK provides **metadata and governance** — it does not replace [rmcp](https://crates.io/crates/rmcp) which provides the MCP protocol transport. A typical server uses both:

```toml
[dependencies]
adk-mcp-sdk = "0.1"                    # Registry contract
rmcp = { version = "1.7", features = ["server", "transport-io", "macros"] }  # MCP protocol
```

- **rmcp** handles: tool routing, JSON-RPC, stdio/HTTP transport, schema generation
- **adk-mcp-sdk** handles: health monitoring, manifest declaration, risk classification, credential binding

## Documentation

| Document | Description |
|----------|-------------|
| [Rust Docs](https://docs.rs/adk-mcp-sdk) | Generated API documentation |
| [mcp-server.example.toml](mcp-server.example.toml) | Annotated manifest example |
| [CHANGELOG.md](CHANGELOG.md) | Version history |
| [LICENSE](LICENSE) | Apache-2.0 license |

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feat/my-feature`)
3. Implement with tests
4. Submit a pull request

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
