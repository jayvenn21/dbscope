//! Change simulation: preview migration impact (structural delta, risk delta, blast radius).

use std::collections::HashSet;
use std::path::Path;

use crate::analysis::{self, ImpactTarget};
use crate::connectors;
use crate::core;
use crate::migration;
use crate::policy;
use crate::query_parser;

pub async fn run_preview(
    schema_uri: &str,
    migration_path: &Path,
    query_log_path: Option<&Path>,
    policy_path: Option<&Path>,
) -> Result<(), anyhow::Error> {
    let raw_before = connectors::extract_schema(schema_uri).await?;
    let sql = std::fs::read_to_string(migration_path)?;
    let stmts = migration::parse_migration_sql(&sql);
    let raw_after = migration::apply_migration_to_schema(&raw_before, &stmts);

    let graph_before = core::DatabaseGraph::from_raw_schema(raw_before.clone());
    let graph_after = core::DatabaseGraph::from_raw_schema(raw_after.clone());
    let metrics_before = analysis::compute_all_metrics_with_operational(
        &graph_before,
        Some(&raw_before),
        None,
    );
    let metrics_after = analysis::compute_all_metrics_with_operational(
        &graph_after,
        Some(&raw_after),
        None,
    );

    let before_tables: HashSet<String> = raw_before
        .tables
        .iter()
        .map(|t| format!("{}.{}", t.schema_name, t.table_name))
        .collect();
    let after_tables: HashSet<String> = raw_after
        .tables
        .iter()
        .map(|t| format!("{}.{}", t.schema_name, t.table_name))
        .collect();
    let removed: Vec<String> = before_tables.difference(&after_tables).cloned().collect();

    let tables_removed = removed.len();
    let before_fks = raw_before.foreign_keys.len();
    let after_fks = raw_after.foreign_keys.len();
    let fks_removed = before_fks.saturating_sub(after_fks);

    let cycles_before = metrics_before.iter().filter(|m| m.in_cycle).count();
    let cycles_after = metrics_after.iter().filter(|m| m.in_cycle).count();
    let new_cycles = cycles_after.saturating_sub(cycles_before);

    let risk_for = |m: &analysis::TableMetrics| m.effective_risk.unwrap_or(m.risk_score);
    let max_risk_before = metrics_before.iter().map(risk_for).fold(0.0_f64, f64::max);
    let max_risk_after = metrics_after.iter().map(risk_for).fold(0.0_f64, f64::max);
    let risk_delta = max_risk_after - max_risk_before;

    // Blast radius: removed ∪ downstream of each removed (from before graph)
    let mut impacted: HashSet<String> = removed.iter().cloned().collect();
    for table_name in &removed {
        if let Some(target) = ImpactTarget::parse(table_name) {
            if let Some(report) = analysis::compute_impact(&target, &graph_before, &raw_before, None) {
                for t in &report.fk_downstream_tables {
                    impacted.insert(t.clone());
                }
            }
        }
    }
    let blast_radius_percent = if before_tables.is_empty() {
        0.0
    } else {
        impacted.len() as f64 / before_tables.len() as f64 * 100.0
    };

    let queries_broken = if let Some(path) = query_log_path {
        let content = std::fs::read_to_string(path)?;
        let queries: Vec<String> = content.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let removed_set: HashSet<&str> = removed.iter().map(|s| s.as_str()).collect();
        let mut broken = 0;
        for q in &queries {
            if let Some(parsed) = query_parser::parse_sql(q) {
                let refs: HashSet<String> = parsed.tables.iter().map(|t| t.qualified_name()).collect();
                if refs.iter().any(|r| removed_set.contains(r.as_str())) {
                    broken += 1;
                }
            }
        }
        Some(broken)
    } else {
        None
    };

    eprintln!("dbscope preview {}", migration_path.display());
    eprintln!();
    eprintln!("Change Summary:");
    eprintln!("  - Tables removed: {}", tables_removed);
    eprintln!("  - FKs removed: {}", fks_removed);
    eprintln!("  - New cycles: {} {}", new_cycles, if new_cycles > 0 { "(critical)" } else { "" });
    eprintln!("  - Risk delta: {:+.2}", risk_delta);
    eprintln!();
    eprintln!("Blast Radius:");
    eprintln!("  - {}% of schema graph impacted ({} of {} tables)", blast_radius_percent.round(), impacted.len(), before_tables.len());
    if let Some(n) = queries_broken {
        eprintln!("  - {} observed query/queries broken", n);
    }
    eprintln!();

    let pol = policy_path.map(|p| policy::Policy::load(p)).unwrap_or_default();
    let mut fail = false;
    if max_risk_after > pol.max_table_risk {
        eprintln!("Policy:");
        eprintln!("  ❌ FAIL: max table risk {:.2} exceeds threshold {:.2}", max_risk_after, pol.max_table_risk);
        fail = true;
    }
    if pol.no_cycles && cycles_after > 0 {
        eprintln!("Policy:");
        eprintln!("  ❌ FAIL: schema has {} table(s) in cycles (no_cycles: true)", cycles_after);
        fail = true;
    }
    if pol.no_orphans {
        let orphans_after = metrics_after.iter().filter(|m| m.is_orphan).count();
        if orphans_after > 0 {
            eprintln!("Policy:");
            eprintln!("  ❌ FAIL: schema has {} orphan(s) (no_orphans: true)", orphans_after);
            fail = true;
        }
    }
    if blast_radius_percent > pol.max_blast_radius_percent {
        eprintln!("Policy:");
        eprintln!("  ❌ FAIL: blast radius {:.0}% exceeds max {:.0}%", blast_radius_percent, pol.max_blast_radius_percent);
        fail = true;
    }
    if !fail && (policy_path.is_some() || pol.max_table_risk < 1.0) {
        eprintln!("Policy:");
        eprintln!("  ✅ PASS: within policy limits");
    }

    if fail {
        std::process::exit(1);
    }
    Ok(())
}
