//! JSON report: overview, table metrics, optional usage summary.
//! JSON report output.

use std::io::Write;

use crate::analysis::{TableMetrics, TableRisk, UsageReport};

/// Root structure for dbscope-report.json.
#[derive(serde::Serialize)]
pub struct JsonReport {
    pub overview: Overview,
    pub table_metrics: Vec<TableMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageSummary>,
}

#[derive(serde::Serialize)]
pub struct Overview {
    pub total_tables: usize,
    pub total_columns: usize,
    pub total_indexes: usize,
    pub total_foreign_keys: usize,
    pub overall_risk_score: f64,
    pub schema_complexity_score: f64,
    pub critical_risk_count: usize,
    pub high_risk_count: usize,
}

#[derive(serde::Serialize)]
pub struct UsageSummary {
    pub total_queries_parsed: usize,
    pub cold_tables: Vec<String>,
    pub cold_columns: Vec<ColdColumnRef>,
    pub hot_tables: Vec<HotTableRef>,
    pub index_suggestions: Vec<IndexSuggestionRef>,
    pub join_hotspots: Vec<JoinHotspotRef>,
}

#[derive(serde::Serialize)]
pub struct ColdColumnRef {
    pub qualified_table: String,
    pub column_name: String,
}

#[derive(serde::Serialize)]
pub struct HotTableRef {
    pub qualified_name: String,
    pub query_count: u64,
}

#[derive(serde::Serialize)]
pub struct IndexSuggestionRef {
    pub qualified_table: String,
    pub column_name: String,
    pub in_where_count: u64,
}

#[derive(serde::Serialize)]
pub struct JoinHotspotRef {
    pub table_a: String,
    pub table_b: String,
    pub join_count: u64,
}

/// Schema complexity 0-1: higher when more tables and FKs.
fn schema_complexity_score(total_tables: usize, total_fks: usize) -> f64 {
    if total_tables == 0 {
        return 0.0;
    }
    let n = total_tables as f64;
    let f = total_fks as f64;
    // Simple: more FKs and tables => higher. Cap at 1.
    let raw = (n * 0.02 + f * 0.05).min(1.0);
    (raw * 100.0).round() / 100.0
}

pub fn render<W: Write>(
    w: &mut W,
    metrics: &[TableMetrics],
    total_tables: usize,
    total_columns: usize,
    total_indexes: usize,
    total_fks: usize,
    usage: Option<&UsageReport>,
) -> std::io::Result<()> {
    let overall_risk = if metrics.is_empty() {
        0.0
    } else {
        metrics.iter().map(|m| m.display_risk()).sum::<f64>() / metrics.len() as f64
    };
    let critical = metrics
        .iter()
        .filter(|m| TableRisk::from_score(m.display_risk()) == TableRisk::Critical)
        .count();
    let high = metrics
        .iter()
        .filter(|m| TableRisk::from_score(m.display_risk()) == TableRisk::High)
        .count();

    let usage_summary = usage.map(|u| UsageSummary {
        total_queries_parsed: u.total_queries_parsed,
        cold_tables: u.cold_tables.iter().map(|t| t.0.clone()).collect(),
        cold_columns: u
            .cold_columns
            .iter()
            .map(|c| ColdColumnRef {
                qualified_table: c.qualified_table.clone(),
                column_name: c.column_name.clone(),
            })
            .collect(),
        hot_tables: u
            .hot_tables
            .iter()
            .map(|h| HotTableRef {
                qualified_name: h.qualified_name.clone(),
                query_count: h.query_count,
            })
            .collect(),
        index_suggestions: u
            .index_suggestions
            .iter()
            .map(|s| IndexSuggestionRef {
                qualified_table: s.qualified_table.clone(),
                column_name: s.column_name.clone(),
                in_where_count: s.in_where_count,
            })
            .collect(),
        join_hotspots: u
            .join_hotspots
            .iter()
            .map(|j| JoinHotspotRef {
                table_a: j.table_a.clone(),
                table_b: j.table_b.clone(),
                join_count: j.join_count,
            })
            .collect(),
    });

    let report = JsonReport {
        overview: Overview {
            total_tables,
            total_columns,
            total_indexes,
            total_foreign_keys: total_fks,
            overall_risk_score: (overall_risk * 100.0).round() / 100.0,
            schema_complexity_score: schema_complexity_score(total_tables, total_fks),
            critical_risk_count: critical,
            high_risk_count: high,
        },
        table_metrics: metrics.to_vec(),
        usage: usage_summary,
    };

    serde_json::to_writer_pretty(w, &report).map_err(std::io::Error::other)
}
