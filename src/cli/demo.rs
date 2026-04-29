//! `dbscope demo`: zero-config onboarding with an embedded e-commerce schema.

use crate::analysis;
use crate::cli::style::Theme;
use crate::core;
use crate::report;
use std::path::Path;

fn demo_schema() -> core::RawSchema {
    let tables = vec![
        ("public", "users"),
        ("public", "products"),
        ("public", "categories"),
        ("public", "orders"),
        ("public", "order_items"),
        ("public", "reviews"),
        ("public", "payments"),
        ("public", "addresses"),
        ("public", "wishlists"),
        ("public", "wishlist_items"),
        ("public", "sessions"),
        ("public", "audit_logs"),
        ("public", "coupons"),
        ("public", "inventory"),
        ("public", "shipping"),
        // Orphans (intentional anti-patterns)
        ("public", "feature_flags"),
        ("public", "schema_migrations"),
    ]
    .into_iter()
    .map(|(s, t)| core::TableMeta {
        schema_name: s.into(),
        table_name: t.into(),
    })
    .collect();

    let cols: Vec<(&str, &str, &str, &str, bool)> = vec![
        // users
        ("users", "id", "integer", "nextval('users_id_seq')", false),
        ("users", "email", "text", "", false),
        ("users", "name", "text", "", true),
        ("users", "password_hash", "text", "", false),
        ("users", "created_at", "timestamptz", "now()", false),
        // products
        ("products", "id", "integer", "", false),
        ("products", "category_id", "integer", "", false),
        ("products", "name", "text", "", false),
        ("products", "description", "text", "", true),
        ("products", "price_cents", "integer", "", false),
        ("products", "created_at", "timestamptz", "now()", false),
        // categories
        ("categories", "id", "integer", "", false),
        ("categories", "name", "text", "", false),
        ("categories", "parent_id", "integer", "", true),
        // orders
        ("orders", "id", "integer", "", false),
        ("orders", "user_id", "integer", "", false),
        ("orders", "status", "text", "pending", false),
        ("orders", "total_cents", "integer", "", false),
        ("orders", "created_at", "timestamptz", "now()", false),
        // order_items
        ("order_items", "id", "integer", "", false),
        ("order_items", "order_id", "integer", "", false),
        ("order_items", "product_id", "integer", "", false),
        ("order_items", "quantity", "integer", "1", false),
        ("order_items", "unit_price_cents", "integer", "", false),
        // reviews
        ("reviews", "id", "integer", "", false),
        ("reviews", "user_id", "integer", "", false),
        ("reviews", "product_id", "integer", "", false),
        ("reviews", "rating", "integer", "", false),
        ("reviews", "body", "text", "", true),
        ("reviews", "created_at", "timestamptz", "now()", false),
        // payments
        ("payments", "id", "integer", "", false),
        ("payments", "order_id", "integer", "", false),
        ("payments", "amount_cents", "integer", "", false),
        ("payments", "status", "text", "pending", false),
        ("payments", "provider", "text", "", false),
        ("payments", "created_at", "timestamptz", "now()", false),
        // addresses
        ("addresses", "id", "integer", "", false),
        ("addresses", "user_id", "integer", "", false),
        ("addresses", "line1", "text", "", false),
        ("addresses", "city", "text", "", false),
        ("addresses", "country", "text", "", false),
        // wishlists
        ("wishlists", "id", "integer", "", false),
        ("wishlists", "user_id", "integer", "", false),
        ("wishlists", "name", "text", "My Wishlist", false),
        // wishlist_items
        ("wishlist_items", "id", "integer", "", false),
        ("wishlist_items", "wishlist_id", "integer", "", false),
        ("wishlist_items", "product_id", "integer", "", false),
        // sessions
        ("sessions", "id", "integer", "", false),
        ("sessions", "user_id", "integer", "", false),
        ("sessions", "token", "text", "", false),
        ("sessions", "expires_at", "timestamptz", "", false),
        // audit_logs
        ("audit_logs", "id", "integer", "", false),
        ("audit_logs", "user_id", "integer", "", true),
        ("audit_logs", "action", "text", "", false),
        ("audit_logs", "resource", "text", "", true),
        ("audit_logs", "created_at", "timestamptz", "now()", false),
        // coupons
        ("coupons", "id", "integer", "", false),
        ("coupons", "code", "text", "", false),
        ("coupons", "discount_percent", "integer", "", false),
        ("coupons", "expires_at", "timestamptz", "", true),
        // inventory
        ("inventory", "id", "integer", "", false),
        ("inventory", "product_id", "integer", "", false),
        ("inventory", "quantity", "integer", "0", false),
        ("inventory", "warehouse", "text", "default", false),
        // shipping
        ("shipping", "id", "integer", "", false),
        ("shipping", "order_id", "integer", "", false),
        ("shipping", "address_id", "integer", "", false),
        ("shipping", "carrier", "text", "", false),
        ("shipping", "tracking_number", "text", "", true),
        ("shipping", "status", "text", "pending", false),
        // feature_flags (orphan)
        ("feature_flags", "id", "integer", "", false),
        ("feature_flags", "key", "text", "", false),
        ("feature_flags", "enabled", "boolean", "false", false),
        // schema_migrations (orphan)
        ("schema_migrations", "version", "bigint", "", false),
        ("schema_migrations", "name", "text", "", false),
    ];

    let columns: Vec<core::ColumnMeta> = cols
        .iter()
        .enumerate()
        .map(|(i, (table, col, dtype, dflt, nullable))| {
            let pos = cols[..=i].iter().filter(|(t, ..)| t == table).count() as i32;
            core::ColumnMeta {
                schema_name: "public".into(),
                table_name: table.to_string(),
                column_name: col.to_string(),
                data_type: dtype.to_string(),
                ordinal_position: pos,
                is_nullable: Some(*nullable),
                default_value: if dflt.is_empty() {
                    None
                } else {
                    Some(dflt.to_string())
                },
            }
        })
        .collect();

    let fks: Vec<(&str, &str, &str, &str, &str)> = vec![
        (
            "products",
            "category_id",
            "categories",
            "id",
            "products_category_fk",
        ),
        (
            "categories",
            "parent_id",
            "categories",
            "id",
            "categories_parent_fk",
        ),
        ("orders", "user_id", "users", "id", "orders_user_fk"),
        (
            "order_items",
            "order_id",
            "orders",
            "id",
            "order_items_order_fk",
        ),
        (
            "order_items",
            "product_id",
            "products",
            "id",
            "order_items_product_fk",
        ),
        ("reviews", "user_id", "users", "id", "reviews_user_fk"),
        (
            "reviews",
            "product_id",
            "products",
            "id",
            "reviews_product_fk",
        ),
        ("payments", "order_id", "orders", "id", "payments_order_fk"),
        ("addresses", "user_id", "users", "id", "addresses_user_fk"),
        ("wishlists", "user_id", "users", "id", "wishlists_user_fk"),
        (
            "wishlist_items",
            "wishlist_id",
            "wishlists",
            "id",
            "wishlist_items_wishlist_fk",
        ),
        (
            "wishlist_items",
            "product_id",
            "products",
            "id",
            "wishlist_items_product_fk",
        ),
        ("sessions", "user_id", "users", "id", "sessions_user_fk"),
        ("audit_logs", "user_id", "users", "id", "audit_logs_user_fk"),
        (
            "inventory",
            "product_id",
            "products",
            "id",
            "inventory_product_fk",
        ),
        ("shipping", "order_id", "orders", "id", "shipping_order_fk"),
        (
            "shipping",
            "address_id",
            "addresses",
            "id",
            "shipping_address_fk",
        ),
    ];

    let foreign_keys = fks
        .into_iter()
        .map(|(from_t, from_c, to_t, to_c, name)| core::ForeignKeyRef {
            name: name.into(),
            from_schema: "public".into(),
            from_table: from_t.into(),
            from_columns: vec![from_c.into()],
            to_schema: "public".into(),
            to_table: to_t.into(),
            to_columns: vec![to_c.into()],
        })
        .collect();

    let idx_defs: Vec<(&str, &str, Vec<&str>, bool)> = vec![
        ("users", "users_pkey", vec!["id"], true),
        ("users", "users_email_key", vec!["email"], true),
        ("products", "products_pkey", vec!["id"], true),
        ("categories", "categories_pkey", vec!["id"], true),
        ("orders", "orders_pkey", vec!["id"], true),
        ("orders", "idx_orders_user_id", vec!["user_id"], false),
        ("order_items", "order_items_pkey", vec!["id"], true),
        (
            "order_items",
            "idx_order_items_order_id",
            vec!["order_id"],
            false,
        ),
        ("reviews", "reviews_pkey", vec!["id"], true),
        ("payments", "payments_pkey", vec!["id"], true),
        ("sessions", "sessions_pkey", vec!["id"], true),
        ("sessions", "sessions_token_key", vec!["token"], true),
        ("shipping", "shipping_pkey", vec!["id"], true),
        // Intentionally missing: no index on reviews.product_id, inventory.product_id, payments.order_id
    ];

    let indexes = idx_defs
        .into_iter()
        .map(|(table, name, cols, unique)| core::IndexMeta {
            schema_name: "public".into(),
            table_name: table.into(),
            index_name: name.into(),
            column_names: cols.into_iter().map(String::from).collect(),
            is_unique: unique,
        })
        .collect();

    let constraints = vec![
        ("users", "users_pkey", "PRIMARY KEY"),
        ("products", "products_pkey", "PRIMARY KEY"),
        ("categories", "categories_pkey", "PRIMARY KEY"),
        ("orders", "orders_pkey", "PRIMARY KEY"),
        ("order_items", "order_items_pkey", "PRIMARY KEY"),
    ]
    .into_iter()
    .map(|(t, n, ty)| core::ConstraintMeta {
        schema_name: "public".into(),
        table_name: t.into(),
        constraint_name: n.into(),
        constraint_type: ty.into(),
    })
    .collect();

    core::RawSchema {
        tables,
        views: vec![],
        materialized_views: vec![],
        columns,
        indexes,
        constraints,
        foreign_keys,
        table_stats: None,
        engine_metadata: None,
    }
}

