-- 0014_hypotheses.sql
-- Hypothesis Ledger. Phase S.3.2 of the Continuity Engine.
--
-- A "hypothesis" is a structured claim the user (or an agent)
-- makes that can be confirmed, refuted, or abandoned. The
-- schema is fully additive. It never modifies any existing
-- table. References to source records are stored as
-- `evidence_for_json` / `evidence_against_json` arrays of
-- `{kind, id, summary, weight}` objects, so the ledger is
-- independent of any specific source table.
--
-- Lifecycle:
--   'open'      -> the user is still gathering evidence
--   'confirmed' -> enough evidence to support
--   'refuted'   -> enough evidence to reject
--   'abandoned' -> the user no longer cares
--
-- The `confidence` field is the user's self-reported prior, in
-- 0.0..=1.0. The radar (S.3.1) ranks open hypotheses with low
-- confidence highest so the user is nudged to test the
-- shakiest claims first.
--
-- `hypothesis_evidence` is the journal of every piece of
-- evidence recorded against a hypothesis. `kind` is
-- 'observation' | 'experiment' | 'quote' | 'reasoning'. The
-- `source_kind`/`source_id` pair points at a row in another
-- table when the evidence is grounded (a memory, a note, a
-- timeline event, a task, an experiment).

CREATE TABLE hypotheses (
  id                      TEXT PRIMARY KEY,
  project_id              TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id                 TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  title                   TEXT NOT NULL,
  statement               TEXT NOT NULL,
  -- User-typed prediction: what the user expects to be true
  -- if the hypothesis is confirmed. Free text.
  prediction              TEXT,
  -- 'open' | 'confirmed' | 'refuted' | 'abandoned'.
  status                  TEXT NOT NULL DEFAULT 'open',
  -- 0.0..=1.0; out-of-range values are clamped at write time.
  confidence              REAL NOT NULL DEFAULT 0.5,
  -- JSON arrays of {kind, id, summary, weight} entries.
  evidence_for_json       TEXT NOT NULL DEFAULT '[]',
  evidence_against_json   TEXT NOT NULL DEFAULT '[]',
  decided_at              INTEGER,
  decision_note           TEXT,
  -- Optional pointer to the memory that backs this hypothesis.
  -- Stored as a free-form id reference; we intentionally do
  -- NOT use a FOREIGN KEY because (a) the memory table is
  -- allowed to be deleted under cascade without a hard
  -- pointer, and (b) the radar / list views survive the
  -- referenced memory being deleted. We do not want a missing
  -- memory to invalidate a hypothesis row.
  memory_id               TEXT,
  created_at              INTEGER NOT NULL,
  updated_at              INTEGER NOT NULL
);
CREATE INDEX idx_hypotheses_project_status
  ON hypotheses(project_id, status, updated_at DESC);
CREATE INDEX idx_hypotheses_task
  ON hypotheses(task_id);
CREATE INDEX idx_hypotheses_confidence
  ON hypotheses(project_id, status, confidence);

CREATE TABLE hypothesis_evidence (
  id                      TEXT PRIMARY KEY,
  hypothesis_id           TEXT NOT NULL REFERENCES hypotheses(id) ON DELETE CASCADE,
  -- 'observation' | 'experiment' | 'quote' | 'reasoning'.
  kind                    TEXT NOT NULL,
  -- Free-text summary the user typed.
  summary                 TEXT NOT NULL,
  -- 0.0..=1.0; clamped at write time.
  weight                  REAL NOT NULL DEFAULT 0.5,
  -- Optional pointer to a row in another table.
  source_kind             TEXT,
  source_id               TEXT,
  -- 'for' or 'against'.
  stance                  TEXT NOT NULL DEFAULT 'for',
  created_at              INTEGER NOT NULL
);
CREATE INDEX idx_hypothesis_evidence_hypothesis
  ON hypothesis_evidence(hypothesis_id, created_at DESC);
