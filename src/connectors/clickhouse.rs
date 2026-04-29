//! ClickHouse connector: extracts schema from system.tables and system.columns into [RawSchema].
//! ClickHouse has no foreign keys or traditional constraints; indexes are represented from sorting/primary keys.

use async_trait::async_trait;
use clickhouse::Client;
use std::collections::HashMap;

use crate::connectors::connector::{Connector, ConnectorError};
use crate::core::{ColumnMeta, IndexMeta, RawSchema, TableMeta};

/// ClickHouse connector. Produces [RawSchema] with tables and columns; FKs are empty
/// (ClickHouse does not enforce referential integrity). Indexes are derived from sorting/primary keys.
#[derive(Debug, Clone, Default)]
pub struct ClickhouseConnector;

#[async_trait]
impl Connector for ClickhouseConnector {
    fn name(&self) -> &'static str {
        "clickhouse"
    }

    async fn extract_schema(&self, connection_uri: &str) -> Result<RawSchema, ConnectorError> {
        extract_schema(connection_uri).await
    }
}

/// Parse clickhouse://user:pass@host:port/database into URL and options.
fn parse_uri(uri: &str) -> Result<(String, String, String, String), ConnectorError> {
    let rest = uri
        .strip_prefix("clickhouse://")
        .ok_or_else(|| ConnectorError::UnsupportedScheme("expected clickhouse://".to_string()))?;
    let (auth, host_db) = rest.split_once('@').unwrap_or(("", rest));
    let (user, password) = if auth.is_empty() {
        ("default".to_string(), String::new())
    } else {
        let (u, p) = auth.split_once(':').unwrap_or((auth, ""));
        (u.to_string(), p.to_string())
    };
    let (host_port, database) = host_db.split_once('/').unwrap_or((host_db, "default"));
    let url = if host_port.contains("://") {
        host_port.to_string()
    } else {
        format!("http://{}", host_port)
    };
    Ok((url, user, password, database.to_string()))
}

pub async fn extract_schema(connection_uri: &str) -> Result<RawSchema, ConnectorError> {
    let (url, user, password, database) = parse_uri(connection_uri)?;

    let client = Client::default()
        .with_url(url)
        .with_database(database)
        .with_user(user)
        .with_password(password);

    let tables = fetch_tables(&client).await?;
    let views = fetch_views(&client).await?;
    let materialized_views = fetch_materialized_views(&client).await?;
    let columns = fetch_columns(&client).await?;
    let indexes = fetch_indexes(&client, &tables).await?;

    Ok(RawSchema {
        tables,
        views,
        materialized_views,
        columns,
        indexes,
        constraints: Vec::new(),
        foreign_keys: Vec::new(),
        table_stats: None,
        engine_metadata: None,
    })
}

async fn fetch_tables(client: &Client) -> Result<Vec<TableMeta>, ConnectorError> {
    let rows = client
        .query(
            "SELECT database, name FROM system.tables WHERE database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema') AND engine NOT IN ('View', 'MaterializedView') ORDER BY database, name",
        )
        .fetch_all::<(String, String)>()
        .await?;
    Ok(rows
        .into_iter()
        .map(|(schema_name, table_name)| TableMeta {
            schema_name,
            table_name,
        })
        .collect())
}

async fn fetch_views(client: &Client) -> Result<Vec<TableMeta>, ConnectorError> {
    let rows = client
        .query(
            "SELECT database, name FROM system.tables WHERE database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema') AND engine = 'View' ORDER BY database, name",
        )
        .fetch_all::<(String, String)>()
        .await?;
    Ok(rows
        .into_iter()
        .map(|(schema_name, table_name)| TableMeta {
            schema_name,
            table_name,
        })
        .collect())
}

async fn fetch_materialized_views(client: &Client) -> Result<Vec<TableMeta>, ConnectorError> {
    let rows = client
        .query(
            "SELECT database, name FROM system.tables WHERE database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema') AND engine = 'MaterializedView' ORDER BY database, name",
        )
        .fetch_all::<(String, String)>()
        .await?;
    Ok(rows
        .into_iter()
        .map(|(schema_name, table_name)| TableMeta {
            schema_name,
            table_name,
        })
        .collect())
}

async fn fetch_columns(client: &Client) -> Result<Vec<ColumnMeta>, ConnectorError> {
    let rows = client
        .query(
            "SELECT database, table, name, type, position FROM system.columns WHERE database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema') ORDER BY database, table, position",
        )
        .fetch_all::<(String, String, String, String, u64)>()
        .await?;
    Ok(rows
        .into_iter()
        .map(
            |(schema_name, table_name, column_name, data_type, position)| ColumnMeta {
                schema_name,
                table_name,
                column_name,
                is_nullable: Some(data_type.starts_with("Nullable")),
                data_type,
                ordinal_position: position as i32,
                default_value: None,
            },
        )
        .collect())
}

async fn fetch_indexes(
    client: &Client,
    tables: &[TableMeta],
) -> Result<Vec<IndexMeta>, ConnectorError> {
    // system.tables has sorting_key, primary_key; we treat them as one logical "index" per table
    let rows = client
        .query(
            "SELECT database, name, primary_key, sorting_key FROM system.tables WHERE database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema')",
        )
        .fetch_all::<(String, String, String, String)>()
        .await?;

    let mut key_exprs: HashMap<(String, String), (String, String)> = HashMap::new();
    for (db, name, pk, sk) in rows {
        key_exprs.insert((db.clone(), name.clone()), (pk, sk));
    }

    let mut indexes = Vec::new();
    for t in tables {
        let key = (t.schema_name.clone(), t.table_name.clone());
        if let Some((primary_key, sorting_key)) = key_exprs.get(&key) {
            // Parse key expression: often "col1, col2" or "tuple(col1, col2)"
            let cols = parse_key_columns(primary_key).or_else(|| parse_key_columns(sorting_key));
            if let Some(column_names) = cols {
                if !column_names.is_empty() {
                    indexes.push(IndexMeta {
                        schema_name: t.schema_name.clone(),
                        table_name: t.table_name.clone(),
                        index_name: format!("{}_sorting_key", t.table_name),
                        column_names,
                        is_unique: false,
                    });
                }
            }
        }
    }
    Ok(indexes)
}

/// Heuristic: extract column names from ClickHouse key expression (e.g. "a, b" or "tuple(a, b)").
fn parse_key_columns(expr: &str) -> Option<Vec<String>> {
    let expr = expr.trim();
    if expr.is_empty() || expr == "tuple()" {
        return Some(Vec::new());
    }
    let inner = expr
        .strip_prefix("tuple(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(expr);
    let cols: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Some(cols)
}
