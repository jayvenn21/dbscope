//! DBScope — static + dynamic analysis for relational databases.
//! Understand your database before you touch it.

use clap::Parser;
use dbscope::cli;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "dbscope", about = "Understand your database before you touch it.")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Analyze schema: extract from DB, build graph, compute metrics, emit reports.
    Analyze {
        /// Postgres connection URI (e.g. postgres://user:pass@localhost/dbname)
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Output directory for report files (default: current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Path to query log file (one SQL statement per line) for Phase 2: cold/hot tables, index suggestions
        #[arg(long)]
        query_log: Option<PathBuf>,
    },

    /// Blast radius: what is affected by changing a table or column (FK downstream, indexes, queries).
    Impact {
        /// Target: table (e.g. users), table.column (e.g. users.email), or schema.table.column
        target: String,

        /// Postgres connection URI (for schema)
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Query log file to count affected queries
        #[arg(long)]
        query_log: Option<PathBuf>,
    },

    /// CI mode: check schema (and optional migration) risk; exit 1 if above threshold.
    Ci {
        /// Postgres connection URI
        #[arg(long, env = "DBSCOPE_SCHEMA_URI")]
        schema: String,

        /// Migration file to simulate (DDL: DROP TABLE, CREATE TABLE, ALTER ADD FK)
        #[arg(long)]
        migration: Option<PathBuf>,

        /// Policy file (YAML: max_table_risk, no_cycles, no_orphans, max_blast_radius_percent). Overrides --threshold when set.
        #[arg(long)]
        policy: Option<PathBuf>,

        /// Fail if any table risk score exceeds this (0–1). Default 0.5. Ignored if --policy is set.
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
    },

    /// Preview migration impact: structural delta, risk delta, blast radius, policy check.
    Preview {
        /// Migration file (DDL) to simulate
        migration: PathBuf,

        /// Postgres connection URI
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
    },
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();
    match args.command {
        Command::Analyze { schema, output, query_log } => {
            cli::run_analyze(&schema, output.as_deref(), query_log.as_deref()).await?;
        }
        Command::Impact { target, schema, query_log } => {
            cli::run_impact(&target, &schema, query_log.as_deref()).await?;
        }
        Command::Ci { schema, migration, policy, threshold } => {
            cli::run_ci(&schema, migration.as_deref(), policy.as_deref(), threshold).await?;
        }
        Command::Plan { action, target, schema } => {
            if action.eq_ignore_ascii_case("drop") {
                cli::run_plan_drop(&schema, &target).await?;
            } else {
                anyhow::bail!("Unknown plan action: {}. Use 'drop'.", action);
            }
        }
        Command::Preview { migration, schema, query_log, policy } => {
            cli::run_preview(&schema, &migration, query_log.as_deref(), policy.as_deref()).await?;
        }
        Command::Summarize { schema, query_log } => {
            cli::run_summarize(&schema, query_log.as_deref()).await?;
        }
        Command::Explain { kind, target, column, schema, query_log } => {
            cli::run_explain(&kind, &target, column.as_deref(), &schema, query_log.as_deref()).await?;
        }
    }
    Ok(())
}
