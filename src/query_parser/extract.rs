//! Extract tables, columns, and WHERE-columns from SQL via sqlparser.

use std::collections::HashMap;

use sqlparser::ast::{Expr, Ident, ObjectName, SelectItem, SetExpr, Statement, TableFactor, TableWithJoins};

/// Qualified table: schema.table or table (schema default "public" when missing).
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct QualifiedTable {
    pub schema: String,
    pub table: String,
}

impl QualifiedTable {
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
}

/// Qualified column: schema.table.column or table.column.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct QualifiedColumn {
    pub schema: String,
    pub table: String,
    pub column: String,
}

impl QualifiedColumn {
    pub fn key(&self) -> (String, String, String) {
        (self.schema.clone(), self.table.clone(), self.column.clone())
    }
}

/// Result of parsing one SQL statement.
#[derive(Debug, Clone, Default)]
pub struct ParsedQuery {
    pub tables: Vec<QualifiedTable>,
    /// Alias (e.g. "p") -> actual table (e.g. public.posts). Used to resolve column refs in WHERE.
    pub alias_to_table: HashMap<String, QualifiedTable>,
    pub columns: Vec<QualifiedColumn>,
    /// Columns that appear in WHERE (and HAVING) — used for index suggestions.
    pub columns_in_where: Vec<QualifiedColumn>,
    /// Pairs of table qualified names that are joined (for join hotspots).
    pub join_pairs: Vec<(String, String)>,
}

/// Parse a single SQL string into ParsedQuery. Returns None if unparseable.
pub fn parse_sql(sql: &str) -> Option<ParsedQuery> {
    let dialect = sqlparser::dialect::PostgreSqlDialect {};
    let stmts = sqlparser::parser::Parser::parse_sql(&dialect, sql).ok()?;
    let stmt = stmts.first()?;
    extract_from_statement(stmt)
}

fn object_name_to_qualified(name: &ObjectName) -> QualifiedTable {
    let parts: Vec<String> = name.0.iter().map(|i| i.value.clone()).collect();
    if parts.len() >= 2 {
        QualifiedTable {
            schema: parts[0].clone(),
            table: parts[1].clone(),
        }
    } else if parts.len() == 1 {
        QualifiedTable {
            schema: "public".to_string(),
            table: parts[0].clone(),
        }
    } else {
        QualifiedTable {
            schema: "public".to_string(),
            table: "unknown".to_string(),
        }
    }
}

fn idents_to_qualified_column(
    idents: &[Ident],
    default_schema: &str,
    default_table: &str,
    alias_to_table: &HashMap<String, QualifiedTable>,
) -> Option<QualifiedColumn> {
    let parts: Vec<String> = idents.iter().map(|i| i.value.clone()).collect();
    if parts.len() >= 3 {
        Some(QualifiedColumn {
            schema: parts[0].clone(),
            table: parts[1].clone(),
            column: parts[2].clone(),
        })
    } else if parts.len() == 2 {
        // table.column — resolve alias to actual table so index suggestions use canonical names
        let (schema, table) = alias_to_table
            .get(&parts[0])
            .map(|qt| (qt.schema.clone(), qt.table.clone()))
            .unwrap_or_else(|| (default_schema.to_string(), parts[0].clone()));
        Some(QualifiedColumn {
            schema,
            table,
            column: parts[1].clone(),
        })
    } else if parts.len() == 1 {
        Some(QualifiedColumn {
            schema: default_schema.to_string(),
            table: default_table.to_string(),
            column: parts[0].clone(),
        })
    } else {
        None
    }
}

