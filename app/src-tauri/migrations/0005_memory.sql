-- 0005_memory.sql
-- Persistent memory scopes for the Assistant.
--
-- `scope` is one of:
--   * 'user'     — long-term user facts (preferences, environment)
--   * 'project'  — facts tied to a specific project_id
--   * 'auto'     — auto-extracted from a chat turn; approved by the user
--   * 'conv'     — ephemeral; tied to a specific conversation
--
-- `approved` gates whether a memory is visible to the model. Auto-extracted
-- memories are stored as `approved = 0` until the user reviews them.

CREATE TABLE memories (
  id TEXT PRIMARY KEY,
  scope TEXT NOT NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
  content TEXT NOT NULL,
  source TEXT,                       -- 'user' | 'agent' | 'auto'
  approved INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_memories_scope ON memories(scope, approved);
CREATE INDEX idx_memories_project ON memories(project_id, approved);
CREATE INDEX idx_memories_conv ON memories(conversation_id);

-- A simple settings table for app-wide UI flags. We use it for "active
-- project" so the chat can pick it up without a separate state file.
CREATE TABLE IF NOT EXISTS ui_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,               -- JSON
  updated_at INTEGER NOT NULL
);
