//! Routing policy. Phase F.
//!
//! The policy is a small set of explicit rules plus a deterministic
//! scoring function. It does not invent capabilities and it does not
//! silently switch models; every decision is recorded.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use super::capabilities::Capabilities;
use super::lifecycle::{self, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind { General, Coding, Extraction, Embedding, Reranking, Structured, Tool }

impl TaskKind {
    pub fn as_str(self) -> &'static str { match self {
        Self::General => "general",
        Self::Coding => "coding",
        Self::Extraction => "extraction",
        Self::Embedding => "embedding",
        Self::Reranking => "reranking",
        Self::Structured => "structured",
        Self::Tool => "tool",
    } }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub selected: Option<String>,
    pub rejected: Vec<RejectedCandidate>,
    pub reason: String,
    pub fell_back: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectedCandidate {
    pub profile_id: String,
    pub reason: String,
}

pub fn new_id() -> String { Ulid::new().to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub id: String,
    pub task: String,
    pub selected: Option<String>,
    pub fell_back: bool,
    pub reason: String,
    pub created_at: i64,
}

pub fn record(pool: &SqlitePool, task: &str, decision: &RoutingDecision) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = chrono::Utc::now().timestamp_millis();
    let selected = decision.selected.clone();
    conn.execute(
        "INSERT INTO router_decisions (id, task, selected, fell_back, reason, rejected_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            Ulid::new().to_string(), task, selected, decision.fell_back as i64, decision.reason,
            serde_json::to_string(&decision.rejected).unwrap_or_else(|_| "[]".into()),
            now,
        ],
    )?;
    Ok(())
}

pub fn list_recent(pool: &SqlitePool, limit: i64) -> AppResult<Vec<DecisionRecord>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, task, selected, fell_back, reason, created_at
         FROM router_decisions ORDER BY created_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |r| Ok(DecisionRecord {
        id: r.get(0)?,
        task: r.get(1)?,
        selected: r.get(2)?,
        fell_back: r.get::<_, i64>(3)? != 0,
        reason: r.get(4)?,
        created_at: r.get(5)?,
    }))?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub task: TaskKind,
    pub min_context_tokens: i64,
    pub prefer_profile_id: Option<String>,
}

pub fn route(pool: &SqlitePool, req: &RouteRequest) -> AppResult<RoutingDecision> {
    let profiles = crate::model_profiles::list(pool)?;
    let caps = super::capabilities::list_all(pool)?;
    let caps_by_id: std::collections::HashMap<String, Capabilities> = caps.into_iter().map(|c| (c.profile_id.clone(), c)).collect();

    // Reject anything that's not enabled, not capable, or already known-broken.
    let mut candidates: Vec<(String, Capabilities, f64)> = Vec::new();
    let mut rejected: Vec<RejectedCandidate> = Vec::new();

    for profile in &profiles {
        if !profile.enabled {
            rejected.push(RejectedCandidate { profile_id: profile.id.clone(), reason: "profile disabled".into() });
            continue;
        }
        if profile.context_len < req.min_context_tokens {
            rejected.push(RejectedCandidate {
                profile_id: profile.id.clone(),
                reason: format!("context {} < required {}", profile.context_len, req.min_context_tokens),
            });
            continue;
        }
        // Fetch or infer the capabilities for this profile.
        let cap: Capabilities = match caps_by_id.get(&profile.id) {
            Some(c) => c.clone(),
            None => {
                // Auto-infer so a fresh profile is not silently disqualified.
                let c = super::capabilities::infer_from_profile(pool, &profile.id)?;
                super::capabilities::upsert(pool, &c)?;
                rejected.push(RejectedCandidate { profile_id: profile.id.clone(), reason: "capabilities inferred (unverified)".into() });
                c
            }
        };
        // Verify capability match.
        let supports = match req.task {
            TaskKind::General => cap.supports_chat,
            TaskKind::Coding => cap.supports_code,
            TaskKind::Embedding => cap.is_embedding,
            TaskKind::Reranking => cap.is_embedding, // we treat embedders as the reranking default
            TaskKind::Extraction | TaskKind::Structured | TaskKind::Tool => cap.supports_chat,
        };
        if !supports {
            rejected.push(RejectedCandidate { profile_id: profile.id.clone(), reason: "capability mismatch".into() });
            continue;
        }
        let mut score = 0.0;
        // Role alignment.
        match (profile.role.as_str(), req.task) {
            ("general", TaskKind::General) => score += 50.0,
            ("coding", TaskKind::Coding) => score += 60.0,
            ("embedding", TaskKind::Embedding) => score += 80.0,
            ("reranker", TaskKind::Reranking) => score += 80.0,
            _ => {}
        }
        if profile.id == req.prefer_profile_id.clone().unwrap_or_default() { score += 25.0; }
        if cap.verified { score += 5.0; }
        candidates.push((profile.id.clone(), cap.clone(), score));
    }

    // The prefer_profile_id is a hard override: if it is set and
    // supports the task, it wins over role alignment.
    if let Some(prefer_id) = req.prefer_profile_id.as_ref() {
        if let Some(slot) = candidates.iter().position(|(id, _, _)| id == prefer_id) {
            let chosen = candidates.remove(slot);
            let reason = format!("selected {} (preferred override); task={}", chosen.0, req.task.as_str());
            let decision = RoutingDecision { selected: Some(chosen.0), rejected, reason, fell_back: false };
            record(pool, req.task.as_str(), &decision)?;
            return Ok(decision);
        }
    }

    if candidates.is_empty() {
        let decision = RoutingDecision { selected: None, rejected, reason: "no candidates met the task requirements".into(), fell_back: false };
        record(pool, req.task.as_str(), &decision)?;
        return Ok(decision);
    }

    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let picked = candidates[0].clone();
    let picked_id = picked.0;
    let picked_cap = picked.1;
    let _score = picked.2;
    // Resource check: if the picked profile is heavyweight and the
    // active model is something else that is also loaded, record the
    // switch the user (or the live inference loop) must make.
    let lifecycle_status = lifecycle::get(pool, &picked_id).ok();
    let mut reason = format!("selected {picked_id} ({}); task={}", picked_cap.profile_id, req.task.as_str());
    let mut fell_back = false;

    if let Some(s) = lifecycle_status {
        if s.state == State::LoadFailed.as_str() {
            // The picked profile is known-broken. Try the second-best.
            if candidates.len() > 1 {
                let alt = candidates[1].clone();
                reason.push_str(&format!("; first pick failed previously, falling back to {}", alt.0));
                fell_back = true;
                rejected.push(RejectedCandidate { profile_id: picked_id, reason: "previously failed to load".into() });
                let decision = RoutingDecision { selected: Some(alt.0), rejected, reason, fell_back };
                record(pool, req.task.as_str(), &decision)?;
                return Ok(decision);
            }
        }
    }
    let decision = RoutingDecision { selected: Some(picked_id), rejected, reason, fell_back };
    record(pool, req.task.as_str(), &decision)?;
    Ok(decision)
}
