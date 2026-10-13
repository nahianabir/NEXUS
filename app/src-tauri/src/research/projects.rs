//! Research project CRUD.

use rusqlite::params;

use super::{now_ms, row_to_project, ResearchProject};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn create(pool: &SqlitePool, p: &ResearchProject) -> AppResult<ResearchProject> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut p = p.clone();
    if p.id.is_empty() { p.id = super::new_id(); }
    if p.created_at == 0 { p.created_at = now; }
    p.updated_at = now;
    if p.status.is_empty() { p.status = "active".into(); }
    conn.execute(
        "INSERT INTO research_projects (id, workspace_id, title, question, description, status, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![p.id, p.workspace_id, p.title, p.question, p.description, p.status, p.created_at, p.updated_at],
    )?;
    Ok(p)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<ResearchProject> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, workspace_id, title, question, description, status, created_at, updated_at
         FROM research_projects WHERE id = ?1",
        params![id],
        row_to_project,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("research project {id}")),
        other => AppError::Db(other),
    })
}

pub fn list(pool: &SqlitePool, workspace_id: Option<&str>, status: Option<&str>, limit: i64) -> AppResult<Vec<ResearchProject>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from("SELECT id, workspace_id, title, question, description, status, created_at, updated_at FROM research_projects WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(w) = workspace_id {
        sql.push_str(" AND workspace_id = ?");
        args.push(Box::new(w.to_string()));
    }
    if let Some(s) = status {
        sql.push_str(" AND status = ?");
        args.push(Box::new(s.to_string()));
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
    args.push(Box::new(limit));
    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref() as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(params_refs), row_to_project)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn update(pool: &SqlitePool, p: &ResearchProject) -> AppResult<ResearchProject> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    conn.execute(
        "UPDATE research_projects SET workspace_id=?1, title=?2, question=?3, description=?4, status=?5, updated_at=?6 WHERE id=?7",
        params![p.workspace_id, p.title, p.question, p.description, p.status, now, p.id],
    )?;
    get(pool, &p.id)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("DELETE FROM research_projects WHERE id = ?1", params![id])?;
    Ok(())
}