fn extract_from_statement(stmt: &Statement) -> Option<ParsedQuery> {
    let mut out = ParsedQuery::default();
    match stmt {
        Statement::Query(q) => {
            extract_from_query(q.as_ref(), &mut out);
        }
        Statement::Insert { table_name, columns, source, .. } => {
            let qt = object_name_to_qualified(table_name);
            out.tables.push(qt.clone());
            if let Some(sub) = source.as_ref() {
                extract_from_query(sub, &mut out);
            }
            for col in columns {
                out.columns.push(QualifiedColumn {
                    schema: qt.schema.clone(),
                    table: qt.table.clone(),
                    column: col.value.clone(),
                });
            }
        }
        Statement::Update { table, selection, .. } => {
            tables_from_table_with_joins(table, &mut out);
            if let Some(expr) = selection {
                columns_from_expr(expr, &out.tables, &out.alias_to_table, true, &mut out.columns_in_where);
            }
        }
        Statement::Delete { tables: delete_tables, selection, .. } => {
            for name in delete_tables {
                out.tables.push(object_name_to_qualified(name));
            }
            if let Some(expr) = selection {
                columns_from_expr(expr, &out.tables, &out.alias_to_table, true, &mut out.columns_in_where);
            }
        }
        _ => return None,
    }
    Some(out)
}


fn tables_from_table_with_joins(twj: &TableWithJoins, out: &mut ParsedQuery) {
    table_factor_to_qualified(&twj.relation, out);
    for join in &twj.joins {
        table_factor_to_qualified(&join.relation, out);
    }
}

fn table_factor_to_qualified(tf: &TableFactor, out: &mut ParsedQuery) {
    match tf {
        TableFactor::Table { name, alias, .. } => {
            let qt = object_name_to_qualified(name);
            out.tables.push(qt.clone());
            if let Some(a) = alias {
                out.alias_to_table.insert(a.name.value.clone(), qt);
            }
        }
        TableFactor::Derived { subquery, .. } => {
            extract_from_query(subquery.as_ref(), &mut ParsedQuery::default());
            // We don't add subquery tables to top-level tables for simplicity
        }
        TableFactor::NestedJoin { table_with_joins, .. } => {
            tables_from_table_with_joins(table_with_joins, out);
        }
        _ => {}
    }
}

fn extract_from_query(query: &sqlparser::ast::Query, out: &mut ParsedQuery) {
    extract_from_set_expr(&query.body, out);
}

fn extract_from_set_expr(expr: &SetExpr, out: &mut ParsedQuery) {
    match expr {
        SetExpr::Select(select) => {
            for twj in &select.from {
                let n_before = out.tables.len();
                tables_from_table_with_joins(twj, out);
                for i in 1..(out.tables.len() - n_before) {
                    let a = n_before + i - 1;
                    let b = n_before + i;
                    let t1 = out.tables[a].qualified_name();
                    let t2 = out.tables[b].qualified_name();
                    if t1 != t2 {
                        out.join_pairs.push((t1, t2));
                    }
                }
            }
            let alias_to_table = &out.alias_to_table;
            for item in &select.projection {
                match item {
                    SelectItem::UnnamedExpr(e) => columns_from_expr(e, &out.tables, alias_to_table, false, &mut out.columns),
                    SelectItem::ExprWithAlias { expr, .. } => columns_from_expr(expr, &out.tables, alias_to_table, false, &mut out.columns),
                    SelectItem::QualifiedWildcard(name, _) => {
                        let qt = object_name_to_qualified(name);
                        out.columns.push(QualifiedColumn {
                            schema: qt.schema,
                            table: qt.table,
                            column: "*".to_string(),
                        });
                    }
                    _ => {}
                }
            }
            if let Some(sel) = &select.selection {
                columns_from_expr(sel, &out.tables, alias_to_table, true, &mut out.columns_in_where);
            }
            if let Some(having) = &select.having {
                columns_from_expr(having, &out.tables, alias_to_table, true, &mut out.columns_in_where);
            }
        }
        SetExpr::Query(q) => extract_from_query(q, out),
        SetExpr::SetOperation { left, right, .. } => {
            extract_from_set_expr(left, out);
            extract_from_set_expr(right, out);
        }
        _ => {}
    }
}

