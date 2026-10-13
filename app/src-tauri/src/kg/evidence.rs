//! Evidence CRUD for relationships.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use super::types::Evidence;

pub fn new_id() -> String { Ulid::new().to_string() }

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CreateEvidence {
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_location: Option<String>,
    pub excerpt: Option<String>,
}

pub fn add_tx(tx: &rusqlite::Transaction<'_>, relationship_id: &str, req: &CreateEvidence) -> AppResult<Evidence> {
    let now = chrono::Utc::now().timestamp_millis();
    let e = Evidence {
        id: new_id(),
        relationship_id: relationship_id.to_string(),
        source_kind: req.source_kind.clone(),
        source_id: req.source_id.clone(),
        source_location: req.source_location.clone(),
        excerpt: req.excerpt.clone(),
        created_at: now,
    };
    tx.execute(
        "INSERT INTO kg_evidence (id, relationship_id, source_kind, source_id, source_location, excerpt, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![e.id, e.relationship_id, e.source_kind, e.source_id, e.source_location, e.excerpt, e.created_at],
    )?;
    Ok(e)
}

pub fn add(pool: &SqlitePool, relationship_id: &str, req: &CreateEvidence) -> AppResult<Evidence> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut tx = conn.unchecked_transaction()?;
    let e = add_tx(&tx, relationship_id, req)?;
    tx.commit()?;
    Ok(e)
}

pub fn list_for(pool: &SqlitePool, relationship_id: &str) -> AppResult<Vec<Evidence>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, relationship_id, source_kind, source_id, source_location, excerpt, created_at
         FROM kg_evidence WHERE relationship_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![relationship_id], |r| {
        Ok(Evidence {
            id: r.get(0)?,
            relationship_id: r.get(1)?,
            source_kind: r.get(2)?,
            source_id: r.get(3)?,
            source_location: r.get(4)?,
            excerpt: r.get(5)?,
            created_at: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM kg_evidence WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("evidence {id}"))); }
    Ok(())
}
