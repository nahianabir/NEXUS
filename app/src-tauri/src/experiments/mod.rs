//! Experiment tracking. Phase H.
//!
//! An experiment is a "what I'm trying" entity with a few runs. A
//! run captures hyperparameters, metrics, code revision, and a free
//! notes blob. We deliberately do not implement MLflow; an optional
//! MLflow sync can be added later without schema changes.

pub mod experiments;
pub mod runs;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRun {
    pub id: String,
    pub experiment_id: String,
    pub name: String,
    pub status: String,
    pub hyperparams: serde_json::Value,
    pub metrics: serde_json::Value,
    pub code_revision: Option<String>,
    pub code_path: Option<String>,
    pub hardware: Option<String>,
    pub notes: Option<String>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn row_to_experiment(r: &rusqlite::Row<'_>) -> rusqlite::Result<Experiment> {
    let tags_json: String = r.get(4)?;
    let tags = serde_json::from_str::<Vec<String>>(&tags_json).unwrap_or_default();
    Ok(Experiment {
        id: r.get(0)?,
        project_id: r.get(1)?,
        name: r.get(2)?,
        description: r.get(3)?,
        tags,
        status: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

pub fn row_to_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<ExperimentRun> {
    let hp_json: String = r.get(4)?;
    let m_json: String = r.get(5)?;
    let hp: serde_json::Value = serde_json::from_str(&hp_json).unwrap_or(serde_json::json!({}));
    let m: serde_json::Value = serde_json::from_str(&m_json).unwrap_or(serde_json::json!({}));
    Ok(ExperimentRun {
        id: r.get(0)?,
        experiment_id: r.get(1)?,
        name: r.get(2)?,
        status: r.get(3)?,
        hyperparams: hp,
        metrics: m,
        code_revision: r.get(6)?,
        code_path: r.get(7)?,
        hardware: r.get(8)?,
        notes: r.get(9)?,
        started_at: r.get(10)?,
        ended_at: r.get(11)?,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

pub fn touch(pool: &SqlitePool, exp_id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("UPDATE experiments SET updated_at = ?1 WHERE id = ?2", params![now_ms(), exp_id])?;
    Ok(())
}
