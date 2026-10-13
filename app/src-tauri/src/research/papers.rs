//! Research paper CRUD.

use rusqlite::params;

use super::{now_ms, row_to_paper, ResearchPaper};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn add(pool: &SqlitePool, p: &ResearchPaper) -> AppResult<ResearchPaper> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut p = p.clone();
    if p.id.is_empty() { p.id = super::new_id(); }
    if p.created_at == 0 { p.created_at = now; }
    if p.position == 0 && false {
        // Position is required to be set by the caller; do not silently
        // zero it.
    }
    conn.execute(
        "INSERT INTO research_papers (id, research_project_id, document_id, title, authors, venue, year, doi, url, notes, position, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![p.id, p.research_project_id, p.document_id, p.title, p.authors, p.venue, p.year, p.doi, p.url, p.notes, p.position, p.created_at],
    )?;
    super::touch_project(pool, &p.research_project_id)?;
    Ok(p)
}

pub fn update(pool: &SqlitePool, p: &ResearchPaper) -> AppResult<ResearchPaper> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE research_papers SET document_id=?1, title=?2, authors=?3, venue=?4, year=?5, doi=?6, url=?7, notes=?8, position=?9 WHERE id=?10",
        params![p.document_id, p.title, p.authors, p.venue, p.year, p.doi, p.url, p.notes, p.position, p.id],
    )?;
    super::touch_project(pool, &p.research_project_id)?;
    get(pool, &p.id)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<ResearchPaper> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, research_project_id, document_id, title, authors, venue, year, doi, url, notes, position, created_at
         FROM research_papers WHERE id = ?1",
        params![id],
        row_to_paper,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("research paper {id}")),
        other => AppError::Db(other),
    })
}

pub fn list_for(pool: &SqlitePool, project_id: &str) -> AppResult<Vec<ResearchPaper>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, research_project_id, document_id, title, authors, venue, year, doi, url, notes, position, created_at
         FROM research_papers WHERE research_project_id = ?1 ORDER BY position, created_at",
    )?;
    let rows = stmt.query_map(params![project_id], row_to_paper)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Look up the project id first so we can bump the project.
    let project_id: String = conn.query_row("SELECT research_project_id FROM research_papers WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("research paper {id}")),
            other => AppError::Db(other),
        })?;
    conn.execute("DELETE FROM research_papers WHERE id = ?1", params![id])?;
    super::touch_project(pool, &project_id)?;
    Ok(())
}
