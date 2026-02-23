//! Phase 3: Blast radius — impact of changing a table or column.

use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::visit::IntoNeighbors;

use crate::core::{DatabaseGraph, RawSchema, SchemaEdge, SchemaNode};
use crate::query_parser::parse_sql;

/// Parsed impact target: schema.table or schema.table.column (column optional).
#[derive(Debug, Clone)]
pub struct ImpactTarget {
    pub schema: String,
    pub table: String,
    pub column: Option<String>,
}

impl ImpactTarget {
    pub fn qualified_table(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }

    /// Parse "users", "users.email", "public.users", or "public.users.email". Default schema = "public".
    /// With two segments: "public.users" is schema.table; "users.email" is table.column.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let parts: Vec<&str> = s.split('.').collect();
        match parts.len() {
            1 => Some(ImpactTarget {
                schema: "public".to_string(),
                table: parts[0].to_string(),
                column: None,
            }),
            2 => {
                if parts[0].eq_ignore_ascii_case("public") {
                    Some(ImpactTarget {
                        schema: "public".to_string(),
                        table: parts[1].to_string(),
                        column: None,
                    })
                } else {
                    Some(ImpactTarget {
                        schema: "public".to_string(),
                        table: parts[0].to_string(),
                        column: Some(parts[1].to_string()),
                    })
                }
            }
            3 => Some(ImpactTarget {
                schema: parts[0].to_string(),
                table: parts[1].to_string(),
                column: Some(parts[2].to_string()),
            }),
            _ => None,
        }
    }
}

/// Explainable breakdown of impact (blast radius) score.
#[derive(Debug, Clone)]
pub struct ImpactRiskBreakdown {
    /// FK downstream contribution (weight 0.4): more dependent tables → higher.
    pub fk_downstream_contrib: f64,
    /// Index dependency contribution (weight 0.3): more indexes on target → higher.
    pub index_contrib: f64,
    /// Queries affected contribution (weight 0.3): more queries touch target → higher.
    pub queries_contrib: f64,
    /// Human-readable formula.
    pub formula: String,
}

/// Result of impact analysis: what is affected by changing the target.
#[derive(Debug, Clone)]
pub struct ImpactReport {
    pub target: ImpactTarget,
    /// Tables that depend on the target table via FK (who references us), including transitive.
    pub fk_downstream_tables: Vec<String>,
    /// Tables the target table references via FK (our dependencies).
    pub fk_upstream_tables: Vec<String>,
    /// Indexes on the target table that reference the target column (or all indexes if column not specified).
    pub index_dependencies: Vec<String>,
    /// Number of queries in the log that reference the target (when query log provided).
    pub queries_affected_count: Option<usize>,
    /// Simple impact score 0–1 (higher = larger blast radius).
    pub risk_delta: f64,
    /// How risk_delta was computed (explainable).
    pub risk_breakdown: ImpactRiskBreakdown,
}

/// Compute blast radius: FK downstream (tables that reference us, recursively).
fn fk_downstream(graph: &DatabaseGraph, table_idx: petgraph::graph::NodeIndex) -> Vec<String> {
    let (fk_graph, old_to_new) = fk_table_graph_for_impact(graph);
    let start_new = match old_to_new.get(&table_idx) {
        Some(&n) => n,
        None => return vec![],
    };
    // Downstream = follow reversed edges (incoming FKs): who points to us?
    let rev = petgraph::visit::Reversed(&fk_graph);
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(start_new);
    visited.insert(start_new);
    let new_to_old: HashMap<_, _> = old_to_new.iter().map(|(k, v)| (*v, *k)).collect();
    let mut out = Vec::new();
    while let Some(n) = queue.pop_front() {
        for neighbor in rev.neighbors(n) {
            if visited.insert(neighbor) {
                queue.push_back(neighbor);
                if let Some(&old_idx) = new_to_old.get(&neighbor) {
                    if let SchemaNode::Table(t) = &graph.graph[old_idx] {
                        out.push(t.qualified_name());
                    }
                }
            }
        }
    }
    out
}

