-- 0001_core.sql
-- Core app shell: settings, jobs, audit.

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  status TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  payload TEXT NOT NULL,
  result TEXT,
  error TEXT,
  progress REAL NOT NULL DEFAULT 0.0,
  attempts INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  started_at INTEGER,
  finished_at INTEGER
);
CREATE INDEX idx_jobs_status ON jobs(status, priority DESC, created_at);

CREATE TABLE audit_events (
  id TEXT PRIMARY KEY,
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  target TEXT,
  detail TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_audit_created ON audit_events(created_at DESC);
