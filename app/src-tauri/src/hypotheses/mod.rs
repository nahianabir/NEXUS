//! Hypothesis Ledger. S.3.2.
//!
//! A "hypothesis" is a structured claim the user (or an agent)
//! makes that can be confirmed, refuted, or abandoned. The
//! ledger is fully additive: it never modifies any existing
//! table. References to source records are stored as small
//! structured JSON arrays of `{kind, id, summary, weight}`
//! entries, so the ledger is independent of any specific
//! source table. When the user deletes a hypothesis, only the
//! `hypotheses` row and its `hypothesis_evidence` rows are
//! touched; the source records (tasks, notes, memories) are
//! not.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisStatus { Open, Confirmed, Refuted, Abandoned }

impl HypothesisStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Confirmed => "confirmed",
            Self::Refuted => "refuted",
            Self::Abandoned => "abandoned",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "open" => Ok(Self::Open),
            "confirmed" => Ok(Self::Confirmed),
            "refuted" => Ok(Self::Refuted),
            "abandoned" => Ok(Self::Abandoned),
            other => Err(AppError::Invalid(format!("hypothesis status: {other}"))),
        }
    }
    /// Is this a terminal status (no more evidence expected)?
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Confirmed | Self::Refuted | Self::Abandoned)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance { For, Against }

