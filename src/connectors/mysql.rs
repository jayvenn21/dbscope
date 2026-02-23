//! MySQL connector: extracts schema from information_schema into [RawSchema].

use async_trait::async_trait;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

use crate::connectors::connector::{Connector, ConnectorError};
use crate::core::{
    ColumnMeta, ConstraintMeta, ForeignKeyRef, IndexMeta, RawSchema, TableMeta,
};

/// MySQL connector. Produces the same [RawSchema] as other engines.
#[derive(Debug, Clone, Default)]
pub struct MysqlConnector;

#[async_trait]
impl Connector for MysqlConnector {
    fn name(&self) -> &'static str {
        "mysql"
    }

    async fn extract_schema(&self, connection_uri: &str) -> Result<RawSchema, ConnectorError> {
        extract_schema(connection_uri).await.map_err(ConnectorError::Mysql)
    }
}

const EXCLUDED_SCHEMAS: &[&str] = &["mysql", "information_schema", "performance_schema", "sys"];

pub async fn extract_schema(connection_uri: &str) -> Result<RawSchema, sqlx::Error> {
    let pool = MySqlPoolOptions::new()
        .max_connections(2)
        .connect(connection_uri)
        .await?;

    let tables = fetch_tables(&pool).await?;
    let columns = fetch_columns(&pool).await?;
    let indexes = fetch_indexes(&pool).await?;
    let constraints = fetch_constraints(&pool).await?;
    let foreign_keys = fetch_foreign_keys(&pool).await?;

    pool.close().await;
    Ok(RawSchema {
        tables,
        views: Vec::new(),
        materialized_views: Vec::new(),
        columns,
        indexes,
        constraints,
        foreign_keys,
        engine_metadata: None,
    })
}

async fn fetch_tables(pool: &MySqlPool) -> Result<Vec<TableMeta>, sqlx::Error> {
    let placeholders = EXCLUDED_SCHEMAS.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let query = format!(
        r#"
        SELECT TABLE_SCHEMA, TABLE_NAME
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA NOT IN ({})
          AND TABLE_TYPE = 'BASE TABLE'
        ORDER BY TABLE_SCHEMA, TABLE_NAME
        "#,
        placeholders
    );
    let mut q = sqlx::query_as::<_, (String, String)>(&query);
    for s in EXCLUDED_SCHEMAS {
        q = q.bind(*s);
    }
    let rows = q.fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(schema_name, table_name)| TableMeta {
            schema_name,
            table_name,
        })
        .collect())
}

