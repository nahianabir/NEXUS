//! Session records. The end-of-conversation summary the user can opt
//! into storing as a long-lived memory.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub workspace_id: Option<String>,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub work_completed: Option<String>,
    pub decisions_made: Option<String>,
    pub files_changed: Option<String>,
    pub experiments_performed: Option<String>,
    pub errors_encountered: Option<String>,
    pub open_questions: Option<String>,
    pub next_actions: Option<String>,
    pub approved: bool,
    pub started_at: i64,
    pub ended_at: i64,
    pub created_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn insert(pool: &SqlitePool, s: &Session) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO memory_sessions (
            id, workspace_id, project_id, conversation_id, title, summary,
            work_completed, decisions_made, files_changed, experiments_performed,
            errors_encountered, open_questions, next_actions, approved,
            started_at, ended_at, created_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
         )",
        params![
            s.id, s.workspace_id, s.project_id, s.conversation_id, s.title, s.summary,
            s.work_completed, s.decisions_made, s.files_changed, s.experiments_performed,
            s.errors_encountered, s.open_questions, s.next_actions, s.approved as i64,
            s.started_at, s.ended_at, s.created_at,
        ],
    )?;
    Ok(())
}

pub fn list_for_project(pool: &SqlitePool, project_id: &str, limit: i64) -> AppResult<Vec<Session>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, workspace_id, project_id, conversation_id, title, summary,
                work_completed, decisions_made, files_changed, experiments_performed,
                errors_encountered, open_questions, next_actions, approved,
                started_at, ended_at, created_at
         FROM memory_sessions WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![project_id, limit], row_to_session)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Session> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, workspace_id, project_id, conversation_id, title, summary,
                work_completed, decisions_made, files_changed, experiments_performed,
                errors_encountered, open_questions, next_actions, approved,
                started_at, ended_at, created_at
         FROM memory_sessions WHERE id = ?1",
        params![id],
        row_to_session,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("session {id}")),
        other => AppError::Db(other),
    })
}

fn row_to_session(r: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        project_id: r.get(2)?,
        conversation_id: r.get(3)?,
        title: r.get(4)?,
        summary: r.get(5)?,
        work_completed: r.get(6)?,
        decisions_made: r.get(7)?,
        files_changed: r.get(8)?,
        experiments_performed: r.get(9)?,
        errors_encountered: r.get(10)?,
        open_questions: r.get(11)?,
        next_actions: r.get(12)?,
        approved: r.get::<_, i64>(13)? != 0,
        started_at: r.get(14)?,
        ended_at: r.get(15)?,
        created_at: r.get(16)?,
    })
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM memory_sessions WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("session {id}"))); }
    Ok(())
}
