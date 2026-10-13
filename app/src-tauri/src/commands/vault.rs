//! Knowledge Vault Tauri commands. Phase 4 exposes:
//!   * sources: list / register / delete
//!   * documents: list / get / delete
//!   * ingest: kick off the background orchestrator, stream progress
//!   * search: hybrid FTS5 + vector retrieval, returns citations
//!   * embeddings: backfill all chunks whose document was ingested with
//!     the configured embedding model
//!   * stats: a small summary the dashboard can show
//!
//! All commands are stateless on the Rust side; the background work runs
//! inside `tokio::spawn` and the channel is bridged to the UI via the
//! `nexus:vault.progress` Tauri event.

use std::collections::HashSet;
use std::sync::Arc;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::{AppError, AppResult};
use nexus_core::ingestion::citation::{persist_citations, Citation};
use nexus_core::ingestion::orchestrator::{self, IngestJobResult, IngestOptions, IngestProgress};
use nexus_core::ingestion::pipeline as pipe;
use nexus_core::ingestion::search;
use nexus_core::ingestion::vector;
use nexus_core::model_profiles;

// ----------------------------- sources --------------------------------

#[tauri::command]
pub fn vault_list_sources(state: State<'_, Arc<AppState>>) -> AppResult<Vec<pipe::SourceRow>> {
    pipe::list_sources(&state.pool)
}

#[tauri::command]
pub fn vault_register_source(
    state: State<'_, Arc<AppState>>,
    request: pipe::IngestSourceRequest,
) -> AppResult<String> {
    let id = pipe::register_source(&state.pool, &request)?;
    audit::log(&state.pool, "user", "vault.source.register", Some(&id), serde_json::json!({
        "path": request.path, "kind": request.kind,
    }))?;
    Ok(id)
}

