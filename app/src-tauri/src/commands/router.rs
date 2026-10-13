//! Tauri commands for the model router. Phase F.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::{AppError, AppResult};
use nexus_core::router;

#[tauri::command]
pub fn router_capabilities_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<router::Capabilities>> {
    router::capabilities::list_all(&state.pool)
}

#[tauri::command]
pub fn router_capabilities_get(state: State<'_, Arc<AppState>>, profile_id: String) -> AppResult<router::Capabilities> {
    router::capabilities::get(&state.pool, &profile_id)
}

#[tauri::command]
pub fn router_capabilities_upsert(state: State<'_, Arc<AppState>>, cap: router::Capabilities) -> AppResult<()> {
    router::capabilities::upsert(&state.pool, &cap)?;
    audit::log(&state.pool, "user", "router.cap.upsert", Some(&cap.profile_id), serde_json::json!({}))?;
    Ok(())
}

#[tauri::command]
pub fn router_capabilities_infer(state: State<'_, Arc<AppState>>, profile_id: String) -> AppResult<router::Capabilities> {
    let c = router::capabilities::infer_from_profile(&state.pool, &profile_id)?;
    router::capabilities::upsert(&state.pool, &c)?;
    Ok(c)
}

#[tauri::command]
pub fn router_lifecycle_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<router::LifecycleStatus>> {
    router::lifecycle::list_all(&state.pool)
}

#[tauri::command]
pub fn router_lifecycle_transition(
    state: State<'_, Arc<AppState>>,
    profile_id: String,
    to: String,
    error: Option<String>,
) -> AppResult<router::LifecycleStatus> {
    let target = router::State::parse(&to).map_err(AppError::from)?;
    let s = router::lifecycle::transition(&state.pool, &profile_id, target, error.as_deref())?;
    audit::log(&state.pool, "user", "router.lifecycle.transition", Some(&profile_id), serde_json::json!({"to": to}))?;
    Ok(s)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCommandRequest {
    pub task: String,                 // "general" | "coding" | "extraction" | "embedding" | "reranking" | "structured" | "tool"
    pub min_context_tokens: Option<i64>,
    pub prefer_profile_id: Option<String>,
}

#[tauri::command]
pub fn router_route(state: State<'_, Arc<AppState>>, request: RouteCommandRequest) -> AppResult<router::RoutingDecision> {
    let task = match request.task.as_str() {
        "general" => router::TaskKind::General,
        "coding" => router::TaskKind::Coding,
        "extraction" => router::TaskKind::Extraction,
        "embedding" => router::TaskKind::Embedding,
        "reranking" => router::TaskKind::Reranking,
        "structured" => router::TaskKind::Structured,
        "tool" => router::TaskKind::Tool,
        other => return Err(AppError::Invalid(format!("unknown task kind: {other}"))),
    };
    let req = router::RouteRequest {
        task,
        min_context_tokens: request.min_context_tokens.unwrap_or(0),
        prefer_profile_id: request.prefer_profile_id,
    };
    let decision = router::policy::route(&state.pool, &req)?;
    audit::log(&state.pool, "user", "router.route", None, serde_json::json!({
        "task": request.task,
        "selected": decision.selected,
        "fell_back": decision.fell_back,
    }))?;
    Ok(decision)
}

#[tauri::command]
pub fn router_decisions_recent(state: State<'_, Arc<AppState>>, limit: Option<i64>) -> AppResult<Vec<router::DecisionRecord>> {
    router::policy::list_recent(&state.pool, limit.unwrap_or(50))
}
