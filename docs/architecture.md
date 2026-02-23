# Architecture: Universal model and connectors (Path A)

DBScope is a **database-agnostic relational graph intelligence engine**. Postgres, MySQL, SQLite, and ClickHouse are connectors; the core never branches on database type.

## Path A alignment

| Principle | Implementation |
|-----------|----------------|
| **Core = pure relational math** | Graph, metrics, impact, usage, and reports depend only on `RawSchema`. No engine names in `core/` or `analysis/`. |
| **Connector = normalization layer** | Each connector maps its catalog into the same canonical shape. Engine-specific features stay in the connector or in `engine_metadata`. |
| **One model, any SQL engine** | `RawSchema` (alias `DatabaseModel`) includes tables, views, materialized views, columns, indexes, constraints, FKs; optional `engine_metadata` for tooling. |

## Universal schema model

All connectors normalize their engine’s catalog into the same shape: **`RawSchema`** (in `src/core/schema.rs`).

- **Tables** – base tables (`schema_name`, `table_name`)
- **Views** – logical views (same shape as table; columns optional)
- **Materialized views** – same shape as table
- **Columns** – per-table/view with `data_type`, `ordinal_position`
- **Indexes** – name, table, column list, uniqueness
- **Constraints** – PRIMARY KEY, UNIQUE, CHECK (FKs are separate)
- **Foreign keys** – from/to (schema, table, columns)
- **engine_metadata** – optional; core never reads it. Connectors may set it for external tooling.

Engine-specific details (e.g. Postgres partial indexes, MySQL engine types) stay in the connector. The core never branches on database type.

## Connector trait

Connectors live in `src/connectors/`. Each implements:

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    fn name(&self) -> &'static str;
    async fn extract_schema(&self, connection_uri: &str) -> Result<RawSchema, ConnectorError>;
}
```

- **Postgres** – `postgres://` / `postgresql://` → `PostgresConnector` (information_schema + pg_catalog)
- **MySQL** – `mysql://` → `MysqlConnector` (information_schema: TABLES, COLUMNS, STATISTICS, TABLE_CONSTRAINTS, KEY_COLUMN_USAGE)
- **SQLite** – `sqlite://` / `file://` → `SqliteConnector` (sqlite_master + PRAGMA table_info/index_list/index_info/foreign_key_list; schema name `main`)
- **ClickHouse** – `clickhouse://` → `ClickhouseConnector` (system.tables, system.columns; no FKs; indexes from sorting_key/primary_key)

## Dispatch by URI

The CLI and library entry point is:

```rust
connectors::extract_schema(connection_uri).await
```

The scheme of the URI selects the connector (`postgres`, `postgresql`, etc.). Unsupported schemes return a clear `ConnectorError::UnsupportedScheme`.

## Pipeline

1. **Connector** → `RawSchema`
2. **Core** → `DatabaseGraph::from_raw_schema(raw)` (graph over tables, columns, indexes, FKs)
3. **Analysis** → metrics, impact, usage (query log) — all operate on the graph and raw schema
4. **Report** → markdown, HTML, JSON, Graphviz

No step after (1) knows which database was used. Adding a new engine means adding a connector that produces `RawSchema`; the rest of the stack is reused.

## What we avoid

- **No engine-specific logic in core or analysis.** Risk scoring, impact, and usage are pure relational math.
- **No per-database hacks in the graph.** Views and materialized views are first-class nodes (same as tables).
- **Single `extract_schema()` contract.** One round-trip, one normalized model; no split into `extract_indexes`/`extract_constraints` unless an engine requires it for streaming.
