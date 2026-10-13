//! `memory_evidence` rows. The chain back to the original source.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub memory_id: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_location: Option<String>,
    pub excerpt: Option<String>,
    pub verified: bool,
    pub created_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn add(
    pool: &SqlitePool,
    memory_id: &str,
    source_kind: &str,
    source_id: Option<&str>,
    source_location: Option<&str>,
    excerpt: Option<&str>,
) -> AppResult<Evidence> {
    let now = chrono::Utc::now().timestamp_millis();
    let e = Evidence {
        id: new_id(),
        memory_id: memory_id.to_string(),
        source_kind: source_kind.to_string(),
        source_id: source_id.map(|s| s.to_string()),
        source_location: source_location.map(|s| s.to_string()),
        excerpt: excerpt.map(|s| s.to_string()),
        verified: false,
        created_at: now,
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO memory_evidence (id, memory_id, source_kind, source_id, source_location, excerpt, verified, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        params![e.id, e.memory_id, e.source_kind, e.source_id, e.source_location, e.excerpt, e.created_at],
    )?;
    Ok(e)
}

pub fn verify(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("UPDATE memory_evidence SET verified = 1 WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("evidence {id}"))); }
    Ok(())
}

pub fn list_for_memory(pool: &SqlitePool, memory_id: &str) -> AppResult<Vec<Evidence>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, memory_id, source_kind, source_id, source_location, excerpt, verified, created_at
         FROM memory_evidence WHERE memory_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![memory_id], |r| {
        Ok(Evidence {
            id: r.get(0)?,
            memory_id: r.get(1)?,
            source_kind: r.get(2)?,
            source_id: r.get(3)?,
            source_location: r.get(4)?,
            excerpt: r.get(5)?,
            verified: r.get::<_, i64>(6)? != 0,
            created_at: r.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM memory_evidence WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("evidence {id}"))); }
    Ok(())
}
