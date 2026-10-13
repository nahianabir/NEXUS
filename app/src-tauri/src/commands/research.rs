//! Tauri commands for the Research Studio. Phase G.

use std::sync::Arc;

use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::error::AppResult;
use nexus_core::research::{
    self, ComparisonMatrix, MatrixCell, MatrixColumn, PaperClaim, ResearchPaper, ResearchProject, ResearchReport,
};

#[tauri::command]
pub fn research_project_create(state: State<'_, Arc<AppState>>, mut project: ResearchProject) -> AppResult<ResearchProject> {
    if project.title.trim().is_empty() { project.title = "Untitled".into(); }
    if project.question.trim().is_empty() { project.question = String::new(); }
    research::projects::create(&state.pool, &project)
}

#[tauri::command]
pub fn research_project_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<ResearchProject> {
    research::projects::get(&state.pool, &id)
}

#[tauri::command]
pub fn research_project_list(state: State<'_, Arc<AppState>>, workspace_id: Option<String>, status: Option<String>, limit: Option<i64>) -> AppResult<Vec<ResearchProject>> {
    research::projects::list(&state.pool, workspace_id.as_deref(), status.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
pub fn research_project_update(state: State<'_, Arc<AppState>>, project: ResearchProject) -> AppResult<ResearchProject> {
    research::projects::update(&state.pool, &project)
}

#[tauri::command]
pub fn research_project_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    research::projects::delete(&state.pool, &id)
}

#[tauri::command]
pub fn research_paper_add(state: State<'_, Arc<AppState>>, mut paper: ResearchPaper) -> AppResult<ResearchPaper> {
    if paper.title.trim().is_empty() { paper.title = "Untitled paper".into(); }
    research::papers::add(&state.pool, &paper)
}

#[tauri::command]
pub fn research_paper_update(state: State<'_, Arc<AppState>>, paper: ResearchPaper) -> AppResult<ResearchPaper> {
    research::papers::update(&state.pool, &paper)
}

#[tauri::command]
pub fn research_paper_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    research::papers::delete(&state.pool, &id)
}

#[tauri::command]
pub fn research_paper_list(state: State<'_, Arc<AppState>>, project_id: String) -> AppResult<Vec<ResearchPaper>> {
    research::papers::list_for(&state.pool, &project_id)
}

#[tauri::command]
pub fn research_claim_add(state: State<'_, Arc<AppState>>, mut claim: PaperClaim) -> AppResult<PaperClaim> {
    if claim.claim.trim().is_empty() { claim.claim = "(empty claim)".into(); }
    research::claims::add(&state.pool, &claim)
}

#[tauri::command]
pub fn research_claim_update(state: State<'_, Arc<AppState>>, claim: PaperClaim) -> AppResult<PaperClaim> {
    research::claims::update(&state.pool, &claim)
}

#[tauri::command]
pub fn research_claim_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    research::claims::delete(&state.pool, &id)
}

#[tauri::command]
pub fn research_claim_list(state: State<'_, Arc<AppState>>, project_id: String, category: Option<String>, paper_id: Option<String>) -> AppResult<Vec<PaperClaim>> {
    research::claims::list_for(&state.pool, &project_id, category.as_deref(), paper_id.as_deref())
}

#[tauri::command]
pub fn research_matrix_create(state: State<'_, Arc<AppState>>, mut matrix: ComparisonMatrix) -> AppResult<ComparisonMatrix> {
    if matrix.title.trim().is_empty() { matrix.title = "Comparison".into(); }
    research::matrices::create_matrix(&state.pool, &matrix)
}

#[tauri::command]
pub fn research_matrix_list(state: State<'_, Arc<AppState>>, project_id: String) -> AppResult<Vec<ComparisonMatrix>> {
    research::matrices::list_matrices(&state.pool, &project_id)
}

#[tauri::command]
pub fn research_matrix_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    research::matrices::delete_matrix(&state.pool, &id)
}

#[tauri::command]
pub fn research_matrix_columns(state: State<'_, Arc<AppState>>, matrix_id: String) -> AppResult<Vec<MatrixColumn>> {
    research::matrices::list_columns(&state.pool, &matrix_id)
}

#[tauri::command]
pub fn research_matrix_column_add(state: State<'_, Arc<AppState>>, mut column: MatrixColumn) -> AppResult<MatrixColumn> {
    if column.label.trim().is_empty() { column.label = "Untitled".into(); }
    research::matrices::add_column(&state.pool, &column)
}

#[tauri::command]
pub fn research_matrix_column_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    research::matrices::delete_column(&state.pool, &id)
}

#[tauri::command]
pub fn research_matrix_cells(state: State<'_, Arc<AppState>>, matrix_id: String) -> AppResult<Vec<MatrixCell>> {
    research::matrices::list_cells(&state.pool, &matrix_id)
}

#[tauri::command]
pub fn research_matrix_cell_set(state: State<'_, Arc<AppState>>, cell: MatrixCell) -> AppResult<MatrixCell> {
    research::matrices::set_cell(&state.pool, &cell)
}

#[tauri::command]
pub fn research_report_save(state: State<'_, Arc<AppState>>, mut report: ResearchReport) -> AppResult<ResearchReport> {
    if report.title.trim().is_empty() { report.title = "Untitled report".into(); }
    research::reports::save(&state.pool, &report)
}

#[tauri::command]
pub fn research_report_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<ResearchReport> {
    research::reports::get(&state.pool, &id)
}

#[tauri::command]
pub fn research_report_list(state: State<'_, Arc<AppState>>, project_id: String) -> AppResult<Vec<ResearchReport>> {
    research::reports::list_for(&state.pool, &project_id)
}

#[tauri::command]
pub fn research_report_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    research::reports::delete(&state.pool, &id)
}

#[tauri::command]
pub fn research_report_build(state: State<'_, Arc<AppState>>, project_id: String) -> AppResult<String> {
    research::markdown::build_project_report(&state.pool, &project_id)
}
