//! Integration test: fixture schema -> graph -> metrics -> markdown + HTML reports.
//! No database required.

use dbscope::analysis;
use dbscope::core;
use dbscope::report;
use std::io::Cursor;

fn fixture_raw_schema() -> core::RawSchema {
    core::RawSchema {
        tables: vec![
            core::TableMeta {
                schema_name: "public".into(),
                table_name: "users".into(),
            },
            core::TableMeta {
                schema_name: "public".into(),
                table_name: "posts".into(),
            },
            core::TableMeta {
                schema_name: "public".into(),
                table_name: "standalone".into(),
            },
        ],
        views: vec![],
        materialized_views: vec![],
        columns: vec![
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "users".into(),
                column_name: "id".into(),
                data_type: "int4".into(),
                ordinal_position: 1,
                is_nullable: Some(false),
                default_value: None,
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "posts".into(),
                column_name: "user_id".into(),
                data_type: "int4".into(),
                ordinal_position: 1,
                is_nullable: Some(true),
                default_value: None,
            },
        ],
        indexes: vec![],
        constraints: vec![],
        foreign_keys: vec![core::ForeignKeyRef {
            name: "posts_user_id_fkey".into(),
            from_schema: "public".into(),
            from_table: "posts".into(),
            from_columns: vec!["user_id".into()],
            to_schema: "public".into(),
            to_table: "users".into(),
            to_columns: vec!["id".into()],
        }],
        table_stats: None,
        engine_metadata: None,
    }
}

/// Large, messy schema: 50 tables in a chain, one cycle, 3 orphans.
/// Used to assert the pipeline handles real-world-style load without regressions.
fn large_realworld_fixture() -> core::RawSchema {
    let n = 50;
    let mut tables = Vec::with_capacity(n + 3);
    let mut columns = Vec::with_capacity((n + 3) * 3);
    let mut indexes = Vec::with_capacity(n + 3);
    let mut foreign_keys = Vec::with_capacity(n + 1);

    for i in 0..n {
        let table = format!("t{i}");
        tables.push(core::TableMeta {
            schema_name: "public".into(),
            table_name: table.clone(),
        });
        for (col, pos) in [("id", 1), ("parent_id", 2), ("name", 3)] {
            columns.push(core::ColumnMeta {
                schema_name: "public".into(),
                table_name: table.clone(),
                column_name: col.into(),
                data_type: if col == "name" { "text" } else { "int4" }.into(),
                ordinal_position: pos,
                is_nullable: Some(col != "id"),
                default_value: None,
            });
        }
        indexes.push(core::IndexMeta {
            schema_name: "public".into(),
            table_name: table.clone(),
            index_name: format!("{table}_pkey"),
            column_names: vec!["id".into()],
            is_unique: true,
        });
        if i > 0 {
            let from_table = format!("t{i}");
            let to_table = format!("t{}", i - 1);
            foreign_keys.push(core::ForeignKeyRef {
                name: format!("fk_{from_table}_parent"),
                from_schema: "public".into(),
                from_table,
                from_columns: vec!["parent_id".into()],
                to_schema: "public".into(),
                to_table,
                to_columns: vec!["id".into()],
            });
        }
    }
    foreign_keys.push(core::ForeignKeyRef {
        name: "fk_t0_refs_t2".into(),
        from_schema: "public".into(),
        from_table: "t0".into(),
        from_columns: vec!["parent_id".into()],
        to_schema: "public".into(),
        to_table: "t2".into(),
        to_columns: vec!["id".into()],
    });

    for i in 0..3 {
        let table = format!("orphan_{i}");
        tables.push(core::TableMeta {
            schema_name: "public".into(),
            table_name: table.clone(),
        });
        columns.push(core::ColumnMeta {
            schema_name: "public".into(),
            table_name: table.clone(),
            column_name: "id".into(),
            data_type: "int4".into(),
            ordinal_position: 1,
            is_nullable: Some(false),
            default_value: None,
        });
    }

    core::RawSchema {
        tables,
        views: vec![],
        materialized_views: vec![],
        columns,
        indexes,
        constraints: vec![],
        foreign_keys,
        table_stats: None,
        engine_metadata: None,
    }
}

#[test]
fn realworld_schema_pipeline() {
    let raw = large_realworld_fixture();
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics(&graph);

    assert_eq!(graph.table_count(), 53, "50 chained + 3 orphans");
    let orphans: Vec<_> = metrics.iter().filter(|m| m.is_orphan).collect();
    assert!(orphans.len() >= 3, "at least 3 orphan tables");
    let in_cycle: Vec<_> = metrics.iter().filter(|m| m.in_cycle).collect();
    assert!(
        !in_cycle.is_empty(),
        "cycle t0->t2->t1->t0 should be detected"
    );

    let total_tables = raw.tables.len();
    let total_columns = raw.columns.len();
    let total_indexes = raw.indexes.len();
    let total_fks = raw.foreign_keys.len();
    let mut html = Cursor::new(Vec::new());
    report::html::render(
        &mut html,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        None,
    )
    .unwrap();
    let html_str = String::from_utf8(html.into_inner()).unwrap();
    assert!(html_str.contains("<!DOCTYPE html>"));
    assert!(html_str.contains("public.t0"));
    assert!(html_str.contains("Orphan"));
}

