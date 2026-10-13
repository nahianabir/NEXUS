//! Entity CRUD + merge support. Phase D.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use super::normalize::normalize_name;
use super::types::Entity;

pub fn new_id() -> String { Ulid::new().to_string() }

fn row_to_entity(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entity> {
    Ok(Entity {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        entity_type: r.get(2)?,
        canonical_name: r.get(3)?,
        description: r.get(4)?,
        metadata_json: r.get(5)?,
        merged_into: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
    })
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Entity> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, workspace_id, entity_type, canonical_name, description, metadata_json, merged_into, created_at, updated_at
         FROM kg_entities WHERE id = ?1",
        params![id],
        row_to_entity,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("entity {id}")),
        other => AppError::Db(other),
    })
}

pub fn find_by_normalized(pool: &SqlitePool, name: &str, entity_type: &str) -> AppResult<Option<Entity>> {
    let n = normalize_name(name);
    if n.is_empty() { return Ok(None); }
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT e.id, e.workspace_id, e.entity_type, e.canonical_name, e.description, e.metadata_json, e.merged_into, e.created_at, e.updated_at
         FROM kg_entities e
         LEFT JOIN kg_aliases a ON a.entity_id = e.id
         WHERE e.merged_into IS NULL AND e.entity_type = ?1
           AND (a.normalized_alias = ?2 OR LOWER(e.canonical_name) = ?2)
         ORDER BY e.created_at ASC LIMIT 1",
    )?;
    let mut rows = stmt.query(params![entity_type, n])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_entity(row)?))
    } else { Ok(None) }
}

#[derive(Debug, Clone, Default)]
pub struct CreateEntity {
    pub workspace_id: Option<String>,
    pub entity_type: String,
    pub canonical_name: String,
    pub description: Option<String>,
    pub metadata_json: Option<String>,
    pub aliases: Vec<String>,
}

pub fn create(pool: &SqlitePool, req: &CreateEntity) -> AppResult<Entity> {
    if req.entity_type.trim().is_empty() { return Err(AppError::Invalid("entity_type required".into())); }
    if req.canonical_name.trim().is_empty() { return Err(AppError::Invalid("canonical_name required".into())); }
    if let Some(existing) = find_by_normalized(pool, &req.canonical_name, &req.entity_type)? {
        return Ok(existing);
    }
    let now = chrono::Utc::now().timestamp_millis();
    let e = Entity {
        id: new_id(),
        workspace_id: req.workspace_id.clone(),
        entity_type: req.entity_type.clone(),
        canonical_name: req.canonical_name.clone(),
        description: req.description.clone(),
        metadata_json: req.metadata_json.clone(),
        merged_into: None,
        created_at: now,
        updated_at: now,
    };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO kg_entities (id, workspace_id, entity_type, canonical_name, description, metadata_json, merged_into, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
        params![e.id, e.workspace_id, e.entity_type, e.canonical_name, e.description, e.metadata_json, e.created_at, e.updated_at],
    )?;
    for alias in &req.aliases {
        add_alias_tx(&tx, &e.id, alias)?;
    }
    // The canonical name is also stored as a normalised alias so the
    // `find_by_normalized` lookup works through the alias index.
    add_alias_tx(&tx, &e.id, &e.canonical_name)?;
    tx.commit()?;
    Ok(e)
}

fn add_alias_tx(tx: &rusqlite::Transaction<'_>, entity_id: &str, alias: &str) -> AppResult<()> {
    let n = normalize_name(alias);
    if n.is_empty() { return Ok(()); }
    tx.execute(
        "INSERT OR IGNORE INTO kg_aliases (id, entity_id, alias, normalized_alias)
         VALUES (?1, ?2, ?3, ?4)",
        params![Ulid::new().to_string(), entity_id, alias, n],
    )?;
    Ok(())
}

