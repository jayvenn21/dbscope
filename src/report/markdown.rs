//! Markdown report renderer for schema analysis.

use std::io::Write;

use crate::analysis::{TableMetrics, TableRisk, UsageReport};

pub fn render<W: Write>(
    w: &mut W,
    metrics: &[TableMetrics],
    total_tables: usize,
    total_columns: usize,
    total_indexes: usize,
    total_fks: usize,
    usage: Option<&UsageReport>,
) -> std::io::Result<()> {
    writeln!(w, "# DBScope Schema Report\n")?;
    writeln!(w, "## Overview\n")?;
    writeln!(w, "- **Tables:** {}", total_tables)?;
    writeln!(w, "- **Columns:** {}", total_columns)?;
    writeln!(w, "- **Indexes:** {}", total_indexes)?;
    writeln!(w, "- **Foreign keys:** {}", total_fks)?;
    writeln!(w, "")?;

    writeln!(w, "## Risk Summary\n")?;
    let critical = metrics.iter().filter(|m| TableRisk::from_score(m.risk_score) == TableRisk::Critical).count();
    let high = metrics.iter().filter(|m| TableRisk::from_score(m.risk_score) == TableRisk::High).count();
    let medium = metrics.iter().filter(|m| TableRisk::from_score(m.risk_score) == TableRisk::Medium).count();
    let low = metrics.iter().filter(|m| TableRisk::from_score(m.risk_score) == TableRisk::Low).count();
    writeln!(w, "| Risk | Count |")?;
    writeln!(w, "|------|-------|")?;
    writeln!(w, "| Critical | {} |", critical)?;
    writeln!(w, "| High | {} |", high)?;
    writeln!(w, "| Medium | {} |", medium)?;
    writeln!(w, "| Low | {} |", low)?;
    writeln!(w, "")?;

    writeln!(w, "## Table Metrics (sortable by risk)\n")?;
    writeln!(w, "| Table | Centrality (in/out) | FK Depth (out/in) | Orphan | In cycle | Risk |")?;
    writeln!(w, "|-------|---------------------|-------------------|--------|----------|------|")?;
    let mut sorted: Vec<&TableMetrics> = metrics.iter().collect();
    sorted.sort_by(|a, b| b.risk_score.partial_cmp(&a.risk_score).unwrap_or(std::cmp::Ordering::Equal));
    for m in sorted {
        let risk = TableRisk::from_score(m.risk_score);
        writeln!(
            w,
            "| {} | {}/{} | {}/{} | {} | {} | {} |",
            m.qualified_name,
            m.centrality_in,
            m.centrality_out,
            m.fk_depth_out,
            m.fk_depth_in,
            if m.is_orphan { "yes" } else { "no" },
            if m.in_cycle { "yes" } else { "no" },
            risk.label(),
        )?;
    }
    writeln!(w, "")?;

    let orphans: Vec<_> = metrics.iter().filter(|m| m.is_orphan).collect();
    if !orphans.is_empty() {
        writeln!(w, "## Orphan Tables (no FK in or out)\n")?;
        for m in orphans {
            writeln!(w, "- {}", m.qualified_name)?;
        }
        writeln!(w, "")?;
    }

    let cycles: Vec<_> = metrics.iter().filter(|m| m.in_cycle).collect();
    if !cycles.is_empty() {
        writeln!(w, "## Tables in Circular Dependencies\n")?;
        for m in cycles {
            writeln!(w, "- {}", m.qualified_name)?;
        }
        writeln!(w, "")?;
    }

    if let Some(u) = usage {
        writeln!(w, "## Query log summary\n")?;
        writeln!(w, "- **Queries parsed:** {}\n", u.total_queries_parsed)?;
        if !u.cold_tables.is_empty() {
            writeln!(w, "## Cold tables (never queried)\n")?;
            for t in &u.cold_tables {
                writeln!(w, "- {}", t.0)?;
            }
            writeln!(w, "")?;
        }
        if !u.cold_columns.is_empty() {
            writeln!(w, "## Cold columns (never referenced)\n")?;
            for c in &u.cold_columns {
                writeln!(w, "- {}.{}", c.qualified_table, c.column_name)?;
            }
            writeln!(w, "")?;
        }
        if !u.hot_tables.is_empty() {
            writeln!(w, "## Hot tables (by query count)\n")?;
            for h in u.hot_tables.iter().take(20) {
                writeln!(w, "- {} ({})", h.qualified_name, h.query_count)?;
            }
            writeln!(w, "")?;
        }
        if !u.index_suggestions.is_empty() {
            writeln!(w, "## Index suggestions (column in WHERE, no index)\n")?;
            writeln!(w, "| Table | Column | WHERE count |")?;
            writeln!(w, "|-------|--------|-------------|")?;
            for s in u.index_suggestions.iter().take(30) {
                writeln!(w, "| {} | {} | {} |", s.qualified_table, s.column_name, s.in_where_count)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TableMetrics;

    fn one_table_metrics() -> Vec<TableMetrics> {
        vec![TableMetrics {
            qualified_name: "public.foo".into(),
            fk_depth_out: 0,
            fk_depth_in: 0,
            is_orphan: true,
            in_cycle: false,
            centrality_out: 0,
            centrality_in: 0,
            risk_score: 0.0,
            risk_breakdown: None,
        }]
    }

    #[test]
    fn markdown_contains_overview_and_table() {
        let mut buf = Vec::new();
        render(&mut buf, &one_table_metrics(), 1, 2, 0, 0, None).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("# DBScope Schema Report"));
        assert!(s.contains("## Overview"));
        assert!(s.contains("**Tables:** 1"));
        assert!(s.contains("**Columns:** 2"));
        assert!(s.contains("public.foo"));
        assert!(s.contains("Orphan"));
    }
}
