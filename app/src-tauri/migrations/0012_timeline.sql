-- 0012_timeline.sql
-- Project Time Machine. Phase S.1.
--
-- `timeline_events` is a provenance-first, append-only event index
-- that the Continuity Engine uses to render a chronological view of
-- a project without duplicating the source rows. Each row points at
-- the original record by `(source_kind, source_id)`; the only thing
-- we copy is a short, factual summary and a structured `detail_json`
-- blob (kept small: an excerpt, a list of relevant entity ids, etc.).
--
-- Design rules:
--   1. The table is *additive*. No existing table is altered. The
--      table starts empty and is populated on explicit user demand.
--   2. Ingestion is *idempotent*. The unique key
--      `(source_kind, source_id, event_kind, event_at)` prevents
--      duplicate rows when a project is re-indexed.
--   3. The table is *read-only against the source*. We never modify
--      audit_events, tasks, notes, memories_v2, kg_relationships,
--      experiments, agent_invocations, research_*, etc. when we
--      build or query the timeline.
--   4. `event_at` is the time the underlying fact happened
--      (e.g. the task's `created_at`); `imported_at` is when NEXUS
--      first saw the event. They are kept separate so an old Git
--      commit imported today does not appear to have happened today.
--   5. `summary` is short and human-readable. `detail_json` is a
--      structured bag for the UI (excerpt, file paths, entity ids).
--      We do not copy large documents here.
--
-- Source kind vocabulary (matches `memory_evidence.source_kind`):
--   'audit'             — audit_events row
--   'task'              — tasks row
--   'note'              — notes row
--   'memory'            — memories_v2 row
--   'experiment'        — experiments row
--   'experiment_run'    — experiment_runs row
--   'kg_relationship'   — kg_relationships row
--   'agent_invocation'  — agent_invocations row
--   'research_paper'    — research_papers row
--   'research_report'   — research_reports row
--   'manual'            — a hand-written timeline event
--
-- Event kind vocabulary:
--   'created' | 'updated' | 'status_changed' | 'completed'
--   'approved' | 'superseded' | 'archived' | 'opened'
--   'invoked' | 'finished' | 'note'
--   'manual'                  — for hand-written entries

CREATE TABLE timeline_events (
  id            TEXT PRIMARY KEY,
  project_id    TEXT REFERENCES projects(id) ON DELETE CASCADE,

  -- High-level grouping. Free-form: the UI groups by this.
  event_type    TEXT NOT NULL,

  -- Source provenance.
  source_kind   TEXT NOT NULL,
  source_id     TEXT,
  source_label  TEXT,

  -- Time. `event_at` is the time the source fact happened; `imported_at`
  -- is the time NEXUS first wrote the timeline row. The two are kept
  -- distinct so the UI can label old events honestly.
  event_at      INTEGER NOT NULL,
  imported_at   INTEGER NOT NULL,

  -- Short, factual summary (max ~240 chars). The detail JSON holds the
  -- longer structured info.
  summary       TEXT NOT NULL,
  detail_json   TEXT NOT NULL DEFAULT '{}',

  -- Provenance of the timeline row itself: the ingestor that wrote it
  -- and the run id (e.g. an ingest batch). The original source's own
  -- provenance is referenced by (source_kind, source_id).
  ingested_by   TEXT NOT NULL DEFAULT 'manual',
  ingest_run_id TEXT,

  created_at    INTEGER NOT NULL
);

-- Idempotency: the same source event cannot be written twice.
CREATE UNIQUE INDEX uq_timeline_source_event
  ON timeline_events(source_kind, source_id, event_type, event_at);

-- The two hot queries:
--   * "all events for project X, newest first"
--   * "filter by source kind / source id"
CREATE INDEX idx_timeline_project_time
  ON timeline_events(project_id, event_at DESC, id ASC);
CREATE INDEX idx_timeline_source
  ON timeline_events(source_kind, source_id);
CREATE INDEX idx_timeline_event_type
  ON timeline_events(event_type);
CREATE INDEX idx_timeline_run
  ON timeline_events(ingest_run_id);
