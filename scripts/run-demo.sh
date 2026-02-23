#!/usr/bin/env bash
# Load demo schema into Postgres and run dbscope (analyze + impact).
# Requires: Docker Compose Postgres on port 5433, or local Postgres with dbscope DB.
# Usage: ./scripts/run-demo.sh [schema_uri]
# Default URI: postgres://dbscope:dbscope@localhost:5433/dbscope

set -e
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
URI="${1:-postgres://dbscope:dbscope@localhost:5433/dbscope}"

echo "Loading demo schema into $URI ..."
psql "$URI" -f "$REPO_ROOT/demo/schema.sql" || { echo "Run: docker compose up -d; then create dummy-data (./scripts/create-dummy-data.sh) or use a DB with demo/schema.sql loaded."; exit 1; }

DBSCOPE_BIN="$REPO_ROOT/target/release/dbscope"
[ -x "$DBSCOPE_BIN" ] || DBSCOPE_BIN="$REPO_ROOT/target/debug/dbscope"
[ -x "$DBSCOPE_BIN" ] || { echo "Build first: cargo build --release"; exit 1; }

echo "Running dbscope analyze (schema + query log) ..."
"$DBSCOPE_BIN" analyze --schema "$URI" --query-log "$REPO_ROOT/demo/queries.txt" -o "$REPO_ROOT/demo-reports"

echo "Running dbscope impact public.users ..."
"$DBSCOPE_BIN" impact public.users --schema "$URI" --query-log "$REPO_ROOT/demo/queries.txt"

echo "Demo reports written to demo-reports/ (if -o was used). Open demo-reports/dbscope-report.html for screenshots."
