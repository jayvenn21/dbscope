//! Comprehensive tests: lint, diff, snapshot, demo, migration ops,
//! JSON roundtrip, policy validation, error paths.

use dbscope::analysis;
use dbscope::cli;
use dbscope::core;
use dbscope::migration;
use dbscope::policy::Policy;

fn ecommerce_fixture() -> core::RawSchema {
    core::RawSchema {
        tables: vec![
            core::TableMeta {
                schema_name: "public".into(),
                table_name: "users".into(),
            },
            core::TableMeta {
                schema_name: "public".into(),
                table_name: "orders".into(),
            },
            core::TableMeta {
                schema_name: "public".into(),
                table_name: "products".into(),
            },
            core::TableMeta {
                schema_name: "public".into(),
                table_name: "order_items".into(),
            },
            core::TableMeta {
                schema_name: "public".into(),
                table_name: "orphan_config".into(),
            },
        ],
        views: vec![],
        materialized_views: vec![],
        columns: vec![
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "users".into(),
                column_name: "id".into(),
                data_type: "integer".into(),
                ordinal_position: 1,
                is_nullable: Some(false),
                default_value: Some("nextval('users_id_seq')".into()),
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "users".into(),
                column_name: "email".into(),
                data_type: "text".into(),
                ordinal_position: 2,
                is_nullable: Some(false),
                default_value: None,
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "users".into(),
                column_name: "status".into(),
                data_type: "text".into(),
                ordinal_position: 3,
                is_nullable: Some(true),
                default_value: Some("active".into()),
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "orders".into(),
                column_name: "id".into(),
                data_type: "integer".into(),
                ordinal_position: 1,
                is_nullable: Some(false),
                default_value: None,
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "orders".into(),
                column_name: "user_id".into(),
                data_type: "integer".into(),
                ordinal_position: 2,
                is_nullable: Some(true),
                default_value: None,
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "orders".into(),
                column_name: "status".into(),
                data_type: "text".into(),
                ordinal_position: 3,
                is_nullable: Some(false),
                default_value: Some("pending".into()),
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "products".into(),
                column_name: "id".into(),
                data_type: "integer".into(),
                ordinal_position: 1,
                is_nullable: Some(false),
                default_value: None,
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "products".into(),
                column_name: "name".into(),
                data_type: "text".into(),
                ordinal_position: 2,
                is_nullable: Some(false),
                default_value: None,
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "order_items".into(),
                column_name: "order_id".into(),
                data_type: "integer".into(),
                ordinal_position: 1,
                is_nullable: Some(false),
                default_value: None,
            },
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: "order_items".into(),
                column_name: "product_id".into(),
                data_type: "integer".into(),
                ordinal_position: 2,
                is_nullable: Some(false),
                default_value: None,
            },
        ],
        indexes: vec![
            core::IndexMeta {
                schema_name: "public".into(),
                table_name: "users".into(),
                index_name: "users_pkey".into(),
                column_names: vec!["id".into()],
                is_unique: true,
            },
            core::IndexMeta {
                schema_name: "public".into(),
                table_name: "orders".into(),
                index_name: "orders_pkey".into(),
                column_names: vec!["id".into()],
                is_unique: true,
            },
        ],
        constraints: vec![
            core::ConstraintMeta {
                schema_name: "public".into(),
                table_name: "users".into(),
                constraint_name: "users_pkey".into(),
                constraint_type: "PRIMARY KEY".into(),
            },
            core::ConstraintMeta {
                schema_name: "public".into(),
                table_name: "orders".into(),
                constraint_name: "orders_pkey".into(),
                constraint_type: "PRIMARY KEY".into(),
            },
        ],
        foreign_keys: vec![
            core::ForeignKeyRef {
                name: "orders_user_fk".into(),
                from_schema: "public".into(),
                from_table: "orders".into(),
                from_columns: vec!["user_id".into()],
                to_schema: "public".into(),
                to_table: "users".into(),
                to_columns: vec!["id".into()],
            },
            core::ForeignKeyRef {
                name: "order_items_order_fk".into(),
                from_schema: "public".into(),
                from_table: "order_items".into(),
                from_columns: vec!["order_id".into()],
                to_schema: "public".into(),
                to_table: "orders".into(),
                to_columns: vec!["id".into()],
            },
            core::ForeignKeyRef {
                name: "order_items_product_fk".into(),
                from_schema: "public".into(),
                from_table: "order_items".into(),
                from_columns: vec!["product_id".into()],
                to_schema: "public".into(),
                to_table: "products".into(),
                to_columns: vec!["id".into()],
            },
        ],
        table_stats: None,
        engine_metadata: None,
    }
}

