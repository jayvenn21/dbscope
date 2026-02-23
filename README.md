# dbscope

**Understand your database before you touch it.**

Universal relational schema intelligence for SQL databases. Read-only static + dynamic analysis. Deterministic risk scoring. Offline reports. No telemetry.

Supports: **PostgreSQL** (production), **MySQL** / **SQLite** / **ClickHouse** (connector interface).

---

## Why dbscope

Most tools manage migrations or monitor performance. dbscope analyzes structure.

It answers:

- What breaks if I drop this table?
- Which tables are central to my schema?
- Which columns are never queried?
- Where am I missing indexes (based on real queries)?
- What is my schema risk profile?

dbscope builds a unified relational graph and computes explainable risk metrics.

---

## Installation

**Build from source:**

```bash
cargo build --release
```

Binary: `./target/release/dbscope`

---

## Quick Start

```bash
export DBSCOPE_SCHEMA_URI="postgres://USER:PASS@localhost:5432/DBNAME"

dbscope analyze
dbscope summarize
dbscope impact public.users
```

Reports generated:

- `dbscope-report.md`
- `dbscope-report.html`
- `dbscope-report.json`
- `dbscope-graph.dot`

Use `-o DIR` to write reports to a directory.

---

## Commands

All commands accept `--schema URI` or the `DBSCOPE_SCHEMA_URI` environment variable. If the env is set, `--schema` can be omitted.

---

### analyze

Extract schema, build graph, compute metrics, generate reports.

```bash
dbscope analyze --schema <URI>
dbscope analyze --schema <URI> -o <DIR>              # write reports to directory
dbscope analyze --schema <URI> --query-log <FILE>    # add cold/hot tables, index suggestions
dbscope analyze --schema <URI> --query-log <FILE> -o <DIR>
```

| Option | Description |
|--------|-------------|
| `--schema` | Connection URI (Postgres, etc.). Required unless `DBSCOPE_SCHEMA_URI` is set. |
| `-o`, `--output` | Output directory for report files (default: current directory). |
| `--query-log` | Path to file with one SQL statement per line (cold/hot, index suggestions). |

---

### impact

Blast radius for a table or column: downstream FKs, upstream FKs, index coupling, affected queries, risk breakdown.

**Target** can be: `users`, `users.email`, or `public.users`, `public.users.email`.

```bash
dbscope impact <TARGET> --schema <URI>
dbscope impact <TARGET> --schema <URI> --query-log <FILE>
```

Examples:

```bash
dbscope impact users --schema postgres://...
dbscope impact public.users --schema postgres://...
dbscope impact users.email --schema postgres://...
dbscope impact public.posts --schema postgres://... --query-log queries.txt
```

| Option | Description |
|--------|-------------|
| `--schema` | Connection URI. Required unless `DBSCOPE_SCHEMA_URI` is set. |
| `--query-log` | Count queries that reference the target (from log file). |

---

### ci

Check schema (and optional migration) risk; exit 1 if any table risk exceeds threshold.

```bash
dbscope ci --schema <URI>
dbscope ci --schema <URI> --threshold <0-1>
dbscope ci --schema <URI> --migration <FILE> --threshold <0-1>
```

Exit code: **0** = pass, **1** = fail.

| Option | Description |
|--------|-------------|
| `--schema` | Connection URI. Required unless `DBSCOPE_SCHEMA_URI` is set. |
| `--migration` | DDL file to simulate (DROP TABLE, CREATE TABLE, ALTER TABLE ADD FK). |
| `--threshold` | Fail if any table risk &gt; this (0–1). Default: 0.5. |

---

### summarize

High-level architecture overview: table/column/FK counts, risk overview, orphans, cycles. With `--query-log`: cold tables, hot tables, index suggestions.

```bash
dbscope summarize --schema <URI>
dbscope summarize --schema <URI> --query-log <FILE>
```

| Option | Description |
|--------|-------------|
| `--schema` | Connection URI. Required unless `DBSCOPE_SCHEMA_URI` is set. |
| `--query-log` | Include cold/hot and index suggestions from query log. |

---

### explain

Explain risk scoring or index recommendations in plain language.

**KIND:** `risk` or `index-suggestion`.

- **risk** — `TARGET` = table (e.g. `public.users`). No column. No query log.
- **index-suggestion** — `TARGET` = qualified table, `COLUMN` = column name. Requires `--query-log` for suggestions.

```bash
dbscope explain risk <TABLE> --schema <URI>
dbscope explain index-suggestion <TABLE> <COLUMN> --schema <URI> --query-log <FILE>
```

Examples:

```bash
dbscope explain risk public.users --schema postgres://...
dbscope explain index-suggestion public.notifications read_at --schema postgres://... --query-log queries.txt
```

| Option | Description |
|--------|-------------|
| `--schema` | Connection URI. Required unless `DBSCOPE_SCHEMA_URI` is set. |
| `--query-log` | Required for `index-suggestion` (source of suggestions). |

---

## Risk Model

All scores are deterministic and documented in **[docs/risk_model.md](docs/risk_model.md)**.

**Table risk:**

```
risk = depth (max 0.4)
     + cycle (0.3 if in FK cycle)
     + centrality (max 0.3)
```

**Impact (blast radius):**

```
impact = 0.4 × FK reach
       + 0.3 × index coupling
       + 0.3 × query usage weight
```

Scores range from 0–1. Levels: **Low**, **Moderate**, **High**, **Critical**.

---

## Reports

dbscope generates:

- Markdown summary
- Static HTML report
- JSON data export
- Graphviz dependency graph

All reports are offline and require no external services.

---

## Architecture

dbscope builds a canonical relational graph: tables, columns, indexes, constraints, foreign keys, and (optionally) queries. All analysis runs on this unified graph. Connectors normalize database metadata into this model.

---

## Performance

Typical graph build + analysis: sub-millisecond for medium schemas (in-memory). End-to-end runtime is dominated by database metadata extraction.

Benchmarks: `cargo bench`

---

## Philosophy

- Read-only
- Deterministic
- Explainable
- CLI-first
- Offline
- No telemetry

dbscope does not modify your database.
