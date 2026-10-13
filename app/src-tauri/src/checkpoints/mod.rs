//! One-Click Work Resume. Phase S.2 of the Continuity Engine.
//!
//! A "checkpoint" is a structured, model-independent record of
//! where work stopped. It contains evidence references (ids into
//! the canonical tables) and small, structured observations
//! (file paths, test results, decisions). It never copies whole
//! documents, chat transcripts, or git repositories. It is
//! read-only against the source tables: editing or deleting a
//! checkpoint never touches tasks, notes, memories, or anything
//! else.
//!
//! Model independence: `model_hint` is a UI hint, not a
//! dependency. The structured fields (evidence refs, observed
//! files, observed tests, decisions) are sufficient to
//! reconstruct the task context with any model. A different
//! model can resume the same checkpoint without any change to
//! the row.
//!
//! Drift detection: at `create` time we capture a small
//! `DriftCheck` snapshot of the world. At `resume_summary` time
//! we re-read the world and produce a `DriftReport` so the user
//! can see what changed since the checkpoint was saved. The
//! drift check is read-only.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub mod drift;

pub use drift::{DriftCheck, DriftReport, FileSnapshot};

/// Lifecycle status of a checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointStatus { Active, Resumed, Archived }

impl CheckpointStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Resumed => "resumed",
            Self::Archived => "archived",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "active" => Ok(Self::Active),
            "resumed" => Ok(Self::Resumed),
            "archived" => Ok(Self::Archived),
            other => Err(AppError::Invalid(format!("checkpoint status: {other}"))),
        }
    }
}

/// A single observed file. We do NOT store contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedFile {
    pub path: String,
    /// Optional human label, e.g. "the schema migration".
    pub label: Option<String>,
    /// Optional hash at checkpoint time, useful for drift detection.
    /// Computed lazily; absent entries are reported as "unknown".
    pub sha256: Option<String>,
    /// Optional size in bytes at checkpoint time.
    pub size_bytes: Option<u64>,
    /// Optional last-modified epoch millis at checkpoint time.
    pub mtime_ms: Option<i64>,
}

/// A single observed test result. We do NOT execute tests; we
/// record the user's stated outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedTest {
    pub name: String,
    /// `'passed' | 'failed' | 'skipped' | 'unknown'`.
    pub outcome: String,
    pub note: Option<String>,
    /// Optional reference to the checkpoint-creation event time.
    pub at_ms: Option<i64>,
}

/// One decision captured at checkpoint time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedDecision {
    pub summary: String,
    pub rationale: Option<String>,
    /// Optional memory id that backs this decision.
    pub memory_id: Option<String>,
}

/// One pending action. Free-form; the UI renders a checkbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingAction {
    pub text: String,
    pub done: bool,
}

/// One error / open problem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedError {
    pub text: String,
    pub at_ms: Option<i64>,
}

/// One constraint the user wants to keep in mind on resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedConstraint {
    pub text: String,
}

/// Evidence reference: a pointer to a record in another table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// `'timeline' | 'memory' | 'note' | 'task' | 'experiment' | 'kg_relationship'`.
    pub kind: String,
    pub id: String,
}

/// The full checkpoint row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub status: String,
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
    pub drift_check: DriftCheck,
    pub created_by: String,
    pub model_hint: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub resumed_at: Option<i64>,
}

pub fn new_id() -> String { Ulid::new().to_string() }

const SELECT_COLS: &str = "id, project_id, task_id, title, status, goal, completed_work, \
                           current_state, next_action_text, observed_files_json, \
                           observed_tests_json, errors_json, decisions_json, \
                           constraints_json, pending_actions_json, evidence_refs_json, \
                           drift_check_json, created_by, model_hint, created_at, \
                           updated_at, resumed_at";

