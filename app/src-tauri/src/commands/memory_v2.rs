//! Tauri commands for the native long-term memory v2 (Phase 5B).
//!
//! The v1 commands in `commands/projects.rs` keep working. New callers
//! should use the `memories_v2_*` family; the v1 endpoints remain so
//! Phase 5 UI does not regress.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::AppResult;
use nexus_core::memory_v2;
use nexus_core::memory_v2::candidates::Candidate;
use nexus_core::memory_v2::evidence::Evidence;
use nexus_core::memory_v2::memory::{MemoryFilter, MemoryV2};
use nexus_core::memory_v2::session::Session;

fn audit_log(state: &AppState, actor: &str, action: &str, target: Option<&str>, data: serde_json::Value) {
    let _ = audit::log(&state.pool, actor, action, target, data);
}

// ---------------------------- memories_v2 ---------------------------------

#[tauri::command]
pub fn memories_v2_list(
    state: State<'_, Arc<AppState>>,
    project_id: Option<String>,
    workspace_id: Option<String>,
    category: Option<String>,
    approved_only: Option<bool>,
    include_superseded: Option<bool>,
    include_deleted: Option<bool>,
    source_kind: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<MemoryV2>> {
    let filter = MemoryFilter {
        project_id,
        workspace_id,
        category,
        approved_only: approved_only.unwrap_or(true),
        include_superseded: include_superseded.unwrap_or(false),
        include_deleted: include_deleted.unwrap_or(false),
        source_kind,
        limit,
    };
    memory_v2::memory::list(&state.pool, &filter)
}

#[tauri::command]
pub fn memories_v2_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<MemoryV2> {
    memory_v2::memory::get(&state.pool, &id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemoryV2Request {
    pub workspace_id: Option<String>,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub category: String,
    pub content: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_label: Option<String>,
    pub source_excerpt: Option<String>,
    pub event_at: Option<i64>,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
    pub pinned: Option<bool>,
}

#[tauri::command]
pub fn memories_v2_create(
    state: State<'_, Arc<AppState>>,
    request: CreateMemoryV2Request,
) -> AppResult<MemoryV2> {
    let now = chrono::Utc::now().timestamp_millis();
    let norm = memory_v2::normalize_content(&request.content);
    let hash = memory_v2::content_hash(&norm);
    if let Some(existing) = memory_v2::memory::find_by_hash(&state.pool, &hash)? {
        return Ok(existing);
    }
    let m = MemoryV2 {
        id: memory_v2::memory::new_id(),
        workspace_id: request.workspace_id,
        project_id: request.project_id,
        conversation_id: request.conversation_id,
        category: request.category,
        content: request.content,
        content_hash: hash,
        normalized_content: norm,
        source_kind: request.source_kind,
        source_id: request.source_id,
        source_label: request.source_label,
        source_excerpt: request.source_excerpt,
        event_at: request.event_at,
        valid_from: request.valid_from,
        valid_until: request.valid_until,
        verified_at: Some(now),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "approved".into(),
        pinned: request.pinned.unwrap_or(false),
        created_at: now,
        updated_at: now,
    };
    memory_v2::memory::insert(&state.pool, &m)?;
    audit_log(&state, "user", "memory_v2.create", Some(&m.id), serde_json::json!({"category": m.category}));
    Ok(m)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemoryV2Request {
    pub id: String,
    pub content: Option<String>,
    pub category: Option<String>,
    pub pinned: Option<bool>,
    /// Tri-state: `None` = leave alone, `Some(None)` = clear, `Some(Some(ms))` = set.
    pub valid_from: Option<Option<i64>>,
    pub valid_until: Option<Option<i64>>,
    pub event_at: Option<Option<i64>>,
    pub verified_at: Option<i64>,
}

#[tauri::command]
pub fn memories_v2_update(
    state: State<'_, Arc<AppState>>,
    request: UpdateMemoryV2Request,
) -> AppResult<MemoryV2> {
    let m = memory_v2::memory::update(
        &state.pool,
        &request.id,
        request.content.as_deref(),
        request.category.as_deref(),
        request.pinned,
        request.valid_from,
        request.valid_until,
        request.event_at,
        request.verified_at,
    )?;
    audit_log(&state, "user", "memory_v2.update", Some(&m.id), serde_json::json!({}));
    Ok(m)
}

#[tauri::command]
pub fn memories_v2_approve(state: State<'_, Arc<AppState>>, id: String) -> AppResult<MemoryV2> {
    let m = memory_v2::memory::approve(&state.pool, &id)?;
    audit_log(&state, "user", "memory_v2.approve", Some(&id), serde_json::json!({}));
    Ok(m)
}

#[tauri::command]
pub fn memories_v2_supersede(state: State<'_, Arc<AppState>>, old_id: String, new_id: String) -> AppResult<()> {
    memory_v2::memory::supersede(&state.pool, &old_id, &new_id)?;
    audit_log(&state, "user", "memory_v2.supersede", Some(&old_id), serde_json::json!({"new_id": new_id}));
    Ok(())
}

#[tauri::command]
pub fn memories_v2_delete(state: State<'_, Arc<AppState>>, id: String, hard: Option<bool>) -> AppResult<()> {
    if hard.unwrap_or(false) {
        memory_v2::memory::hard_delete(&state.pool, &id)?;
    } else {
        memory_v2::memory::soft_delete(&state.pool, &id)?;
    }
    audit_log(&state, "user", "memory_v2.delete", Some(&id), serde_json::json!({"hard": hard.unwrap_or(false)}));
    Ok(())
}

#[tauri::command]
pub fn memories_v2_search(
    state: State<'_, Arc<AppState>>,
    query: String,
    category: Option<String>,
    project_id: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<MemoryV2>> {
    let ids = memory_v2::search::search(
        &state.pool,
        &query,
        category.as_deref(),
        project_id.as_deref(),
        limit.unwrap_or(50) as usize,
    )?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(m) = memory_v2::memory::get(&state.pool, &id) {
            out.push(m);
        }
    }
    Ok(out)
}

// ---------------------------- explicit save --------------------------------
//
// Phase P4 — Bug F. The chat pipeline used to forward
// every user message to the model as a normal chat
// completion. When the user wrote "Save to your memory
// that my age is 22", the model had to figure out the
// intent on its own — but no `save_memory` tool is
// exposed, so the model emitted a 91-second apology
// instead of persisting the fact.
//
// This command is the deterministic local service: it
// looks for the explicit save phrase, extracts the
// "that <fact>" clause, classifies it, and writes it
// through the canonical `memories_v2_create` path. The
// model is NOT invoked.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySaveExplicitRequest {
    /// The raw user message (e.g. "Save to your memory
    /// that my age is 22.").
    pub text: String,
    /// Optional conversation scope.
    pub conversation_id: Option<String>,
    /// Optional project scope.
    pub project_id: Option<String>,
}

#[tauri::command]
pub fn memory_save_explicit(
    state: State<'_, Arc<AppState>>,
    request: MemorySaveExplicitRequest,
) -> AppResult<memory_v2::explicit_save::SaveResult> {
    let res = memory_v2::explicit_save::try_handle(
        &state.pool,
        &request.text,
        request.conversation_id.as_deref(),
        request.project_id.as_deref(),
    )?;
    // `try_handle` returns `None` when the input is not
    // an explicit save. The frontend can use this to
    // decide whether to fall through to the normal chat
    // path. The Tauri command always returns a
    // `SaveResult` so the wire format is stable; for the
    // "not a save" case we return `Rejected` with a
    // descriptive reason.
    let res = res.unwrap_or_else(|| memory_v2::explicit_save::SaveResult::rejected("not an explicit save request"));
    if matches!(res.outcome.as_str(), "saved" | "updated") {
        let _ = audit::log(
            &state.pool,
            "user",
            "memory.explicit_save",
            res.memory_id.as_deref(),
            serde_json::json!({
                "outcome": res.outcome,
                "category": res.category,
                "content": res.content,
            }),
        );
    }
    Ok(res)
}

#[tauri::command]
pub fn memories_v2_categories() -> Vec<String> {
    memory_v2::CATEGORIES.iter().map(|s| s.to_string()).collect()
}

#[tauri::command]
pub fn memories_v2_migrate_from_v1(state: State<'_, Arc<AppState>>) -> AppResult<i64> {
    let n = memory_v2::memory::migrate_from_v1(&state.pool)?;
    audit_log(&state, "system", "memory_v2.migrate_v1", None, serde_json::json!({"count": n}));
    Ok(n as i64)
}

// ---------------------------- evidence ------------------------------------

#[tauri::command]
pub fn memory_evidence_list(state: State<'_, Arc<AppState>>, memory_id: String) -> AppResult<Vec<Evidence>> {
    memory_v2::evidence::list_for_memory(&state.pool, &memory_id)
}

#[tauri::command]
pub fn memory_evidence_add(
    state: State<'_, Arc<AppState>>,
    memory_id: String,
    source_kind: String,
    source_id: Option<String>,
    source_location: Option<String>,
    excerpt: Option<String>,
) -> AppResult<Evidence> {
    let e = memory_v2::evidence::add(
        &state.pool,
        &memory_id,
        &source_kind,
        source_id.as_deref(),
        source_location.as_deref(),
        excerpt.as_deref(),
    )?;
    audit_log(&state, "user", "memory.evidence.add", Some(&memory_id), serde_json::json!({"source_kind": source_kind}));
    Ok(e)
}

#[tauri::command]
pub fn memory_evidence_verify(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    memory_v2::evidence::verify(&state.pool, &id)
}

#[tauri::command]
pub fn memory_evidence_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    memory_v2::evidence::delete(&state.pool, &id)
}

// ---------------------------- candidates ----------------------------------

#[tauri::command]
pub fn memory_candidates_list(
    state: State<'_, Arc<AppState>>,
    status: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<Candidate>> {
    let status = status.as_deref().unwrap_or("pending");
    memory_v2::candidates::list(&state.pool, status, limit.unwrap_or(100))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposeCandidateRequest {
    pub category: String,
    pub content: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_excerpt: Option<String>,
    pub confidence: f64,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
}

#[tauri::command]
pub fn memory_candidates_propose(
    state: State<'_, Arc<AppState>>,
    request: ProposeCandidateRequest,
) -> AppResult<Candidate> {
    let c = memory_v2::candidates::propose(
        &state.pool,
        &request.category,
        &request.content,
        &request.source_kind,
        request.source_id.as_deref(),
        request.source_excerpt.as_deref(),
        request.confidence,
        request.project_id.as_deref(),
        request.conversation_id.as_deref(),
    )?;
    audit_log(&state, "user", "memory.candidate.propose", Some(&c.id), serde_json::json!({"category": c.category}));
    Ok(c)
}

#[tauri::command]
pub fn memory_candidates_approve(state: State<'_, Arc<AppState>>, id: String) -> AppResult<MemoryV2> {
    let m = memory_v2::candidates::approve(&state.pool, &id)?;
    audit_log(&state, "user", "memory.candidate.approve", Some(&id), serde_json::json!({"memory_id": m.id}));
    Ok(m)
}

#[tauri::command]
pub fn memory_candidates_reject(
    state: State<'_, Arc<AppState>>,
    id: String,
    reason: Option<String>,
) -> AppResult<()> {
    memory_v2::candidates::reject(&state.pool, &id, reason.as_deref())?;
    audit_log(&state, "user", "memory.candidate.reject", Some(&id), serde_json::json!({}));
    Ok(())
}

// ---------------------------- sessions ------------------------------------

#[tauri::command]
pub fn memory_sessions_list(
    state: State<'_, Arc<AppState>>,
    project_id: String,
    limit: Option<i64>,
) -> AppResult<Vec<Session>> {
    memory_v2::session::list_for_project(&state.pool, &project_id, limit.unwrap_or(50))
}

#[tauri::command]
pub fn memory_sessions_get(state: State<'_, Arc<AppState>>, id: String) -> AppResult<Session> {
    memory_v2::session::get(&state.pool, &id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub workspace_id: Option<String>,
    pub project_id: Option<String>,
    pub conversation_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub work_completed: Option<String>,
    pub decisions_made: Option<String>,
    pub files_changed: Option<String>,
    pub experiments_performed: Option<String>,
    pub errors_encountered: Option<String>,
    pub open_questions: Option<String>,
    pub next_actions: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
}

#[tauri::command]
pub fn memory_sessions_create(
    state: State<'_, Arc<AppState>>,
    request: CreateSessionRequest,
) -> AppResult<Session> {
    let now = chrono::Utc::now().timestamp_millis();
    let s = Session {
        id: memory_v2::session::new_id(),
        workspace_id: request.workspace_id,
        project_id: request.project_id,
        conversation_id: request.conversation_id,
        title: request.title,
        summary: request.summary,
        work_completed: request.work_completed,
        decisions_made: request.decisions_made,
        files_changed: request.files_changed,
        experiments_performed: request.experiments_performed,
        errors_encountered: request.errors_encountered,
        open_questions: request.open_questions,
        next_actions: request.next_actions,
        approved: true,
        started_at: request.started_at,
        ended_at: request.ended_at,
        created_at: now,
    };
    memory_v2::session::insert(&state.pool, &s)?;
    audit_log(&state, "user", "memory.session.create", Some(&s.id), serde_json::json!({"title": s.title}));
    Ok(s)
}

#[tauri::command]
pub fn memory_sessions_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    memory_v2::session::delete(&state.pool, &id)?;
    audit_log(&state, "user", "memory.session.delete", Some(&id), serde_json::json!({}));
    Ok(())
}
