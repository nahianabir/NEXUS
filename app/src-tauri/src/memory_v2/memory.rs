//! `memories_v2` CRUD. Phase 5B.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use super::{content_hash, normalize_content};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryV2 {
    pub id: String,
    pub workspace_id: Option<String>,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub category: String,
    pub content: String,
    pub content_hash: String,
    pub normalized_content: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_label: Option<String>,
    pub source_excerpt: Option<String>,
    pub event_at: Option<i64>,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
    pub verified_at: Option<i64>,
    pub superseded_by: Option<String>,
    pub deleted_at: Option<i64>,
    pub approved: bool,
    pub verification: String,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

#[derive(Debug, Default, Clone)]
pub struct MemoryFilter {
    pub project_id: Option<String>,
    pub workspace_id: Option<String>,
    pub category: Option<String>,
    pub approved_only: bool,
    pub include_superseded: bool,
    pub include_deleted: bool,
    pub source_kind: Option<String>,
    pub limit: Option<i64>,
}

pub fn insert(pool: &SqlitePool, m: &MemoryV2) -> AppResult<()> {
    if m.content.trim().is_empty() {
        return Err(AppError::Invalid("memory content is empty".into()));
    }
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO memories_v2 (
            id, workspace_id, project_id, conversation_id, category, content,
            content_hash, normalized_content, source_kind, source_id, source_label,
            source_excerpt, event_at, valid_from, valid_until, verified_at,
            superseded_by, deleted_at, approved, verification, pinned, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
            ?17, ?18, ?19, ?20, ?21, ?22, ?23
         )",
        params![
            m.id, m.workspace_id, m.project_id, m.conversation_id, m.category, m.content,
            m.content_hash, m.normalized_content, m.source_kind, m.source_id, m.source_label,
            m.source_excerpt, m.event_at, m.valid_from, m.valid_until, m.verified_at,
            m.superseded_by, m.deleted_at, m.approved as i64, m.verification, m.pinned as i64,
            m.created_at, m.updated_at,
        ],
    )?;
    // Mirror to FTS5 (only the live, non-deleted content).
    if m.deleted_at.is_none() {
        tx.execute(
            "INSERT INTO memories_v2_fts (memory_id, content, normalized_content, category)
             VALUES (?1, ?2, ?3, ?4)",
            params![m.id, m.content, m.normalized_content, m.category],
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<MemoryV2> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, workspace_id, project_id, conversation_id, category, content,
                content_hash, normalized_content, source_kind, source_id, source_label,
                source_excerpt, event_at, valid_from, valid_until, verified_at,
                superseded_by, deleted_at, approved, verification, pinned, created_at, updated_at
         FROM memories_v2 WHERE id = ?1",
        params![id],
        row_to_memory,
    )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("memory {id}")),
            other => AppError::Db(other),
        })
}

fn row_to_memory(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryV2> {
    Ok(MemoryV2 {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        project_id: r.get(2)?,
        conversation_id: r.get(3)?,
        category: r.get(4)?,
        content: r.get(5)?,
        content_hash: r.get(6)?,
        normalized_content: r.get(7)?,
        source_kind: r.get(8)?,
        source_id: r.get(9)?,
        source_label: r.get(10)?,
        source_excerpt: r.get(11)?,
        event_at: r.get(12)?,
        valid_from: r.get(13)?,
        valid_until: r.get(14)?,
        verified_at: r.get(15)?,
        superseded_by: r.get(16)?,
        deleted_at: r.get(17)?,
        approved: r.get::<_, i64>(18)? != 0,
        verification: r.get(19)?,
        pinned: r.get::<_, i64>(20)? != 0,
        created_at: r.get(21)?,
        updated_at: r.get(22)?,
    })
}

pub fn list(pool: &SqlitePool, filter: &MemoryFilter) -> AppResult<Vec<MemoryV2>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from(
        "SELECT id, workspace_id, project_id, conversation_id, category, content,
                content_hash, normalized_content, source_kind, source_id, source_label,
                source_excerpt, event_at, valid_from, valid_until, verified_at,
                superseded_by, deleted_at, approved, verification, pinned, created_at, updated_at
         FROM memories_v2 WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(p) = &filter.project_id {
        sql.push_str(" AND project_id = ?"); args.push(Box::new(p.clone()));
    }
    if let Some(w) = &filter.workspace_id {
        sql.push_str(" AND workspace_id = ?"); args.push(Box::new(w.clone()));
    }
    if let Some(c) = &filter.category {
        sql.push_str(" AND category = ?"); args.push(Box::new(c.clone()));
    }
    if let Some(s) = &filter.source_kind {
        sql.push_str(" AND source_kind = ?"); args.push(Box::new(s.clone()));
    }
    if filter.approved_only { sql.push_str(" AND approved = 1"); }
    if !filter.include_superseded { sql.push_str(" AND superseded_by IS NULL"); }
    if !filter.include_deleted { sql.push_str(" AND deleted_at IS NULL"); }
    sql.push_str(" ORDER BY pinned DESC, updated_at DESC");
    let lim = filter.limit.unwrap_or(200).clamp(1, 1000);
    sql.push_str(" LIMIT ?"); args.push(Box::new(lim));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), row_to_memory)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn list_for_context(
    pool: &SqlitePool,
    project_id: Option<&str>,
    conversation_id: Option<&str>,
    limit: usize,
) -> AppResult<Vec<MemoryV2>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let sql = "SELECT id, workspace_id, project_id, conversation_id, category, content,
                      content_hash, normalized_content, source_kind, source_id, source_label,
                      source_excerpt, event_at, valid_from, valid_until, verified_at,
                      superseded_by, deleted_at, approved, verification, pinned, created_at, updated_at
               FROM memories_v2
               WHERE approved = 1 AND deleted_at IS NULL AND superseded_by IS NULL
                 AND (project_id = ?1 OR ?1 IS NULL)
                 AND (conversation_id = ?2 OR ?2 IS NULL)
               ORDER BY pinned DESC, verified_at DESC, updated_at DESC
               LIMIT ?3";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![project_id, conversation_id, limit as i64], row_to_memory)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn approve(pool: &SqlitePool, id: &str) -> AppResult<MemoryV2> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE memories_v2 SET approved = 1, verification = 'approved', verified_at = COALESCE(verified_at, ?1), updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("memory {id}"))); }
    get(pool, id)
}

