//! MCP (Model Context Protocol) server over stdio.
//! Exposes dbscope analysis tools for AI assistants (Claude, Cursor, Copilot).
//!
//! Run with: `dbscope mcp`
//! Configure in MCP client: `{"command": "dbscope", "args": ["mcp"]}`

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

use crate::analysis::{self, ImpactTarget};
use crate::connectors;
use crate::core;

const SERVER_NAME: &str = "dbscope";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run_mcp() -> Result<(), anyhow::Error> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(json!({}));

        let response = match method {
            "initialize" => handle_initialize(id.clone()),
            "notifications/initialized" => continue,
            "tools/list" => handle_tools_list(id.clone()),
            "tools/call" => handle_tools_call(id.clone(), &params),
            "ping" => json_rpc_response(id.clone(), json!({})),
            _ => json_rpc_error(id.clone(), -32601, &format!("Method not found: {}", method)),
        };

        let out = serde_json::to_string(&response)?;
        writeln!(stdout, "{}", out)?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_initialize(id: Option<Value>) -> Value {
    json_rpc_response(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        }),
    )
}

fn handle_tools_list(id: Option<Value>) -> Value {
    json_rpc_response(
        id,
        json!({
            "tools": [
                {
                    "name": "analyze_schema",
                    "description": "Analyze a database schema: compute risk scores, detect cycles, find orphans, and return table metrics. Connects read-only to the database.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "uri": {
                                "type": "string",
                                "description": "Database connection URI (postgres://, mysql://, sqlite://, clickhouse://)"
                            }
                        },
                        "required": ["uri"]
                    }
                },
                {
                    "name": "explain_risk",
                    "description": "Explain why a specific table has its risk score. Returns the score breakdown: FK depth, cycle membership, centrality.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "uri": {
                                "type": "string",
                                "description": "Database connection URI"
                            },
                            "table": {
                                "type": "string",
                                "description": "Table name (e.g. 'users', 'public.users')"
                            }
                        },
                        "required": ["uri", "table"]
                    }
                },
                {
                    "name": "impact",
                    "description": "Compute blast radius: what tables, indexes, and queries are affected if you change a specific table or column.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "uri": {
                                "type": "string",
                                "description": "Database connection URI"
                            },
                            "target": {
                                "type": "string",
                                "description": "Target: table (e.g. 'users'), table.column (e.g. 'users.email'), or schema.table.column"
                            }
                        },
                        "required": ["uri", "target"]
                    }
                },
                {
                    "name": "lint_schema",
                    "description": "Detect schema anti-patterns: missing primary keys, wide tables, missing FK indexes, naming violations, nullable FKs, redundant indexes.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "uri": {
                                "type": "string",
                                "description": "Database connection URI"
                            }
                        },
                        "required": ["uri"]
                    }
                },
                {
                    "name": "deps",
                    "description": "Show the dependency tree for a table: what depends on it (downstream FK references) and what it depends on (upstream).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "uri": {
                                "type": "string",
                                "description": "Database connection URI"
                            },
                            "table": {
                                "type": "string",
                                "description": "Table name (e.g. 'users', 'public.orders')"
                            }
                        },
                        "required": ["uri", "table"]
                    }
                },
                {
                    "name": "diff_schemas",
                    "description": "Compare two schema snapshots and return structural differences (added/removed tables, columns, indexes, FKs).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "before_uri": {
                                "type": "string",
                                "description": "Connection URI or snapshot file path for the 'before' state"
                            },
                            "after_uri": {
                                "type": "string",
                                "description": "Connection URI or snapshot file path for the 'after' state"
                            }
                        },
                        "required": ["before_uri", "after_uri"]
                    }
                }
            ]
        }),
    )
}

fn handle_tools_call(id: Option<Value>, params: &Value) -> Value {
    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => return tool_error(id, &format!("Failed to create runtime: {}", e)),
    };

    match tool_name {
        "analyze_schema" => rt.block_on(tool_analyze(id.clone(), &arguments)),
        "explain_risk" => rt.block_on(tool_explain_risk(id.clone(), &arguments)),
        "impact" => rt.block_on(tool_impact(id.clone(), &arguments)),
        "lint_schema" => rt.block_on(tool_lint(id.clone(), &arguments)),
        "deps" => rt.block_on(tool_deps(id.clone(), &arguments)),
        "diff_schemas" => rt.block_on(tool_diff(id.clone(), &arguments)),
        _ => tool_error(id, &format!("Unknown tool: {}", tool_name)),
    }
}

