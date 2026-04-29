//! Live database integration tests.
//! These require real database connections and are skipped unless
//! the corresponding env vars are set (CI sets them via service containers).
//!
//! Run locally with:
//!   docker run -d --name pg -e POSTGRES_PASSWORD=test -p 5432:5432 postgres:16
//!   TEST_POSTGRES_URI=postgres://postgres:test@localhost:5432/postgres cargo test --test live_db_test

use std::env;

fn pg_uri() -> Option<String> {
    env::var("TEST_POSTGRES_URI").ok()
}

fn mysql_uri() -> Option<String> {
    env::var("TEST_MYSQL_URI").ok()
}

fn sqlite_uri() -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.db");
    std::mem::forget(dir);
    format!("sqlite://{}?mode=rwc", path.display())
}

async fn create_sqlite_pool(uri: &str) -> sqlx::SqlitePool {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::str::FromStr;
    let opts = SqliteConnectOptions::from_str(uri)
        .expect("parse sqlite uri")
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("connect to SQLite")
}

// --- PostgreSQL ---

#[tokio::test]
async fn postgres_extract_schema() {
    let uri = match pg_uri() {
        Some(u) => u,
        None => {
            eprintln!("SKIP: TEST_POSTGRES_URI not set");
            return;
        }
    };

    setup_postgres(&uri).await;

    let raw = dbscope::connectors::extract_schema(&uri)
        .await
        .expect("extract_schema should succeed on live Postgres");

    assert!(
        raw.tables.iter().any(|t| t.table_name == "users"),
        "should find 'users' table"
    );
    assert!(
        raw.tables.iter().any(|t| t.table_name == "posts"),
        "should find 'posts' table"
    );

    let user_cols: Vec<&str> = raw
        .columns
        .iter()
        .filter(|c| c.table_name == "users")
        .map(|c| c.column_name.as_str())
        .collect();
    assert!(user_cols.contains(&"id"), "users should have 'id' column");
    assert!(
        user_cols.contains(&"email"),
        "users should have 'email' column"
    );

    assert!(
        raw.foreign_keys
            .iter()
            .any(|fk| fk.from_table == "posts" && fk.to_table == "users"),
        "should find FK from posts to users"
    );

    assert!(
        raw.indexes.iter().any(|i| i.table_name == "users"),
        "should find indexes on users"
    );
}

#[tokio::test]
async fn postgres_full_pipeline() {
    let uri = match pg_uri() {
        Some(u) => u,
        None => {
            eprintln!("SKIP: TEST_POSTGRES_URI not set");
            return;
        }
    };

    setup_postgres(&uri).await;

    let raw = dbscope::connectors::extract_schema(&uri).await.unwrap();
    let graph = dbscope::core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = dbscope::analysis::compute_all_metrics_with_operational(&graph, Some(&raw), None);

    assert!(!metrics.is_empty(), "should compute metrics for tables");

    let users_metric = metrics
        .iter()
        .find(|m| m.qualified_name.ends_with("users"))
        .expect("should have metrics for users table");

    assert!(
        users_metric.risk_score >= 0.0 && users_metric.risk_score <= 1.0,
        "risk score should be in [0, 1]"
    );
    assert!(
        users_metric.centrality_in > 0,
        "users should have incoming references"
    );
}

#[tokio::test]
async fn postgres_lint_finds_issues() {
    let uri = match pg_uri() {
        Some(u) => u,
        None => {
            eprintln!("SKIP: TEST_POSTGRES_URI not set");
            return;
        }
    };

    setup_postgres(&uri).await;

    let raw = dbscope::connectors::extract_schema(&uri).await.unwrap();
    let violations = dbscope::cli::lint::lint_schema(&raw);

    // Our test schema has a table without a PK (audit_log)
    assert!(
        violations.iter().any(|v| v.rule == "missing-pk"),
        "should detect missing PK on audit_log"
    );
}

#[tokio::test]
async fn postgres_impact_analysis() {
    let uri = match pg_uri() {
        Some(u) => u,
        None => {
            eprintln!("SKIP: TEST_POSTGRES_URI not set");
            return;
        }
    };

    setup_postgres(&uri).await;

    let raw = dbscope::connectors::extract_schema(&uri).await.unwrap();
    let graph = dbscope::core::DatabaseGraph::from_raw_schema(raw.clone());
    let default_schema = raw.default_schema();

    let target =
        dbscope::analysis::ImpactTarget::parse_with_default("users", &default_schema).unwrap();
    let report = dbscope::analysis::compute_impact(&target, &graph, &raw, None)
        .expect("should compute impact for users");

    assert!(
        !report.fk_downstream_tables.is_empty(),
        "users should have downstream dependents"
    );
}

