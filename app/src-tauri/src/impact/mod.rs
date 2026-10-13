//! Impact Simulator. S.3.5 of the Continuity Engine.
//!
//! Read-only diff preview. Given a structured `ImpactRequest`,
//! the simulator walks the existing tables and reports what
//! would be affected by the proposed change. The function
//! never writes to any table. It never executes a network
//! call. It is fully deterministic given a fixed snapshot of
//! the database.
//!
//! The supported `ImpactAction` values are:
//!   * `DeleteNote`        — what cites this note?
//!   * `DeleteMemory`      — what references this memory?
//!   * `DeleteTask`        — what depends on this task?
//!   * `DeleteCheckpoint`  — what references this checkpoint?
//!   * `DeleteHypothesis`  — what cites this hypothesis?
//!   * `DeleteProject`     — what would orphan in this project?
//!   * `CompleteTask`      — what pending actions would close?
//!
//! The simulator does NOT validate the existence of the
//! target row. It reports the count of dependents regardless.
//! The caller is expected to first `get` the row (e.g.
//! `notes::get`) if it wants the row's title; the simulator
//! stays free of cross-module imports and only knows about
//! the foreign keys and evidence-ref patterns that exist in
//! the schema.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactAction {
    DeleteNote,
    DeleteMemory,
    DeleteTask,
    DeleteCheckpoint,
    DeleteHypothesis,
    DeleteProject,
    CompleteTask,
}

