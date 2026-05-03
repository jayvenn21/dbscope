<h1 align="center">dbscope</h1>

<p align="center">
  Understand your database before you touch it.
</p>

<p align="center">
  <a href="https://github.com/jayvenn21/dbscope/releases"><img src="https://img.shields.io/github/v/release/jayvenn21/dbscope" alt="Release"></a>
  <a href="https://crates.io/crates/dbscope"><img src="https://img.shields.io/crates/v/dbscope" alt="crates.io"></a>
  <img src="https://img.shields.io/badge/postgres-supported-blue" alt="Postgres">
  <img src="https://img.shields.io/badge/mysql-supported-blue" alt="MySQL">
  <img src="https://img.shields.io/badge/sqlite-supported-blue" alt="SQLite">
  <img src="https://img.shields.io/badge/read--only-safe-success" alt="Read Only">
  <img src="https://img.shields.io/badge/risk-deterministic-purple" alt="Risk Model">
  <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-lightgrey" alt="License">
</p>

<p align="center">
  Read-only schema intelligence for SQL databases.<br>
  Graph-based risk scoring · Blast radius analysis · CI gating · Migration preview<br>
  Deterministic. Offline. No telemetry.
</p>

<p align="center">
  <a href="#the-problem">Problem</a> ·
  <a href="#install">Install</a> ·
  <a href="#example">Example</a> ·
  <a href="#commands">Commands</a> ·
  <a href="#risk-model">Risk Model</a> ·
  <a href="#how-dbscope-is-different">How It's Different</a> ·
  <a href="#ci-integration">CI</a> ·
  <a href="#architecture">Architecture</a>
</p>

---

## The Problem

Schema migrations cause production outages. Circular foreign key dependencies hide in plain sight. Orphan tables accumulate silently. Teams drop tables without understanding the blast radius. Nobody has a complete picture of their schema's structural health.

Migration linters (squawk, Atlas, pgfence) tell you if your SQL is dangerous. **dbscope tells you which tables you shouldn't touch in the first place.**

It connects to your database (read-only), builds a relational graph of every table, column, index, constraint, and foreign key, computes deterministic risk scores, and generates offline reports. Same schema, same query log, same scores - every time.

**Supports:** PostgreSQL · MySQL · SQLite · ClickHouse

---

## Install

```bash
cargo install dbscope
```

Or build from source:

```bash
git clone https://github.com/jayvenn21/dbscope.git
cd dbscope
cargo build --release
```

---

## Example

No database needed. Run the built-in demo to see what dbscope does:

```
$ dbscope demo

  Schema
    tables    17  (2 orphans: feature_flags, schema_migrations)
    columns   75
    indexes   13
    FKs       17  (self-ref: categories.parent_id)
    queries   23 analyzed

  Risk
    0 critical  0 high  0 medium  3 orphans

  Top Risk
    public.products                0.10 (Low)
    public.users                   0.10 (Low)
    public.orders                  0.10 (Low)
    public.reviews                 0.06 (Low)
    public.categories              0.05 (Low)

  Missing Indexes
    public.reviews.product_id  3 WHERE hits
    public.inventory.product_id  2 WHERE hits

  Cold Tables
    public.addresses
    public.wishlist_items
    public.coupons

  Reports
    ./dbscope-report.html  open in browser
    ./dbscope-report.json  machine-readable
    ./dbscope-graph.dot    render with: dot -Tsvg ... -o graph.svg
```

Point it at a real database:

```bash
export DBSCOPE_SCHEMA_URI="postgres://user:pass@localhost:5432/mydb"

dbscope analyze                          # full schema analysis + reports
dbscope impact public.users              # blast radius of a single table
dbscope impact public.users.email        # blast radius of a single column
dbscope ci --threshold 0.5               # fail if any table risk > 0.5
dbscope summarize                        # plain-language overview
dbscope plan drop public.legacy_orders   # safe drop plan with FK ordering
```

**Reports:** `dbscope-report.html`, `dbscope-report.json`, `dbscope-report.md`, `dbscope-graph.dot`

Use `-o DIR` to set output directory.

---

## How dbscope Is Different

| | Migration linters (squawk, Atlas, pgfence) | **dbscope** |
|---|---|---|
| **What it analyzes** | SQL migration files | Your live database schema |
| **How it works** | Pattern-matches dangerous DDL | Builds a relational graph, computes structural risk |
| **Risk model** | "This DDL acquires ACCESS EXCLUSIVE lock" | "This table has 0.82 risk because of FK depth, cycle membership, and centrality" |
| **Blast radius** | Which queries this migration blocks | Which tables, indexes, and queries are affected if you change this table |
| **Operational data** | Table size estimates | Real `pg_stat_user_tables` (row counts, insert/update/delete rates) |
| **CI gating** | Fail if DDL is risky | Fail if schema structure exceeds policy thresholds |
| **Multi-database** | PostgreSQL only (most) | PostgreSQL, MySQL, SQLite, ClickHouse |

**Use both.** dbscope for schema intelligence, squawk/Atlas for migration linting. They're complementary.

---

## Commands

All commands accept `--schema URI` or the `DBSCOPE_SCHEMA_URI` environment variable.

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
| `--query-log` | One SQL per line - enables cold/hot tables, index suggestions. |

### impact

Blast radius for a table or column: downstream/upstream FKs, index coupling, affected queries, risk breakdown.

**Target formats:** `users`, `users.email`, `public.users`, `public.users.email`

```bash
dbscope impact <TARGET> --schema <URI>
dbscope impact <TARGET> --schema <URI> --query-log <FILE>
```

### plan

Safe refactor plan for dropping a table: lists FKs to drop first, then the DROP TABLE step. Read-only - apply changes manually.

