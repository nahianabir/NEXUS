//! Per-profile lifecycle state machine. Phase F.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State { Unloaded, Loading, Ready, Busy, Unloading, LoadFailed, UnloadFailed }

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unloaded => "unloaded",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Busy => "busy",
            Self::Unloading => "unloading",
            Self::LoadFailed => "load_failed",
            Self::UnloadFailed => "unload_failed",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "unloaded" => Ok(Self::Unloaded),
            "loading" => Ok(Self::Loading),
            "ready" => Ok(Self::Ready),
            "busy" => Ok(Self::Busy),
            "unloading" => Ok(Self::Unloading),
            "load_failed" => Ok(Self::LoadFailed),
            "unload_failed" => Ok(Self::UnloadFailed),
            other => Err(AppError::Invalid(format!("lifecycle state: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub profile_id: String,
    pub state: String,
    pub last_transition: i64,
    pub last_error: Option<String>,
    pub loaded_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub inflight: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn ensure_row(pool: &SqlitePool, profile_id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR IGNORE INTO router_lifecycle (profile_id, state, last_transition, inflight)
         VALUES (?1, 'unloaded', ?2, 0)",
        params![profile_id, now],
    )?;
    Ok(())
}

pub fn transition(pool: &SqlitePool, profile_id: &str, to: State, error: Option<&str>) -> AppResult<Status> {
    ensure_row(pool, profile_id)?;
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let loaded_at = if to == State::Ready { Some(now) } else { None };
    conn.execute(
        "UPDATE router_lifecycle SET state = ?1, last_transition = ?2, last_error = ?3, loaded_at = COALESCE(?4, loaded_at)
         WHERE profile_id = ?5",
        params![to.as_str(), now, error, loaded_at, profile_id],
    )?;
    get(pool, profile_id)
}

pub fn bump_inflight(pool: &SqlitePool, profile_id: &str, delta: i64) -> AppResult<Status> {
    ensure_row(pool, profile_id)?;
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE router_lifecycle SET inflight = MAX(0, inflight + ?1) WHERE profile_id = ?2",
        params![delta, profile_id],
    )?;
    get(pool, profile_id)
}

pub fn touch_used(pool: &SqlitePool, profile_id: &str) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE router_lifecycle SET last_used_at = ?1 WHERE profile_id = ?2",
        params![now, profile_id],
    )?;
    Ok(())
}

pub fn get(pool: &SqlitePool, profile_id: &str) -> AppResult<Status> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT profile_id, state, last_transition, last_error, loaded_at, last_used_at, inflight
         FROM router_lifecycle WHERE profile_id = ?1",
        params![profile_id],
        |r| Ok(Status {
            profile_id: r.get(0)?,
            state: r.get(1)?,
            last_transition: r.get(2)?,
            last_error: r.get(3)?,
            loaded_at: r.get(4)?,
            last_used_at: r.get(5)?,
            inflight: r.get(6)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("lifecycle for {profile_id}")),
        other => AppError::Db(other),
    })
}

pub fn list_all(pool: &SqlitePool) -> AppResult<Vec<Status>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT profile_id, state, last_transition, last_error, loaded_at, last_used_at, inflight FROM router_lifecycle",
    )?;
    let rows = stmt.query_map([], |r| Ok(Status {
        profile_id: r.get(0)?,
        state: r.get(1)?,
        last_transition: r.get(2)?,
        last_error: r.get(3)?,
        loaded_at: r.get(4)?,
        last_used_at: r.get(5)?,
        inflight: r.get(6)?,
    }))?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn lifecycle_walks_through_states() {
        let dir = tempdir();
        let pool = db::open_pool(&db::default_db_path(&dir)).unwrap();
        let conn = pool.get().unwrap();
        let mut conn = conn;
        db::migrations::run(&mut conn).unwrap();
        // Need a parent row in model_profiles because router_lifecycle
        // has a FK to it. Insert a minimal profile directly.
        let id = format!("test-profile-{}", ulid::Ulid::new());
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO model_profiles (id, role, display_name, provider, model_id, context_len, temperature, top_p, max_tokens, enabled, created_at, updated_at)
             VALUES (?1, 'general', 'test', 'lmstudio', 'm', 4096, 0.7, 0.95, 1024, 1, ?2, ?2)",
            rusqlite::params![id, now],
        ).unwrap();
        ensure_row(&pool, &id).unwrap();
        transition(&pool, &id, State::Loading, None).unwrap();
        transition(&pool, &id, State::Ready, None).unwrap();
        let s = get(&pool, &id).unwrap();
        assert_eq!(s.state, "ready");
        assert!(s.loaded_at.is_some());
        transition(&pool, &id, State::Busy, None).unwrap();
        bump_inflight(&pool, &id, 1).unwrap();
        let s = get(&pool, &id).unwrap();
        assert_eq!(s.inflight, 1);
    }

    fn tempdir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("nexus-router-{}", Ulid::new()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
