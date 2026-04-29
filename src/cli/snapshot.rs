//! `dbscope snapshot`: serialize the current schema to JSON for offline analysis,
//! diffing, trending, and audit trails.

use crate::connectors::extract_schema;
use crate::core::RawSchema;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// On-disk schema snapshot with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSnapshot {
    pub version: u32,
    pub created_at: String,
    pub source_uri_hash: String,
    pub schema: RawSchema,
}

impl SchemaSnapshot {
    pub fn new(schema: RawSchema, source_uri: &str) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        source_uri.hash(&mut hasher);
        let hash = format!("{:016x}", hasher.finish());
        Self {
            version: 1,
            created_at: Utc::now().to_rfc3339(),
            source_uri_hash: hash,
            schema,
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), anyhow::Error> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, anyhow::Error> {
        let data = std::fs::read_to_string(path)?;
        let snap: Self = serde_json::from_str(&data)?;
        Ok(snap)
    }
}

pub async fn run_snapshot(schema_uri: &str, output: &Path) -> Result<(), anyhow::Error> {
    let raw = extract_schema(schema_uri).await?;
    let snap = SchemaSnapshot::new(raw, schema_uri);
    snap.save(output)?;
    let table_count = snap.schema.tables.len();
    let fk_count = snap.schema.foreign_keys.len();
    eprintln!(
        "Snapshot saved: {} ({} tables, {} FKs, {})",
        output.display(),
        table_count,
        fk_count,
        snap.created_at
    );
    Ok(())
}
