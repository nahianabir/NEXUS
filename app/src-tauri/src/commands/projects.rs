//! Tauri commands for workspaces, projects, notes, tasks, and memory.
//!
//! Phase 5 keeps the command surface small and predictable:
//!   * `workspaces_list` / `_create` / `_rename` / `_delete`
//!   * `projects_list` / `_get` / `_create` / `_update` / `_archive` / `_delete`
//!   * `notes_list` / `_get` / `_create` / `_update` / `_delete`
//!   * `tasks_list` / `_get` / `_create` / `_update` / `_set_status` / `_delete`
//!   * `tasks_due_in_range` — for the calendar view
//!   * `memory_list` / `_create` / `_update` / `_approve` / `_delete`
//!   * `ui_state_get` / `ui_state_set` (used for "active project" pointer)

use serde::{Deserialize, Serialize};
use tauri::State;

use std::sync::Arc;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::{AppError, AppResult};
use nexus_core::memory::{self, Memory, MemoryScope};
use nexus_core::projects::{notes, projects, tasks, ui_state, workspaces};

// -------------------------------- workspaces -----------------------------

#[tauri::command]
pub fn workspaces_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<workspaces::Workspace>> {
    workspaces::list(&state.pool)
}

#[tauri::command]
pub fn workspaces_create(
    state: State<'_, Arc<AppState>>,
    name: String,
    description: Option<String>,
) -> AppResult<workspaces::Workspace> {
    let w = workspaces::create(&state.pool, &name, description.as_deref())?;
    audit::log(&state.pool, "user", "workspace.create", Some(&w.id), serde_json::json!({"name": name}))?;
    Ok(w)
}

#[tauri::command]
pub fn workspaces_rename(state: State<'_, Arc<AppState>>, id: String, name: String) -> AppResult<()> {
    workspaces::rename(&state.pool, &id, &name)
}

#[tauri::command]
pub fn workspaces_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    workspaces::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "workspace.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

// -------------------------------- projects --------------------------------

#[tauri::command]
pub fn projects_list(state: State<'_, Arc<AppState>>, include_archived: Option<bool>) -> AppResult<Vec<projects::Project>> {
    projects::list(&state.pool, include_archived.unwrap_or(false))
}

