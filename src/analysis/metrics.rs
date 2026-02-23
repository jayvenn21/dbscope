//! Compute FK dependency depth, orphan detection, cycles, centrality, risk score.

use petgraph::algo::is_cyclic_directed;
use petgraph::graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoNeighbors, Reversed};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::{DatabaseGraph, RawSchema, SchemaEdge, SchemaNode};
use super::usage::UsageReport;

/// Explainable breakdown of table risk score. Industry-credible: weighted components.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RiskScoreBreakdown {
    /// FK depth contribution (max 0.4): (depth_out + depth_in) / 20, capped.
    pub depth_contrib: f64,
    /// Cycle contribution (0 or 0.3): in a circular dependency.
    pub cycle_contrib: f64,
    /// Centrality contribution (max 0.3): (centrality_in + centrality_out) / 30, capped.
    pub centrality_contrib: f64,
    /// Human-readable formula for this table.
    pub formula: String,
}

/// Per-table metrics for reporting.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TableMetrics {
    pub qualified_name: String,
    pub fk_depth_out: u32,   // max path length following outgoing FKs
    pub fk_depth_in: u32,    // max path length following incoming FKs
    pub is_orphan: bool,     // no FK in and no FK out
    pub in_cycle: bool,
    pub centrality_out: u32, // number of tables this table references (out degree)
    pub centrality_in: u32,  // number of tables that reference this (in degree)
    pub risk_score: f64,
    /// How the risk score was computed (explainable scoring).
    pub risk_breakdown: Option<RiskScoreBreakdown>,
    /// Operational weight (0.2–1.0) when table_stats or query usage available. effective_risk = risk_score * this.
    pub operational_weight: Option<f64>,
    /// risk_score * operational_weight when operational weighting applied; else None.
    pub effective_risk: Option<f64>,
}

impl TableMetrics {
    /// Risk to use for display and policy (operational-weighted when available).
    pub fn display_risk(&self) -> f64 {
        self.effective_risk.unwrap_or(self.risk_score)
    }
}

/// Risk level for display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum TableRisk {
    Low,
    Medium,
    High,
    Critical,
}

impl TableRisk {
    pub fn from_score(score: f64) -> Self {
        if score >= 0.75 {
            TableRisk::Critical
        } else if score >= 0.5 {
            TableRisk::High
        } else if score >= 0.25 {
            TableRisk::Medium
        } else {
            TableRisk::Low
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TableRisk::Low => "Low",
            TableRisk::Medium => "Medium",
            TableRisk::High => "High",
            TableRisk::Critical => "Critical",
        }
    }
}

/// Build a subgraph of only table nodes and FK edges for depth/cycle/centrality.
fn fk_table_graph(graph: &DatabaseGraph) -> (petgraph::Graph<(), ()>, HashMap<NodeIndex, NodeIndex>) {
    use petgraph::graph::Graph;
    let mut g: Graph<(), ()> = Graph::new();
    let mut old_to_new: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    for &idx in &graph.table_node_list {
        let new_idx = g.add_node(());
        old_to_new.insert(idx, new_idx);
    }
    for edge in graph.graph.edge_references() {
        let w: &SchemaEdge = edge.weight();
        if matches!(w, SchemaEdge::ForeignKey { .. }) {
            if let (Some(&a), Some(&b)) = (old_to_new.get(&edge.source()), old_to_new.get(&edge.target())) {
                g.add_edge(a, b, ());
            }
        }
    }
    (g, old_to_new)
}

/// Max distance (depth) following outgoing FK edges from `start`, BFS.
fn max_depth_out(graph: &DatabaseGraph, start: NodeIndex) -> u32 {
    let (fk_graph, old_to_new) = fk_table_graph(graph);
    let start_new = match old_to_new.get(&start) {
        Some(&n) => n,
        None => return 0,
    };
    let mut dist: HashMap<petgraph::graph::NodeIndex, u32> = HashMap::new();
    dist.insert(start_new, 0);
    let mut queue = VecDeque::new();
    queue.push_back(start_new);
    while let Some(n) = queue.pop_front() {
        let d = dist[&n];
        for neighbor in fk_graph.neighbors(n) {
            if !dist.contains_key(&neighbor) {
                dist.insert(neighbor, d + 1);
                queue.push_back(neighbor);
            }
        }
    }
    dist.values().copied().max().unwrap_or(0)
}

