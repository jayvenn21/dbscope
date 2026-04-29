//! CLI orchestration only. No business logic.

mod analyze;
mod ci;
pub mod completions;
pub mod demo;
pub mod deps;
pub mod diff;
mod explain;
mod impact;
pub mod lint;
pub mod mcp;
mod plan;
mod preview;
pub mod snapshot;
pub mod style;
mod summarize;

pub use analyze::run_analyze;
pub use ci::run_ci;
pub use explain::run_explain;
pub use impact::run_impact;
pub use plan::run_plan_drop;
pub use preview::run_preview;
pub use summarize::run_summarize;
