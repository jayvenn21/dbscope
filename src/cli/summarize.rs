//! Phase 5: Summarize architecture — human-readable overview (no AI, rule-based).

use std::path::Path;

use crate::analysis::{self, TableRisk};
use crate::connectors::{self, query_log};
use crate::core;

pub async fn run_summarize(
    schema_uri: &str,
    query_log_path: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let raw: core::RawSchema = connectors::extract_schema(schema_uri).await?;
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics(&graph);

    let total_tables = metrics.len();
    let total_columns = raw.columns.len();
    let total_fks = raw.foreign_keys.len();
    let orphans: Vec<_> = metrics.iter().filter(|m| m.is_orphan).collect();
    let in_cycle: Vec<_> = metrics.iter().filter(|m| m.in_cycle).collect();
    let critical = metrics.iter().filter(|m| TableRisk::from_score(m.risk_score) == TableRisk::Critical).count();
    let high = metrics.iter().filter(|m| TableRisk::from_score(m.risk_score) == TableRisk::High).count();
    let overall_risk = if metrics.is_empty() {
        0.0
    } else {
        metrics.iter().map(|m| m.risk_score).sum::<f64>() / metrics.len() as f64
    };

    println!("## Schema summary");
    println!("  Tables: {}  Columns: {}  Foreign keys: {}", total_tables, total_columns, total_fks);
    println!("  Overall risk score: {:.2}  (Critical: {}, High: {})", overall_risk, critical, high);
    if !orphans.is_empty() {
        println!("  Orphan tables (no FK in/out): {}", orphans.len());
        for m in orphans.iter().take(10) {
            println!("    - {}", m.qualified_name);
        }
        if orphans.len() > 10 {
            println!("    ... and {} more", orphans.len() - 10);
        }
    }
    if !in_cycle.is_empty() {
        println!("  Tables in circular dependencies: {}", in_cycle.len());
        for m in in_cycle.iter().take(5) {
            println!("    - {}", m.qualified_name);
        }
    }

    if let Some(path) = query_log_path {
        let queries = query_log::read_query_log(path)?;
        let (usage, parsed_count) = analysis::build_usage_from_queries(&queries);
        let report = analysis::compute_usage_report(&raw, &usage, parsed_count);
        println!();
        println!("## Query log summary");
        println!("  Queries parsed: {}", parsed_count);
        println!("  Cold tables (never referenced): {}", report.cold_tables.len());
        for t in report.cold_tables.iter().take(5) {
            println!("    - {}", t.0);
        }
        println!("  Hot tables: {}", report.hot_tables.len());
        println!("  Index suggestions (column in WHERE, no index): {}", report.index_suggestions.len());
        for s in report.index_suggestions.iter().take(5) {
            println!("    - {}.{} (WHERE count: {})", s.qualified_table, s.column_name, s.in_where_count);
        }
    }

    Ok(())
}
