//! `dbscope analyze --schema <uri>`: extract schema, build graph, run analysis, emit reports.
//! Optional `--query-log <file>`: parse queries for cold/hot tables, index suggestions.

use std::path::Path;

use crate::analysis;
use crate::connectors::{self, query_log};
use crate::core;
use crate::report;

pub async fn run_analyze(
    schema_uri: &str,
    output_dir: Option<&Path>,
    query_log_path: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let raw: core::RawSchema = connectors::extract_schema(schema_uri).await?;
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());

    let usage_report = if let Some(path) = query_log_path {
        let queries = query_log::read_query_log(path)?;
        let (usage, parsed_count) = analysis::build_usage_from_queries(&queries);
        Some(analysis::compute_usage_report(&raw, &usage, parsed_count))
    } else {
        None
    };
    let metrics = analysis::compute_all_metrics_with_operational(
        &graph,
        Some(&raw),
        usage_report.as_ref(),
    );

    let total_tables = graph.table_count();
    let total_columns = raw.columns.len();
    let total_indexes = raw.indexes.len();
    let total_fks = raw.foreign_keys.len();

    // CLI summary
    eprintln!("dbscope analyze");
    eprintln!("  tables:   {}", total_tables);
    eprintln!("  columns:  {}", total_columns);
    eprintln!("  indexes:  {}", total_indexes);
    eprintln!("  FKs:      {}", total_fks);
    eprintln!("  metrics:  {} table(s)", metrics.len());
    let risk_for = |m: &analysis::TableMetrics| m.effective_risk.unwrap_or(m.risk_score);
    let critical = metrics.iter().filter(|m| risk_for(m) >= 0.75).count();
    let high = metrics.iter().filter(|m| { let r = risk_for(m); r >= 0.5 && r < 0.75 }).count();
    if critical > 0 || high > 0 {
        eprintln!("  risk:     {} critical, {} high", critical, high);
    }
    if let Some(ref u) = usage_report {
        eprintln!("  queries:  {} parsed (cold/hot + index suggestions in report)", u.total_queries_parsed);
    }

    let out = output_dir.unwrap_or(Path::new("."));
    if !out.exists() {
        std::fs::create_dir_all(out)?;
    }
    let md_path = out.join("dbscope-report.md");
    let html_path = out.join("dbscope-report.html");
    let json_path = out.join("dbscope-report.json");
    let dot_path = out.join("dbscope-graph.dot");

    let mut md_file = std::fs::File::create(&md_path)?;
    report::markdown::render(
        &mut md_file,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        usage_report.as_ref(),
    )?;
    eprintln!("  report:   {} (markdown)", md_path.display());

    let mut html_file = std::fs::File::create(&html_path)?;
    report::html::render(
        &mut html_file,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        usage_report.as_ref(),
    )?;
    eprintln!("  report:   {} (HTML)", html_path.display());

    let mut json_file = std::fs::File::create(&json_path)?;
    report::json::render(
        &mut json_file,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        usage_report.as_ref(),
    )?;
    eprintln!("  report:   {} (JSON)", json_path.display());

    let mut dot_file = std::fs::File::create(&dot_path)?;
    report::graphviz::render(&mut dot_file, &graph, Some(&metrics))?;
    eprintln!("  report:   {} (Graphviz)", dot_path.display());

    Ok(())
}