async fn tool_analyze(id: Option<Value>, args: &Value) -> Value {
    let uri = match args.get("uri").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => return tool_error(id, "Missing required parameter: uri"),
    };

    let raw = match connectors::extract_schema(uri).await {
        Ok(r) => r,
        Err(e) => return tool_error(id, &format!("Connection failed: {}", e)),
    };

    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics_with_operational(&graph, Some(&raw), None);

    let tables: Vec<Value> = metrics
        .iter()
        .map(|m| {
            json!({
                "table": m.qualified_name,
                "risk_score": (m.effective_risk.unwrap_or(m.risk_score) * 100.0).round() / 100.0,
                "risk_level": analysis::TableRisk::from_score(m.effective_risk.unwrap_or(m.risk_score)).label(),
                "in_cycle": m.in_cycle,
                "is_orphan": m.is_orphan,
                "centrality_in": m.centrality_in,
                "centrality_out": m.centrality_out,
                "fk_depth_in": m.fk_depth_in,
                "fk_depth_out": m.fk_depth_out,
            })
        })
        .collect();

    let high_risk: Vec<&Value> = tables
        .iter()
        .filter(|t| t.get("risk_score").and_then(|s| s.as_f64()).unwrap_or(0.0) >= 0.5)
        .collect();

    let cycles: Vec<&str> = metrics
        .iter()
        .filter(|m| m.in_cycle)
        .map(|m| m.qualified_name.as_str())
        .collect();

    let orphans: Vec<&str> = metrics
        .iter()
        .filter(|m| m.is_orphan)
        .map(|m| m.qualified_name.as_str())
        .collect();

    let summary = format!(
        "{} tables analyzed. {} high-risk (>=0.5). {} in FK cycles. {} orphans.",
        tables.len(),
        high_risk.len(),
        cycles.len(),
        orphans.len()
    );

    tool_result(
        id,
        &json!({
            "summary": summary,
            "total_tables": tables.len(),
            "high_risk_count": high_risk.len(),
            "cycles": cycles,
            "orphans": orphans,
            "tables": tables,
        })
        .to_string(),
    )
}

async fn tool_explain_risk(id: Option<Value>, args: &Value) -> Value {
    let uri = match args.get("uri").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => return tool_error(id, "Missing required parameter: uri"),
    };
    let table_name = match args.get("table").and_then(|t| t.as_str()) {
        Some(t) => t,
        None => return tool_error(id, "Missing required parameter: table"),
    };

    let raw = match connectors::extract_schema(uri).await {
        Ok(r) => r,
        Err(e) => return tool_error(id, &format!("Connection failed: {}", e)),
    };

    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics_with_operational(&graph, Some(&raw), None);

    let normalized = if table_name.contains('.') {
        table_name.to_string()
    } else {
        format!("{}.{}", raw.default_schema(), table_name)
    };

    let m = match metrics
        .iter()
        .find(|x| x.qualified_name == normalized)
        .or_else(|| {
            metrics
                .iter()
                .find(|x| x.qualified_name.ends_with(table_name))
        }) {
        Some(m) => m,
        None => return tool_error(id, &format!("Table not found: {}", table_name)),
    };

    let score = m.effective_risk.unwrap_or(m.risk_score);
    let level = analysis::TableRisk::from_score(score).label();

    let explanation = if m.is_orphan {
        format!(
            "{} has a risk score of 0.00 ({}). It is an orphan table with no foreign key relationships.",
            m.qualified_name, level
        )
    } else {
        let mut parts = Vec::new();
        if let Some(ref b) = m.risk_breakdown {
            if b.depth_contrib > 0.0 {
                parts.push(format!(
                    "FK depth contributes {:.2} (deeper in FK chains = harder to change safely)",
                    b.depth_contrib
                ));
            }
            if b.cycle_contrib > 0.0 {
                parts.push("It participates in a circular FK dependency (+0.30 risk)".to_string());
            }
            if b.centrality_contrib > 0.0 {
                parts.push(format!(
                    "Centrality contributes {:.2} ({} tables reference it, it references {})",
                    b.centrality_contrib, m.centrality_in, m.centrality_out
                ));
            }
        }
        format!(
            "{} has a risk score of {:.2} ({}).\n{}",
            m.qualified_name,
            score,
            level,
            parts.join("\n")
        )
    };

    tool_result(
        id,
        &json!({
            "table": m.qualified_name,
            "risk_score": score,
            "risk_level": level,
            "in_cycle": m.in_cycle,
            "is_orphan": m.is_orphan,
            "explanation": explanation,
        })
        .to_string(),
    )
}

