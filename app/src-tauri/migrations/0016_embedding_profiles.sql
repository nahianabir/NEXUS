-- 0016_embedding_profiles.sql
-- Phase R3 — embedding dimension awareness.
--
-- The previous schema hardcoded the vector dimension to 384
-- inside the `chunk_vectors` vec0 virtual table
-- (see 0004_vault.sql). That made it impossible to use an
-- embedding model that returns 768 dimensions (e.g.
-- nomic-embed-text-v1.5) without re-creating the database.
--
-- This migration:
--   1. Adds `dimension` and a stable `model_identity` to
--      `model_profiles` so each profile records the
--      dimension and identity of the model that produced its
--      vectors. `model_identity` is a hash of provider +
--      model_id, and is what the user sees in the UI.
--   2. Renames the legacy 384-dim vec0 table to
--      `chunk_vectors_384`. This preserves any pre-existing
--      user vectors: an SQLite rename is fast and atomic.
--   3. Adds an `embedding_collections` registry so the
--      application can discover which dimensions are
--      present and refuse to mix incompatible embeddings.

ALTER TABLE model_profiles ADD COLUMN dimension INTEGER;
ALTER TABLE model_profiles ADD COLUMN model_identity TEXT;

-- Move the existing vec0 table to a dimension-suffixed name.
-- 0004_vault.sql declared it as:
--   CREATE VIRTUAL TABLE chunk_vectors USING vec0(
--     chunk_id TEXT PRIMARY KEY,
--     embedding float[384]
--   );
-- We preserve the contents so existing users keep their
-- pre-Phase-R3 vectors. The vec0 extension also creates
-- shadow helper tables; the application code renames those
-- programmatically before this SQL runs (see
-- `db::migrations::run`).
ALTER TABLE chunk_vectors RENAME TO chunk_vectors_384;

-- Registry of dimensions present in the database. The
-- application consults this before creating a new
-- chunk_vectors_<dim> table and when running a search.
CREATE TABLE IF NOT EXISTS embedding_collections (
  dimension      INTEGER PRIMARY KEY,
  -- The model that produced the *first* set of vectors in
  -- this collection. A different model with the same
  -- dimension is *not* a valid substitute.
  model_id       TEXT NOT NULL,
  model_identity TEXT NOT NULL,
  created_at     INTEGER NOT NULL,
  -- Number of vectors in the table. Updated by the
  -- application; we do not maintain a strict trigger
  -- because the per-dimension tables are append-only.
  vector_rows    INTEGER NOT NULL DEFAULT 0
);

-- Seed the 384 row if we have any pre-existing vectors.
INSERT OR IGNORE INTO embedding_collections (dimension, model_id, model_identity, created_at, vector_rows)
  SELECT 384, 'legacy-384', 'legacy', strftime('%s','now') * 1000, COUNT(*)
  FROM chunk_vectors_384;
