//! DBScope: static and dynamic analysis for relational databases.
//! Understand your database before you touch it.

use clap::Parser;
use clap_complete::Shell;
use dbscope::cli;
use dbscope::connectors;
use dbscope::core;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "dbscope",
    version,
    about = "Understand your database before you touch it.",
    long_about = "Read-only schema intelligence for SQL databases.\n\
        Graph-based risk scoring · Blast radius analysis · CI gating · Migration preview\n\
        Schema diffing · Lint rules · Dependency trees · Snapshots\n\
        Supports: PostgreSQL, MySQL, SQLite, ClickHouse"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Analyze schema: extract from DB, build graph, compute metrics, emit reports.
    Analyze {
        /// Database connection URI (e.g. postgres://user:pass@localhost/dbname)
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Output directory for report files (default: current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Path to query log file (one SQL statement per line) for cold/hot tables, index suggestions
        #[arg(long)]
        query_log: Option<PathBuf>,

        /// Report formats to generate (comma-separated: md,html,json,dot). Default: all.
        #[arg(long)]
        format: Option<String>,
    },

    /// Blast radius: what is affected by changing a table or column (FK downstream, indexes, queries).
    Impact {
        /// Target: table (e.g. users), table.column (e.g. users.email), or schema.table.column
        target: String,

        /// Database connection URI
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Query log file to count affected queries
        #[arg(long)]
        query_log: Option<PathBuf>,

        /// Output as JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },

    /// CI mode: check schema (and optional migration) risk; exit 1 if above threshold.
    Ci {
        /// Database connection URI
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Migration file to simulate (DDL: DROP TABLE, CREATE TABLE, ALTER ADD FK)
        #[arg(long)]
        migration: Option<PathBuf>,

        /// Policy file (YAML: max_table_risk, no_cycles, no_orphans, max_blast_radius_percent). Overrides --threshold when set.
        #[arg(long)]
        policy: Option<PathBuf>,

        /// Fail if any table risk score exceeds this (0-1). Default 0.5. Ignored if --policy is set.
        #[arg(long, default_value = "0.5")]
        threshold: f64,
    },

    /// Safe refactor plan: steps to drop a table (remove FKs first, then drop).
    Plan {
        /// Subcommand: drop
        action: String,
        /// Target table (e.g. users or public.users)
        target: String,
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Output as JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },

    /// Preview migration impact: structural delta, risk delta, blast radius, policy check.
    Preview {
        /// Migration file (DDL) to simulate
        migration: PathBuf,

        /// Database connection URI
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Query log file to count broken queries
        #[arg(long)]
        query_log: Option<PathBuf>,

        /// Policy file (YAML). If absent, only reports; with policy, exits 1 on violation.
        #[arg(long)]
        policy: Option<PathBuf>,
    },

    /// Summarize architecture: table count, risk overview, orphans, cycles, cold/hot (if query log).
    Summarize {
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        #[arg(long)]
        query_log: Option<PathBuf>,

        /// Output as JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },

    /// Explain risk or index suggestion in plain language.
    Explain {
        /// What to explain: "risk" or "index-suggestion"
        kind: String,

        /// For risk: table name (e.g. public.users). For index-suggestion: qualified table (e.g. public.posts)
        target: String,

        /// For index-suggestion only: column name (e.g. user_id)
        column: Option<String>,

        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        #[arg(long)]
        query_log: Option<PathBuf>,

        /// Output as JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },

    /// Run a demo analysis on an embedded e-commerce schema (no database required).
    Demo {
        /// Output directory for demo reports (default: current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Save current schema to a JSON snapshot file for offline diffing and auditing.
    Snapshot {
        /// Database connection URI
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Output file path (e.g. schema-2024-01-15.json)
        #[arg(short, long, default_value = "dbscope-snapshot.json")]
        output: PathBuf,
    },

    /// Compare two schema snapshots or a snapshot vs. live database. Shows structural delta.
    Diff {
        /// Path to the "before" snapshot JSON file
        before: PathBuf,

        /// Path to "after" snapshot file OR a database connection URI
        after: String,

        /// Output as JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },

    /// Detect schema anti-patterns: missing PKs, wide tables, missing FK indexes, naming issues.
    Lint {
        /// Database connection URI
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Output as JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },

    /// Show the full dependency tree for a table (upstream and downstream FK chains).
    Deps {
        /// Target table (e.g. users or public.users)
        target: String,

        /// Database connection URI
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Output as JSON instead of human-readable text
        #[arg(long)]
        json: bool,
    },

    /// Generate shell completions for bash, zsh, fish, or powershell.
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },

    /// Run as an MCP (Model Context Protocol) server over stdio for AI assistants.
    Mcp,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let result = run(args).await;
    match result {
        Ok(()) => {}
        Err(e) => {
            let msg = format!("{e}");
            if msg.contains("policy violation") || msg.contains("risk check failed") {
                eprintln!("{e:#}");
                std::process::exit(1);
            }
            eprintln!("error: {e:#}");
            std::process::exit(2);
        }
    }
}

async fn run(args: Args) -> Result<(), anyhow::Error> {
    match args.command {
        Command::Analyze {
            schema,
            output,
            query_log,
            format,
        } => {
            let formats = format.as_deref().map(parse_formats).transpose()?;
            cli::run_analyze(
                &schema,
                output.as_deref(),
                query_log.as_deref(),
                formats.as_deref(),
            )
            .await?;
        }
        Command::Impact {
            target,
            schema,
            query_log,
            json,
        } => {
            cli::run_impact(&target, &schema, query_log.as_deref(), json).await?;
        }
        Command::Ci {
            schema,
            migration,
            policy,
            threshold,
        } => {
            cli::run_ci(&schema, migration.as_deref(), policy.as_deref(), threshold).await?;
        }
        Command::Plan {
            action,
            target,
            schema,
            json,
        } => {
            if action.eq_ignore_ascii_case("drop") {
                cli::run_plan_drop(&schema, &target, json).await?;
            } else {
                anyhow::bail!("Unknown plan action: {}. Use 'drop'.", action);
            }
        }
        Command::Preview {
            migration,
            schema,
            query_log,
            policy,
        } => {
            cli::run_preview(&schema, &migration, query_log.as_deref(), policy.as_deref()).await?;
        }
        Command::Summarize {
            schema,
            query_log,
            json,
        } => {
            cli::run_summarize(&schema, query_log.as_deref(), json).await?;
        }
        Command::Explain {
            kind,
            target,
            column,
            schema,
            query_log,
            json,
        } => {
            cli::run_explain(
                &kind,
                &target,
                column.as_deref(),
                &schema,
                query_log.as_deref(),
                json,
            )
            .await?;
        }
        Command::Demo { output } => {
            cli::demo::run_demo(output.as_deref())?;
        }
        Command::Snapshot { schema, output } => {
            cli::snapshot::run_snapshot(&schema, &output).await?;
        }
        Command::Diff {
            before,
            after,
            json,
        } => {
            cli::diff::run_diff(&before, &after, json).await?;
        }
        Command::Lint { schema, json } => {
            let raw = connectors::extract_schema(&schema).await?;
            cli::lint::run_lint(&raw, json)?;
        }
        Command::Deps {
            target,
            schema,
            json,
        } => {
            let raw = connectors::extract_schema(&schema).await?;
            let graph = core::DatabaseGraph::from_raw_schema(raw.clone());
            let qualified = if target.contains('.') {
                target
            } else {
                format!("{}.{}", raw.default_schema(), target)
            };
            cli::deps::run_deps(&graph, &qualified, json)?;
        }
        Command::Completions { shell } => {
            cli::completions::run_completions::<Args>(shell);
        }
        Command::Mcp => {
            cli::mcp::run_mcp()?;
        }
    }
    Ok(())
}

fn parse_formats(s: &str) -> Result<Vec<String>, anyhow::Error> {
    let valid = ["md", "html", "json", "dot"];
    let formats: Vec<String> = s.split(',').map(|f| f.trim().to_lowercase()).collect();
    for f in &formats {
        if !valid.contains(&f.as_str()) {
            anyhow::bail!("Unknown format '{}'. Valid: md, html, json, dot", f);
        }
    }
    Ok(formats)
}
