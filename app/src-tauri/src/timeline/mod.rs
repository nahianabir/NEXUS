//! Project Time Machine. Phase S.1 of the Continuity Engine.
//!
//! This module is **read-only against the source tables**. It walks
//! existing rows in `audit_events`, `tasks`, `notes`, `memories_v2`,
//! `experiments`, `experiment_runs`, `kg_relationships`,
//! `agent_invocations`, `research_papers`, and `research_reports`,
//! and copies a small, structured summary into `timeline_events`.
//! Re-ingesting the same source row is a no-op (UNIQUE constraint on
//! `(source_kind, source_id, event_type, event_at)`).
//!
//! `detail_json` is a small JSON blob: an excerpt, a list of
//! relevant entity ids, etc. We never copy whole documents or
//! chat messages into the timeline — the source row remains the
//! canonical record and the UI navigates back to it.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub mod ingest;
pub mod reconstruct;
pub mod sources;
pub mod query;

pub use reconstruct::{compare, reconstruct_at, Comparison, Reconstruction, SnapshotSection};

/// The row written to / read from `timeline_events`. Mirrors the
/// schema in `migrations/0012_timeline.sql` exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub project_id: Option<String>,
    pub event_type: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_label: Option<String>,
    pub event_at: i64,
    pub imported_at: i64,
    pub summary: String,
    pub detail_json: serde_json::Value,
    pub ingested_by: String,
    pub ingest_run_id: Option<String>,
    pub created_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

/// The filter accepted by [`list`]. All fields are optional.
#[derive(Debug, Default, Clone)]
pub struct TimelineFilter {
    pub project_id: Option<String>,
    /// Restrict to a source kind ("audit", "task", ...).
    pub source_kind: Option<String>,
    /// Restrict to a high-level event type ("created", "completed", ...).
    pub event_type: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IngestReport {
    pub run_id: String,
    pub examined: u64,
    pub inserted: u64,
    pub skipped_existing: u64,
    pub sources: Vec<SourceReport>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SourceReport {
    pub source_kind: String,
    pub examined: u64,
    pub inserted: u64,
    pub skipped_existing: u64,
}

const SELECT_COLS: &str = "id, project_id, event_type, source_kind, source_id, source_label, \
                           event_at, imported_at, summary, detail_json, ingested_by, \
                           ingest_run_id, created_at";

fn row_to_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<TimelineEvent> {
    let detail_raw: String = r.get(9)?;
    let detail: serde_json::Value = serde_json::from_str(&detail_raw).unwrap_or(serde_json::Value::Null);
    Ok(TimelineEvent {
        id: r.get(0)?,
        project_id: r.get(1)?,
        event_type: r.get(2)?,
        source_kind: r.get(3)?,
        source_id: r.get(4)?,
        source_label: r.get(5)?,
        event_at: r.get(6)?,
        imported_at: r.get(7)?,
        summary: r.get(8)?,
        detail_json: detail,
        ingested_by: r.get(10)?,
        ingest_run_id: r.get(11)?,
        created_at: r.get(12)?,
    })
}

/// Insert (or no-op) a single timeline event. Returns `true` if a
/// new row was written, `false` if the unique key already existed.
///
/// `detail_json` must be a JSON-serialisable value. `summary` is
/// trimmed and truncated to 240 chars; a long summary becomes the
/// head plus `…`.
pub fn insert(
    pool: &SqlitePool,
    project_id: Option<&str>,
    event_type: &str,
    source_kind: &str,
    source_id: Option<&str>,
    source_label: Option<&str>,
    event_at: i64,
    summary: &str,
    detail: serde_json::Value,
    ingested_by: &str,
    ingest_run_id: Option<&str>,
) -> AppResult<bool> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = chrono::Utc::now().timestamp_millis();
    let id = new_id();
    let summary = trim_summary(summary);
    let detail_str = serde_json::to_string(&detail).unwrap_or_else(|_| "{}".into());
    let n = conn.execute(
        "INSERT OR IGNORE INTO timeline_events (
            id, project_id, event_type, source_kind, source_id, source_label,
            event_at, imported_at, summary, detail_json, ingested_by, ingest_run_id, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            id,
            project_id,
            event_type,
            source_kind,
            source_id,
            source_label,
            event_at,
            now,
            summary,
            detail_str,
            ingested_by,
            ingest_run_id,
            now,
        ],
    )?;
    Ok(n > 0)
}

