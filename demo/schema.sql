-- Serious demo schema for DBScope: ~14 tables, realistic FK web, indexes, and intentional bad patterns.
-- Use with: psql postgres://dbscope:dbscope@localhost:5433/dbscope -f demo/schema.sql
-- Then: ./target/release/dbscope analyze --schema "postgres://dbscope:dbscope@localhost:5433/dbscope" --query-log demo/queries.txt -o demo-reports

-- Drop in reverse dependency order so we can re-run
DROP TABLE IF EXISTS permissions CASCADE;
DROP TABLE IF EXISTS role_assignments CASCADE;
DROP TABLE IF EXISTS experiments CASCADE;
DROP TABLE IF EXISTS feature_flags CASCADE;
DROP TABLE IF EXISTS audit_logs CASCADE;
DROP TABLE IF EXISTS payments CASCADE;
DROP TABLE IF EXISTS sessions CASCADE;
DROP TABLE IF EXISTS notifications CASCADE;
DROP TABLE IF EXISTS likes CASCADE;
DROP TABLE IF EXISTS comments CASCADE;
DROP TABLE IF EXISTS posts CASCADE;
DROP TABLE IF EXISTS roles CASCADE;
DROP TABLE IF EXISTS users CASCADE;

-- Core
CREATE TABLE users (
  id         SERIAL PRIMARY KEY,
  email      TEXT NOT NULL,
  name       TEXT,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE roles (
  id   SERIAL PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE posts (
  id         SERIAL PRIMARY KEY,
  user_id    INT NOT NULL REFERENCES users(id),
  title      TEXT NOT NULL,
  body       TEXT,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE comments (
  id         SERIAL PRIMARY KEY,
  post_id    INT NOT NULL REFERENCES posts(id),
  user_id    INT NOT NULL REFERENCES users(id),
  body       TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE likes (
  id        SERIAL PRIMARY KEY,
  user_id   INT NOT NULL REFERENCES users(id),
  post_id   INT REFERENCES posts(id),
  comment_id INT REFERENCES comments(id),
  created_at TIMESTAMPTZ DEFAULT now(),
  CONSTRAINT likes_target CHECK (post_id IS NOT NULL OR comment_id IS NOT NULL)
);

CREATE TABLE notifications (
  id        SERIAL PRIMARY KEY,
  user_id   INT NOT NULL REFERENCES users(id),
  kind      TEXT NOT NULL,
  payload   JSONB,
  read_at   TIMESTAMPTZ,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE sessions (
  id           SERIAL PRIMARY KEY,
  user_id      INT NOT NULL REFERENCES users(id),
  token        TEXT NOT NULL UNIQUE,
  expires_at   TIMESTAMPTZ NOT NULL,
  created_at   TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE payments (
  id          SERIAL PRIMARY KEY,
  user_id     INT NOT NULL REFERENCES users(id),
  amount_cents INT NOT NULL,
  currency    TEXT NOT NULL DEFAULT 'USD',
  status      TEXT NOT NULL,
  created_at  TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE audit_logs (
  id         SERIAL PRIMARY KEY,
  user_id    INT REFERENCES users(id),
  action     TEXT NOT NULL,
  resource   TEXT,
  details    JSONB,
  created_at TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE feature_flags (
  id          SERIAL PRIMARY KEY,
  key         TEXT NOT NULL UNIQUE,
  enabled     BOOLEAN NOT NULL DEFAULT false,
  description TEXT,
  updated_at  TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE experiments (
  id          SERIAL PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,
  variant     TEXT NOT NULL,
  user_id     INT REFERENCES users(id),
  created_at  TIMESTAMPTZ DEFAULT now()
);

CREATE TABLE role_assignments (
  id      SERIAL PRIMARY KEY,
  user_id INT NOT NULL REFERENCES users(id),
  role_id INT NOT NULL REFERENCES roles(id),
  UNIQUE (user_id, role_id)
);

CREATE TABLE permissions (
  id          SERIAL PRIMARY KEY,
  role_id     INT NOT NULL REFERENCES roles(id),
  resource    TEXT NOT NULL,
  action      TEXT NOT NULL,
  UNIQUE (role_id, resource, action)
);

-- Good indexes
CREATE INDEX idx_posts_user_id ON posts(user_id);
CREATE INDEX idx_posts_created_at ON posts(created_at);
CREATE INDEX idx_comments_post_id ON comments(post_id);
CREATE INDEX idx_comments_user_id ON comments(user_id);
CREATE INDEX idx_likes_user_id ON likes(user_id);
CREATE INDEX idx_likes_post_id ON likes(post_id);
CREATE INDEX idx_notifications_user_id ON notifications(user_id);
CREATE INDEX idx_notifications_read_at ON notifications(user_id) WHERE read_at IS NULL;
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX idx_payments_user_id ON payments(user_id);
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);
CREATE UNIQUE INDEX idx_sessions_token ON sessions(token);

-- Composite index (realistic)
CREATE INDEX idx_comments_post_created ON comments(post_id, created_at);

-- Intentional bad pattern: no index on notifications.kind (often filtered in WHERE)
-- Intentional: audit_logs.action filtered often but no index (will show in index suggestions)