// -- Lint tests-----------------------------------------------------------

#[test]
fn lint_detects_missing_fk_indexes() {
    let raw = ecommerce_fixture();
    let violations = cli::lint::lint_schema(&raw);
    let missing_fk = violations
        .iter()
        .filter(|v| v.rule == "missing-fk-index")
        .collect::<Vec<_>>();
    assert!(
        !missing_fk.is_empty(),
        "should detect missing indexes on FK columns"
    );
    assert!(missing_fk.iter().any(|v| v.message.contains("user_id")));
}

#[test]
fn lint_detects_text_enum_columns() {
    let raw = ecommerce_fixture();
    let violations = cli::lint::lint_schema(&raw);
    let text_enums = violations
        .iter()
        .filter(|v| v.rule == "text-enum")
        .collect::<Vec<_>>();
    assert!(
        !text_enums.is_empty(),
        "should detect text columns that look like enums"
    );
    assert!(text_enums.iter().any(|v| v.message.contains("status")));
}

#[test]
fn lint_detects_nullable_fks() {
    let raw = ecommerce_fixture();
    let violations = cli::lint::lint_schema(&raw);
    let nullable_fks = violations
        .iter()
        .filter(|v| v.rule == "nullable-fk")
        .collect::<Vec<_>>();
    assert!(
        !nullable_fks.is_empty(),
        "should detect nullable FK columns"
    );
    assert!(nullable_fks.iter().any(|v| v.message.contains("user_id")));
}

#[test]
fn lint_clean_schema_no_errors() {
    let raw = core::RawSchema {
        tables: vec![core::TableMeta {
            schema_name: "public".into(),
            table_name: "users".into(),
        }],
        columns: vec![core::ColumnMeta {
            schema_name: "public".into(),
            table_name: "users".into(),
            column_name: "id".into(),
            data_type: "integer".into(),
            ordinal_position: 1,
            is_nullable: Some(false),
            default_value: None,
        }],
        constraints: vec![core::ConstraintMeta {
            schema_name: "public".into(),
            table_name: "users".into(),
            constraint_name: "users_pkey".into(),
            constraint_type: "PRIMARY KEY".into(),
        }],
        ..Default::default()
    };
    let violations = cli::lint::lint_schema(&raw);
    let errors = violations
        .iter()
        .filter(|v| v.severity == cli::lint::LintSeverity::Error)
        .count();
    assert_eq!(
        errors, 0,
        "clean schema should have no error-level violations"
    );
}

// -- Diff tests-----------------------------------------------------------

#[test]
fn diff_detects_added_table() {
    let before = ecommerce_fixture();
    let mut after = ecommerce_fixture();
    after.tables.push(core::TableMeta {
        schema_name: "public".into(),
        table_name: "reviews".into(),
    });
    let diff = cli::diff::compute_diff(&before, &after);
    assert!(diff.tables_added.contains(&"public.reviews".to_string()));
    assert_eq!(diff.summary.breaking_changes, 0);
    assert!(diff.summary.risk_assessment.contains("safe"));
}

#[test]
fn diff_detects_removed_table() {
    let before = ecommerce_fixture();
    let mut after = ecommerce_fixture();
    after.tables.retain(|t| t.table_name != "orphan_config");
    let diff = cli::diff::compute_diff(&before, &after);
    assert!(diff
        .tables_removed
        .contains(&"public.orphan_config".to_string()));
    assert!(diff.summary.breaking_changes > 0);
}

#[test]
fn diff_detects_added_column() {
    let before = ecommerce_fixture();
    let mut after = ecommerce_fixture();
    after.columns.push(core::ColumnMeta {
        schema_name: "public".into(),
        table_name: "users".into(),
        column_name: "phone".into(),
        data_type: "text".into(),
        ordinal_position: 4,
        is_nullable: Some(true),
        default_value: None,
    });
    let diff = cli::diff::compute_diff(&before, &after);
    assert!(diff.columns_added.iter().any(|c| c.column == "phone"));
}

#[test]
fn diff_detects_column_type_change() {
    let before = ecommerce_fixture();
    let mut after = ecommerce_fixture();
    for c in &mut after.columns {
        if c.table_name == "users" && c.column_name == "email" {
            c.data_type = "varchar(255)".into();
        }
    }
    let diff = cli::diff::compute_diff(&before, &after);
    assert!(diff
        .columns_type_changed
        .iter()
        .any(|c| c.column == "email" && c.old_type == "text" && c.new_type == "varchar(255)"));
}

