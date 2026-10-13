//! Tauri command for inspecting the unified context builder. Phase E.

use std::sync::Arc;

use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::context::{self, BuildRequest, ContextBundle};
use nexus_core::error::AppResult;

#[tauri::command]
pub fn context_preview(
    state: State<'_, Arc<AppState>>,
    project_id: Option<String>,
    conversation_id: Option<String>,
    user_message: String,
    max_tokens: Option<i64>,
) -> AppResult<ContextBundle> {
    let req = BuildRequest {
        workspace_id: None,
        project_id,
        conversation_id,
        user_message,
        max_tokens: max_tokens.unwrap_or(1500),
        pinned: vec![],
    };
    context::build(&state.pool, &state.memory_backends, &req)
}