fn row_to_checkpoint(r: &rusqlite::Row<'_>) -> rusqlite::Result<Checkpoint> {
    fn parse_json<T: serde::de::DeserializeOwned + Default>(s: &str) -> T {
        serde_json::from_str(s).unwrap_or_default()
    }
    let observed_files_raw: String = r.get(9)?;
    let observed_tests_raw: String = r.get(10)?;
    let errors_raw: String = r.get(11)?;
    let decisions_raw: String = r.get(12)?;
    let constraints_raw: String = r.get(13)?;
    let pending_raw: String = r.get(14)?;
    let evidence_raw: String = r.get(15)?;
    let drift_raw: String = r.get(16)?;
    Ok(Checkpoint {
        id: r.get(0)?,
        project_id: r.get(1)?,
        task_id: r.get(2)?,
        title: r.get(3)?,
        status: r.get(4)?,
        goal: r.get(5)?,
        completed_work: r.get(6)?,
        current_state: r.get(7)?,
        next_action_text: r.get(8)?,
        observed_files: parse_json(&observed_files_raw),
        observed_tests: parse_json(&observed_tests_raw),
        errors: parse_json(&errors_raw),
        decisions: parse_json(&decisions_raw),
        constraints: parse_json(&constraints_raw),
        pending_actions: parse_json(&pending_raw),
        evidence_refs: parse_json(&evidence_raw),
        drift_check: serde_json::from_str(&drift_raw).unwrap_or_default(),
        created_by: r.get(17)?,
        model_hint: r.get(18)?,
        created_at: r.get(19)?,
        updated_at: r.get(20)?,
        resumed_at: r.get(21)?,
    })
}

/// The user-supplied payload for a new checkpoint. The caller
/// (the UI) provides everything that is human-typed; the
/// `model_hint` and `drift_check` are filled in by the backend
/// when the row is written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCheckpoint {
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

/// Create a checkpoint. Captures the drift snapshot at write time.
/// Returns the new row.
pub fn create(pool: &SqlitePool, req: &CreateCheckpoint) -> AppResult<Checkpoint> {
    if req.title.trim().is_empty() {
        return Err(AppError::Invalid("checkpoint title is required".into()));
    }
    let now = chrono::Utc::now().timestamp_millis();
    let id = new_id();
    // Capture the drift snapshot. The drift module is read-only
    // against source tables.
    let drift = drift::capture(pool, &req.project_id)?;
    let observed_files_json = serde_json::to_string(&req.observed_files).unwrap_or_else(|_| "[]".into());
    let observed_tests_json = serde_json::to_string(&req.observed_tests).unwrap_or_else(|_| "[]".into());
    let errors_json = serde_json::to_string(&req.errors).unwrap_or_else(|_| "[]".into());
    let decisions_json = serde_json::to_string(&req.decisions).unwrap_or_else(|_| "[]".into());
    let constraints_json = serde_json::to_string(&req.constraints).unwrap_or_else(|_| "[]".into());
    let pending_json = serde_json::to_string(&req.pending_actions).unwrap_or_else(|_| "[]".into());
    let evidence_json = serde_json::to_string(&req.evidence_refs).unwrap_or_else(|_| "[]".into());
    let drift_json = serde_json::to_string(&drift).unwrap_or_else(|_| "{}".into());
    let created_by = req.created_by.clone().unwrap_or_else(|| "user".into());
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO checkpoints (
            id, project_id, task_id, title, status, goal, completed_work,
            current_state, next_action_text, observed_files_json,
            observed_tests_json, errors_json, decisions_json,
            constraints_json, pending_actions_json, evidence_refs_json,
            drift_check_json, created_by, model_hint, created_at,
            updated_at, resumed_at
         ) VALUES (
            ?1, ?2, ?3, ?4, 'active', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, NULL
         )",
        params![
            id,
            req.project_id,
            req.task_id,
            req.title,
            req.goal,
            req.completed_work,
            req.current_state,
            req.next_action_text,
            observed_files_json,
            observed_tests_json,
            errors_json,
            decisions_json,
            constraints_json,
            pending_json,
            evidence_json,
            drift_json,
            created_by,
            req.model_hint,
            now,
            now,
        ],
    )?;
    get(pool, &id)
}

/// Fetch a single checkpoint by id. Returns `NotFound` if absent.
pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Checkpoint> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let sql = format!("SELECT {SELECT_COLS} FROM checkpoints WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_checkpoint)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("checkpoint {id}")),
            other => AppError::Db(other),
        })
}

