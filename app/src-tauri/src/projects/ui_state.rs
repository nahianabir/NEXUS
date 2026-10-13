//! Tiny key/value store for UI state. The "active project" pointer lives
//! here so the chat command can pick it up without a separate state file.
//!
//! Values are JSON-encoded strings. The schema is intentionally generic so
//! future UI flags (sidebar collapse, last-opened source, etc.) can land
//! here without a new migration.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn get<T: for<'de> Deserialize<'de>>(pool: &SqlitePool, key: &str) -> AppResult<Option<T>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare("SELECT value FROM ui_state WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    let Some(row) = rows.next()? else { return Ok(None) };
    let raw: String = row.get(0)?;
    let v = serde_json::from_str(&raw).map_err(AppError::Json)?;
    Ok(Some(v))
}

pub fn set<T: Serialize>(pool: &SqlitePool, key: &str, value: &T) -> AppResult<()> {
    let raw = serde_json::to_string(value)?;
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO ui_state (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, raw, now],
    )?;
    Ok(())
}

pub fn unset(pool: &SqlitePool, key: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("DELETE FROM ui_state WHERE key = ?1", params![key])?;
    Ok(())
}

pub const ACTIVE_PROJECT: &str = "active_project";
