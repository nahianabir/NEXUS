-- 0003_conversations.sql
-- Conversation and message storage. Phase 3 uses these.

CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  project_id TEXT,
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
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  status TEXT NOT NULL,
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
