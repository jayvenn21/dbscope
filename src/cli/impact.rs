//! `dbscope impact <target>`: blast radius for a table or column.

use std::path::Path;

use crate::analysis;
use crate::connectors::{self, query_log};
use crate::core;

pub async fn run_impact(
    target_str: &str,
    schema_uri: &str,
    query_log_path: Option<&Path>,
    json_output: bool,
) -> Result<(), anyhow::Error> {
    let raw: core::RawSchema = connectors::extract_schema(schema_uri).await?;
    let default_schema = raw.default_schema();
    let target = match analysis::ImpactTarget::parse_with_default(target_str, &default_schema) {
        Some(t) => t,
        None => {
            anyhow::bail!(
                "Invalid target '{}'. Use: table (e.g. users), table.column (e.g. users.email), or schema.table.column",
                target_str
            );
        }
    };

    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());

    let queries_affected_count = if let Some(path) = query_log_path {
        let queries = query_log::read_query_log(path)?;
        Some(analysis::count_queries_affected(&queries, &target))
    } else {
        None
    };

    let report = match analysis::compute_impact(&target, &graph, &raw, queries_affected_count) {
        Some(r) => r,
        None => {
            anyhow::bail!(
                "Table '{}' not found in schema. Check schema and table name.",
                target.qualified_table()
            );
        }
    };

    let total_tables = graph.table_count();
    let affected_tables = 1 + report.fk_downstream_tables.len();
    let pct_graph = if total_tables > 0 {
        (affected_tables as f64 / total_tables as f64 * 100.0).round() as u32
    } else {
        0
    };

    if json_output {
        let json = serde_json::json!({
            "target": report.target.qualified_table(),
            "column": report.target.column,
            "risk_delta": report.risk_delta,
            "risk_level": impact_risk_label(report.risk_delta),
            "fk_downstream_tables": report.fk_downstream_tables,
            "fk_upstream_tables": report.fk_upstream_tables,
            "index_dependencies": report.index_dependencies,
            "queries_affected": report.queries_affected_count,
            "schema_impact_percent": pct_graph,
            "total_tables": total_tables,
            "breakdown": {
                "fk_downstream_contrib": report.risk_breakdown.fk_downstream_contrib,
                "index_contrib": report.risk_breakdown.index_contrib,
                "queries_contrib": report.risk_breakdown.queries_contrib,
                "formula": report.risk_breakdown.formula,
            }
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    let risk_label = impact_risk_label(report.risk_delta);

    let col_info = report
        .target
        .column
        .as_deref()
        .map(|c| format!(".{}", c))
        .unwrap_or_default();
    eprintln!(
        "dbscope impact {}{}",
        report.target.qualified_table(),
        col_info
    );
    eprintln!();
    eprintln!("  Dropping or changing this will:");
    eprintln!(
        "    - Affect {} table(s) (direct + transitive FK dependents)",
        report.fk_downstream_tables.len()
    );
    for t in &report.fk_downstream_tables {
        eprintln!("      - {}", t);
    }
    if let Some(n) = report.queries_affected_count {
        eprintln!("    - Break {} observed query/queries (from log)", n);
    }
    eprintln!(
        "    - Impact {}% of schema graph ({} of {} tables)",
        pct_graph, affected_tables, total_tables
    );
    eprintln!(
        "    - Risk level: {} ({:.2})",
        risk_label, report.risk_delta
    );
    eprintln!();
    eprintln!("  Breakdown:");
    eprintln!(
        "    FK reach (downstream):     {:.2}",
        report.risk_breakdown.fk_downstream_contrib
    );
    eprintln!(
        "    Index coupling:           {:.2}",
        report.risk_breakdown.index_contrib
    );
    eprintln!(
        "    Query usage weight:       {:.2}",
        report.risk_breakdown.queries_contrib
    );
    eprintln!();
    eprintln!(
        "  FK upstream (this table depends on): {}",
        report.fk_upstream_tables.len()
    );
    for t in &report.fk_upstream_tables {
        eprintln!("    - {}", t);
    }
    eprintln!(
        "  Index dependencies on target: {}",
        report.index_dependencies.len()
    );
    for i in &report.index_dependencies {
        eprintln!("    - {}", i);
    }

    Ok(())
}

fn impact_risk_label(score: f64) -> &'static str {
    if score >= 0.75 {
        "Critical"
    } else if score >= 0.5 {
        "High"
    } else if score >= 0.25 {
        "Moderate"
    } else {
        "Low"
    }
}
