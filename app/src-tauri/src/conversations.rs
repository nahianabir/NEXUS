//! Conversation and message storage. Schema lives in migration 0003.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub model_profile_id: Option<String>,
    pub system_prompt: Option<String>,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub status: String,
    pub token_count: Option<i64>,
    pub latency_ms: Option<i64>,
    pub model_profile_id: Option<String>,
    pub created_at: i64,
}

pub fn list(pool: &SqlitePool, archived: bool) -> AppResult<Vec<Conversation>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, project_id, title, model_profile_id, system_prompt, archived, created_at, updated_at
         FROM conversations WHERE archived = ?1 ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map(params![archived as i64], |r| {
        Ok(Conversation {
            id: r.get(0)?,
            project_id: r.get(1)?,
            title: r.get(2)?,
            model_profile_id: r.get(3)?,
            system_prompt: r.get(4)?,
            archived: r.get::<_, i64>(5)? != 0,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Conversation> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, project_id, title, model_profile_id, system_prompt, archived, created_at, updated_at
         FROM conversations WHERE id = ?1",
        params![id],
        |r| {
            Ok(Conversation {
                id: r.get(0)?,
                project_id: r.get(1)?,
                title: r.get(2)?,
                model_profile_id: r.get(3)?,
                system_prompt: r.get(4)?,
                archived: r.get::<_, i64>(5)? != 0,
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("conversation {id}")))
}

pub fn create(pool: &SqlitePool, title: Option<String>, system_prompt: Option<String>, model_profile_id: Option<String>) -> AppResult<Conversation> {
    let now = chrono::Utc::now().timestamp_millis();
    let c = Conversation {
        id: Ulid::new().to_string(),
        project_id: None,
        title: title.unwrap_or_else(|| "New conversation".into()),
        model_profile_id,
        system_prompt,
        archived: false,
        created_at: now,
        updated_at: now,
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO conversations (id, project_id, title, model_profile_id, system_prompt, archived, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![c.id, c.project_id, c.title, c.model_profile_id, c.system_prompt, c.archived as i64, c.created_at, c.updated_at],
    )?;
    Ok(c)
}

pub fn rename(pool: &SqlitePool, id: &str, title: &str) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
        params![title, now, id],
    )?;
    Ok(())
}

pub fn set_archived(pool: &SqlitePool, id: &str, archived: bool) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE conversations SET archived = ?1, updated_at = ?2 WHERE id = ?3",
        params![archived as i64, now, id],
    )?;
    Ok(())
}

pub fn set_system_prompt(pool: &SqlitePool, id: &str, system_prompt: Option<&str>) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE conversations SET system_prompt = ?1, updated_at = ?2 WHERE id = ?3",
        params![system_prompt, now, id],
    )?;
    Ok(())
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn touch(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("UPDATE conversations SET updated_at = ?1 WHERE id = ?2", params![now, id])?;
    Ok(())
}

pub fn set_project(pool: &SqlitePool, id: &str, project_id: Option<&str>) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE conversations SET project_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![project_id, now, id],
    )?;
    Ok(())
}

pub fn add_message(pool: &SqlitePool, m: &Message) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, status, token_count, latency_ms, model_profile_id, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![m.id, m.conversation_id, m.role, m.content, m.status, m.token_count, m.latency_ms, m.model_profile_id, m.created_at],
    )?;
    Ok(())
}

pub fn list_messages(pool: &SqlitePool, conversation_id: &str, limit: i64) -> AppResult<Vec<Message>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, status, token_count, latency_ms, model_profile_id, created_at
         FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![conversation_id, limit], |r| {
        Ok(Message {
            id: r.get(0)?,
            conversation_id: r.get(1)?,
            role: r.get(2)?,
            content: r.get(3)?,
            status: r.get(4)?,
            token_count: r.get(5)?,
            latency_ms: r.get(6)?,
            model_profile_id: r.get(7)?,
            created_at: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn update_message(pool: &SqlitePool, id: &str, content: &str, status: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE messages SET content = ?1, status = ?2 WHERE id = ?3",
        params![content, status, id],
    )?;
    Ok(())
}

pub fn new_id() -> String { Ulid::new().to_string() }
