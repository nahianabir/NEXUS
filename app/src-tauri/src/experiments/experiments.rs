//! Experiment CRUD.

use rusqlite::params;

use super::{now_ms, row_to_experiment, Experiment};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn create(pool: &SqlitePool, e: &Experiment) -> AppResult<Experiment> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut e = e.clone();
    if e.id.is_empty() { e.id = super::new_id(); }
    if e.created_at == 0 { e.created_at = now; }
    e.updated_at = now;
    if e.status.is_empty() { e.status = "active".into(); }
    let tags_json = serde_json::to_string(&e.tags).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "INSERT INTO experiments (id, project_id, name, description, tags_json, status, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![e.id, e.project_id, e.name, e.description, tags_json, e.status, e.created_at, e.updated_at],
    )?;
    Ok(e)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Experiment> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, project_id, name, description, tags_json, status, created_at, updated_at FROM experiments WHERE id = ?1",
        params![id],
        row_to_experiment,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("experiment {id}")),
        other => AppError::Db(other),
    })
}

pub fn list(pool: &SqlitePool, project_id: Option<&str>, status: Option<&str>, limit: i64) -> AppResult<Vec<Experiment>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from("SELECT id, project_id, name, description, tags_json, status, created_at, updated_at FROM experiments WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(p) = project_id { sql.push_str(" AND project_id = ?"); args.push(Box::new(p.to_string())); }
    if let Some(s) = status { sql.push_str(" AND status = ?"); args.push(Box::new(s.to_string())); }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
    args.push(Box::new(limit));
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref() as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(refs), row_to_experiment)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn update(pool: &SqlitePool, e: &Experiment) -> AppResult<Experiment> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let tags_json = serde_json::to_string(&e.tags).unwrap_or_else(|_| "[]".into());
    conn.execute(
        "UPDATE experiments SET project_id=?1, name=?2, description=?3, tags_json=?4, status=?5, updated_at=?6 WHERE id=?7",
        params![e.project_id, e.name, e.description, tags_json, e.status, now, e.id],
    )?;
    get(pool, &e.id)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("DELETE FROM experiments WHERE id = ?1", params![id])?;
    Ok(())
}
