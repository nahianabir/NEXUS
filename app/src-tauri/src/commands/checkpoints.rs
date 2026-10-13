//! Tauri commands for the One-Click Work Resume feature. Phase S.2.
//!
//! These commands are mostly thin wrappers around the
//! `nexus_core::checkpoints` data layer. Writes are wrapped in
//! an `audit::log` call so the operation is traceable. Reads are
//! not logged individually (they would flood the audit log);
//! only the resume action is logged because it has user-visible
//! side effects on the row.

use serde::{Deserialize, Serialize};
use tauri::State;

use std::sync::Arc;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::checkpoints::{
    self, drift::DriftReport, CapturedConstraint, CapturedDecision, CapturedError,
    Checkpoint, CheckpointStatus, CreateCheckpoint, EvidenceRef, ObservedFile,
    ObservedTest, PendingAction, ResumeRecord, UpdateCheckpoint,
};
use nexus_core::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCheckpointsRequest {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[tauri::command]
pub fn checkpoints_list(
    state: State<'_, Arc<AppState>>,
    request: ListCheckpointsRequest,
) -> AppResult<Vec<Checkpoint>> {
    let filter = checkpoints::ListFilter {
        project_id: request.project_id,
        task_id: request.task_id,
        status: request.status,
        limit: request.limit,
    };
    checkpoints::list(&state.pool, &filter)
}

#[tauri::command]
pub fn checkpoints_get(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<Checkpoint> {
    checkpoints::get(&state.pool, &id)
}

#[tauri::command]
pub fn checkpoints_create(
    state: State<'_, Arc<AppState>>,
    request: CreateCheckpointRequest,
) -> AppResult<Checkpoint> {
    // Read the audit-relevant fields first, then move the
    // request into the create call.
    let actor = request.created_by.clone().unwrap_or_else(|| "user".into());
    let c = checkpoints::create(&state.pool, &request.into())?;
    let _ = audit::log(
        &state.pool,
        &actor,
        "checkpoint.create",
        Some(&c.id),
        serde_json::json!({
            "project_id": c.project_id,
            "task_id": c.task_id,
            "title": c.title,
        }),
    );
    Ok(c)
}

#[tauri::command]
pub fn checkpoints_update(
    state: State<'_, Arc<AppState>>,
    id: String,
    request: UpdateCheckpointRequest,
) -> AppResult<Checkpoint> {
    let update = UpdateCheckpoint {
        title: request.title,
        goal: request.goal,
        completed_work: request.completed_work,
        current_state: request.current_state,
        next_action_text: request.next_action_text,
        observed_files: request.observed_files,
        observed_tests: request.observed_tests,
        errors: request.errors,
        decisions: request.decisions,
        constraints: request.constraints,
        pending_actions: request.pending_actions,
        evidence_refs: request.evidence_refs,
    };
    let c = checkpoints::update(&state.pool, &id, &update)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "checkpoint.update",
        Some(&id),
        serde_json::json!({}),
    );
    Ok(c)
}

#[tauri::command]
pub fn checkpoints_set_status(
    state: State<'_, Arc<AppState>>,
    id: String,
    status: String,
) -> AppResult<Checkpoint> {
    let s = CheckpointStatus::parse(&status)?;
    let c = checkpoints::set_status(&state.pool, &id, s)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "checkpoint.set_status",
        Some(&id),
        serde_json::json!({ "status": status }),
    );
    Ok(c)
}

#[tauri::command]
pub fn checkpoints_delete(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    checkpoints::delete(&state.pool, &id)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "checkpoint.delete",
        Some(&id),
        serde_json::json!({}),
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResponse {
    pub checkpoint: Checkpoint,
    pub drift_report: DriftReport,
    pub summary: String,
    pub resume_record_id: String,
}

#[tauri::command]
pub fn checkpoints_resume(
    state: State<'_, Arc<AppState>>,
    id: String,
    note: Option<String>,
) -> AppResult<ResumeResponse> {
    let (checkpoint, report, summary) = checkpoints::resume_summary(&state.pool, &id, note)?;
    // Find the most recent resume_record id for the response.
    let records = checkpoints::list_resumes(&state.pool, &id)?;
    let record_id = records.first().map(|r| r.id.clone()).unwrap_or_default();
    let _ = audit::log(
        &state.pool,
        "user",
        "checkpoint.resume",
        Some(&id),
        serde_json::json!({
            "drift_items": report.items.len(),
            "files_now": report.files_now.len(),
        }),
    );
    Ok(ResumeResponse {
        checkpoint,
        drift_report: report,
        summary,
        resume_record_id: record_id,
    })
}

#[tauri::command]
pub fn checkpoints_list_resumes(
    state: State<'_, Arc<AppState>>,
    checkpoint_id: String,
) -> AppResult<Vec<ResumeRecord>> {
    checkpoints::list_resumes(&state.pool, &checkpoint_id)
}

// The frontend-friendly `CreateCheckpoint` shape. Mirrors the
// `nexus_core::checkpoints::CreateCheckpoint` exactly so we can
// `into()` it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCheckpointRequest {
    pub project_id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub goal: Option<String>,
    pub completed_work: Option<String>,
    pub current_state: Option<String>,
    pub next_action_text: Option<String>,
    pub observed_files: Vec<ObservedFile>,
    pub observed_tests: Vec<ObservedTest>,
    pub errors: Vec<CapturedError>,
    pub decisions: Vec<CapturedDecision>,
    pub constraints: Vec<CapturedConstraint>,
    pub pending_actions: Vec<PendingAction>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub created_by: Option<String>,
    pub model_hint: Option<String>,
}

impl From<CreateCheckpointRequest> for CreateCheckpoint {
    fn from(r: CreateCheckpointRequest) -> Self {
        Self {
            project_id: r.project_id,
            task_id: r.task_id,
            title: r.title,
            goal: r.goal,
            completed_work: r.completed_work,
            current_state: r.current_state,
            next_action_text: r.next_action_text,
            observed_files: r.observed_files,
            observed_tests: r.observed_tests,
            errors: r.errors,
            decisions: r.decisions,
            constraints: r.constraints,
            pending_actions: r.pending_actions,
            evidence_refs: r.evidence_refs,
            created_by: r.created_by,
            model_hint: r.model_hint,
        }
    }
}

// Mirror of `UpdateCheckpoint` with the tri-state Option<Option<...>>
// fields flattened into a JSON-friendly `null | "set" | "clear"`.
//
// We use a `serde_with`-style encoding: if the JSON has
// `"goal": null` and `goal_set: false` we treat it as "leave
// alone"; if the JSON has `"goal": "..."` we treat it as "set";
// if the JSON has `"goal": null` and `goal_set: true` we treat
// it as "clear". That keeps the wire shape minimal.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCheckpointRequest {
    pub title: Option<String>,
    /// `None` => leave alone; `Some(None)` => clear; `Some(Some(s))` => set.
    pub goal: Option<Option<String>>,
    pub completed_work: Option<Option<String>>,
    pub current_state: Option<Option<String>>,
    pub next_action_text: Option<Option<String>>,
    pub observed_files: Option<Vec<ObservedFile>>,
    pub observed_tests: Option<Vec<ObservedTest>>,
    pub errors: Option<Vec<CapturedError>>,
    pub decisions: Option<Vec<CapturedDecision>>,
    pub constraints: Option<Vec<CapturedConstraint>>,
    pub pending_actions: Option<Vec<PendingAction>>,
    pub evidence_refs: Option<Vec<EvidenceRef>>,
}
