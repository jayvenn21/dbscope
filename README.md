<p align="center">
  <img src="dbscope-logo-cropped.png" width="65%" alt="dbscope logo">
</p>

<p align="center">
  Universal relational schema intelligence
</p>

<p align="center">
  <a href="https://github.com/jayvenn21/dbscope/releases"><img src="https://img.shields.io/github/v/release/jayvenn21/dbscope" alt="Release"></a>
  <img src="https://img.shields.io/badge/status-active-brightgreen" alt="Status">
  <img src="https://img.shields.io/badge/postgres-supported-blue" alt="Postgres">
  <img src="https://img.shields.io/badge/read--only-safe-success" alt="Read Only">
  <img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License">
  <img src="https://img.shields.io/badge/risk-deterministic-purple" alt="Risk Model">
</p>

<p align="center">
  <strong>Understand your database before you touch it.</strong><br>
  Read-only static + dynamic analysis for SQL databases.<br>
  Deterministic risk scoring. Offline reports. No telemetry.
</p>

<p align="center">
  PostgreSQL (production) · MySQL / SQLite / ClickHouse (connector interface)
</p>

<p align="center">
  <a href="#why-dbscope">Why</a> ·
  <a href="#quick-start">Quick Start</a> ·
  <a href="#commands">Commands</a> ·
  <a href="#plan">plan</a> ·
  <a href="#policy">Policy</a> ·
  <a href="#risk-model">Risk Model</a> ·
  <a href="#reports">Reports</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#philosophy">Philosophy</a>
</p>

---

## Why dbscope

Most tools manage migrations or monitor performance. dbscope analyzes structure. It answers: what breaks if I drop this table? Which tables are central? Which columns are never queried? Where am I missing indexes (from real queries)? What is my schema risk profile? It builds a unified relational graph and computes explainable risk metrics.

**Supports:** PostgreSQL (production), MySQL / SQLite / ClickHouse (connector interface).

---

## Quick Start

```bash
cargo build --release
export DBSCOPE_SCHEMA_URI="postgres://USER:PASS@localhost:5432/DBNAME"

dbscope analyze
dbscope impact public.users
dbscope summarize
```

**Reports:** `dbscope-report.html`, `dbscope-report.json`, `dbscope-report.md`, `dbscope-graph.dot` (use `-o DIR` to set output directory).

---

## Example

<p align="center">
  <img src="dbscope.gif" width="850" alt="Terminal demo: analyze, impact, ci">
</p>

---

## Commands

All commands accept `--schema URI` or `DBSCOPE_SCHEMA_URI`. Omit `--schema` when the env is set.

### analyze

Extract schema, build graph, compute metrics, generate reports.

```bash
dbscope analyze --schema <URI>
dbscope analyze --schema <URI> -o <DIR>
dbscope analyze --schema <URI> --query-log <FILE>
```

| Option | Description |
|--------|-------------|
| `--schema` | Connection URI. Required unless `DBSCOPE_SCHEMA_URI` is set. |
| `-o`, `--output` | Output directory for reports (default: current directory). |
| `--query-log` | One SQL per line → cold/hot tables, index suggestions. |

### impact

Blast radius for a table or column: downstream/upstream FKs, index coupling, affected queries, risk breakdown. **Target:** `users`, `users.email`, `public.users`, `public.users.email`.

```bash
dbscope impact <TARGET> --schema <URI>
dbscope impact <TARGET> --schema <URI> --query-log <FILE>
```

### plan

Safe refactor plan for dropping a table: lists FKs to drop first, then the DROP TABLE step. Read-only; apply changes manually.

```bash
dbscope plan drop public.users --schema <URI>
dbscope plan drop users --schema <URI>
```

### preview

Simulate a migration and report structural delta, risk delta, blast radius, and policy result. Exits 1 if policy is violated (when `--policy` is set).

```bash
dbscope preview <MIGRATION.sql> --schema <URI>
dbscope preview <MIGRATION.sql> --schema <URI> --query-log <FILE> --policy dbscope.policy.yaml
```

Output: tables removed, FKs removed, new cycles, risk delta, % of schema graph impacted, observed queries broken; then Policy PASS/FAIL.

| Option | Description |
|--------|-------------|
| `--query-log` | Count queries that reference removed tables (broken). |
| `--policy` | YAML policy file. If set, exit 1 on violation. |

### ci

Exit 1 if any table risk exceeds threshold (or policy). Optional `--migration` to simulate DDL. With `--policy`, enforces `max_table_risk`, `no_cycles`, `no_orphans`.

```bash
dbscope ci --schema <URI>
dbscope ci --schema <URI> --threshold 0.5 --migration <FILE>
dbscope ci --schema <URI> --policy dbscope.policy.yaml
```

| Option | Description |
|--------|-------------|
| `--threshold` | Fail if table risk &gt; this (0–1). Default: 0.5. Ignored if `--policy` is set. |
| `--migration` | DDL file to simulate (DROP/CREATE TABLE, ALTER ADD FK). |
| `--policy` | YAML file: `max_table_risk`, `no_cycles`, `no_orphans`, `max_blast_radius_percent`. |

### summarize

Table/column/FK counts, risk overview, orphans, cycles. With `--query-log`: cold/hot tables, index suggestions.

```bash
dbscope summarize --schema <URI>
dbscope summarize --schema <URI> --query-log <FILE>
```

### explain

Explain risk score or index recommendation. **KIND:** `risk` or `index-suggestion`. For `index-suggestion`, pass table + column and `--query-log`.

```bash
dbscope explain risk <TABLE> --schema <URI>
dbscope explain index-suggestion <TABLE> <COLUMN> --schema <URI> --query-log <FILE>
```

---

## Policy

Optional YAML policy for `ci` and `preview`: `max_table_risk`, `no_cycles`, `no_orphans`, `max_blast_radius_percent`. Copy `dbscope.policy.example.yaml` to `dbscope.policy.yaml` and use `--policy dbscope.policy.yaml`.

---

## Risk Model

All scores are deterministic. When connected to Postgres, **operational weighting** is applied when available: risk uses `pg_stat_user_tables` (row counts, insert/update/delete activity) and optionally query-log frequency. **effective_risk = structural_risk × operational_weight** (0.2–1.0). Reports and CI use effective risk when present. Full spec: **[docs/risk_model.md](docs/risk_model.md)**.

**Table risk:** `risk = depth (max 0.4) + cycle (0.3 if in FK cycle) + centrality (max 0.3)`

**Impact (blast radius):** `impact = 0.4×FK reach + 0.3×index coupling + 0.3×query usage weight`

Scores 0–1. Levels: **Low**, **Moderate**, **High**, **Critical**.

---

## Reports

- Markdown summary  
- Static HTML report  
- JSON export  
- Graphviz dependency graph  

All offline. No external services.

---

## Architecture

dbscope builds a canonical relational graph: tables, columns, indexes, constraints, foreign keys, and (optionally) queries. Connectors normalize database metadata into this model. All analysis runs on the graph.

**Performance:** Sub-ms in-memory for typical schemas. End-to-end cost is DB metadata extraction. Run `cargo bench` for benchmarks.

---

## Philosophy

- Read-only  
- Deterministic  
- Explainable  
- CLI-first  
- Offline  
- No telemetry  

dbscope does not modify your database.

---

## License

MIT OR Apache-2.0