#[test]
fn diff_no_changes() {
    let schema = ecommerce_fixture();
    let diff = cli::diff::compute_diff(&schema, &schema);
    assert_eq!(diff.summary.total_changes, 0);
    assert!(diff.summary.risk_assessment.contains("No changes"));
}

// -- Snapshot tests-------------------------------------------------------

#[test]
fn snapshot_roundtrip() {
    let raw = ecommerce_fixture();
    let snap = cli::snapshot::SchemaSnapshot::new(raw.clone(), "postgres://test@localhost/db");
    let tmp = std::env::temp_dir().join("dbscope_test_snapshot.json");
    snap.save(&tmp).unwrap();
    let loaded = cli::snapshot::SchemaSnapshot::load(&tmp).unwrap();
    assert_eq!(loaded.schema.tables.len(), raw.tables.len());
    assert_eq!(loaded.schema.foreign_keys.len(), raw.foreign_keys.len());
    assert_eq!(loaded.schema.columns.len(), raw.columns.len());
    assert_eq!(loaded.version, 1);
    assert!(!loaded.source_uri_hash.is_empty());
    std::fs::remove_file(&tmp).ok();
}

#[test]
fn snapshot_different_uris_different_hashes() {
    let raw = ecommerce_fixture();
    let snap_a = cli::snapshot::SchemaSnapshot::new(raw.clone(), "postgres://a@localhost/db");
    let snap_b = cli::snapshot::SchemaSnapshot::new(raw, "postgres://b@localhost/other");
    assert_ne!(snap_a.source_uri_hash, snap_b.source_uri_hash);
}

// -- Demo tests-----------------------------------------------------------

#[test]
fn demo_runs_without_error() {
    let tmp_dir = std::env::temp_dir().join("dbscope_demo_test");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    cli::demo::run_demo(Some(tmp_dir.as_path())).unwrap();
    assert!(tmp_dir.join("dbscope-report.html").exists());
    assert!(tmp_dir.join("dbscope-report.json").exists());
    assert!(tmp_dir.join("dbscope-graph.dot").exists());
    std::fs::remove_dir_all(&tmp_dir).ok();
}

// -- Migration operation tests--------------------------------------------

#[test]
fn migration_alter_add_column() {
    let base = ecommerce_fixture();
    let stmts = migration::parse_migration_sql("ALTER TABLE public.users ADD COLUMN phone TEXT;");
    let result = migration::apply_migration_to_schema(&base, &stmts);
    assert!(result
        .columns
        .iter()
        .any(|c| c.table_name == "users" && c.column_name == "phone"));
}

#[test]
fn migration_alter_drop_column() {
    let base = ecommerce_fixture();
    let stmts = migration::parse_migration_sql("ALTER TABLE public.users DROP COLUMN status;");
    let result = migration::apply_migration_to_schema(&base, &stmts);
    assert!(!result
        .columns
        .iter()
        .any(|c| c.table_name == "users" && c.column_name == "status"));
}

#[test]
fn migration_rename_table() {
    let base = ecommerce_fixture();
    let stmts = migration::parse_migration_sql("ALTER TABLE public.users RENAME TO customers;");
    let result = migration::apply_migration_to_schema(&base, &stmts);
    assert!(!result.tables.iter().any(|t| t.table_name == "users"));
    assert!(result.tables.iter().any(|t| t.table_name == "customers"));
    assert!(result
        .columns
        .iter()
        .any(|c| c.table_name == "customers" && c.column_name == "email"));
}

#[test]
fn migration_drop_constraint() {
    let base = ecommerce_fixture();
    let stmts =
        migration::parse_migration_sql("ALTER TABLE public.orders DROP CONSTRAINT orders_user_fk;");
    let result = migration::apply_migration_to_schema(&base, &stmts);
    assert!(!result
        .foreign_keys
        .iter()
        .any(|fk| fk.name == "orders_user_fk"));
}

#[test]
fn migration_create_table() {
    let base = ecommerce_fixture();
    let stmts = migration::parse_migration_sql(
        "CREATE TABLE public.reviews (id INT, user_id INT, body TEXT);",
    );
    let result = migration::apply_migration_to_schema(&base, &stmts);
    assert!(result.tables.iter().any(|t| t.table_name == "reviews"));
    assert_eq!(
        result
            .columns
            .iter()
            .filter(|c| c.table_name == "reviews")
            .count(),
        3
    );
}

