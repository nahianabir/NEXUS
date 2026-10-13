//! Research Studio. Phase G.
//!
//! A research project is a workspace for a literature question. It
//! owns papers, claims extracted from those papers, comparison
//! matrices, and Markdown reports. Everything is intentionally
//! lightweight: the goal is "write down what I read and how the
//! papers compare", not a full Zotero integration.
//!
//! The split between `projects`, `papers`, `claims`, `matrices`, and
//! `reports` mirrors the four tabs of the Research Studio page.

pub mod papers;
pub mod claims;
pub mod matrices;
pub mod projects;
pub mod reports;
pub mod markdown;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn new_id() -> String { ulid::Ulid::new().to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchProject {
    pub id: String,
    pub workspace_id: Option<String>,
    pub title: String,
    pub question: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPaper {
    pub id: String,
    pub research_project_id: String,
    pub document_id: Option<String>,
    pub title: String,
    pub authors: Option<String>,
    pub venue: Option<String>,
    pub year: Option<i64>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub position: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperClaim {
    pub id: String,
    pub research_project_id: String,
    pub paper_id: Option<String>,
    pub document_id: Option<String>,
    pub claim: String,
    pub category: String,
    pub evidence: Option<String>,
    pub confidence: f64,
    pub kg_entity_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonMatrix {
    pub id: String,
    pub research_project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixColumn {
    pub id: String,
    pub matrix_id: String,
    pub label: String,
    pub kind: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixCell {
    pub id: String,
    pub matrix_id: String,
    pub paper_id: String,
    pub column_id: String,
    pub value_text: Option<String>,
    pub value_number: Option<f64>,
    pub value_bool: Option<bool>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchReport {
    pub id: String,
    pub research_project_id: String,
    pub title: String,
    pub body_md: String,
    pub paper_ids: Vec<String>,
    pub claim_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Convenience: row mapping for the projects table.
pub fn row_to_project(r: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchProject> {
    Ok(ResearchProject {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        title: r.get(2)?,
        question: r.get(3)?,
        description: r.get(4)?,
        status: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
    })
}

pub fn row_to_paper(r: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchPaper> {
    Ok(ResearchPaper {
        id: r.get(0)?,
        research_project_id: r.get(1)?,
        document_id: r.get(2)?,
        title: r.get(3)?,
        authors: r.get(4)?,
        venue: r.get(5)?,
        year: r.get(6)?,
        doi: r.get(7)?,
        url: r.get(8)?,
        notes: r.get(9)?,
        position: r.get(10)?,
        created_at: r.get(11)?,
    })
}

pub fn row_to_claim(r: &rusqlite::Row<'_>) -> rusqlite::Result<PaperClaim> {
    let confidence: f64 = r.get(7)?;
    Ok(PaperClaim {
        id: r.get(0)?,
        research_project_id: r.get(1)?,
        paper_id: r.get(2)?,
        document_id: r.get(3)?,
        claim: r.get(4)?,
        category: r.get(5)?,
        evidence: r.get(6)?,
        confidence,
        kg_entity_id: r.get(8)?,
        created_at: r.get(9)?,
    })
}

pub fn row_to_matrix(r: &rusqlite::Row<'_>) -> rusqlite::Result<ComparisonMatrix> {
    Ok(ComparisonMatrix {
        id: r.get(0)?,
        research_project_id: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        created_at: r.get(4)?,
    })
}

pub fn row_to_column(r: &rusqlite::Row<'_>) -> rusqlite::Result<MatrixColumn> {
    Ok(MatrixColumn {
        id: r.get(0)?,
        matrix_id: r.get(1)?,
        label: r.get(2)?,
        kind: r.get(3)?,
        position: r.get(4)?,
    })
}

pub fn row_to_cell(r: &rusqlite::Row<'_>) -> rusqlite::Result<MatrixCell> {
    let value_bool: Option<i64> = r.get(6)?;
    Ok(MatrixCell {
        id: r.get(0)?,
        matrix_id: r.get(1)?,
        paper_id: r.get(2)?,
        column_id: r.get(3)?,
        value_text: r.get(4)?,
        value_number: r.get(5)?,
        value_bool: value_bool.map(|v| v != 0),
        updated_at: r.get(7)?,
    })
}

pub fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

pub fn touch_project(pool: &SqlitePool, project_id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE research_projects SET updated_at = ?1 WHERE id = ?2",
        params![now_ms(), project_id],
    )?;
    Ok(())
}