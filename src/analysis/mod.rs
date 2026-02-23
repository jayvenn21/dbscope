//! Metrics computed from the unified graph: FK depth, orphans, cycles,
//! centrality, risk score per table. Phase 2: usage. Phase 3: impact (blast radius).

mod impact;
mod metrics;
mod usage;

pub use impact::{compute_impact, count_queries_affected, ImpactReport, ImpactTarget};
pub use metrics::{
    compute_all_metrics, compute_all_metrics_with_operational, RiskScoreBreakdown, TableMetrics,
    TableRisk,
};
pub use usage::{
    build_usage_from_queries, compute_usage_report, ColdColumn, ColdTable, HotTable,
    IndexSuggestion, JoinHotspot, UsageReport,
};
