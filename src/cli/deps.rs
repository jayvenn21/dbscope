//! `dbscope deps <table>`: full dependency tree visualization.
//! Shows what depends on a table (downstream) and what it depends on (upstream)
//! as a human-readable tree.

use crate::core::{DatabaseGraph, FkGraph};
use petgraph::graph::NodeIndex;
use petgraph::visit::IntoNeighbors;
use std::collections::HashSet;

pub struct DepTree {
    pub target: String,
    pub downstream: Vec<DepNode>,
    pub upstream: Vec<DepNode>,
}

pub struct DepNode {
    pub name: String,
    pub depth: usize,
    pub children: Vec<DepNode>,
}

fn collect_downstream(
    graph: &DatabaseGraph,
    fk: &FkGraph,
    old_idx: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    depth: usize,
) -> Vec<DepNode> {
    let new_idx = match fk.old_to_new.get(&old_idx) {
        Some(&n) => n,
        None => return vec![],
    };
    let rev = petgraph::visit::Reversed(&fk.graph);
    let mut children = Vec::new();
    for neighbor in rev.neighbors(new_idx) {
        if let Some(&orig) = fk.new_to_old.get(&neighbor) {
            if visited.insert(orig) {
                let name = graph.table_name(orig).unwrap_or_default();
                let sub = collect_downstream(graph, fk, orig, visited, depth + 1);
                children.push(DepNode {
                    name,
                    depth: depth + 1,
                    children: sub,
                });
            }
        }
    }
    children
}

fn collect_upstream(
    graph: &DatabaseGraph,
    fk: &FkGraph,
    old_idx: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    depth: usize,
) -> Vec<DepNode> {
    let new_idx = match fk.old_to_new.get(&old_idx) {
        Some(&n) => n,
        None => return vec![],
    };
    let mut children = Vec::new();
    for neighbor in fk.graph.neighbors(new_idx) {
        if let Some(&orig) = fk.new_to_old.get(&neighbor) {
            if visited.insert(orig) {
                let name = graph.table_name(orig).unwrap_or_default();
                let sub = collect_upstream(graph, fk, orig, visited, depth + 1);
                children.push(DepNode {
                    name,
                    depth: depth + 1,
                    children: sub,
                });
            }
        }
    }
    children
}

pub fn build_dep_tree(graph: &DatabaseGraph, target: &str) -> Option<DepTree> {
    let table_idx = graph.table_index(target)?;
    let fk = graph.build_fk_graph();

    let mut visited_down = HashSet::new();
    visited_down.insert(table_idx);
    let downstream = collect_downstream(graph, &fk, table_idx, &mut visited_down, 0);

    let mut visited_up = HashSet::new();
    visited_up.insert(table_idx);
    let upstream = collect_upstream(graph, &fk, table_idx, &mut visited_up, 0);

    Some(DepTree {
        target: target.to_string(),
        downstream,
        upstream,
    })
}

fn print_tree(nodes: &[DepNode], is_last_vec: &[bool]) {
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == nodes.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let mut line_prefix = String::new();
        for &was_last in is_last_vec {
            line_prefix.push_str(if was_last { "    " } else { "│   " });
        }
        println!("{}{}{}", line_prefix, connector, node.name);
        let mut next_last = is_last_vec.to_vec();
        next_last.push(is_last);
        print_tree(&node.children, &next_last);
    }
}

pub fn run_deps(
    graph: &DatabaseGraph,
    target: &str,
    json_output: bool,
) -> Result<(), anyhow::Error> {
    let dep_tree = build_dep_tree(graph, target)
        .ok_or_else(|| anyhow::anyhow!("Table '{}' not found in schema", target))?;

    if json_output {
        let json = serde_json::json!({
            "target": dep_tree.target,
            "downstream": tree_to_json(&dep_tree.downstream),
            "upstream": tree_to_json(&dep_tree.upstream),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    let down_count = count_nodes(&dep_tree.downstream);
    let up_count = count_nodes(&dep_tree.upstream);

    println!("Dependency tree for: {}", dep_tree.target);
    println!(
        "  {} downstream (depend on this)  |  {} upstream (this depends on)\n",
        down_count, up_count
    );

    if !dep_tree.downstream.is_empty() {
        println!("  Downstream (what breaks if {} changes):", dep_tree.target);
        println!("  {}", dep_tree.target);
        print_tree(&dep_tree.downstream, &[]);
    }

    if !dep_tree.upstream.is_empty() {
        println!("\n  Upstream (what {} depends on):", dep_tree.target);
        println!("  {}", dep_tree.target);
        print_tree(&dep_tree.upstream, &[]);
    }

    if down_count == 0 && up_count == 0 {
        println!("  {} is an orphan, no FK dependencies.", dep_tree.target);
    }

    Ok(())
}

fn tree_to_json(nodes: &[DepNode]) -> serde_json::Value {
    serde_json::Value::Array(
        nodes
            .iter()
            .map(|n| {
                serde_json::json!({
                    "table": n.name,
                    "children": tree_to_json(&n.children),
                })
            })
            .collect(),
    )
}

fn count_nodes(nodes: &[DepNode]) -> usize {
    nodes.iter().map(|n| 1 + count_nodes(&n.children)).sum()
}
