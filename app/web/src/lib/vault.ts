// Typed IPC bindings for the Knowledge Vault (Phase 4).
//
// Sources are absolute paths (file or directory). Documents are content-hash
// deduplicated and chunked; FTS5 is populated by triggers; vectors live in
// sqlite-vec. The UI listens to `nexus:vault.progress` / `nexus:vault.done`
// for live ingest feedback.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type SourceKind = "directory" | "file" | "note" | "paper" | "experiment";

export type SourceRow = {
  id: string;
  root_path: string;
  display_name: string;
  kind: string;
  enabled: number;
  last_indexed_at: number | null;
  created_at: number;
};

export type DocumentRow = {
  id: string;
  source_id: string | null;
  project_id: string | null;
  title: string;
  path: string;
  mime_type: string | null;
  size_bytes: number;
  /** SHA-256 of the file content. May be null for legacy records
   *  predating the column; the Vault page treats it as a best-effort
   *  display value and falls back to "no hash" instead of throwing. */
  content_hash: string | null;
  status: string;
  error: string | null;
  metadata: string | null;
  created_at: number;
  updated_at: number;
};

export type ChunkRow = {
  id: string;
  document_id: string;
  document_title: string;
  document_path: string;
  ordinal: number;
  text: string;
  token_estimate: number | null;
  start_offset: number | null;
  end_offset: number | null;
};

export type SearchHit = {
  chunk: ChunkRow;
  score: number;
  source: "fts" | "vector" | "hybrid";
};

export type SearchResult = {
  hits: SearchHit[];
  used_vector: boolean;
};

export type Citation = {
  index: number;
  chunk_id: string;
  document_id: string;
  document_title: string;
  document_path: string;
  score: number;
  source: string;
  text: string;
};

export type IngestProgress = {
  job_id: string;
  source_id: string | null;
  files_seen: number;
  files_done: number;
  files_failed: number;
  chunks: number;
  stage: string;
  message: string;
};

export type IngestRequest = {
  path: string;
  kind: SourceKind;
  display_name?: string | null;
  chunk_tokens?: number | null;
  chunk_overlap?: number | null;
  max_file_bytes?: number | null;
  embedding_profile_id?: string | null;
};

export type IngestResult = {
  job_id: string;
  source_id: string;
  files_seen: number;
  files_indexed: number;
  files_failed: number;
  files_skipped: number;
  chunks: number;
  embedded_chunks: number;
  errors: string[];
};

export type VaultStats = {
  sources: number;
  documents: number;
  documents_failed: number;
  chunks: number;
  vector_rows: number;
  fts_rows: number;
  embedding_profiles: number;
  notes: number;
};

// ---------- sources ----------

export async function vaultListSources(): Promise<SourceRow[]> {
  return invoke("vault_list_sources");
}

export async function vaultRegisterSource(request: { path: string; kind: SourceKind; display_name?: string | null }): Promise<string> {
  return invoke("vault_register_source", { request });
}

export async function vaultDeleteSource(id: string): Promise<void> {
  return invoke("vault_delete_source", { id });
}

// ---------- documents ----------

export async function vaultListDocuments(limit?: number): Promise<DocumentRow[]> {
  return invoke("vault_list_documents", { limit: limit ?? null });
}

export async function vaultGetDocument(id: string): Promise<DocumentRow> {
  return invoke("vault_get_document", { id });
}

export async function vaultDeleteDocument(id: string): Promise<void> {
  return invoke("vault_delete_document", { id });
}

// ---------- ingest ----------

export async function vaultIngestSource(request: IngestRequest): Promise<string> {
  return invoke("vault_ingest_source", { request });
}

export async function onVaultProgress(handler: (p: IngestProgress) => void): Promise<UnlistenFn> {
  return listen<IngestProgress>("nexus:vault.progress", (e) => handler(e.payload));
}

export async function onVaultDone(handler: (r: { job_id: string; result: IngestResult }) => void): Promise<UnlistenFn> {
  return listen<{ job_id: string; result: IngestResult }>("nexus:vault.done", (e) => handler(e.payload));
}

export async function onVaultError(handler: (e: { job_id: string; error: string }) => void): Promise<UnlistenFn> {
  return listen<{ job_id: string; error: string }>("nexus:vault.error", (e) => handler(e.payload));
}

export async function onVaultBackfill(handler: (b: { done: number; total: number }) => void): Promise<UnlistenFn> {
  return listen<{ done: number; total: number }>("nexus:vault.backfill", (e) => handler(e.payload));
}

// ---------- search & citations ----------

export async function vaultSearch(request: { query: string; limit?: number | null; use_vector?: boolean | null; query_embedding?: number[] | null }): Promise<SearchResult> {
  return invoke("vault_search", { request });
}

export async function vaultListCitations(assistantMessageId: string): Promise<Citation[]> {
  return invoke("vault_list_citations", { assistantMessageId });
}

// ---------- embeddings backfill ----------

export async function vaultEmbedBackfill(profileId: string, limit?: number): Promise<number> {
  return invoke("vault_embed_backfill", { profileId, limit: limit ?? null });
}

// ---------- stats ----------

export async function vaultStats(): Promise<VaultStats> {
  return invoke("vault_stats");
}