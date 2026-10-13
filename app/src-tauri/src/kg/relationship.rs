//! Relationship CRUD. Phase D.

use rusqlite::{params, OptionalExtension};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use super::evidence;
use super::graph;
use super::types::Relationship;

pub fn new_id() -> String { Ulid::new().to_string() }

fn row_to_rel(r: &rusqlite::Row<'_>) -> rusqlite::Result<Relationship> {
    Ok(Relationship {
        id: r.get(0)?,
        source_entity_id: r.get(1)?,
        relationship_type: r.get(2)?,
        target_entity_id: r.get(3)?,
        valid_from: r.get(4)?,
        valid_until: r.get(5)?,
        created_at: r.get(6)?,
        verification_status: r.get(7)?,
        weight: r.get(8)?,
        metadata_json: r.get(9)?,
    })
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Relationship> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, source_entity_id, relationship_type, target_entity_id, valid_from, valid_until, created_at, verification_status, weight, metadata_json
         FROM kg_relationships WHERE id = ?1",
        params![id],
        row_to_rel,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("relationship {id}")),
        other => AppError::Db(other),
    })
}

#[derive(Debug, Clone, Default)]
pub struct CreateRelationship {
    pub source_entity_id: String,
    pub relationship_type: String,
    pub target_entity_id: String,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
    pub verification_status: Option<String>,
    pub weight: Option<f64>,
    pub metadata_json: Option<String>,
    pub evidence: Vec<evidence::CreateEvidence>,
}

pub fn create(pool: &SqlitePool, req: &CreateRelationship) -> AppResult<Relationship> {
    if req.source_entity_id == req.target_entity_id {
        return Err(AppError::Invalid("self-referential relationships are not allowed".into()));
    }
    if req.relationship_type.trim().is_empty() {
        return Err(AppError::Invalid("relationship_type required".into()));
    }
    // Validate that both entities exist.
    let _ = graph::get(pool, &req.source_entity_id)?;
    let _ = graph::get(pool, &req.target_entity_id)?;
    let now = chrono::Utc::now().timestamp_millis();
    let id = new_id();
    let status = req.verification_status.clone().unwrap_or_else(|| "inferred".into());
    let weight = req.weight.unwrap_or(1.0);
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let tx = conn.unchecked_transaction()?;
    // Upsert: if the same (source, type, target) exists, update weight + status.
    let existing: Option<String> = tx.query_row(
        "SELECT id FROM kg_relationships WHERE source_entity_id = ?1 AND relationship_type = ?2 AND target_entity_id = ?3",
        params![req.source_entity_id, req.relationship_type, req.target_entity_id],
        |r| r.get(0),
    ).optional().map_err(AppError::Db)?;
    let rel_id = match existing {
        Some(eid) => {
            tx.execute(
                "UPDATE kg_relationships SET weight = MAX(weight, ?1), verification_status = ?2, valid_from = COALESCE(?3, valid_from), valid_until = COALESCE(?4, valid_until), metadata_json = COALESCE(?5, metadata_json) WHERE id = ?6",
                params![weight, status, req.valid_from, req.valid_until, req.metadata_json, eid],
            )?;
            eid
        }
        None => {
            tx.execute(
                "INSERT INTO kg_relationships (id, source_entity_id, relationship_type, target_entity_id, valid_from, valid_until, created_at, updated_at, verification_status, weight, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9, ?10)",
                params![id, req.source_entity_id, req.relationship_type, req.target_entity_id, req.valid_from, req.valid_until, now, status, weight, req.metadata_json],
            )?;
            id
        }
    };
    for ev in &req.evidence {
        evidence::add_tx(&tx, &rel_id, ev)?;
    }
    tx.commit()?;
    get(pool, &rel_id)
}

pub fn set_status(pool: &SqlitePool, id: &str, status: &str) -> AppResult<Relationship> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE kg_relationships SET verification_status = ?1 WHERE id = ?2",
        params![status, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("relationship {id}"))); }
    get(pool, id)
}

pub fn list_for(pool: &SqlitePool, entity_id: &str, direction: &str, limit: usize) -> AppResult<Vec<Relationship>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let (col, order_col) = match direction {
        "out" => ("source_entity_id", "source_entity_id"),
        "in" => ("target_entity_id", "target_entity_id"),
        _ => return Err(AppError::Invalid("direction must be 'in' or 'out'".into())),
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT id, source_entity_id, relationship_type, target_entity_id, valid_from, valid_until, created_at, verification_status, weight, metadata_json
         FROM kg_relationships WHERE {col} = ?1 ORDER BY weight DESC, created_at DESC LIMIT ?2"
    ))?;
    let _ = order_col; // suppress
    let rows = stmt.query_map(params![entity_id, limit as i64], row_to_rel)?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM kg_relationships WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("relationship {id}"))); }
    Ok(())
}
