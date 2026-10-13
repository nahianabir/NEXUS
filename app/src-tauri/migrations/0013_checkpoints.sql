-- 0013_checkpoints.sql
-- One-Click Work Resume. Phase S.2.
--
-- A "checkpoint" is a structured, model-independent record of
-- where work stopped. It contains *evidence references* (ids into
-- the canonical tables) and small, structured observations (file
-- paths, test results, decisions). It never copies whole
-- documents, chat transcripts, or git repositories. It is
-- read-only against the source tables: editing or deleting a
-- checkpoint never touches tasks, notes, memories, or anything
-- else.
--
-- `observed_files` stores **paths**, not contents. The paths are
-- used at resume time to detect drift (missing, modified,
-- size-changed). A missing or corrupted referenced file does not
-- invalidate the checkpoint.
--
-- `drift_check_json` is a small snapshot of the world at
-- checkpoint-creation time: e.g. task counts, file mtimes (when
-- accessible), the active model id (as a hint, not as a hard
-- dependency). The resume function re-reads the world and
-- produces a `DriftReport` so the user can see what changed.
--
-- `evidence_refs_json` is an array of
--   { kind: "timeline" | "memory" | "note" | "task", id: "..." }
-- entries. The UI uses these to render links back to the source
-- records. References that no longer resolve are reported as
-- "missing" at resume time — the checkpoint itself is preserved.
--
-- `status` lifecycle: 'active' | 'resumed' | 'archived'. We do
-- not delete checkpoints automatically; the user is the source
-- of truth.

CREATE TABLE checkpoints (
  id                    TEXT PRIMARY KEY,
  project_id            TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id               TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  title                 TEXT NOT NULL,
  status                TEXT NOT NULL DEFAULT 'active',
  -- Free-text user input.
  goal                  TEXT,
  completed_work        TEXT,
  current_state         TEXT,
  next_action_text      TEXT,
  -- Structured, machine-friendly observations.
  observed_files_json   TEXT NOT NULL DEFAULT '[]',
  observed_tests_json   TEXT NOT NULL DEFAULT '[]',
  errors_json           TEXT NOT NULL DEFAULT '[]',
  decisions_json        TEXT NOT NULL DEFAULT '[]',
  constraints_json      TEXT NOT NULL DEFAULT '[]',
  pending_actions_json  TEXT NOT NULL DEFAULT '[]',
  -- Evidence + drift snapshot.
  evidence_refs_json    TEXT NOT NULL DEFAULT '[]',
  drift_check_json      TEXT NOT NULL DEFAULT '{}',
  -- Provenance. `created_by` is 'user' | 'agent:<run_id>'. The
  -- active model id is captured as a *hint* so the user knows
  -- which model produced the optional AI summary, but the
  -- checkpoint itself is model-independent.
  created_by            TEXT NOT NULL DEFAULT 'user',
  model_hint            TEXT,
  -- Bookkeeping.
  created_at            INTEGER NOT NULL,
  updated_at            INTEGER NOT NULL,
  resumed_at            INTEGER
);
CREATE INDEX idx_checkpoints_project_time
  ON checkpoints(project_id, created_at DESC);
CREATE INDEX idx_checkpoints_task
  ON checkpoints(task_id);
CREATE INDEX idx_checkpoints_status
  ON checkpoints(status, created_at DESC);

-- Resume history: every time the user opens or "applies" a
-- checkpoint, a row is written here. Useful for "you resumed
-- this checkpoint 3 times already" hints. Not the source of
-- truth — the source is the audit_events row written by the
-- Tauri command — but a denormalised cache for the UI.
CREATE TABLE checkpoint_resumes (
  id                    TEXT PRIMARY KEY,
  checkpoint_id         TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE CASCADE,
  resumed_at            INTEGER NOT NULL,
  drift_report_json     TEXT NOT NULL DEFAULT '{}',
  note                  TEXT
);
CREATE INDEX idx_checkpoint_resumes_checkpoint
  ON checkpoint_resumes(checkpoint_id, resumed_at DESC);
