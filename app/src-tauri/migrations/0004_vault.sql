-- 0004_vault.sql
-- Knowledge Vault tables. Migration 0003 already created `message_sources`,
-- which we reuse for citations.

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
  goals TEXT,
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
CREATE INDEX idx_sources_kind ON sources(kind);

CREATE TABLE documents (
  id TEXT PRIMARY KEY,
  source_id TEXT REFERENCES sources(id) ON DELETE CASCADE,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  path TEXT NOT NULL,
  mime_type TEXT,
  size_bytes INTEGER NOT NULL,
  content_hash TEXT NOT NULL,
  status TEXT NOT NULL,
  error TEXT,
  metadata TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_documents_hash ON documents(content_hash);
CREATE INDEX idx_documents_status ON documents(status);
CREATE INDEX idx_documents_source ON documents(source_id);

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

-- FTS5 virtual table mirrors document_chunks.text. We populate via triggers
-- so application inserts/updates/deletes stay in sync.
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
  text,
  content='document_chunks',
  content_rowid='rowid',
  tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON document_chunks BEGIN
  INSERT INTO chunks_fts(rowid, text) VALUES (new.rowid, new.text);
END;
CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON document_chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES('delete', old.rowid, old.text);
END;
CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON document_chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES('delete', old.rowid, old.text);
  INSERT INTO chunks_fts(rowid, text) VALUES (new.rowid, new.text);
END;

-- Vector index. We use a fixed dimension of 384 (compatible with
-- text-embedding-nomic-embed-text-v1.5 and MiniLM-L6). The dimension is
-- set at migration time; future changes require a new migration.
CREATE VIRTUAL TABLE IF NOT EXISTS chunk_vectors USING vec0(
  chunk_id TEXT PRIMARY KEY,
  embedding float[384]
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

CREATE TABLE tasks (
  id TEXT PRIMARY KEY,
  project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL,
  priority TEXT NOT NULL,
  due_at INTEGER,
  checklist TEXT,
  notes TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  completed_at INTEGER
);
CREATE INDEX idx_tasks_status ON tasks(status, due_at);
