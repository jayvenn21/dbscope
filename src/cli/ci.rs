//! Phase 4: CI mode — run schema/migration risk check and fail if over threshold.

use std::path::Path;

use crate::analysis;
use crate::connectors;
use crate::core;
use crate::migration;

pub async fn run_ci(
    schema_uri: &str,
    migration_path: Option<&Path>,
    threshold: f64,
) -> Result<(), anyhow::Error> {
    let mut raw: core::RawSchema = connectors::extract_schema(schema_uri).await?;

    if let Some(path) = migration_path {
        let sql = std::fs::read_to_string(path)?;
        let stmts = migration::parse_migration_sql(&sql);
        raw = migration::apply_migration_to_schema(&raw, &stmts);
        eprintln!("dbscope ci: applied {} DDL statement(s) from {}", stmts.len(), path.display());
    }

    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics(&graph);

    let max_risk = metrics
        .iter()
        .map(|m| m.risk_score)
        .fold(0.0_f64, f64::max);
    let overall_risk = if metrics.is_empty() {
        0.0
    } else {
        metrics.iter().map(|m| m.risk_score).sum::<f64>() / metrics.len() as f64
    };

    eprintln!("dbscope ci: {} tables, max table risk = {:.2}, overall = {:.2}, threshold = {:.2}",
        metrics.len(), max_risk, overall_risk, threshold);

    if max_risk > threshold {
        let over: Vec<_> = metrics.iter().filter(|m| m.risk_score > threshold).collect();
        eprintln!("FAIL: {} table(s) exceed risk threshold {:.2}:", over.len(), threshold);
        for m in over {
            eprintln!("  - {} (risk {:.2})", m.qualified_name, m.risk_score);
        }
        std::process::exit(1);
    }

    eprintln!("PASS: no table risk above threshold.");
    Ok(())
}
