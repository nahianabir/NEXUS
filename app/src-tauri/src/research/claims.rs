//! Paper claim CRUD.

use rusqlite::params;

use super::{now_ms, row_to_claim, PaperClaim};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn add(pool: &SqlitePool, c: &PaperClaim) -> AppResult<PaperClaim> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut c = c.clone();
    if c.id.is_empty() { c.id = super::new_id(); }
    if c.created_at == 0 { c.created_at = now; }
    if c.category.is_empty() { c.category = "finding".into(); }
    if c.confidence < 0.0 || c.confidence > 1.0 { c.confidence = 0.5; }
    conn.execute(
        "INSERT INTO paper_claims (id, research_project_id, paper_id, document_id, claim, category, evidence, confidence, kg_entity_id, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![c.id, c.research_project_id, c.paper_id, c.document_id, c.claim, c.category, c.evidence, c.confidence, c.kg_entity_id, c.created_at],
    )?;
    super::touch_project(pool, &c.research_project_id)?;
    Ok(c)
}

pub fn update(pool: &SqlitePool, c: &PaperClaim) -> AppResult<PaperClaim> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE paper_claims SET paper_id=?1, document_id=?2, claim=?3, category=?4, evidence=?5, confidence=?6, kg_entity_id=?7 WHERE id=?8",
        params![c.paper_id, c.document_id, c.claim, c.category, c.evidence, c.confidence, c.kg_entity_id, c.id],
    )?;
    super::touch_project(pool, &c.research_project_id)?;
    get(pool, &c.id)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<PaperClaim> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, research_project_id, paper_id, document_id, claim, category, evidence, confidence, kg_entity_id, created_at
         FROM paper_claims WHERE id = ?1",
        params![id],
        row_to_claim,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("claim {id}")),
        other => AppError::Db(other),
    })
}

pub fn list_for(pool: &SqlitePool, project_id: &str, category: Option<&str>, paper_id: Option<&str>) -> AppResult<Vec<PaperClaim>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from("SELECT id, research_project_id, paper_id, document_id, claim, category, evidence, confidence, kg_entity_id, created_at FROM paper_claims WHERE research_project_id = ?1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    args.push(Box::new(project_id.to_string()));
    if let Some(cat) = category {
        sql.push_str(" AND category = ?");
        args.push(Box::new(cat.to_string()));
    }
    if let Some(pid) = paper_id {
        sql.push_str(" AND paper_id = ?");
        args.push(Box::new(pid.to_string()));
    }
    sql.push_str(" ORDER BY category, created_at");
    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref() as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(params_refs), row_to_claim)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let project_id: String = conn.query_row("SELECT research_project_id FROM paper_claims WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("claim {id}")),
            other => AppError::Db(other),
        })?;
    conn.execute("DELETE FROM paper_claims WHERE id = ?1", params![id])?;
    super::touch_project(pool, &project_id)?;
    Ok(())
}
