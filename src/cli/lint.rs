//! `dbscope lint`: schema anti-pattern detection engine.
//! Catches structural issues that lead to operational problems.

use crate::core::RawSchema;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintSeverity::Error => write!(f, "ERROR"),
            LintSeverity::Warning => write!(f, "WARN"),
            LintSeverity::Info => write!(f, "INFO"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LintViolation {
    pub rule: String,
    pub severity: LintSeverity,
    pub table: String,
    pub message: String,
    pub suggestion: String,
}

/// All lint rules applied to a schema.
pub fn lint_schema(raw: &RawSchema) -> Vec<LintViolation> {
    let mut violations = Vec::new();

    check_missing_primary_keys(raw, &mut violations);
    check_wide_tables(raw, &mut violations);
    check_missing_fk_indexes(raw, &mut violations);
    check_naming_conventions(raw, &mut violations);
    check_nullable_fks(raw, &mut violations);
    check_implicit_many_to_many(raw, &mut violations);
    check_redundant_indexes(raw, &mut violations);
    check_untyped_text_columns(raw, &mut violations);

    violations.sort_by(|a, b| {
        severity_ord(a.severity)
            .cmp(&severity_ord(b.severity))
            .then(a.table.cmp(&b.table))
    });

    violations
}

fn severity_ord(s: LintSeverity) -> u8 {
    match s {
        LintSeverity::Error => 0,
        LintSeverity::Warning => 1,
        LintSeverity::Info => 2,
    }
}

fn check_missing_primary_keys(raw: &RawSchema, violations: &mut Vec<LintViolation>) {
    let tables_with_pk: HashSet<String> = raw
        .constraints
        .iter()
        .filter(|c| c.constraint_type.to_uppercase().contains("PRIMARY"))
        .map(|c| format!("{}.{}", c.schema_name, c.table_name))
        .collect();

    let tables_with_unique_id: HashSet<String> = raw
        .indexes
        .iter()
        .filter(|i| i.is_unique && i.column_names.iter().any(|c| c == "id"))
        .map(|i| format!("{}.{}", i.schema_name, i.table_name))
        .collect();

    for t in &raw.tables {
        let q = t.qualified_name();
        if !tables_with_pk.contains(&q) && !tables_with_unique_id.contains(&q) {
            violations.push(LintViolation {
                rule: "no-primary-key".into(),
                severity: LintSeverity::Error,
                table: q,
                message: "Table has no PRIMARY KEY constraint".into(),
                suggestion: "Add a PRIMARY KEY. Every table should have one for data integrity and replication.".into(),
            });
        }
    }
}

fn check_wide_tables(raw: &RawSchema, violations: &mut Vec<LintViolation>) {
    let mut col_counts: HashMap<String, usize> = HashMap::new();
    for c in &raw.columns {
        let q = format!("{}.{}", c.schema_name, c.table_name);
        *col_counts.entry(q).or_default() += 1;
    }
    for (table, count) in col_counts {
        if count > 50 {
            violations.push(LintViolation {
                rule: "wide-table".into(),
                severity: LintSeverity::Warning,
                table,
                message: format!("Table has {} columns (>50)", count),
                suggestion: "Consider vertical partitioning or extracting a related table to reduce row width.".into(),
            });
        } else if count > 30 {
            violations.push(LintViolation {
                rule: "wide-table".into(),
                severity: LintSeverity::Info,
                table,
                message: format!("Table has {} columns (>30)", count),
                suggestion: "Review if all columns belong here or if some can be extracted.".into(),
            });
        }
    }
}

fn check_missing_fk_indexes(raw: &RawSchema, violations: &mut Vec<LintViolation>) {
    let indexed_cols: HashSet<(String, String, String)> = raw
        .indexes
        .iter()
        .flat_map(|i| {
            i.column_names
                .iter()
                .map(move |c| (i.schema_name.clone(), i.table_name.clone(), c.clone()))
        })
        .collect();

    for fk in &raw.foreign_keys {
        for col in &fk.from_columns {
            let key = (fk.from_schema.clone(), fk.from_table.clone(), col.clone());
            if !indexed_cols.contains(&key) {
                violations.push(LintViolation {
                    rule: "missing-fk-index".into(),
                    severity: LintSeverity::Warning,
                    table: format!("{}.{}", fk.from_schema, fk.from_table),
                    message: format!("FK column '{}' has no index", col),
                    suggestion: format!(
                        "CREATE INDEX idx_{}_{} ON {}.{}({});",
                        fk.from_table, col, fk.from_schema, fk.from_table, col
                    ),
                });
            }
        }
    }
}

fn check_naming_conventions(raw: &RawSchema, violations: &mut Vec<LintViolation>) {
    for t in &raw.tables {
        if t.table_name.chars().any(|c| c.is_uppercase()) {
            violations.push(LintViolation {
                rule: "naming-convention".into(),
                severity: LintSeverity::Info,
                table: t.qualified_name(),
                message: "Table name contains uppercase characters".into(),
                suggestion: "Use snake_case for table names (PostgreSQL convention).".into(),
            });
        }
    }

    for c in &raw.columns {
        if c.column_name.chars().any(|ch| ch.is_uppercase()) {
            violations.push(LintViolation {
                rule: "naming-convention".into(),
                severity: LintSeverity::Info,
                table: format!("{}.{}", c.schema_name, c.table_name),
                message: format!("Column '{}' contains uppercase characters", c.column_name),
                suggestion: "Use snake_case for column names.".into(),
            });
        }
    }
}

fn check_nullable_fks(raw: &RawSchema, violations: &mut Vec<LintViolation>) {
    let nullable_cols: HashSet<(String, String, String)> = raw
        .columns
        .iter()
        .filter(|c| c.is_nullable == Some(true))
        .map(|c| {
            (
                c.schema_name.clone(),
                c.table_name.clone(),
                c.column_name.clone(),
            )
        })
        .collect();

    for fk in &raw.foreign_keys {
        for col in &fk.from_columns {
            let key = (fk.from_schema.clone(), fk.from_table.clone(), col.clone());
            if nullable_cols.contains(&key) {
                violations.push(LintViolation {
                    rule: "nullable-fk".into(),
                    severity: LintSeverity::Info,
                    table: format!("{}.{}", fk.from_schema, fk.from_table),
                    message: format!("FK column '{}' is nullable, allows orphaned references", col),
                    suggestion: "Consider making the FK column NOT NULL unless the relationship is truly optional.".into(),
                });
            }
        }
    }
}

fn check_implicit_many_to_many(raw: &RawSchema, violations: &mut Vec<LintViolation>) {
    let mut fk_count_per_table: HashMap<String, usize> = HashMap::new();
    let mut col_count_per_table: HashMap<String, usize> = HashMap::new();

    for fk in &raw.foreign_keys {
        let q = format!("{}.{}", fk.from_schema, fk.from_table);
        *fk_count_per_table.entry(q).or_default() += 1;
    }
    for c in &raw.columns {
        let q = format!("{}.{}", c.schema_name, c.table_name);
        *col_count_per_table.entry(q).or_default() += 1;
    }

    for (table, fk_count) in &fk_count_per_table {
        if *fk_count >= 2 {
            let col_count = col_count_per_table.get(table).copied().unwrap_or(0);
            if col_count <= fk_count + 2 {
                violations.push(LintViolation {
                    rule: "junction-table".into(),
                    severity: LintSeverity::Info,
                    table: table.clone(),
                    message: format!(
                        "Looks like a junction table ({} FKs, {} cols). Ensure composite PK or unique constraint",
                        fk_count, col_count
                    ),
                    suggestion: "Add PRIMARY KEY or UNIQUE constraint on the FK column combination.".into(),
                });
            }
        }
    }
}

fn check_redundant_indexes(raw: &RawSchema, violations: &mut Vec<LintViolation>) {
    let mut table_indexes: HashMap<String, Vec<(&str, &[String])>> = HashMap::new();
    for idx in &raw.indexes {
        let q = format!("{}.{}", idx.schema_name, idx.table_name);
        table_indexes
            .entry(q)
            .or_default()
            .push((&idx.index_name, &idx.column_names));
    }

    for (table, indexes) in &table_indexes {
        for (i, (name_a, cols_a)) in indexes.iter().enumerate() {
            for (name_b, cols_b) in indexes.iter().skip(i + 1) {
                if cols_a.len() < cols_b.len() && cols_b.starts_with(cols_a) {
                    violations.push(LintViolation {
                        rule: "redundant-index".into(),
                        severity: LintSeverity::Warning,
                        table: table.clone(),
                        message: format!(
                            "Index '{}' ({}) is a prefix of '{}' ({})",
                            name_a,
                            cols_a.join(", "),
                            name_b,
                            cols_b.join(", ")
                        ),
                        suggestion: format!(
                            "Consider dropping '{}'; the composite index covers it.",
                            name_a
                        ),
                    });
                } else if cols_b.len() < cols_a.len() && cols_a.starts_with(cols_b) {
                    violations.push(LintViolation {
                        rule: "redundant-index".into(),
                        severity: LintSeverity::Warning,
                        table: table.clone(),
                        message: format!(
                            "Index '{}' ({}) is a prefix of '{}' ({})",
                            name_b,
                            cols_b.join(", "),
                            name_a,
                            cols_a.join(", ")
                        ),
                        suggestion: format!(
                            "Consider dropping '{}'; the composite index covers it.",
                            name_b
                        ),
                    });
                }
            }
        }
    }
}

fn check_untyped_text_columns(raw: &RawSchema, violations: &mut Vec<LintViolation>) {
    let text_types = [
        "text",
        "varchar",
        "character varying",
        "longtext",
        "mediumtext",
        "tinytext",
    ];
    for c in &raw.columns {
        let lower = c.data_type.to_lowercase();
        if text_types.iter().any(|t| lower.contains(t)) {
            let name_lower = c.column_name.to_lowercase();
            let smells_like_enum = name_lower.ends_with("_type")
                || name_lower.ends_with("_status")
                || name_lower == "status"
                || name_lower == "type"
                || name_lower == "kind"
                || name_lower == "role"
                || name_lower == "state"
                || name_lower == "level"
                || name_lower == "priority"
                || name_lower == "category";
            if smells_like_enum {
                violations.push(LintViolation {
                    rule: "text-enum".into(),
                    severity: LintSeverity::Info,
                    table: format!("{}.{}", c.schema_name, c.table_name),
                    message: format!("Column '{}' looks like an enum but uses text type", c.column_name),
                    suggestion: "Consider using a CHECK constraint, ENUM type, or a reference table to enforce valid values.".into(),
                });
            }
        }
    }
}

pub fn run_lint(raw: &RawSchema, json_output: bool) -> Result<(), anyhow::Error> {
    use super::style::Theme;
    let violations = lint_schema(raw);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&violations)?);
        return Ok(());
    }

    let t = Theme::detect();

    if violations.is_empty() {
        println!("{}", t.risk_low("No lint violations. Schema looks clean."));
        return Ok(());
    }

    let errors = violations
        .iter()
        .filter(|v| v.severity == LintSeverity::Error)
        .count();
    let warnings = violations
        .iter()
        .filter(|v| v.severity == LintSeverity::Warning)
        .count();
    let infos = violations
        .iter()
        .filter(|v| v.severity == LintSeverity::Info)
        .count();

    println!(
        "\n  {} {} violations  {} errors  {} warnings  {} info\n",
        t.heading("dbscope lint"),
        t.bold(&violations.len().to_string()),
        t.risk_critical(&errors.to_string()),
        t.risk_medium(&warnings.to_string()),
        t.dim(&infos.to_string()),
    );

    for v in &violations {
        let severity_str = match v.severity {
            LintSeverity::Error => t.risk_critical(&format!("{}", v.severity)),
            LintSeverity::Warning => t.risk_medium(&format!("{}", v.severity)),
            LintSeverity::Info => t.dim(&format!("{}", v.severity)),
        };
        println!(
            "  {} {} {}",
            severity_str,
            t.muted(&v.table),
            t.bold(&v.rule)
        );
        println!("    {}", v.message);
        println!("    {}\n", t.dim(&format!("fix: {}", v.suggestion)));
    }

    if errors > 0 {
        anyhow::bail!("risk check failed: {} lint error(s) found", errors);
    }

    Ok(())
}
