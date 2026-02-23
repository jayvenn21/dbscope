//! Graphviz .dot export of FK dependency graph (Phase 1 output).
//! Use: dot -Tsvg dbscope-graph.dot -o dbscope-graph.svg

use std::io::Write;

use petgraph::visit::EdgeRef;

use crate::analysis::{TableMetrics, TableRisk};
use crate::core::{DatabaseGraph, SchemaEdge, SchemaNode};

/// Escape a label for DOT (replace " and \ and newlines).
fn dot_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            _ => o.push(c),
        }
    }
    o
}

/// Write DOT to w. If metrics is Some, node labels include risk and we add color.
pub fn render<W: Write>(
    w: &mut W,
    graph: &DatabaseGraph,
    metrics: Option<&[TableMetrics]>,
) -> std::io::Result<()> {
    let risk_by_table: std::collections::HashMap<String, TableRisk> = metrics
        .map(|m| {
            m.iter()
                .map(|t| (t.qualified_name.clone(), TableRisk::from_score(t.risk_score)))
                .collect()
        })
        .unwrap_or_default();

    writeln!(w, "digraph dbscope {{")?;
    writeln!(w, "  rankdir=LR;")?;
    writeln!(w, "  node [shape=box, fontname=\"Helvetica\"];")?;

    for &idx in &graph.table_node_list {
        let name = match &graph.graph[idx] {
            SchemaNode::Table(t) => t.qualified_name(),
            _ => continue,
        };
        let (color, display_label) = risk_by_table
            .get(&name)
            .map(|r| {
                let (color, risk_label) = match r {
                    TableRisk::Critical => ("#f85149", "Critical"),
                    TableRisk::High => ("#db6d28", "High"),
                    TableRisk::Medium => ("#d29922", "Medium"),
                    TableRisk::Low => ("#3fb950", "Low"),
                };
                (color, format!("{} ({})", name, risk_label))
            })
            .unwrap_or(("#8b949e", name.clone()));

        writeln!(
            w,
            "  \"{}\" [label=\"{}\", style=filled, fillcolor=\"{}\", fontcolor=white];",
            dot_escape(&name),
            dot_escape(&display_label),
            color
        )?;
    }

    for edge in graph.graph.edge_references() {
        if !matches!(edge.weight(), SchemaEdge::ForeignKey { .. }) {
            continue;
        }
        let src = match &graph.graph[edge.source()] {
            SchemaNode::Table(t) => t.qualified_name(),
            _ => continue,
        };
        let dst = match &graph.graph[edge.target()] {
            SchemaNode::Table(t) => t.qualified_name(),
            _ => continue,
        };
        writeln!(w, "  \"{}\" -> \"{}\";", dot_escape(&src), dot_escape(&dst))?;
    }

    writeln!(w, "}}")?;
    Ok(())
}