#[test]
fn migration_add_fk_constraint() {
    let base = ecommerce_fixture();
    let sql = "ALTER TABLE public.products ADD CONSTRAINT products_user_fk FOREIGN KEY (name) REFERENCES public.users (email);";
    let stmts = migration::parse_migration_sql(sql);
    let result = migration::apply_migration_to_schema(&base, &stmts);
    assert!(result
        .foreign_keys
        .iter()
        .any(|fk| fk.name == "products_user_fk"));
}

#[test]
fn migration_drop_table_removes_fks() {
    let base = ecommerce_fixture();
    let stmts = migration::parse_migration_sql("DROP TABLE IF EXISTS public.orders;");
    let result = migration::apply_migration_to_schema(&base, &stmts);
    assert!(!result.tables.iter().any(|t| t.table_name == "orders"));
    assert!(
        !result
            .foreign_keys
            .iter()
            .any(|fk| fk.from_table == "orders" || fk.to_table == "orders"),
        "FKs referencing dropped table should be removed"
    );
}

// -- JSON roundtrip tests-------------------------------------------------

#[test]
fn raw_schema_json_roundtrip() {
    let raw = ecommerce_fixture();
    let json = serde_json::to_string(&raw).unwrap();
    let deserialized: core::RawSchema = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.tables.len(), raw.tables.len());
    assert_eq!(deserialized.columns.len(), raw.columns.len());
    assert_eq!(deserialized.foreign_keys.len(), raw.foreign_keys.len());
    assert_eq!(deserialized.indexes.len(), raw.indexes.len());
}

#[test]
fn table_metrics_json_roundtrip() {
    let raw = ecommerce_fixture();
    let graph = core::DatabaseGraph::from_raw_schema(raw);
    let metrics = analysis::compute_all_metrics(&graph);
    let json = serde_json::to_string(&metrics).unwrap();
    let deserialized: Vec<analysis::TableMetrics> = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.len(), metrics.len());
    for (orig, deser) in metrics.iter().zip(deserialized.iter()) {
        assert_eq!(orig.qualified_name, deser.qualified_name);
        assert!((orig.risk_score - deser.risk_score).abs() < 1e-10);
    }
}

// -- Policy validation tests----------------------------------------------

#[test]
fn policy_default_values() {
    let policy = Policy::default();
    assert!(!policy.no_cycles);
    assert!(!policy.no_orphans);
}

#[test]
fn policy_validate_catches_bad_values() {
    let policy = Policy {
        max_table_risk: 2.0,
        no_cycles: false,
        no_orphans: false,
        max_blast_radius_percent: -10.0,
    };
    let warnings = policy.validate();
    assert_eq!(
        warnings.len(),
        2,
        "should warn about both out-of-range values"
    );
}

#[test]
fn policy_validate_passes_good_values() {
    let policy = Policy {
        max_table_risk: 0.5,
        no_cycles: true,
        no_orphans: true,
        max_blast_radius_percent: 50.0,
    };
    let warnings = policy.validate();
    assert!(warnings.is_empty());
}

// -- Deps tree tests------------------------------------------------------

#[test]
fn deps_tree_structure() {
    let raw = ecommerce_fixture();
    let graph = core::DatabaseGraph::from_raw_schema(raw);
    let tree = cli::deps::build_dep_tree(&graph, "public.users").unwrap();
    assert_eq!(tree.target, "public.users");
    assert!(
        !tree.downstream.is_empty(),
        "users should have downstream deps"
    );
    assert!(tree.upstream.is_empty(), "users has no upstream deps");
}

#[test]
fn deps_tree_leaf_table() {
    let raw = ecommerce_fixture();
    let graph = core::DatabaseGraph::from_raw_schema(raw);
    let tree = cli::deps::build_dep_tree(&graph, "public.order_items").unwrap();
    assert!(tree.downstream.is_empty(), "order_items is a leaf");
    assert!(!tree.upstream.is_empty(), "order_items has upstream deps");
}

#[test]
fn deps_tree_orphan() {
    let raw = ecommerce_fixture();
    let graph = core::DatabaseGraph::from_raw_schema(raw);
    let tree = cli::deps::build_dep_tree(&graph, "public.orphan_config").unwrap();
    assert!(tree.downstream.is_empty());
    assert!(tree.upstream.is_empty());
}

