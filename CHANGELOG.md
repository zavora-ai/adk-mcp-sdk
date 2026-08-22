# Changelog

## [0.3.0] - 2026-08-22

### Added
- Automatic promotion of JSON text tool results to MCP `structuredContent`, including tool-level error classification for `{ "ok": false }` responses.
- Generic JSON output schemas, human-readable tool titles, and MCP read-only, destructive, idempotent, and open-world annotations.
- Per-tool Task TTL overrides and task-local status context through `current_task_context` and `set_current_task_status`.
- Server-specific runtime instructions and tests covering structured results, annotations, conditional Tasks capability advertisement, and TTL overrides.

### Fixed
- Servers with no task-capable tools no longer advertise the Tasks extension.
- Long-running tools can set TTLs that exceed their maximum execution timeout; this prevents the task manager from expiring valid work.

### Changed
- Static tool catalogs are intended to use long public cache TTLs. Tool results are not cached by this policy.
- rmcp is pinned exactly to 3.1.2.

## [0.2.0] - 2026-08-13

### Changed
- Upgraded to rmcp 3.1.2 and raised the minimum supported Rust version to 1.94.1.
- Added MCP 2026-07-28 stateless request handling while retaining MCP 2025-11-25 initialization compatibility.

### Added
- Per-request identity and protocol metadata, on-demand discovery/cache hints, and the configured Tasks and sealed MRTR approval policies.

## [0.1.3] - 2025-05-24

### Added
- `ServerManifest::validate()` — validates manifest fields and returns a list of errors
  - Checks for empty `server_id`, `display_name`, `version`
  - Requires at least one transport
  - Detects duplicate tool names
  - Enforces `vault://` URI format for all credential bindings

### Changed
- Template server now enforces manifest validation on startup

## [0.1.2] - 2025-05-24

### Added
- Architecture SVG diagram (renders on crates.io via absolute GitHub raw URL)
- Removed ASCII fallback diagram — SVG is the single source of truth

## [0.1.1] - 2025-05-24

### Added
- Comprehensive README with architecture diagram, full API reference, risk class table, manifest reference, and complete server listing
- Example manifest file (`mcp-server.example.toml`)

### Changed
- Documentation improvements across all public types

## [0.1.0] - 2025-05-23

### Added
- `ServerManifest` — TOML-loadable server identity and capability declaration
- `HealthCheck` trait — async health status for registry monitoring
- `HealthStatus` — health response struct with status, message, latency
- `ToolMeta` — per-tool metadata with risk class and credential bindings
- `RiskClass` — 8-level tool risk classification enum
- `RiskLevel` — 4-level server risk designation enum
- `Transport` — supported transport protocol enum
- `WritesAllowed` — write permission level enum
- `ManifestError` — typed errors for manifest loading
