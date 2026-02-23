//! Unified database graph. Nodes: Table, Column, Index, Constraint.
//! Edges: FK, Table->Index, Table->Column, etc.

use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::HashMap;

use crate::core::schema::{
    ColumnMeta, ConstraintMeta, ForeignKeyRef, IndexMeta, RawSchema, TableMeta,
};

#[derive(Debug, Clone)]
pub enum SchemaNode {
    Table(TableMeta),
    Column(ColumnMeta),
    Index(IndexMeta),
    Constraint(ConstraintMeta),
}

#[derive(Debug, Clone)]
pub enum SchemaEdge {
    /// Table contains column
    TableToColumn,
    /// Table has index
    TableToIndex,
    /// Table has constraint
    TableToConstraint,
    /// Foreign key: from_table -> to_table
    ForeignKey { fk: ForeignKeyRef },
}

/// Unified graph over schema. Table nodes are the primary keys for analysis.
pub struct DatabaseGraph {
    pub graph: Graph<SchemaNode, SchemaEdge>,
    /// Table qualified name -> node index (for table nodes only)
    pub table_indices: HashMap<String, NodeIndex>,
    /// All table node indices in insertion order for stable iteration
    pub table_node_list: Vec<NodeIndex>,
}

impl DatabaseGraph {
    pub fn from_raw_schema(raw: RawSchema) -> Self {
        let mut g: Graph<SchemaNode, SchemaEdge> = Graph::new();
        let mut table_indices = HashMap::new();
        let mut table_node_list = Vec::new();

        // Create table nodes (base tables, views, materialized views) and column/index/constraint nodes + edges
        for t in raw.tables.iter().chain(raw.views.iter()).chain(raw.materialized_views.iter()) {
            let q = t.qualified_name();
            let idx = g.add_node(SchemaNode::Table(t.clone()));
            table_indices.insert(q.clone(), idx);
            table_node_list.push(idx);
        }

        for c in &raw.columns {
            let q = format!("{}.{}", c.schema_name, c.table_name);
            if let Some(&tidx) = table_indices.get(&q) {
                let cidx = g.add_node(SchemaNode::Column(c.clone()));
                g.add_edge(tidx, cidx, SchemaEdge::TableToColumn);
            }
        }

        for i in &raw.indexes {
            let q = format!("{}.{}", i.schema_name, i.table_name);
            if let Some(&tidx) = table_indices.get(&q) {
                let iidx = g.add_node(SchemaNode::Index(i.clone()));
                g.add_edge(tidx, iidx, SchemaEdge::TableToIndex);
            }
        }

        for c in &raw.constraints {
            let q = format!("{}.{}", c.schema_name, c.table_name);
            if let Some(&tidx) = table_indices.get(&q) {
                let cidx = g.add_node(SchemaNode::Constraint(c.clone()));
                g.add_edge(tidx, cidx, SchemaEdge::TableToConstraint);
            }
        }

        // FK edges: from_table -> to_table (table node to table node)
        for fk in &raw.foreign_keys {
            let from_q = format!("{}.{}", fk.from_schema, fk.from_table);
            let to_q = format!("{}.{}", fk.to_schema, fk.to_table);
            if let (Some(&from_idx), Some(&to_idx)) =
                (table_indices.get(&from_q), table_indices.get(&to_q))
            {
                g.add_edge(
                    from_idx,
                    to_idx,
                    SchemaEdge::ForeignKey {
                        fk: fk.clone(),
                    },
                );
            }
        }

        DatabaseGraph {
            graph: g,
            table_indices,
            table_node_list,
        }
    }

    /// Returns the table node index for a qualified table name, if present.
    pub fn table_index(&self, qualified_name: &str) -> Option<NodeIndex> {
        self.table_indices.get(qualified_name).copied()
    }

    /// Neighbors along outgoing FK edges only (this table references others).
    pub fn fk_out_neighbors(&self, table_idx: NodeIndex) -> Vec<NodeIndex> {
        self.graph
            .neighbors_directed(table_idx, Direction::Outgoing)
            .filter(|&n| {
                self.graph.find_edge(table_idx, n).map_or(false, |e| {
                    matches!(self.graph[e], SchemaEdge::ForeignKey { .. })
                })
            })
            .collect()
    }

    /// Neighbors along incoming FK edges only (others reference this table).
    pub fn fk_in_neighbors(&self, table_idx: NodeIndex) -> Vec<NodeIndex> {
        self.graph
            .neighbors_directed(table_idx, Direction::Incoming)
            .filter(|&n| {
                self.graph.find_edge(n, table_idx).map_or(false, |e| {
                    matches!(self.graph[e], SchemaEdge::ForeignKey { .. })
                })
            })
            .collect()
    }

    /// All FK edges from this table (outgoing). Returns (target NodeIndex, edge).
    pub fn fk_out_edges(&self, table_idx: NodeIndex) -> Vec<(NodeIndex, &SchemaEdge)> {
        self.graph
            .edges_directed(table_idx, Direction::Outgoing)
            .filter_map(|e| {
                let w = e.weight();
                match w {
                    SchemaEdge::ForeignKey { .. } => Some((e.target(), w)),
                    _ => None,
                }
            })
            .collect()
    }

    /// Table node count.
    pub fn table_count(&self) -> usize {
        self.table_indices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ForeignKeyRef, RawSchema, TableMeta};

    /// Fixture: public.users, public.posts, public.comments (chain), public.standalone (orphan).
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
            engine_metadata: None,
        }
    }

    #[test]
    fn graph_from_raw_schema_has_tables_and_fk_edges() {
        let raw = fixture_raw_schema();
        let graph = DatabaseGraph::from_raw_schema(raw);
        assert_eq!(graph.table_count(), 4);
        assert!(graph.table_index("public.users").is_some());
        assert!(graph.table_index("public.posts").is_some());
        assert!(graph.table_index("public.standalone").is_some());
        assert!(graph.table_index("public.nonexistent").is_none());

        let users_idx = graph.table_index("public.users").unwrap();
        let posts_idx = graph.table_index("public.posts").unwrap();
        let comments_idx = graph.table_index("public.comments").unwrap();
        let standalone_idx = graph.table_index("public.standalone").unwrap();

        // users: referenced by posts (in=1, out=0)
        assert_eq!(graph.fk_in_neighbors(users_idx).len(), 1);
        assert_eq!(graph.fk_out_neighbors(users_idx).len(), 0);
        // posts: references users, referenced by comments (in=1, out=1)
        assert_eq!(graph.fk_in_neighbors(posts_idx).len(), 1);
        assert_eq!(graph.fk_out_neighbors(posts_idx).len(), 1);
        // comments: references posts only (in=0, out=1)
        assert_eq!(graph.fk_in_neighbors(comments_idx).len(), 0);
        assert_eq!(graph.fk_out_neighbors(comments_idx).len(), 1);
        // standalone: orphan
        assert_eq!(graph.fk_in_neighbors(standalone_idx).len(), 0);
        assert_eq!(graph.fk_out_neighbors(standalone_idx).len(), 0);
    }
}
