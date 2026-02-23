//! SQLite connector: extracts schema from sqlite_master and PRAGMAs into [RawSchema].

use async_trait::async_trait;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use crate::connectors::connector::{Connector, ConnectorError};
use crate::core::{
    ColumnMeta, ConstraintMeta, ForeignKeyRef, IndexMeta, RawSchema, TableMeta,
};

/// SQLite connector. Produces the same [RawSchema] as other engines.
/// Uses "main" as schema name (SQLite has no multi-schema like Postgres).
#[derive(Debug, Clone, Default)]
pub struct SqliteConnector;

#[async_trait]
impl Connector for SqliteConnector {
    fn name(&self) -> &'static str {
        "sqlite"
    }

    async fn extract_schema(&self, connection_uri: &str) -> Result<RawSchema, ConnectorError> {
        extract_schema(connection_uri).await.map_err(ConnectorError::Sqlite)
    }
}

const SCHEMA_NAME: &str = "main";

pub async fn extract_schema(connection_uri: &str) -> Result<RawSchema, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect(connection_uri)
        .await?;

    let tables = fetch_tables(&pool).await?;
    let columns = fetch_columns(&pool, &tables).await?;
    let (indexes, constraints) = fetch_indexes_and_constraints(&pool, &tables).await?;
    let foreign_keys = fetch_foreign_keys(&pool, &tables).await?;

    pool.close().await;
    Ok(RawSchema {
        tables,
        views: Vec::new(),
        materialized_views: Vec::new(),
        columns,
        indexes,
        constraints,
        foreign_keys,
        table_stats: None,
        engine_metadata: None,
    })
}

async fn fetch_tables(pool: &SqlitePool) -> Result<Vec<TableMeta>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT name FROM sqlite_master
        WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(table_name,)| TableMeta {
            schema_name: SCHEMA_NAME.to_string(),
            table_name,
        })
        .collect())
}

async fn fetch_columns(
    pool: &SqlitePool,
    tables: &[TableMeta],
) -> Result<Vec<ColumnMeta>, sqlx::Error> {
    let mut columns = Vec::new();
    for t in tables {
        // PRAGMA table_info returns: cid, name, type, notnull, dflt_value, pk
        let rows = sqlx::query_as::<_, (i32, String, String, i32, Option<String>, i32)>(
            "PRAGMA table_info(?)",
        )
        .bind(&t.table_name)
        .fetch_all(pool)
        .await?;

        for (ordinal_position, column_name, data_type, _notnull, _dflt, _pk) in rows {
            columns.push(ColumnMeta {
                schema_name: t.schema_name.clone(),
                table_name: t.table_name.clone(),
                column_name,
                data_type,
                ordinal_position: ordinal_position + 1, // 1-based for consistency
            });
        }
    }
    Ok(columns)
}

async fn fetch_indexes_and_constraints(
    pool: &SqlitePool,
    tables: &[TableMeta],
) -> Result<(Vec<IndexMeta>, Vec<ConstraintMeta>), sqlx::Error> {
    let mut indexes = Vec::new();
    let mut constraints = Vec::new();

    for t in tables {
        // PRAGMA index_list(table): seq, name, unique
        let list_rows = sqlx::query_as::<_, (i32, String, i32)>("PRAGMA index_list(?)")
            .bind(&t.table_name)
            .fetch_all(pool)
            .await?;

        for (_seq, index_name, unique) in list_rows {
            // Skip auto-created indexes for PK/UNIQUE (sqlite_autoindex_*)
            let is_auto = index_name.starts_with("sqlite_autoindex_");
            if is_auto {
                constraints.push(ConstraintMeta {
                    schema_name: t.schema_name.clone(),
                    table_name: t.table_name.clone(),
                    constraint_name: index_name.clone(),
                    constraint_type: if unique != 0 {
                        "UNIQUE"
                    } else {
                        "PRIMARY KEY"
                    }
                    .to_string(),
                });
                continue;
            }

            // PRAGMA index_info(index_name): seqno, cid, name
            let col_rows = sqlx::query_as::<_, (i32, i32, String)>("PRAGMA index_info(?)")
                .bind(&index_name)
                .fetch_all(pool)
                .await?;

            let column_names: Vec<String> = col_rows
                .into_iter()
                .map(|(_seqno, _cid, name)| name)
                .collect();

            indexes.push(IndexMeta {
                schema_name: t.schema_name.clone(),
                table_name: t.table_name.clone(),
                index_name,
                column_names,
                is_unique: unique != 0,
            });
        }

        // PRIMARY KEY from table_info (pk column)
        let pragma_rows = sqlx::query_as::<_, (i32, String, String, i32, Option<String>, i32)>(
            "PRAGMA table_info(?)",
        )
        .bind(&t.table_name)
        .fetch_all(pool)
        .await?;
        let pk_cols: Vec<String> = pragma_rows
            .into_iter()
            .filter(|(_cid, _name, _ty, _notnull, _dflt, pk)| *pk != 0)
            .map(|(_cid, name, _ty, _notnull, _dflt, _pk)| name)
            .collect();
        if !pk_cols.is_empty() && !constraints.iter().any(|c| c.table_name == t.table_name && c.constraint_type == "PRIMARY KEY") {
            constraints.push(ConstraintMeta {
                schema_name: t.schema_name.clone(),
                table_name: t.table_name.clone(),
                constraint_name: format!("{}_pkey", t.table_name),
                constraint_type: "PRIMARY KEY".to_string(),
            });
        }
    }

    Ok((indexes, constraints))
}

async fn fetch_foreign_keys(
    pool: &SqlitePool,
    tables: &[TableMeta],
) -> Result<Vec<ForeignKeyRef>, sqlx::Error> {
    let mut foreign_keys = Vec::new();
    for t in tables {
        // PRAGMA foreign_key_list(table): id, seq, table, from, to, on_update, on_delete, match
        let rows = sqlx::query_as::<_, (i32, i32, String, String, String, String, String, String)>(
            "PRAGMA foreign_key_list(?)",
        )
        .bind(&t.table_name)
        .fetch_all(pool)
        .await?;

        // Group by id (each id is one FK constraint, possibly multi-column)
        let mut by_id: std::collections::HashMap<
            i32,
            (String, String, Vec<String>, String, Vec<String>),
        > = std::collections::HashMap::new();
        for (id, _seq, ref_table, from_col, to_col, _on_update, _on_delete, _match) in rows {
            let entry = by_id.entry(id).or_insert_with(|| {
                (
                    t.schema_name.clone(),
                    t.table_name.clone(),
                    Vec::new(),
                    ref_table,
                    Vec::new(),
                )
            });
            entry.2.push(from_col);
            entry.4.push(to_col);
        }

        for (_, (from_schema, from_table, from_columns, to_table, to_columns)) in by_id {
            foreign_keys.push(ForeignKeyRef {
                name: format!("fk_{}_{}", from_table, to_table),
                from_schema,
                from_table,
                from_columns,
                to_schema: SCHEMA_NAME.to_string(),
                to_table,
                to_columns,
            });
        }
    }
    Ok(foreign_keys)
}