/// Max distance following incoming FK edges from `start`, BFS on reversed graph.
fn max_depth_in(graph: &DatabaseGraph, start: NodeIndex) -> u32 {
    let (fk_graph, old_to_new) = fk_table_graph(graph);
    let start_new = match old_to_new.get(&start) {
        Some(&n) => n,
        None => return 0,
    };
    let rev = Reversed(&fk_graph);
    let mut dist: HashMap<petgraph::graph::NodeIndex, u32> = HashMap::new();
    dist.insert(start_new, 0);
    let mut queue = VecDeque::new();
    queue.push_back(start_new);
    while let Some(n) = queue.pop_front() {
        let d = dist[&n];
        for neighbor in rev.neighbors(n) {
            if !dist.contains_key(&neighbor) {
                dist.insert(neighbor, d + 1);
                queue.push_back(neighbor);
            }
        }
    }
    dist.values().copied().max().unwrap_or(0)
}

/// Find all table node indices that participate in a cycle (in the FK table graph).
fn tables_in_cycles(graph: &DatabaseGraph) -> HashSet<NodeIndex> {
    let (fk_graph, old_to_new) = fk_table_graph(graph);
    if !is_cyclic_directed(&fk_graph) {
        return HashSet::new();
    }
    let scc = petgraph::algo::kosaraju_scc(&fk_graph);
    let new_to_old: HashMap<NodeIndex, NodeIndex> = old_to_new.iter().map(|(k, v)| (*v, *k)).collect();
    let mut in_cycle = HashSet::new();
    for comp in scc {
        if comp.len() > 1 {
            for &new_idx in &comp {
                if let Some(&old_idx) = new_to_old.get(&new_idx) {
                    in_cycle.insert(old_idx);
                }
            }
        }
    }
    in_cycle
}

/// Compute centrality (in/out degree) for table nodes in the FK graph.
fn centrality(graph: &DatabaseGraph, table_idx: NodeIndex) -> (u32, u32) {
    let out = graph.fk_out_neighbors(table_idx).len() as u32;
    let inc = graph.fk_in_neighbors(table_idx).len() as u32;
    (inc, out)
}

/// Risk score in [0, 1]. Higher = riskier.
/// Factors: FK depth (more dependencies = more impact), cycle membership,
/// centrality (more connections = more impact), orphan (low risk).
/// Weights for table risk (explainable). Sum of contribs capped at 1.0.
const DEPTH_WEIGHT_CAP: f64 = 0.4;
const CYCLE_WEIGHT: f64 = 0.3;
const CENTRALITY_WEIGHT_CAP: f64 = 0.3;

fn risk_score_with_breakdown(
    fk_depth_out: u32,
    fk_depth_in: u32,
    in_cycle: bool,
    centrality_in: u32,
    centrality_out: u32,
    is_orphan: bool,
) -> (f64, Option<RiskScoreBreakdown>) {
    if is_orphan {
        return (
            0.0,
            Some(RiskScoreBreakdown {
                depth_contrib: 0.0,
                cycle_contrib: 0.0,
                centrality_contrib: 0.0,
                formula: "orphan (no FK in/out) → risk = 0".to_string(),
            }),
        );
    }
    let depth_contrib = ((fk_depth_out + fk_depth_in) as f64 / 20.0).min(DEPTH_WEIGHT_CAP);
    let cycle_contrib = if in_cycle { CYCLE_WEIGHT } else { 0.0 };
    let centrality_contrib = ((centrality_in + centrality_out) as f64 / 30.0).min(CENTRALITY_WEIGHT_CAP);
    let raw = (depth_contrib + cycle_contrib + centrality_contrib).min(1.0);
    let formula = format!(
        "risk = depth({:.2}) + cycle({:.2}) + centrality({:.2}) = {:.2}",
        depth_contrib, cycle_contrib, centrality_contrib, raw
    );
    let breakdown = RiskScoreBreakdown {
        depth_contrib,
        cycle_contrib,
        centrality_contrib,
        formula,
    };
    (raw, Some(breakdown))
}