/// FK upstream: tables we reference (one hop).
fn fk_upstream(graph: &DatabaseGraph, table_idx: petgraph::graph::NodeIndex) -> Vec<String> {
    graph
        .fk_out_neighbors(table_idx)
        .iter()
        .filter_map(|&idx| {
            if let SchemaNode::Table(t) = &graph.graph[idx] {
                Some(t.qualified_name())
            } else {
                None
            }
        })
        .collect()
}

/// Indexes on this table that mention the column (or all indexes if column is None).
fn index_dependencies(raw: &RawSchema, target: &ImpactTarget) -> Vec<String> {
    raw.indexes
        .iter()
        .filter(|idx| {
            idx.schema_name == target.schema
                && idx.table_name == target.table
                && (target.column.is_none()
                    || idx.column_names.iter().any(|c| c == target.column.as_ref().unwrap()))
        })
        .map(|idx| format!("{}.{}", idx.schema_name, idx.index_name))
        .collect()
}

/// Count queries in the log that reference the target table (and column if specified).
pub fn count_queries_affected(queries: &[String], target: &ImpactTarget) -> usize {
    let qualified_table = target.qualified_table();
    queries
        .iter()
        .filter(|sql| {
            let parsed = match parse_sql(sql) {
                Some(p) => p,
                None => return false,
            };
            if !parsed.tables.iter().any(|t| t.qualified_name() == qualified_table) {
                return false;
            }
            if let Some(ref col) = target.column {
                parsed.columns.iter().any(|c| c.column == *col && c.table == target.table && c.schema == target.schema)
                    || parsed.columns_in_where.iter().any(|c| c.column == *col && c.table == target.table && c.schema == target.schema)
            } else {
                true
            }
        })
        .count()
}

const IMPACT_FK_WEIGHT: f64 = 0.4;
const IMPACT_INDEX_WEIGHT: f64 = 0.3;
const IMPACT_QUERIES_WEIGHT: f64 = 0.3;

/// Compute impact score 0–1 and explainable breakdown.
fn impact_score(
    fk_downstream: usize,
    index_count: usize,
    queries_affected: Option<usize>,
) -> (f64, ImpactRiskBreakdown) {
    let fk_downstream_contrib = (fk_downstream as f64).min(20.0) / 20.0 * IMPACT_FK_WEIGHT;
    let index_contrib = (index_count as f64).min(10.0) / 10.0 * IMPACT_INDEX_WEIGHT;
    let queries_contrib = queries_affected
        .map(|n| (n as f64).min(50.0) / 50.0 * IMPACT_QUERIES_WEIGHT)
        .unwrap_or(0.0);
    let risk_delta = (fk_downstream_contrib + index_contrib + queries_contrib).min(1.0);
    let formula = format!(
        "risk_delta = 0.4×FK_downstream({:.2}) + 0.3×index_deps({:.2}) + 0.3×queries_affected({:.2}) = {:.2}",
        fk_downstream_contrib, index_contrib, queries_contrib, risk_delta
    );
    let risk_breakdown = ImpactRiskBreakdown {
        fk_downstream_contrib,
        index_contrib,
        queries_contrib,
        formula,
    };
    (risk_delta, risk_breakdown)
}

/// Build the FK table graph (table nodes + FK edges).
fn fk_table_graph_for_impact(
    graph: &DatabaseGraph,
) -> (petgraph::Graph<(), ()>, HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex>) {
    use petgraph::graph::Graph;
    use petgraph::visit::EdgeRef;

    let mut g: Graph<(), ()> = Graph::new();
    let mut old_to_new: HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex> = HashMap::new();
    for &idx in &graph.table_node_list {
        let new_idx = g.add_node(());
        old_to_new.insert(idx, new_idx);
    }
    for edge in graph.graph.edge_references() {
        let w = edge.weight();
        if matches!(w, SchemaEdge::ForeignKey { .. }) {
            if let (Some(&a), Some(&b)) = (old_to_new.get(&edge.source()), old_to_new.get(&edge.target())) {
                g.add_edge(a, b, ());
            }
        }
    }
    (g, old_to_new)
}