async fn fetch_columns(pool: &MySqlPool) -> Result<Vec<ColumnMeta>, sqlx::Error> {
    let placeholders = EXCLUDED_SCHEMAS.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let query = format!(
        r#"
        SELECT TABLE_SCHEMA, TABLE_NAME, COLUMN_NAME, COALESCE(DATA_TYPE, ''), ORDINAL_POSITION
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA NOT IN ({})
        ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION
        "#,
        placeholders
    );
    let mut q = sqlx::query_as::<_, (String, String, String, String, i32)>(&query);
    for s in EXCLUDED_SCHEMAS {
        q = q.bind(*s);
    }
    let rows = q.fetch_all(pool).await?;
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

async fn fetch_indexes(pool: &MySqlPool) -> Result<Vec<IndexMeta>, sqlx::Error> {
    let placeholders = EXCLUDED_SCHEMAS.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let query = format!(
        r#"
        SELECT TABLE_SCHEMA, TABLE_NAME, INDEX_NAME, MAX(NON_UNIQUE) = 0
        FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA NOT IN ({})
        GROUP BY TABLE_SCHEMA, TABLE_NAME, INDEX_NAME
        ORDER BY TABLE_SCHEMA, TABLE_NAME, INDEX_NAME
        "#,
        placeholders
    );
    let mut q = sqlx::query_as::<_, (String, String, String, u8)>(&query);
    for s in EXCLUDED_SCHEMAS {
        q = q.bind(*s);
    }
    let index_rows = q.fetch_all(pool).await?;

    let query_cols = format!(
        r#"
        SELECT TABLE_SCHEMA, TABLE_NAME, INDEX_NAME, COLUMN_NAME
        FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA NOT IN ({})
        ORDER BY TABLE_SCHEMA, TABLE_NAME, INDEX_NAME, SEQ_IN_INDEX
        "#,
        placeholders
    );
    let mut qc = sqlx::query_as::<_, (String, String, String, String)>(&query_cols);
    for s in EXCLUDED_SCHEMAS {
        qc = qc.bind(*s);
    }
    let col_rows = qc.fetch_all(pool).await?;

    let mut by_key: std::collections::HashMap<(String, String, String), Vec<String>> =
        std::collections::HashMap::new();
    for (schema, table, index, col) in col_rows {
        by_key
            .entry((schema, table, index))
            .or_default()
            .push(col);
    }

    let indexes = index_rows
        .into_iter()
        .map(|(schema_name, table_name, index_name, is_unique)| {
            let column_names = by_key
                .remove(&(schema_name.clone(), table_name.clone(), index_name.clone()))
                .unwrap_or_default();
            IndexMeta {
                schema_name,
                table_name,
                index_name,
                column_names,
                is_unique: is_unique != 0,
            }
        })
        .collect();
    Ok(indexes)
}

async fn fetch_constraints(pool: &MySqlPool) -> Result<Vec<ConstraintMeta>, sqlx::Error> {
    let placeholders = EXCLUDED_SCHEMAS.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let query = format!(
        r#"
        SELECT TABLE_SCHEMA, TABLE_NAME, CONSTRAINT_NAME, CONSTRAINT_TYPE
        FROM information_schema.TABLE_CONSTRAINTS
        WHERE TABLE_SCHEMA NOT IN ({})
          AND CONSTRAINT_TYPE IN ('PRIMARY KEY', 'UNIQUE', 'CHECK')
        ORDER BY TABLE_SCHEMA, TABLE_NAME, CONSTRAINT_NAME
        "#,
        placeholders
    );
    let mut q = sqlx::query_as::<_, (String, String, String, String)>(&query);
    for s in EXCLUDED_SCHEMAS {
        q = q.bind(*s);
    }
    let rows = q.fetch_all(pool).await?;
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

async fn fetch_foreign_keys(pool: &MySqlPool) -> Result<Vec<ForeignKeyRef>, sqlx::Error> {
    let placeholders = EXCLUDED_SCHEMAS.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let query = format!(
        r#"
        SELECT
            kcu.CONSTRAINT_NAME,
            kcu.TABLE_SCHEMA,
            kcu.TABLE_NAME,
            kcu.COLUMN_NAME,
            kcu.REFERENCED_TABLE_SCHEMA,
            kcu.REFERENCED_TABLE_NAME,
            kcu.REFERENCED_COLUMN_NAME
        FROM information_schema.KEY_COLUMN_USAGE kcu
        WHERE kcu.TABLE_SCHEMA NOT IN ({})
          AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
        ORDER BY kcu.TABLE_SCHEMA, kcu.TABLE_NAME, kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION
        "#,
        placeholders
    );
    let mut q = sqlx::query_as::<_, (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    )>(&query);
    for s in EXCLUDED_SCHEMAS {
        q = q.bind(*s);
    }
    let rows = q.fetch_all(pool).await?;

    type FkKey = (String, String, String);
    let mut by_constraint: std::collections::HashMap<
        FkKey,
        (String, String, String, Vec<String>, String, String, Vec<String>),
    > = std::collections::HashMap::new();
    for (
        name,
        from_schema,
        from_table,
        from_col,
        to_schema,
        to_table,
        to_col,
    ) in rows
    {
        let key = (from_schema.clone(), from_table.clone(), name.clone());
        let entry = by_constraint.entry(key).or_insert_with(|| {
            (
                name,
                from_schema,
                from_table,
                Vec::new(),
                to_schema,
                to_table,
                Vec::new(),
            )
        });
        entry.3.push(from_col);
        entry.6.push(to_col);
    }

    let foreign_keys = by_constraint
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
