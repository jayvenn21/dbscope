//! Phase 2: cold/hot tables, cold columns, index suggestions from query usage.

use crate::core::RawSchema;
use crate::query_parser::{aggregate_queries, parse_sql, QueryUsage};

/// Tables that were never referenced in the query log.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(transparent)]
pub struct ColdTable(pub String);

/// Columns that were never referenced.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ColdColumn {
    pub qualified_table: String,
    pub column_name: String,
}

/// Table with high query count (for "hot" ranking).
#[derive(Debug, Clone, serde::Serialize)]
pub struct HotTable {
    pub qualified_name: String,
    pub query_count: u64,
}

/// Column frequently used in WHERE but not (adequately) indexed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexSuggestion {
    pub qualified_table: String,
    pub column_name: String,
    pub in_where_count: u64,
}

/// Join pair with frequency (for join hotspots).
#[derive(Debug, Clone, serde::Serialize)]
pub struct JoinHotspot {
    pub table_a: String,
    pub table_b: String,
    pub join_count: u64,
}

/// Result of Phase 2 usage analysis.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UsageReport {
    pub cold_tables: Vec<ColdTable>,
    pub cold_columns: Vec<ColdColumn>,
    pub hot_tables: Vec<HotTable>,
    pub index_suggestions: Vec<IndexSuggestion>,
    pub join_hotspots: Vec<JoinHotspot>,
    pub total_queries_parsed: usize,
}

/// Returns true if the table has an index that includes this column (any position).
fn column_has_index(raw: &RawSchema, schema: &str, table: &str, column: &str) -> bool {
    raw.indexes.iter().any(|idx| {
        idx.schema_name == schema && idx.table_name == table && idx.column_names.iter().any(|c| c == column)
    })
}

/// Build usage from query strings (e.g. log lines). Skips unparseable lines.
pub fn build_usage_from_queries(queries: &[String]) -> (QueryUsage, usize) {
    let parsed: Vec<_> = queries.iter().filter_map(|s| parse_sql(s)).collect();
    let count = parsed.len();
    let usage = aggregate_queries(&parsed);
    (usage, count)
}

/// Compute usage report from schema and query usage.
pub fn compute_usage_report(raw: &RawSchema, usage: &QueryUsage, total_queries_parsed: usize) -> UsageReport {
    let mut cold_tables = Vec::new();
    let mut cold_columns = Vec::new();
    let mut hot_tables: Vec<HotTable> = usage
        .table_hits
        .iter()
        .map(|(name, count)| HotTable {
            qualified_name: name.clone(),
            query_count: *count,
        })
        .collect();
    hot_tables.sort_by(|a, b| b.query_count.cmp(&a.query_count));

    for t in &raw.tables {
        let q = t.qualified_name();
        if usage.table_hits.get(&q).copied().unwrap_or(0) == 0 {
            cold_tables.push(ColdTable(q));
        }
    }

    for c in &raw.columns {
        let key = (c.schema_name.clone(), c.table_name.clone(), c.column_name.clone());
        let (ref_count, _) = usage.column_hits.get(&key).copied().unwrap_or((0, 0));
        if ref_count == 0 {
            cold_columns.push(ColdColumn {
                qualified_table: format!("{}.{}", c.schema_name, c.table_name),
                column_name: c.column_name.clone(),
            });
        }
    }

    let mut index_suggestions = Vec::new();
    for ((schema, table, column), (_ref_count, in_where)) in &usage.column_hits {
        if *in_where > 0 && !column_has_index(raw, schema, table, column) {
            index_suggestions.push(IndexSuggestion {
                qualified_table: format!("{}.{}", schema, table),
                column_name: column.to_string(),
                in_where_count: *in_where,
            });
        }
    }
    index_suggestions.sort_by(|a, b| b.in_where_count.cmp(&a.in_where_count));

    let mut join_hotspots: Vec<JoinHotspot> = usage
        .join_pairs
        .iter()
        .map(|((a, b), count)| JoinHotspot {
            table_a: a.clone(),
            table_b: b.clone(),
            join_count: *count,
        })
        .collect::<Vec<_>>();
    join_hotspots.sort_by(|a, b| b.join_count.cmp(&a.join_count));

    UsageReport {
        cold_tables,
        cold_columns,
        hot_tables,
        index_suggestions,
        join_hotspots,
        total_queries_parsed,
    }
}
