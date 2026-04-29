//! Parse SQL into tables, columns, and WHERE-columns for usage aggregation.

mod extract;

pub use extract::{
    aggregate_queries, parse_sql, parse_sql_with_dialect, ParsedQuery, QualifiedColumn,
    QualifiedTable, QueryUsage,
};