fn trim_summary(s: &str) -> String {
    let s = s.trim();
    if s.len() <= 240 { return s.to_string(); }
    let mut end = 237.min(s.len());
    while !s.is_char_boundary(end) { end -= 1; }
    format!("{}…", &s[..end])
}

/// Build an `ingest_run_id` shared by every event written during a
/// single ingest call. Useful for "undo last ingest" or for the
/// `timeline_run` index to show "this whole timeline was built at
/// the same time".
pub fn new_run_id() -> String { Ulid::new().to_string() }

/// Return the highest `imported_at` currently stored, or `None` if
/// the table is empty. Lets the ingestors compute incremental ranges
/// without re-scanning every row.
pub fn last_imported_at(pool: &SqlitePool, project_id: Option<&str>) -> AppResult<Option<i64>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let row: Option<Option<i64>> = match project_id {
        Some(pid) => conn.query_row(
            "SELECT MAX(imported_at) FROM timeline_events WHERE project_id = ?1",
            params![pid],
            |r| r.get::<_, Option<i64>>(0),
        ).ok(),
        None => conn.query_row(
            "SELECT MAX(imported_at) FROM timeline_events",
            [],
            |r| r.get::<_, Option<i64>>(0),
        ).ok(),
    };
    Ok(row.flatten())
}

/// Paginated, deterministic list. Sort is `(event_at DESC, id ASC)`
/// so equal timestamps remain stable across requests.
pub fn list(pool: &SqlitePool, filter: &TimelineFilter) -> AppResult<Vec<TimelineEvent>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = format!("SELECT {SELECT_COLS} FROM timeline_events WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(p) = &filter.project_id {
        sql.push_str(" AND project_id = ?"); args.push(Box::new(p.clone()));
    }
    if let Some(s) = &filter.source_kind {
        sql.push_str(" AND source_kind = ?"); args.push(Box::new(s.clone()));
    }
    if let Some(e) = &filter.event_type {
        sql.push_str(" AND event_type = ?"); args.push(Box::new(e.clone()));
    }
    if let Some(s) = filter.since_ms {
        sql.push_str(" AND event_at >= ?"); args.push(Box::new(s));
    }
    if let Some(u) = filter.until_ms {
        sql.push_str(" AND event_at <= ?"); args.push(Box::new(u));
    }
    sql.push_str(" ORDER BY event_at DESC, id ASC");
    let lim = filter.limit.unwrap_or(200).clamp(1, 1000);
    sql.push_str(" LIMIT ?"); args.push(Box::new(lim as i64));
    let mut stmt = conn.prepare(&sql)?;
    let mapped = stmt.query_map(rusqlite::params_from_iter(args.iter()), row_to_event)?;
    let mut out = Vec::new();
    for row in mapped { out.push(row?); }
    Ok(out)
}

/// Count of events for a project. Used by the dashboard tile.
pub fn count_for_project(pool: &SqlitePool, project_id: &str) -> AppResult<i64> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM timeline_events WHERE project_id = ?1",
        params![project_id],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Delete a single timeline event. Used by the manual "remove from
/// timeline" affordance. Does not touch the source record.
pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM timeline_events WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("timeline event {id}"))); }
    Ok(())
}

/// Manually add a timeline entry. Used by the "Add a milestone"
/// affordance. `event_at` defaults to "now" if `None` is passed.
pub fn insert_manual(
    pool: &SqlitePool,
    project_id: &str,
    summary: &str,
    event_at: Option<i64>,
    detail: serde_json::Value,
) -> AppResult<TimelineEvent> {
    let now = chrono::Utc::now().timestamp_millis();
    let event_at = event_at.unwrap_or(now);
    let run = new_run_id();
    insert(
        pool,
        Some(project_id),
        "manual",
        "manual",
        None,
        None,
        event_at,
        summary,
        detail,
        "manual",
        Some(&run),
    )?;
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        &format!("SELECT {SELECT_COLS} FROM timeline_events
                  WHERE source_kind = 'manual' AND event_type = 'manual'
                    AND event_at = ?1 AND project_id = ?2
                  ORDER BY imported_at DESC, id DESC LIMIT 1"),
        params![event_at, project_id],
        row_to_event,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::Internal("manual event not found after insert".into()),
        other => AppError::Db(other),
    })
}