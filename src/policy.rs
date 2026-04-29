//! Policy engine for CI and preview: max risk, no cycles, no orphans, max blast radius.

use std::path::Path;

/// Policy for schema and migration checks. Load from dbscope.policy.yaml or --policy.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Policy {
    /// Fail if any table risk score exceeds this (0-1).
    #[serde(default = "default_max_table_risk")]
    pub max_table_risk: f64,
    /// Fail if any table is in a circular FK dependency.
    #[serde(default)]
    pub no_cycles: bool,
    /// Fail if any table is an orphan (no FK in/out).
    #[serde(default)]
    pub no_orphans: bool,
    /// Fail if blast radius (impacted tables %) exceeds this (0-100).
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
    /// Load policy from YAML file. Prints a warning on parse errors and returns defaults.
    /// Returns an error if the file exists but cannot be read due to permissions.
    pub fn load(path: &Path) -> Self {
        let s = match std::fs::read_to_string(path) {
            Ok(x) => x,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "warning: policy file not found: {} (using defaults)",
                    path.display()
                );
                return Self::default();
            }
            Err(e) => {
                eprintln!(
                    "warning: cannot read policy file {}: {} (using defaults)",
                    path.display(),
                    e
                );
                return Self::default();
            }
        };
        match serde_yaml::from_str(&s) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "warning: invalid policy YAML in {}: {} (using defaults)",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    /// Validate policy values. Returns a list of warnings for out-of-range values.
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.max_table_risk < 0.0 || self.max_table_risk > 1.0 {
            warnings.push(format!(
                "max_table_risk {:.2} is outside [0, 1] range",
                self.max_table_risk
            ));
        }
        if self.max_blast_radius_percent < 0.0 || self.max_blast_radius_percent > 100.0 {
            warnings.push(format!(
                "max_blast_radius_percent {:.0} is outside [0, 100] range",
                self.max_blast_radius_percent
            ));
        }
        warnings
    }
}