#[test]
fn deps_tree_nonexistent_returns_none() {
    let raw = ecommerce_fixture();
    let graph = core::DatabaseGraph::from_raw_schema(raw);
    assert!(cli::deps::build_dep_tree(&graph, "public.nonexistent").is_none());
}

// -- Query parser multi-dialect tests-------------------------------------

#[test]
fn parse_mysql_style_query() {
    let q = dbscope::query_parser::parse_sql("SELECT `id`, `name` FROM `users` WHERE `id` = 1");
    assert!(
        q.is_some(),
        "GenericDialect should handle MySQL backtick quoting"
    );
    let q = q.unwrap();
    assert!(q.tables.iter().any(|t| t.table == "users"));
}

#[test]
fn parse_insert_returning() {
    let q = dbscope::query_parser::parse_sql(
        "INSERT INTO public.users (email) VALUES ('test@test.com')",
    );
    assert!(q.is_some());
    let q = q.unwrap();
    assert!(q
        .tables
        .iter()
        .any(|t| t.qualified_name() == "public.users"));
}

#[test]
fn parse_update_with_where() {
    let q = dbscope::query_parser::parse_sql(
        "UPDATE public.orders SET status = 'shipped' WHERE id = 1",
    );
    assert!(q.is_some());
    let q = q.unwrap();
    assert!(q
        .tables
        .iter()
        .any(|t| t.qualified_name() == "public.orders"));
    assert!(!q.columns_in_where.is_empty());
}

// -- Impact edge case tests-----------------------------------------------

#[test]
fn impact_on_orphan_table() {
    let raw = ecommerce_fixture();
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let target = analysis::ImpactTarget::parse("public.orphan_config").unwrap();
    let report = analysis::compute_impact(&target, &graph, &raw, None).unwrap();
    assert!(report.fk_downstream_tables.is_empty());
    assert!(report.fk_upstream_tables.is_empty());
    assert!(
        report.risk_delta < 0.1,
        "orphan should have near-zero blast radius"
    );
}

#[test]
fn impact_with_query_count() {
    let raw = ecommerce_fixture();
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
    let target = analysis::ImpactTarget::parse("public.users").unwrap();
    let without_queries = analysis::compute_impact(&target, &graph, &raw, None).unwrap();
    let with_queries = analysis::compute_impact(&target, &graph, &raw, Some(50)).unwrap();
    assert!(
        with_queries.risk_delta > without_queries.risk_delta,
        "more affected queries should increase risk delta"
    );
}

// -- Metrics edge case tests----------------------------------------------

#[test]
fn metrics_empty_schema() {
    let raw = core::RawSchema::default();
    let graph = core::DatabaseGraph::from_raw_schema(raw);
    let metrics = analysis::compute_all_metrics(&graph);
    assert!(metrics.is_empty());
}

#[test]
fn metrics_single_table() {
    let raw = core::RawSchema {
        tables: vec![core::TableMeta {
            schema_name: "public".into(),
            table_name: "solo".into(),
        }],
        ..Default::default()
    };
    let graph = core::DatabaseGraph::from_raw_schema(raw);
    let metrics = analysis::compute_all_metrics(&graph);
    assert_eq!(metrics.len(), 1);
    assert!(metrics[0].is_orphan);
    assert_eq!(metrics[0].risk_score, 0.0);
}

#[test]
fn metrics_self_referencing_table() {
    let raw = core::RawSchema {
        tables: vec![core::TableMeta {
            schema_name: "public".into(),
            table_name: "categories".into(),
        }],
        foreign_keys: vec![core::ForeignKeyRef {
            name: "cat_parent_fk".into(),
            from_schema: "public".into(),
            from_table: "categories".into(),
            from_columns: vec!["parent_id".into()],
            to_schema: "public".into(),
            to_table: "categories".into(),
            to_columns: vec!["id".into()],
        }],
        ..Default::default()
    };
    let graph = core::DatabaseGraph::from_raw_schema(raw);
    let metrics = analysis::compute_all_metrics(&graph);
    assert_eq!(metrics.len(), 1);
    // Self-reference is not a multi-table cycle (SCC size=1), so in_cycle is false.
    // The table is not orphan because it has FK edges.
    assert!(
        !metrics[0].is_orphan,
        "self-referencing table is not orphan"
    );
    assert!(
        metrics[0].risk_score > 0.0,
        "self-referencing table has nonzero risk"
    );
}