fn columns_from_expr(
    expr: &Expr,
    tables: &[QualifiedTable],
    alias_to_table: &HashMap<String, QualifiedTable>,
    in_where: bool,
    out: &mut Vec<QualifiedColumn>,
) {
    let default_schema = tables.first().map(|t| t.schema.as_str()).unwrap_or("public");
    let default_table = tables.first().map(|t| t.table.as_str()).unwrap_or("");
    match expr {
        Expr::Identifier(ident) => {
            if in_where {
                out.push(QualifiedColumn {
                    schema: default_schema.to_string(),
                    table: default_table.to_string(),
                    column: ident.value.clone(),
                });
            } else {
                out.push(QualifiedColumn {
                    schema: default_schema.to_string(),
                    table: default_table.to_string(),
                    column: ident.value.clone(),
                });
            }
        }
        Expr::CompoundIdentifier(idents) => {
            if let Some(qc) = idents_to_qualified_column(idents, default_schema, default_table, alias_to_table) {
                out.push(qc);
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            columns_from_expr(left, tables, alias_to_table, in_where, out);
            columns_from_expr(right, tables, alias_to_table, in_where, out);
        }
        Expr::UnaryOp { expr, .. } => columns_from_expr(expr, tables, alias_to_table, in_where, out),
        Expr::IsNull(expr) | Expr::IsNotNull(expr) => columns_from_expr(expr, tables, alias_to_table, in_where, out),
        Expr::InList { expr, list, .. } => {
            columns_from_expr(expr, tables, alias_to_table, in_where, out);
            for e in list {
                columns_from_expr(e, tables, alias_to_table, in_where, out);
            }
        }
        Expr::Between { expr, low, high, .. } => {
            columns_from_expr(expr, tables, alias_to_table, in_where, out);
            columns_from_expr(low, tables, alias_to_table, in_where, out);
            columns_from_expr(high, tables, alias_to_table, in_where, out);
        }
        Expr::Like { expr, pattern, .. } | Expr::ILike { expr, pattern, .. } => {
            columns_from_expr(expr, tables, alias_to_table, in_where, out);
            columns_from_expr(pattern, tables, alias_to_table, in_where, out);
        }
        Expr::Nested(e) => columns_from_expr(e, tables, alias_to_table, in_where, out),
        Expr::Case { conditions, results, else_result, .. } => {
            for c in conditions {
                columns_from_expr(c, tables, alias_to_table, in_where, out);
            }
            for r in results {
                columns_from_expr(r, tables, alias_to_table, in_where, out);
            }
            if let Some(e) = else_result {
                columns_from_expr(e, tables, alias_to_table, in_where, out);
            }
        }
        _ => {}
    }
}

/// Aggregate many parsed queries into table hits, column hits, and column-in-where hits.
#[derive(Debug, Clone, Default)]
pub struct QueryUsage {
    pub table_hits: HashMap<String, u64>,
    /// (schema, table, column) -> (ref_count, in_where_count)
    pub column_hits: HashMap<(String, String, String), (u64, u64)>,
    pub join_pairs: HashMap<(String, String), u64>,
}

pub fn aggregate_queries(queries: &[ParsedQuery]) -> QueryUsage {
    let mut usage = QueryUsage::default();
    for q in queries {
        for t in &q.tables {
            *usage.table_hits.entry(t.qualified_name()).or_insert(0) += 1;
        }
        for c in &q.columns {
            if c.column != "*" {
                let k = c.key();
                let e = usage.column_hits.entry(k).or_insert((0, 0));
                e.0 += 1;
            }
        }
        for c in &q.columns_in_where {
            let k = c.key();
            let e = usage.column_hits.entry(k).or_insert((0, 0));
            e.0 += 1;
            e.1 += 1;
        }
        for (a, b) in &q.join_pairs {
            let pair = if a.as_str() < b.as_str() {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            };
            *usage.join_pairs.entry(pair).or_insert(0) += 1;
        }
    }
    usage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_select_extracts_tables_and_columns() {
        let q = parse_sql("SELECT id, name FROM public.users WHERE id = 1").unwrap();
        assert_eq!(q.tables.len(), 1);
        assert_eq!(q.tables[0].qualified_name(), "public.users");
        assert!(!q.columns.is_empty());
        assert!(!q.columns_in_where.is_empty());
    }

    #[test]
    fn parse_select_join_extracts_join_pairs() {
        let q = parse_sql("SELECT * FROM public.users u JOIN public.posts p ON u.id = p.user_id").unwrap();
        assert!(q.tables.len() >= 2);
        assert!(!q.join_pairs.is_empty());
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_sql("not valid sql").is_none());
    }
}