/// Run full impact analysis.
pub fn compute_impact(
    target: &ImpactTarget,
    graph: &DatabaseGraph,
    raw: &RawSchema,
    queries_affected_count: Option<usize>,
) -> Option<ImpactReport> {
    let table_idx = graph.table_index(&target.qualified_table())?;
    let fk_downstream_tables = fk_downstream(graph, table_idx);
    let fk_upstream_tables = fk_upstream(graph, table_idx);
    let index_dependencies = index_dependencies(raw, target);
    let (risk_delta, risk_breakdown) = impact_score(
        fk_downstream_tables.len(),
        index_dependencies.len(),
        queries_affected_count,
    );
    Some(ImpactReport {
        target: target.clone(),
        fk_downstream_tables,
        fk_upstream_tables,
        index_dependencies,
        queries_affected_count,
        risk_delta,
        risk_breakdown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{DatabaseGraph, ForeignKeyRef, RawSchema, TableMeta};

    fn fixture_raw_schema() -> RawSchema {
        RawSchema {
            tables: vec![
                TableMeta { schema_name: "public".into(), table_name: "users".into() },
                TableMeta { schema_name: "public".into(), table_name: "posts".into() },
                TableMeta { schema_name: "public".into(), table_name: "comments".into() },
            ],
            views: vec![],
            materialized_views: vec![],
            columns: vec![],
            indexes: vec![
                crate::core::IndexMeta {
                    schema_name: "public".into(),
                    table_name: "users".into(),
                    index_name: "users_pkey".into(),
                    column_names: vec!["id".into()],
                    is_unique: true,
                },
            ],
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
    fn impact_target_parse() {
        let t = ImpactTarget::parse("users").unwrap();
        assert_eq!(t.schema, "public");
        assert_eq!(t.table, "users");
        assert!(t.column.is_none());

        let t = ImpactTarget::parse("users.email").unwrap();
        assert_eq!(t.schema, "public");
        assert_eq!(t.table, "users");
        assert_eq!(t.column.as_deref(), Some("email"));

        let t = ImpactTarget::parse("public.users.email").unwrap();
        assert_eq!(t.schema, "public");
        assert_eq!(t.table, "users");
        assert_eq!(t.column.as_deref(), Some("email"));

        assert!(ImpactTarget::parse("a.b.c.d").is_none());
    }

    #[test]
    fn impact_fk_downstream_and_upstream() {
        let raw = fixture_raw_schema();
        let graph = DatabaseGraph::from_raw_schema(raw.clone());
        assert_eq!(graph.table_count(), 3);
        // users: downstream = posts, comments (both reference users directly or via posts)
        let target = ImpactTarget::parse("public.users").unwrap();
        assert_eq!(target.qualified_table(), "public.users");
        assert!(graph.table_index(&target.qualified_table()).is_some());
        let report = compute_impact(&target, &graph, &raw, None)
            .expect("public.users should be in graph");
        assert!(report.fk_downstream_tables.contains(&"public.posts".to_string()));
        assert!(report.fk_downstream_tables.contains(&"public.comments".to_string()));
        assert!(report.fk_upstream_tables.is_empty());

        // posts: downstream = comments; upstream = users
        let target = ImpactTarget::parse("public.posts").unwrap();
        let report = compute_impact(&target, &graph, &raw, None)
            .expect("public.posts should be in graph");
        assert!(report.fk_downstream_tables.contains(&"public.comments".to_string()));
        assert!(report.fk_upstream_tables.contains(&"public.users".to_string()));
    }

    #[test]
    fn impact_index_dependencies() {
        let raw = fixture_raw_schema();
        let graph = DatabaseGraph::from_raw_schema(raw.clone());
        let target = ImpactTarget::parse("public.users.id").unwrap();
        let report = compute_impact(&target, &graph, &raw, None).unwrap();
        assert!(report.index_dependencies.iter().any(|i| i.contains("users_pkey")));
    }
}
