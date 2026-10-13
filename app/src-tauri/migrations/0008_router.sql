-- 0008_router.sql
-- Model router: capability registry, lifecycle, decision log. Phase F.

CREATE TABLE router_capabilities (
  profile_id TEXT PRIMARY KEY REFERENCES model_profiles(id) ON DELETE CASCADE,
  supports_chat INTEGER NOT NULL DEFAULT 0,
  supports_code INTEGER NOT NULL DEFAULT 0,
  supports_structured_output INTEGER NOT NULL DEFAULT 0,
  supports_tool_calling INTEGER NOT NULL DEFAULT 0,
  supports_streaming INTEGER NOT NULL DEFAULT 0,
  is_embedding INTEGER NOT NULL DEFAULT 0,
  approx_ctx_tokens INTEGER NOT NULL DEFAULT 0,
  approx_memory_mb INTEGER NOT NULL DEFAULT 0,
  verified INTEGER NOT NULL DEFAULT 0,
  notes TEXT,
  updated_at INTEGER NOT NULL
);

CREATE TABLE router_lifecycle (
  profile_id TEXT PRIMARY KEY REFERENCES model_profiles(id) ON DELETE CASCADE,
  state TEXT NOT NULL DEFAULT 'unloaded',
  last_transition INTEGER NOT NULL,
  last_error TEXT,
  loaded_at INTEGER,
  last_used_at INTEGER,
  inflight INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE router_decisions (
  id TEXT PRIMARY KEY,
  task TEXT NOT NULL,
  selected TEXT,
  fell_back INTEGER NOT NULL DEFAULT 0,
  reason TEXT NOT NULL,
  rejected_json TEXT NOT NULL DEFAULT '[]',
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_router_decisions_created ON router_decisions(created_at DESC);
CREATE INDEX idx_router_decisions_task ON router_decisions(task, created_at DESC);
