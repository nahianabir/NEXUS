//! Tauri commands for the controlled agent. Phase I.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use nexus_core::agent::{self, AgentRun, Invocation, Tool};
use nexus_core::app_state::AppState;
use nexus_core::error::AppResult;

#[tauri::command]
pub fn agent_tools_list(state: State<'_, Arc<AppState>>, category: Option<String>, include_disabled: Option<bool>) -> AppResult<Vec<Tool>> {
    agent::registry::list(&state.pool, category.as_deref(), include_disabled.unwrap_or(false))
}

#[tauri::command]
pub fn agent_tool_get(state: State<'_, Arc<AppState>>, name: String) -> AppResult<Tool> {
    agent::registry::get(&state.pool, &name)
}

#[tauri::command]
pub fn agent_tool_set_enabled(state: State<'_, Arc<AppState>>, name: String, enabled: bool) -> AppResult<Tool> {
    let t = agent::registry::set_enabled(&state.pool, &name, enabled)?;
    nexus_core::audit::log(&state.pool, "user", "agent.tool.toggle", Some(&name), serde_json::json!({ "enabled": enabled }))?;
    Ok(t)
}

#[tauri::command]
pub fn agent_run_create(state: State<'_, Arc<AppState>>, mut run: AgentRun) -> AppResult<AgentRun> {
    if run.title.trim().is_empty() { run.title = "Agent run".into(); }
    let r = agent::runs::create(&state.pool, &run)?;
    nexus_core::audit::log(&state.pool, "user", "agent.run.create", Some(&r.id), serde_json::json!({ "title": r.title }))?;
    Ok(r)
}

#[tauri::command]
pub fn agent_run_list(state: State<'_, Arc<AppState>>, limit: Option<i64>) -> AppResult<Vec<AgentRun>> {
    agent::runs::list(&state.pool, limit.unwrap_or(50))
}

#[tauri::command]
pub fn agent_run_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<AgentRun> {
    agent::runs::get(&state.pool, &id)
}

#[tauri::command]
pub fn agent_run_set_status(state: State<'_, Arc<AppState>>, id: String, status: String, finished: Option<bool>) -> AppResult<AgentRun> {
    let r = agent::runs::set_status(&state.pool, &id, &status, finished.unwrap_or(false))?;
    nexus_core::audit::log(&state.pool, "user", "agent.run.status", Some(&id), serde_json::json!({ "status": status }))?;
    Ok(r)
}

#[tauri::command]
pub fn agent_run_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    agent::runs::delete(&state.pool, &id)
}

#[tauri::command]
pub fn agent_invocation_create(state: State<'_, Arc<AppState>>, inv: Invocation) -> AppResult<Invocation> {
    agent::invocations::create(&state.pool, &inv)
}

#[tauri::command]
pub fn agent_invocation_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<Invocation> {
    agent::invocations::get(&state.pool, &id)
}

#[tauri::command]
pub fn agent_invocation_list(state: State<'_, Arc<AppState>>, run_id: String) -> AppResult<Vec<Invocation>> {
    agent::invocations::list_for_run(&state.pool, &run_id)
}

#[tauri::command]
pub fn agent_invocation_recent(state: State<'_, Arc<AppState>>, limit: Option<i64>) -> AppResult<Vec<Invocation>> {
    agent::invocations::list_recent(&state.pool, limit.unwrap_or(50))
}

#[derive(Serialize)]
pub struct RunToolResult {
    pub invocation: Invocation,
    pub output: serde_json::Value,
}

#[tauri::command]
pub fn agent_run_tool(
    state: State<'_, Arc<AppState>>,
    invocation_id: String,
    file_allowlist: Option<Vec<String>>,
    shell_allowlist: Option<Vec<String>>,
) -> AppResult<RunToolResult> {
    let inv = agent::invocations::get(&state.pool, &invocation_id)?;
    let out = agent::executor::run_tool(
        &state.pool,
        &invocation_id,
        &inv.tool_name,
        &inv.input,
        &file_allowlist.unwrap_or_default(),
        &shell_allowlist.unwrap_or_default(),
    )?;
    let fresh = agent::invocations::get(&state.pool, &invocation_id)?;
    nexus_core::audit::log(&state.pool, "user", "agent.tool.run", Some(&inv.tool_name), serde_json::json!({ "invocation": invocation_id, "status": fresh.status }))?;
    Ok(RunToolResult { invocation: fresh, output: out })
}

#[tauri::command]
pub fn agent_invocation_approve(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    agent::executor::approve(&state.pool, &id)?;
    nexus_core::audit::log(&state.pool, "user", "agent.invocation.approve", Some(&id), serde_json::json!({}))?;
    Ok(())
}

#[tauri::command]
pub fn agent_invocation_deny(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    agent::executor::deny(&state.pool, &id)?;
    nexus_core::audit::log(&state.pool, "user", "agent.invocation.deny", Some(&id), serde_json::json!({}))?;
    Ok(())
}
