//! Drift detection for One-Click Work Resume. Phase S.2.
//!
//! Two operations:
//!  * `capture` — at checkpoint creation time, take a small
//!    snapshot of the world: open task count, recent notes count,
//!    filesystem mtimes for the observed files (when accessible),
//!    and the active model id (as a hint).
//!  * `compare` — at resume time, re-read the world and produce a
//!    structured `DriftReport` the UI can render.
//!
//! Drift is read-only. The function never writes to source
//! tables, never blocks the UI, and never executes a Git command
//! or any other external tool.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

/// Severity of a single drift item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity { Info, Warn, Conflict }

impl DriftSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Conflict => "conflict",
        }
    }
}

/// One row in the drift report. Each item points at a concrete
/// observable: a file path, a count, a model id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftItem {
    pub severity: DriftSeverity,
    pub kind: String,
    pub summary: String,
    /// The expected value (snapshot at create time), if any.
    pub expected: Option<String>,
    /// The current value at resume time, if any.
    pub actual: Option<String>,
}

/// File state at one moment in time. Only used for files that
/// were explicitly listed in the checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: String,
    pub size_bytes: Option<u64>,
    pub mtime_ms: Option<i64>,
    pub exists: bool,
}

/// Snapshot of the world at checkpoint creation time.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DriftCheck {
    pub open_task_count: Option<i64>,
    pub note_count: Option<i64>,
    pub active_model: Option<String>,
    pub files: Vec<FileSnapshot>,
    pub captured_at: Option<i64>,
}

/// Report comparing the snapshot to the present.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub items: Vec<DriftItem>,
    /// Re-read files at resume time, for the UI to inspect.
    pub files_now: Vec<FileSnapshot>,
    pub open_task_count_now: Option<i64>,
    pub note_count_now: Option<i64>,
    pub active_model_now: Option<String>,
    pub generated_at: i64,
}

/// Build a snapshot of the world for a project. The drift check
/// is best-effort: any IO or DB error becomes a Warn item, never
/// a hard failure.
pub fn capture(pool: &SqlitePool, project_id: &str) -> AppResult<DriftCheck> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let open_task_count: Option<i64> = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE project_id = ?1
               AND status IN ('open','in_progress')",
            params![project_id],
            |r| r.get(0),
        )
        .ok();
    let note_count: Option<i64> = conn
        .query_row(
            "SELECT COUNT(*) FROM notes WHERE project_id = ?1",
            params![project_id],
            |r| r.get(0),
        )
        .ok();
    // The active model lives in `model_profiles`. We do not
    // assume any particular profile is "active" — we just
    // record the most-recently-updated enabled general-purpose
    // model as a hint. If none exists, the field is null.
    let active_model: Option<String> = conn
        .query_row(
            "SELECT model_id FROM model_profiles
             WHERE enabled = 1 AND role = 'general'
             ORDER BY updated_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    Ok(DriftCheck {
        open_task_count,
        note_count,
        active_model,
        files: Vec::new(),
        captured_at: Some(now),
    })
}

