//! Tauri commands for Phase S.3.
//!
//! Sub-features, each behind its own command prefix:
//!
//!   * `radar_*`           — Opportunity Radar
//!   * `hypotheses_*`      — Hypothesis Ledger
//!   * `archaeologist_*`   — Codebase Archaeologist
//!   * `serendipity_*`     — Serendipity (read-only)
//!   * `impact_*`          — Impact Simulator (read-only)
//!
//! Every write wraps an `audit::log` call. The radar, the
//! archaeologist, the serendipity view, and the impact
//! simulator are read-only and do not write to any table.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::archaeologist as arch;
use nexus_core::audit;
use nexus_core::error::{AppError, AppResult};
use nexus_core::hypotheses::{
    self, AddEvidence, CreateHypothesis, EvidenceEntry, Hypothesis, HypothesisEvidence,
    HypothesisStatus, UpdateHypothesis,
};
use nexus_core::impact::{self, ImpactAction, ImpactReport, ImpactRequest};
use nexus_core::radar::{self, RadarOptions, RadarReport, Weights};
use nexus_core::serendipity::{self, SerendipityOptions, SerendipityReport, SerendipityWeights};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RadarRequest {
    pub project_id: String,
    #[serde(default)]
    pub stale_days: Option<i64>,
    #[serde(default)]
    pub memory_recent_days: Option<i64>,
    #[serde(default)]
    pub high_importance_threshold: Option<f64>,
    #[serde(default)]
    pub per_kind_limit: Option<usize>,
    #[serde(default)]
    pub weights: Option<Weights>,
}

#[tauri::command]
pub fn radar_report(
    state: State<'_, Arc<AppState>>,
    request: RadarRequest,
) -> AppResult<RadarReport> {
    let opts = RadarOptions {
        stale_days: request.stale_days.unwrap_or(7),
        memory_recent_days: request.memory_recent_days.unwrap_or(14),
        high_importance_threshold: request.high_importance_threshold.unwrap_or(0.7),
        per_kind_limit: request.per_kind_limit.unwrap_or(25),
        weights: request.weights.unwrap_or_default(),
    };
    radar::report(&state.pool, &request.project_id, &opts)
}