impl ImpactAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeleteNote => "delete_note",
            Self::DeleteMemory => "delete_memory",
            Self::DeleteTask => "delete_task",
            Self::DeleteCheckpoint => "delete_checkpoint",
            Self::DeleteHypothesis => "delete_hypothesis",
            Self::DeleteProject => "delete_project",
            Self::CompleteTask => "complete_task",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "delete_note" => Ok(Self::DeleteNote),
            "delete_memory" => Ok(Self::DeleteMemory),
            "delete_task" => Ok(Self::DeleteTask),
            "delete_checkpoint" => Ok(Self::DeleteCheckpoint),
            "delete_hypothesis" => Ok(Self::DeleteHypothesis),
            "delete_project" => Ok(Self::DeleteProject),
            "complete_task" => Ok(Self::CompleteTask),
            other => Err(crate::error::AppError::Invalid(format!("impact action: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactRequest {
    pub action: ImpactAction,
    /// The id of the row being acted on. For `delete_project`
    /// the project id; for `complete_task` the task id; etc.
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedRow {
    pub table: String,
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactCount {
    pub table: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub action: ImpactAction,
    pub target_id: String,
    pub generated_at: i64,
    pub counts: Vec<ImpactCount>,
    pub affected: Vec<AffectedRow>,
    pub warnings: Vec<String>,
    /// True when the requested action is destructive and
    /// would change a FK cascade chain.
    pub destructive: bool,
}

const SAMPLE_LIMIT: i64 = 25;

pub fn simulate(pool: &SqlitePool, req: &ImpactRequest) -> AppResult<ImpactReport> {
    let now = chrono::Utc::now().timestamp_millis();
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let mut counts: Vec<ImpactCount> = Vec::new();
    let mut affected: Vec<AffectedRow> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let destructive = matches!(req.action,
        ImpactAction::DeleteNote
        | ImpactAction::DeleteMemory
        | ImpactAction::DeleteTask
        | ImpactAction::DeleteCheckpoint
        | ImpactAction::DeleteHypothesis
        | ImpactAction::DeleteProject
    );

    match req.action {
        ImpactAction::DeleteNote => {
            count_and_list(&conn, "SELECT COUNT(*) FROM timeline_events WHERE source_kind='note' AND source_id=?1",
                params![req.target_id], "timeline_events", &mut counts);
            list_rows(&conn, "SELECT id, 'timeline event ' || coalesce(event_type,'') FROM timeline_events WHERE source_kind='note' AND source_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "timeline_events", &mut affected);
            count_checkpoints_citing(&conn, "note", &req.target_id, &mut counts, &mut affected);
        }
        ImpactAction::DeleteMemory => {
            count_and_list(&conn, "SELECT COUNT(*) FROM timeline_events WHERE source_kind='memory' AND source_id=?1",
                params![req.target_id], "timeline_events", &mut counts);
            list_rows(&conn, "SELECT id, 'timeline event ' || coalesce(event_type,'') FROM timeline_events WHERE source_kind='memory' AND source_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "timeline_events", &mut affected);
            count_checkpoints_citing(&conn, "memory", &req.target_id, &mut counts, &mut affected);
            count_and_list(&conn, "SELECT COUNT(*) FROM hypotheses WHERE memory_id=?1",
                params![req.target_id], "hypotheses", &mut counts);
            list_rows(&conn, "SELECT id, title FROM hypotheses WHERE memory_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "hypotheses", &mut affected);
        }
        ImpactAction::DeleteTask => {
            count_and_list(&conn, "SELECT COUNT(*) FROM timeline_events WHERE source_kind='task' AND source_id=?1",
                params![req.target_id], "timeline_events", &mut counts);
            list_rows(&conn, "SELECT id, 'timeline event' FROM timeline_events WHERE source_kind='task' AND source_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "timeline_events", &mut affected);
            count_and_list(&conn, "SELECT COUNT(*) FROM checkpoints WHERE task_id=?1",
                params![req.target_id], "checkpoints", &mut counts);
            list_rows(&conn, "SELECT id, title FROM checkpoints WHERE task_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "checkpoints", &mut affected);
            count_and_list(&conn, "SELECT COUNT(*) FROM hypotheses WHERE task_id=?1",
                params![req.target_id], "hypotheses", &mut counts);
            list_rows(&conn, "SELECT id, title FROM hypotheses WHERE task_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "hypotheses", &mut affected);
        }
        ImpactAction::DeleteCheckpoint => {
            count_and_list(&conn, "SELECT COUNT(*) FROM checkpoint_resumes WHERE checkpoint_id=?1",
                params![req.target_id], "checkpoint_resumes", &mut counts);
            list_rows(&conn, "SELECT id, 'resume record' FROM checkpoint_resumes WHERE checkpoint_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "checkpoint_resumes", &mut affected);
        }
        ImpactAction::DeleteHypothesis => {
            count_and_list(&conn, "SELECT COUNT(*) FROM hypothesis_evidence WHERE hypothesis_id=?1",
                params![req.target_id], "hypothesis_evidence", &mut counts);
            list_rows(&conn, "SELECT id, 'evidence entry' FROM hypothesis_evidence WHERE hypothesis_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "hypothesis_evidence", &mut affected);
        }
        ImpactAction::DeleteProject => {
            for (table, _col) in [
                ("tasks", "project_id"),
                ("notes", "project_id"),
                ("memories_v2", "project_id"),
                ("checkpoints", "project_id"),
                ("hypotheses", "project_id"),
                ("experiments", "project_id"),
            ] {
                let exists: bool = conn.query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table], |_| Ok(true),
                ).unwrap_or(false);
                if !exists { continue; }
                let n: i64 = conn.query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE project_id=?1"),
                    params![req.target_id], |r| r.get(0),
                ).unwrap_or(0);
                counts.push(ImpactCount { table: table.to_string(), count: n });
                if n > 0 && n <= SAMPLE_LIMIT {
                    list_rows(&conn,
                        &format!("SELECT id, coalesce(title,'') FROM {table} WHERE project_id=?1 LIMIT ?2"),
                        params![req.target_id, SAMPLE_LIMIT], table, &mut affected);
                } else if n > SAMPLE_LIMIT {
                    warnings.push(format!("{n} rows in {table} would be affected; sample omitted."));
                }
            }
        }
        ImpactAction::CompleteTask => {
            // Count how many checkpoints cite this task and how
            // many of them have open pending actions.
            let ck: i64 = conn.query_row(
                "SELECT COUNT(*) FROM checkpoints WHERE task_id=?1",
                params![req.target_id], |r| r.get(0),
            ).unwrap_or(0);
            counts.push(ImpactCount { table: "checkpoints".into(), count: ck });
            list_rows(&conn, "SELECT id, title FROM checkpoints WHERE task_id=?1 LIMIT ?2",
                params![req.target_id, SAMPLE_LIMIT], "checkpoints", &mut affected);
            warnings.push(
                "Completing this task will not auto-close pending checkpoint actions; \
                 review them on the Work Resume page."
                    .into(),
            );
        }
    }

    Ok(ImpactReport {
        action: req.action,
        target_id: req.target_id.clone(),
        generated_at: now,
        counts,
        affected,
        warnings,
        destructive,
    })
}

fn count_and_list<P: rusqlite::Params>(
    conn: &rusqlite::Connection,
    sql: &str,
    args: P,
    table: &str,
    counts: &mut Vec<ImpactCount>,
) {
    let n: i64 = conn.query_row(sql, args, |r| r.get(0)).unwrap_or(0);
    counts.push(ImpactCount { table: table.to_string(), count: n });
}

fn list_rows<P: rusqlite::Params>(
    conn: &rusqlite::Connection,
    sql: &str,
    args: P,
    table: &str,
    affected: &mut Vec<AffectedRow>,
) {
    let exists: bool = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        params![table], |_| Ok(true),
    ).unwrap_or(false);
    if !exists { return; }
    let mut stmt = match conn.prepare(sql) { Ok(s) => s, Err(_) => return };
    let rows = match stmt.query_map(args, |r| {
        let id: String = r.get(0)?;
        let title: String = r.get(1).unwrap_or_default();
        Ok(AffectedRow {
            table: table.to_string(),
            id,
            title,
        })
    }) {
        Ok(r) => r,
        Err(_) => return,
    };
    for r in rows.flatten() {
        affected.push(r);
    }
}

fn count_checkpoints_citing(
    conn: &rusqlite::Connection,
    kind: &str,
    target_id: &str,
    counts: &mut Vec<ImpactCount>,
    affected: &mut Vec<AffectedRow>,
) {
    // evidence_refs_json is a JSON array; we cannot index it,
    // so we walk all checkpoints and filter in Rust. This is
    // bounded by the checkpoint cap.
    let exists: bool = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='checkpoints'",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if !exists { return; }
    let mut stmt = match conn.prepare(
        "SELECT id, title, evidence_refs_json FROM checkpoints",
    ) { Ok(s) => s, Err(_) => return };
    let rows = match stmt.query_map([], |r| {
        let id: String = r.get(0)?;
        let title: String = r.get(1)?;
        let raw: String = r.get(2)?;
        Ok((id, title, raw))
    }) { Ok(r) => r, Err(_) => return };
    let mut n = 0i64;
    for row in rows.flatten() {
        let (cp_id, title, raw) = row;
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
        let mut hit = false;
        if let Some(arr) = parsed.as_array() {
            for v in arr {
                if v.get("kind").and_then(|x| x.as_str()) == Some(kind)
                    && v.get("id").and_then(|x| x.as_str()) == Some(target_id)
                {
                    hit = true;
                    break;
                }
            }
        }
        if hit {
            n += 1;
            if n <= SAMPLE_LIMIT {
                affected.push(AffectedRow {
                    table: "checkpoints".into(),
                    id: cp_id.clone(),
                    title,
                });
            }
        }
    }
    counts.push(ImpactCount {
        table: "checkpoints".into(),
        count: n,
    });
}
