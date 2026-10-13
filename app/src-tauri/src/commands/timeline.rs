//! Tauri commands for the Project Time Machine. Phase S.1.
//!
//! These commands never mutate the source tables. The only writes
//! are to `timeline_events`, which is append-only and idempotent.

use serde::{Deserialize, Serialize};
use tauri::State;

use std::sync::Arc;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::AppResult;
use nexus_core::timeline::{
    self, Comparison, IngestReport, Reconstruction, TimelineEvent, TimelineFilter,
};

/// Build (or rebuild) the timeline for one project. Idempotent: a
/// second call on the same project with no new source rows is a
/// no-op (the UNIQUE key prevents duplicates).
#[tauri::command]
pub fn timeline_ingest_project(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> AppResult<IngestReport> {
    let report = timeline::ingest::ingest_all_for_project(&state.pool, &project_id)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "timeline.ingest",
        Some(&project_id),
        serde_json::json!({
            "examined": report.examined,
            "inserted": report.inserted,
            "skipped_existing": report.skipped_existing,
            "run_id": report.run_id,
        }),
    );
    Ok(report)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineListRequest {
    pub project_id: Option<String>,
    pub source_kind: Option<String>,
    pub event_type: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub limit: Option<usize>,
}

#[tauri::command]
pub fn timeline_list(
    state: State<'_, Arc<AppState>>,
    request: TimelineListRequest,
) -> AppResult<Vec<TimelineEvent>> {
    let filter = TimelineFilter {
        project_id: request.project_id,
        source_kind: request.source_kind,
        event_type: request.event_type,
        since_ms: request.since_ms,
        until_ms: request.until_ms,
        limit: request.limit,
    };
    timeline::list(&state.pool, &filter)
}

#[tauri::command]
pub fn timeline_count(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> AppResult<i64> {
    timeline::count_for_project(&state.pool, &project_id)
}

#[tauri::command]
pub fn timeline_reconstruct(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    at_ms: i64,
) -> AppResult<Reconstruction> {
    timeline::reconstruct_at(&state.pool, &project_id, at_ms)
}

#[tauri::command]
pub fn timeline_compare(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    from_ms: i64,
    to_ms: i64,
) -> AppResult<Comparison> {
    timeline::compare(&state.pool, &project_id, from_ms, to_ms)
}

/// Memories or kg_relationships whose `valid_until` is set or
/// whose `superseded_by` is non-null, scoped to this project.
/// Read-only; the rows themselves are not modified.
#[tauri::command]
pub fn timeline_superseded(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> AppResult<Superseded> {
    Superseded::build(&state.pool, &project_id)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Superseded {
    pub memories: Vec<SupersededItem>,
    pub kg_relationships: Vec<SupersededItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupersededItem {
    pub id: String,
    pub label: String,
    pub at_ms: i64,
    pub valid_until: Option<i64>,
    pub superseded_by: Option<String>,
    pub detail: serde_json::Value,
}

impl Superseded {
    fn build(pool: &nexus_core::db::SqlitePool, project_id: &str) -> AppResult<Self> {
        // The actual query lives in `nexus_core::timeline::query`
        // so the test suite can call it directly. The Tauri command
        // is a thin wrapper that re-exposes it to the UI.
        let out = nexus_core::timeline::query::list_superseded(pool, project_id)?;
        Ok(Self {
            memories: out.memories.into_iter().map(|m| SupersededItem {
                id: m.id,
                label: m.label,
                at_ms: m.at_ms,
                valid_until: m.valid_until,
                superseded_by: m.superseded_by,
                detail: m.detail,
            }).collect(),
            kg_relationships: out.kg_relationships.into_iter().map(|r| SupersededItem {
                id: r.id,
                label: r.label,
                at_ms: r.at_ms,
                valid_until: r.valid_until,
                superseded_by: r.superseded_by,
                detail: r.detail,
            }).collect(),
        })
    }
}

#[tauri::command]
pub fn timeline_decisions(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> AppResult<Vec<TimelineEvent>> {
    // Decision archaeology is a *query* over the existing memory
    // table, not a fresh timeline write. The shared query layer
    // returns synthesised read-only `TimelineEvent` rows so the
    // same data shape is used by the UI.
    timeline::query::list_decisions(&state.pool, &project_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertManualRequest {
    pub project_id: String,
    pub summary: String,
    pub event_at: Option<i64>,
    pub detail: serde_json::Value,
}

#[tauri::command]
pub fn timeline_insert_manual(
    state: State<'_, Arc<AppState>>,
    request: InsertManualRequest,
) -> AppResult<TimelineEvent> {
    let ev = timeline::insert_manual(
        &state.pool,
        &request.project_id,
        &request.summary,
        request.event_at,
        request.detail,
    )?;
    let _ = audit::log(
        &state.pool,
        "user",
        "timeline.manual",
        Some(&request.project_id),
        serde_json::json!({ "summary": request.summary, "event_at": request.event_at }),
    );
    Ok(ev)
}

#[tauri::command]
pub fn timeline_delete(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    timeline::delete(&state.pool, &id)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "timeline.delete",
        Some(&id),
        serde_json::json!({}),
    );
    Ok(())
}
