//! Static HTML report: overview dashboard, risk table, cold/orphan sections.
//! No server; open locally or publish as CI artifact.

use std::collections::HashMap;
use std::io::Write;

use crate::analysis::{TableMetrics, TableRisk, UsageReport};

fn schema_complexity(total_tables: usize, total_fks: usize) -> f64 {
    if total_tables == 0 {
        return 0.0;
    }
    let n = total_tables as f64;
    let f = total_fks as f64;
    ((n * 0.02 + f * 0.05).min(1.0) * 100.0).round() / 100.0
}

fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

pub fn render<W: Write>(
    w: &mut W,
    metrics: &[TableMetrics],
    total_tables: usize,
    total_columns: usize,
    total_indexes: usize,
    total_fks: usize,
    usage: Option<&UsageReport>,
) -> std::io::Result<()> {
    let overall_risk = if metrics.is_empty() {
        0.0
    } else {
        metrics.iter().map(|m| m.display_risk()).sum::<f64>() / metrics.len() as f64
    };
    let _critical = metrics
        .iter()
        .filter(|m| TableRisk::from_score(m.display_risk()) == TableRisk::Critical)
        .count();
    let _high = metrics
        .iter()
        .filter(|m| TableRisk::from_score(m.display_risk()) == TableRisk::High)
        .count();
    let orphans: Vec<_> = metrics.iter().filter(|m| m.is_orphan).collect();
    let in_cycle: Vec<_> = metrics.iter().filter(|m| m.in_cycle).collect();
    let mut sorted: Vec<&TableMetrics> = metrics.iter().collect();
    sorted.sort_by(|a, b| {
        b.display_risk()
            .partial_cmp(&a.display_risk())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let queries_card = usage
        .map(|u| {
            format!(
                r#"<div class="stat"><span class="stat-val">{}</span> <span class="stat-label">queries</span></div>"#,
                u.total_queries_parsed
            )
        })
        .unwrap_or_default();
    let complexity = schema_complexity(total_tables, total_fks);
    let hotness_map: HashMap<String, u64> = usage
        .map(|u| {
            u.hot_tables
                .iter()
                .map(|h| (h.qualified_name.clone(), h.query_count))
                .collect()
        })
        .unwrap_or_default();

    writeln!(
        w,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>dbscope</title>
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{
      font-family: 'IBM Plex Sans', -apple-system, BlinkMacSystemFont, sans-serif;
      font-size: 14px;
      color: #1a1a2e;
      background: #fff;
      line-height: 1.5;
      -webkit-font-smoothing: antialiased;
    }}
    .wrap {{
      max-width: 960px;
      margin: 0 auto;
      padding: 48px 32px;
    }}
    .mark {{
      font-size: 13px;
      font-weight: 600;
      letter-spacing: 0.02em;
      color: #4F46E5;
      margin-bottom: 4px;
    }}
    h1 {{
      font-size: 20px;
      font-weight: 500;
      color: #1a1a2e;
      margin-bottom: 32px;
    }}
    .stats {{
      display: flex;
      gap: 32px;
      flex-wrap: wrap;
      margin-bottom: 40px;
      padding-bottom: 24px;
      border-bottom: 1px solid #eaeaea;
    }}
    .stat {{
      display: flex;
      align-items: baseline;
      gap: 6px;
    }}
    .stat-val {{
      font-size: 22px;
      font-weight: 600;
      color: #1a1a2e;
      font-variant-numeric: tabular-nums;
    }}
    .stat-label {{
      font-size: 12px;
      color: #6b7280;
    }}
    h2 {{
      font-size: 13px;
      font-weight: 600;
      color: #6b7280;
      margin-bottom: 12px;
      margin-top: 32px;
    }}
    .note {{
      font-size: 12px;
      color: #9ca3af;
      margin-bottom: 12px;
    }}
    .risk-critical {{ color: #dc2626; font-weight: 600; }}
    .risk-high {{ color: #ea580c; font-weight: 600; }}
    .risk-medium {{ color: #d97706; font-weight: 600; }}
    .risk-low {{ color: #16a34a; font-weight: 600; }}
    table {{
      width: 100%;
      border-collapse: collapse;
      font-size: 13px;
      margin-bottom: 32px;
    }}
    th {{
      text-align: left;
      padding: 8px 12px;
      font-weight: 500;
      font-size: 11px;
      color: #9ca3af;
      border-bottom: 1px solid #eaeaea;
      cursor: pointer;
      user-select: none;
    }}
    th:hover {{ color: #4F46E5; }}
    td {{
      padding: 7px 12px;
      border-bottom: 1px solid #f5f5f5;
      color: #374151;
    }}
    tr:hover td {{ background: #fafafa; }}
    td:first-child {{
      font-family: 'IBM Plex Mono', 'SF Mono', monospace;
      font-size: 12px;
      color: #1a1a2e;
    }}
    .sort-asc::after {{ content: " \u2191"; color: #4F46E5; }}
    .sort-desc::after {{ content: " \u2193"; color: #4F46E5; }}
    input[type="search"] {{
      border: 1px solid #e5e7eb;
      border-radius: 4px;
      padding: 6px 10px;
      font-size: 13px;
      color: #374151;
      width: 240px;
      margin-bottom: 12px;
      font-family: inherit;
    }}
    input[type="search"]::placeholder {{ color: #9ca3af; }}
    input[type="search"]:focus {{
      outline: none;
      border-color: #4F46E5;
    }}
    ul {{ list-style: none; }}
    ul li {{
      font-family: 'IBM Plex Mono', 'SF Mono', monospace;
      font-size: 12px;
      color: #6b7280;
      padding: 3px 0;
    }}
    code {{
      font-family: 'IBM Plex Mono', 'SF Mono', monospace;
      font-size: 12px;
      background: #f3f4f6;
      padding: 2px 5px;
      border-radius: 3px;
    }}
    .hidden {{ display: none; }}
  </style>
</head>
<body>
<div class="wrap">
  <div class="mark">dbscope</div>
  <h1>Schema Report</h1>

  <div class="stats">
    <div class="stat"><span class="stat-val">{}</span> <span class="stat-label">tables</span></div>
    <div class="stat"><span class="stat-val">{}</span> <span class="stat-label">columns</span></div>
    <div class="stat"><span class="stat-val">{}</span> <span class="stat-label">indexes</span></div>
    <div class="stat"><span class="stat-val">{}</span> <span class="stat-label">foreign keys</span></div>
    <div class="stat"><span class="stat-val">{:.2}</span> <span class="stat-label">risk</span></div>
    <div class="stat"><span class="stat-val">{:.2}</span> <span class="stat-label">complexity</span></div>
    {}
  </div>

  <h2>Dependency graph</h2>
  <p class="note">FK graph: <code>dbscope-graph.dot</code> &mdash; render with <code>dot -Tsvg dbscope-graph.dot -o graph.svg</code></p>
"#,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        overall_risk,
        complexity,
        queries_card,
    )?;
    writeln!(
        w,
        r#"
  <h2>Risk scoring</h2>
  <p class="note">risk = depth (max 0.4) + cycle (0.3) + centrality (max 0.3). Orphans = 0.</p>

  <h2>Risk table</h2>
  <input type="search" id="table-search" placeholder="Filter..." aria-label="Filter tables">
  <table id="risk-table">
    <thead>
      <tr>
        <th>Table</th>
        <th>Centrality (in/out)</th>
        <th>FK depth (out/in)</th>
        <th>Orphan</th>
        <th>Cycle</th>
        <th>Risk</th>"#
    )?;
    if usage.is_some() {
        writeln!(w, r#"        <th>Hotness</th>"#)?;
    }
    writeln!(
        w,
        r#"
      </tr>
    </thead>
    <tbody>"#
    )?;

    for m in &sorted {
        let risk = TableRisk::from_score(m.display_risk());
        let risk_class = match risk {
            TableRisk::Critical => "risk-critical",
            TableRisk::High => "risk-high",
            TableRisk::Medium => "risk-medium",
            TableRisk::Low => "risk-low",
        };
        let hotness_cell = if usage.is_some() {
            hotness_map
                .get(&m.qualified_name)
                .map(|c| format!("{}", c))
                .unwrap_or_else(|| "-".to_string())
        } else {
            String::new()
        };
        write!(
            w,
            r#"      <tr><td>{}</td><td>{} / {}</td><td>{} / {}</td><td>{}</td><td>{}</td><td class="{}">{}</td>"#,
            escape(&m.qualified_name),
            m.centrality_in,
            m.centrality_out,
            m.fk_depth_out,
            m.fk_depth_in,
            if m.is_orphan { "yes" } else { "-" },
            if m.in_cycle { "yes" } else { "-" },
            risk_class,
            risk.label(),
        )?;
        if usage.is_some() {
            write!(w, "<td>{}</td>", escape(&hotness_cell))?;
        }
        writeln!(w, "</tr>")?;
    }

    writeln!(
        w,
        r#"    </tbody>
  </table>"#
    )?;

    if !orphans.is_empty() {
        writeln!(
            w,
            r#"
  <h2>Orphan tables</h2>
  <p class="note">No FK references in or out.</p>
  <ul>"#
        )?;
        for m in orphans {
            writeln!(w, "    <li>{}</li>", escape(&m.qualified_name))?;
        }
        writeln!(w, "  </ul>")?;
    }

    if !in_cycle.is_empty() {
        writeln!(
            w,
            r#"
  <h2>Circular dependencies</h2>
  <ul>"#
        )?;
        for m in in_cycle {
            writeln!(w, "    <li>{}</li>", escape(&m.qualified_name))?;
        }
        writeln!(w, "  </ul>")?;
    }

    if let Some(u) = usage {
        if !u.cold_tables.is_empty() {
            writeln!(
                w,
                r#"
  <h2>Cold tables</h2>
  <p class="note">Never queried.</p>
  <ul>"#
            )?;
            for ct in &u.cold_tables {
                writeln!(w, "    <li>{}</li>", escape(&ct.0))?;
            }
            writeln!(w, "  </ul>")?;
        }
        if !u.cold_columns.is_empty() {
            writeln!(
                w,
                r#"
  <h2>Cold columns</h2>
  <p class="note">Never referenced.</p>
  <ul>"#
            )?;
            for c in u.cold_columns.iter().take(100) {
                writeln!(
                    w,
                    "    <li>{}.{}</li>",
                    escape(&c.qualified_table),
                    escape(&c.column_name)
                )?;
            }
            writeln!(w, "  </ul>")?;
        }
        if !u.index_suggestions.is_empty() {
            writeln!(
                w,
                r#"
  <h2>Index suggestions</h2>
  <p class="note">Columns in WHERE without a covering index.</p>
  <table>
    <thead><tr><th>Table</th><th>Column</th><th>WHERE count</th></tr></thead>
    <tbody>"#
            )?;
            for s in u.index_suggestions.iter().take(30) {
                writeln!(
                    w,
                    "      <tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape(&s.qualified_table),
                    escape(&s.column_name),
                    s.in_where_count
                )?;
            }
            writeln!(w, "    </tbody>\n  </table>")?;
        }
    }

    writeln!(
        w,
        r#"
</div>
<script>
  (function(){{
    var table=document.getElementById("risk-table");
    if(!table)return;
    var thead=table.querySelector("thead");
    var tbody=table.querySelector("tbody");
    var headers=thead.querySelectorAll("th");
    var rows=Array.from(tbody.querySelectorAll("tr"));
    var sortCol=-1,sortAsc=true;
    function parseVal(td,i){{
      var t=td.textContent.trim();
      if(i===0)return t.toLowerCase();
      var n=parseFloat(t.replace(/[^0-9.\-]/g,""));
      return isNaN(n)?t.toLowerCase():n;
    }}
    headers.forEach(function(h,i){{
      h.addEventListener("click",function(){{
        if(sortCol===i)sortAsc=!sortAsc;else{{sortCol=i;sortAsc=true;}}
        headers.forEach(function(x){{x.classList.remove("sort-asc","sort-desc");}});
        h.classList.add(sortAsc?"sort-asc":"sort-desc");
        rows.sort(function(a,b){{
          var va=parseVal(a.cells[i],i),vb=parseVal(b.cells[i],i);
          if(va<vb)return sortAsc?-1:1;
          if(va>vb)return sortAsc?1:-1;
          return 0;
        }});
        rows.forEach(function(r){{tbody.appendChild(r);}});
      }});
    }});
    var search=document.getElementById("table-search");
    if(search)search.addEventListener("input",function(){{
      var q=this.value.toLowerCase();
      rows.forEach(function(r){{
        r.classList.toggle("hidden",r.cells[0].textContent.toLowerCase().indexOf(q)===-1);
      }});
    }});
  }})();
  </script>
</body>
</html>"#
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::TableMetrics;

    fn one_table_metrics() -> Vec<TableMetrics> {
        vec![TableMetrics {
            qualified_name: "public.foo".into(),
            fk_depth_out: 0,
            fk_depth_in: 0,
            is_orphan: true,
            in_cycle: false,
            centrality_out: 0,
            centrality_in: 0,
            risk_score: 0.0,
            risk_breakdown: None,
            operational_weight: None,
            effective_risk: None,
        }]
    }

    #[test]
    fn html_contains_doctype_and_overview() {
        let mut buf = Vec::new();
        render(&mut buf, &one_table_metrics(), 1, 2, 0, 0, None).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("<!DOCTYPE html>"));
        assert!(s.contains("dbscope"));
        assert!(s.contains("tables"));
        assert!(s.contains("public.foo"));
        assert!(s.contains("Risk table"));
    }
}
