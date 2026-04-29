//! Universal relational schema model. Connectors normalize engine-specific catalogs
//! into this canonical shape. The core graph and analysis depend only on this model,
//! not engine-specific logic. Engine-specific features (e.g. Postgres partial index,
//! MySQL engine type, ClickHouse MergeTree) stay in the connector or in engine_metadata.
//!
//! One model that can represent any SQL engine.

use serde::{Deserialize, Serialize};

/// One table's metadata as extracted from the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMeta {
    pub schema_name: String,
    pub table_name: String,
    pub column_name: String,
    pub data_type: String,
    pub ordinal_position: i32,
    /// Whether the column accepts NULL values. None if the connector does not report this.
    #[serde(default)]
    pub is_nullable: Option<bool>,
    /// Default expression if any (e.g. "now()", "0"). None when no default or not reported.
    #[serde(default)]
    pub default_value: Option<String>,
}

/// One index's metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub schema_name: String,
    pub table_name: String,
    pub index_name: String,
    pub column_names: Vec<String>,
    pub is_unique: bool,
}

/// One constraint's metadata (PK, UNIQUE, CHECK; FK is separate).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintMeta {
    pub schema_name: String,
    pub table_name: String,
    pub constraint_name: String,
    pub constraint_type: String, // PRIMARY KEY, UNIQUE, CHECK
}

/// Foreign key relationship between two tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKeyRef {
    pub name: String,
    pub from_schema: String,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_schema: String,
    pub to_table: String,
    pub to_columns: Vec<String>,
}

/// Per-table operational stats (e.g. from pg_stat_user_tables). Used for operational weighting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStats {
    pub schema_name: String,
    pub table_name: String,
    pub row_estimate: u64,
    pub n_tup_ins: u64,
    pub n_tup_upd: u64,
    pub n_tup_del: u64,
}

/// Canonical database-agnostic schema: tables, views, materialized views, columns,
/// indexes, constraints, FKs. All connectors produce this shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Optional per-table stats (row counts, write activity). Postgres: pg_stat_user_tables.
    pub table_stats: Option<Vec<TableStats>>,
    /// Optional engine-specific data; core never reads this. Connectors may set it for tooling.
    pub engine_metadata: Option<serde_json::Value>,
}

impl RawSchema {
    /// Infer the default schema name from the extracted tables.
    /// Returns the most common schema_name, or "public" if no tables exist.
    pub fn default_schema(&self) -> String {
        let mut counts = std::collections::HashMap::<&str, usize>::new();
        for t in &self.tables {
            *counts.entry(&t.schema_name).or_insert(0) += 1;
        }
        counts
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(name, _)| name.to_string())
            .unwrap_or_else(|| "public".to_string())
    }
}

/// Alias for the universal model (same as RawSchema). Use when emphasizing engine-agnostic use.
#[allow(dead_code)]
pub type DatabaseModel = RawSchema;