fn compute_all_metrics_inner(
    graph: &DatabaseGraph,
    raw: Option<&RawSchema>,
    usage: Option<&UsageReport>,
) -> Vec<TableMetrics> {
    let in_cycle_set = tables_in_cycles(graph);
    let mut results = Vec::with_capacity(graph.table_count());
    for &table_idx in &graph.table_node_list {
        let qualified_name = match &graph.graph[table_idx] {
            SchemaNode::Table(t) => t.qualified_name(),
            _ => continue,
        };
        let fk_out = graph.fk_out_neighbors(table_idx).len();
        let fk_in = graph.fk_in_neighbors(table_idx).len();
        let is_orphan = fk_out == 0 && fk_in == 0;
        let fk_depth_out = max_depth_out(graph, table_idx);
        let fk_depth_in = max_depth_in(graph, table_idx);
        let in_cycle = in_cycle_set.contains(&table_idx);
        let (centrality_in, centrality_out) = centrality(graph, table_idx);
        let (risk_score, risk_breakdown) = risk_score_with_breakdown(
            fk_depth_out,
            fk_depth_in,
            in_cycle,
            centrality_in,
            centrality_out,
            is_orphan,
        );
        let (operational_weight, effective_risk) = compute_operational(
            &qualified_name,
            risk_score,
            raw,
            usage,
        );
        results.push(TableMetrics {
            qualified_name,
            fk_depth_out,
            fk_depth_in,
            is_orphan,
            in_cycle,
            centrality_out,
            centrality_in,
            risk_score,
            risk_breakdown,
            operational_weight,
            effective_risk,
        });
    }
    results
}

/// Operational weight from table_stats (row count, writes) and/or usage (query count). Returns (weight, effective_risk).
fn compute_operational(
    qualified_name: &str,
    structural_risk: f64,
    raw: Option<&RawSchema>,
    usage: Option<&UsageReport>,
) -> (Option<f64>, Option<f64>) {
    let stats = raw.and_then(|r| r.table_stats.as_ref());
    let row_estimate = stats.and_then(|s| {
        s.iter()
            .find(|t| format!("{}.{}", t.schema_name, t.table_name) == qualified_name)
    });
    let query_count = usage.and_then(|u| {
        u.hot_tables
            .iter()
            .find(|h| h.qualified_name == qualified_name)
            .map(|h| h.query_count)
    }).unwrap_or(0);

    let has_any = row_estimate.is_some() || usage.is_some();
    if !has_any {
        return (None, None);
    }

    let row_factor = row_estimate
        .map(|r| (r.row_estimate as f64 / 1_000_000.0).min(1.0))
        .unwrap_or(0.0);
    let write_factor = row_estimate
        .map(|r| {
            let w = r.n_tup_ins + r.n_tup_upd + r.n_tup_del;
            (w as f64 / 1_000_000.0).min(1.0)
        })
        .unwrap_or(0.0);
    let max_q = usage
        .map(|u| u.hot_tables.iter().map(|h| h.query_count).max().unwrap_or(1).max(1))
        .unwrap_or(1);
    let query_factor = (query_count as f64 / max_q as f64).min(1.0);

    let combined = (0.4 * row_factor + 0.3 * write_factor + 0.3 * query_factor).min(1.0);
    let weight = 0.2 + 0.8 * combined;
    let effective = (structural_risk * weight).min(1.0);
    (Some(weight), Some(effective))
}

/// Compute metrics for every table, with optional operational weighting (raw stats + query usage).
pub fn compute_all_metrics_with_operational(
    graph: &DatabaseGraph,
    raw: Option<&RawSchema>,
    usage: Option<&UsageReport>,
) -> Vec<TableMetrics> {
    compute_all_metrics_inner(graph, raw, usage)
}

/// Compute metrics (structural only). For operational weighting use compute_all_metrics_with_operational.
pub fn compute_all_metrics(graph: &DatabaseGraph) -> Vec<TableMetrics> {
    compute_all_metrics_inner(graph, None, None)
}

#[cfg(test)]
mod tests {
    use crate::analysis::{compute_all_metrics, TableRisk};
    use crate::core::{DatabaseGraph, ForeignKeyRef, RawSchema, TableMeta};

