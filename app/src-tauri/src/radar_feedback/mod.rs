//! Opportunity Radar feedback. Phase F.
//!
//! A "radar_feedback" row records the user's response to a
//! surfaced opportunity. The kinds are: `dismiss`,
//! `snooze`, `pin`, `done`, `false`.
//!
//! The radar (S.3.1) consults this table on every scan to
//! exclude dismissed / done / false opportunities and to
//! expire snoozes. Pinned rows do not exclude — they sort
//! first.
//!
//! The "key" is a stable string derived from the opportunity
//! kind and the target id (e.g. `"overdue_urgent_task:abc123"`).
//! It is independent of any radar report id, so a feedback
//! row outlives the report that produced the opportunity.
//!
//! Deleting a feedback row re-surfaces the opportunity.
//! There is no automatic cleanup; the user is the source of
//! truth.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind { Dismiss, Snooze, Pin, Done, False }

impl FeedbackKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dismiss => "dismiss",
            Self::Snooze => "snooze",
            Self::Pin => "pin",
            Self::Done => "done",
            Self::False => "false",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "dismiss" => Ok(Self::Dismiss),
            "snooze" => Ok(Self::Snooze),
            "pin" => Ok(Self::Pin),
            "done" => Ok(Self::Done),
            "false" => Ok(Self::False),
            other => Err(AppError::Invalid(format!("radar feedback kind: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarFeedback {
    pub id: String,
    pub project_id: String,
    pub key: String,
    pub kind: String,
    pub title: Option<String>,
    pub note: Option<String>,
    pub until_ms: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertFeedback {
    pub project_id: String,
    pub key: String,
    pub kind: FeedbackKind,
    pub title: Option<String>,
    pub note: Option<String>,
    /// Required when `kind` is `Snooze`; ignored otherwise.
    pub until_ms: Option<i64>,
}

pub fn upsert(pool: &SqlitePool, req: &UpsertFeedback) -> AppResult<RadarFeedback> {
    if req.key.trim().is_empty() {
        return Err(AppError::Invalid("radar feedback key is required".into()));
    }
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Idempotent: if a row exists for (project_id, key), update
    // it; otherwise insert. The user can change their mind
    // (dismiss → pin) and the new state replaces the old.
    let existing: Option<String> = conn.query_row(
        "SELECT id FROM radar_feedback WHERE project_id = ?1 AND key = ?2",
        params![req.project_id, req.key],
        |r| r.get(0),
    ).ok();
    let id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE radar_feedback SET
                    kind = ?1, title = ?2, note = ?3, until_ms = ?4, updated_at = ?5
                 WHERE id = ?6",
                params![
                    req.kind.as_str(), req.title, req.note, req.until_ms, now, id,
                ],
            )?;
            id
        }
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO radar_feedback
                    (id, project_id, key, kind, title, note, until_ms, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                params![
                    id, req.project_id, req.key, req.kind.as_str(),
                    req.title, req.note, req.until_ms, now,
                ],
            )?;
            id
        }
    };
    get(pool, &id)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<RadarFeedback> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, project_id, key, kind, title, note, until_ms, created_at, updated_at
         FROM radar_feedback WHERE id = ?1",
        params![id],
        |r| Ok(RadarFeedback {
            id: r.get(0)?,
            project_id: r.get(1)?,
            key: r.get(2)?,
            kind: r.get(3)?,
            title: r.get(4)?,
            note: r.get(5)?,
            until_ms: r.get(6)?,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("radar feedback {id}")),
        other => AppError::Db(other),
    })
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM radar_feedback WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("radar feedback {id}"))); }
    Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct ListFilter {
    pub project_id: Option<String>,
    pub kind: Option<String>,
    pub limit: Option<usize>,
}

pub fn list(pool: &SqlitePool, filter: &ListFilter) -> AppResult<Vec<RadarFeedback>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from(
        "SELECT id, project_id, key, kind, title, note, until_ms, created_at, updated_at
         FROM radar_feedback WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(p) = &filter.project_id {
        sql.push_str(" AND project_id = ?"); args.push(Box::new(p.clone()));
    }
    if let Some(k) = &filter.kind {
        sql.push_str(" AND kind = ?"); args.push(Box::new(k.clone()));
    }
    sql.push_str(" ORDER BY updated_at DESC");
    let lim = filter.limit.unwrap_or(200).clamp(1, 1000);
    sql.push_str(" LIMIT ?"); args.push(Box::new(lim as i64));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok(RadarFeedback {
            id: r.get(0)?,
            project_id: r.get(1)?,
            key: r.get(2)?,
            kind: r.get(3)?,
            title: r.get(4)?,
            note: r.get(5)?,
            until_ms: r.get(6)?,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

/// Compute the set of feedback rules to apply during a radar
/// scan. The output is `(exclude_keys, pinned_keys)`. A key
/// in `exclude_keys` is removed from the report. A key in
/// `pinned_keys` is sorted first. Snoozed keys whose
/// `until_ms` is in the past are ignored (the snooze has
/// expired).
pub fn rules_for(
    pool: &SqlitePool,
    project_id: &str,
    now: i64,
) -> AppResult<(std::collections::HashSet<String>, std::collections::HashSet<String>)> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut exclude: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut pinned: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut stmt = conn.prepare(
        "SELECT key, kind, until_ms FROM radar_feedback
         WHERE project_id = ?1",
    )?;
    let rows = stmt.query_map(params![project_id], |r| {
        let key: String = r.get(0)?;
        let kind: String = r.get(1)?;
        let until: Option<i64> = r.get(2)?;
        Ok((key, kind, until))
    })?;
    for row in rows {
        let (key, kind, until) = row?;
        match kind.as_str() {
            "dismiss" | "done" | "false" => { exclude.insert(key); }
            "snooze" => {
                if let Some(u) = until {
                    if u > now { exclude.insert(key); }
                }
            }
            "pin" => { pinned.insert(key); }
            _ => {}
        }
    }
    Ok((exclude, pinned))
}

/// Compute the radar "key" for an opportunity, used to
/// match against feedback rows. Stable across runs.
pub fn opportunity_key(kind: &str, target_id: &str) -> String {
    format!("{kind}:{target_id}")
}