#[tauri::command]
pub fn radar_counts(
    state: State<'_, Arc<AppState>>,
    project_id: String,
) -> AppResult<radar::RadarCounts> {
    // A lightweight counts-only query. The full `report` is
    // expensive; the counts view is what the sidebar / header
    // needs.
    let opts = RadarOptions::default();
    let r = radar::report(&state.pool, &project_id, &opts)?;
    Ok(r.counts)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListHypothesesRequest {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[tauri::command]
pub fn hypotheses_list(
    state: State<'_, Arc<AppState>>,
    request: ListHypothesesRequest,
) -> AppResult<Vec<Hypothesis>> {
    let filter = hypotheses::ListFilter {
        project_id: request.project_id,
        task_id: request.task_id,
        status: request.status,
        limit: request.limit,
    };
    hypotheses::list(&state.pool, &filter)
}

#[tauri::command]
pub fn hypotheses_get(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<Hypothesis> {
    hypotheses::get(&state.pool, &id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHypothesisRequest {
    pub project_id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub statement: String,
    pub prediction: Option<String>,
    pub confidence: f64,
    pub evidence_for: Vec<EvidenceEntry>,
    pub evidence_against: Vec<EvidenceEntry>,
    pub memory_id: Option<String>,
}

impl From<CreateHypothesisRequest> for CreateHypothesis {
    fn from(r: CreateHypothesisRequest) -> Self {
        Self {
            project_id: r.project_id,
            task_id: r.task_id,
            title: r.title,
            statement: r.statement,
            prediction: r.prediction,
            confidence: r.confidence,
            evidence_for: r.evidence_for,
            evidence_against: r.evidence_against,
            memory_id: r.memory_id,
        }
    }
}

#[tauri::command]
pub fn hypotheses_create(
    state: State<'_, Arc<AppState>>,
    request: CreateHypothesisRequest,
) -> AppResult<Hypothesis> {
    let h = hypotheses::create(&state.pool, &request.into())?;
    let _ = audit::log(
        &state.pool,
        "user",
        "hypothesis.create",
        Some(&h.id),
        serde_json::json!({
            "project_id": h.project_id,
            "title": h.title,
            "confidence": h.confidence,
        }),
    );
    Ok(h)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateHypothesisRequest {
    pub title: Option<String>,
    pub statement: Option<String>,
    pub prediction: Option<Option<String>>,
    pub confidence: Option<f64>,
    pub evidence_for: Option<Vec<EvidenceEntry>>,
    pub evidence_against: Option<Vec<EvidenceEntry>>,
    pub memory_id: Option<Option<String>>,
}

impl From<UpdateHypothesisRequest> for UpdateHypothesis {
    fn from(r: UpdateHypothesisRequest) -> Self {
        Self {
            title: r.title,
            statement: r.statement,
            prediction: r.prediction,
            confidence: r.confidence,
            evidence_for: r.evidence_for,
            evidence_against: r.evidence_against,
            memory_id: r.memory_id,
        }
    }
}

#[tauri::command]
pub fn hypotheses_update(
    state: State<'_, Arc<AppState>>,
    id: String,
    request: UpdateHypothesisRequest,
) -> AppResult<Hypothesis> {
    let h = hypotheses::update(&state.pool, &id, &request.into())?;
    let _ = audit::log(
        &state.pool,
        "user",
        "hypothesis.update",
        Some(&id),
        serde_json::json!({}),
    );
    Ok(h)
}

#[tauri::command]
pub fn hypotheses_decide(
    state: State<'_, Arc<AppState>>,
    id: String,
    decision: String,
    note: Option<String>,
) -> AppResult<Hypothesis> {
    let status = HypothesisStatus::parse(&decision)?;
    let h = hypotheses::decide(&state.pool, &id, status, note)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "hypothesis.decide",
        Some(&id),
        serde_json::json!({ "decision": decision }),
    );
    Ok(h)
}

#[tauri::command]
pub fn hypotheses_delete(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    hypotheses::delete(&state.pool, &id)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "hypothesis.delete",
        Some(&id),
        serde_json::json!({}),
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEvidenceRequest {
    pub hypothesis_id: String,
    pub kind: String,
    pub summary: String,
    pub weight: f64,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub stance: String,
}

impl From<AddEvidenceRequest> for AddEvidence {
    fn from(r: AddEvidenceRequest) -> Self {
        Self {
            hypothesis_id: r.hypothesis_id,
            kind: r.kind,
            summary: r.summary,
            weight: r.weight,
            source_kind: r.source_kind,
            source_id: r.source_id,
            stance: r.stance,
        }
    }
}

#[tauri::command]
pub fn hypotheses_add_evidence(
    state: State<'_, Arc<AppState>>,
    request: AddEvidenceRequest,
) -> AppResult<HypothesisEvidence> {
    let ev = hypotheses::add_evidence(&state.pool, &request.into())?;
    let _ = audit::log(
        &state.pool,
        "user",
        "hypothesis.add_evidence",
        Some(&ev.hypothesis_id),
        serde_json::json!({
            "kind": ev.kind,
            "stance": ev.stance,
            "weight": ev.weight,
        }),
    );
    Ok(ev)
}

#[tauri::command]
pub fn hypotheses_list_evidence(
    state: State<'_, Arc<AppState>>,
    hypothesis_id: String,
) -> AppResult<Vec<HypothesisEvidence>> {
    hypotheses::list_evidence(&state.pool, &hypothesis_id)
}

#[tauri::command]
pub fn hypotheses_delete_evidence(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    hypotheses::delete_evidence(&state.pool, &id)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "hypothesis.delete_evidence",
        Some(&id),
        serde_json::json!({}),
    );
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexRequest {
    pub root: String,
    #[serde(default)]
    pub hash: Option<bool>,
    #[serde(default)]
    pub max_file_bytes: Option<u64>,
    #[serde(default)]
    pub max_files: Option<usize>,
    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default)]
    pub exclusions: Option<Vec<String>>,
}

#[tauri::command]
pub fn archaeologist_index(
    state: State<'_, Arc<AppState>>,
    request: IndexRequest,
) -> AppResult<arch::IndexSummary> {
    let _ = state; // read-only; we just need the state to be available.
    let mut opts = arch::IndexOptions::default();
    if let Some(b) = request.hash { opts.hash = b; }
    if let Some(n) = request.max_file_bytes { opts.max_file_bytes = n; }
    if let Some(n) = request.max_files { opts.max_files = n; }
    if let Some(n) = request.max_depth { opts.max_depth = n; }
    if let Some(xs) = request.exclusions { opts.exclusions = xs; }
    let root = PathBuf::from(&request.root);
    let r = arch::index(&root, &opts)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "archaeologist.index",
        None,
        serde_json::json!({
            "root": r.root,
            "total_files": r.total_files,
            "total_bytes": r.total_bytes,
        }),
    );
    Ok(r)
}

#[tauri::command]
pub fn archaeologist_head(
    state: State<'_, Arc<AppState>>,
    root: String,
    rel_path: String,
    n: Option<usize>,
) -> AppResult<arch::HeadResult> {
    let _ = state;
    let p = arch::resolve_under(&PathBuf::from(&root), &rel_path)?;
    arch::head(&p, n.unwrap_or(200))
}

// ---------------- Serendipity ----------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SerendipityRequest {
    pub project_id: String,
    pub dormant_days: Option<i64>,
    pub per_kind_limit: Option<usize>,
    pub cross_project_limit: Option<usize>,
    pub weights: Option<SerendipityWeights>,
}

#[tauri::command]
pub fn serendipity_report(
    state: State<'_, Arc<AppState>>,
    request: SerendipityRequest,
) -> AppResult<SerendipityReport> {
    let opts = SerendipityOptions {
        dormant_days: request.dormant_days.unwrap_or(30),
        per_kind_limit: request.per_kind_limit.unwrap_or(25),
        cross_project_limit: request.cross_project_limit.unwrap_or(10),
        weights: request.weights.unwrap_or_default(),
    };
    serendipity::report(&state.pool, &request.project_id, &opts)
}

// ---------------- Impact Simulator ----------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactCommandRequest {
    pub action: String,
    pub target_id: String,
}

#[tauri::command]
pub fn impact_simulate(
    state: State<'_, Arc<AppState>>,
    request: ImpactCommandRequest,
) -> AppResult<ImpactReport> {
    let action = ImpactAction::parse(&request.action)?;
    let r = impact::simulate(&state.pool, &ImpactRequest {
        action, target_id: request.target_id,
    })?;
    let _ = audit::log(
        &state.pool,
        "user",
        "impact.simulate",
        Some(&r.target_id),
        serde_json::json!({
            "action": r.action.as_str(),
            "counts": r.counts.len(),
            "affected": r.affected.len(),
        }),
    );
    Ok(r)
}

// ---------------- Radar Feedback (Phase F) ----------------

use nexus_core::radar_feedback::{self, FeedbackKind, RadarFeedback, UpsertFeedback};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertRadarFeedbackRequest {
    pub project_id: String,
    pub key: String,
    pub kind: String,
    pub title: Option<String>,
    pub note: Option<String>,
    pub until_ms: Option<i64>,
}

impl From<UpsertRadarFeedbackRequest> for UpsertFeedback {
    fn from(r: UpsertRadarFeedbackRequest) -> Self {
        Self {
            project_id: r.project_id,
            key: r.key,
            kind: FeedbackKind::parse(&r.kind).unwrap_or(FeedbackKind::Dismiss),
            title: r.title,
            note: r.note,
            until_ms: r.until_ms,
        }
    }
}

#[tauri::command]
pub fn radar_feedback_upsert(
    state: State<'_, Arc<AppState>>,
    request: UpsertRadarFeedbackRequest,
) -> AppResult<RadarFeedback> {
    let req: UpsertFeedback = request.into();
    let fb = radar_feedback::upsert(&state.pool, &req)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "radar_feedback.upsert",
        Some(&fb.id),
        serde_json::json!({
            "project_id": fb.project_id,
            "key": fb.key,
            "kind": fb.kind,
        }),
    );
    Ok(fb)
}

