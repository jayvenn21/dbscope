//! Read query log: one SQL statement per line (or blank-line separated).
//! Returns Vec of query strings for parsing.

use std::path::Path;

/// Read queries from a file. Each non-empty line is treated as one SQL statement.
/// Lines starting with -- are skipped. Empty lines separate statements if we want to support multi-line later.
pub fn read_query_log(path: &Path) -> std::io::Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let queries: Vec<String> = content
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with("--"))
        .map(String::from)
        .collect();
    Ok(queries)
}
