//! Citation rendering. The Knowledge Vault's hybrid search produces
//! `(chunk_id, score)` rows; the assistant converts those into
//! `message_sources` rows + a list of `Citation` objects for the UI.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

/// One rendered citation. The `index` is the bracket number the model
/// should embed in its answer ([1], [2], ...). `text` is a short preview
/// of the chunk; the UI may expand it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub index: i64,
    pub chunk_id: String,
    pub document_id: String,
    pub document_title: String,
    pub document_path: String,
    pub score: f64,
    pub source: String, // "fts" | "vector" | "hybrid"
    pub text: String,
}

/// Persist the citations used by an assistant message. The model-side
/// `assistant_message_id` is the row in `messages`; the `chunk_id` rows
/// live in `document_chunks`. We de-duplicate by `(message_id, chunk_id)`
/// before inserting.
pub fn persist_citations(
    pool: &SqlitePool,
    assistant_message_id: &str,
    citations: &[Citation],
) -> AppResult<()> {
    let mut conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let tx = conn.transaction()?;
    for c in citations {
        // (message_id, chunk_id) is unique in spirit; the schema PK is
        // (message_id, citation_index) so we delete any existing row first.
        tx.execute(
            "DELETE FROM message_sources WHERE message_id = ?1 AND chunk_id = ?2",
            params![assistant_message_id, c.chunk_id],
        )?;
        tx.execute(
            "INSERT INTO message_sources (message_id, chunk_id, score, citation_index) VALUES (?1, ?2, ?3, ?4)",
            params![assistant_message_id, c.chunk_id, c.score, c.index],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Load the citations for an assistant message. Used by the UI to render
/// the "Sources" panel under the message.
pub fn list_for_message(pool: &SqlitePool, assistant_message_id: &str) -> AppResult<Vec<Citation>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT ms.citation_index, ms.chunk_id, ms.score, c.document_id, c.text, d.title, d.path
         FROM message_sources ms
         JOIN document_chunks c ON c.id = ms.chunk_id
         JOIN documents d ON d.id = c.document_id
         WHERE ms.message_id = ?1
         ORDER BY ms.citation_index",
    )?;
    let rows = stmt.query_map(params![assistant_message_id], |r| {
        Ok(Citation {
            index: r.get(0)?,
            chunk_id: r.get(1)?,
            score: r.get(2)?,
            document_id: r.get(3)?,
            text: r.get(4)?,
            document_title: r.get(5)?,
            document_path: r.get(6)?,
            source: "hybrid".into(), // we do not currently record which path produced the hit
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn new_citation_id() -> String { Ulid::new().to_string() }
