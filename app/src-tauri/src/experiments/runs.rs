//! Experiment runs CRUD.

use rusqlite::params;
use serde::Serialize;

use super::{now_ms, row_to_run, ExperimentRun};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn create(pool: &SqlitePool, r: &ExperimentRun) -> AppResult<ExperimentRun> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut r = r.clone();
    if r.id.is_empty() { r.id = super::new_id(); }
    if r.created_at == 0 { r.created_at = now; }
    r.updated_at = now;
    if r.status.is_empty() { r.status = "running".into(); }
    if r.hyperparams.is_null() { r.hyperparams = serde_json::json!({}); }
    if r.metrics.is_null() { r.metrics = serde_json::json!({}); }
    let hp_json = serde_json::to_string(&r.hyperparams).unwrap_or_else(|_| "{}".into());
    let m_json = serde_json::to_string(&r.metrics).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO experiment_runs (id, experiment_id, name, status, hyperparams_json, metrics_json, code_revision, code_path, hardware, notes, started_at, ended_at, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![r.id, r.experiment_id, r.name, r.status, hp_json, m_json, r.code_revision, r.code_path, r.hardware, r.notes, r.started_at, r.ended_at, r.created_at, r.updated_at],
    )?;
    super::touch(pool, &r.experiment_id)?;
    Ok(r)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<ExperimentRun> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, experiment_id, name, status, hyperparams_json, metrics_json, code_revision, code_path, hardware, notes, started_at, ended_at, created_at, updated_at
         FROM experiment_runs WHERE id = ?1",
        params![id],
        row_to_run,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("run {id}")),
        other => AppError::Db(other),
    })
}

pub fn list_for(pool: &SqlitePool, experiment_id: &str) -> AppResult<Vec<ExperimentRun>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, experiment_id, name, status, hyperparams_json, metrics_json, code_revision, code_path, hardware, notes, started_at, ended_at, created_at, updated_at
         FROM experiment_runs WHERE experiment_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![experiment_id], row_to_run)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn update(pool: &SqlitePool, r: &ExperimentRun) -> AppResult<ExperimentRun> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let hp_json = serde_json::to_string(&r.hyperparams).unwrap_or_else(|_| "{}".into());
    let m_json = serde_json::to_string(&r.metrics).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "UPDATE experiment_runs SET name=?1, status=?2, hyperparams_json=?3, metrics_json=?4, code_revision=?5, code_path=?6, hardware=?7, notes=?8, started_at=?9, ended_at=?10, updated_at=?11 WHERE id=?12",
        params![r.name, r.status, hp_json, m_json, r.code_revision, r.code_path, r.hardware, r.notes, r.started_at, r.ended_at, now, r.id],
    )?;
    let got = get(pool, &r.id)?;
    super::touch(pool, &got.experiment_id)?;
    Ok(got)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let exp_id: String = conn.query_row("SELECT experiment_id FROM experiment_runs WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("run {id}")),
            other => AppError::Db(other),
        })?;
    conn.execute("DELETE FROM experiment_runs WHERE id = ?1", params![id])?;
    super::touch(pool, &exp_id)?;
    Ok(())
}

/// Compare the metric values across runs of the same experiment.
/// Returns one row per (metric_name, run_id) with the numeric value.
#[derive(Debug, Clone, Serialize)]
pub struct MetricPoint {
    pub run_id: String,
    pub run_name: String,
    pub metric: String,
    pub value: f64,
}

pub fn metric_history(pool: &SqlitePool, experiment_id: &str) -> AppResult<Vec<MetricPoint>> {
    let runs = list_for(pool, experiment_id)?;
    let mut out = Vec::new();
    for r in runs {
        if let Some(obj) = r.metrics.as_object() {
            for (k, v) in obj {
                if let Some(n) = v.as_f64() {
                    out.push(MetricPoint { run_id: r.id.clone(), run_name: r.name.clone(), metric: k.clone(), value: n });
                } else if let Some(n) = v.as_i64() {
                    out.push(MetricPoint { run_id: r.id.clone(), run_name: r.name.clone(), metric: k.clone(), value: n as f64 });
                }
            }
        }
    }
    Ok(out)
}
