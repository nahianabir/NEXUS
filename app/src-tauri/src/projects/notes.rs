//! Notes: free-form text attached to (optionally) a project.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub body: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn list(pool: &SqlitePool, project_id: Option<&str>) -> AppResult<Vec<Note>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match project_id {
        Some(_) => (
            "SELECT id, project_id, title, body, created_at, updated_at
             FROM notes WHERE project_id = ?1 ORDER BY updated_at DESC",
            vec![Box::new(project_id.unwrap().to_string())],
        ),
        None => (
            "SELECT id, project_id, title, body, created_at, updated_at
             FROM notes ORDER BY updated_at DESC",
            vec![],
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let mapped = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
        Ok(Note {
            id: r.get(0)?,
            project_id: r.get(1)?,
            title: r.get(2)?,
            body: r.get(3)?,
            created_at: r.get(4)?,
            updated_at: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in mapped { out.push(row?); }
    Ok(out)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Note> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, project_id, title, body, created_at, updated_at FROM notes WHERE id = ?1",
        params![id],
        |r| Ok(Note {
            id: r.get(0)?,
            project_id: r.get(1)?,
            title: r.get(2)?,
            body: r.get(3)?,
            created_at: r.get(4)?,
            updated_at: r.get(5)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("note {id}")),
        other => AppError::Db(other),
    })
}

pub fn create(pool: &SqlitePool, project_id: Option<&str>, title: &str, body: &str) -> AppResult<Note> {
    let now = chrono::Utc::now().timestamp_millis();
    let n = Note {
        id: new_id(),
        project_id: project_id.map(|s| s.to_string()),
        title: title.to_string(),
        body: body.to_string(),
        created_at: now,
        updated_at: now,
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO notes (id, project_id, title, body, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![n.id, n.project_id, n.title, n.body, n.created_at, n.updated_at],
    )?;
    Ok(n)
}

pub fn update(pool: &SqlitePool, id: &str, title: Option<&str>, body: Option<&str>) -> AppResult<Note> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE notes SET
            title      = COALESCE(?1, title),
            body       = COALESCE(?2, body),
            updated_at = ?3
         WHERE id = ?4",
        params![title, body, now, id],
    )?;
    get(pool, id)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("note {id}"))); }
    Ok(())
}