/// List checkpoints, newest first. All fields are optional filters.
#[derive(Debug, Default, Clone)]
pub struct ListFilter {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

pub fn list(pool: &SqlitePool, filter: &ListFilter) -> AppResult<Vec<Checkpoint>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = format!("SELECT {SELECT_COLS} FROM checkpoints WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(p) = &filter.project_id {
        sql.push_str(" AND project_id = ?"); args.push(Box::new(p.clone()));
    }
    if let Some(t) = &filter.task_id {
        sql.push_str(" AND task_id = ?"); args.push(Box::new(t.clone()));
    }
    if let Some(s) = &filter.status {
        sql.push_str(" AND status = ?"); args.push(Box::new(s.clone()));
    }
    sql.push_str(" ORDER BY created_at DESC");
    let lim = filter.limit.unwrap_or(100).clamp(1, 500);
    sql.push_str(" LIMIT ?"); args.push(Box::new(lim as i64));
    let mut stmt = conn.prepare(&sql)?;
    let mapped = stmt.query_map(rusqlite::params_from_iter(args.iter()), row_to_checkpoint)?;
    let mut out = Vec::new();
    for row in mapped { out.push(row?); }
    Ok(out)
}

/// Mutable fields a user can edit on a checkpoint. Status is
/// controlled via the dedicated `set_status` command to keep the
/// audit log clean.
#[derive(Debug, Default, Clone)]
pub struct UpdateCheckpoint {
    pub title: Option<String>,
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

/// Edit a checkpoint. We do **not** allow changing `project_id`,
/// `task_id`, `created_at`, or `drift_check` here; the drift
/// snapshot stays pinned to the moment of creation so future
/// resume calls can compare against it.
pub fn update(pool: &SqlitePool, id: &str, req: &UpdateCheckpoint) -> AppResult<Checkpoint> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Build a dynamic update with COALESCE-friendly values.
    let observed_files_json = req.observed_files.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let observed_tests_json = req.observed_tests.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let errors_json = req.errors.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let decisions_json = req.decisions.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let constraints_json = req.constraints.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let pending_json = req.pending_actions.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let evidence_json = req.evidence_refs.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    conn.execute(
        "UPDATE checkpoints SET
            title             = COALESCE(?1, title),
            goal              = CASE WHEN ?2 = 0 THEN goal  WHEN ?2 = 1 THEN NULL WHEN ?2 = 2 THEN ?3  END,
            completed_work    = CASE WHEN ?4 = 0 THEN completed_work WHEN ?4 = 1 THEN NULL WHEN ?4 = 2 THEN ?5  END,
            current_state     = CASE WHEN ?6 = 0 THEN current_state WHEN ?6 = 1 THEN NULL WHEN ?6 = 2 THEN ?7  END,
            next_action_text  = CASE WHEN ?8 = 0 THEN next_action_text WHEN ?8 = 1 THEN NULL WHEN ?8 = 2 THEN ?9  END,
            observed_files_json   = COALESCE(?10, observed_files_json),
            observed_tests_json   = COALESCE(?11, observed_tests_json),
            errors_json           = COALESCE(?12, errors_json),
            decisions_json        = COALESCE(?13, decisions_json),
            constraints_json      = COALESCE(?14, constraints_json),
            pending_actions_json  = COALESCE(?15, pending_actions_json),
            evidence_refs_json    = COALESCE(?16, evidence_refs_json),
            updated_at = ?17
         WHERE id = ?18",
        params![
            req.title,
            // Tri-state encoding: 0 = leave alone, 1 = clear, 2 = set.
            tri_state(req.goal.as_ref()),
            req.goal.as_ref().and_then(|o| o.as_deref()),
            tri_state(req.completed_work.as_ref()),
            req.completed_work.as_ref().and_then(|o| o.as_deref()),
            tri_state(req.current_state.as_ref()),
            req.current_state.as_ref().and_then(|o| o.as_deref()),
            tri_state(req.next_action_text.as_ref()),
            req.next_action_text.as_ref().and_then(|o| o.as_deref()),
            observed_files_json,
            observed_tests_json,
            errors_json,
            decisions_json,
            constraints_json,
            pending_json,
            evidence_json,
            now,
            id,
        ],
    )?;
    get(pool, id)
}

fn tri_state(o: Option<&Option<String>>) -> i64 {
    match o {
        None => 0,
        Some(None) => 1,
        Some(Some(_)) => 2,
    }
}

/// Change a checkpoint's status. Writes a `resumed_at` timestamp
/// for the `Resumed` transition. Use `Archived` to hide from
/// the active list.
pub fn set_status(pool: &SqlitePool, id: &str, status: CheckpointStatus) -> AppResult<Checkpoint> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = match status {
        CheckpointStatus::Resumed => conn.execute(
            "UPDATE checkpoints SET status = ?1, updated_at = ?2, resumed_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, id],
        )?,
        _ => conn.execute(
            "UPDATE checkpoints SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, id],
        )?,
    };
    if n == 0 { return Err(AppError::NotFound(format!("checkpoint {id}"))); }
    get(pool, id)
}

