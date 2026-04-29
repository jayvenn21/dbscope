//! `dbscope diff`: compare two schema snapshots or a snapshot vs. live database.
//! Shows structural delta: added/removed/modified tables, columns, indexes, FKs.

use crate::cli::snapshot::SchemaSnapshot;
use crate::connectors::extract_schema;
use crate::core::RawSchema;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct SchemaDiff {
    pub tables_added: Vec<String>,
    pub tables_removed: Vec<String>,
    pub columns_added: Vec<ColumnChange>,
    pub columns_removed: Vec<ColumnChange>,
    pub columns_type_changed: Vec<ColumnTypeChange>,
    pub indexes_added: Vec<String>,
    pub indexes_removed: Vec<String>,
    pub fks_added: Vec<String>,
    pub fks_removed: Vec<String>,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnChange {
    pub table: String,
    pub column: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnTypeChange {
    pub table: String,
    pub column: String,
    pub old_type: String,
    pub new_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffSummary {
    pub total_changes: usize,
    pub breaking_changes: usize,
    pub risk_assessment: String,
}

pub fn compute_diff(before: &RawSchema, after: &RawSchema) -> SchemaDiff {
    let before_tables: BTreeSet<String> =
        before.tables.iter().map(|t| t.qualified_name()).collect();
    let after_tables: BTreeSet<String> = after.tables.iter().map(|t| t.qualified_name()).collect();

    let tables_added: Vec<String> = after_tables.difference(&before_tables).cloned().collect();
    let tables_removed: Vec<String> = before_tables.difference(&after_tables).cloned().collect();

    let before_col_set: BTreeSet<(String, String)> = before
        .columns
        .iter()
        .map(|c| {
            (
                format!("{}.{}", c.schema_name, c.table_name),
                c.column_name.clone(),
            )
        })
        .collect();
    let after_col_set: BTreeSet<(String, String)> = after
        .columns
        .iter()
        .map(|c| {
            (
                format!("{}.{}", c.schema_name, c.table_name),
                c.column_name.clone(),
            )
        })
        .collect();

    let mut columns_added = Vec::new();
    for (table, col) in after_col_set.difference(&before_col_set) {
        let dtype = after
            .columns
            .iter()
            .find(|c| {
                format!("{}.{}", c.schema_name, c.table_name) == *table && c.column_name == *col
            })
            .map(|c| c.data_type.clone())
            .unwrap_or_default();
        columns_added.push(ColumnChange {
            table: table.clone(),
            column: col.clone(),
            data_type: dtype,
        });
    }

    let mut columns_removed = Vec::new();
    for (table, col) in before_col_set.difference(&after_col_set) {
        let dtype = before
            .columns
            .iter()
            .find(|c| {
                format!("{}.{}", c.schema_name, c.table_name) == *table && c.column_name == *col
            })
            .map(|c| c.data_type.clone())
            .unwrap_or_default();
        columns_removed.push(ColumnChange {
            table: table.clone(),
            column: col.clone(),
            data_type: dtype,
        });
    }

    let mut columns_type_changed = Vec::new();
    let before_col_types: BTreeMap<(String, String), String> = before
        .columns
        .iter()
        .map(|c| {
            (
                (
                    format!("{}.{}", c.schema_name, c.table_name),
                    c.column_name.clone(),
                ),
                c.data_type.clone(),
            )
        })
        .collect();
    let after_col_types: BTreeMap<(String, String), String> = after
        .columns
        .iter()
        .map(|c| {
            (
                (
                    format!("{}.{}", c.schema_name, c.table_name),
                    c.column_name.clone(),
                ),
                c.data_type.clone(),
            )
        })
        .collect();
    for (key, old_type) in &before_col_types {
        if let Some(new_type) = after_col_types.get(key) {
            if old_type != new_type {
                columns_type_changed.push(ColumnTypeChange {
                    table: key.0.clone(),
                    column: key.1.clone(),
                    old_type: old_type.clone(),
                    new_type: new_type.clone(),
                });
            }
        }
    }

    let before_idx: BTreeSet<String> = before
        .indexes
        .iter()
        .map(|i| format!("{}.{}.{}", i.schema_name, i.table_name, i.index_name))
        .collect();
    let after_idx: BTreeSet<String> = after
        .indexes
        .iter()
        .map(|i| format!("{}.{}.{}", i.schema_name, i.table_name, i.index_name))
        .collect();
    let indexes_added: Vec<String> = after_idx.difference(&before_idx).cloned().collect();
    let indexes_removed: Vec<String> = before_idx.difference(&after_idx).cloned().collect();

    let before_fks: BTreeSet<String> = before.foreign_keys.iter().map(|f| f.name.clone()).collect();
    let after_fks: BTreeSet<String> = after.foreign_keys.iter().map(|f| f.name.clone()).collect();
    let fks_added: Vec<String> = after_fks.difference(&before_fks).cloned().collect();
    let fks_removed: Vec<String> = before_fks.difference(&after_fks).cloned().collect();

    let breaking = tables_removed.len()
        + columns_removed.len()
        + columns_type_changed.len()
        + fks_removed.len();
    let total = tables_added.len()
        + tables_removed.len()
        + columns_added.len()
        + columns_removed.len()
        + columns_type_changed.len()
        + indexes_added.len()
        + indexes_removed.len()
        + fks_added.len()
        + fks_removed.len();

    let risk_assessment = if breaking == 0 && total == 0 {
        "No changes detected".to_string()
    } else if breaking == 0 {
        "Additive only, safe to deploy".to_string()
    } else if breaking <= 2 {
        format!("{} breaking change(s), review required", breaking)
    } else {
        format!(
            "{} breaking changes, high risk, requires careful migration",
            breaking
        )
    };

    SchemaDiff {
        tables_added,
        tables_removed,
        columns_added,
        columns_removed,
        columns_type_changed,
        indexes_added,
        indexes_removed,
        fks_added,
        fks_removed,
        summary: DiffSummary {
            total_changes: total,
            breaking_changes: breaking,
            risk_assessment,
        },
    }
}

fn print_diff(diff: &SchemaDiff) {
    if diff.summary.total_changes == 0 {
        println!("No schema changes detected.");
        return;
    }

    println!(
        "Schema diff: {} total changes, {} breaking\n",
        diff.summary.total_changes, diff.summary.breaking_changes
    );

    if !diff.tables_added.is_empty() {
        println!("  Tables added:");
        for t in &diff.tables_added {
            println!("    + {}", t);
        }
    }
    if !diff.tables_removed.is_empty() {
        println!("  Tables removed (BREAKING):");
        for t in &diff.tables_removed {
            println!("    - {}", t);
        }
    }
    if !diff.columns_added.is_empty() {
        println!("  Columns added:");
        for c in &diff.columns_added {
            println!("    + {}.{} ({})", c.table, c.column, c.data_type);
        }
    }
    if !diff.columns_removed.is_empty() {
        println!("  Columns removed (BREAKING):");
        for c in &diff.columns_removed {
            println!("    - {}.{} ({})", c.table, c.column, c.data_type);
        }
    }
    if !diff.columns_type_changed.is_empty() {
        println!("  Column type changes (BREAKING):");
        for c in &diff.columns_type_changed {
            println!(
                "    ~ {}.{}: {} → {}",
                c.table, c.column, c.old_type, c.new_type
            );
        }
    }
    if !diff.indexes_added.is_empty() {
        println!("  Indexes added:");
        for i in &diff.indexes_added {
            println!("    + {}", i);
        }
    }
    if !diff.indexes_removed.is_empty() {
        println!("  Indexes removed:");
        for i in &diff.indexes_removed {
            println!("    - {}", i);
        }
    }
    if !diff.fks_added.is_empty() {
        println!("  Foreign keys added:");
        for f in &diff.fks_added {
            println!("    + {}", f);
        }
    }
    if !diff.fks_removed.is_empty() {
        println!("  Foreign keys removed (BREAKING):");
        for f in &diff.fks_removed {
            println!("    - {}", f);
        }
    }

    println!("\n  Assessment: {}", diff.summary.risk_assessment);
}

pub async fn run_diff(
    before_path: &Path,
    after_source: &str,
    json_output: bool,
) -> Result<(), anyhow::Error> {
    let before_snap = SchemaSnapshot::load(before_path)?;

    let after_schema = if Path::new(after_source).exists() {
        let after_snap = SchemaSnapshot::load(Path::new(after_source))?;
        after_snap.schema
    } else {
        extract_schema(after_source).await?
    };

    let diff = compute_diff(&before_snap.schema, &after_schema);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else {
        print_diff(&diff);
    }

    Ok(())
}
