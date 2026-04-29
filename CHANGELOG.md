# Changelog

All notable changes to dbscope will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- **Live database integration tests**: Postgres, MySQL (via Docker services in CI), and SQLite (local file). Tests cover `extract_schema`, full analysis pipeline, lint, and impact analysis against real databases. 71 total tests.
- **CI `live-db` job**: GitHub Actions spins up Postgres 16 and MySQL 8 service containers and runs connector tests end-to-end
- **`dbscope mcp`**: MCP (Model Context Protocol) server over stdio for AI assistants (Claude, Cursor, Copilot). Exposes 6 tools: `analyze_schema`, `explain_risk`, `impact`, `lint_schema`, `deps`, `diff_schemas`
- **Docker workflow**: auto-publish to ghcr.io on release tag
- **Homebrew tap formula**: `brew install jayvenn21/tap/dbscope`
- **`dbscope demo`**: zero-config onboarding with embedded 17-table e-commerce schema (no database required)
- **`dbscope snapshot`**: save schema to JSON for offline analysis, auditing, and diffing
- **`dbscope diff`**: compare two schema snapshots or snapshot vs. live database; shows structural delta with breaking change detection
- **`dbscope lint`**: schema anti-pattern detection: missing PKs, wide tables, missing FK indexes, naming conventions, nullable FKs, redundant indexes, text-enum columns, junction table hints (8 rules)
- **`dbscope deps`**: full dependency tree visualization (upstream and downstream FK chains)
- **`dbscope completions`**: shell completions for bash, zsh, fish, and powershell
- **GitHub Action** (`action/action.yml`): 3-line CI integration for schema health checks
- **View & materialized view extraction** for Postgres, MySQL, SQLite, and ClickHouse
- **`--json` structured output** for `impact`, `summarize`, `plan`, `lint`, `deps`, `diff`, `explain` commands
- **`--format` flag** for `analyze`: select report formats (md, html, json, dot)
- **`--version` flag** with long description
- **Interactive HTML report**: click column headers to sort, search/filter tables
- **Column nullable & default tracking**: `is_nullable` and `default_value` in schema model
- **ALTER TABLE support in migration simulator**: ADD COLUMN, DROP COLUMN, RENAME TABLE, DROP CONSTRAINT
- **Policy validation** with warnings for out-of-range values
- **Deserialize on all core types**: JSON reports can be loaded back programmatically
- **Multi-dialect query parser**: GenericDialect handles PostgreSQL, MySQL, and ClickHouse syntax
- **Connection error diagnostics**: actionable messages for refused connections, auth failures, missing databases, timeouts
- **63 tests**: comprehensive test suite covering lint, diff, snapshot, demo, migrations, JSON roundtrip, policy validation, impact edge cases, CLI subprocess smoke tests
- **Database-aware default schema**: unqualified table names resolve using the actual schema data (MySQL uses DB name, SQLite uses `main`, ClickHouse uses `default`) instead of hardcoding `public`
- **GitHub Action handles all commands**: ci, analyze, lint, summarize, impact, deps, preview, snapshot, diff
- CI workflow (GitHub Actions: test, clippy, fmt, bench across platforms)
- Cross-compiled release workflow (linux/mac/windows, amd64/arm64)
- GitHub issue templates and PR template
- VISION.md with monetization plan and roadmap
- SECURITY.md for vulnerability reporting
- Dockerfile for containerized usage
- docs/cloud.md: Cloud product vision
- docs/positioning.md: How dbscope compares to migration linters
- Expanded README with comparison table, CI integration guide, and architecture diagram

### Fixed
- **SQLite connector PRAGMA bind params**: PRAGMAs (`table_info`, `index_list`, `foreign_key_list`) used `?` bind parameters which SQLite rejects; now uses quoted identifiers directly. Discovered by live integration tests.
- **Critical O(n^2) performance bug**: FK subgraph was rebuilt for every table during metrics computation; now built once and reused
- **`process::exit(1)` in library code**: `ci` and `preview` commands now return proper errors instead of hard-exiting, enabling clean error handling and testability
- **Duplicate FK subgraph builder**: consolidated into shared `FkGraph` utility in `core::graph`
- **Policy silent failures**: `Policy::load` now warns on missing/malformed files instead of silently falling back to defaults
- **Cargo.lock committed** for reproducible builds (removed from .gitignore)
- **Dockerfile handles missing Cargo.lock** gracefully
- **ClickHouse connector** no longer includes View/MaterializedView engines in base table list
- All Clippy warnings resolved (map_entry, redundant_closure, writeln_empty_string, type_complexity, io_other_error, collapsible_match, etc.)

## [0.1.0] - 2026-02-23

### Added
- Core CLI: `analyze`, `impact`, `ci`, `summarize`, `explain` commands
- Multi-database connectors: PostgreSQL, MySQL, SQLite, ClickHouse
- Graph-based risk model: FK depth, cycle detection, centrality scoring
- Operational weighting from `pg_stat_user_tables` (Postgres)
- Blast radius analysis: FK reach, index coupling, query usage weight
- Migration simulation: `preview` command with structural delta and risk delta
- Policy engine: YAML-based `max_table_risk`, `no_cycles`, `no_orphans`, `max_blast_radius_percent`
- Safe refactor planning: `plan drop` with FK dependency ordering
- Report generation: HTML, JSON, Markdown, Graphviz
- Query log analysis: cold/hot tables, index suggestions
- Benchmark suite (`cargo bench`)
- CONTRIBUTING.md with deterministic design principles

[Unreleased]: https://github.com/jayvenn21/dbscope/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jayvenn21/dbscope/releases/tag/v0.1.0
