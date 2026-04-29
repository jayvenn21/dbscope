# How dbscope Is Different

## Schema Intelligence vs. Migration Linting

The database tooling ecosystem has two distinct layers:

1. **Migration linters**: Analyze SQL migration files for dangerous DDL patterns (missing `CONCURRENTLY`, lock mode risks, type rewrites)
2. **Schema intelligence**: Analyze the live database schema for structural risk (FK topology, cycles, centrality, blast radius)

Most tools in the market are migration linters. dbscope is the only tool that does schema intelligence with a graph-based risk model.

## Detailed Comparison

### squawk (1,100+ stars)

- **What it does:** Lints SQL files for 31 dangerous PostgreSQL patterns
- **How it works:** Pattern-matching on DDL statements
- **Limitation:** No database connection, no structural analysis, no risk scoring
- **Relationship to dbscope:** Complementary. Use squawk in pre-commit for fast DDL linting. Use dbscope in CI for structural risk assessment.

### Atlas `migrate lint`

- **What it does:** Migration linting with a dev database for simulation
- **How it works:** Compares migration state against dev DB
- **Limitation:** Now Pro-only (paid). No graph-based structural risk. Primarily migration lifecycle management.
- **Relationship to dbscope:** Different layer. Atlas manages the migration lifecycle. dbscope analyzes the schema structure.

### pgfence

- **What it does:** Lock mode analysis, risk scoring, ORM migration support
- **How it works:** Parses DDL (including ORM patterns) and predicts lock behavior
- **Limitation:** PostgreSQL only. No graph-based risk model. Cloud upsell for team features.
- **Relationship to dbscope:** Overlap on migration preview, but pgfence focuses on lock prediction while dbscope focuses on structural risk.

### pg-blast-radius

- **What it does:** Workload-aware query blocking forecasts
- **How it works:** Connects to pg_stat_statements to predict which queries will be blocked
- **Limitation:** PostgreSQL only. Focuses on migration impact, not ongoing schema health.
- **Relationship to dbscope:** Complementary. pg-blast-radius tells you what queries will be blocked during a migration. dbscope tells you the structural risk of the tables involved.

### MigrationPilot

- **What it does:** 83 safety rules, auto-fix, GitHub Action, MCP server, $19/mo Pro
- **How it works:** Real PostgreSQL parser (libpg-query), pattern-based rule engine
- **Limitation:** PostgreSQL only. Node.js. Migration-focused, not schema-focused.
- **Relationship to dbscope:** Different problem. MigrationPilot asks "is this migration safe?" dbscope asks "what is the structural health of my schema?"

## The Recommended Stack

For comprehensive database safety:

```
Pre-commit:  squawk (fast DDL linting, 31 rules)
CI pipeline: dbscope ci (structural risk gating, policy enforcement)
PR review:   MigrationPilot or pgfence (migration-specific safety)
Ongoing:     dbscope Cloud (schema health monitoring, trending, alerts)
```

dbscope is not competing with migration linters. It is the structural intelligence layer that makes migration decisions better informed.

## What Only dbscope Does

1. **Graph-based risk model**: Builds a petgraph of your entire schema. Computes FK depth, cycle detection, centrality, and transitive reach. No other tool does this.

2. **Multi-database**: PostgreSQL, MySQL, SQLite, ClickHouse through a universal connector interface. Every competitor is PostgreSQL-only.

3. **Operational weighting**: Multiplies structural risk by live `pg_stat_user_tables` data (row counts, insert/update/delete activity). Combines static and dynamic analysis.

4. **Migration simulation with risk delta**: `dbscope preview` doesn't just say "this DDL is dangerous." It shows how the entire risk profile of your schema changes.

5. **Deterministic, explainable scoring**: Every score has a documented formula. No heuristics, no AI, no probabilistic analysis. Same schema = same score.

6. **Policy engine**: Define organizational risk thresholds in YAML. Enforce in CI and migration preview.
