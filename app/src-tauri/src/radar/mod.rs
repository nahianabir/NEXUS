//! Opportunity Radar. S.3.1.
//!
//! Read-only derived view. Given a project, the radar returns
//! a list of "opportunities" the user might want to look at:
//! overdue tasks, stale open tasks, recently-resumed
//! checkpoints with pending actions, high-importance recent
//! memories, and open hypotheses (if the hypothesis table
//! exists in this build).
//!
//! The radar never writes to any table. It never executes a
//! network call. The function is deterministic given a fixed
//! snapshot of the database — the same data, the same output.
//!
//! Scoring is intentionally simple. Each opportunity has a
//! `score` (a non-negative number, higher = more urgent) and
//! a `kind` so the UI can group / colour-code it. The score
//! is the sum of a small set of typed weights; the test suite
//! pins those weights exactly so the radar's output is stable.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::AppResult;

pub mod weights;

pub use weights::Weights;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpportunityKind {
    OverdueUrgentTask,
    OverdueHighTask,
    StaleOpenTask,
    ResumedCheckpointWithPending,
    HighImportanceRecentMemory,
    OpenHypothesis,
}

impl OpportunityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OverdueUrgentTask => "overdue_urgent_task",
            Self::OverdueHighTask => "overdue_high_task",
            Self::StaleOpenTask => "stale_open_task",
            Self::ResumedCheckpointWithPending => "resumed_checkpoint_with_pending",
            Self::HighImportanceRecentMemory => "high_importance_recent_memory",
            Self::OpenHypothesis => "open_hypothesis",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityRef {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    pub kind: OpportunityKind,
    pub title: String,
    pub summary: String,
    pub score: f64,
    pub refs: Vec<OpportunityRef>,
    pub suggested_action: String,
    /// Optional numeric anchor, e.g. days overdue, days stale.
    pub metric: Option<f64>,
    /// Stable feedback key, derived from `kind` and the first
    /// ref's id. The radar consults this key against the
    /// `radar_feedback` table to apply dismiss / snooze /
    /// pin / done / false rules.
    pub key: String,
}

/// Top-level radar response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarReport {
    pub project_id: String,
    pub generated_at: i64,
    pub items: Vec<Opportunity>,
    pub counts: RadarCounts,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RadarCounts {
    pub open_tasks: i64,
    pub overdue_tasks: i64,
    pub resumed_checkpoints: i64,
    pub open_hypotheses: i64,
    pub high_importance_memories: i64,
}

#[derive(Debug, Clone)]
pub struct RadarOptions {
    /// Days of inactivity before an open task is "stale". Default 7.
    pub stale_days: i64,
    /// Days of recency for a "recent" memory. Default 14.
    pub memory_recent_days: i64,
    /// Importance threshold for "high importance". Default 0.7.
    pub high_importance_threshold: f64,
    /// Cap on items returned per kind. Default 25.
    pub per_kind_limit: usize,
    /// Weights used to score opportunities.
    pub weights: Weights,
}

impl Default for RadarOptions {
    fn default() -> Self {
        Self {
            stale_days: 7,
            memory_recent_days: 14,
            high_importance_threshold: 0.7,
            per_kind_limit: 25,
            weights: Weights::default(),
        }
    }
}

