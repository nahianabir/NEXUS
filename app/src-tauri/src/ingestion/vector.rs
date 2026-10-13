//! Vector store. We use sqlite-vec with one vec0 table per
//! embedding dimension: `chunk_vectors_<dim>`. Migration 0016
//! renamed the legacy 384-dim table to `chunk_vectors_384` so
//! the data is preserved.
//!
//! The dimension is **no longer a hardcoded constant**; it is
//! stored against the profile that produced the vectors and
//! against the `embedding_collections` registry. The
//! application probes a candidate profile by sending a single
//! embed call before running the bulk backfill. Two embedding
//! models that share a dimension are *not* interchangeable
//! (semantically incompatible), so the `embedding_collections`
//! table records the originating `model_id` and a stable
//! `model_identity` and refuses to mix them.

use rusqlite::params;
use rusqlite::OptionalExtension;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

/// Name of the per-dimension vec0 table. Tests and migrations
/// rely on this format.
pub fn chunk_vectors_table(dimension: u32) -> String {
    format!("chunk_vectors_{dimension}")
}

/// Returns the names of the user-facing per-dimension vec0
/// tables. The vec0 extension also creates shadow helper
/// tables (e.g. `chunk_vectors_384_rowids`,
/// `chunk_vectors_384_vector_chunks00`,
/// `chunk_vectors_384_info`, `chunk_vectors_384_chunks`)
/// that do NOT have a `chunk_id` column and must never be
/// included in user-facing queries. This helper filters to
/// tables whose name is exactly `chunk_vectors_` followed by
/// an unsigned integer.
fn list_user_dim_tables(conn: &rusqlite::Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'chunk_vectors_%'",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out: Vec<String> = Vec::new();
    for row in rows {
        let name = row?;
        let Some(suffix) = name.strip_prefix("chunk_vectors_") else {
            continue;
        };
        if suffix.is_empty() {
            continue;
        }
        if !suffix.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        out.push(name);
    }
    Ok(out)
}

/// Information about a registered embedding collection.
#[derive(Debug, Clone)]
pub struct EmbeddingCollection {
    pub dimension: u32,
    pub model_id: String,
    pub model_identity: String,
    pub vector_rows: i64,
}

/// Ensure a per-dimension vec0 table exists. Idempotent.
/// Returns true if a new table was created by this call.
pub fn ensure_dimension_table(pool: &SqlitePool, dimension: u32) -> AppResult<bool> {
    if dimension == 0 || dimension > 4096 {
        return Err(AppError::Invalid(format!(
            "embedding dimension {dimension} is outside the supported range [1, 4096]"
        )));
    }
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let table = chunk_vectors_table(dimension);
    // Pre-check whether the table already exists. vec0
    // shadow tables (e.g. `<table>_rowids`,
    // `<table>_vector_chunks00`) also count as evidence the
    // table is present.
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND (name = ?1 OR name LIKE ?2)",
            params![table, format!("{table}_%")],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let sql = format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS {table} USING vec0(chunk_id TEXT PRIMARY KEY, embedding float[{dimension}])"
    );
    conn.execute(&sql, [])?;
    Ok(exists == 0)
}

/// Register a new embedding collection. Called by the backfill
/// before the first vector insert. Refuses to mix models
/// within the same dimension.
pub fn register_collection(
    pool: &SqlitePool,
    dimension: u32,
    model_id: &str,
    model_identity: &str,
) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = chrono::Utc::now().timestamp_millis();
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT model_id, model_identity FROM embedding_collections WHERE dimension = ?1",
            params![dimension as i64],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .unwrap_or(None);
    if let Some((existing_model, existing_identity)) = existing {
        let same_model = existing_model == model_id;
        let same_identity = existing_identity == model_identity
            || existing_identity == "legacy"
            || model_identity == "legacy";
        if !same_model && !same_identity {
            return Err(AppError::Invalid(format!(
                "embedding collection for dimension {dimension} already exists for model '{existing_model}' \
                 (identity '{existing_identity}'). Pick a different collection (delete the existing one \
                 from the Knowledge Vault) or use the same model. Refusing to mix incompatible embeddings."
            )));
        }
        return Ok(());
    }
    conn.execute(
        "INSERT INTO embedding_collections (dimension, model_id, model_identity, created_at, vector_rows) \
         VALUES (?1, ?2, ?3, ?4, 0)",
        params![dimension as i64, model_id, model_identity, now],
    )?;
    Ok(())
}

