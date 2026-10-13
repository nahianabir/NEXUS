//! Serendipity. S.3.4 of the Continuity Engine.
//!
//! Read-only "you have not looked at this in a while" view.
//! For a project, the serendipity module surfaces dormant
//! records — memories, notes, experiments, open hypotheses,
//! archived checkpoints — whose `updated_at` (or `created_at`
//! for archived items) is older than a configurable threshold.
//!
//! A "surprise score" is computed for each surfaced item:
//!
//! ```text
//! score = log2(1 + days_dormant) * (1 + importance_factor)
//! ```
//!
//! where `importance_factor` defaults to `0.0` for notes and
//! experiments, `0.3` for archived checkpoints, `0.5` for
//! open hypotheses, and `importance` (0.0..1.0) for memories
//! when the `memories_v2.importance` column is present.
//!
//! The cross-project panel takes a small sample of the
//! dormant items' text, normalises whitespace, and groups by
//! shared trigram. Pairs of items from different projects
//! that share at least one trigram are reported together.
//! The grouping is bounded so the response stays small
//! (`cross_project_limit`, default 10).
//!
//! The serendipity module never writes to any table. It
//! never executes a network call. The function is
//! deterministic given a fixed snapshot of the database.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::AppResult;

pub mod weights;

pub use weights::SerendipityWeights;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DormantKind {
    DormantMemory,
    DormantNote,
    DormantExperiment,
    ForgottenHypothesis,
    ColdCheckpoint,
}