async fn tool_impact(id: Option<Value>, args: &Value) -> Value {
    let uri = match args.get("uri").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => return tool_error(id, "Missing required parameter: uri"),
    };
    let target_str = match args.get("target").and_then(|t| t.as_str()) {
        Some(t) => t,
        None => return tool_error(id, "Missing required parameter: target"),
    };

    let raw = match connectors::extract_schema(uri).await {
        Ok(r) => r,
        Err(e) => return tool_error(id, &format!("Connection failed: {}", e)),
    };

    let default_schema = raw.default_schema();
    let target = match ImpactTarget::parse_with_default(target_str, &default_schema) {
        Some(t) => t,
        None => return tool_error(id, &format!("Invalid target: {}", target_str)),
    };

    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let report = match analysis::compute_impact(&target, &graph, &raw, None) {
        Some(r) => r,
        None => {
            return tool_error(
                id,
                &format!("Table not found: {}", target.qualified_table()),
            )
        }
    };

    let total_tables = graph.table_count();
    let affected = 1 + report.fk_downstream_tables.len();
    let pct = if total_tables > 0 {
        (affected as f64 / total_tables as f64 * 100.0).round() as u32
    } else {
        0
    };

    tool_result(
        id,
        &json!({
            "target": target.qualified_table(),
            "column": target.column,
            "risk_delta": report.risk_delta,
            "downstream_tables": report.fk_downstream_tables,
            "upstream_tables": report.fk_upstream_tables,
            "index_dependencies": report.index_dependencies,
            "schema_impact_percent": pct,
            "summary": format!(
                "Changing {} affects {} table(s) ({}% of schema). Risk delta: {:.2}.",
                target.qualified_table(),
                report.fk_downstream_tables.len(),
                pct,
                report.risk_delta
            ),
        })
        .to_string(),
    )
}

async fn tool_lint(id: Option<Value>, args: &Value) -> Value {
    let uri = match args.get("uri").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => return tool_error(id, "Missing required parameter: uri"),
    };

    let raw = match connectors::extract_schema(uri).await {
        Ok(r) => r,
        Err(e) => return tool_error(id, &format!("Connection failed: {}", e)),
    };

    let violations = crate::cli::lint::lint_schema(&raw);

    let items: Vec<Value> = violations
        .iter()
        .map(|v| {
            json!({
                "rule": v.rule,
                "severity": format!("{}", v.severity),
                "table": v.table,
                "message": v.message,
                "suggestion": v.suggestion,
            })
        })
        .collect();

    let summary = if violations.is_empty() {
        "No lint violations found. Schema looks clean.".to_string()
    } else {
        format!(
            "{} lint violation(s) found across the schema.",
            violations.len()
        )
    };

    tool_result(
        id,
        &json!({
            "summary": summary,
            "violation_count": violations.len(),
            "violations": items,
        })
        .to_string(),
    )
}

