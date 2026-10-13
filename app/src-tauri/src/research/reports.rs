//! Research reports: Markdown snapshots of a project's evidence.

use rusqlite::params;

use super::{now_ms, ResearchReport};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

fn parse_json_array(s: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(s).unwrap_or_default()
}

pub fn save(pool: &SqlitePool, r: &ResearchReport) -> AppResult<ResearchReport> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut r = r.clone();
    if r.id.is_empty() { r.id = super::new_id(); }
    if r.created_at == 0 { r.created_at = now; }
    r.updated_at = now;
    let paper_ids_json = serde_json::to_string(&r.paper_ids).unwrap_or_else(|_| "[]".into());
    let claim_ids_json = serde_json::to_string(&r.claim_ids).unwrap_or_else(|_| "[]".into());
    let existing: Option<String> = conn.query_row(
        "SELECT id FROM research_reports WHERE id = ?1",
        params![r.id],
        |r| r.get(0),
    ).ok();
    if let Some(id) = existing {
        r.id = id;
        conn.execute(
            "UPDATE research_reports SET title=?1, body_md=?2, paper_ids=?3, claim_ids=?4, updated_at=?5 WHERE id=?6",
            params![r.title, r.body_md, paper_ids_json, claim_ids_json, r.updated_at, r.id],
        )?;
    } else {
        conn.execute(
            "INSERT INTO research_reports (id, research_project_id, title, body_md, paper_ids, claim_ids, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![r.id, r.research_project_id, r.title, r.body_md, paper_ids_json, claim_ids_json, r.created_at, r.updated_at],
        )?;
    }
    super::touch_project(pool, &r.research_project_id)?;
    get(pool, &r.id)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<ResearchReport> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, research_project_id, title, body_md, paper_ids, claim_ids, created_at, updated_at
         FROM research_reports WHERE id = ?1",
        params![id],
        |r| {
            let paper_ids: String = r.get(4)?;
            let claim_ids: String = r.get(5)?;
            Ok(ResearchReport {
                id: r.get(0)?,
                research_project_id: r.get(1)?,
                title: r.get(2)?,
                body_md: r.get(3)?,
                paper_ids: parse_json_array(&paper_ids),
                claim_ids: parse_json_array(&claim_ids),
                created_at: r.get(6)?,
                updated_at: r.get(7)?,
            })
        },
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("report {id}")),
        other => AppError::Db(other),
    })
}

pub fn list_for(pool: &SqlitePool, project_id: &str) -> AppResult<Vec<ResearchReport>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, research_project_id, title, body_md, paper_ids, claim_ids, created_at, updated_at
         FROM research_reports WHERE research_project_id = ?1 ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map(params![project_id], |r| {
        let paper_ids: String = r.get(4)?;
        let claim_ids: String = r.get(5)?;
        Ok(ResearchReport {
            id: r.get(0)?,
            research_project_id: r.get(1)?,
            title: r.get(2)?,
            body_md: r.get(3)?,
            paper_ids: parse_json_array(&paper_ids),
            claim_ids: parse_json_array(&claim_ids),
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let project_id: String = conn.query_row("SELECT research_project_id FROM research_reports WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("report {id}")),
            other => AppError::Db(other),
        })?;
    conn.execute("DELETE FROM research_reports WHERE id = ?1", params![id])?;
    super::touch_project(pool, &project_id)?;
    Ok(())
}
