//! Simulate schema changes from a migration file (DDL) for CI risk check.

use sqlparser::ast::{AlterTableOperation, ObjectName, ObjectType, Statement, TableConstraint};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::core::{ColumnMeta, ForeignKeyRef, RawSchema, TableMeta};

fn object_name_to_schema_table(name: &ObjectName) -> (String, String) {
    let parts: Vec<String> = name.0.iter().map(|i| i.value.clone()).collect();
    if parts.len() >= 2 {
        (parts[0].clone(), parts[1].clone())
    } else if parts.len() == 1 {
        ("public".to_string(), parts[0].clone())
    } else {
        ("public".to_string(), "unknown".to_string())
    }
}

/// Parse a migration file into a list of DDL statements. Returns empty on parse error.
pub fn parse_migration_sql(sql: &str) -> Vec<Statement> {
    let dialect = PostgreSqlDialect {};
    Parser::parse_sql(&dialect, sql).unwrap_or_default()
}

/// Apply DDL statements to a copy of the schema. Best-effort: supports DROP TABLE, CREATE TABLE (basic), ALTER TABLE ADD CONSTRAINT FK.
pub fn apply_migration_to_schema(base: &RawSchema, statements: &[Statement]) -> RawSchema {
    let mut raw = base.clone();

    for stmt in statements {
        match stmt {
            Statement::Drop {
                object_type: ObjectType::Table,
                names,
                ..
            } => {
                for name in names {
                    let (schema, table) = object_name_to_schema_table(name);
                    let q = format!("{}.{}", schema, table);
                    raw.tables
                        .retain(|t| format!("{}.{}", t.schema_name, t.table_name) != q);
                    raw.columns
                        .retain(|c| c.schema_name != schema || c.table_name != table);
                    raw.indexes
                        .retain(|i| i.schema_name != schema || i.table_name != table);
                    raw.constraints
                        .retain(|c| c.schema_name != schema || c.table_name != table);
                    raw.foreign_keys.retain(|fk| {
                        format!("{}.{}", fk.from_schema, fk.from_table) != q
                            && format!("{}.{}", fk.to_schema, fk.to_table) != q
                    });
                }
            }
            Statement::CreateTable { name, columns, .. } => {
                let (schema, table) = object_name_to_schema_table(name);
                let q = format!("{}.{}", schema, table);
                if !raw
                    .tables
                    .iter()
                    .any(|t| format!("{}.{}", t.schema_name, t.table_name) == q)
                {
                    raw.tables.push(TableMeta {
                        schema_name: schema.clone(),
                        table_name: table.clone(),
                    });
                    for (i, col) in columns.iter().enumerate() {
                        raw.columns.push(ColumnMeta {
                            schema_name: schema.clone(),
                            table_name: table.clone(),
                            column_name: col.name.value.clone(),
                            data_type: col.data_type.to_string(),
                            ordinal_position: (i + 1) as i32,
                            is_nullable: None,
                            default_value: None,
                        });
                    }
                }
            }
            Statement::AlterTable {
                name, operations, ..
            } => {
                let (schema, table) = object_name_to_schema_table(name);
                for op in operations {
                    match op {
                        AlterTableOperation::AddConstraint(TableConstraint::ForeignKey {
                            name: fk_name_opt,
                            columns: from_cols,
                            foreign_table: ref_table_name,
                            referred_columns: to_cols,
                            ..
                        }) => {
                            let (to_schema, to_table) = object_name_to_schema_table(ref_table_name);
                            let fk_name = fk_name_opt
                                .as_ref()
                                .map(|i| i.value.clone())
                                .unwrap_or_else(|| format!("{}_fk", table));
                            raw.foreign_keys.push(ForeignKeyRef {
                                name: fk_name,
                                from_schema: schema.clone(),
                                from_table: table.clone(),
                                from_columns: from_cols.iter().map(|c| c.value.clone()).collect(),
                                to_schema,
                                to_table,
                                to_columns: to_cols.iter().map(|c| c.value.clone()).collect(),
                            });
                        }
                        AlterTableOperation::AddConstraint(_) => {}
                        AlterTableOperation::AddColumn { column_def, .. } => {
                            let position = raw
                                .columns
                                .iter()
                                .filter(|c| c.schema_name == schema && c.table_name == table)
                                .count() as i32
                                + 1;
                            raw.columns.push(ColumnMeta {
                                schema_name: schema.clone(),
                                table_name: table.clone(),
                                column_name: column_def.name.value.clone(),
                                data_type: column_def.data_type.to_string(),
                                ordinal_position: position,
                                is_nullable: None,
                                default_value: None,
                            });
                        }
                        AlterTableOperation::DropColumn { column_name, .. } => {
                            let col = column_name.value.clone();
                            raw.columns.retain(|c| {
                                !(c.schema_name == schema
                                    && c.table_name == table
                                    && c.column_name == col)
                            });
                            raw.indexes.retain(|i| {
                                !(i.schema_name == schema
                                    && i.table_name == table
                                    && i.column_names.contains(&col))
                            });
                        }
                        AlterTableOperation::RenameTable { table_name } => {
                            let (new_schema, new_table) = object_name_to_schema_table(table_name);
                            let old_q = format!("{}.{}", schema, table);
                            for t in &mut raw.tables {
                                if format!("{}.{}", t.schema_name, t.table_name) == old_q {
                                    t.schema_name = new_schema.clone();
                                    t.table_name = new_table.clone();
                                }
                            }
                            for c in &mut raw.columns {
                                if c.schema_name == schema && c.table_name == table {
                                    c.schema_name = new_schema.clone();
                                    c.table_name = new_table.clone();
                                }
                            }
                            for i in &mut raw.indexes {
                                if i.schema_name == schema && i.table_name == table {
                                    i.schema_name = new_schema.clone();
                                    i.table_name = new_table.clone();
                                }
                            }
                            for fk in &mut raw.foreign_keys {
                                if fk.from_schema == schema && fk.from_table == table {
                                    fk.from_schema = new_schema.clone();
                                    fk.from_table = new_table.clone();
                                }
                                if fk.to_schema == schema && fk.to_table == table {
                                    fk.to_schema = new_schema.clone();
                                    fk.to_table = new_table.clone();
                                }
                            }
                        }
                        AlterTableOperation::DropConstraint {
                            name: constraint_name,
                            ..
                        } => {
                            let cn = constraint_name.value.clone();
                            raw.constraints.retain(|c| {
                                !(c.schema_name == schema
                                    && c.table_name == table
                                    && c.constraint_name == cn)
                            });
                            raw.foreign_keys.retain(|fk| {
                                !(fk.from_schema == schema
                                    && fk.from_table == table
                                    && fk.name == cn)
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_apply_drop_table() {
        let base = RawSchema {
            tables: vec![
                TableMeta {
                    schema_name: "public".into(),
                    table_name: "a".into(),
                },
                TableMeta {
                    schema_name: "public".into(),
                    table_name: "b".into(),
                },
            ],
            views: vec![],
            materialized_views: vec![],
            columns: vec![],
            indexes: vec![],
            constraints: vec![],
            foreign_keys: vec![],
            table_stats: None,
            engine_metadata: None,
        };
        let stmts = parse_migration_sql("DROP TABLE IF EXISTS public.a;");
        let out = apply_migration_to_schema(&base, &stmts);
        assert_eq!(out.tables.len(), 1);
        assert_eq!(out.tables[0].table_name, "b");
    }
}
