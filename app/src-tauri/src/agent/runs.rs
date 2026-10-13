//! Agent run CRUD.

use rusqlite::params;

use super::{now_ms, AgentRun};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn row_to_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRun> {
    Ok(AgentRun {
        id: r.get(0)?,
        conversation_id: r.get(1)?,
        project_id: r.get(2)?,
        title: r.get(3)?,
        status: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
        finished_at: r.get(7)?,
    })
}

pub fn create(pool: &SqlitePool, r: &AgentRun) -> AppResult<AgentRun> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut r = r.clone();
    if r.id.is_empty() { r.id = super::new_id(); }
    if r.created_at == 0 { r.created_at = now; }
    r.updated_at = now;
    if r.status.is_empty() { r.status = "running".into(); }
    conn.execute(
        "INSERT INTO agent_runs (id, conversation_id, project_id, title, status, created_at, updated_at, finished_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![r.id, r.conversation_id, r.project_id, r.title, r.status, r.created_at, r.updated_at, r.finished_at],
    )?;
    Ok(r)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<AgentRun> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, conversation_id, project_id, title, status, created_at, updated_at, finished_at FROM agent_runs WHERE id = ?1",
        params![id],
        row_to_run,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("agent run {id}")),
        other => AppError::Db(other),
    })
}

pub fn list(pool: &SqlitePool, limit: i64) -> AppResult<Vec<AgentRun>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, project_id, title, status, created_at, updated_at, finished_at FROM agent_runs ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], row_to_run)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn set_status(pool: &SqlitePool, id: &str, status: &str, finished: bool) -> AppResult<AgentRun> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let finished_at = if finished { Some(now) } else { None };
    conn.execute(
        "UPDATE agent_runs SET status = ?1, updated_at = ?2, finished_at = COALESCE(?3, finished_at) WHERE id = ?4",
        params![status, now, finished_at, id],
    )?;
    get(pool, id)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("DELETE FROM agent_runs WHERE id = ?1", params![id])?;
    Ok(())
}
