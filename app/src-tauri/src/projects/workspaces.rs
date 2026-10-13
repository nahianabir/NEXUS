//! Workspaces: the top-level container for projects.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn list(pool: &SqlitePool) -> AppResult<Vec<Workspace>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, name, description, created_at FROM workspaces ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Workspace {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            created_at: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Workspace> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, name, description, created_at FROM workspaces WHERE id = ?1",
        params![id],
        |r| Ok(Workspace {
            id: r.get(0)?,
            name: r.get(1)?,
            description: r.get(2)?,
            created_at: r.get(3)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("workspace {id}")),
        other => AppError::Db(other),
    })
}

pub fn create(pool: &SqlitePool, name: &str, description: Option<&str>) -> AppResult<Workspace> {
    let w = Workspace {
        id: new_id(),
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO workspaces (id, name, description, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![w.id, w.name, w.description, w.created_at],
    )?;
    Ok(w)
}

pub fn rename(pool: &SqlitePool, id: &str, name: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("UPDATE workspaces SET name = ?1 WHERE id = ?2", params![name, id])?;
    if n == 0 { return Err(AppError::NotFound(format!("workspace {id}"))); }
    Ok(())
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // ON DELETE SET NULL keeps the projects intact; the UI can re-attach.
    let n = conn.execute("DELETE FROM workspaces WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("workspace {id}"))); }
    Ok(())
}
