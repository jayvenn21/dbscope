//! Policy engine for CI and preview: max risk, no cycles, no orphans, max blast radius.

use std::path::Path;

/// Policy for schema and migration checks. Load from dbscope.policy.yaml or --policy.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Policy {
    /// Fail if any table risk score exceeds this (0–1).
    #[serde(default = "default_max_table_risk")]
    pub max_table_risk: f64,
    /// Fail if any table is in a circular FK dependency.
    #[serde(default)]
    pub no_cycles: bool,
    /// Fail if any table is an orphan (no FK in/out).
    #[serde(default)]
    pub no_orphans: bool,
    /// Fail if blast radius (impacted tables %) exceeds this (0–100).
    #[serde(default = "default_max_blast_radius")]
    pub max_blast_radius_percent: f64,
}

fn default_max_table_risk() -> f64 {
    0.5
}
fn default_max_blast_radius() -> f64 {
    100.0
}

impl Policy {
    /// Load policy from YAML file. Returns default policy on error or missing file.
    pub fn load(path: &Path) -> Self {
        let s = match std::fs::read_to_string(path) {
            Ok(x) => x,
            Err(_) => return Self::default(),
        };
        serde_yaml::from_str(&s).unwrap_or_default()
    }
}