impl Stance {
    pub fn as_str(self) -> &'static str {
        match self { Self::For => "for", Self::Against => "against" }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "for" => Ok(Self::For),
            "against" => Ok(Self::Against),
            other => Err(AppError::Invalid(format!("stance: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceEntry {
    pub kind: String,
    pub id: String,
    pub summary: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub statement: String,
    pub prediction: Option<String>,
    pub status: String,
    pub confidence: f64,
    pub evidence_for: Vec<EvidenceEntry>,
    pub evidence_against: Vec<EvidenceEntry>,
    pub decided_at: Option<i64>,
    pub decision_note: Option<String>,
    pub memory_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisEvidence {
    pub id: String,
    pub hypothesis_id: String,
    pub kind: String,
    pub summary: String,
    pub weight: f64,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub stance: String,
    pub created_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

fn clamp_confidence(c: f64) -> f64 {
    if c.is_nan() { 0.5 } else { c.clamp(0.0, 1.0) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHypothesis {
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

pub fn create(pool: &SqlitePool, req: &CreateHypothesis) -> AppResult<Hypothesis> {
    if req.title.trim().is_empty() {
        return Err(AppError::Invalid("hypothesis title is required".into()));
    }
    if req.statement.trim().is_empty() {
        return Err(AppError::Invalid("hypothesis statement is required".into()));
    }
    let now = chrono::Utc::now().timestamp_millis();
    let id = new_id();
    let confidence = clamp_confidence(req.confidence);
    let for_json = serde_json::to_string(&req.evidence_for).unwrap_or_else(|_| "[]".into());
    let against_json = serde_json::to_string(&req.evidence_against).unwrap_or_else(|_| "[]".into());
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO hypotheses
            (id, project_id, task_id, title, statement, prediction,
             status, confidence, evidence_for_json, evidence_against_json,
             memory_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?8, ?9, ?10, ?11, ?11)",
        params![
            id, req.project_id, req.task_id, req.title, req.statement, req.prediction,
            confidence, for_json, against_json, req.memory_id, now,
        ],
    )?;
    get(pool, &id)
}

pub fn get(pool: &SqlitePool, id: &str) -> AppResult<Hypothesis> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let row = conn.query_row(
        "SELECT id, project_id, task_id, title, statement, prediction,
                status, confidence, evidence_for_json, evidence_against_json,
                decided_at, decision_note, memory_id, created_at, updated_at
         FROM hypotheses WHERE id = ?1",
        params![id],
        |r| {
            let for_raw: String = r.get(8)?;
            let against_raw: String = r.get(9)?;
            Ok(Hypothesis {
                id: r.get(0)?,
                project_id: r.get(1)?,
                task_id: r.get(2)?,
                title: r.get(3)?,
                statement: r.get(4)?,
                prediction: r.get(5)?,
                status: r.get(6)?,
                confidence: r.get(7)?,
                evidence_for: serde_json::from_str(&for_raw).unwrap_or_default(),
                evidence_against: serde_json::from_str(&against_raw).unwrap_or_default(),
                decided_at: r.get(10)?,
                decision_note: r.get(11)?,
                memory_id: r.get(12)?,
                created_at: r.get(13)?,
                updated_at: r.get(14)?,
            })
        },
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("hypothesis {id}")),
        other => AppError::Db(other),
    })?;
    Ok(row)
}

#[derive(Debug, Default, Clone)]
pub struct ListFilter {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

pub fn list(pool: &SqlitePool, filter: &ListFilter) -> AppResult<Vec<Hypothesis>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from(
        "SELECT id, project_id, task_id, title, statement, prediction,
                status, confidence, evidence_for_json, evidence_against_json,
                decided_at, decision_note, memory_id, created_at, updated_at
         FROM hypotheses WHERE 1=1",
    );
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
    sql.push_str(" ORDER BY updated_at DESC");
    let lim = filter.limit.unwrap_or(100).clamp(1, 500);
    sql.push_str(" LIMIT ?"); args.push(Box::new(lim as i64));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        let for_raw: String = r.get(8)?;
        let against_raw: String = r.get(9)?;
        Ok(Hypothesis {
            id: r.get(0)?,
            project_id: r.get(1)?,
            task_id: r.get(2)?,
            title: r.get(3)?,
            statement: r.get(4)?,
            prediction: r.get(5)?,
            status: r.get(6)?,
            confidence: r.get(7)?,
            evidence_for: serde_json::from_str(&for_raw).unwrap_or_default(),
            evidence_against: serde_json::from_str(&against_raw).unwrap_or_default(),
            decided_at: r.get(10)?,
            decision_note: r.get(11)?,
            memory_id: r.get(12)?,
            created_at: r.get(13)?,
            updated_at: r.get(14)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

#[derive(Debug, Default, Clone)]
pub struct UpdateHypothesis {
    pub title: Option<String>,
    pub statement: Option<String>,
    pub prediction: Option<Option<String>>,
    pub confidence: Option<f64>,
    pub evidence_for: Option<Vec<EvidenceEntry>>,
    pub evidence_against: Option<Vec<EvidenceEntry>>,
    pub memory_id: Option<Option<String>>,
}

pub fn update(pool: &SqlitePool, id: &str, req: &UpdateHypothesis) -> AppResult<Hypothesis> {
    let now = chrono::Utc::now().timestamp_millis();
    let for_json = req.evidence_for.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let against_json = req.evidence_against.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".into()));
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let confidence = req.confidence.map(clamp_confidence);
    conn.execute(
        "UPDATE hypotheses SET
            title = COALESCE(?1, title),
            statement = COALESCE(?2, statement),
            prediction = CASE WHEN ?3 = 0 THEN prediction WHEN ?3 = 1 THEN NULL WHEN ?3 = 2 THEN ?4 END,
            confidence = COALESCE(?5, confidence),
            evidence_for_json = COALESCE(?6, evidence_for_json),
            evidence_against_json = COALESCE(?7, evidence_against_json),
            memory_id = CASE WHEN ?8 = 0 THEN memory_id WHEN ?8 = 1 THEN NULL WHEN ?8 = 2 THEN ?9 END,
            updated_at = ?10
         WHERE id = ?11",
        params![
            req.title,
            req.statement,
            tri_state(req.prediction.as_ref()),
            req.prediction.as_ref().and_then(|o| o.as_deref()),
            confidence,
            for_json,
            against_json,
            tri_state(req.memory_id.as_ref()),
            req.memory_id.as_ref().and_then(|o| o.as_deref()),
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

/// Decide a hypothesis. Writes `decided_at` and `decision_note`,
/// transitions status, and refuses to overwrite a terminal
/// decision.
pub fn decide(
    pool: &SqlitePool,
    id: &str,
    decision: HypothesisStatus,
    note: Option<String>,
) -> AppResult<Hypothesis> {
    let now = chrono::Utc::now().timestamp_millis();
    let h = get(pool, id)?;
    if HypothesisStatus::parse(&h.status)?.is_terminal() {
        return Err(AppError::Invalid(format!(
            "hypothesis {id} is already in terminal status '{}'", h.status
        )));
    }
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute(
        "UPDATE hypotheses
         SET status = ?1, decided_at = ?2, decision_note = ?3, updated_at = ?2
         WHERE id = ?4",
        params![decision.as_str(), now, note, id],
    )?;
    if n == 0 { return Err(AppError::NotFound(format!("hypothesis {id}"))); }
    get(pool, id)
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM hypotheses WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("hypothesis {id}"))); }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEvidence {
    pub hypothesis_id: String,
    pub kind: String,
    pub summary: String,
    pub weight: f64,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub stance: String,
}

pub fn add_evidence(pool: &SqlitePool, req: &AddEvidence) -> AppResult<HypothesisEvidence> {
    let stance = Stance::parse(&req.stance)?;
    let weight = clamp_confidence(req.weight);
    if req.summary.trim().is_empty() {
        return Err(AppError::Invalid("evidence summary is required".into()));
    }
    let now = chrono::Utc::now().timestamp_millis();
    let id = new_id();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO hypothesis_evidence
            (id, hypothesis_id, kind, summary, weight, source_kind, source_id, stance, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id, req.hypothesis_id, req.kind, req.summary, weight,
            req.source_kind, req.source_id, stance.as_str(), now,
        ],
    )?;
    get_evidence(pool, &id)
}

pub fn get_evidence(pool: &SqlitePool, id: &str) -> AppResult<HypothesisEvidence> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, hypothesis_id, kind, summary, weight, source_kind, source_id, stance, created_at
         FROM hypothesis_evidence WHERE id = ?1",
        params![id],
        |r| Ok(HypothesisEvidence {
            id: r.get(0)?,
            hypothesis_id: r.get(1)?,
            kind: r.get(2)?,
            summary: r.get(3)?,
            weight: r.get(4)?,
            source_kind: r.get(5)?,
            source_id: r.get(6)?,
            stance: r.get(7)?,
            created_at: r.get(8)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("evidence {id}")),
        other => AppError::Db(other),
    })
}

pub fn list_evidence(pool: &SqlitePool, hypothesis_id: &str) -> AppResult<Vec<HypothesisEvidence>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, hypothesis_id, kind, summary, weight, source_kind, source_id, stance, created_at
         FROM hypothesis_evidence
         WHERE hypothesis_id = ?1
         ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![hypothesis_id], |r| Ok(HypothesisEvidence {
        id: r.get(0)?,
        hypothesis_id: r.get(1)?,
        kind: r.get(2)?,
        summary: r.get(3)?,
        weight: r.get(4)?,
        source_kind: r.get(5)?,
        source_id: r.get(6)?,
        stance: r.get(7)?,
        created_at: r.get(8)?,
    }))?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn delete_evidence(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let n = conn.execute("DELETE FROM hypothesis_evidence WHERE id = ?1", params![id])?;
    if n == 0 { return Err(AppError::NotFound(format!("evidence {id}"))); }
    Ok(())
}