#[tauri::command]
pub fn radar_feedback_delete(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<()> {
    radar_feedback::delete(&state.pool, &id)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "radar_feedback.delete",
        Some(&id),
        serde_json::json!({}),
    );
    Ok(())
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ListRadarFeedbackRequest {
    pub project_id: Option<String>,
    pub kind: Option<String>,
    pub limit: Option<usize>,
}

#[tauri::command]
pub fn radar_feedback_list(
    state: State<'_, Arc<AppState>>,
    request: ListRadarFeedbackRequest,
) -> AppResult<Vec<RadarFeedback>> {
    let filter = radar_feedback::ListFilter {
        project_id: request.project_id,
        kind: request.kind,
        limit: request.limit,
    };
    radar_feedback::list(&state.pool, &filter)
}

// ---------------- Evidence Tribunal (Phase G) ----------------

use nexus_core::tribunal::{self, TribunalReport, TribunalRequest};

#[tauri::command]
pub fn evidence_tribunal_run(
    state: State<'_, Arc<AppState>>,
    request: TribunalRequest,
) -> AppResult<TribunalReport> {
    let q = request.question.trim().to_string();
    if q.is_empty() {
        return Err(AppError::Invalid("question is required".into()));
    }
    let report = tribunal::run(&state.pool, &request)?;
    let _ = audit::log(
        &state.pool,
        "user",
        "evidence_tribunal.run",
        None,
        serde_json::json!({
            "question_len": q.len(),
            "project_id": request.project_id,
            "cards": report.cards.len(),
            "supports": report.counts.supports,
            "contradicts": report.counts.contradicts,
        }),
    );
    Ok(report)
}

// ---------------- Knowledge Vault export (Phase J) ----------------

use nexus_core::vault_export::{self, ExportArchive, ExportRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultExportResponse {
    pub json: String,
    pub archive: ExportArchive,
    pub path: String,
}

#[tauri::command]
pub fn vault_export_run(
    state: State<'_, Arc<AppState>>,
    request: ExportRequest,
    dest: String,
) -> AppResult<VaultExportResponse> {
    if dest.trim().is_empty() {
        return Err(AppError::Invalid("destination path is required".into()));
    }
    let archive = vault_export::export(&state.pool, &request)?;
    let json = vault_export::to_pretty_json(&archive)?;
    std::fs::write(&dest, json.as_bytes())
        .map_err(|e| AppError::Internal(format!("write {dest}: {e}")))?;
    let _ = audit::log(
        &state.pool,
        "user",
        "vault.export",
        None,
        serde_json::json!({
            "path": dest,
            "format": archive.manifest.format,
            "schema_version": archive.manifest.schema_version,
            "kinds": archive.manifest.kinds,
            "record_counts": archive.manifest.record_counts,
        }),
    );
    Ok(VaultExportResponse {
        json,
        archive,
        path: dest,
    })
}

#[tauri::command]
pub fn vault_export_inspect(
    state: State<'_, Arc<AppState>>,
    request: ExportRequest,
) -> AppResult<ExportArchive> {
    vault_export::export(&state.pool, &request)
}