impl DormantKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DormantMemory => "dormant_memory",
            Self::DormantNote => "dormant_note",
            Self::DormantExperiment => "dormant_experiment",
            Self::ForgottenHypothesis => "forgotten_hypothesis",
            Self::ColdCheckpoint => "cold_checkpoint",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DormantItem {
    pub kind: DormantKind,
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub summary: String,
    pub days_dormant: f64,
    pub score: f64,
    pub ref_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossProjectHit {
    pub left: DormantItem,
    pub right: DormantItem,
    pub shared_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerendipityReport {
    pub project_id: String,
    pub generated_at: i64,
    pub items: Vec<DormantItem>,
    pub cross_project: Vec<CrossProjectHit>,
}

#[derive(Debug, Clone)]
pub struct SerendipityOptions {
    /// Days of inactivity before a record is "dormant". Default 30.
    pub dormant_days: i64,
    /// Cap on items returned per kind. Default 25.
    pub per_kind_limit: usize,
    /// Cap on cross-project pairs returned. Default 10.
    pub cross_project_limit: usize,
    /// Scoring weights.
    pub weights: SerendipityWeights,
}

impl Default for SerendipityOptions {
    fn default() -> Self {
        Self {
            dormant_days: 30,
            per_kind_limit: 25,
            cross_project_limit: 10,
            weights: SerendipityWeights::default(),
        }
    }
}

/// Build a serendipity report for a project.
pub fn report(
    pool: &SqlitePool,
    project_id: &str,
    opts: &SerendipityOptions,
) -> AppResult<SerendipityReport> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut items = Vec::new();
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let dormant_cutoff = now - opts.dormant_days * 86_400_000;

    // Dormant memories.
    let has_importance: bool = conn.query_row(
        "SELECT 1 FROM pragma_table_info('memories_v2')
         WHERE name = 'importance' LIMIT 1",
        [], |_| Ok(true),
    ).unwrap_or(false);
    let has_mem_updated: bool = conn.query_row(
        "SELECT 1 FROM pragma_table_info('memories_v2')
         WHERE name = 'updated_at' LIMIT 1",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if has_mem_updated {
        // The `importance` column is optional. We probe for
        // it and read it only if it exists; older DBs without
        // `importance` get a flat score of 0.
        let mut stmt = if has_importance {
            conn.prepare(
                "SELECT id, content, updated_at, importance
                 FROM memories_v2
                 WHERE project_id = ?1
                   AND updated_at < ?2
                   AND deleted_at IS NULL
                 ORDER BY updated_at ASC
                 LIMIT ?3",
            )?
        } else {
            conn.prepare(
                "SELECT id, content, updated_at
                 FROM memories_v2
                 WHERE project_id = ?1
                   AND updated_at < ?2
                   AND deleted_at IS NULL
                 ORDER BY updated_at ASC
                 LIMIT ?3",
            )?
        };
        let rows = stmt.query_map(
            params![project_id, dormant_cutoff, opts.per_kind_limit as i64],
            |r| {
                let id: String = r.get(0)?;
                let content: String = r.get(1)?;
                let updated_at: i64 = r.get(2)?;
                let importance: f64 = if has_importance { r.get(3)? } else { 0.0 };
                Ok((id, content, updated_at, importance))
            },
        )?;
        for row in rows {
            let (id, content, updated_at, importance) = row?;
            let days = ((now - updated_at) as f64 / 86_400_000.0).max(0.0);
            let score = surprise_score(days, importance, opts.weights.dormant_memory_factor);
            let title = if content.len() > 80 {
                format!("{}…", &content[..80])
            } else { content.clone() };
            items.push(DormantItem {
                kind: DormantKind::DormantMemory,
                id: id.clone(),
                project_id: project_id.to_string(),
                title,
                summary: content,
                days_dormant: days,
                score,
                ref_kind: "memory".into(),
            });
        }
    }

    // Dormant notes.
    let has_notes_updated: bool = conn.query_row(
        "SELECT 1 FROM pragma_table_info('notes')
         WHERE name = 'updated_at' LIMIT 1",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if has_notes_updated {
        let mut stmt = conn.prepare(
            "SELECT id, title, updated_at
             FROM notes
             WHERE project_id = ?1
               AND updated_at < ?2
             ORDER BY updated_at ASC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![project_id, dormant_cutoff, opts.per_kind_limit as i64],
            |r| {
                let id: String = r.get(0)?;
                let title: String = r.get(1)?;
                let updated_at: i64 = r.get(2)?;
                Ok((id, title, updated_at))
            },
        )?;
        for row in rows {
            let (id, title, updated_at) = row?;
            let days = ((now - updated_at) as f64 / 86_400_000.0).max(0.0);
            let score = surprise_score(days, 0.0, opts.weights.dormant_note_factor);
            items.push(DormantItem {
                kind: DormantKind::DormantNote,
                id: id.clone(),
                project_id: project_id.to_string(),
                title: title.clone(),
                summary: title,
                days_dormant: days,
                score,
                ref_kind: "note".into(),
            });
        }
    }

    // Dormant experiments.
    let has_exp: bool = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='experiments'",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if has_exp {
        let has_exp_updated: bool = conn.query_row(
            "SELECT 1 FROM pragma_table_info('experiments')
             WHERE name = 'updated_at' LIMIT 1",
            [], |_| Ok(true),
        ).unwrap_or(false);
        if has_exp_updated {
            let mut stmt = conn.prepare(
                "SELECT id, name, updated_at
                 FROM experiments
                 WHERE project_id = ?1
                   AND updated_at < ?2
                 ORDER BY updated_at ASC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(
                params![project_id, dormant_cutoff, opts.per_kind_limit as i64],
                |r| {
                    let id: String = r.get(0)?;
                    let name: String = r.get(1)?;
                    let updated_at: i64 = r.get(2)?;
                    Ok((id, name, updated_at))
                },
            )?;
            for row in rows {
                let (id, name, updated_at) = row?;
                let days = ((now - updated_at) as f64 / 86_400_000.0).max(0.0);
                let score = surprise_score(days, 0.0, opts.weights.dormant_experiment_factor);
                items.push(DormantItem {
                    kind: DormantKind::DormantExperiment,
                    id: id.clone(),
                    project_id: project_id.to_string(),
                    title: name.clone(),
                    summary: name,
                    days_dormant: days,
                    score,
                    ref_kind: "experiment".into(),
                });
            }
        }
    }

    // Forgotten hypotheses (open, but not updated in N days).
    let has_hyp: bool = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='hypotheses'",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if has_hyp {
        let has_hyp_updated: bool = conn.query_row(
            "SELECT 1 FROM pragma_table_info('hypotheses')
             WHERE name = 'updated_at' LIMIT 1",
            [], |_| Ok(true),
        ).unwrap_or(false);
        if has_hyp_updated {
            let mut stmt = conn.prepare(
                "SELECT id, title, updated_at, confidence
                 FROM hypotheses
                 WHERE project_id = ?1
                   AND status = 'open'
                   AND updated_at < ?2
                 ORDER BY updated_at ASC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(
                params![project_id, dormant_cutoff, opts.per_kind_limit as i64],
                |r| {
                    let id: String = r.get(0)?;
                    let title: String = r.get(1)?;
                    let updated_at: i64 = r.get(2)?;
                    let confidence: f64 = r.get(3)?;
                    Ok((id, title, updated_at, confidence))
                },
            )?;
            for row in rows {
                let (id, title, updated_at, confidence) = row?;
                let days = ((now - updated_at) as f64 / 86_400_000.0).max(0.0);
                // Hypotheses weight by (1 - confidence) so
                // shakier claims float to the top of the list.
                let factor = opts.weights.forgotten_hypothesis_base
                    + opts.weights.forgotten_hypothesis_confidence_factor * (1.0 - confidence);
                let score = surprise_score(days, 0.0, factor);
                items.push(DormantItem {
                    kind: DormantKind::ForgottenHypothesis,
                    id: id.clone(),
                    project_id: project_id.to_string(),
                    title: title.clone(),
                    summary: title,
                    days_dormant: days,
                    score,
                    ref_kind: "hypothesis".into(),
                });
            }
        }
    }

    // Cold checkpoints (archived, older than N days).
    let has_ckp: bool = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='checkpoints'",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if has_ckp {
        let has_ckp_created: bool = conn.query_row(
            "SELECT 1 FROM pragma_table_info('checkpoints')
             WHERE name = 'created_at' LIMIT 1",
            [], |_| Ok(true),
        ).unwrap_or(false);
        if has_ckp_created {
            let mut stmt = conn.prepare(
                "SELECT id, title, created_at
                 FROM checkpoints
                 WHERE project_id = ?1
                   AND status = 'archived'
                   AND created_at < ?2
                 ORDER BY created_at ASC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(
                params![project_id, dormant_cutoff, opts.per_kind_limit as i64],
                |r| {
                    let id: String = r.get(0)?;
                    let title: String = r.get(1)?;
                    let created_at: i64 = r.get(2)?;
                    Ok((id, title, created_at))
                },
            )?;
            for row in rows {
                let (id, title, created_at) = row?;
                let days = ((now - created_at) as f64 / 86_400_000.0).max(0.0);
                let score = surprise_score(days, 0.0, opts.weights.cold_checkpoint_factor);
                items.push(DormantItem {
                    kind: DormantKind::ColdCheckpoint,
                    id: id.clone(),
                    project_id: project_id.to_string(),
                    title: title.clone(),
                    summary: title,
                    days_dormant: days,
                    score,
                    ref_kind: "checkpoint".into(),
                });
            }
        }
    }

    // Sort by score desc, then by id for stability.
    items.sort_by(|a, b| {
        b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    // Cross-project pairs.
    let cross = cross_project_pairs(pool, &items, opts, now)?;

    Ok(SerendipityReport {
        project_id: project_id.to_string(),
        generated_at: now,
        items,
        cross_project: cross,
    })
}

fn surprise_score(days: f64, importance: f64, factor: f64) -> f64 {
    let log_days = (1.0 + days.max(0.0)).log2();
    log_days * (1.0 + factor + importance.max(0.0))
}

fn cross_project_pairs(
    pool: &SqlitePool,
    items: &[DormantItem],
    opts: &SerendipityOptions,
    now: i64,
) -> AppResult<Vec<CrossProjectHit>> {
    if items.is_empty() { return Ok(Vec::new()); }
    // Pull a small sample of recent items from OTHER projects
    // and pair them by shared trigrams.
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let project_id = items.first().map(|i| i.project_id.clone()).unwrap_or_default();
    let mut out: Vec<CrossProjectHit> = Vec::new();
    let dormant_cutoff = now - opts.dormant_days * 86_400_000;
    let mut other_items: Vec<DormantItem> = Vec::new();
    // Dormant notes from other projects.
    let has_notes_updated: bool = conn.query_row(
        "SELECT 1 FROM pragma_table_info('notes')
         WHERE name = 'updated_at' LIMIT 1",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if has_notes_updated {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, title, updated_at
             FROM notes
             WHERE project_id != ?1
               AND updated_at < ?2
             ORDER BY updated_at ASC
             LIMIT 50",
        )?;
        let rows = stmt.query_map(
            params![project_id, dormant_cutoff],
            |r| {
                let id: String = r.get(0)?;
                let project_id: String = r.get(1)?;
                let title: String = r.get(2)?;
                let updated_at: i64 = r.get(3)?;
                Ok((id, project_id, title, updated_at))
            },
        )?;
        for row in rows {
            let (id, project_id, title, updated_at) = row?;
            let days = ((now - updated_at) as f64 / 86_400_000.0).max(0.0);
            other_items.push(DormantItem {
                kind: DormantKind::DormantNote,
                id,
                project_id,
                title: title.clone(),
                summary: title,
                days_dormant: days,
                score: 0.0,
                ref_kind: "note".into(),
            });
        }
    }
    // Pair by trigrams. For each item in `items`, build its
    // trigram set, then scan other_items for shared trigrams.
    // Cap the work at `per_kind_limit * 4` to keep this
    // bounded.
    let limit = (items.len()).min(opts.per_kind_limit * 4);
    for left in items.iter().take(limit) {
        let left_grams = trigrams(&left.summary);
        if left_grams.is_empty() { continue; }
        let mut best: Option<(usize, &DormantItem)> = None;
        for right in &other_items {
            if right.project_id == left.project_id { continue; }
            let right_grams = trigrams(&right.summary);
            let shared = left_grams.intersection(&right_grams).count();
            if shared == 0 { continue; }
            if best.map_or(true, |(s, _)| shared > s) {
                best = Some((shared, right));
            }
        }
        if let Some((shared, right)) = best {
            out.push(CrossProjectHit {
                left: clone_light(left),
                right: clone_light(right),
                shared_count: shared,
            });
        }
        if out.len() >= opts.cross_project_limit { break; }
    }
    Ok(out)
}

fn clone_light(d: &DormantItem) -> DormantItem {
    DormantItem {
        kind: d.kind,
        id: d.id.clone(),
        project_id: d.project_id.clone(),
        title: d.title.clone(),
        summary: d.summary.clone(),
        days_dormant: d.days_dormant,
        score: d.score,
        ref_kind: d.ref_kind.clone(),
    }
}

fn trigrams(s: &str) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let s = s.to_ascii_lowercase();
    let s: String = s.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 3 { return out; }
    for w in chars.windows(3) {
        if w.iter().all(|c| c.is_whitespace()) { continue; }
        out.insert(w.iter().collect::<String>());
    }
    out
}
