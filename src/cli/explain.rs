//! Phase 5: Explain risk or index suggestion in plain language (rule-based, no AI).

use std::path::Path;

use crate::analysis::{self, TableRisk};
use crate::connectors::{self, query_log};
use crate::core;

pub async fn run_explain(
    kind: &str,
    target: &str,
    column: Option<&str>,
    schema_uri: &str,
    query_log_path: Option<&Path>,
) -> Result<(), anyhow::Error> {
    match kind.to_lowercase().as_str() {
        "risk" => explain_risk(target, schema_uri).await,
        "index-suggestion" | "index" => {
            let col = column.ok_or_else(|| anyhow::anyhow!("explain index-suggestion requires target and column (e.g. public.posts user_id)"))?;
            explain_index_suggestion(target, col, schema_uri, query_log_path).await
        }
        _ => anyhow::bail!("Unknown explain kind: {}. Use 'risk' or 'index-suggestion'.", kind),
    }
}

async fn explain_risk(table_target: &str, schema_uri: &str) -> Result<(), anyhow::Error> {
    let raw: core::RawSchema = connectors::extract_schema(schema_uri).await?;
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics(&graph);

    let normalized = if table_target.contains('.') {
        table_target.to_string()
    } else {
        format!("public.{}", table_target)
    };
    let m = metrics
        .iter()
        .find(|x| x.qualified_name == normalized || x.qualified_name == table_target)
        .or_else(|| metrics.iter().find(|x| x.qualified_name.ends_with(table_target)))
        .ok_or_else(|| anyhow::anyhow!("Table not found: {}", table_target))?;

    let risk = TableRisk::from_score(m.risk_score);
    println!("Table: {}", m.qualified_name);
    println!("Risk score: {:.2} ({})", m.risk_score, risk.label());
    println!();
    if let Some(ref b) = m.risk_breakdown {
        println!("How this score is computed:");
        println!("  FK depth contribution:   {:.2} (depth in + depth out, max 0.4)", b.depth_contrib);
        println!("  Cycle contribution:      {:.2} (0.3 if in a circular FK dependency)", b.cycle_contrib);
        println!("  Centrality contribution: {:.2} (in/out degree, max 0.3)", b.centrality_contrib);
        println!("  Formula: {}", b.formula);
    } else if m.is_orphan {
        println!("This table is an orphan (no foreign keys in or out), so risk = 0.");
    }
    println!();
    println!("  Centrality: {} tables reference this, this table references {} tables.", m.centrality_in, m.centrality_out);
    println!("  FK depth: {} (out), {} (in). In cycle: {}.", m.fk_depth_out, m.fk_depth_in, m.in_cycle);

    Ok(())
}

async fn explain_index_suggestion(
    qualified_table: &str,
    column_name: &str,
    schema_uri: &str,
    query_log_path: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let raw: core::RawSchema = connectors::extract_schema(schema_uri).await?;
    let queries = query_log_path
        .map(|p| query_log::read_query_log(p))
        .transpose()?
        .unwrap_or_default();
    let (usage, parsed_count) = analysis::build_usage_from_queries(&queries);
    let report = analysis::compute_usage_report(&raw, &usage, parsed_count);

    let normalized_table = if qualified_table.contains('.') {
        qualified_table.to_string()
    } else {
        format!("public.{}", qualified_table)
    };
    let s = report
        .index_suggestions
        .iter()
        .find(|s| s.qualified_table == normalized_table && s.column_name == column_name)
        .ok_or_else(|| anyhow::anyhow!("No index suggestion found for {}.{} (or query log not provided).", normalized_table, column_name))?;

    println!("Index suggestion: {}.{}", s.qualified_table, s.column_name);
    println!();
    println!("Reason: This column appears in WHERE clauses {} time(s) in the query log, but there is no index on it.", s.in_where_count);
    println!("Adding an index on {} could improve query performance for those filters.", s.column_name);
    println!();
    println!("(Based on {} parsed queries.)", parsed_count);

    Ok(())
}