    fn fixture_raw_schema() -> RawSchema {
        RawSchema {
            tables: vec![
                TableMeta { schema_name: "public".into(), table_name: "users".into() },
                TableMeta { schema_name: "public".into(), table_name: "posts".into() },
                TableMeta { schema_name: "public".into(), table_name: "comments".into() },
                TableMeta { schema_name: "public".into(), table_name: "standalone".into() },
            ],
            views: vec![],
            materialized_views: vec![],
            columns: vec![],
            indexes: vec![],
            constraints: vec![],
            foreign_keys: vec![
                ForeignKeyRef {
                    name: "posts_user_id_fkey".into(),
                    from_schema: "public".into(),
                    from_table: "posts".into(),
                    from_columns: vec!["user_id".into()],
                    to_schema: "public".into(),
                    to_table: "users".into(),
                    to_columns: vec!["id".into()],
                },
                ForeignKeyRef {
                    name: "comments_post_id_fkey".into(),
                    from_schema: "public".into(),
                    from_table: "comments".into(),
                    from_columns: vec!["post_id".into()],
                    to_schema: "public".into(),
                    to_table: "posts".into(),
                    to_columns: vec!["id".into()],
                },
            ],
            table_stats: None,
            engine_metadata: None,
        }
    }

    #[test]
    fn metrics_orphan_has_zero_risk_and_flagged() {
        let graph = DatabaseGraph::from_raw_schema(fixture_raw_schema());
        let metrics = compute_all_metrics(&graph);
        let standalone = metrics.iter().find(|m| m.qualified_name == "public.standalone").unwrap();
        assert!(standalone.is_orphan);
        assert_eq!(standalone.risk_score, 0.0);
        assert!(!standalone.in_cycle);
    }

    #[test]
    fn metrics_fk_chain_has_depths_and_centrality() {
        let graph = DatabaseGraph::from_raw_schema(fixture_raw_schema());
        let metrics = compute_all_metrics(&graph);
        // comments -> posts -> users: comments has out-depth 2 (comments, posts, users)
        let comments = metrics.iter().find(|m| m.qualified_name == "public.comments").unwrap();
        assert_eq!(comments.fk_depth_out, 2);
        assert_eq!(comments.centrality_out, 1);
        assert_eq!(comments.centrality_in, 0);
        // users: only incoming FKs, so in-depth from users reaches posts and comments
        let users = metrics.iter().find(|m| m.qualified_name == "public.users").unwrap();
        assert_eq!(users.fk_depth_in, 2);
        assert_eq!(users.centrality_in, 1);
    }

    #[test]
    fn metrics_risk_levels() {
        let graph = DatabaseGraph::from_raw_schema(fixture_raw_schema());
        let metrics = compute_all_metrics(&graph);
        assert_eq!(metrics.len(), 4);
        let standalone = metrics.iter().find(|m| m.qualified_name == "public.standalone").unwrap();
        assert_eq!(TableRisk::from_score(standalone.risk_score), TableRisk::Low);
    }

    #[test]
    fn metrics_detect_cycle() {
        // Cycle: a -> b -> c -> a
        let raw = RawSchema {
            tables: vec![
                TableMeta { schema_name: "public".into(), table_name: "a".into() },
                TableMeta { schema_name: "public".into(), table_name: "b".into() },
                TableMeta { schema_name: "public".into(), table_name: "c".into() },
            ],
            views: vec![],
            materialized_views: vec![],
            columns: vec![],
            indexes: vec![],
            constraints: vec![],
            foreign_keys: vec![
                ForeignKeyRef {
                    name: "b_a".into(),
                    from_schema: "public".into(),
                    from_table: "b".into(),
                    from_columns: vec!["a_id".into()],
                    to_schema: "public".into(),
                    to_table: "a".into(),
                    to_columns: vec!["id".into()],
                },
                ForeignKeyRef {
                    name: "c_b".into(),
                    from_schema: "public".into(),
                    from_table: "c".into(),
                    from_columns: vec!["b_id".into()],
                    to_schema: "public".into(),
                    to_table: "b".into(),
                    to_columns: vec!["id".into()],
                },
                ForeignKeyRef {
                    name: "a_c".into(),
                    from_schema: "public".into(),
                    from_table: "a".into(),
                    from_columns: vec!["c_id".into()],
                    to_schema: "public".into(),
                    to_table: "c".into(),
                    to_columns: vec!["id".into()],
                },
            ],
            table_stats: None,
            engine_metadata: None,
        };
        let graph = DatabaseGraph::from_raw_schema(raw);
        let metrics = compute_all_metrics(&graph);
        for m in &metrics {
            assert!(m.in_cycle, "expected {} to be in cycle", m.qualified_name);
        }
    }
}