/// Insert (or replace) a vector for a chunk. The caller MUST
/// have already registered the collection and ensured the
/// per-dimension vec0 table exists. Mixing dimensions in the
/// same table is impossible because the SQL is built from
/// `chunk_vectors_<dim>`.
pub fn upsert(pool: &SqlitePool, chunk_id: &str, embedding: &[f32]) -> AppResult<()> {
    let dim = embedding.len() as u32;
    if dim == 0 {
        return Err(AppError::Invalid("empty embedding".into()));
    }
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let table = chunk_vectors_table(dim);
    let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
    let sql = format!(
        "INSERT OR REPLACE INTO {table} (chunk_id, embedding) VALUES (?1, ?2)"
    );
    conn.execute(&sql, params![chunk_id, bytes])?;
    Ok(())
}

pub fn delete_for_document(pool: &SqlitePool, document_id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    for table in list_user_dim_tables(&conn)? {
        let sql = format!(
            "DELETE FROM {table} WHERE chunk_id IN (SELECT id FROM document_chunks WHERE document_id = ?1)"
        );
        let _ = conn.execute(&sql, params![document_id]);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct VectorHit {
    pub chunk_id: String,
    pub document_id: String,
    pub distance: f32,
    pub text: String,
    pub ordinal: i64,
}

pub fn list_collections(pool: &SqlitePool) -> AppResult<Vec<EmbeddingCollection>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT dimension, model_id, model_identity, vector_rows FROM embedding_collections ORDER BY dimension",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(EmbeddingCollection {
            dimension: r.get::<_, i64>(0)? as u32,
            model_id: r.get(1)?,
            model_identity: r.get(2)?,
            vector_rows: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

/// Number of vectors across all per-dimension tables.
pub fn total_vector_rows(pool: &SqlitePool) -> AppResult<i64> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut total = 0i64;
    for name in list_user_dim_tables(&conn)? {
        let sql = format!("SELECT COUNT(*) FROM {name}");
        let n: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap_or(0);
        total += n;
    }
    Ok(total)
}

/// Search the collection that matches the query dimension. If
/// the database has no collection for the query dimension we
/// return an empty result (not an error) so that retrieval
/// degrades gracefully when the user switches embedding
/// models.
pub fn search(pool: &SqlitePool, query: &[f32], limit: usize) -> AppResult<Vec<VectorHit>> {
    let dim = query.len() as u32;
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let registered: bool = conn
        .query_row(
            "SELECT 1 FROM embedding_collections WHERE dimension = ?1",
            params![dim as i64],
            |_| Ok(true),
        )
        .optional()
        .unwrap_or(None)
        .unwrap_or(false);
    if !registered {
        return Ok(Vec::new());
    }
    let table = chunk_vectors_table(dim);
    let bytes: Vec<u8> = query.iter().flat_map(|f| f.to_le_bytes()).collect();
    // vec0 KNN queries require a `k = ?` constraint on the
    // vec0 table itself. The `MATCH` operator cannot be used
    // inside a subquery against a vec0 table, so we run two
    // queries: first the KNN against the vec0 table, then a
    // lookup of the matching chunks in `document_chunks`. We
    // keep the order from the KNN result.
    let k = (limit as i64).clamp(1, 4096);
    let knn_sql = format!(
        "SELECT chunk_id, distance FROM {table} WHERE embedding MATCH ?1 AND k = ?2"
    );
    let mut knn_stmt = conn.prepare(&knn_sql)?;
    let raw: Vec<(String, f32)> = knn_stmt
        .query_map(params![bytes, k], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f32>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    // Look up the matching chunk rows in one batch.
    let placeholders: Vec<String> = (0..raw.len()).map(|i| format!("?{}", i + 1)).collect();
    let chunk_sql = format!(
        "SELECT id, document_id, text, ordinal FROM document_chunks WHERE id IN ({})",
        placeholders.join(",")
    );
    let mut chunk_stmt = conn.prepare(&chunk_sql)?;
    let chunk_ids: Vec<&str> = raw.iter().map(|(id, _)| id.as_str()).collect();
    let mut rows = chunk_stmt.query(rusqlite::params_from_iter(chunk_ids.iter()))?;
    let mut by_id: std::collections::HashMap<String, (String, String, i64)> = std::collections::HashMap::new();
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        by_id.insert(id, (row.get(1)?, row.get(2)?, row.get(3)?));
    }
    let mut out = Vec::with_capacity(raw.len());
    for (chunk_id, distance) in raw {
        if let Some((document_id, text, ordinal)) = by_id.get(&chunk_id) {
            out.push(VectorHit {
                chunk_id,
                document_id: document_id.clone(),
                distance,
                text: text.clone(),
                ordinal: *ordinal,
            });
        }
    }
    Ok(out)
}