async fn tool_deps(id: Option<Value>, args: &Value) -> Value {
    let uri = match args.get("uri").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => return tool_error(id, "Missing required parameter: uri"),
    };
    let table_name = match args.get("table").and_then(|t| t.as_str()) {
        Some(t) => t,
        None => return tool_error(id, "Missing required parameter: table"),
    };

    let raw = match connectors::extract_schema(uri).await {
        Ok(r) => r,
        Err(e) => return tool_error(id, &format!("Connection failed: {}", e)),
    };

    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let qualified = if table_name.contains('.') {
        table_name.to_string()
    } else {
        format!("{}.{}", raw.default_schema(), table_name)
    };

    let dep_tree = match crate::cli::deps::build_dep_tree(&graph, &qualified) {
        Some(t) => t,
        None => return tool_error(id, &format!("Table not found: {}", qualified)),
    };

    fn collect_names(nodes: &[crate::cli::deps::DepNode]) -> Vec<String> {
        let mut out = Vec::new();
        for n in nodes {
            out.push(n.name.clone());
            out.extend(collect_names(&n.children));
        }
        out
    }

    let downstream = collect_names(&dep_tree.downstream);
    let upstream = collect_names(&dep_tree.upstream);

    tool_result(
        id,
        &json!({
            "table": qualified,
            "downstream": downstream,
            "upstream": upstream,
            "summary": format!(
                "{} has {} downstream dependent(s) and {} upstream dependency/dependencies.",
                qualified,
                downstream.len(),
                upstream.len()
            ),
        })
        .to_string(),
    )
}

async fn tool_diff(id: Option<Value>, args: &Value) -> Value {
    let before_uri = match args.get("before_uri").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => return tool_error(id, "Missing required parameter: before_uri"),
    };
    let after_uri = match args.get("after_uri").and_then(|u| u.as_str()) {
        Some(u) => u,
        None => return tool_error(id, "Missing required parameter: after_uri"),
    };

    let rt_result: Result<(core::RawSchema, core::RawSchema), String> = async {
        let before = load_schema_or_snapshot(before_uri)
            .await
            .map_err(|e| format!("Failed to load 'before': {}", e))?;
        let after = load_schema_or_snapshot(after_uri)
            .await
            .map_err(|e| format!("Failed to load 'after': {}", e))?;
        Ok((before, after))
    }
    .await;

    let (before, after) = match rt_result {
        Ok(pair) => pair,
        Err(e) => return tool_error(id, &e),
    };

    let diff = crate::cli::diff::compute_diff(&before, &after);

    tool_result(
        id,
        &json!({
            "tables_added": diff.tables_added,
            "tables_removed": diff.tables_removed,
            "columns_added": diff.columns_added.iter().map(|c| format!("{}.{}", c.table, c.column)).collect::<Vec<_>>(),
            "columns_removed": diff.columns_removed.iter().map(|c| format!("{}.{}", c.table, c.column)).collect::<Vec<_>>(),
            "indexes_added": diff.indexes_added,
            "indexes_removed": diff.indexes_removed,
            "fks_added": diff.fks_added,
            "fks_removed": diff.fks_removed,
            "summary": format!(
                "Schema diff: +{} tables, -{} tables, +{} columns, -{} columns, +{} FKs, -{} FKs.",
                diff.tables_added.len(),
                diff.tables_removed.len(),
                diff.columns_added.len(),
                diff.columns_removed.len(),
                diff.fks_added.len(),
                diff.fks_removed.len()
            ),
        })
        .to_string(),
    )
}

async fn load_schema_or_snapshot(uri: &str) -> Result<core::RawSchema, anyhow::Error> {
    if uri.contains("://") {
        connectors::extract_schema(uri).await.map_err(|e| e.into())
    } else {
        let data = std::fs::read_to_string(uri)?;
        let snapshot: crate::cli::snapshot::SchemaSnapshot = serde_json::from_str(&data)?;
        Ok(snapshot.schema)
    }
}

fn json_rpc_response(id: Option<Value>, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn json_rpc_error(id: Option<Value>, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn tool_result(id: Option<Value>, text: &str) -> Value {
    json_rpc_response(
        id,
        json!({
            "content": [
                {
                    "type": "text",
                    "text": text
                }
            ]
        }),
    )
}

fn tool_error(id: Option<Value>, message: &str) -> Value {
    json_rpc_response(
        id,
        json!({
            "content": [
                {
                    "type": "text",
                    "text": message
                }
            ],
            "isError": true
        }),
    )
}
