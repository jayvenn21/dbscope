mod connector;
pub mod mysql;
pub mod postgres;
pub mod query_log;
pub mod sqlite;
pub mod clickhouse;

pub use connector::{Connector, ConnectorError};

use crate::core::RawSchema;

/// Extract schema from any supported database URI. Dispatches by scheme:
/// - `postgres://` / `postgresql://` → Postgres
/// - `mysql://` → MySQL
/// - `sqlite://` / `file://` (path ending in .db/.sqlite) → SQLite
/// - `clickhouse://` → ClickHouse
pub async fn extract_schema(connection_uri: &str) -> Result<RawSchema, ConnectorError> {
    let scheme = connection_uri
        .split("://")
        .next()
        .unwrap_or_default()
        .to_lowercase();
    let connector: Box<dyn Connector> = match scheme.as_str() {
        "postgres" | "postgresql" => Box::new(postgres::PostgresConnector),
        "mysql" => Box::new(mysql::MysqlConnector),
        "sqlite" | "file" => Box::new(sqlite::SqliteConnector),
        "clickhouse" => Box::new(clickhouse::ClickhouseConnector),
        _ => return Err(ConnectorError::UnsupportedScheme(scheme)),
    };
    connector.extract_schema(connection_uri).await
}