async fn setup_postgres(uri: &str) {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(uri)
        .await
        .expect("connect to Postgres");

    sqlx::query(
        "
        DROP TABLE IF EXISTS audit_log CASCADE;
        DROP TABLE IF EXISTS comments CASCADE;
        DROP TABLE IF EXISTS posts CASCADE;
        DROP TABLE IF EXISTS users CASCADE;
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "
        CREATE TABLE users (
            id SERIAL PRIMARY KEY,
            email VARCHAR(255) NOT NULL UNIQUE,
            name VARCHAR(100),
            created_at TIMESTAMP DEFAULT NOW()
        );
        CREATE TABLE posts (
            id SERIAL PRIMARY KEY,
            user_id INTEGER NOT NULL REFERENCES users(id),
            title VARCHAR(255) NOT NULL,
            body TEXT,
            published_at TIMESTAMP
        );
        CREATE TABLE comments (
            id SERIAL PRIMARY KEY,
            post_id INTEGER NOT NULL REFERENCES posts(id),
            user_id INTEGER NOT NULL REFERENCES users(id),
            body TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT NOW()
        );
        CREATE TABLE audit_log (
            event_id BIGINT,
            table_name VARCHAR(100),
            action VARCHAR(20),
            payload JSONB,
            created_at TIMESTAMP DEFAULT NOW()
        );
        CREATE INDEX idx_posts_user_id ON posts(user_id);
        CREATE INDEX idx_comments_post_id ON comments(post_id);
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
}

// --- MySQL ---

#[tokio::test]
async fn mysql_extract_schema() {
    let uri = match mysql_uri() {
        Some(u) => u,
        None => {
            eprintln!("SKIP: TEST_MYSQL_URI not set");
            return;
        }
    };

    setup_mysql(&uri).await;

    let raw = dbscope::connectors::extract_schema(&uri)
        .await
        .expect("extract_schema should succeed on live MySQL");

    assert!(
        raw.tables.iter().any(|t| t.table_name == "users"),
        "should find 'users' table"
    );
    assert!(
        raw.tables.iter().any(|t| t.table_name == "orders"),
        "should find 'orders' table"
    );

    assert!(
        raw.foreign_keys
            .iter()
            .any(|fk| fk.from_table == "orders" && fk.to_table == "users"),
        "should find FK from orders to users"
    );
}

#[tokio::test]
async fn mysql_default_schema_is_database_name() {
    let uri = match mysql_uri() {
        Some(u) => u,
        None => {
            eprintln!("SKIP: TEST_MYSQL_URI not set");
            return;
        }
    };

    setup_mysql(&uri).await;

    let raw = dbscope::connectors::extract_schema(&uri).await.unwrap();
    let default = raw.default_schema();

    // MySQL uses the database name as schema, not "public"
    assert_ne!(default, "public", "MySQL schema should not be 'public'");
    assert!(
        !default.is_empty(),
        "MySQL default schema should not be empty"
    );
}

async fn setup_mysql(uri: &str) {
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .connect(uri)
        .await
        .expect("connect to MySQL");

    sqlx::query("DROP TABLE IF EXISTS orders")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TABLE IF EXISTS users")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE users (
            id INT AUTO_INCREMENT PRIMARY KEY,
            email VARCHAR(255) NOT NULL UNIQUE,
            name VARCHAR(100)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE orders (
            id INT AUTO_INCREMENT PRIMARY KEY,
            user_id INT NOT NULL,
            total_cents INT NOT NULL,
            CONSTRAINT fk_orders_user FOREIGN KEY (user_id) REFERENCES users(id)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
}

// --- SQLite ---

#[tokio::test]
async fn sqlite_extract_schema() {
    let uri = sqlite_uri();

    setup_sqlite(&uri).await;

    // Use the plain sqlite:// URI (without ?mode=rwc) for extraction since file now exists
    let extract_uri = uri.split('?').next().unwrap_or(&uri);
    let raw = dbscope::connectors::extract_schema(extract_uri)
        .await
        .expect("extract_schema should succeed on SQLite");

    assert!(
        raw.tables.iter().any(|t| t.table_name == "tasks"),
        "should find 'tasks' table"
    );
    assert!(
        raw.tables.iter().any(|t| t.table_name == "projects"),
        "should find 'projects' table"
    );

    let default = raw.default_schema();
    assert_eq!(default, "main", "SQLite schema should be 'main'");
}

#[tokio::test]
async fn sqlite_full_pipeline() {
    let uri = sqlite_uri();

    setup_sqlite(&uri).await;

    let extract_uri = uri.split('?').next().unwrap_or(&uri);
    let raw = dbscope::connectors::extract_schema(extract_uri)
        .await
        .unwrap();
    let graph = dbscope::core::DatabaseGraph::from_raw_schema(raw.clone());
    let metrics = dbscope::analysis::compute_all_metrics_with_operational(&graph, Some(&raw), None);

    assert!(!metrics.is_empty());

    let projects = metrics
        .iter()
        .find(|m| m.qualified_name.contains("projects"))
        .expect("should have projects metrics");

    assert!(
        projects.centrality_in > 0,
        "projects should have references"
    );
}

async fn setup_sqlite(uri: &str) {
    let pool = create_sqlite_pool(uri).await;

    sqlx::query(
        "CREATE TABLE projects (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE tasks (
            id INTEGER PRIMARY KEY,
            project_id INTEGER NOT NULL REFERENCES projects(id),
            title TEXT NOT NULL,
            done INTEGER DEFAULT 0
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE comments (
            id INTEGER PRIMARY KEY,
            task_id INTEGER NOT NULL REFERENCES tasks(id),
            body TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
}