/// Build a radar report for a project.
pub fn report(
    pool: &SqlitePool,
    project_id: &str,
    opts: &RadarOptions,
) -> AppResult<RadarReport> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut items: Vec<Opportunity> = Vec::new();
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;

    // 1. Overdue tasks (priority urgent/high, due in the past).
    let mut overdue_stmt = conn.prepare(
        "SELECT id, title, due_at, priority,
                CAST((?1 - due_at) AS REAL) / 86400000.0 AS days_overdue
         FROM tasks
         WHERE project_id = ?2
           AND status IN ('open','in_progress')
           AND due_at IS NOT NULL
           AND due_at < ?1
         ORDER BY days_overdue DESC
         LIMIT ?3",
    )?;
    let overdue_rows = overdue_stmt.query_map(
        params![now, project_id, opts.per_kind_limit as i64],
        |r| {
            let id: String = r.get(0)?;
            let title: String = r.get(1)?;
            let due_at: i64 = r.get(2)?;
            let priority: String = r.get(3)?;
            let days_overdue: f64 = r.get(4)?;
            Ok((id, title, due_at, priority, days_overdue))
        },
    )?;
    for row in overdue_rows {
        let (id, title, _due_at, priority, days_overdue) = row?;
        let kind = if priority == "urgent" {
            OpportunityKind::OverdueUrgentTask
        } else {
            OpportunityKind::OverdueHighTask
        };
        let score = if priority == "urgent" {
            opts.weights.overdue_urgent_task_base
                + opts.weights.overdue_day_factor * days_overdue
        } else {
            opts.weights.overdue_high_task_base
                + opts.weights.overdue_day_factor * days_overdue
        };
        let key = crate::radar_feedback::opportunity_key(kind.as_str(), &id);
        items.push(Opportunity {
            kind,
            title: title.clone(),
            summary: format!("{priority} task overdue by {days_overdue:.1} day(s)."),
            score,
            refs: vec![OpportunityRef { kind: "task".into(), id }],
            suggested_action: "Reassign, defer, or complete this task.".into(),
            metric: Some(days_overdue),
            key,
        });
    }

    // 2. Stale open tasks (no activity in N days).
    let stale_cutoff = now - opts.stale_days * 86_400_000;
    let mut stale_stmt = conn.prepare(
        "SELECT id, title, updated_at, priority,
                CAST((?1 - updated_at) AS REAL) / 86400000.0 AS days_stale
         FROM tasks
         WHERE project_id = ?2
           AND status = 'open'
           AND updated_at < ?3
         ORDER BY days_stale DESC
         LIMIT ?4",
    )?;
    let stale_rows = stale_stmt.query_map(
        params![now, project_id, stale_cutoff, opts.per_kind_limit as i64],
        |r| {
            let id: String = r.get(0)?;
            let title: String = r.get(1)?;
            let updated_at: i64 = r.get(2)?;
            let _priority: String = r.get(3)?;
            let days_stale: f64 = r.get(4)?;
            Ok((id, title, updated_at, days_stale))
        },
    )?;
    for row in stale_rows {
        let (id, title, _updated_at, days_stale) = row?;
        let score = opts.weights.stale_open_task_base
            + opts.weights.stale_day_factor * days_stale;
        let key = crate::radar_feedback::opportunity_key("stale_open_task", &id);
        items.push(Opportunity {
            kind: OpportunityKind::StaleOpenTask,
            title: title.clone(),
            summary: format!("Open task untouched for {days_stale:.1} day(s)."),
            score,
            refs: vec![OpportunityRef { kind: "task".into(), id }],
            suggested_action: "Re-prioritise, schedule, or close.".into(),
            metric: Some(days_stale),
            key,
        });
    }

    // 3. Resumed checkpoints with pending actions.
    //    A "pending" action is one whose `done == false`.
    //    We read pending_actions_json from the row.
    let mut ck_stmt = conn.prepare(
        "SELECT id, title, pending_actions_json
         FROM checkpoints
         WHERE project_id = ?1
           AND status = 'resumed'
         ORDER BY resumed_at DESC
         LIMIT ?2",
    )?;
    let ck_rows = ck_stmt.query_map(
        params![project_id, opts.per_kind_limit as i64],
        |r| {
            let id: String = r.get(0)?;
            let title: String = r.get(1)?;
            let raw: String = r.get(2)?;
            Ok((id, title, raw))
        },
    )?;
    for row in ck_rows {
        let (id, title, raw) = row?;
        let pending: Vec<serde_json::Value> =
            serde_json::from_str(&raw).unwrap_or_default();
        let open_count = pending.iter()
            .filter(|p| p.get("done").and_then(|v| v.as_bool()).unwrap_or(false) == false)
            .count();
        if open_count == 0 { continue; }
        let score = opts.weights.resumed_checkpoint_base
            + opts.weights.resumed_pending_factor * (open_count as f64);
        let key = crate::radar_feedback::opportunity_key("resumed_checkpoint_with_pending", &id);
        items.push(Opportunity {
            kind: OpportunityKind::ResumedCheckpointWithPending,
            title: title.clone(),
            summary: format!("Resumed with {open_count} pending action(s)."),
            score,
            refs: vec![OpportunityRef { kind: "checkpoint".into(), id }],
            suggested_action: "Open the checkpoint and tick off pending actions.".into(),
            metric: Some(open_count as f64),
            key,
        });
    }

    // 4. Recent high-importance memories.
    //    Only if the `memories_v2` table has an `importance`
    //    column AND an `updated_at` / `created_at` column.
    //    We inspect the table once and silently skip the kind
    //    if the column is missing (older DBs).
    let memory_recent_cutoff = now - opts.memory_recent_days * 86_400_000;
    let has_importance: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('memories_v2')
             WHERE name = 'importance' LIMIT 1",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    let has_updated_at: bool = conn
        .query_row(
            "SELECT 1 FROM pragma_table_info('memories_v2')
             WHERE name = 'updated_at' LIMIT 1",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if has_importance && has_updated_at {
        let mut mem_stmt = conn.prepare(
            "SELECT id, summary, importance, updated_at
             FROM memories_v2
             WHERE project_id = ?1
               AND importance >= ?2
               AND updated_at >= ?3
             ORDER BY importance DESC, updated_at DESC
             LIMIT ?4",
        )?;
        let mem_rows = mem_stmt.query_map(
            params![project_id, opts.high_importance_threshold, memory_recent_cutoff, opts.per_kind_limit as i64],
            |r| {
                let id: String = r.get(0)?;
                let summary: String = r.get(1)?;
                let importance: f64 = r.get(2)?;
                Ok((id, summary, importance))
            },
        )?;
        for row in mem_rows {
            let (id, summary, importance) = row?;
            let score = opts.weights.high_importance_memory_base
                + opts.weights.high_importance_factor * importance;
            let key = crate::radar_feedback::opportunity_key("high_importance_recent_memory", &id);
            items.push(Opportunity {
                kind: OpportunityKind::HighImportanceRecentMemory,
                title: summary.clone(),
                summary: format!("Recent memory at importance {importance:.2}."),
                score,
                refs: vec![OpportunityRef { kind: "memory".into(), id }],
                suggested_action: "Review this memory — it may have downstream implications.".into(),
                metric: Some(importance),
                key,
            });
        }
    }

    // 5. Open hypotheses (if the table exists).
    let has_hypotheses: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='hypotheses'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if has_hypotheses {
        let mut h_stmt = conn.prepare(
            "SELECT id, title, statement, confidence
             FROM hypotheses
             WHERE project_id = ?1
               AND status = 'open'
             ORDER BY confidence DESC, updated_at DESC
             LIMIT ?2",
        )?;
        let h_rows = h_stmt.query_map(
            params![project_id, opts.per_kind_limit as i64],
            |r| {
                let id: String = r.get(0)?;
                let title: String = r.get(1)?;
                let statement: String = r.get(2)?;
                let confidence: f64 = r.get(3)?;
                Ok((id, title, statement, confidence))
            },
        )?;
        for row in h_rows {
            let (id, title, statement, confidence) = row?;
            let score = opts.weights.open_hypothesis_base
                + opts.weights.open_hypothesis_confidence_factor * (1.0 - confidence);
            let key = crate::radar_feedback::opportunity_key("open_hypothesis", &id);
            items.push(Opportunity {
                kind: OpportunityKind::OpenHypothesis,
                title: title.clone(),
                summary: format!("Open: {statement} (confidence {confidence:.2})."),
                score,
                refs: vec![OpportunityRef { kind: "hypothesis".into(), id }],
                suggested_action: "Gather evidence to confirm or refute.".into(),
                metric: Some(confidence),
                key,
            });
        }
    }

    // Aggregate counts. These are read independently so the UI
    // can show a header even when `items` is empty.
    let open_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE project_id = ?1 AND status = 'open'",
            params![project_id], |r| r.get(0),
        ).unwrap_or(0);
    let overdue_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE project_id = ?1
               AND status IN ('open','in_progress')
               AND due_at IS NOT NULL AND due_at < ?2",
            params![project_id, now], |r| r.get(0),
        ).unwrap_or(0);
    let resumed_checkpoints: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM checkpoints
             WHERE project_id = ?1 AND status = 'resumed'",
            params![project_id], |r| r.get(0),
        ).unwrap_or(0);
    let open_hypotheses: i64 = if has_hypotheses {
        conn.query_row(
            "SELECT COUNT(*) FROM hypotheses
             WHERE project_id = ?1 AND status = 'open'",
            params![project_id], |r| r.get(0),
        ).unwrap_or(0)
    } else { 0 };
    let high_importance_memories: i64 = if has_importance && has_updated_at {
        conn.query_row(
            "SELECT COUNT(*) FROM memories_v2
             WHERE project_id = ?1
               AND importance >= ?2
               AND updated_at >= ?3",
            params![project_id, opts.high_importance_threshold, memory_recent_cutoff],
            |r| r.get(0),
        ).unwrap_or(0)
    } else { 0 };

    // Apply feedback: exclude dismissed / done / false
    // opportunities, and unexpired snoozes. Then sort with
    // pinned opportunities first, then by score desc, then
    // by title for stability.
    let (exclude, pinned) = crate::radar_feedback::rules_for(pool, project_id, now)?;
    items.retain(|o| !exclude.contains(&o.key));
    items.sort_by(|a, b| {
        let ap = if pinned.contains(&a.key) { 1 } else { 0 };
        let bp = if pinned.contains(&b.key) { 1 } else { 0 };
        bp.cmp(&ap)
            .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.title.cmp(&b.title))
    });

    Ok(RadarReport {
        project_id: project_id.to_string(),
        generated_at: now,
        items,
        counts: RadarCounts {
            open_tasks,
            overdue_tasks,
            resumed_checkpoints,
            open_hypotheses,
            high_importance_memories,
        },
    })
}
