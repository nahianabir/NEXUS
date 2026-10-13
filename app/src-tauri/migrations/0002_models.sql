-- 0002_models.sql
-- Model profiles and provider endpoints. Phase 2 uses these.

CREATE TABLE provider_endpoints (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  base_url TEXT NOT NULL,
  api_key_ref TEXT,
  managed_lifecycle INTEGER NOT NULL DEFAULT 0,
  last_seen_status TEXT,
  last_seen_at INTEGER,
  created_at INTEGER NOT NULL
);

CREATE TABLE model_profiles (
  id TEXT PRIMARY KEY,
  role TEXT NOT NULL,
  display_name TEXT NOT NULL,
  provider TEXT NOT NULL,
  model_id TEXT NOT NULL,
  gguf_path TEXT,
  context_len INTEGER NOT NULL DEFAULT 4096,
  gpu_layers INTEGER,
  temperature REAL NOT NULL DEFAULT 0.7,
  top_p REAL NOT NULL DEFAULT 0.95,
  max_tokens INTEGER NOT NULL DEFAULT 1024,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_model_profiles_role ON model_profiles(role);
