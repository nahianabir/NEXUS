//! Tauri commands for Document Intelligence. Phase J.

use std::sync::Arc;

use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::error::AppResult;
use nexus_core::intelligence;
use nexus_core::intelligence::csv::CsvOptions;
use nexus_core::intelligence::tables::Table;

#[derive(serde::Deserialize)]
pub struct TableExportRequest {
    pub text: String,
    /// 0-based index of which extracted table to export. If None,
    /// export the first one.
    pub index: Option<usize>,
    pub delimiter: Option<char>,
    pub include_header: Option<bool>,
}

#[derive(serde::Serialize)]
pub struct TableExportResponse {
    pub table: Table,
    pub body: String,
    pub markdown: String,
    pub row_count: usize,
    pub column_count: usize,
}

#[tauri::command]
pub fn document_extract_tables(text: String) -> AppResult<Vec<Table>> {
    Ok(intelligence::tables::extract(&text))
}

#[tauri::command]
pub fn document_export_csv(request: TableExportRequest) -> AppResult<TableExportResponse> {
    let tables = intelligence::tables::extract(&request.text);
    let idx = request.index.unwrap_or(0);
    let t = tables.get(idx).ok_or_else(|| nexus_core::error::AppError::NotFound(format!("table {idx}")))?;
    let opts = CsvOptions { delimiter: request.delimiter.unwrap_or(','), include_header: request.include_header.unwrap_or(true) };
    let body = intelligence::csv::to_csv(t, &opts);
    let md = intelligence::tables::render_markdown(t);
    let row_count = t.rows.len();
    let column_count = t.headers.len();
    Ok(TableExportResponse { table: t.clone(), body, markdown: md, row_count, column_count })
}

#[tauri::command]
pub fn document_diff(a: String, b: String) -> AppResult<intelligence::diff::DiffSummary> {
    Ok(intelligence::diff::diff_lines(&a, &b))
}

#[tauri::command]
pub fn _ignore_state(_state: State<'_, Arc<AppState>>) -> AppResult<()> { Ok(()) }
