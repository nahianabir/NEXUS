# NEXUS — Data Model (Phase C–M addendum)

The migrations below were added in the second half of the build.
They follow the same conventions: ULID primary keys, epoch-millis
timestamps, explicit indexes, and foreign keys to the core tables
where appropriate.

## 0007 — Knowledge graph

* `kg_entities(id, workspace_id, entity_type, canonical_name,
  description, metadata_json, merged_into, created_at, updated_at)`
* `kg_aliases(id, entity_id, alias, normalized, created_at)`
* `kg_relationships(id, source_entity_id, relationship_type,
  target_entity_id, valid_from, valid_until, weight,
  verification_status, metadata_json, created_at)`
* `kg_evidence(id, relationship_id, source_kind, source_id,
  source_location, excerpt, created_at)`

## 0008 — Model router

* `router_capabilities(profile_id, supports_chat, supports_code,
  supports_structured_output, supports_tool_calling,
  supports_streaming, is_embedding, approx_ctx_tokens,
  approx_memory_mb, verified, notes, updated_at)`
* `router_lifecycle(profile_id, state, last_transition,
  last_error, loaded_at, last_used_at, inflight)`
* `router_decisions(id, task, selected, fell_back, reason,
  rejected_json, created_at)`

## 0009 — Research studio

* `research_projects(id, workspace_id, title, question,
  description, status, created_at, updated_at)`
* `research_papers(id, research_project_id, document_id, title,
  authors, venue, year, doi, url, notes, position, created_at)`
* `paper_claims(id, research_project_id, paper_id, document_id,
  claim, category, evidence, confidence, kg_entity_id, created_at)`
* `comparison_matrices(id, research_project_id, title,
  description, created_at)`
* `matrix_columns(id, matrix_id, label, kind, position)`
* `matrix_cells(id, matrix_id, paper_id, column_id, value_text,
  value_number, value_bool, updated_at)` — `UNIQUE(paper_id, column_id)`.
* `research_reports(id, research_project_id, title, body_md,
  paper_ids, claim_ids, created_at, updated_at)`

## 0010 — Experiments

* `experiments(id, project_id, name, description, status,
  tags_json, created_at, updated_at)`
* `experiment_runs(id, experiment_id, name, status,
  hyperparams_json, metrics_json, code_revision, code_path,
  hardware, notes, started_at, ended_at, created_at, updated_at)`

## 0011 — Controlled agent

* `agent_tools(id, name, description, category, permission,
  enabled, schema_json, created_at)` — `name` is `UNIQUE`.
* `agent_runs(id, conversation_id, project_id, title, status,
  created_at, updated_at, finished_at)`
* `agent_invocations(id, run_id, tool_name, permission,
  input_json, output_json, status, error, started_at, finished_at)`

All migration files are in `app/src-tauri/migrations/`. The runner
in `app/src-tauri/src/db/migrations.rs` is the single source of
truth for which version is current.