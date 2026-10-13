-- 0011_agent.sql
-- Controlled Agent. Phase I.
--
-- A tool is a registered capability (e.g. "vault.search", "shell.run").
-- Each tool has a permission level the user must grant. The agent
-- session log records every tool invocation and its outcome. The
-- goal is to be inspectable, not autonomous: every action is logged
-- and can be reviewed.

CREATE TABLE agent_tools (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,        -- e.g. 'vault.search', 'shell.run', 'kg.search'
  description TEXT NOT NULL,
  category TEXT NOT NULL,           -- 'knowledge' | 'research' | 'projects' | 'files' | 'dev' | 'system'
  permission TEXT NOT NULL,         -- 'always' | 'ask' | 'restricted' | 'deny'
  enabled INTEGER NOT NULL DEFAULT 1,
  schema_json TEXT NOT NULL DEFAULT '{}',  -- JSON schema for the tool's parameters
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_agent_tools_category ON agent_tools(category, enabled);

CREATE TABLE agent_runs (
  id TEXT PRIMARY KEY,
  conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'running',  -- 'running' | 'completed' | 'failed' | 'cancelled'
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  finished_at INTEGER
);
CREATE INDEX idx_agent_runs_created ON agent_runs(created_at DESC);

CREATE TABLE agent_invocations (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  tool_name TEXT NOT NULL,
  permission TEXT NOT NULL,
  input_json TEXT NOT NULL,
  output_json TEXT,
  status TEXT NOT NULL DEFAULT 'pending',  -- 'pending' | 'approved' | 'denied' | 'executed' | 'failed'
  error TEXT,
  started_at INTEGER NOT NULL,
  finished_at INTEGER
);
CREATE INDEX idx_agent_invocations_run ON agent_invocations(run_id, started_at);