```bash
dbscope plan drop public.users --schema <URI>
```

### preview

Simulate a migration and report structural delta, risk delta, blast radius, and policy result.

```bash
dbscope preview <MIGRATION.sql> --schema <URI>
dbscope preview <MIGRATION.sql> --schema <URI> --query-log <FILE> --policy dbscope.policy.yaml
```

Output: tables removed, FKs removed, new cycles, risk delta, % of schema graph impacted, observed queries broken, then Policy PASS/FAIL.

| Option | Description |
|--------|-------------|
| `--query-log` | Count queries that reference removed tables. |
| `--policy` | YAML policy file. Exit 1 on violation. |

### ci

CI gate: exit 1 if schema risk exceeds threshold or policy. Optional `--migration` to simulate DDL first.

```bash
dbscope ci --schema <URI>
dbscope ci --schema <URI> --threshold 0.5 --migration <FILE>
dbscope ci --schema <URI> --policy dbscope.policy.yaml
```

| Option | Description |
|--------|-------------|
| `--threshold` | Fail if any table risk > this (0-1). Default: 0.5. Ignored if `--policy` is set. |
| `--migration` | DDL file to simulate before checking risk. |
| `--policy` | YAML policy: `max_table_risk`, `no_cycles`, `no_orphans`, `max_blast_radius_percent`. |

### summarize

Table/column/FK counts, risk overview, orphans, cycles. With `--query-log`: cold/hot tables, index suggestions.

```bash
dbscope summarize --schema <URI>
dbscope summarize --schema <URI> --query-log <FILE>
```

### explain

Explain a risk score or index recommendation in plain language.

**KIND:** `risk` or `index-suggestion`

```bash
dbscope explain risk <TABLE> --schema <URI>
dbscope explain index-suggestion <TABLE> <COLUMN> --schema <URI> --query-log <FILE>
```

---

## CI Integration

### GitHub Actions

```yaml
- name: Schema health check
  run: |
    cargo install dbscope
    dbscope ci --schema ${{ secrets.DATABASE_URL }} --policy dbscope.policy.yaml
```

### Policy File

Copy `dbscope.policy.example.yaml` to `dbscope.policy.yaml`:

```yaml
max_table_risk: 0.5
no_cycles: false
no_orphans: false
max_blast_radius_percent: 50
```

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | All checks passed |
| `1` | Risk or policy violation detected |
| `2` | Connection or parse error |

---

## Risk Model

All scores are deterministic - same schema + same query log = same scores. Full specification: **[docs/risk_model.md](docs/risk_model.md)**.

### Table Risk (structural)

```
risk = depth_contrib (max 0.4) + cycle_contrib (0.3 if in FK cycle) + centrality_contrib (max 0.3)
```

- **FK depth:** Max path length following outgoing and incoming FK edges.
- **Cycle:** 0.3 if the table participates in a circular FK dependency.
- **Centrality:** Number of direct FK neighbors (in-degree + out-degree).

### Operational Weighting (Postgres)

When connected to Postgres, risk is adjusted by live activity data from `pg_stat_user_tables`:

```
effective_risk = structural_risk × operational_weight (0.2-1.0)
```

### Impact (blast radius)

```
impact = 0.4 × FK_reach + 0.3 × index_coupling + 0.3 × query_usage_weight
```

| Score | Level | Meaning |
|-------|-------|---------|
| 0.75-1.0 | **Critical** | Very central, deep in FK chains, and/or in a cycle |
| 0.50-0.75 | **High** | Significant centrality or depth |
| 0.25-0.50 | **Moderate** | Some dependency depth or centrality |
| 0-0.25 | **Low** | Few dependencies, shallow in graph |

---

## Reports

Every `analyze` run generates four report formats:

- **Markdown** - Human-readable summary
- **HTML** - Static report with risk visualization
- **JSON** - Machine-readable for pipelines and dashboards
- **Graphviz** - Dependency graph (render with `dot -Tpng dbscope-graph.dot -o graph.png`)

All reports are generated offline. No external services.

---

## Architecture

dbscope builds a canonical relational graph from database metadata. Connectors normalize each engine's catalog into a universal `RawSchema` model. All analysis runs on this graph - the core never branches on database type.

```
Connector → RawSchema → DatabaseGraph → Analysis → Reports
   │                         │                │
   ├─ PostgresConnector      ├─ petgraph       ├─ risk metrics
   ├─ MysqlConnector         ├─ cycle detect   ├─ impact/blast radius
   ├─ SqliteConnector        ├─ centrality     ├─ usage analysis
   └─ ClickhouseConnector    └─ FK depth       └─ index suggestions
```

**Performance:** Sub-ms in-memory for typical schemas. End-to-end cost is dominated by DB metadata extraction. Run `cargo bench` for benchmarks.

Full architecture doc: **[docs/architecture.md](docs/architecture.md)**

---

## Philosophy

- **Read-only** - dbscope never modifies your database
- **Deterministic** - Same input, same output, every time
- **Explainable** - Every score has a documented formula
- **CLI-first** - Works in your terminal and CI pipeline
- **Offline** - No external services, no network calls
- **No telemetry** - Nothing leaves your machine

---

## Documentation

- **[Risk Model](docs/risk_model.md)** - Full scoring specification
- **[Architecture](docs/architecture.md)** - Universal model, connectors, pipeline
- **[Cloud](docs/cloud.md)** - dbscope Cloud product vision
- **[Positioning](docs/positioning.md)** - How dbscope compares to migration linters
- **[Contributing](CONTRIBUTING.md)** - Guidelines for contributors
- **[Changelog](CHANGELOG.md)** - Release history
- **[Security](SECURITY.md)** - Vulnerability reporting

---

## License

MIT OR Apache-2.0
