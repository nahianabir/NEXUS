//! `memory_candidates` — auto-extracted memories pending user review.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use super::memory::MemoryV2;
use super::{content_hash, normalize_content};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub category: String,
    pub content: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_excerpt: Option<String>,
    pub confidence: f64,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub created_at: i64,
    pub decided_at: Option<i64>,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn propose(
    pool: &SqlitePool,
    category: &str,
    content: &str,
    source_kind: &str,
    source_id: Option<&str>,
    source_excerpt: Option<&str>,
    confidence: f64,
    project_id: Option<&str>,
    conversation_id: Option<&str>,
) -> AppResult<Candidate> {
    let now = chrono::Utc::now().timestamp_millis();
    let c = Candidate {
        id: new_id(),
        category: category.to_string(),
        content: content.to_string(),
        source_kind: source_kind.to_string(),
        source_id: source_id.map(|s| s.to_string()),
        source_excerpt: source_excerpt.map(|s| s.to_string()),
        confidence: confidence.clamp(0.0, 1.0),
        status: "pending".into(),
        rejection_reason: None,
        project_id: project_id.map(|s| s.to_string()),
        conversation_id: conversation_id.map(|s| s.to_string()),
        created_at: now,
        decided_at: None,
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO memory_candidates
         (id, category, content, source_kind, source_id, source_excerpt, confidence, status, project_id, conversation_id, created_at, decided_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,NULL)",
        params![c.id, c.category, c.content, c.source_kind, c.source_id, c.source_excerpt,
                c.confidence, c.status, c.project_id, c.conversation_id, c.created_at],
    )?;
    Ok(c)
}

pub fn list(pool: &SqlitePool, status: &str, limit: i64) -> AppResult<Vec<Candidate>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, category, content, source_kind, source_id, source_excerpt, confidence, status,
                rejection_reason, project_id, conversation_id, created_at, decided_at
         FROM memory_candidates WHERE status = ?1 ORDER BY confidence DESC, created_at ASC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![status, limit], |r| {
        Ok(Candidate {
            id: r.get(0)?,
            category: r.get(1)?,
            content: r.get(2)?,
            source_kind: r.get(3)?,
            source_id: r.get(4)?,
            source_excerpt: r.get(5)?,
            confidence: r.get(6)?,
            status: r.get(7)?,
            rejection_reason: r.get(8)?,
            project_id: r.get(9)?,
            conversation_id: r.get(10)?,
            created_at: r.get(11)?,
            decided_at: r.get(12)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn approve(pool: &SqlitePool, id: &str) -> AppResult<MemoryV2> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = chrono::Utc::now().timestamp_millis();
    let candidate: Candidate = conn.query_row(
        "SELECT id, category, content, source_kind, source_id, source_excerpt, confidence, status,
                rejection_reason, project_id, conversation_id, created_at, decided_at
         FROM memory_candidates WHERE id = ?1",
        params![id],
        |r| Ok(Candidate {
            id: r.get(0)?, category: r.get(1)?, content: r.get(2)?, source_kind: r.get(3)?,
            source_id: r.get(4)?, source_excerpt: r.get(5)?, confidence: r.get(6)?,
            status: r.get(7)?, rejection_reason: r.get(8)?, project_id: r.get(9)?,
            conversation_id: r.get(10)?, created_at: r.get(11)?, decided_at: r.get(12)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("candidate {id}")),
        other => AppError::Db(other),
    })?;

    let norm = normalize_content(&candidate.content);
    let hash = content_hash(&norm);
    if let Some(existing) = super::memory::find_by_hash(pool, &hash)? {
        // Dedupe: keep the existing approved row.
        let n = conn.execute(
            "UPDATE memory_candidates SET status = 'approved', decided_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        if n == 0 { return Err(AppError::NotFound(format!("candidate {id}"))); }
        return Ok(existing);
    }

    let m = MemoryV2 {
        id: super::memory::new_id(),
        workspace_id: None,
        project_id: candidate.project_id.clone(),
        conversation_id: candidate.conversation_id.clone(),
        category: candidate.category.clone(),
        content: candidate.content.clone(),
        content_hash: hash,
        normalized_content: norm,
        source_kind: candidate.source_kind.clone(),
        source_id: candidate.source_id.clone(),
        source_label: None,
        source_excerpt: candidate.source_excerpt.clone(),
        event_at: None,
        valid_from: None,
        valid_until: None,
        verified_at: Some(now),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "approved".into(),
        pinned: false,
        created_at: now,
        updated_at: now,
    };
    super::memory::insert(pool, &m)?;
    let n = conn.execute(
        "UPDATE memory_candidates SET status = 'approved', decided_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("candidate {id}"))); }
    Ok(m)
}

pub fn reject(pool: &SqlitePool, id: &str, reason: Option<&str>) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE memory_candidates SET status = 'rejected', rejection_reason = ?1, decided_at = ?2 WHERE id = ?3",
        params![reason, now, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("candidate {id}"))); }
    Ok(())
}
