//! Benchmarks for the analysis pipeline: RawSchema -> graph -> metrics -> reports.
//! No database required; uses a programmatically built "real-world-style" schema.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dbscope::analysis;
use dbscope::core;
use dbscope::report;
use std::io::Cursor;

/// Build a large, messy schema: many tables, FK chains, one cycle, orphans.
/// Roughly 50 tables, 150+ columns, 50+ FKs. Represents "real-world-style" load.
fn large_raw_schema() -> core::RawSchema {
    let n = 50;
    let mut tables = Vec::with_capacity(n);
    let mut columns = Vec::with_capacity(n * 4);
    let mut indexes = Vec::with_capacity(n);
    let mut foreign_keys = Vec::with_capacity(n);

    for i in 0..n {
        let table = format!("t{i}");
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
        columns.push(core::ColumnMeta {
            schema_name: "public".into(),
            table_name: table.clone(),
            column_name: "parent_id".into(),
            data_type: "int4".into(),
            ordinal_position: 2,
            is_nullable: Some(true),
            default_value: None,
        });
        columns.push(core::ColumnMeta {
            schema_name: "public".into(),
            table_name: table.clone(),
            column_name: "name".into(),
            data_type: "text".into(),
            ordinal_position: 3,
            is_nullable: Some(true),
            default_value: None,
        });
        indexes.push(core::IndexMeta {
            schema_name: "public".into(),
            table_name: table.clone(),
            index_name: format!("{table}_pkey"),
            column_names: vec!["id".into()],
            is_unique: true,
        });
        // FK chain: t_i -> t_{i-1}; t0 has no FK (orphan at head); add cycle t1 -> t0, t0 -> t1 for one cycle
        if i > 0 {
            let from_table = format!("t{i}");
            let to_table = format!("t{}", i - 1);
            foreign_keys.push(core::ForeignKeyRef {
                name: format!("fk_{from_table}_parent"),
                from_schema: "public".into(),
                from_table: from_table.clone(),
                from_columns: vec!["parent_id".into()],
                to_schema: "public".into(),
                to_table: to_table.clone(),
                to_columns: vec!["id".into()],
            });
        }
    }
    // One cycle: t2 -> t1 -> t0 -> t2 (already have t2->t1, t1->t0; add t0->t2)
    foreign_keys.push(core::ForeignKeyRef {
        name: "fk_t0_refs_t2".into(),
        from_schema: "public".into(),
        from_table: "t0".into(),
        from_columns: vec!["parent_id".into()],
        to_schema: "public".into(),
        to_table: "t2".into(),
        to_columns: vec!["id".into()],
    });

    // A few orphan tables (no FK in or out)
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

fn bench_graph_build(c: &mut Criterion) {
    let raw = large_raw_schema();
    c.bench_function("graph_from_raw_schema_50tables", |b| {
        b.iter(|| {
            let g = core::DatabaseGraph::from_raw_schema(black_box(raw.clone()));
            black_box(g)
        })
    });
}

fn bench_metrics(c: &mut Criterion) {
    let raw = large_raw_schema();
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    c.bench_function("compute_all_metrics_50tables", |b| {
        b.iter(|| {
            let m = analysis::compute_all_metrics(black_box(&graph));
            black_box(m)
        })
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    let raw = large_raw_schema();
    c.bench_function("pipeline_graph_metrics_reports_50tables", |b| {
        b.iter(|| {
            let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
            let metrics = analysis::compute_all_metrics(&graph);
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
            black_box((md.into_inner(), html.into_inner()))
        })
    });
}

criterion_group!(
    benches,
    bench_graph_build,
    bench_metrics,
    bench_full_pipeline
);
criterion_main!(benches);