pub fn update(
    pool: &SqlitePool,
    id: &str,
    content: Option<&str>,
    category: Option<&str>,
    pinned: Option<bool>,
    valid_from: Option<Option<i64>>,
    valid_until: Option<Option<i64>>,
    event_at: Option<Option<i64>>,
    verified_at: Option<i64>,
) -> AppResult<MemoryV2> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let tx = conn.unchecked_transaction()?;
    let new_content;
    let new_norm;
    let new_hash;
    if let Some(c) = content {
        new_content = c.to_string();
        new_norm = normalize_content(c);
        new_hash = content_hash(&new_norm);
    } else {
        let existing = get(pool, id)?;
        new_content = existing.content;
        new_norm = existing.normalized_content;
        new_hash = existing.content_hash;
    }
    let pinned_inner = match pinned { Some(p) => Some(p as i64), None => None };
    let vf = match valid_from { Some(v) => v, None => None };
    let vu = match valid_until { Some(v) => v, None => None };
    let ea = match event_at { Some(v) => v, None => None };
    let n = tx.execute(
        "UPDATE memories_v2 SET
            content           = COALESCE(?1, content),
            normalized_content = ?2,
            content_hash      = ?3,
            category          = COALESCE(?4, category),
            pinned            = COALESCE(?5, pinned),
            valid_from        = ?6,
            valid_until       = ?7,
            event_at          = ?8,
            verified_at       = COALESCE(?9, verified_at),
            updated_at        = ?10
         WHERE id = ?11",
        params![
            content.map(|s| s.to_string()),
            new_norm,
            new_hash,
            category.map(|s| s.to_string()),
            pinned_inner,
            vf,
            vu,
            ea,
            verified_at,
            now,
            id,
        ],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("memory {id}"))); }
    // Refresh FTS row.
    tx.execute("DELETE FROM memories_v2_fts WHERE memory_id = ?1", params![id])?;
    tx.execute(
        "INSERT INTO memories_v2_fts (memory_id, content, normalized_content, category)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, new_content, new_norm, category.unwrap_or("")],
    )?;
    tx.commit()?;
    get(pool, id)
}

pub fn supersede(pool: &SqlitePool, old_id: &str, new_id: &str) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE memories_v2 SET superseded_by = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_id, now, old_id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("memory {old_id}"))); }
    Ok(())
}

pub fn soft_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE memories_v2 SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("memory {id}"))); }
    conn.execute("DELETE FROM memories_v2_fts WHERE memory_id = ?1", params![id])?;
    Ok(())
}

pub fn hard_delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM memories_v2 WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("memory {id}"))); }
    Ok(())
}

/// Find the canonical row for a content hash. If a row exists already,
/// the caller should re-use the existing id rather than creating a new
/// one (dedupe).
pub fn find_by_hash(pool: &SqlitePool, hash: &str) -> AppResult<Option<MemoryV2>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, workspace_id, project_id, conversation_id, category, content,
                content_hash, normalized_content, source_kind, source_id, source_label,
                source_excerpt, event_at, valid_from, valid_until, verified_at,
                superseded_by, deleted_at, approved, verification, pinned, created_at, updated_at
         FROM memories_v2 WHERE content_hash = ?1 AND deleted_at IS NULL
         ORDER BY created_at ASC LIMIT 1",
    )?;
    let mut rows = stmt.query(params![hash])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_memory(row)?))
    } else { Ok(None) }
}

/// Migration helper: pull every v1 row that has not been migrated, write
/// it into v2 with category='lesson' and verification='approved', and
/// mark it migrated. Idempotent: each v1 row is touched at most once.
pub fn migrate_from_v1(pool: &SqlitePool) -> AppResult<usize> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, project_id, conversation_id, content, source, approved, created_at
         FROM memories",
    )?;
    let rows: Vec<(String, Option<String>, Option<String>, String, Option<String>, i64, i64)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            v.push((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                row.get(5)?, row.get(6)?,
            ));
        }
        v
    };
    let mut count = 0;
    for (id, project_id, conversation_id, content, source, approved, created_at) in rows {
        let norm = normalize_content(&content);
        let hash = content_hash(&norm);
        // Skip if a v2 row with the same hash already exists.
        if find_by_hash(pool, &hash)?.is_some() { continue; }
        let now = chrono::Utc::now().timestamp_millis();
        let m = MemoryV2 {
            id: id.clone(),
            workspace_id: None,
            project_id: project_id.clone(),
            conversation_id: conversation_id.clone(),
            category: "lesson".into(),
            content: content.clone(),
            content_hash: hash,
            normalized_content: norm,
            source_kind: source.clone().unwrap_or_else(|| "user".into()),
            source_id: None,
            source_label: source,
            source_excerpt: None,
            event_at: None,
            valid_from: None,
            valid_until: None,
            verified_at: if approved != 0 { Some(created_at) } else { None },
            superseded_by: None,
            deleted_at: None,
            approved: approved != 0,
            verification: if approved != 0 { "approved".into() } else { "user_stated".into() },
            pinned: false,
            created_at,
            updated_at: now,
        };
        insert(pool, &m)?;
        count += 1;
    }
    Ok(count)
}
