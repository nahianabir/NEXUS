//! Audit log helpers. Every privileged action writes a row here.

use rusqlite::params;
use serde::Serialize;
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::AppResult;

pub fn log(
    pool: &SqlitePool,
    actor: &str,
    action: &str,
    target: Option<&str>,
    detail: serde_json::Value,
) -> AppResult<()> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let id = Ulid::new().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let detail_str = serde_json::to_string(&detail)?;
    conn.execute(
        "INSERT INTO audit_events (id, actor, action, target, detail, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, actor, action, target, detail_str, now],
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct AuditEvent {
    pub id: String,
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
    pub detail: serde_json::Value,
    pub created_at: i64,
}

/// Delete every audit event. Returns the number of rows removed.
pub fn clear(pool: &SqlitePool) -> AppResult<usize> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM audit_events", [])?;
    Ok(n)
}

/// Delete events older than `before_ms`. Returns the number of rows removed.
pub fn prune_before(pool: &SqlitePool, before_ms: i64) -> AppResult<usize> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM audit_events WHERE created_at < ?1", [before_ms])?;
    Ok(n)
}

/// Convenience: prune with the `audit_retention_days` setting.
pub fn prune_by_retention(pool: &SqlitePool, retention_days: u32) -> AppResult<usize> {
    if retention_days == 0 { return Ok(0); }
    let now = chrono::Utc::now().timestamp_millis();
    let cutoff = now - (retention_days as i64) * 86_400_000;
    prune_before(pool, cutoff)
}

pub fn recent(pool: &SqlitePool, limit: usize) -> AppResult<Vec<AuditEvent>> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, actor, action, target, detail, created_at FROM audit_events ORDER BY created_at DESC LIMIT ?1",
    )?;
    let mut rows = stmt.query(params![limit as i64])?;
    let mut out = Vec::with_capacity(limit);
    while let Some(row) = rows.next()? {
        let detail_raw: String = row.get(4)?;
        let detail: serde_json::Value = serde_json::from_str(&detail_raw).unwrap_or(serde_json::Value::Null);
        out.push(AuditEvent {
            id: row.get(0)?,
            actor: row.get(1)?,
            action: row.get(2)?,
            target: row.get(3)?,
            detail,
            created_at: row.get(5)?,
        });
    }
    Ok(out)
}
