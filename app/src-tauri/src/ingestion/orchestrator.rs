//! Background ingestion orchestrator.
//!
//! The vault `ingest_source` Tauri command spawns this; it walks the source,
//! hashes, extracts, chunks, persists, and finally (best-effort) embeds the
//! chunks if an embedding profile is enabled. Progress is emitted as
//! `nexus:vault.progress` events so the UI can render a live progress bar.

use std::path::Path;
use std::sync::Arc;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use crate::ingestion::chunker::{chunk_text, Chunk};
use crate::ingestion::extractors;
use crate::ingestion::hash;
use crate::ingestion::pipeline as pipe;
use crate::ingestion::vector;
use crate::model_profiles::{self, ModelProfile};
use crate::providers::ModelProvider;

/// Live progress event published on the `nexus:vault.progress` channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestProgress {
    pub job_id: String,
    pub source_id: Option<String>,
    pub files_seen: usize,
    pub files_done: usize,
    pub files_failed: usize,
    pub chunks: usize,
    pub stage: String, // "scanning" | "extracting" | "chunking" | "embedding" | "done" | "failed" | "cancelled"
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestJobResult {
    pub stats: pipe::IngestStats,
    pub source_id: String,
    pub embedded_chunks: usize,
}

/// Settings that influence ingest: chunk size, overlap, file-size cap, and
/// whether to run an embedding backfill pass after indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOptions {
    pub chunk_tokens: usize,
    pub chunk_overlap: usize,
    pub max_file_bytes: u64,
    /// If set, embeddings are generated using this profile's `model_id`.
    pub embedding_profile_id: Option<String>,
}

pub fn spawn(
    pool: SqlitePool,
    request: pipe::IngestSourceRequest,
    options: IngestOptions,
    embedding_provider: Option<Arc<dyn ModelProvider>>,
) -> (String, mpsc::Receiver<IngestProgress>, tokio::task::JoinHandle<AppResult<IngestJobResult>>) {
    let job_id = Ulid::new().to_string();
    let (tx, rx) = mpsc::channel::<IngestProgress>(64);
    let cancel = CancellationToken::new();
    let job_id_clone = job_id.clone();
    let tx_clone = tx.clone();
    let handle = tokio::spawn(async move {
        run(pool, options, request, job_id_clone, cancel, tx_clone, embedding_provider).await
    });
    (job_id, rx, handle)
}

