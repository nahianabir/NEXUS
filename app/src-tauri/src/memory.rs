//! Persistent memory for the Assistant.
//!
//! Memories are short facts the model should remember across conversations.
//! They have a scope (`user` | `project` | `auto` | `conv`) and an approval
//! flag. `auto` memories are extracted from chat turns and start as
//! unapproved; the user can promote them to user/project scope.
//!
//! Phase 5 ships the CRUD + the approve/edit/delete UI. Phase 8 wires
//! auto-extraction as a post-turn hook.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope { User, Project, Auto, Conv }

impl MemoryScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Auto => "auto",
            Self::Conv => "conv",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "user" => Ok(Self::User),
            "project" => Ok(Self::Project),
            "auto" => Ok(Self::Auto),
            "conv" => Ok(Self::Conv),
            other => Err(AppError::Invalid(format!("memory scope: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub scope: String,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub content: String,
    pub source: Option<String>,
    pub approved: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn list(pool: &SqlitePool, scope: Option<&str>, approved_only: bool) -> AppResult<Vec<Memory>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = "SELECT id, scope, project_id, conversation_id, content, source, approved, created_at, updated_at
                   FROM memories WHERE 1=1".to_string();
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = scope {
        sql.push_str(" AND scope = ?");
        args.push(Box::new(s.to_string()));
    }
    if approved_only {
        sql.push_str(" AND approved = 1");
    }
    sql.push_str(" ORDER BY updated_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let mapped = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok(Memory {
            id: r.get(0)?,
            scope: r.get(1)?,
            project_id: r.get(2)?,
            conversation_id: r.get(3)?,
            content: r.get(4)?,
            source: r.get(5)?,
            approved: r.get::<_, i64>(6)? != 0,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in mapped { out.push(row?); }
    Ok(out)
}

pub fn list_for_context(
    pool: &SqlitePool,
    project_id: Option<&str>,
    conversation_id: Option<&str>,
) -> AppResult<Vec<Memory>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Approved memories are visible to the model. Scope rules:
    //   * user      — always
    //   * project   — if the conversation's project matches (or none specified)
    //   * conv      — if the conversation matches
    //   * auto      — once approved, treated like user (transient facts that
    //                 have been promoted to a long-lived memory)
    let sql = "SELECT id, scope, project_id, conversation_id, content, source, approved, created_at, updated_at
               FROM memories
               WHERE approved = 1
                 AND (
                    scope IN ('user', 'auto')
                    OR (scope = 'project' AND (project_id = ?1 OR ?1 IS NULL))
                    OR (scope = 'conv' AND (conversation_id = ?2 OR ?2 IS NULL))
                 )
               ORDER BY scope ASC, updated_at DESC";
    let mut stmt = conn.prepare(sql)?;
    let mapped = stmt.query_map(params![project_id, conversation_id], |r| {
        Ok(Memory {
            id: r.get(0)?,
            scope: r.get(1)?,
            project_id: r.get(2)?,
            conversation_id: r.get(3)?,
            content: r.get(4)?,
            source: r.get(5)?,
            approved: r.get::<_, i64>(6)? != 0,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in mapped { out.push(row?); }
    Ok(out)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Memory> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, scope, project_id, conversation_id, content, source, approved, created_at, updated_at
         FROM memories WHERE id = ?1",
        params![id],
        |r| Ok(Memory {
            id: r.get(0)?,
            scope: r.get(1)?,
            project_id: r.get(2)?,
            conversation_id: r.get(3)?,
            content: r.get(4)?,
            source: r.get(5)?,
            approved: r.get::<_, i64>(6)? != 0,
            created_at: r.get(7)?,
            updated_at: r.get(8)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("memory {id}")),
        other => AppError::Db(other),
    })
}

pub fn create(
    pool: &SqlitePool,
    scope: MemoryScope,
    project_id: Option<&str>,
    conversation_id: Option<&str>,
    content: &str,
    source: Option<&str>,
    approved: bool,
) -> AppResult<Memory> {
    let now = chrono::Utc::now().timestamp_millis();
    let m = Memory {
        id: new_id(),
        scope: scope.as_str().into(),
        project_id: project_id.map(|s| s.to_string()),
        conversation_id: conversation_id.map(|s| s.to_string()),
        content: content.to_string(),
        source: source.map(|s| s.to_string()),
        approved,
        created_at: now,
        updated_at: now,
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO memories (id, scope, project_id, conversation_id, content, source, approved, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![m.id, m.scope, m.project_id, m.conversation_id, m.content, m.source, m.approved as i64, m.created_at, m.updated_at],
    )?;
    Ok(m)
}

pub fn update(
    pool: &SqlitePool,
    id: &str,
    scope: Option<MemoryScope>,
    content: Option<&str>,
    project_id: Option<Option<&str>>,
    approved: Option<bool>,
) -> AppResult<Memory> {
    let now = chrono::Utc::now().timestamp_millis();
    let scope_str = scope.map(|s| s.as_str().to_string());
    let project_inner = match project_id { Some(v) => v.map(|s| s.to_string()), None => None };
    let approved_int = approved.map(|b| b as i64);
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE memories SET
            scope      = COALESCE(?1, scope),
            content    = COALESCE(?2, content),
            project_id = ?3,
            approved   = COALESCE(?4, approved),
            updated_at = ?5
         WHERE id = ?6",
        params![scope_str, content, project_inner, approved_int, now, id],
    )?;
    get(pool, id)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("memory {id}"))); }
    Ok(())
}

pub fn approve(pool: &SqlitePool, id: &str) -> AppResult<Memory> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE memories SET approved = 1, updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("memory {id}"))); }
    get(pool, id)
}
