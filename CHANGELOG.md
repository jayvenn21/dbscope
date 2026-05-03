# Changelog

All notable changes to dbscope will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- CI workflow (GitHub Actions: test, clippy, fmt, bench across platforms)
- Cross-compiled release workflow (linux/mac/windows, amd64/arm64)
- GitHub issue templates and PR template
- VISION.md with monetization plan and roadmap
- SECURITY.md for vulnerability reporting
- Dockerfile for containerized usage
- docs/cloud.md - Cloud product vision
- docs/positioning.md - How dbscope compares to migration linters
- Expanded README with comparison table, CI integration guide, and architecture diagram

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
