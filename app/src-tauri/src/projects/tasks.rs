//! Tasks: priority, status, due date, checklist, and project link.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus { Open, InProgress, Done, Cancelled }

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "open" => Ok(Self::Open),
            "in_progress" => Ok(Self::InProgress),
            "done" => Ok(Self::Done),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(AppError::Invalid(format!("task status: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority { Low, Normal, High, Urgent }

impl TaskPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "urgent" => Ok(Self::Urgent),
            other => Err(AppError::Invalid(format!("task priority: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub due_at: Option<i64>,
    pub checklist: Vec<ChecklistItem>,
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
}

pub fn new_id() -> String { Ulid::new().to_string() }

fn row_to_task(r: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let checklist_raw: Option<String> = r.get(6)?;
    let checklist: Vec<ChecklistItem> = match checklist_raw {
        Some(s) => serde_json::from_str(&s).unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(Task {
        id: r.get(0)?,
        project_id: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        status: r.get(4)?,
        priority: r.get(5)?,
        checklist,
        notes: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
        due_at: r.get(10)?,
        completed_at: r.get(11)?,
    })
}

const SELECT_COLS: &str = "id, project_id, title, description, status, priority, checklist, notes, created_at, updated_at, due_at, completed_at";

pub fn list(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
    include_done: bool,
) -> AppResult<Vec<Task>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = format!("SELECT {SELECT_COLS} FROM tasks WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(pid) = project_id {
        sql.push_str(" AND project_id = ?");
        args.push(Box::new(pid.to_string()));
    }
    if let Some(st) = status {
        sql.push_str(" AND status = ?");
        args.push(Box::new(st.to_string()));
    } else if !include_done {
        sql.push_str(" AND status IN ('open','in_progress')");
    }
    sql.push_str(" ORDER BY CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'normal' THEN 2 ELSE 3 END, due_at ASC NULLS LAST, created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let mapped = stmt.query_map(rusqlite::params_from_iter(args.iter()), row_to_task)?;
    let mut out = Vec::new();
    for row in mapped { out.push(row?); }
    Ok(out)
}

pub fn list_due_in_range(pool: &SqlitePool, from: i64, to: i64) -> AppResult<Vec<Task>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} FROM tasks
         WHERE due_at IS NOT NULL AND due_at >= ?1 AND due_at <= ?2
         ORDER BY due_at ASC"
    ))?;
    let mapped = stmt.query_map(params![from, to], row_to_task)?;
    let mut out = Vec::new();
    for row in mapped { out.push(row?); }
    Ok(out)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let sql = format!("SELECT {SELECT_COLS} FROM tasks WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_task).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("task {id}")),
        other => AppError::Db(other),
    })
}

pub fn create(
    pool: &SqlitePool,
    project_id: Option<&str>,
    title: &str,
    description: Option<&str>,
    priority: TaskPriority,
    due_at: Option<i64>,
    checklist: &[ChecklistItem],
) -> AppResult<Task> {
    let now = chrono::Utc::now().timestamp_millis();
    let t = Task {
        id: new_id(),
        project_id: project_id.map(|s| s.to_string()),
        title: title.to_string(),
        description: description.map(|s| s.to_string()),
        status: TaskStatus::Open.as_str().into(),
        priority: priority.as_str().into(),
        due_at,
        checklist: checklist.to_vec(),
        notes: None,
        created_at: now,
        updated_at: now,
        completed_at: None,
    };
    let checklist_json = serde_json::to_string(&t.checklist).unwrap_or_else(|_| "[]".into());
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO tasks (id, project_id, title, description, status, priority, due_at, checklist, notes, created_at, updated_at, completed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)",
        params![t.id, t.project_id, t.title, t.description, t.status, t.priority, t.due_at, checklist_json, t.notes, t.created_at, t.updated_at],
    )?;
    Ok(t)
}

pub fn update(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    priority: Option<TaskPriority>,
    due_at: Option<Option<i64>>,
    checklist: Option<&[ChecklistItem]>,
    notes: Option<&str>,
) -> AppResult<Task> {
    let now = chrono::Utc::now().timestamp_millis();
    let checklist_json = checklist.map(|c| serde_json::to_string(c).unwrap_or_else(|_| "[]".into()));
    let due_inner = match due_at { Some(v) => v, None => None };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE tasks SET
            title       = COALESCE(?1, title),
            description = COALESCE(?2, description),
            priority    = COALESCE(?3, priority),
            due_at      = ?4,
            checklist   = COALESCE(?5, checklist),
            notes       = COALESCE(?6, notes),
            updated_at  = ?7
         WHERE id = ?8",
        params![
            title,
            description,
            priority.map(|p| p.as_str().to_string()),
            due_inner,
            checklist_json,
            notes,
            now,
            id,
        ],
    )?;
    get(pool, id)
}

pub fn set_status(pool: &SqlitePool, id: &str, status: TaskStatus) -> AppResult<Task> {
    let now = chrono::Utc::now().timestamp_millis();
    let completed_at = match status {
        TaskStatus::Done | TaskStatus::Cancelled => Some(now),
        _ => None,
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE tasks SET status = ?1, completed_at = ?2, updated_at = ?3 WHERE id = ?4",
        params![status.as_str(), completed_at, now, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("task {id}"))); }
    get(pool, id)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("task {id}"))); }
    Ok(())
}
