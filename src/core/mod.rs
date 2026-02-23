//! Graph model and schema types. Connectors produce raw schema; this module
//! builds the unified graph and exposes it for analysis.

mod graph;
mod schema;

pub use graph::{DatabaseGraph, SchemaEdge, SchemaNode};
pub use schema::{
    ColumnMeta, ConstraintMeta, ForeignKeyRef, IndexMeta, RawSchema, TableMeta, TableStats,
};
