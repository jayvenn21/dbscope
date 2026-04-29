//! CI mode: run schema/migration risk check and fail if over threshold or policy.

use std::path::Path;

use crate::analysis;
use crate::connectors;
use crate::core;
use crate::migration;
use crate::policy;

pub async fn run_ci(
    schema_uri: &str,
    migration_path: Option<&Path>,
    policy_path: Option<&Path>,
    threshold: f64,
) -> Result<(), anyhow::Error> {
    let mut raw: core::RawSchema = connectors::extract_schema(schema_uri).await?;

    if let Some(path) = migration_path {
        let sql = std::fs::read_to_string(path)?;
        let stmts = migration::parse_migration_sql(&sql);
        raw = migration::apply_migration_to_schema(&raw, &stmts);
        eprintln!(
            "dbscope ci: applied {} DDL statement(s) from {}",
            stmts.len(),
            path.display()
        );
    }

    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics_with_operational(&graph, Some(&raw), None);

    let risk_for = |m: &analysis::TableMetrics| m.effective_risk.unwrap_or(m.risk_score);
    let max_risk = metrics.iter().map(risk_for).fold(0.0_f64, f64::max);
    let overall_risk = if metrics.is_empty() {
        0.0
    } else {
        metrics.iter().map(risk_for).sum::<f64>() / metrics.len() as f64
    };

    let pol = policy_path.map(policy::Policy::load).unwrap_or_default();
    let effective_threshold = if policy_path.is_some() {
        pol.max_table_risk
    } else {
        threshold
    };

    eprintln!(
        "dbscope ci: {} tables, max table risk = {:.2}, overall = {:.2}, threshold = {:.2}",
        metrics.len(),
        max_risk,
        overall_risk,
        effective_threshold
    );

    let mut fail = false;
    if max_risk > effective_threshold {
        let over: Vec<_> = metrics
            .iter()
            .filter(|m| risk_for(m) > effective_threshold)
            .collect();
        eprintln!(
            "FAIL: {} table(s) exceed risk threshold {:.2}:",
            over.len(),
            effective_threshold
        );
        for m in over {
            eprintln!("  - {} (risk {:.2})", m.qualified_name, risk_for(m));
        }
        fail = true;
    }
    if pol.no_cycles {
        let in_cycle: Vec<_> = metrics.iter().filter(|m| m.in_cycle).collect();
        if !in_cycle.is_empty() {
            eprintln!(
                "FAIL: {} table(s) in cycles (no_cycles: true):",
                in_cycle.len()
            );
            for m in in_cycle.iter().take(5) {
                eprintln!("  - {}", m.qualified_name);
            }
            if in_cycle.len() > 5 {
                eprintln!("  ... and {} more", in_cycle.len() - 5);
            }
            fail = true;
        }
    }
    if pol.no_orphans {
        let orphans: Vec<_> = metrics.iter().filter(|m| m.is_orphan).collect();
        if !orphans.is_empty() {
            eprintln!(
                "FAIL: {} orphan table(s) (no_orphans: true):",
                orphans.len()
            );
            for m in orphans.iter().take(5) {
                eprintln!("  - {}", m.qualified_name);
            }
            if orphans.len() > 5 {
                eprintln!("  ... and {} more", orphans.len() - 5);
            }
            fail = true;
        }
    }

    if fail {
        anyhow::bail!("Schema risk check failed. See details above.");
    }
    eprintln!("PASS: no table risk above threshold.");
    Ok(())
}
