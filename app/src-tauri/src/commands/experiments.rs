//! Tauri commands for experiment tracking. Phase H.

use std::sync::Arc;

use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::error::AppResult;
use nexus_core::experiments::{self, Experiment, ExperimentRun};

#[tauri::command]
pub fn experiment_create(state: State<'_, Arc<AppState>>, mut exp: Experiment) -> AppResult<Experiment> {
    if exp.name.trim().is_empty() { exp.name = "Untitled experiment".into(); }
    experiments::experiments::create(&state.pool, &exp)
}

#[tauri::command]
pub fn experiment_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<Experiment> {
    experiments::experiments::get(&state.pool, &id)
}

#[tauri::command]
pub fn experiment_list(state: State<'_, Arc<AppState>>, project_id: Option<String>, status: Option<String>, limit: Option<i64>) -> AppResult<Vec<Experiment>> {
    experiments::experiments::list(&state.pool, project_id.as_deref(), status.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
pub fn experiment_update(state: State<'_, Arc<AppState>>, exp: Experiment) -> AppResult<Experiment> {
    experiments::experiments::update(&state.pool, &exp)
}

#[tauri::command]
pub fn experiment_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    experiments::experiments::delete(&state.pool, &id)
}

#[tauri::command]
pub fn experiment_run_create(state: State<'_, Arc<AppState>>, mut run: ExperimentRun) -> AppResult<ExperimentRun> {
    if run.name.trim().is_empty() { run.name = "Untitled run".into(); }
    experiments::runs::create(&state.pool, &run)
}

#[tauri::command]
pub fn experiment_run_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<ExperimentRun> {
    experiments::runs::get(&state.pool, &id)
}

#[tauri::command]
pub fn experiment_run_list(state: State<'_, Arc<AppState>>, experiment_id: String) -> AppResult<Vec<ExperimentRun>> {
    experiments::runs::list_for(&state.pool, &experiment_id)
}

#[tauri::command]
pub fn experiment_run_update(state: State<'_, Arc<AppState>>, run: ExperimentRun) -> AppResult<ExperimentRun> {
    experiments::runs::update(&state.pool, &run)
}

#[tauri::command]
pub fn experiment_run_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    experiments::runs::delete(&state.pool, &id)
}

#[tauri::command]
pub fn experiment_metric_history(state: State<'_, Arc<AppState>>, experiment_id: String) -> AppResult<Vec<experiments::runs::MetricPoint>> {
    experiments::runs::metric_history(&state.pool, &experiment_id)
}
