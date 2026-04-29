//! Safe refactor plan: step-by-step order to drop a table (remove FKs first, then drop).

use crate::analysis::{self, ImpactTarget};
use crate::connectors;
use crate::core;
pub async fn run_plan_drop(
    schema_uri: &str,
    target_str: &str,
    json_output: bool,
) -> Result<(), anyhow::Error> {
    let raw: core::RawSchema = connectors::extract_schema(schema_uri).await?;
    let default_schema = raw.default_schema();
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());

    let target =
        ImpactTarget::parse_with_default(target_str, &default_schema).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid target '{}'. Use table (e.g. users) or schema.table (e.g. public.users)",
                target_str
            )
        })?;

    let report = analysis::compute_impact(&target, &graph, &raw, None)
        .ok_or_else(|| anyhow::anyhow!("Table not found: {}", target.qualified_table()))?;

    let fks_to_drop: Vec<_> = raw
        .foreign_keys
        .iter()
        .filter(|fk| format!("{}.{}", fk.to_schema, fk.to_table) == target.qualified_table())
        .collect();

    if json_output {
        let steps: Vec<serde_json::Value> = {
            let mut s = Vec::new();
            for fk in &fks_to_drop {
                s.push(serde_json::json!({
                    "action": "drop_constraint",
                    "sql": format!("ALTER TABLE \"{}\".\"{}\" DROP CONSTRAINT \"{}\";", fk.from_schema, fk.from_table, fk.name),
                    "table": format!("{}.{}", fk.from_schema, fk.from_table),
                    "constraint": &fk.name,
                }));
            }
            s.push(serde_json::json!({
                "action": "drop_table",
                "sql": format!("DROP TABLE IF EXISTS \"{}\".\"{}\";", target.schema, target.table),
                "table": target.qualified_table(),
            }));
            s
        };
        let json = serde_json::json!({
            "target": target.qualified_table(),
            "steps": steps,
            "blast_radius": report.fk_downstream_tables.len(),
            "downstream_tables": report.fk_downstream_tables,
            "index_dependencies": report.index_dependencies.len(),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    eprintln!("dbscope plan drop {}", target.qualified_table());
    eprintln!();
    eprintln!("Safe refactor plan (read-only; apply manually):");
    eprintln!();

    let mut step = 1;

    if !fks_to_drop.is_empty() {
        eprintln!(
            "  {}. Remove foreign keys that reference {}:",
            step,
            target.qualified_table()
        );
        for fk in &fks_to_drop {
            eprintln!(
                "     ALTER TABLE \"{}\".\"{}\" DROP CONSTRAINT \"{}\";",
                fk.from_schema, fk.from_table, fk.name
            );
        }
        step += 1;
        eprintln!();
    }

    if !report.fk_downstream_tables.is_empty() {
        eprintln!(
            "  {}. (Optional) Migrate or backfill data in dependent tables:",
            step
        );
        for t in &report.fk_downstream_tables {
            eprintln!("     - {}", t);
        }
        eprintln!(
            "     Then ensure no application code references {}.",
            target.qualified_table()
        );
        step += 1;
        eprintln!();
    }

    eprintln!("  {}. Drop the table:", step);
    eprintln!(
        "     DROP TABLE IF EXISTS \"{}\".\"{}\";",
        target.schema, target.table
    );
    eprintln!();
    eprintln!(
        "Blast radius: {} table(s) depend on this table (direct + transitive).",
        report.fk_downstream_tables.len()
    );
    if !report.index_dependencies.is_empty() {
        eprintln!(
            "Indexes on target: {} (will be dropped with table).",
            report.index_dependencies.len()
        );
    }

    Ok(())
}
