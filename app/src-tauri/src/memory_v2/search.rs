//! FTS5 search over memory content. Returns `(memory_id, score)` pairs
//! where higher score = better match. We use bm25 (lower is better) and
//! invert to keep the convention used by the Knowledge Vault search.

use rusqlite::params;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn search_fts(pool: &SqlitePool, query: &str, limit: usize) -> AppResult<Vec<(String, f64)>> {
    if query.trim().is_empty() { return Ok(Vec::new()); }
    let escaped: Vec<String> = query.split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| {
            let cleaned: String = t.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
            if cleaned.is_empty() { String::new() } else { format!("\"{}\"*", cleaned) }
        })
        .filter(|s| !s.is_empty())
        .collect();
    if escaped.is_empty() { return Ok(Vec::new()); }
    // OR the terms so partial matches are not lost. The bm25 score
    // already rewards documents that contain more of the query.
    let q = escaped.join(" OR ");

    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT memory_id, bm25(memories_v2_fts) AS score
         FROM memories_v2_fts
         WHERE memories_v2_fts MATCH ?1
         ORDER BY score LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![q, limit as i64], |r| {
        let raw: f64 = r.get(1)?;
        Ok((r.get::<_, String>(0)?, -raw))
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

/// Combined search: keyword (FTS5) + category filter. Returns the
/// matching memory ids in relevance order, deduplicated.
pub fn search(
    pool: &SqlitePool,
    query: &str,
    category: Option<&str>,
    project_id: Option<&str>,
    limit: usize,
) -> AppResult<Vec<String>> {
    let ids: Vec<String> = if query.trim().is_empty() {
        // No keyword: return recent, filtered.
        let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
        let mut sql = String::from(
            "SELECT id FROM memories_v2
             WHERE deleted_at IS NULL AND superseded_by IS NULL AND approved = 1",
        );
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(c) = category {
            sql.push_str(" AND category = ?"); args.push(Box::new(c.to_string()));
        }
        if let Some(p) = project_id {
            sql.push_str(" AND project_id = ?"); args.push(Box::new(p.to_string()));
        }
        sql.push_str(" ORDER BY verified_at DESC, updated_at DESC LIMIT ?");
        args.push(Box::new(limit as i64));
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows { out.push(row?); }
        out
    } else {
        let hits = search_fts(pool, query, limit * 2)?;
        hits.into_iter().map(|(id, _)| id).collect()
    };
    // Apply the project / category filter and keep `limit` rows.
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from(
        "SELECT id FROM memories_v2
         WHERE id IN (SELECT value FROM json_each(?1))
           AND deleted_at IS NULL AND superseded_by IS NULL AND approved = 1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(serde_json::to_string(&ids).unwrap_or_else(|_| "[]".into()))];
    if let Some(c) = category {
        sql.push_str(" AND category = ?"); args.push(Box::new(c.to_string()));
    }
    if let Some(p) = project_id {
        sql.push_str(" AND project_id = ?"); args.push(Box::new(p.to_string()));
    }
    sql.push_str(" LIMIT ?"); args.push(Box::new(limit as i64));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}
