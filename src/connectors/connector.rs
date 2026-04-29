//! Connector trait and error type. Each engine (Postgres, MySQL, SQLite, etc.)
//! implements [Connector] and normalizes its catalog into [crate::core::RawSchema].

use async_trait::async_trait;
use thiserror::Error;

use crate::core::RawSchema;

#[derive(Error, Debug)]
pub enum ConnectorError {
    #[error(
        "Unsupported database scheme: {0}. Use postgres://, mysql://, sqlite://, or clickhouse://."
    )]
    UnsupportedScheme(String),

    #[error("{0}")]
    Connection(String),

    #[error("Postgres: {0}")]
    Postgres(#[from] sqlx::Error),

    #[error("MySQL: {0}")]
    Mysql(sqlx::Error),

    #[error("SQLite: {0}")]
    Sqlite(sqlx::Error),

    #[error("ClickHouse: {0}")]
    Clickhouse(#[from] clickhouse::error::Error),
}

/// Database-agnostic schema extractor. Implementations connect to a given URI
/// and return the same canonical [RawSchema] regardless of engine.
#[async_trait]
pub trait Connector: Send + Sync {
    fn name(&self) -> &'static str;

    /// Extract full schema (tables, columns, indexes, constraints, FKs) into
    /// the universal model. Connection URI format is engine-specific.
    async fn extract_schema(&self, connection_uri: &str) -> Result<RawSchema, ConnectorError>;
}
