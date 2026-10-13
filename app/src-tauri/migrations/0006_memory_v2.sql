-- 0006_memory_v2.sql
-- Native long-term memory v2. Phase 5B.
--
-- The v1 `memories` table (migration 0005) keeps working. v2 adds:
--   * 12 explicit categories
--   * temporal fields: event_at, valid_from, valid_until, verified_at
--   * supersession chain (superseded_by)
--   * content hash for dedupe
--   * verification status (unverified | user_stated | derived | approved)
--   * FTS5 mirror for keyword search
--
-- We keep both schemas because Phase 5 callers already exist; the v1 rows
-- are migrated into v2 with category='lesson' and verification='approved'
-- the first time v2 is read.

CREATE TABLE memories_v2 (
  id TEXT PRIMARY KEY,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,

  -- 12 categories. Free-form: any new value is accepted but the canonical
  -- set is documented in `docs/DATA_MODEL.md`.
  category TEXT NOT NULL,

  content TEXT NOT NULL,
  content_hash TEXT NOT NULL,                -- sha256(normalized content)
  normalized_content TEXT NOT NULL,          -- lowercased, whitespace-folded

  -- Provenance. Each memory points to the source that produced it.
  source_kind TEXT NOT NULL,                 -- 'user' | 'auto' | 'import' | 'extraction' | 'doc' | 'decision'
  source_id TEXT,                            -- id of the row in the source table (doc id, note id, etc.)
  source_label TEXT,                         -- human-friendly label
  source_excerpt TEXT,                       -- optional verbatim quote

  -- Temporal. `event_at` is the time the underlying fact happened; the
  -- record's own `created_at`/`updated_at` are when the system wrote it.
  event_at INTEGER,
  valid_from INTEGER,                        -- the fact is true from this time
  valid_until INTEGER,                       -- the fact stops being true at this time
  verified_at INTEGER,                       -- last verification

  -- Lifecycle. `superseded_by` is a non-null pointer to the row that
  -- replaces this one; we keep the original for history.
  superseded_by TEXT REFERENCES memories_v2(id) ON DELETE SET NULL,
  deleted_at INTEGER,                        -- soft delete; the row remains for audit

  -- Status flags.
  approved INTEGER NOT NULL DEFAULT 0,
  verification TEXT NOT NULL DEFAULT 'unverified',  -- 'unverified' | 'user_stated' | 'derived' | 'approved'
  pinned INTEGER NOT NULL DEFAULT 0,         -- never auto-evicted

  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX idx_memv2_project ON memories_v2(project_id, approved);
CREATE INDEX idx_memv2_conv ON memories_v2(conversation_id);
CREATE INDEX idx_memv2_workspace ON memories_v2(workspace_id);
CREATE INDEX idx_memv2_hash ON memories_v2(content_hash);
CREATE INDEX idx_memv2_super ON memories_v2(superseded_by);
CREATE INDEX idx_memv2_category ON memories_v2(category, approved);

-- FTS5 mirror of approved memory content. We keep this in a separate
-- virtual table so the v2 row stays compact and editable.
CREATE VIRTUAL TABLE memories_v2_fts USING fts5(
  memory_id UNINDEXED,
  content,
  normalized_content,
  category UNINDEXED,
  tokenize = 'unicode61'
);

-- Evidence: the chain back to the original source. One memory can have
-- multiple evidence rows (e.g. a decision backed by a paper AND a note).
CREATE TABLE memory_evidence (
  id TEXT PRIMARY KEY,
  memory_id TEXT NOT NULL REFERENCES memories_v2(id) ON DELETE CASCADE,
  source_kind TEXT NOT NULL,                 -- 'document' | 'note' | 'message' | 'experiment' | 'import' | 'paper'
  source_id TEXT,
  source_location TEXT,                      -- e.g. 'p.4' or 'messages:abc'
  excerpt TEXT,
  verified INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_memory_evidence_memory ON memory_evidence(memory_id);
CREATE INDEX idx_memory_evidence_source ON memory_evidence(source_kind, source_id);

-- Candidates: auto-extracted but not yet approved. The user reviews them
-- in the Memory Center and either promotes them to memories_v2 or rejects.
CREATE TABLE memory_candidates (
  id TEXT PRIMARY KEY,
  category TEXT NOT NULL,
  content TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  source_id TEXT,
  source_excerpt TEXT,
  confidence REAL NOT NULL DEFAULT 0.5,
  status TEXT NOT NULL DEFAULT 'pending',    -- 'pending' | 'approved' | 'rejected'
  rejection_reason TEXT,
  project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  created_at INTEGER NOT NULL,
  decided_at INTEGER
);
CREATE INDEX idx_memcand_status ON memory_candidates(status, created_at);
CREATE INDEX idx_memcand_project ON memory_candidates(project_id);

-- Mirror v1 memories into v2. This is done in code (not SQL) so we can
-- compute the normalized content and content hash consistently.

-- Session records: the end-of-session summary the user can opt into.
CREATE TABLE memory_sessions (
  id TEXT PRIMARY KEY,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  work_completed TEXT,
  decisions_made TEXT,
  files_changed TEXT,
  experiments_performed TEXT,
  errors_encountered TEXT,
  open_questions TEXT,
  next_actions TEXT,
  approved INTEGER NOT NULL DEFAULT 0,
  started_at INTEGER NOT NULL,
  ended_at INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_memory_sessions_project ON memory_sessions(project_id, created_at);

-- Add a `category` field to `documents` so PDFs imported as research
-- papers are first-class. We do this here (instead of in 0004) so
-- existing databases don't trip on the missing column when migrating.
ALTER TABLE documents ADD COLUMN doc_kind TEXT NOT NULL DEFAULT 'document';
ALTER TABLE documents ADD COLUMN venue TEXT;
ALTER TABLE documents ADD COLUMN year INTEGER;
ALTER TABLE documents ADD COLUMN doi TEXT;
ALTER TABLE documents ADD COLUMN authors TEXT;

CREATE INDEX idx_documents_kind ON documents(doc_kind, year);
