//! The ingestion pipeline. Walks a source, hashes files, extracts text,
//! chunks, and stores everything in SQLite. Embeddings are filled in
//! later by an offline job — the documents are still searchable via
//! FTS5 even without embeddings.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;
use walkdir::WalkDir;

use crate::audit;
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use crate::ingestion::chunker::{chunk_text, Chunk};
use crate::ingestion::extractors;
use crate::ingestion::hash;
use crate::ingestion::vector;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestStats {
    pub files_seen: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_failed: usize,
    pub chunks: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestSourceRequest {
    pub kind: String,            // 'directory' | 'file'
    pub path: String,
    pub display_name: Option<String>,
}

pub fn register_source(pool: &SqlitePool, req: &IngestSourceRequest) -> AppResult<String> {
    let id = Ulid::new().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let display = req.display_name.clone().unwrap_or_else(|| {
        Path::new(&req.path).file_name().and_then(|s| s.to_str()).unwrap_or(&req.path).to_string()
    });
    conn.execute(
        "INSERT INTO sources (id, root_path, display_name, kind, enabled, created_at) VALUES (?1, ?2, ?3, ?4, 1, ?5)",
        params![id, req.path, display, req.kind, now],
    )?;
    Ok(id)
}

pub fn list_sources(pool: &SqlitePool) -> AppResult<Vec<SourceRow>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare("SELECT id, root_path, display_name, kind, enabled, last_indexed_at FROM sources ORDER BY created_at DESC")?;
    let rows = stmt.query_map([], |r| {
        Ok(SourceRow {
            id: r.get(0)?,
            root_path: r.get(1)?,
            display_name: r.get(2)?,
            kind: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
            last_indexed_at: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn delete_source(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("DELETE FROM sources WHERE id = ?1", params![id])?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRow {
    pub id: String,
    pub root_path: String,
    pub display_name: String,
    pub kind: String,
    pub enabled: bool,
    pub last_indexed_at: Option<i64>,
}

pub fn list_documents(pool: &SqlitePool, limit: i64) -> AppResult<Vec<DocumentRow>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, source_id, project_id, title, path, mime_type, size_bytes, content_hash, status, error, created_at, updated_at
         FROM documents ORDER BY updated_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| {
        Ok(DocumentRow {
            id: r.get(0)?,
            source_id: r.get(1)?,
            project_id: r.get(2)?,
            title: r.get(3)?,
            path: r.get(4)?,
            mime_type: r.get(5)?,
            size_bytes: r.get(6)?,
            content_hash: r.get(7)?,
            status: r.get(8)?,
            error: r.get(9)?,
            created_at: r.get(10)?,
            updated_at: r.get(11)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn get_document(pool: &SqlitePool, id: &str) -> AppResult<DocumentRow> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, source_id, project_id, title, path, mime_type, size_bytes, content_hash, status, error, created_at, updated_at
         FROM documents WHERE id = ?1",
        params![id],
        |r| {
            Ok(DocumentRow {
                id: r.get(0)?,
                source_id: r.get(1)?,
                project_id: r.get(2)?,
                title: r.get(3)?,
                path: r.get(4)?,
                mime_type: r.get(5)?,
                size_bytes: r.get(6)?,
                content_hash: r.get(7)?,
                status: r.get(8)?,
                error: r.get(9)?,
                created_at: r.get(10)?,
                updated_at: r.get(11)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("document {id}")),
        other => AppError::Db(other),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRow {
    pub id: String,
    pub source_id: Option<String>,
    pub project_id: Option<String>,
    pub title: String,
    pub path: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    /// SHA-256 hash of the file content. Optional in the wire format
    /// for backward compatibility with older records that predate the
    /// column; the frontend treats it as a best-effort display value
    /// (see `app/web/src/pages/Knowledge.tsx`).
    pub content_hash: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn ingest_source(
    pool: SqlitePool,
    request: IngestSourceRequest,
    target_tokens: usize,
    overlap_tokens: usize,
    max_file_bytes: u64,
    cancel: CancellationToken,
) -> AppResult<IngestStats> {
    let mut stats = IngestStats { files_seen: 0, files_indexed: 0, files_skipped: 0, files_failed: 0, chunks: 0, errors: Vec::new() };
    let source_id = register_source(&pool, &request)?;
    let path = PathBuf::from(&request.path);
    if !path.exists() {
        return Err(AppError::NotFound(format!("path {} not found", path.display())));
    }

    let files: Vec<PathBuf> = if path.is_dir() {
        WalkDir::new(&path)
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

    for file in files {
        if cancel.is_cancelled() { break; }
        // Coarse size filter.
        let meta = match std::fs::metadata(&file) {
            Ok(m) => m,
            Err(e) => { stats.errors.push(format!("{}: {e}", file.display())); stats.files_failed += 1; continue; }
        };
        if meta.len() > max_file_bytes {
            stats.files_skipped += 1;
            continue;
        }

        // Compute hash; dedup by content hash.
        let (hash, size) = match hash::hash_file(&file) {
            Ok(t) => t,
            Err(e) => { stats.errors.push(format!("{}: {e}", file.display())); stats.files_failed += 1; continue; }
        };

        if let Some(existing) = find_document_by_hash(&pool, &hash)? {
            stats.files_skipped += 1;
            let _ = existing; // could be used to update the source linkage
            continue;
        }

        // Extract text.
        let extraction = match extractors::extract(&file) {
            Ok(e) => e,
            Err(e) => {
                let id = insert_document(&pool, Some(&source_id), &file, &hash, size, "failed", Some(&e.to_string()))?;
                stats.errors.push(format!("{}: {e}", file.display()));
                stats.files_failed += 1;
                let _ = id;
                continue;
            }
        };

        // Insert the document row.
        let doc_id = insert_document(&pool, Some(&source_id), &file, &hash, size, "indexed", None)?;

        // Chunk and insert.
        let chunks: Vec<Chunk> = chunk_text(&extraction.text, target_tokens, overlap_tokens);
        let metadata_str = serde_json::to_string(&extraction.metadata).unwrap_or_else(|_| "{}".into());
        for c in &chunks {
            insert_chunk(&pool, &doc_id, c, &metadata_str)?;
        }
        stats.chunks += chunks.len();
        stats.files_indexed += 1;
    }

    // Update last_indexed_at.
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute("UPDATE sources SET last_indexed_at = ?1 WHERE id = ?2", params![now, source_id])?;

    // Audit.
    let _ = audit::log(&pool, "system", "ingest.source", Some(&source_id),
        serde_json::json!({"files_seen": stats.files_seen, "files_indexed": stats.files_indexed, "chunks": stats.chunks}));

    let _ = Arc::new(()); // keep Arc import used; future: chunker workers
    Ok(stats)
}

fn is_skipped_dir(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(name, ".git" | "node_modules" | "target" | "dist" | "build" | ".venv" | "venv" | "__pycache__" | ".idea" | ".vscode")
}

fn find_document_by_hash(pool: &SqlitePool, content_hash: &str) -> AppResult<Option<String>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare("SELECT id FROM documents WHERE content_hash = ?1")?;
    let mut rows = stmt.query(params![content_hash])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn insert_document(pool: &SqlitePool, source_id: Option<&str>, path: &Path, hash: &str, size: u64, status: &str, error: Option<&str>) -> AppResult<String> {
    let id = Ulid::new().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let title = path.file_name().and_then(|n| n.to_str()).unwrap_or("(unnamed)").to_string();
    let mime = mime_from_path(path);
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO documents (id, source_id, project_id, title, path, mime_type, size_bytes, content_hash, status, error, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",
        params![id, source_id, Option::<String>::None, title, path.display().to_string(), mime, size as i64, hash, status, error, now],
    )?;
    Ok(id)
}

fn mime_from_path(p: &Path) -> Option<String> {
    let ext = p.extension().and_then(|s| s.to_str())?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        "json" => "application/json",
        "csv" => "text/csv",
        "pdf" => "application/pdf",
        "html" | "htm" => "text/html",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ipynb" => "application/x-ipynb",
        _ => "text/plain",
    }.to_string())
}

pub fn insert_chunk(pool: &SqlitePool, document_id: &str, c: &Chunk, metadata: &str) -> AppResult<String> {
    let id = Ulid::new().to_string();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO document_chunks (id, document_id, ordinal, text, token_estimate, start_offset, end_offset, metadata)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![id, document_id, c.ordinal as i64, c.text, c.token_estimate as i64, c.start_offset as i64, c.end_offset as i64, metadata],
    )?;
    Ok(id)
}

pub fn delete_document(pool: &SqlitePool, id: &str) -> AppResult<()> {
    // Remove vectors first.
    let _ = vector::delete_for_document(pool, id);
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_chunk(pool: &SqlitePool, chunk_id: &str) -> AppResult<Option<ChunkRow>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT c.id, c.document_id, c.ordinal, c.text, c.page, c.section, c.start_offset, c.end_offset, d.title, d.path
         FROM document_chunks c JOIN documents d ON d.id = c.document_id WHERE c.id = ?1",
    )?;
    let mut rows = stmt.query(params![chunk_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(ChunkRow {
            id: row.get(0)?,
            document_id: row.get(1)?,
            ordinal: row.get(2)?,
            text: row.get(3)?,
            page: row.get(4)?,
            section: row.get(5)?,
            start_offset: row.get(6)?,
            end_offset: row.get(7)?,
            document_title: row.get(8)?,
            document_path: row.get(9)?,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRow {
    pub id: String,
    pub document_id: String,
    pub ordinal: i64,
    pub text: String,
    pub page: Option<i64>,
    pub section: Option<String>,
    pub start_offset: i64,
    pub end_offset: i64,
    pub document_title: String,
    pub document_path: String,
}