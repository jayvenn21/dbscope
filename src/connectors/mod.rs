pub mod clickhouse;
mod connector;
pub mod mysql;
pub mod postgres;
pub mod query_log;
pub mod sqlite;

pub use connector::{Connector, ConnectorError};

use crate::core::RawSchema;

/// Extract schema from any supported database URI. Dispatches by scheme:
/// - `postgres://` / `postgresql://` → Postgres
/// - `mysql://` → MySQL
/// - `sqlite://` / `file://` (path ending in .db/.sqlite) → SQLite
/// - `clickhouse://` → ClickHouse
///
/// Wraps raw connection errors with actionable diagnostics.
pub async fn extract_schema(connection_uri: &str) -> Result<RawSchema, ConnectorError> {
    let uri = connection_uri.trim();
    if uri.is_empty() {
        return Err(ConnectorError::Connection(
            "Connection URI is empty. Provide a URI like: postgres://user:pass@localhost:5432/dbname".into(),
        ));
    }
    if !uri.contains("://") {
        return Err(ConnectorError::Connection(format!(
            "Invalid URI '{}': missing scheme. Expected: postgres://, mysql://, sqlite://, or clickhouse://",
            uri
        )));
    }

    let scheme = uri.split("://").next().unwrap_or_default().to_lowercase();
    let connector: Box<dyn Connector> = match scheme.as_str() {
        "postgres" | "postgresql" => Box::new(postgres::PostgresConnector),
        "mysql" => Box::new(mysql::MysqlConnector),
        "sqlite" | "file" => Box::new(sqlite::SqliteConnector),
        "clickhouse" => Box::new(clickhouse::ClickhouseConnector),
        _ => return Err(ConnectorError::UnsupportedScheme(scheme)),
    };

    connector.extract_schema(uri).await.map_err(|e| {
        let engine = connector.name();
        let hint = match &e {
            ConnectorError::Postgres(sqlx_err) => connection_hint(engine, uri, sqlx_err),
            ConnectorError::Mysql(sqlx_err) => connection_hint(engine, uri, sqlx_err),
            ConnectorError::Sqlite(sqlx_err) => connection_hint(engine, uri, sqlx_err),
            _ => None,
        };
        if let Some(hint) = hint {
            ConnectorError::Connection(format!("{e}\n\n  Hint: {hint}"))
        } else {
            e
        }
    })
}

fn connection_hint(engine: &str, uri: &str, err: &sqlx::Error) -> Option<String> {
    let msg = format!("{err}");
    let lower = msg.to_lowercase();

    if lower.contains("connection refused") || lower.contains("connect") && lower.contains("error")
    {
        let host = uri
            .split("://")
            .nth(1)
            .and_then(|s| s.split('@').next_back())
            .and_then(|s| s.split('/').next())
            .unwrap_or("localhost");
        return Some(format!(
            "Cannot connect to {} at {}. Is the server running and accepting connections?",
            engine, host
        ));
    }

    if lower.contains("password") || lower.contains("authentication") || lower.contains("denied") {
        return Some(format!(
            "Authentication failed for {}. Check your username and password in the connection URI.",
            engine
        ));
    }

    if lower.contains("does not exist") || lower.contains("unknown database") {
        return Some(format!(
            "Database not found. Check the database name in your connection URI for {}.",
            engine
        ));
    }

    if lower.contains("timeout") || lower.contains("timed out") {
        return Some(format!(
            "Connection to {} timed out. Check that the host and port are correct and reachable.",
            engine
        ));
    }

    None
}
