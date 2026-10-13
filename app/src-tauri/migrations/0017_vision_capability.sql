-- 0017_vision_capability.sql
-- Phase P3 — image attachment crash (Bug D).
--
-- The previous implementation assumed every model that
-- accepts a text prompt would also accept an OpenAI-style
-- `image_url` content part. When the user attached a
-- photo and the active model was a text-only LLM, LM
-- Studio (and most OpenAI-compatible servers) returned a
-- 4xx error, which surfaced as a crash because the chat
-- pipeline did not have a recoverable path for
-- "this model does not support vision".
--
-- This migration:
--   1. Adds `supports_vision` (INTEGER 0|1) to
--      `model_profiles`. Default 0 (off) so the field
--      is strictly opt-in; an existing row never
--      claims vision capability.
--   2. Adds a `kind` index on `model_profiles.role` if
--      missing (so the per-role lookup in the chat
--      pipeline stays cheap as the table grows).
--
-- Backwards compatibility: existing rows that predate
-- this migration will get `supports_vision = 0` after
-- the migration runs. Users opt-in to vision by editing
-- the model profile in Model Manager.

ALTER TABLE model_profiles ADD COLUMN supports_vision INTEGER NOT NULL DEFAULT 0;

-- Helpful index that was implicitly missing.
CREATE INDEX IF NOT EXISTS idx_model_profiles_role_enabled
    ON model_profiles(role, enabled);