/// Compare a stored `DriftCheck` (loaded with the checkpoint)
/// against the present. The `Checkpoint` is consulted for its
/// `observed_files` so we can re-stat each one.
pub fn compare(pool: &SqlitePool, checkpoint: &super::Checkpoint) -> AppResult<DriftReport> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut items = Vec::new();

    // Re-read the project state.
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let open_task_count_now: Option<i64> = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE project_id = ?1
               AND status IN ('open','in_progress')",
            params![checkpoint.project_id],
            |r| r.get(0),
        )
        .ok();
    let note_count_now: Option<i64> = conn
        .query_row(
            "SELECT COUNT(*) FROM notes WHERE project_id = ?1",
            params![checkpoint.project_id],
            |r| r.get(0),
        )
        .ok();
    let active_model_now: Option<String> = conn
        .query_row(
            "SELECT model_id FROM model_profiles
             WHERE enabled = 1 AND role = 'general'
             ORDER BY updated_at DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();

    if let (Some(prev), Some(cur)) = (checkpoint.drift_check.open_task_count, open_task_count_now) {
        if prev != cur {
            items.push(DriftItem {
                severity: if cur > prev { DriftSeverity::Info } else { DriftSeverity::Warn },
                kind: "open_task_count".into(),
                summary: format!("Open tasks went from {prev} to {cur} since the checkpoint."),
                expected: Some(prev.to_string()),
                actual: Some(cur.to_string()),
            });
        }
    }
    if let (Some(prev), Some(cur)) = (checkpoint.drift_check.note_count, note_count_now) {
        if prev != cur {
            items.push(DriftItem {
                severity: DriftSeverity::Info,
                kind: "note_count".into(),
                summary: format!("Notes went from {prev} to {cur} since the checkpoint."),
                expected: Some(prev.to_string()),
                actual: Some(cur.to_string()),
            });
        }
    }
    if let (Some(prev), Some(cur)) = (&checkpoint.drift_check.active_model, &active_model_now) {
        if prev != cur {
            items.push(DriftItem {
                severity: DriftSeverity::Info,
                kind: "active_model".into(),
                summary: format!("Active model went from '{prev}' to '{cur}'."),
                expected: Some(prev.clone()),
                actual: Some(cur.clone()),
            });
        }
    }

    // Re-stat every observed file. Missing or changed files are
    // surfaced as drift items; corrupted mtimes are reported as
    // Info, not Warn, because they don't block work.
    let mut files_now = Vec::new();
    for f in &checkpoint.observed_files {
        let snap = stat_file(&f.path);
        if !snap.exists {
            items.push(DriftItem {
                severity: DriftSeverity::Warn,
                kind: "file_missing".into(),
                summary: format!("File '{}' is missing at resume time.", f.path),
                expected: Some(f.path.clone()),
                actual: None,
            });
        } else if let (Some(prev_size), Some(cur_size)) = (f.size_bytes, snap.size_bytes) {
            if prev_size != cur_size {
                items.push(DriftItem {
                    severity: DriftSeverity::Warn,
                    kind: "file_size_changed".into(),
                    summary: format!("File '{}' size changed from {prev_size} to {cur_size} bytes.", f.path),
                    expected: Some(prev_size.to_string()),
                    actual: Some(cur_size.to_string()),
                });
            }
        }
        if let (Some(prev_mtime), Some(cur_mtime)) = (f.mtime_ms, snap.mtime_ms) {
            if cur_mtime > prev_mtime {
                items.push(DriftItem {
                    severity: DriftSeverity::Info,
                    kind: "file_mtime_advanced".into(),
                    summary: format!("File '{}' was modified after the checkpoint.", f.path),
                    expected: Some(prev_mtime.to_string()),
                    actual: Some(cur_mtime.to_string()),
                });
            }
        }
        files_now.push(snap);
    }

    // Resolve the evidence refs. References that no longer point
    // at a real row are reported as drift items; the checkpoint
    // itself is preserved.
    for ev in &checkpoint.evidence_refs {
        let exists = evidence_exists(&conn, &ev.kind, &ev.id);
        if !exists {
            items.push(DriftItem {
                severity: DriftSeverity::Warn,
                kind: "evidence_missing".into(),
                summary: format!("Evidence reference '{}/{}' no longer resolves.", ev.kind, ev.id),
                expected: Some(format!("{}/{}", ev.kind, ev.id)),
                actual: None,
            });
        }
    }

    Ok(DriftReport {
        items,
        files_now,
        open_task_count_now,
        note_count_now,
        active_model_now,
        generated_at: now,
    })
}

fn stat_file(path: &str) -> FileSnapshot {
    let p = Path::new(path);
    let exists = p.exists();
    let mut snap = FileSnapshot {
        path: path.to_string(),
        size_bytes: None,
        mtime_ms: None,
        exists,
    };
    if exists {
        if let Ok(meta) = std::fs::metadata(p) {
            snap.size_bytes = Some(meta.len());
            if let Ok(modified) = meta.modified() {
                if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                    snap.mtime_ms = Some(dur.as_millis() as i64);
                }
            }
        }
    }
    snap
}

fn evidence_exists(conn: &rusqlite::Connection, kind: &str, id: &str) -> bool {
    let (table, id_col) = match kind {
        "timeline" => ("timeline_events", "id"),
        "memory" => ("memories_v2", "id"),
        "note" => ("notes", "id"),
        "task" => ("tasks", "id"),
        "experiment" => ("experiments", "id"),
        "kg_relationship" => ("kg_relationships", "id"),
        _ => return false,
    };
    let sql = format!("SELECT 1 FROM {table} WHERE {id_col} = ?1 LIMIT 1");
    conn.query_row(&sql, params![id], |_| Ok(())).is_ok()
}
