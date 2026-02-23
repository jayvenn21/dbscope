//! Postgres connector: extracts schema via information_schema and pg_catalog into
//! the universal [RawSchema]. Read-only.

use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::connectors::connector::{Connector, ConnectorError};
use crate::core::{
    ColumnMeta, ConstraintMeta, ForeignKeyRef, IndexMeta, RawSchema, TableMeta, TableStats,
};

/// Postgres connector. Produces the same [RawSchema] as other engines.
#[derive(Debug, Clone, Default)]
pub struct PostgresConnector;

#[async_trait]
impl Connector for PostgresConnector {
    fn name(&self) -> &'static str {
        "postgres"
    }

    async fn extract_schema(&self, connection_uri: &str) -> Result<RawSchema, ConnectorError> {
        extract_schema(connection_uri).await.map_err(ConnectorError::from)
    }
}

/// Connect and extract full schema. Excludes system schemas (pg_*, information_schema)
/// unless explicitly included.
pub async fn extract_schema(connection_uri: &str) -> Result<RawSchema, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(connection_uri)
        .await?;

    let schema = extract_schema_from_pool(&pool).await?;
    pool.close().await;
    Ok(schema)
}

async fn extract_schema_from_pool(pool: &PgPool) -> Result<RawSchema, sqlx::Error> {
    let tables = fetch_tables(pool).await?;
    let columns = fetch_columns(pool).await?;
    let indexes = fetch_indexes(pool).await?;
    let constraints = fetch_constraints(pool).await?;
    let foreign_keys = fetch_foreign_keys(pool).await?;
    let table_stats = fetch_table_stats(pool).await?;

    Ok(RawSchema {
        tables,
        views: Vec::new(),
        materialized_views: Vec::new(),
        columns,
        indexes,
        constraints,
        foreign_keys,
        table_stats,
        engine_metadata: None,
    })
}

async fn fetch_tables(pool: &PgPool) -> Result<Vec<TableMeta>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT table_schema, table_name
        FROM information_schema.tables
        WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
          AND table_type = 'BASE TABLE'
        ORDER BY table_schema, table_name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(schema_name, table_name)| TableMeta {
            schema_name,
            table_name,
        })
        .collect())
}

async fn fetch_columns(pool: &PgPool) -> Result<Vec<ColumnMeta>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i32)>(
        r#"
        SELECT table_schema, table_name, column_name, data_type, ordinal_position::int4
        FROM information_schema.columns
        WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
        ORDER BY table_schema, table_name, ordinal_position
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(schema_name, table_name, column_name, data_type, ordinal_position)| {
            ColumnMeta {
                schema_name,
                table_name,
                column_name,
                data_type,
                ordinal_position,
            }
        })
        .collect())
}

async fn fetch_indexes(pool: &PgPool) -> Result<Vec<IndexMeta>, sqlx::Error> {
    // pg_indexes + index columns from pg_index / pg_attribute
    let rows = sqlx::query_as::<_, (String, String, String, bool)>(
        r#"
        SELECT schemaname, tablename, indexname, indexdef LIKE '%UNIQUE%'
        FROM pg_indexes
        WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY schemaname, tablename, indexname
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut indexes = Vec::new();
    for (schema_name, table_name, index_name, is_unique) in rows {
        let column_names = fetch_index_columns(pool, &schema_name, &index_name).await?;
        indexes.push(IndexMeta {
            schema_name,
            table_name,
            index_name,
            column_names,
            is_unique,
        });
    }
    Ok(indexes)
}

async fn fetch_index_columns(
    pool: &PgPool,
    schema_name: &str,
    index_name: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT a.attname
        FROM pg_index i
        JOIN pg_class t ON t.oid = i.indrelid
        JOIN pg_namespace n ON n.oid = t.relnamespace AND n.nspname = $1
        JOIN pg_class c ON c.oid = i.indexrelid AND c.relname = $2
        JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(i.indkey) AND a.attnum > 0 AND NOT a.attisdropped
        ORDER BY array_position(i.indkey, a.attnum)
        "#,
    )
    .bind(schema_name)
    .bind(index_name)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(c,)| c).collect())
}

async fn fetch_constraints(pool: &PgPool) -> Result<Vec<ConstraintMeta>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT table_schema, table_name, constraint_name, constraint_type
        FROM information_schema.table_constraints
        WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
          AND constraint_type IN ('PRIMARY KEY', 'UNIQUE', 'CHECK')
        ORDER BY table_schema, table_name, constraint_name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(schema_name, table_name, constraint_name, constraint_type)| ConstraintMeta {
                schema_name,
                table_name,
                constraint_name,
                constraint_type,
            },
        )
        .collect())
}

async fn fetch_foreign_keys(pool: &PgPool) -> Result<Vec<ForeignKeyRef>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )>(
        r#"
        SELECT
            tc.constraint_name,
            tc.table_schema   AS from_schema,
            tc.table_name     AS from_table,
            kcu.column_name   AS from_column,
            ccu.table_schema  AS to_schema,
            ccu.table_name    AS to_table,
            ccu.column_name   AS to_column
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
          ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema
        JOIN information_schema.constraint_column_usage ccu
          ON tc.constraint_name = ccu.constraint_name AND tc.table_schema = ccu.table_schema
        WHERE tc.constraint_type = 'FOREIGN KEY'
          AND tc.table_schema NOT IN ('pg_catalog', 'information_schema')
        ORDER BY tc.table_schema, tc.table_name, kcu.ordinal_position
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Group by constraint name to build (from_columns, to_columns) lists
    let mut by_name: std::collections::HashMap<
        String,
        (String, String, String, Vec<String>, String, String, Vec<String>),
    > = std::collections::HashMap::new();
    for (
        constraint_name,
        from_schema,
        from_table,
        from_column,
        to_schema,
        to_table,
        to_column,
    ) in rows
    {
        let key = format!("{}.{}.{}", from_schema, from_table, constraint_name);
        let entry = by_name.entry(key).or_insert_with(|| {
            (
                constraint_name,
                from_schema,
                from_table,
                Vec::new(),
                to_schema,
                to_table,
                Vec::new(),
            )
        });
        entry.3.push(from_column);
        entry.6.push(to_column);
    }

    let foreign_keys = by_name
        .into_values()
        .map(
            |(name, from_schema, from_table, from_columns, to_schema, to_table, to_columns)| {
                ForeignKeyRef {
                    name,
                    from_schema,
                    from_table,
                    from_columns,
                    to_schema,
                    to_table,
                    to_columns,
                }
            },
        )
        .collect();
    Ok(foreign_keys)
}

async fn fetch_table_stats(pool: &PgPool) -> Result<Option<Vec<TableStats>>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, i64, i64)>(
        r#"
        SELECT schemaname, relname, n_live_tup, n_tup_ins, n_tup_upd, n_tup_del
        FROM pg_stat_user_tables
        WHERE schemaname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY schemaname, relname
        "#,
    )
    .fetch_all(pool)
    .await?;

    let table_stats: Vec<TableStats> = rows
        .into_iter()
        .map(|(schema_name, table_name, n_live_tup, n_tup_ins, n_tup_upd, n_tup_del)| {
            TableStats {
                schema_name,
                table_name,
                row_estimate: n_live_tup.max(0) as u64,
                n_tup_ins: n_tup_ins.max(0) as u64,
                n_tup_upd: n_tup_upd.max(0) as u64,
                n_tup_del: n_tup_del.max(0) as u64,
            }
        })
        .collect();

    Ok(Some(table_stats))
}
