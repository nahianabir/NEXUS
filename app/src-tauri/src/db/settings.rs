//! `app_settings` key/value helpers. Values are JSON strings.

use rusqlite::params;
use serde::{de::DeserializeOwned, Serialize};

use crate::error::AppResult;
use crate::db::SqlitePool;

pub fn get<T: DeserializeOwned>(pool: &SqlitePool, key: &str) -> AppResult<Option<T>> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        let raw: String = row.get(0)?;
        let v: T = serde_json::from_str(&raw)?;
        Ok(Some(v))
    } else {
        Ok(None)
    }
}

pub fn set<T: Serialize>(pool: &SqlitePool, key: &str, value: &T) -> AppResult<()> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let raw = serde_json::to_string(value)?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, raw, now],
    )?;
    Ok(())
}
