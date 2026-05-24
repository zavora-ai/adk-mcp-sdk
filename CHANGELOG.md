# Changelog

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