#[tauri::command]
pub fn vault_delete_source(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    pipe::delete_source(&state.pool, &id)?;
    audit::log(&state.pool, "user", "vault.source.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

// ----------------------------- documents ------------------------------

#[tauri::command]
pub fn vault_list_documents(
    state: State<'_, Arc<AppState>>,
    limit: Option<i64>,
) -> AppResult<Vec<pipe::DocumentRow>> {
    pipe::list_documents(&state.pool, limit.unwrap_or(200))
}

#[tauri::command]
pub fn vault_get_document(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<pipe::DocumentRow> {
    pipe::get_document(&state.pool, &id)
}

#[tauri::command]
pub fn vault_delete_document(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    pipe::delete_document(&state.pool, &id)?;
    audit::log(&state.pool, "user", "vault.document.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

// ----------------------------- ingest ---------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestRequest {
    pub path: String,
    pub kind: String,                 // "directory" | "file"
    pub display_name: Option<String>,
    pub chunk_tokens: Option<usize>,
    pub chunk_overlap: Option<usize>,
    pub max_file_bytes: Option<u64>,
    pub embedding_profile_id: Option<String>,
}

#[tauri::command]
pub async fn vault_ingest_source(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    request: IngestRequest,
) -> AppResult<String> {
    let s = state.inner().clone();
    let cfg = s.config();
    let chunk_tokens = request.chunk_tokens.unwrap_or(cfg.ingestion.chunk_tokens);
    let chunk_overlap = request.chunk_overlap.unwrap_or(cfg.ingestion.chunk_overlap);
    let max_mb = cfg.ingestion.max_file_mb.max(1);
    let max_file_bytes = request.max_file_bytes.unwrap_or(max_mb * 1024 * 1024);

    let embedding_profile_id = if cfg.ingestion.embedding_enabled {
        request.embedding_profile_id.clone()
    } else {
        None
    };

    // The embedding provider is the active one — we use the same backend
    // for chat. If a different provider is preferred for embeddings the
    // user can configure that in Phase 11.
    let embedding_provider = {
        let guard = s.providers.read();
        if embedding_profile_id.is_some() {
            Some(guard.active())
        } else {
            None
        }
    };

    let request2 = pipe::IngestSourceRequest {
        kind: request.kind.clone(),
        path: request.path.clone(),
        display_name: request.display_name.clone(),
    };
    let options = IngestOptions {
        chunk_tokens,
        chunk_overlap,
        max_file_bytes,
        embedding_profile_id,
    };

    let (job_id, mut rx, handle) = orchestrator::spawn(s.pool.clone(), request2, options, embedding_provider);

    audit::log(&s.pool, "user", "vault.ingest.start", Some(&job_id), serde_json::json!({
        "path": request.path, "kind": request.kind,
    }))?;

    // Bridge the orchestrator's mpsc to Tauri events on a fresh task.
    let app_for_bridge = app.clone();
    let job_id_for_bridge = job_id.clone();
    tokio::spawn(async move {
        let mut last: Option<IngestProgress> = None;
        while let Some(progress) = rx.recv().await {
            let _ = app_for_bridge.emit("nexus:vault.progress", &progress);
            last = Some(progress);
        }
        match handle.await {
            Ok(Ok(result)) => {
                let _ = app_for_bridge.emit("nexus:vault.done", serde_json::json!({
                    "job_id": job_id_for_bridge,
                    "result": result,
                }));
            }
            Ok(Err(e)) => {
                let _ = app_for_bridge.emit("nexus:vault.error", serde_json::json!({
                    "job_id": job_id_for_bridge,
                    "error": e.to_string(),
                }));
            }
            Err(e) => {
                let _ = app_for_bridge.emit("nexus:vault.error", serde_json::json!({
                    "job_id": job_id_for_bridge,
                    "error": format!("join: {e}"),
                }));
            }
        }
        let _ = last; // suppress unused warning if the compiler ever tightens
    });

    Ok(job_id)
}

// ----------------------------- search ---------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<usize>,
    /// Force a vector search even if the embedding model is not configured.
    /// Requires `query_embedding` to be provided.
    pub use_vector: Option<bool>,
    /// Pre-computed query embedding; if absent, the search falls back to
    /// FTS5 only.
    pub query_embedding: Option<Vec<f32>>,
}

#[tauri::command]
pub async fn vault_search(
    state: State<'_, Arc<AppState>>,
    request: SearchRequest,
) -> AppResult<search::SearchResult> {
    let s = state.inner().clone();
    let limit = request.limit.unwrap_or(10).clamp(1, 50);
    let res = search::search_hybrid(s.pool.clone(), &request.query, request.query_embedding, limit).await?;
    Ok(res)
}

#[tauri::command]
pub fn vault_list_citations(
    state: State<'_, Arc<AppState>>,
    assistant_message_id: String,
) -> AppResult<Vec<Citation>> {
    nexus_core::ingestion::citation::list_for_message(&state.pool, &assistant_message_id)
}

// ----------------------------- backfill embeddings --------------------

/// A backfill iterates every chunk that does not have a vector yet and
/// asks the active provider for an embedding. We process in batches of 32.
#[tauri::command]
pub async fn vault_embed_backfill(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    profile_id: String,
    limit: Option<i64>,
) -> AppResult<u64> {
    let s = state.inner().clone();
    let profile = model_profiles::list(&s.pool)?
        .into_iter()
        .find(|p| p.id == profile_id && p.role == "embedding" && p.enabled)
        .ok_or_else(|| AppError::NotFound(format!("embedding profile {profile_id}")))?;

    let provider = { let g = s.providers.read(); g.active() };
    let health = provider.health().await;
    if !health.reachable {
        return Err(AppError::Upstream(format!("provider not reachable: {}", health.error.unwrap_or_default())));
    }

    let cap = limit.unwrap_or(2000);
    // Find chunks without vectors yet. We use a NOT IN
    // subquery against a union of all per-dimension vec0
    // tables so chunks that already have a vector in *any*
    // collection are skipped. **Important**: the `LIKE
    // 'chunk_vectors_%'` query also matches the vec0
    // *shadow* helper tables (`chunk_vectors_384_rowids`,
    // `chunk_vectors_384_vector_chunks00`,
    // `chunk_vectors_384_info`, `chunk_vectors_384_chunks`)
    // which do NOT have a `chunk_id` column. Including them
    // in the `UNION ALL` produces a `no such column: chunk_id`
    // error. We filter to the user-facing tables only,
    // identified by a name that is exactly `chunk_vectors_`
    // followed by an unsigned integer.
    let chunks: Vec<(String, String)> = {
        let conn = s.pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
        let mut tables: Vec<String> = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'chunk_vectors_%'",
            )?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for r in rows { tables.push(r?); }
        }
        // Keep only the per-dimension user-facing tables.
        // vec0 shadow tables are named like
        // `chunk_vectors_<dim>_<suffix>` and are filtered out
        // by requiring the trailing portion to be a pure
        // integer.
        let is_dim_table = |name: &str| -> bool {
            let Some(suffix) = name.strip_prefix("chunk_vectors_") else {
                return false;
            };
            !suffix.is_empty()
                && suffix.chars().all(|c| c.is_ascii_digit())
        };
        let user_tables: Vec<String> = tables
            .into_iter()
            .filter(|t| is_dim_table(t))
            .collect();
        if user_tables.is_empty() {
            // First backfill ever; there are no per-dim
            // user-facing tables yet. Just pick everything
            // up to `cap`.
            let mut stmt = conn.prepare(
                "SELECT id, text FROM document_chunks ORDER BY rowid LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![cap], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut out = Vec::new();
            for row in rows { out.push(row?); }
            out
        } else {
            // Tables are constructed from our own literal
            // pattern (already filtered to integer suffixes)
            // so they are not user input.
            let union_sql: String = user_tables
                .iter()
                .map(|t| format!("SELECT chunk_id AS id FROM {t}"))
                .collect::<Vec<_>>()
                .join(" UNION ALL ");
            let sql = format!(
                "SELECT c.id, c.text FROM document_chunks c
                 WHERE c.id NOT IN ({union_sql})
                 ORDER BY c.rowid LIMIT ?1"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![cap], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut out = Vec::new();
            for row in rows { out.push(row?); }
            out
        }
    };

    // Establish the per-dimension table and register the
    // collection. If the profile already has a dimension
    // we use it; otherwise we discover it from the first
    // successful embedding call.
    let identity = format!("{}::{}", profile.provider, profile.model_id);
    if let Some(d) = profile.dimension {
        if let Err(e) = vector::ensure_dimension_table(&s.pool, d) {
            return Err(e);
        }
        if let Err(e) = vector::register_collection(&s.pool, d, &profile.model_id, &identity) {
            return Err(e);
        }
    }

    let total = chunks.len() as u64;
    let mut done = 0u64;
    for batch in chunks.chunks(32) {
        let inputs: Vec<String> = batch.iter().map(|(_, t)| t.clone()).collect();
        let vectors = provider.embeddings(&profile.model_id, &inputs).await?;
        if vectors.len() != batch.len() {
            return Err(AppError::Upstream(format!(
                "embedding backfill: got {} vectors for {} inputs",
                vectors.len(), batch.len()
            )));
        }
        for ((cid, _), v) in batch.iter().zip(vectors.into_iter()) {
            // First-time dimension detection: if the profile
            // does not have a dimension, infer it from the
            // first vector and register a collection.
            if profile.dimension.is_none() {
                let d = v.len() as u32;
                if let Err(e) = vector::ensure_dimension_table(&s.pool, d) {
                    return Err(e);
                }
                if let Err(e) = vector::register_collection(&s.pool, d, &profile.model_id, &identity) {
                    return Err(e);
                }
            }
            vector::upsert(&s.pool, cid, &v)?;
            done += 1;
        }
        let _ = app.emit("nexus:vault.backfill", serde_json::json!({
            "done": done, "total": total,
        }));
    }
    let _ = audit::log(&s.pool, "user", "vault.embed.backfill", Some(&profile_id), serde_json::json!({
        "embedded": done, "model": profile.model_id,
    }));
    Ok(done)
}

// ----------------------------- stats ----------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStats {
    pub sources: i64,
    pub documents: i64,
    pub documents_failed: i64,
    pub chunks: i64,
    pub vector_rows: i64,
    pub fts_rows: i64,
    pub embedding_profiles: i64,
    pub notes: i64,
}

#[tauri::command]
pub fn vault_stats(state: State<'_, Arc<AppState>>) -> AppResult<VaultStats> {
    let conn = state.pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let count = |sql: &str| -> i64 {
        conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0)
    };
    let profiles = model_profiles::list(&state.pool)?
        .into_iter()
        .filter(|p| p.role == "embedding" && p.enabled)
        .count() as i64;
    let vector_rows = vector::total_vector_rows(&state.pool).unwrap_or_else(|_| {
        // Backwards-compat path: pre-migration 0016 the table
        // is still named `chunk_vectors`. Read from it
        // directly so the UI does not show 0 in that case.
        count("SELECT COUNT(*) FROM chunk_vectors")
    });
    Ok(VaultStats {
        sources: count("SELECT COUNT(*) FROM sources"),
        documents: count("SELECT COUNT(*) FROM documents WHERE status='indexed'"),
        documents_failed: count("SELECT COUNT(*) FROM documents WHERE status='failed'"),
        chunks: count("SELECT COUNT(*) FROM document_chunks"),
        vector_rows,
        fts_rows: count("SELECT COUNT(*) FROM chunks_fts"),
        embedding_profiles: profiles,
        notes: count("SELECT COUNT(*) FROM notes"),
    })
}

// ----------------------------- helpers for chat -----------------------

/// Build a `Citation` for each top-N search hit and persist them. The
/// chat command uses this to remember what the model saw.
pub fn build_citations_from_hits(
    hits: &[nexus_core::ingestion::search::SearchHit],
) -> Vec<Citation> {
    hits.iter().enumerate().map(|(i, h)| Citation {
        index: (i + 1) as i64,
        chunk_id: h.chunk.id.clone(),
        document_id: h.chunk.document_id.clone(),
        document_title: h.chunk.document_title.clone(),
        document_path: h.chunk.document_path.clone(),
        score: h.score,
        source: h.source.clone(),
        text: short_preview(&h.chunk.text, 280),
    }).collect()
}

pub fn short_preview(text: &str, max: usize) -> String {
    let s = text.trim().split_whitespace().collect::<Vec<_>>().join(" ");
    if s.len() <= max { s } else { format!("{}…", &s[..max]) }
}

/// Attach a context block to a user message. The format is plain Markdown;
/// the model is told to use the `[n]` markers when it draws on a chunk.
pub fn build_context_block(citations: &[Citation]) -> String {
    if citations.is_empty() { return String::new(); }
    let mut s = String::from("\n\n## Knowledge Vault context\n");
    s.push_str("The following excerpts may be relevant. Cite them as `[1]`, `[2]`, ... when you draw on them. If the answer is not in the excerpts, say so plainly and do not fabricate.\n\n");
    for c in citations {
        s.push_str(&format!(
            "[{}] {} — {}\n```\n{}\n```\n\n",
            c.index, c.document_title, c.document_path, c.text
        ));
    }
    s
}

/// Persist citations for an assistant message. Returns the number persisted.
pub fn save_citations(
    pool: &nexus_core::db::SqlitePool,
    assistant_message_id: &str,
    citations: &[Citation],
) -> AppResult<usize> {
    // Deduplicate by chunk_id (keep the first occurrence).
    let mut seen: HashSet<String> = HashSet::new();
    let mut unique: Vec<Citation> = Vec::with_capacity(citations.len());
    for c in citations {
        if seen.insert(c.chunk_id.clone()) {
            unique.push(c.clone());
        }
    }
    persist_citations(pool, assistant_message_id, &unique)?;
    Ok(unique.len())
}

// Quiet the unused-import lints.
#[allow(dead_code)]
fn _typecheck(_: CancellationToken, _: mpsc::Sender<IngestProgress>, _: IngestJobResult) {}
