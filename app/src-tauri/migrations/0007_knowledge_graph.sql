-- 0007_knowledge_graph.sql
-- Database-backed knowledge graph. Phase D.
--
-- The graph is small, typed, and explicitly provenance-tracked. It
-- is a real store of facts about entities (projects, papers, models,
-- datasets, decisions, ...) and the relationships between them, not
-- a UI-side decoration.
--
-- Each `kg_evidence` row points back to the source record that
-- produced it (a document, a memory, a chat message, an experiment,
-- etc.). The relationships and the evidence survive the deletion of
-- the original record; the `verification_status` flag distinguishes
-- "inferred" from "user-confirmed".

CREATE TABLE kg_entities (
  id TEXT PRIMARY KEY,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
  entity_type TEXT NOT NULL,
  canonical_name TEXT NOT NULL,
  description TEXT,
  metadata_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_kg_entities_type ON kg_entities(entity_type, canonical_name);
CREATE INDEX idx_kg_entities_workspace ON kg_entities(workspace_id);

CREATE TABLE kg_aliases (
  id TEXT PRIMARY KEY,
  entity_id TEXT NOT NULL REFERENCES kg_entities(id) ON DELETE CASCADE,
  alias TEXT NOT NULL,
  normalized_alias TEXT NOT NULL
);
CREATE INDEX idx_kg_aliases_entity ON kg_aliases(entity_id);
CREATE UNIQUE INDEX uq_kg_aliases_normalized ON kg_aliases(normalized_alias);

CREATE TABLE kg_relationships (
  id TEXT PRIMARY KEY,
  source_entity_id TEXT NOT NULL REFERENCES kg_entities(id) ON DELETE CASCADE,
  relationship_type TEXT NOT NULL,
  target_entity_id TEXT NOT NULL REFERENCES kg_entities(id) ON DELETE CASCADE,
  valid_from INTEGER,
  valid_until INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  verification_status TEXT NOT NULL DEFAULT 'inferred',  -- 'inferred' | 'user_confirmed' | 'disputed'
  weight REAL NOT NULL DEFAULT 1.0,
  metadata_json TEXT
);
CREATE INDEX idx_kg_rel_source ON kg_relationships(source_entity_id, relationship_type);
CREATE INDEX idx_kg_rel_target ON kg_relationships(target_entity_id);
CREATE UNIQUE INDEX uq_kg_rel_pair ON kg_relationships(source_entity_id, relationship_type, target_entity_id);

CREATE TABLE kg_evidence (
  id TEXT PRIMARY KEY,
  relationship_id TEXT NOT NULL REFERENCES kg_relationships(id) ON DELETE CASCADE,
  source_kind TEXT NOT NULL,            -- 'document' | 'note' | 'message' | 'experiment' | 'memory' | 'paper' | 'manual'
  source_id TEXT,                       -- row id in the source table
  source_location TEXT,                 -- e.g. 'p.4' or 'chunks:abc'
  excerpt TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_kg_evidence_rel ON kg_evidence(relationship_id);
CREATE INDEX idx_kg_evidence_source ON kg_evidence(source_kind, source_id);

-- A merge log: when two entities are merged, the loser stays with
-- `merged_into` pointing at the winner, so the history is auditable.
ALTER TABLE kg_entities ADD COLUMN merged_into TEXT REFERENCES kg_entities(id) ON DELETE SET NULL;
CREATE INDEX idx_kg_entities_merged ON kg_entities(merged_into);
