-- 0009_research.sql
-- Research Studio. Phase G.
--
-- A research project is a workspace for a literature question. It owns:
--   * papers (already in `documents` with doc_kind='paper')
--   * claims (verifiable statements extracted from a paper, optionally
--     linked to a KG entity for cross-paper synthesis)
--   * comparison matrices (rows = papers, columns = criteria, cells = text)
--   * reports (Markdown snapshots of the current evidence)
--
-- Everything is intentionally lightweight: the goal is "write down
-- what I read and how the papers compare", not a Zotero replacement.

CREATE TABLE research_projects (
  id TEXT PRIMARY KEY,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  question TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'active',  -- 'active' | 'archived'
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_research_projects_ws ON research_projects(workspace_id, status);
CREATE INDEX idx_research_projects_updated ON research_projects(updated_at DESC);

CREATE TABLE research_papers (
  id TEXT PRIMARY KEY,
  research_project_id TEXT NOT NULL REFERENCES research_projects(id) ON DELETE CASCADE,
  document_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  authors TEXT,
  venue TEXT,
  year INTEGER,
  doi TEXT,
  url TEXT,
  notes TEXT,
  position INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_research_papers_proj ON research_papers(research_project_id, position);

CREATE TABLE paper_claims (
  id TEXT PRIMARY KEY,
  research_project_id TEXT NOT NULL REFERENCES research_projects(id) ON DELETE CASCADE,
  paper_id TEXT REFERENCES research_papers(id) ON DELETE CASCADE,
  document_id TEXT REFERENCES documents(id) ON DELETE SET NULL,
  claim TEXT NOT NULL,
  category TEXT NOT NULL DEFAULT 'finding', -- 'finding' | 'method' | 'limitation' | 'question' | 'fact'
  evidence TEXT,
  confidence REAL NOT NULL DEFAULT 0.5,
  kg_entity_id TEXT REFERENCES kg_entities(id) ON DELETE SET NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_paper_claims_project ON paper_claims(research_project_id, category);
CREATE INDEX idx_paper_claims_paper ON paper_claims(paper_id);

CREATE TABLE comparison_matrices (
  id TEXT PRIMARY KEY,
  research_project_id TEXT NOT NULL REFERENCES research_projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_comparison_matrices_project ON comparison_matrices(research_project_id);

CREATE TABLE matrix_columns (
  id TEXT PRIMARY KEY,
  matrix_id TEXT NOT NULL REFERENCES comparison_matrices(id) ON DELETE CASCADE,
  label TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'text',     -- 'text' | 'number' | 'bool' | 'rating'
  position INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_matrix_columns_matrix ON matrix_columns(matrix_id, position);

CREATE TABLE matrix_cells (
  id TEXT PRIMARY KEY,
  matrix_id TEXT NOT NULL REFERENCES comparison_matrices(id) ON DELETE CASCADE,
  paper_id TEXT NOT NULL REFERENCES research_papers(id) ON DELETE CASCADE,
  column_id TEXT NOT NULL REFERENCES matrix_columns(id) ON DELETE CASCADE,
  value_text TEXT,
  value_number REAL,
  value_bool INTEGER,
  updated_at INTEGER NOT NULL,
  UNIQUE (paper_id, column_id)
);
CREATE INDEX idx_matrix_cells_matrix ON matrix_cells(matrix_id);
CREATE INDEX idx_matrix_cells_paper ON matrix_cells(paper_id);

CREATE TABLE research_reports (
  id TEXT PRIMARY KEY,
  research_project_id TEXT NOT NULL REFERENCES research_projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  body_md TEXT NOT NULL,
  paper_ids TEXT NOT NULL DEFAULT '[]',     -- JSON array of paper ids
  claim_ids TEXT NOT NULL DEFAULT '[]',     -- JSON array of claim ids
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX idx_research_reports_project ON research_reports(research_project_id, updated_at DESC);
