//! Projects: a project belongs to (optionally) a workspace and holds
//! instructions, goals, conversations, notes, tasks, and knowledge-vault
//! documents. The active project id is stored in `ui_state`.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub workspace_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub goals: Vec<String>,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn list(pool: &SqlitePool, include_archived: bool) -> AppResult<Vec<Project>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let sql = "SELECT id, workspace_id, name, description, instructions, goals, archived, created_at, updated_at
               FROM projects
               WHERE ?1 OR archived = 0
               ORDER BY archived ASC, updated_at DESC";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![include_archived as i64], |r| {
        Ok(Project {
            id: r.get(0)?,
            workspace_id: r.get(1)?,
            name: r.get(2)?,
            description: r.get(3)?,
            instructions: r.get(4)?,
            goals: serde_json::from_str(&r.get::<_, Option<String>>(5)?.unwrap_or_else(|| "[]".into())).unwrap_or_default(),
            archived: r.get::<_, i64>(6)? != 0,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Project> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, workspace_id, name, description, instructions, goals, archived, created_at, updated_at
         FROM projects WHERE id = ?1",
        params![id],
        |r| Ok(Project {
            id: r.get(0)?,
            workspace_id: r.get(1)?,
            name: r.get(2)?,
            description: r.get(3)?,
            instructions: r.get(4)?,
            goals: serde_json::from_str(&r.get::<_, Option<String>>(5)?.unwrap_or_else(|| "[]".into())).unwrap_or_default(),
            archived: r.get::<_, i64>(6)? != 0,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("project {id}")),
        other => AppError::Db(other),
    })
}

pub fn create(
    pool: &SqlitePool,
    workspace_id: Option<&str>,
    name: &str,
    description: Option<&str>,
    instructions: Option<&str>,
    goals: &[String],
) -> AppResult<Project> {
    let now = chrono::Utc::now().timestamp_millis();
    let p = Project {
        id: new_id(),
        workspace_id: workspace_id.map(|s| s.to_string()),
        name: name.to_string(),
        description: description.map(|s| s.to_string()),
        instructions: instructions.map(|s| s.to_string()),
        goals: goals.to_vec(),
        archived: false,
        created_at: now,
        updated_at: now,
    };
    let goals_json = serde_json::to_string(&p.goals).unwrap_or_else(|_| "[]".into());
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO projects (id, workspace_id, name, description, instructions, goals, archived, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)",
        params![p.id, p.workspace_id, p.name, p.description, p.instructions, goals_json, p.created_at, p.updated_at],
    )?;
    Ok(p)
}

pub fn update(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
    instructions: Option<&str>,
    goals: Option<&[String]>,
) -> AppResult<Project> {
    let now = chrono::Utc::now().timestamp_millis();
    let goals_json = match goals {
        Some(g) => Some(serde_json::to_string(g).unwrap_or_else(|_| "[]".into())),
        None => None,
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Build a dynamic update so we can pass None for "leave alone".
    // SQLite's `coalesce` makes this clean.
    conn.execute(
        "UPDATE projects SET
            name         = COALESCE(?1, name),
            description  = COALESCE(?2, description),
            instructions = COALESCE(?3, instructions),
            goals        = COALESCE(?4, goals),
            updated_at   = ?5
         WHERE id = ?6",
        rusqlite::params![name, description, instructions, goals_json, now, id],
    )?;
    get(pool, id)
}

pub fn set_archived(pool: &SqlitePool, id: &str, archived: bool) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE projects SET archived = ?1, updated_at = ?2 WHERE id = ?3",
        params![archived as i64, now, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("project {id}"))); }
    Ok(())
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("project {id}"))); }
    Ok(())
}

pub fn count_tasks(pool: &SqlitePool, project_id: &str, open_only: bool) -> AppResult<i64> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let sql = if open_only {
        "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND status IN ('open','in_progress')"
    } else {
        "SELECT COUNT(*) FROM tasks WHERE project_id = ?1"
    };
    Ok(conn.query_row(sql, params![project_id], |r| r.get::<_, i64>(0))?)
}

pub fn count_notes(pool: &SqlitePool, project_id: &str) -> AppResult<i64> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM notes WHERE project_id = ?1",
        params![project_id],
        |r| r.get::<_, i64>(0),
    )?)
}