fn demo_queries() -> Vec<String> {
    vec![
        "SELECT id, email, name FROM public.users WHERE id = $1",
        "SELECT id, email FROM public.users WHERE email = $1",
        "SELECT p.id, p.name, p.price_cents FROM public.products p JOIN public.categories c ON p.category_id = c.id WHERE c.name = $1",
        "SELECT p.id, p.name FROM public.products p WHERE p.category_id = $1 ORDER BY p.created_at DESC",
        "SELECT o.id, o.status, o.total_cents FROM public.orders o WHERE o.user_id = $1",
        "SELECT o.id, o.status FROM public.orders o WHERE o.user_id = $1 AND o.status = $2",
        "SELECT oi.id, oi.quantity, p.name FROM public.order_items oi JOIN public.products p ON oi.product_id = p.id WHERE oi.order_id = $1",
        "SELECT r.id, r.rating, r.body FROM public.reviews r WHERE r.product_id = $1",
        "SELECT r.rating, COUNT(*) FROM public.reviews r WHERE r.product_id = $1 GROUP BY r.rating",
        "SELECT AVG(r.rating) FROM public.reviews r WHERE r.product_id = $1",
        "INSERT INTO public.orders (user_id, status, total_cents) VALUES ($1, 'pending', $2)",
        "INSERT INTO public.order_items (order_id, product_id, quantity, unit_price_cents) VALUES ($1, $2, $3, $4)",
        "UPDATE public.orders SET status = $1 WHERE id = $2",
        "SELECT pay.id, pay.status FROM public.payments pay WHERE pay.order_id = $1",
        "INSERT INTO public.payments (order_id, amount_cents, status, provider) VALUES ($1, $2, 'pending', $3)",
        "SELECT s.id, s.token FROM public.sessions s WHERE s.user_id = $1 AND s.expires_at > now()",
        "SELECT i.quantity FROM public.inventory i WHERE i.product_id = $1",
        "UPDATE public.inventory SET quantity = quantity - $1 WHERE product_id = $2",
        "SELECT sh.status, sh.tracking_number FROM public.shipping sh WHERE sh.order_id = $1",
        "SELECT w.id, w.name FROM public.wishlists w WHERE w.user_id = $1",
        "SELECT al.action, al.resource FROM public.audit_logs al WHERE al.user_id = $1 ORDER BY al.created_at DESC",
        "SELECT p.id, p.name FROM public.products p WHERE p.price_cents BETWEEN $1 AND $2",
        "SELECT u.id, u.name, COUNT(o.id) FROM public.users u LEFT JOIN public.orders o ON u.id = o.user_id GROUP BY u.id, u.name",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

pub fn run_demo(output_dir: Option<&Path>) -> Result<(), anyhow::Error> {
    let raw = demo_schema();
    let graph = core::DatabaseGraph::from_raw_schema(raw.clone());

    let queries = demo_queries();
    let (usage, parsed_count) = analysis::build_usage_from_queries(&queries);
    let usage_report = analysis::compute_usage_report(&raw, &usage, parsed_count);
    let metrics =
        analysis::compute_all_metrics_with_operational(&graph, Some(&raw), Some(&usage_report));

    let total_tables = graph.table_count();
    let total_columns = raw.columns.len();
    let total_indexes = raw.indexes.len();
    let total_fks = raw.foreign_keys.len();

    let risk_for = |m: &analysis::TableMetrics| m.effective_risk.unwrap_or(m.risk_score);
    let t = Theme::detect();

    eprintln!();
    eprintln!("  {} {}", t.heading("dbscope"), t.dim("demo"));
    eprintln!("  {}", t.muted("e-commerce schema, no database required"));
    eprintln!();
    eprintln!("  {}", t.heading("Schema"));
    eprintln!(
        "    tables    {}  {}",
        t.value(&total_tables.to_string()),
        t.muted("(2 orphans: feature_flags, schema_migrations)")
    );
    eprintln!("    columns   {}", t.value(&total_columns.to_string()));
    eprintln!("    indexes   {}", t.value(&total_indexes.to_string()));
    eprintln!(
        "    FKs       {}  {}",
        t.value(&total_fks.to_string()),
        t.muted("(self-ref: categories.parent_id)")
    );
    eprintln!(
        "    queries   {} {}",
        t.value(&parsed_count.to_string()),
        t.muted("analyzed")
    );
    eprintln!();

    let critical = metrics.iter().filter(|m| risk_for(m) >= 0.75).count();
    let high = metrics
        .iter()
        .filter(|m| (0.5..0.75).contains(&risk_for(m)))
        .count();
    let medium = metrics
        .iter()
        .filter(|m| (0.25..0.5).contains(&risk_for(m)))
        .count();
    let orphans = metrics.iter().filter(|m| m.is_orphan).count();

    eprintln!("  {}", t.heading("Risk"));
    eprintln!(
        "    {} {}  {} {}  {} {}  {} {}",
        t.risk_critical(&critical.to_string()),
        t.muted("critical"),
        t.risk_high(&high.to_string()),
        t.muted("high"),
        t.risk_medium(&medium.to_string()),
        t.muted("medium"),
        t.muted(&orphans.to_string()),
        t.muted("orphans"),
    );

    let mut sorted: Vec<&analysis::TableMetrics> = metrics.iter().collect();
    sorted.sort_by(|a, b| {
        b.display_risk()
            .partial_cmp(&a.display_risk())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    eprintln!();
    eprintln!("  {}", t.heading("Top Risk"));
    for m in sorted.iter().take(5) {
        let risk = analysis::TableRisk::from_score(m.display_risk());
        let score_str = format!("{:.2}", m.display_risk());
        let label = format!("({})", risk.label());
        eprintln!(
            "    {:<30} {} {}",
            t.muted(&m.qualified_name),
            t.risk_color(m.display_risk(), &score_str),
            t.dim(&label),
        );
    }

    if !usage_report.index_suggestions.is_empty() {
        eprintln!();
        eprintln!("  {}", t.heading("Missing Indexes"));
        for s in usage_report.index_suggestions.iter().take(5) {
            eprintln!(
                "    {}  {}",
                t.muted(&format!("{}.{}", s.qualified_table, s.column_name)),
                t.dim(&format!("{} WHERE hits", s.in_where_count)),
            );
        }
    }

    if !usage_report.cold_tables.is_empty() {
        eprintln!();
        eprintln!("  {}", t.heading("Cold Tables"));
        for ct in &usage_report.cold_tables {
            eprintln!("    {}", t.muted(&ct.0));
        }
    }

    let out = output_dir.unwrap_or(Path::new("."));
    if !out.exists() {
        std::fs::create_dir_all(out)?;
    }

    let html_path = out.join("dbscope-report.html");
    let mut html_file = std::fs::File::create(&html_path)?;
    report::html::render(
        &mut html_file,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        Some(&usage_report),
    )?;

    let json_path = out.join("dbscope-report.json");
    let mut json_file = std::fs::File::create(&json_path)?;
    report::json::render(
        &mut json_file,
        &metrics,
        total_tables,
        total_columns,
        total_indexes,
        total_fks,
        Some(&usage_report),
    )?;

    let dot_path = out.join("dbscope-graph.dot");
    let mut dot_file = std::fs::File::create(&dot_path)?;
    report::graphviz::render(&mut dot_file, &graph, Some(&metrics))?;

    eprintln!();
    eprintln!("  {}", t.heading("Reports"));
    eprintln!(
        "    {}  {}",
        t.brand(&html_path.display().to_string()),
        t.muted("open in browser")
    );
    eprintln!(
        "    {}  {}",
        t.brand(&json_path.display().to_string()),
        t.muted("machine-readable")
    );
    eprintln!(
        "    {}  {}",
        t.brand(&dot_path.display().to_string()),
        t.muted("render with: dot -Tsvg ... -o graph.svg")
    );
    eprintln!();
    eprintln!(
        "  {} {}",
        t.dim("Try:"),
        t.brand("dbscope impact public.users")
    );
    eprintln!();

    Ok(())
}
