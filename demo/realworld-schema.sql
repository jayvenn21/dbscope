-- Real-world-style schema: messy, deep FK chains, orphans, multiple domains.
-- Use: psql "postgres://dbscope:dbscope@localhost:5433/dbscope" -f demo/realworld-schema.sql
-- Then: ./target/release/dbscope analyze --schema "postgres://dbscope:dbscope@localhost:5433/dbscope" -o realworld-reports
--
-- ~30 tables: tenants, users, projects, tasks, comments, tags, audit, config, logs.
-- Intentionally not curated: long chains, a few orphans, no indexes on some hot paths.

DROP TABLE IF EXISTS task_dependencies CASCADE;
DROP TABLE IF EXISTS task_tags CASCADE;
DROP TABLE IF EXISTS comment_mentions CASCADE;
DROP TABLE IF EXISTS comments CASCADE;
DROP TABLE IF EXISTS tasks CASCADE;
DROP TABLE IF EXISTS tags CASCADE;
DROP TABLE IF EXISTS project_members CASCADE;
DROP TABLE IF EXISTS projects CASCADE;
DROP TABLE IF EXISTS audit_events CASCADE;
DROP TABLE IF EXISTS user_sessions CASCADE;
DROP TABLE IF EXISTS user_preferences CASCADE;
DROP TABLE IF EXISTS tenant_settings CASCADE;
DROP TABLE IF EXISTS tenant_invites CASCADE;
DROP TABLE IF EXISTS users CASCADE;
DROP TABLE IF EXISTS tenants CASCADE;
DROP TABLE IF EXISTS config_flags CASCADE;
DROP TABLE IF EXISTS schema_migrations CASCADE;
DROP TABLE IF EXISTS app_logs CASCADE;
DROP TABLE IF EXISTS dead_letter_queue CASCADE;

CREATE TABLE tenants (
  id         SERIAL PRIMARY KEY,
  name       TEXT NOT NULL,
  slug       TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE users (
  id          SERIAL PRIMARY KEY,
  tenant_id   INT NOT NULL REFERENCES tenants(id),
  email       TEXT NOT NULL,
  name        TEXT,
  created_at  TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE tenant_invites (
  id        SERIAL PRIMARY KEY,
  tenant_id INT NOT NULL REFERENCES tenants(id),
  email     TEXT NOT NULL,
  token     TEXT NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE tenant_settings (
  id        SERIAL PRIMARY KEY,
  tenant_id INT NOT NULL REFERENCES tenants(id),
  key       TEXT NOT NULL,
  value     JSONB,
  UNIQUE (tenant_id, key)
);

CREATE TABLE user_preferences (
  id        SERIAL PRIMARY KEY,
  user_id   INT NOT NULL REFERENCES users(id),
  key       TEXT NOT NULL,
  value     TEXT,
  UNIQUE (user_id, key)
);

CREATE TABLE user_sessions (
  id         SERIAL PRIMARY KEY,
  user_id    INT NOT NULL REFERENCES users(id),
  token      TEXT NOT NULL UNIQUE,
  expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE projects (
  id          SERIAL PRIMARY KEY,
  tenant_id   INT NOT NULL REFERENCES tenants(id),
  owner_id    INT NOT NULL REFERENCES users(id),
  name        TEXT NOT NULL,
  slug        TEXT NOT NULL,
  created_at  TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE project_members (
  id         SERIAL PRIMARY KEY,
  project_id INT NOT NULL REFERENCES projects(id),
  user_id    INT NOT NULL REFERENCES users(id),
  role       TEXT NOT NULL,
  UNIQUE (project_id, user_id)
);

CREATE TABLE tags (
  id         SERIAL PRIMARY KEY,
  project_id INT NOT NULL REFERENCES projects(id),
  name       TEXT NOT NULL,
  color      TEXT,
  UNIQUE (project_id, name)
);

CREATE TABLE tasks (
  id          SERIAL PRIMARY KEY,
  project_id  INT NOT NULL REFERENCES projects(id),
  creator_id  INT NOT NULL REFERENCES users(id),
  assignee_id INT REFERENCES users(id),
  title       TEXT NOT NULL,
  status      TEXT NOT NULL,
  created_at  TIMESTAMPTZ DEFAULT now(),
  updated_at  TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE task_tags (
  task_id INT NOT NULL REFERENCES tasks(id),
  tag_id  INT NOT NULL REFERENCES tags(id),
  PRIMARY KEY (task_id, tag_id)
);

CREATE TABLE task_dependencies (
  task_id         INT NOT NULL REFERENCES tasks(id),
  depends_on_id   INT NOT NULL REFERENCES tasks(id),
  PRIMARY KEY (task_id, depends_on_id),
  CHECK (task_id != depends_on_id)
);

CREATE TABLE comments (
  id         SERIAL PRIMARY KEY,
  task_id    INT NOT NULL REFERENCES tasks(id),
  author_id  INT NOT NULL REFERENCES users(id),
  body       TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE comment_mentions (
  id          SERIAL PRIMARY KEY,
  comment_id  INT NOT NULL REFERENCES comments(id),
  user_id     INT NOT NULL REFERENCES users(id)
);

CREATE TABLE audit_events (
  id          SERIAL PRIMARY KEY,
  tenant_id   INT NOT NULL REFERENCES tenants(id),
  user_id     INT REFERENCES users(id),
  action      TEXT NOT NULL,
  resource    TEXT,
  details     JSONB,
  created_at  TIMESTAMPTZ DEFAULT now()
);

-- Orphans: no FK in/out (config, migrations, logs, dlq)
CREATE TABLE config_flags (
  id    SERIAL PRIMARY KEY,
  key   TEXT NOT NULL UNIQUE,
  value BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE schema_migrations (
  version BIGINT PRIMARY KEY,
  name    TEXT NOT NULL
);

CREATE TABLE app_logs (
  id        SERIAL PRIMARY KEY,
  level     TEXT NOT NULL,
  message   TEXT,
  metadata  JSONB,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE dead_letter_queue (
  id         SERIAL PRIMARY KEY,
  payload    JSONB NOT NULL,
  error      TEXT,
  created_at TIMESTAMPTZ DEFAULT now()
);

-- Intentionally few indexes: stress index-suggestion and cold columns
CREATE INDEX idx_users_tenant_id ON users(tenant_id);
CREATE INDEX idx_projects_tenant_id ON projects(tenant_id);
CREATE INDEX idx_tasks_project_id ON tasks(project_id);
CREATE INDEX idx_comments_task_id ON comments(task_id);
