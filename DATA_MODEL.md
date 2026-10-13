# NEXUS — Data Model

The schema below is the **target** model. Phase 1 ships the core tables; later phases add the rest as Migrations `0002..0011`. Every table uses an `id TEXT PRIMARY KEY` (ULID/UUIDv7) and a `created_at INTEGER` epoch millis. Foreign keys are enabled; indexes are explicit.

```sql
-- 0001_core.sql
CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,          -- JSON
  updated_at INTEGER NOT NULL
);

CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at INTEGER NOT NULL,
  description TEXT
);

CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,            -- 'ingest' | 'embed' | 'reindex' | 'agent' | 'experiment' | 'export'
  status TEXT NOT NULL,          -- 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled'
  priority INTEGER NOT NULL,
  payload TEXT NOT NULL,         -- JSON
  result TEXT,                   -- JSON
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
  actor TEXT NOT NULL,           -- 'user' | 'agent:<conv>' | 'system'
  action TEXT NOT NULL,
  target TEXT,
  detail TEXT,                   -- JSON
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_audit_created ON audit_events(created_at DESC);

-- 0002_models.sql
CREATE TABLE model_profiles (
  id TEXT PRIMARY KEY,
  role TEXT NOT NULL,            -- 'general' | 'coding' | 'embedding' | 'reranker'
  display_name TEXT NOT NULL,
  provider TEXT NOT NULL,        -- 'lmstudio' | 'llamacpp' | 'ollama'
  model_id TEXT NOT NULL,        -- provider-side model identifier
  gguf_path TEXT,                -- for display only; backend is started by user
  context_len INTEGER NOT NULL DEFAULT 4096,
  gpu_layers INTEGER,            -- null = backend default
  temperature REAL NOT NULL DEFAULT 0.7,
  top_p REAL NOT NULL DEFAULT 0.95,
  max_tokens INTEGER NOT NULL DEFAULT 1024,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE provider_endpoints (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  base_url TEXT NOT NULL,
  api_key_ref TEXT,              -- references secret id, if any
  managed_lifecycle INTEGER NOT NULL DEFAULT 0,
  last_seen_status TEXT,
  last_seen_at INTEGER,
  created_at INTEGER NOT NULL
);

-- 0003_conversations.sql
CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  project_id TEXT,               -- nullable
  title TEXT NOT NULL,
  model_profile_id TEXT,
  system_prompt TEXT,
  archived INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_conv_updated ON conversations(updated_at DESC);

CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  role TEXT NOT NULL,            -- 'system' | 'user' | 'assistant' | 'tool'
  content TEXT NOT NULL,
  status TEXT NOT NULL,          -- 'complete' | 'streaming' | 'cancelled' | 'error'
  token_count INTEGER,
  latency_ms INTEGER,
  model_profile_id TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_msg_conv_created ON messages(conversation_id, created_at);

CREATE TABLE message_sources (
  message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  chunk_id TEXT NOT NULL,
  score REAL NOT NULL,
  citation_index INTEGER NOT NULL,
  PRIMARY KEY (message_id, citation_index)
);

-- 0004_vault.sql
CREATE TABLE workspaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE projects (
  id TEXT PRIMARY KEY,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  description TEXT,
  instructions TEXT,
  goals TEXT,                    -- JSON array
  archived INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  parent_id TEXT REFERENCES collections(id) ON DELETE CASCADE,
  created_at INTEGER NOT NULL
);

CREATE TABLE sources (
  id TEXT PRIMARY KEY,
  root_path TEXT NOT NULL,
  display_name TEXT NOT NULL,
  kind TEXT NOT NULL,            -- 'directory' | 'file' | 'note' | 'paper' | 'experiment'
  enabled INTEGER NOT NULL DEFAULT 1,
  last_indexed_at INTEGER,
  created_at INTEGER NOT NULL
);

CREATE TABLE documents (
  id TEXT PRIMARY KEY,
  source_id TEXT REFERENCES sources(id) ON DELETE CASCADE,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  path TEXT NOT NULL,
  mime_type TEXT,
  size_bytes INTEGER NOT NULL,
  content_hash TEXT NOT NULL,    -- SHA-256 of original bytes
  status TEXT NOT NULL,          -- 'pending' | 'extracting' | 'chunking' | 'embedding' | 'indexed' | 'failed'
  error TEXT,
  metadata TEXT,                 -- JSON
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_documents_hash ON documents(content_hash);
CREATE INDEX idx_documents_status ON documents(status);

CREATE TABLE document_chunks (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL,
  text TEXT NOT NULL,
  token_estimate INTEGER,
  page INTEGER,
  section TEXT,
  start_offset INTEGER,
  end_offset INTEGER,
  metadata TEXT,
  UNIQUE(document_id, ordinal)
);
CREATE INDEX idx_chunks_doc ON document_chunks(document_id);

-- FTS5 mirror table populated by triggers
CREATE VIRTUAL TABLE chunks_fts USING fts5(
  text,
  content='document_chunks',
  content_rowid='rowid',
  tokenize='porter unicode61'
);

-- vectors stored by sqlite-vec
CREATE VIRTUAL TABLE chunk_vectors USING vec0(
  chunk_id TEXT PRIMARY KEY,
  embedding float[384]            -- dimension set at migration time
);

CREATE TABLE tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);
CREATE TABLE document_tags (
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (document_id, tag_id)
);

CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_notes_project ON notes(project_id, updated_at DESC);

-- 0005_tasks.sql
CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL,          -- 'open' | 'in_progress' | 'done' | 'cancelled'
  priority TEXT NOT NULL,        -- 'low' | 'normal' | 'high' | 'urgent'
  due_at INTEGER,
  checklist TEXT,                -- JSON
  notes TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  completed_at INTEGER
);
CREATE INDEX idx_tasks_status ON tasks(status, due_at);

-- 0006_research.sql
CREATE TABLE papers (
  id TEXT PRIMARY KEY,
  source_document_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  authors TEXT,
  year INTEGER,
  venue TEXT,
  doi TEXT,
  abstract TEXT,
  metadata TEXT,                 -- JSON
  created_at INTEGER NOT NULL
);

CREATE TABLE paper_claims (
  id TEXT PRIMARY KEY,
  paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,            -- 'problem' | 'method' | 'dataset' | 'metric' | 'result' | 'limitation' | 'future'
  text TEXT NOT NULL,
  page INTEGER,
  section TEXT,
  verbatim TEXT,
  confidence REAL NOT NULL DEFAULT 0.0,
  verified INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_claims_paper ON paper_claims(paper_id);

CREATE TABLE research_projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  question TEXT,
  objectives TEXT,
  scope TEXT,
  methodology TEXT,
  related_work TEXT,
  datasets TEXT,                 -- JSON
  results TEXT,
  limitations TEXT,
  open_questions TEXT,
  references TEXT,               -- JSON
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE paper_comparisons (
  id TEXT PRIMARY KEY,
  research_project_id TEXT REFERENCES research_projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  matrix TEXT NOT NULL,          -- JSON: rows=claims, cols=papers
  created_at INTEGER NOT NULL
);

CREATE TABLE research_reports (
  id TEXT PRIMARY KEY,
  research_project_id TEXT REFERENCES research_projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  provenance TEXT NOT NULL,      -- JSON: section -> list of claim ids
  format TEXT NOT NULL DEFAULT 'md',
  created_at INTEGER NOT NULL
);

-- 0007_experiments.sql
CREATE TABLE experiments (
  id TEXT PRIMARY KEY,
  research_project_id TEXT REFERENCES research_projects(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  run_id TEXT NOT NULL UNIQUE,
  model_profile_id TEXT,
  dataset_version TEXT,
  dataset_fingerprint TEXT,
  code_revision TEXT,
  hyperparameters TEXT,         -- JSON
  seed INTEGER,
  hardware_info TEXT,            -- JSON
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  status TEXT NOT NULL,
  notes TEXT,
  error TEXT
);

CREATE TABLE experiment_metrics (
  id TEXT PRIMARY KEY,
  experiment_id TEXT NOT NULL REFERENCES experiments(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  value REAL NOT NULL,
  source TEXT NOT NULL,          -- 'logged' | 'manual'
  step INTEGER,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_metrics_exp ON experiment_metrics(experiment_id, name);

CREATE TABLE experiment_artifacts (
  id TEXT PRIMARY KEY,
  experiment_id TEXT NOT NULL REFERENCES experiments(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  path TEXT NOT NULL,
  size_bytes INTEGER,
  created_at INTEGER NOT NULL
);

-- 0008_agent.sql
CREATE TABLE tools (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT NOT NULL,
  json_schema TEXT NOT NULL,     -- JSON schema for args
  required_permission TEXT NOT NULL,
  timeout_ms INTEGER NOT NULL DEFAULT 30000,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL
);

CREATE TABLE tool_executions (
  id TEXT PRIMARY KEY,
  agent_run_id TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  arguments TEXT NOT NULL,       -- JSON
  validated_arguments TEXT,
  result TEXT,                   -- JSON
  error TEXT,
  status TEXT NOT NULL,
  permission_grant TEXT,         -- 'auto' | 'user' | 'policy' | 'denied'
  started_at INTEGER NOT NULL,
  finished_at INTEGER
);
CREATE INDEX idx_texec_run ON tool_executions(agent_run_id);

CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
  objective TEXT NOT NULL,
  plan TEXT,                     -- JSON: ordered steps
  status TEXT NOT NULL,
  step_count INTEGER NOT NULL DEFAULT 0,
  final_answer TEXT,
  created_at INTEGER NOT NULL,
  finished_at INTEGER
);

CREATE TABLE permissions (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,           -- 'global' | 'project:<id>'
  level TEXT NOT NULL,           -- 'readonly' | 'workspace_write' | 'confirm' | 'denied'
  updated_at INTEGER NOT NULL
);

CREATE TABLE approvals (
  id TEXT PRIMARY KEY,
  tool_execution_id TEXT REFERENCES tool_executions(id) ON DELETE CASCADE,
  granted INTEGER NOT NULL,      -- 1 = approved, 0 = denied
  note TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE memories (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,           -- 'conversation' | 'project' | 'user' | 'auto'
  content TEXT NOT NULL,
  source TEXT,                   -- 'user' | 'agent' | 'tool'
  approved INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_memories_scope ON memories(scope, approved);
```

## Index and storage notes

* `documents.path` and `sources.root_path` are stored as absolute strings. NEXUS never resolves relative paths from the UI.
* Vectors are stored in `chunk_vectors` (sqlite-vec). The vector dimension is fixed at migration time and matches the configured embedding model.
* FTS5 is maintained via `AFTER INSERT/UPDATE/DELETE` triggers on `document_chunks` (added in the same migration).
* All timestamps are epoch milliseconds in UTC; the UI formats them.
* `app_settings` is a key-value bag; the Rust side owns the schema for individual keys.
