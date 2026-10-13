-- 0010_experiments.sql
-- Experiment tracking. Phase H.
--
-- An experiment is a "what I'm trying" entity: a name, optional
-- description, and a few bookkeeping fields. A run is a single
-- attempt with hyperparameters, metrics, code revision, and hardware
-- notes. We do not try to replace MLflow — we keep the schema
-- friendly enough that a future MLflow sync (read-only) can import
-- runs.

CREATE TABLE experiments (
  id TEXT PRIMARY KEY,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'active',  -- 'active' | 'paused' | 'done' | 'archived'
  tags_json TEXT NOT NULL DEFAULT '[]',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_experiments_project ON experiments(project_id, status);
CREATE INDEX idx_experiments_updated ON experiments(updated_at DESC);

CREATE TABLE experiment_runs (
  id TEXT PRIMARY KEY,
  experiment_id TEXT NOT NULL REFERENCES experiments(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'running',  -- 'running' | 'completed' | 'failed' | 'cancelled'
  hyperparams_json TEXT NOT NULL DEFAULT '{}',
  metrics_json TEXT NOT NULL DEFAULT '{}',
  code_revision TEXT,
  code_path TEXT,
  hardware TEXT,
  notes TEXT,
  started_at INTEGER,
  ended_at INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_experiment_runs_exp ON experiment_runs(experiment_id, created_at DESC);
CREATE INDEX idx_experiment_runs_status ON experiment_runs(status, updated_at DESC);
