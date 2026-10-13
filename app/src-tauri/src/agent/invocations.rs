//! Invocation log: one row per tool call.

use rusqlite::params;

use super::{now_ms, Invocation};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn row_to_inv(r: &rusqlite::Row<'_>) -> rusqlite::Result<Invocation> {
    let input_json: String = r.get(3)?;
    let output_json: Option<String> = r.get(4)?;
    let input: serde_json::Value = serde_json::from_str(&input_json).unwrap_or(serde_json::json!({}));
    let output = output_json.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    Ok(Invocation {
        id: r.get(0)?,
        run_id: r.get(1)?,
        tool_name: r.get(2)?,
        permission: r.get(8)?,
        input,
        output,
        status: r.get(5)?,
        error: r.get(6)?,
        started_at: r.get(7)?,
        finished_at: r.get(9)?,
    })
}

pub fn create(pool: &SqlitePool, inv: &Invocation) -> AppResult<Invocation> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut inv = inv.clone();
    if inv.id.is_empty() { inv.id = super::new_id(); }
    if inv.started_at == 0 { inv.started_at = now; }
    let input_str = serde_json::to_string(&inv.input).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO agent_invocations (id, run_id, tool_name, input_json, output_json, status, error, started_at, finished_at, permission)
         VALUES (?1,?2,?3,?4,NULL,?5,?6,?7,NULL,?8)",
        params![inv.id, inv.run_id, inv.tool_name, input_str, inv.status, inv.error, inv.started_at, inv.permission],
    )?;
    Ok(inv)
}

pub fn update_status(pool: &SqlitePool, id: &str, status: &str, output: Option<&serde_json::Value>, error: Option<&str>, finished: bool) -> AppResult<Invocation> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let output_str = output.and_then(|v| serde_json::to_string(v).ok());
    let finished_at = if finished { Some(now) } else { None };
    conn.execute(
        "UPDATE agent_invocations SET status = ?1, output_json = COALESCE(?2, output_json), error = COALESCE(?3, error), finished_at = COALESCE(?4, finished_at) WHERE id = ?5",
        params![status, output_str, error, finished_at, id],
    )?;
    get(pool, id)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Invocation> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, run_id, tool_name, input_json, output_json, status, error, started_at, permission, finished_at
         FROM agent_invocations WHERE id = ?1",
        params![id],
        row_to_inv,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("invocation {id}")),
        other => AppError::Db(other),
    })
}

pub fn list_for_run(pool: &SqlitePool, run_id: &str) -> AppResult<Vec<Invocation>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, run_id, tool_name, input_json, output_json, status, error, started_at, permission, finished_at
         FROM agent_invocations WHERE run_id = ?1 ORDER BY started_at",
    )?;
    let rows = stmt.query_map(params![run_id], row_to_inv)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn list_recent(pool: &SqlitePool, limit: i64) -> AppResult<Vec<Invocation>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, run_id, tool_name, input_json, output_json, status, error, started_at, permission, finished_at
         FROM agent_invocations ORDER BY started_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], row_to_inv)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}