#[test]
fn pipeline_fixture_to_reports() {
    let raw = fixture_raw_schema();
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics(&graph);

    assert_eq!(graph.table_count(), 3);
    assert_eq!(metrics.len(), 3);

    let users = metrics
        .iter()
        .find(|m| m.qualified_name == "public.users")
        .unwrap();
    let standalone = metrics
        .iter()
        .find(|m| m.qualified_name == "public.standalone")
        .unwrap();
    assert!(standalone.is_orphan);
    assert_eq!(standalone.risk_score, 0.0);
    assert_eq!(users.centrality_in, 1);
    assert_eq!(users.fk_depth_in, 1);

    let total_tables = raw.tables.len();
    let total_columns = raw.columns.len();
    let total_indexes = raw.indexes.len();
    let total_fks = raw.foreign_keys.len();

    let mut md = Cursor::new(Vec::new());
    report::markdown::render(
        &mut md,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        None,
    )
    .unwrap();
    let md_str = String::from_utf8(md.into_inner()).unwrap();
    assert!(md_str.contains("# DBScope Schema Report"));
    assert!(md_str.contains("public.users"));
    assert!(md_str.contains("public.standalone"));

    let mut html = Cursor::new(Vec::new());
    report::html::render(
        &mut html,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        None,
    )
    .unwrap();
    let html_str = String::from_utf8(html.into_inner()).unwrap();
    assert!(html_str.contains("<!DOCTYPE html>"));
    assert!(html_str.contains("public.users"));
    assert!(html_str.contains("Orphan tables"));

    // JSON report
    let mut json_buf = Cursor::new(Vec::new());
    report::json::render(
        &mut json_buf,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        None,
    )
    .unwrap();
    let json_str = String::from_utf8(json_buf.into_inner()).unwrap();
    assert!(json_str.contains("\"overview\""));
    assert!(json_str.contains("\"table_metrics\""));
    assert!(json_str.contains("public.users"));

    // Graphviz export
    let mut dot_buf = Cursor::new(Vec::new());
    report::graphviz::render(&mut dot_buf, &graph, Some(&metrics)).unwrap();
    let dot_str = String::from_utf8(dot_buf.into_inner()).unwrap();
    assert!(dot_str.starts_with("digraph dbscope"));
    assert!(dot_str.contains("public.users"));
    assert!(dot_str.contains("public.posts"));
    assert!(dot_str.contains("->"));
}

#[test]
fn phase2_usage_report_and_render() {
    let raw = fixture_raw_schema();
    let queries = vec![
        "SELECT id FROM public.users WHERE id = 1".to_string(),
        "SELECT user_id FROM public.posts WHERE user_id = 2".to_string(),
    ];
    let (usage, parsed_count) = analysis::build_usage_from_queries(&queries);
    assert_eq!(parsed_count, 2);
    let usage_report = analysis::compute_usage_report(&raw, &usage, parsed_count);

    // standalone was never queried
    assert!(usage_report
        .cold_tables
        .iter()
        .any(|t| t.0 == "public.standalone"));
    // users and posts were queried
    assert!(usage_report
        .hot_tables
        .iter()
        .any(|h| h.qualified_name == "public.users"));
    assert!(usage_report
        .hot_tables
        .iter()
        .any(|h| h.qualified_name == "public.posts"));
    // user_id in WHERE but no index on posts.user_id -> suggestion
    assert!(usage_report
        .index_suggestions
        .iter()
        .any(|s| s.column_name == "user_id" && s.qualified_table == "public.posts"));

    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = analysis::compute_all_metrics(&graph);
    let total_tables = raw.tables.len();
    let total_columns = raw.columns.len();
    let total_indexes = raw.indexes.len();
    let total_fks = raw.foreign_keys.len();

    let mut html = Cursor::new(Vec::new());
    report::html::render(
        &mut html,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        Some(&usage_report),
    )
    .unwrap();
    let html_str = String::from_utf8(html.into_inner()).unwrap();
    assert!(html_str.contains("queries"));
    assert!(html_str.contains("Cold tables"));
    assert!(html_str.contains("Index suggestions"));
}

/// Blast radius: impact report from fixture schema (no DB).
#[test]
fn phase3_impact_report() {
    let raw = fixture_raw_schema();
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());

    // Impact on users: posts references users via FK -> downstream = [public.posts]
    let target = analysis::ImpactTarget::parse("public.users").unwrap();
    assert_eq!(target.qualified_table(), "public.users");
    let report = analysis::compute_impact(&target, &graph, &raw, None)
        .expect("public.users should be in graph");
    assert!(
        report
            .fk_downstream_tables
            .contains(&"public.posts".to_string()),
        "changing users should list posts as FK downstream"
    );
    assert!(report.fk_upstream_tables.is_empty());

    // Impact on posts: references users -> upstream = [public.users]; no tables reference posts
    let target = analysis::ImpactTarget::parse("public.posts").unwrap();
    let report = analysis::compute_impact(&target, &graph, &raw, None)
        .expect("public.posts should be in graph");
    assert!(report.fk_downstream_tables.is_empty());
    assert!(
        report
            .fk_upstream_tables
            .contains(&"public.users".to_string()),
        "changing posts should list users as FK upstream"
    );

    // risk_delta is computed
    assert!(report.risk_delta >= 0.0 && report.risk_delta <= 1.0);
}