pub fn add_alias(pool: &SqlitePool, entity_id: &str, alias: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut tx = conn.unchecked_transaction()?;
    add_alias_tx(&tx, entity_id, alias)?;
    tx.commit()?;
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    pub entity_type: Option<String>,
    pub workspace_id: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub include_merged: bool,
}

pub fn list(pool: &SqlitePool, filter: &ListFilter) -> AppResult<Vec<Entity>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from(
        "SELECT id, workspace_id, entity_type, canonical_name, description, metadata_json, merged_into, created_at, updated_at
         FROM kg_entities WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(t) = &filter.entity_type { sql.push_str(" AND entity_type = ?"); args.push(Box::new(t.clone())); }
    if let Some(w) = &filter.workspace_id { sql.push_str(" AND workspace_id = ?"); args.push(Box::new(w.clone())); }
    if !filter.include_merged { sql.push_str(" AND merged_into IS NULL"); }
    if let Some(s) = &filter.search {
        sql.push_str(" AND (LOWER(canonical_name) LIKE ? OR id IN (SELECT entity_id FROM kg_aliases WHERE normalized_alias LIKE ?))");
        let needle = format!("%{}%", s.to_ascii_lowercase());
        args.push(Box::new(needle.clone()));
        args.push(Box::new(needle));
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
    let lim = filter.limit.unwrap_or(500).clamp(1, 5000);
    args.push(Box::new(lim));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), row_to_entity)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub winner_id: String,
    pub loser_id: String,
    pub reason: Option<String>,
}

/// Merge `loser_id` into `winner_id`. The loser stays in the table
/// with `merged_into` pointing at the winner; the relationships on
/// the loser are migrated to the winner. Both directions are updated.
pub fn merge(pool: &SqlitePool, req: &MergeRequest) -> AppResult<Entity> {
    if req.winner_id == req.loser_id {
        return Err(AppError::Invalid("cannot merge into self".into()));
    }
    let winner = get(pool, &req.winner_id)?;
    let loser = get(pool, &req.loser_id)?;
    if loser.merged_into.is_some() {
        return Err(AppError::Invalid("loser already merged".into()));
    }
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let tx = conn.unchecked_transaction()?;
    // Move relationships: any relationship pointing at the loser now
    // points at the winner. We collapse the (source, type, target)
    // uniqueness to keep the graph clean.
    tx.execute(
        "UPDATE OR IGNORE kg_relationships SET target_entity_id = ?1, updated_at = created_at
         WHERE target_entity_id = ?2",
        params![&req.winner_id, &req.loser_id],
    )?;
    // Delete relationships that couldn't be merged because of the
    // uniqueness constraint.
    tx.execute("DELETE FROM kg_relationships WHERE target_entity_id = ?1", params![&req.loser_id])?;
    tx.execute(
        "UPDATE OR IGNORE kg_relationships SET source_entity_id = ?1
         WHERE source_entity_id = ?2",
        params![&req.winner_id, &req.loser_id],
    )?;
    tx.execute("DELETE FROM kg_relationships WHERE source_entity_id = ?1", params![&req.loser_id])?;
    // The loser's aliases move to the winner (best-effort).
    tx.execute("UPDATE OR IGNORE kg_aliases SET entity_id = ?1 WHERE entity_id = ?2", params![&req.winner_id, &req.loser_id])?;
    tx.execute("DELETE FROM kg_aliases WHERE entity_id = ?1", params![&req.loser_id])?;
    // Mark the loser as merged.
    let metadata = serde_json::json!({
        "merge": { "reason": req.reason, "at": now, "by": "user" },
    }).to_string();
    tx.execute(
        "UPDATE kg_entities SET merged_into = ?1, updated_at = ?2, metadata_json = COALESCE(metadata_json, ?3) WHERE id = ?4",
        params![&req.winner_id, now, metadata, &req.loser_id],
    )?;
    tx.commit()?;
    let _ = loser; // suppress unused
    Ok(winner)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM kg_entities WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("entity {id}"))); }
    Ok(())
}