/// Delete a checkpoint. Cascades to `checkpoint_resumes` via FK.
/// The source records (tasks, notes, memories, ...) are not
/// touched.
pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM checkpoints WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("checkpoint {id}"))); }
    Ok(())
}

/// A persisted record of one resume attempt. The drift report is
/// stored for later inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeRecord {
    pub id: String,
    pub checkpoint_id: String,
    pub resumed_at: i64,
    pub drift_report: DriftReport,
    pub note: Option<String>,
}

pub fn new_resume_id() -> String { Ulid::new().to_string() }

/// Build a resume summary: returns the checkpoint, the current
/// drift report, and the rendered textual summary. The
/// `record_resume` flag (default true) writes a
/// `checkpoint_resumes` row.
pub fn resume_summary(
    pool: &SqlitePool,
    checkpoint_id: &str,
    note: Option<String>,
) -> AppResult<(Checkpoint, DriftReport, String)> {
    let checkpoint = get(pool, checkpoint_id)?;
    let report = drift::compare(pool, &checkpoint)?;
    let summary = render_resume_summary(&checkpoint, &report);
    // Persist a checkpoint_resumes row.
    let now = chrono::Utc::now().timestamp_millis();
    let rid = new_resume_id();
    let drift_json = serde_json::to_string(&report).unwrap_or_else(|_| "{}".into());
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO checkpoint_resumes (id, checkpoint_id, resumed_at, drift_report_json, note)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![rid, checkpoint_id, now, drift_json, note],
    )?;
    // Bump the parent checkpoint's status to 'resumed' and stamp
    // resumed_at. This is a one-way transition: future edits can
    // move it to 'archived' but not back to 'active'.
    conn.execute(
        "UPDATE checkpoints SET status = 'resumed', updated_at = ?1, resumed_at = ?1 WHERE id = ?2",
        params![now, checkpoint_id],
    )?;
    // Re-read the checkpoint to return its updated status.
    let updated = get(pool, checkpoint_id)?;
    Ok((updated, report, summary))
}

/// List prior resume attempts for a checkpoint.
pub fn list_resumes(pool: &SqlitePool, checkpoint_id: &str) -> AppResult<Vec<ResumeRecord>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, checkpoint_id, resumed_at, drift_report_json, note
         FROM checkpoint_resumes WHERE checkpoint_id = ?1
         ORDER BY resumed_at DESC LIMIT 100",
    )?;
    let mapped = stmt.query_map(params![checkpoint_id], |r| {
        let raw: String = r.get(3)?;
        let report: DriftReport = serde_json::from_str(&raw).unwrap_or_default();
        Ok(ResumeRecord {
            id: r.get(0)?,
            checkpoint_id: r.get(1)?,
            resumed_at: r.get(2)?,
            drift_report: report,
            note: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for row in mapped { out.push(row?); }
    Ok(out)
}

/// Render a human-readable resume summary. The summary is
/// deterministic so the UI can test it.
pub fn render_resume_summary(c: &Checkpoint, r: &DriftReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Resuming: {}\n", c.title));
    if let Some(goal) = &c.goal { out.push_str(&format!("Goal: {goal}\n")); }
    if let Some(state) = &c.current_state {
        out.push_str(&format!("State at save time: {state}\n"));
    }
    out.push_str("\nWhat changed since this checkpoint:\n");
    if r.items.is_empty() {
        out.push_str("  - Nothing the drift check can detect.\n");
    } else {
        for item in &r.items {
            out.push_str(&format!("  - [{}] {}\n", item.severity.as_str(), item.summary));
        }
    }
    if !c.observed_files.is_empty() {
        out.push_str("\nFiles observed at save time:\n");
        for f in &c.observed_files {
            out.push_str(&format!("  - {}\n", f.path));
        }
    }
    if let Some(next) = &c.next_action_text {
        out.push_str(&format!("\nRecommended next step:\n  {next}\n"));
    } else if let Some(first_pending) = c.pending_actions.iter().find(|p| !p.done) {
        out.push_str(&format!("\nRecommended next step:\n  {}\n", first_pending.text));
    }
    out
}
