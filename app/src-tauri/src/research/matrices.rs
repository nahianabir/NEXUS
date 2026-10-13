//! Comparison matrices: rows = papers, columns = criteria.

use rusqlite::params;

use super::{now_ms, row_to_cell, row_to_column, row_to_matrix, ComparisonMatrix, MatrixCell, MatrixColumn};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn create_matrix(pool: &SqlitePool, m: &ComparisonMatrix) -> AppResult<ComparisonMatrix> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut m = m.clone();
    if m.id.is_empty() { m.id = super::new_id(); }
    if m.created_at == 0 { m.created_at = now; }
    conn.execute(
        "INSERT INTO comparison_matrices (id, research_project_id, title, description, created_at)
         VALUES (?1,?2,?3,?4,?5)",
        params![m.id, m.research_project_id, m.title, m.description, m.created_at],
    )?;
    super::touch_project(pool, &m.research_project_id)?;
    Ok(m)
}

pub fn get_matrix(pool: &SqlitePool, id: &str) -> AppResult<ComparisonMatrix> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, research_project_id, title, description, created_at FROM comparison_matrices WHERE id = ?1",
        params![id],
        row_to_matrix,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("matrix {id}")),
        other => AppError::Db(other),
    })
}

pub fn list_matrices(pool: &SqlitePool, project_id: &str) -> AppResult<Vec<ComparisonMatrix>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, research_project_id, title, description, created_at FROM comparison_matrices WHERE research_project_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt.query_map(params![project_id], row_to_matrix)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn delete_matrix(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let project_id: String = conn.query_row("SELECT research_project_id FROM comparison_matrices WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("matrix {id}")),
            other => AppError::Db(other),
        })?;
    conn.execute("DELETE FROM comparison_matrices WHERE id = ?1", params![id])?;
    super::touch_project(pool, &project_id)?;
    Ok(())
}

pub fn add_column(pool: &SqlitePool, c: &MatrixColumn) -> AppResult<MatrixColumn> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut c = c.clone();
    if c.id.is_empty() { c.id = super::new_id(); }
    if c.kind.is_empty() { c.kind = "text".into(); }
    conn.execute(
        "INSERT INTO matrix_columns (id, matrix_id, label, kind, position) VALUES (?1,?2,?3,?4,?5)",
        params![c.id, c.matrix_id, c.label, c.kind, c.position],
    )?;
    let project_id: String = conn.query_row("SELECT research_project_id FROM comparison_matrices WHERE id = ?1", params![c.matrix_id], |r| r.get(0))?;
    super::touch_project(pool, &project_id)?;
    Ok(c)
}

pub fn list_columns(pool: &SqlitePool, matrix_id: &str) -> AppResult<Vec<MatrixColumn>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, matrix_id, label, kind, position FROM matrix_columns WHERE matrix_id = ?1 ORDER BY position",
    )?;
    let rows = stmt.query_map(params![matrix_id], row_to_column)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn delete_column(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let matrix_id: String = conn.query_row("SELECT matrix_id FROM matrix_columns WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("column {id}")),
            other => AppError::Db(other),
        })?;
    let project_id: String = conn.query_row("SELECT research_project_id FROM comparison_matrices WHERE id = ?1", params![matrix_id], |r| r.get(0))?;
    conn.execute("DELETE FROM matrix_columns WHERE id = ?1", params![id])?;
    super::touch_project(pool, &project_id)?;
    Ok(())
}

pub fn set_cell(pool: &SqlitePool, cell: &MatrixCell) -> AppResult<MatrixCell> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let mut cell = cell.clone();
    if cell.id.is_empty() { cell.id = super::new_id(); }
    cell.updated_at = now;
    let bool_int: Option<i64> = cell.value_bool.map(|v| if v { 1 } else { 0 });
    // upsert via the UNIQUE(paper_id, column_id) constraint.
    let existing: Option<String> = conn.query_row(
        "SELECT id FROM matrix_cells WHERE paper_id = ?1 AND column_id = ?2",
        params![cell.paper_id, cell.column_id],
        |r| r.get(0),
    ).ok();
    if let Some(id) = existing {
        cell.id = id;
        conn.execute(
            "UPDATE matrix_cells SET value_text=?1, value_number=?2, value_bool=?3, updated_at=?4 WHERE id=?5",
            params![cell.value_text, cell.value_number, bool_int, cell.updated_at, cell.id],
        )?;
    } else {
        conn.execute(
            "INSERT INTO matrix_cells (id, matrix_id, paper_id, column_id, value_text, value_number, value_bool, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![cell.id, cell.matrix_id, cell.paper_id, cell.column_id, cell.value_text, cell.value_number, bool_int, cell.updated_at],
        )?;
    }
    let project_id: String = conn.query_row("SELECT research_project_id FROM comparison_matrices WHERE id = ?1", params![cell.matrix_id], |r| r.get(0))?;
    super::touch_project(pool, &project_id)?;
    get_cell(pool, &cell.id)
}

pub fn get_cell(pool: &SqlitePool, id: &str) -> AppResult<MatrixCell> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, matrix_id, paper_id, column_id, value_text, value_number, value_bool, updated_at FROM matrix_cells WHERE id = ?1",
        params![id],
        row_to_cell,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("cell {id}")),
        other => AppError::Db(other),
    })
}

pub fn list_cells(pool: &SqlitePool, matrix_id: &str) -> AppResult<Vec<MatrixCell>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, matrix_id, paper_id, column_id, value_text, value_number, value_bool, updated_at FROM matrix_cells WHERE matrix_id = ?1",
    )?;
    let rows = stmt.query_map(params![matrix_id], row_to_cell)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}
