//! Tauri commands for the MemPalace adapter. Phase C.
//!
//! The integration is optional. The app boots and works without it.
//! When the user activates the MemPalace backend we spawn the binary
//! they point at and speak JSON-RPC. Live tests are gated.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::{AppError, AppResult};
use nexus_core::mempalace::adapter::*;
use nexus_core::mempalace::sidecar::MemPalaceConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendStatus {
    pub active: String,
    pub native_healthy: bool,
    pub mempalace_healthy: bool,
    pub mempalace_version: Option<String>,
    pub mempalace_message: String,
}

#[tauri::command]
pub fn memory_backend_status(state: State<'_, Arc<AppState>>) -> AppResult<BackendStatus> {
    let kind = state.memory_backends.current();
    let set = state.memory_backends.set.read();
    let native_healthy = set.native.health().map(|d| d.healthy).unwrap_or(false);
    let (mp_healthy, mp_version, mp_msg) = match set.mempalace.as_deref() {
        Some(b) => match b.health() {
            Ok(d) => (d.healthy, d.version, d.message),
            Err(e) => (false, None, e.to_string()),
        },
        None => (false, None, "not configured".into()),
    };
    Ok(BackendStatus {
        active: kind.as_str().into(),
        native_healthy,
        mempalace_healthy: mp_healthy,
        mempalace_version: mp_version,
        mempalace_message: mp_msg,
    })
}

#[tauri::command]
pub fn memory_backend_activate_native(state: State<'_, Arc<AppState>>) -> AppResult<()> {
    state.memory_backends.activate_native();
    audit::log(&state.pool, "user", "memory.backend.activate", None, serde_json::json!({"backend": "native"}))?;
    Ok(())
}

#[tauri::command]
pub fn memory_backend_disable(state: State<'_, Arc<AppState>>) -> AppResult<()> {
    state.memory_backends.disable();
    audit::log(&state.pool, "user", "memory.backend.activate", None, serde_json::json!({"backend": "disabled"}))?;
    Ok(())
}

#[tauri::command]
pub fn memory_backend_activate_mempalace(state: State<'_, Arc<AppState>>, config: MemPalaceConfig) -> AppResult<BackendStatus> {
    if config.binary.trim().is_empty() {
        return Err(AppError::Invalid("mempalace binary path is required".into()));
    }
    state.memory_backends.activate_mempalace(config);
    audit::log(&state.pool, "user", "memory.backend.activate", None, serde_json::json!({"backend": "mempalace"}))?;
    memory_backend_status(state)
}

#[tauri::command]
pub fn memory_backend_search(
    state: State<'_, Arc<AppState>>,
    query: String,
    project_id: Option<String>,
    limit: Option<usize>,
) -> AppResult<Vec<AdapterMemory>> {
    let lim = limit.unwrap_or(25);
    Ok(state.memory_backends.search(&query, project_id.as_deref(), lim))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreAdapterRequest {
    pub project_id: Option<String>,
    pub workspace_id: Option<String>,
    pub category: String,
    pub content: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_excerpt: Option<String>,
    pub event_at: Option<i64>,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
}

#[tauri::command]
pub fn memory_backend_store(state: State<'_, Arc<AppState>>, request: StoreAdapterRequest) -> AppResult<String> {
    let record = AdapterMemory {
        id: String::new(),
        backend: "native".into(),
        project_id: request.project_id,
        workspace_id: request.workspace_id,
        category: request.category,
        content: request.content,
        source_kind: request.source_kind,
        source_id: request.source_id,
        source_excerpt: request.source_excerpt,
        event_at: request.event_at,
        valid_from: request.valid_from,
        valid_until: request.valid_until,
        created_at: 0,
        updated_at: 0,
    };
    let set = state.memory_backends.set.read();
    let result = set.current().store(&record);
    drop(set);
    result.map_err(|e| AppError::Internal(format!("mempalace store: {e}")))
}

#[tauri::command]
pub fn memory_backend_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    let result = {
        let set = state.memory_backends.set.read();
        set.current().delete(&id)
    };
    result.map_err(|e| AppError::Internal(format!("mempalace delete: {e}")))?;
    audit::log(&state.pool, "user", "memory.backend.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}