#[allow(clippy::too_many_arguments)]
async fn run(
    pool: SqlitePool,
    options: IngestOptions,
    request: pipe::IngestSourceRequest,
    job_id: String,
    _cancel: CancellationToken,
    tx: mpsc::Sender<IngestProgress>,
    embedding_provider: Option<Arc<dyn ModelProvider>>,
) -> AppResult<IngestJobResult> {
    let mut stats = pipe::IngestStats {
        files_seen: 0, files_indexed: 0, files_skipped: 0, files_failed: 0, chunks: 0, errors: Vec::new(),
    };

    let source_id = pipe::register_source(&pool, &request)?;
    let _ = tx.send(IngestProgress {
        job_id: job_id.clone(),
        source_id: Some(source_id.clone()),
        files_seen: 0, files_done: 0, files_failed: 0, chunks: 0,
        stage: "scanning".into(), message: format!("scanning {}", request.path),
    }).await;

    let path = std::path::PathBuf::from(&request.path);
    if !path.exists() {
        return Err(AppError::NotFound(format!("path {} not found", path.display())));
    }
    let files: Vec<std::path::PathBuf> = if path.is_dir() {
        walkdir::WalkDir::new(&path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_skipped_dir(e.path()))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .collect()
    } else {
        vec![path.clone()]
    };
    stats.files_seen = files.len();
    let _ = tx.send(IngestProgress {
        job_id: job_id.clone(),
        source_id: Some(source_id.clone()),
        files_seen: stats.files_seen, files_done: 0, files_failed: 0, chunks: 0,
        stage: "scanning".into(), message: format!("found {} file(s)", stats.files_seen),
    }).await;

    // Resolve the embedding profile up-front so a typo in the id fails
    // clearly rather than mid-stream.
    let embedding_profile: Option<ModelProfile> = if let Some(pid) = options.embedding_profile_id.as_ref() {
        let list = model_profiles::list(&pool)?;
        list.into_iter().find(|p| &p.id == pid && p.role == "embedding" && p.enabled)
    } else { None };

    let mut all_chunks: Vec<(String, String)> = Vec::new(); // (chunk_id, text)
    for (i, file) in files.iter().enumerate() {
        let stage_msg = format!("{}/{} {}", i + 1, stats.files_seen, file.display());
        let _ = tx.send(IngestProgress {
            job_id: job_id.clone(),
            source_id: Some(source_id.clone()),
            files_seen: stats.files_seen,
            files_done: i,
            files_failed: stats.files_failed,
            chunks: stats.chunks,
            stage: "extracting".into(),
            message: stage_msg,
        }).await;

        let meta = match std::fs::metadata(file) {
            Ok(m) => m,
            Err(e) => { stats.errors.push(format!("{}: {e}", file.display())); stats.files_failed += 1; continue; }
        };
        if meta.len() > options.max_file_bytes {
            stats.files_skipped += 1;
            continue;
        }
        let (digest, size) = match hash::hash_file(file) {
            Ok(t) => t,
            Err(e) => { stats.errors.push(format!("{}: {e}", file.display())); stats.files_failed += 1; continue; }
        };
        if find_document_by_hash(&pool, &digest).is_some() {
            stats.files_skipped += 1;
            continue;
        }

        let extraction = match extractors::extract(file) {
            Ok(e) => e,
            Err(e) => {
                let id = pipe::insert_document(&pool, Some(&source_id), file, &digest, size, "failed", Some(&e.to_string()))?;
                let _ = id;
                stats.errors.push(format!("{}: {e}", file.display()));
                stats.files_failed += 1;
                continue;
            }
        };

        let doc_id = pipe::insert_document(&pool, Some(&source_id), file, &digest, size, "indexed", None)?;
        let chunks: Vec<Chunk> = chunk_text(&extraction.text, options.chunk_tokens, options.chunk_overlap);
        let metadata_str = serde_json::to_string(&extraction.metadata).unwrap_or_else(|_| "{}".into());
        for c in &chunks {
            let id = pipe::insert_chunk(&pool, &doc_id, c, &metadata_str)?;
            all_chunks.push((id, c.text.clone()));
        }
        stats.chunks += chunks.len();
        stats.files_indexed += 1;
    }

    let mut embedded_chunks = 0usize;
    if let (Some(profile), Some(provider)) = (embedding_profile.as_ref(), embedding_provider.as_ref()) {
        let _ = tx.send(IngestProgress {
            job_id: job_id.clone(),
            source_id: Some(source_id.clone()),
            files_seen: stats.files_seen,
            files_done: stats.files_indexed,
            files_failed: stats.files_failed,
            chunks: stats.chunks,
            stage: "embedding".into(),
            message: format!("embedding {} chunks with {}", all_chunks.len(), profile.model_id),
        }).await;

        // Establish the embedding collection up front so the
        // per-dimension vec0 table exists and the registry
        // refuses incompatible inserts. We use the profile's
        // declared dimension if known, otherwise the
        // embedding from the first successful batch sets the
        // dimension. The identity is derived from the
        // profile's provider + model id so two profiles that
        // share a dimension but use different models do not
        // silently mix.
        let identity = format!("{}::{}", profile.provider, profile.model_id);
        if let Some(d) = profile.dimension {
            if let Err(e) = vector::ensure_dimension_table(&pool, d) {
                stats.errors.push(format!("embedding table init: {e}"));
            }
            if let Err(e) = vector::register_collection(&pool, d, &profile.model_id, &identity) {
                stats.errors.push(format!("embedding collection init: {e}"));
            }
        }

        for batch in all_chunks.chunks(32) {
            let inputs: Vec<String> = batch.iter().map(|(_, t)| t.clone()).collect();
            match provider.embeddings(&profile.model_id, &inputs).await {
                Ok(vectors) => {
                    if vectors.len() == batch.len() {
                        for ((cid, _), vec) in batch.iter().zip(vectors.into_iter()) {
                            // First-time dimension detection.
                            if profile.dimension.is_none() {
                                let d = vec.len() as u32;
                                if let Err(e) = vector::ensure_dimension_table(&pool, d) {
                                    stats.errors.push(format!("embedding table init dim={d}: {e}"));
                                }
                                if let Err(e) = vector::register_collection(&pool, d, &profile.model_id, &identity) {
                                    stats.errors.push(format!("embedding collection init dim={d}: {e}"));
                                }
                            }
                            if let Err(e) = vector::upsert(&pool, cid, &vec) {
                                stats.errors.push(format!("vector upsert {cid}: {e}"));
                            } else {
                                embedded_chunks += 1;
                            }
                        }
                    } else {
                        stats.errors.push(format!(
                            "embedding batch returned {} vectors for {} inputs",
                            vectors.len(), batch.len()
                        ));
                    }
                }
                Err(e) => {
                    stats.errors.push(format!("embedding batch: {e}"));
                    break;
                }
            }
        }
    } else if embedding_profile.is_some() && embedding_provider.is_none() {
        stats.errors.push("embedding profile set; active provider not reachable".into());
    }

    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute("UPDATE sources SET last_indexed_at = ?1 WHERE id = ?2", params![now, source_id])?;

    let stage = if stats.files_failed > 0 { "done-with-errors" } else { "done" };
    let _ = tx.send(IngestProgress {
        job_id: job_id.clone(),
        source_id: Some(source_id.clone()),
        files_seen: stats.files_seen,
        files_done: stats.files_indexed,
        files_failed: stats.files_failed,
        chunks: stats.chunks,
        stage: stage.into(),
        message: format!("indexed {} files, {} chunks, embedded {}", stats.files_indexed, stats.chunks, embedded_chunks),
    }).await;

    let _ = crate::audit::log(&pool, "system", "vault.ingest", Some(&source_id), serde_json::json!({
        "files_seen": stats.files_seen,
        "files_indexed": stats.files_indexed,
        "files_failed": stats.files_failed,
        "chunks": stats.chunks,
        "embedded_chunks": embedded_chunks,
    }))?;

    Ok(IngestJobResult { stats, source_id, embedded_chunks })
}

fn is_skipped_dir(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(name, ".git" | "node_modules" | "target" | "dist" | "build" | ".venv" | "venv" | "__pycache__" | ".idea" | ".vscode")
}

fn find_document_by_hash(pool: &SqlitePool, content_hash: &str) -> Option<String> {
    let conn = pool.get().ok()?;
    let mut stmt = conn.prepare("SELECT id FROM documents WHERE content_hash = ?1").ok()?;
    let mut rows = stmt.query(params![content_hash]).ok()?;
    if let Some(row) = rows.next().ok()? {
        row.get::<_, String>(0).ok()
    } else {
        None
    }
}