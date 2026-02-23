//! Universal relational schema model. Connectors normalize engine-specific catalogs
//! into this canonical shape. The core graph and analysis depend only on this model—
//! no engine-specific logic. Engine-specific features (e.g. Postgres partial index,
//! MySQL engine type, ClickHouse MergeTree) stay in the connector or in engine_metadata.
//!
//! Path A: This is the "moat" — one model that can represent any SQL engine.

use serde::Serialize;

/// One table's metadata as extracted from the database.
#[derive(Debug, Clone, Serialize)]
pub struct TableMeta {
    pub schema_name: String,
    pub table_name: String,
}

impl TableMeta {
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema_name, self.table_name)
    }
}

/// One column's metadata.
#[derive(Debug, Clone, Serialize)]
pub struct ColumnMeta {
    pub schema_name: String,
    pub table_name: String,
    pub column_name: String,
    pub data_type: String,
    pub ordinal_position: i32,
}

/// One index's metadata.
#[derive(Debug, Clone, Serialize)]
pub struct IndexMeta {
    pub schema_name: String,
    pub table_name: String,
    pub index_name: String,
    pub column_names: Vec<String>,
    pub is_unique: bool,
}

/// One constraint's metadata (PK, UNIQUE, CHECK; FK is separate).
#[derive(Debug, Clone, Serialize)]
pub struct ConstraintMeta {
    pub schema_name: String,
    pub table_name: String,
    pub constraint_name: String,
    pub constraint_type: String, // PRIMARY KEY, UNIQUE, CHECK
}

/// Foreign key relationship between two tables.
#[derive(Debug, Clone, Serialize)]
pub struct ForeignKeyRef {
    pub name: String,
    pub from_schema: String,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_schema: String,
    pub to_table: String,
    pub to_columns: Vec<String>,
}

/// Canonical database-agnostic schema: tables, views, materialized views, columns,
/// indexes, constraints, FKs. All connectors produce this shape.
#[derive(Debug, Clone, Default)]
pub struct RawSchema {
    pub tables: Vec<TableMeta>,
    /// Views (logical); same shape as table for graph nodes. Columns may be empty if not extracted.
    pub views: Vec<TableMeta>,
    /// Materialized views; same shape as table. Columns may be empty if not extracted.
    pub materialized_views: Vec<TableMeta>,
    pub columns: Vec<ColumnMeta>,
    pub indexes: Vec<IndexMeta>,
    pub constraints: Vec<ConstraintMeta>,
    pub foreign_keys: Vec<ForeignKeyRef>,
    /// Optional engine-specific data; core never reads this. Connectors may set it for tooling.
    pub engine_metadata: Option<serde_json::Value>,
}

/// Alias for the universal model (same as RawSchema). Use when emphasizing engine-agnostic use.
#[allow(dead_code)]
pub type DatabaseModel = RawSchema;
