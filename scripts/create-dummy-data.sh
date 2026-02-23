#!/usr/bin/env bash
# Creates dummy-data/ with schema.sql and queries.txt for trying dbscope.
# Run once, then: docker compose up -d && dbscope analyze --schema 'postgres://dbscope:dbscope@localhost:5432/dbscope' ...

set -e
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$REPO_ROOT/dummy-data"

cat > "$REPO_ROOT/dummy-data/schema.sql" << 'EOF'
-- Dummy schema for dbscope: users -> posts, standalone (orphan).
CREATE TABLE IF NOT EXISTS users (
  id    SERIAL PRIMARY KEY,
  email TEXT
);

CREATE TABLE IF NOT EXISTS posts (
  id      SERIAL PRIMARY KEY,
  user_id INT NOT NULL REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS standalone (
  id SERIAL PRIMARY KEY
);
EOF

cat > "$REPO_ROOT/dummy-data/queries.txt" << 'EOF'
SELECT id FROM public.users WHERE id = 1
SELECT user_id FROM public.posts WHERE user_id = 2
SELECT * FROM public.users WHERE email IS NOT NULL
SELECT p.id, u.email FROM public.posts p JOIN public.users u ON p.user_id = u.id
EOF

echo "Created dummy-data/schema.sql and dummy-data/queries.txt"