#[tauri::command]
pub fn projects_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<projects::Project> {
    projects::get(&state.pool, &id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectRequest {
    pub workspace_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub goals: Option<Vec<String>>,
}

#[tauri::command]
pub fn projects_create(state: State<'_, Arc<AppState>>, request: CreateProjectRequest) -> AppResult<projects::Project> {
    let p = projects::create(
        &state.pool,
        request.workspace_id.as_deref(),
        &request.name,
        request.description.as_deref(),
        request.instructions.as_deref(),
        request.goals.as_deref().unwrap_or(&[]),
    )?;
    audit::log(&state.pool, "user", "project.create", Some(&p.id), serde_json::json!({"name": p.name}))?;
    Ok(p)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProjectRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub goals: Option<Vec<String>>,
}

#[tauri::command]
pub fn projects_update(state: State<'_, Arc<AppState>>, request: UpdateProjectRequest) -> AppResult<projects::Project> {
    let goals_slice: Option<&[String]> = request.goals.as_deref().map(|g| &g[..]);
    let p = projects::update(
        &state.pool,
        &request.id,
        request.name.as_deref(),
        request.description.as_deref(),
        request.instructions.as_deref(),
        goals_slice,
    )?;
    let _ = p;
    Ok(p)
}

#[tauri::command]
pub fn projects_archive(state: State<'_, Arc<AppState>>, id: String, archived: bool) -> AppResult<()> {
    projects::set_archived(&state.pool, &id, archived)
}

#[tauri::command]
pub fn projects_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    projects::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "project.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project: projects::Project,
    pub task_count: i64,
    pub open_task_count: i64,
    pub note_count: i64,
}

#[tauri::command]
pub fn projects_summaries(state: State<'_, Arc<AppState>>, include_archived: Option<bool>) -> AppResult<Vec<ProjectSummary>> {
    let ps = projects::list(&state.pool, include_archived.unwrap_or(false))?;
    let mut out = Vec::with_capacity(ps.len());
    for p in ps {
        let task_count = projects::count_tasks(&state.pool, &p.id, false).unwrap_or(0);
        let open_task_count = projects::count_tasks(&state.pool, &p.id, true).unwrap_or(0);
        let note_count = projects::count_notes(&state.pool, &p.id).unwrap_or(0);
        out.push(ProjectSummary { project: p, task_count, open_task_count, note_count });
    }
    Ok(out)
}

// -------------------------------- notes -----------------------------------

#[tauri::command]
pub fn notes_list(state: State<'_, Arc<AppState>>, project_id: Option<String>) -> AppResult<Vec<notes::Note>> {
    notes::list(&state.pool, project_id.as_deref())
}

#[tauri::command]
pub fn notes_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<notes::Note> {
    notes::get(&state.pool, &id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNoteRequest {
    pub project_id: Option<String>,
    pub title: String,
    pub body: String,
}

#[tauri::command]
pub fn notes_create(state: State<'_, Arc<AppState>>, request: CreateNoteRequest) -> AppResult<notes::Note> {
    let n = notes::create(&state.pool, request.project_id.as_deref(), &request.title, &request.body)?;
    audit::log(&state.pool, "user", "note.create", Some(&n.id), serde_json::json!({"title": n.title}))?;
    Ok(n)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNoteRequest {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
}

#[tauri::command]
pub fn notes_update(state: State<'_, Arc<AppState>>, request: UpdateNoteRequest) -> AppResult<notes::Note> {
    notes::update(&state.pool, &request.id, request.title.as_deref(), request.body.as_deref())
}

#[tauri::command]
pub fn notes_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    notes::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "note.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

// -------------------------------- tasks -----------------------------------

#[tauri::command]
pub fn tasks_list(
    state: State<'_, Arc<AppState>>,
    project_id: Option<String>,
    status: Option<String>,
    include_done: Option<bool>,
) -> AppResult<Vec<tasks::Task>> {
    tasks::list(&state.pool, project_id.as_deref(), status.as_deref(), include_done.unwrap_or(true))
}

#[tauri::command]
pub fn tasks_due_in_range(state: State<'_, Arc<AppState>>, from_ms: i64, to_ms: i64) -> AppResult<Vec<tasks::Task>> {
    tasks::list_due_in_range(&state.pool, from_ms, to_ms)
}

#[tauri::command]
pub fn tasks_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<tasks::Task> {
    tasks::get(&state.pool, &id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub project_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,           // "low" | "normal" | "high" | "urgent"
    pub due_at: Option<i64>,
    pub checklist: Option<Vec<tasks::ChecklistItem>>,
}

#[tauri::command]
pub fn tasks_create(state: State<'_, Arc<AppState>>, request: CreateTaskRequest) -> AppResult<tasks::Task> {
    let priority = tasks::TaskPriority::parse(&request.priority)?;
    let t = tasks::create(
        &state.pool,
        request.project_id.as_deref(),
        &request.title,
        request.description.as_deref(),
        priority,
        request.due_at,
        request.checklist.as_deref().unwrap_or(&[]),
    )?;
    audit::log(&state.pool, "user", "task.create", Some(&t.id), serde_json::json!({"title": t.title}))?;
    Ok(t)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<String>,
    /// Tri-state: `None` = leave alone, `Some(None)` = clear due, `Some(Some(ms))` = set.
    pub due_at: Option<Option<i64>>,
    pub checklist: Option<Vec<tasks::ChecklistItem>>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn tasks_update(state: State<'_, Arc<AppState>>, request: UpdateTaskRequest) -> AppResult<tasks::Task> {
    let priority = match request.priority.as_deref() {
        Some(s) => Some(tasks::TaskPriority::parse(s)?),
        None => None,
    };
    let checklist_slice: Option<&[tasks::ChecklistItem]> = request.checklist.as_deref().map(|c| &c[..]);
    tasks::update(
        &state.pool,
        &request.id,
        request.title.as_deref(),
        request.description.as_deref(),
        priority,
        request.due_at,
        checklist_slice,
        request.notes.as_deref(),
    )
}

#[tauri::command]
pub fn tasks_set_status(state: State<'_, Arc<AppState>>, id: String, status: String) -> AppResult<tasks::Task> {
    let s = tasks::TaskStatus::parse(&status)?;
    tasks::set_status(&state.pool, &id, s)
}

#[tauri::command]
pub fn tasks_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    tasks::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "task.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

// -------------------------------- memory ----------------------------------

#[tauri::command]
pub fn memory_list(state: State<'_, Arc<AppState>>, scope: Option<String>, approved_only: Option<bool>) -> AppResult<Vec<Memory>> {
    memory::list(&state.pool, scope.as_deref(), approved_only.unwrap_or(false))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemoryRequest {
    pub scope: String,                              // "user" | "project" | "auto" | "conv"
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub content: String,
    pub source: Option<String>,
    pub approved: Option<bool>,
}

#[tauri::command]
pub fn memory_create(state: State<'_, Arc<AppState>>, request: CreateMemoryRequest) -> AppResult<Memory> {
    let s = MemoryScope::parse(&request.scope)?;
    let m = memory::create(
        &state.pool,
        s,
        request.project_id.as_deref(),
        request.conversation_id.as_deref(),
        &request.content,
        request.source.as_deref(),
        request.approved.unwrap_or(false),
    )?;
    audit::log(&state.pool, "user", "memory.create", Some(&m.id), serde_json::json!({"scope": m.scope}))?;
    Ok(m)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemoryRequest {
    pub id: String,
    pub scope: Option<String>,
    pub content: Option<String>,
    /// Tri-state: `None` = leave alone, `Some(None)` = clear, `Some(Some(id))` = set.
    pub project_id: Option<Option<String>>,
    pub approved: Option<bool>,
}

#[tauri::command]
pub fn memory_update(state: State<'_, Arc<AppState>>, request: UpdateMemoryRequest) -> AppResult<Memory> {
    let s = match request.scope.as_deref() {
        Some(s) => Some(MemoryScope::parse(s)?),
        None => None,
    };
    let project_owned: Option<Option<String>> = request.project_id.clone();
    let project_inner = project_owned.as_ref().map(|p| p.as_deref());
    memory::update(
        &state.pool,
        &request.id,
        s,
        request.content.as_deref(),
        project_inner,
        request.approved,
    )
}

#[tauri::command]
pub fn memory_approve(state: State<'_, Arc<AppState>>, id: String) -> AppResult<Memory> {
    let m = memory::approve(&state.pool, &id)?;
    audit::log(&state.pool, "user", "memory.approve", Some(&id), serde_json::json!({}))?;
    Ok(m)
}

#[tauri::command]
pub fn memory_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    memory::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "memory.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

// -------------------------------- ui_state --------------------------------

/// Returns the JSON value stored at `key`, or `None` if the key is absent.
#[tauri::command]
pub fn ui_state_get(state: State<'_, Arc<AppState>>, key: String) -> AppResult<Option<serde_json::Value>> {
    let v: Option<serde_json::Value> = ui_state::get(&state.pool, &key)?;
    Ok(v)
}

#[tauri::command]
pub fn ui_state_set(state: State<'_, Arc<AppState>>, key: String, value: serde_json::Value) -> AppResult<()> {
    ui_state::set(&state.pool, &key, &value)
}

#[tauri::command]
pub fn ui_state_unset(state: State<'_, Arc<AppState>>, key: String) -> AppResult<()> {
    ui_state::unset(&state.pool, &key)
}

#[tauri::command]
pub fn get_active_project(state: State<'_, Arc<AppState>>) -> AppResult<Option<projects::Project>> {
    let v: Option<String> = ui_state::get(&state.pool, ui_state::ACTIVE_PROJECT)?;
    match v {
        Some(id) if !id.is_empty() => Ok(Some(projects::get(&state.pool, &id).map_err(|e| match e {
            AppError::NotFound(_) => AppError::NotFound(format!("active project {id}")),
            other => other,
        })?)),
        _ => Ok(None),
    }
}

#[tauri::command]
pub fn set_active_project(state: State<'_, Arc<AppState>>, id: Option<String>) -> AppResult<()> {
    match id {
        Some(s) if !s.is_empty() => ui_state::set(&state.pool, ui_state::ACTIVE_PROJECT, &s)?,
        _ => ui_state::unset(&state.pool, ui_state::ACTIVE_PROJECT)?,
    }
    Ok(())
}

// ---------------------------- context blocks ------------------------------

// `build_project_context_block` and `build_memory_context_block` live in
// `nexus_core::projects::context` so unit tests can call them without
// pulling in the Tauri command module. We re-export here for convenience.
pub use nexus_core::projects::context::{build_memory_context_block, build_project_context_block};
