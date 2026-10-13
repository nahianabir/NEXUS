-- 0015_radar_feedback.sql
-- Opportunity Radar feedback. Phase F.
--
-- A "radar_feedback" row records the user's response to a
-- surfaced opportunity. The kind is one of:
--   'dismiss'  — the user does not want to see this again
--   'snooze'   — the user wants to hide it until 'until_ms'
--   'pin'      — the user wants this pinned to the top
--   'done'     — the user has acted on it (no longer surfaces)
--   'false'    — the user marked the opportunity as irrelevant
--
-- The "key" is a stable string derived from the opportunity
-- kind + target id (e.g. "overdue_urgent_task:abc123"). It
-- is independent of any radar report id, so a feedback row
-- outlives the report that produced the opportunity.
--
-- The radar query (S.3.1) reads from this table on every
-- scan and excludes the corresponding keys. Snoozed rows
-- are excluded until `until_ms`. Pinned rows are not
-- excluded but are sorted first.
--
-- The table is fully additive. No existing table is
-- modified. Deleting a feedback row re-surfaces the
-- opportunity.

CREATE TABLE radar_feedback (
  id          TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  key         TEXT NOT NULL,
  kind        TEXT NOT NULL,
  -- Optional denormalised snapshot so the UI can show
  -- "you dismissed X on 2025-..." without a separate query.
  title       TEXT,
  note        TEXT,
  until_ms    INTEGER,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX idx_radar_feedback_project_key
  ON radar_feedback(project_id, key);
CREATE INDEX idx_radar_feedback_kind
  ON radar_feedback(project_id, kind, created_at DESC);
