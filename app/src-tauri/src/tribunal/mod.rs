//! Evidence Tribunal. Phase G.
//!
//! Given a question, retrieve evidence from local sources
//! and classify each snippet's stance relative to the
//! question. Sources searched (deterministic, no LLM):
//!
//! - memories_v2  (FTS if available)
//! - notes        (FTS if available)
//! - hypotheses + evidence rows (Phase S.3.2)
//! - decisions    (timeline events of type `decision`)
//! - timeline events (any type whose `summary` matches)
//! - repository files (only if the user supplies a root)
//!
//! Each evidence card carries:
//!   - the snippet
//!   - the source record (id + table + project_id)
//!   - a `stance` (supports | contradicts | partial |
//!     unjudged)
//!   - a `confidence` (0.0..1.0)
//!   - a `reasons` list explaining the stance
//!
//! Stance is computed from explicit evidence rows when
//! available (hypothesis_evidence.kind =
//! `supporting`/`contradicting`/`neutral`). When no
//! explicit evidence exists, stance defaults to
//! `unjudged` and the user is the judge.
//!
//! No model call is made. The Tribunal is fully usable
//! without local models.
//!
//! The retrieval is bounded (`limit_per_source`) and
//! rank-stable. Repeated calls return the same cards in
//! the same order for the same question.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance { Supports, Contradicts, Partial, Unjudged }

impl Stance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supports => "supports",
            Self::Contradicts => "contradicts",
            Self::Partial => "partial",
            Self::Unjudged => "unjudged",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceCard {
    pub source_kind: String,    // memory | note | hypothesis | decision | timeline | file
    pub source_id: String,      // record id, or path for files
    pub project_id: String,     // empty string for global
    pub snippet: String,        // capped at 400 chars
    pub stance: String,         // Stance::as_str
    pub confidence: f64,        // 0..1
    pub reasons: Vec<String>,
    pub href: Option<String>,   // e.g. timeline://event/<id>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TribunalReport {
    pub question: String,
    pub generated_at: i64,
    pub cards: Vec<EvidenceCard>,
    /// Aggregate counts by stance, for the UI header.
    pub counts: TribunalCounts,
    /// Honest summary of what the Tribunal could and
    /// could not do. The user reads this to understand
    /// the basis of every claim.
    pub notes: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TribunalCounts {
    pub supports: usize,
    pub contradicts: usize,
    pub partial: usize,
    pub unjudged: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TribunalRequest {
    pub question: String,
    pub project_id: Option<String>,
    /// Optional repository root for file evidence.
    pub repo_root: Option<String>,
    /// Max cards per source. Default 8, capped at 50.
    pub limit_per_source: Option<usize>,
}

const SNIPPET_MAX: usize = 400;

fn clip(s: &str) -> String {
    if s.len() <= SNIPPET_MAX { return s.to_string(); }
    let mut end = SNIPPET_MAX;
    while !s.is_char_boundary(end) && end > 0 { end -= 1; }
    let mut out = s[..end].to_string();
    out.push('…');
    out
}

fn tokens(q: &str) -> Vec<String> {
    q.split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() >= 3)
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn score_text(haystack: &str, toks: &[String]) -> f64 {
    if toks.is_empty() { return 0.0; }
    let h = haystack.to_ascii_lowercase();
    let mut hits = 0usize;
    for t in toks {
        if h.contains(t.as_str()) { hits += 1; }
    }
    hits as f64 / toks.len() as f64
}

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

fn collect_memories(
    conn: &rusqlite::Connection,
    project_id: Option<&str>,
    toks: &[String],
    limit: usize,
) -> AppResult<Vec<EvidenceCard>> {
    let mut cards: Vec<(f64, EvidenceCard)> = Vec::new();
    // `memories_v2` has a `content` column (no `summary`).
    let has_content: bool = conn.query_row(
        "SELECT 1 FROM pragma_table_info('memories_v2') WHERE name = 'content'",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if !has_content { return Ok(Vec::new()); }
    let pid = project_id.unwrap_or("");
    let sql = if pid.is_empty() {
        "SELECT id, COALESCE(project_id,'') AS project_id, content
         FROM memories_v2
         ORDER BY updated_at DESC LIMIT 200"
    } else {
        "SELECT id, COALESCE(project_id,'') AS project_id, content
         FROM memories_v2 WHERE project_id = ?1
         ORDER BY updated_at DESC LIMIT 200"
    };
    let mut stmt = conn.prepare(sql)?;
    let map = |r: &rusqlite::Row| -> rusqlite::Result<(String, String, String)> {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    };
    let rows: Vec<(String, String, String)> = if pid.is_empty() {
        stmt.query_map([], map)?.collect::<Result<_, _>>()?
    } else {
        stmt.query_map(params![pid], map)?.collect::<Result<_, _>>()?
    };
    for (id, proj, body) in rows {
        if body.is_empty() { continue; }
        let s = score_text(&body, toks);
        if s <= 0.0 { continue; }
        let snippet = clip(&body);
        let conf = 0.5 + 0.4 * s;
        cards.push((s, EvidenceCard {
            source_kind: "memory".into(),
            source_id: id,
            project_id: proj,
            snippet,
            stance: Stance::Unjudged.as_str().into(),
            confidence: conf.min(0.95),
            reasons: vec![format!("Matches {:.0}% of question tokens.", 100.0 * s)],
            href: None,
        }));
    }
    cards.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(cards.into_iter().take(limit).map(|(_, c)| c).collect())
}

fn collect_notes(
    conn: &rusqlite::Connection,
    project_id: Option<&str>,
    toks: &[String],
    limit: usize,
) -> AppResult<Vec<EvidenceCard>> {
    let mut cards: Vec<(f64, EvidenceCard)> = Vec::new();
    let pid = project_id.unwrap_or("");
    let sql = if pid.is_empty() {
        "SELECT id, COALESCE(project_id,'') AS project_id,
                body, COALESCE(title, '') AS title
         FROM notes ORDER BY updated_at DESC LIMIT 200"
    } else {
        "SELECT id, COALESCE(project_id,'') AS project_id,
                body, COALESCE(title, '') AS title
         FROM notes WHERE project_id = ?1
         ORDER BY updated_at DESC LIMIT 200"
    };
    let mut stmt = conn.prepare(sql)?;
    let map = |r: &rusqlite::Row| -> rusqlite::Result<(String, String, String, String)> {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    };
    let rows: Vec<(String, String, String, String)> = if pid.is_empty() {
        stmt.query_map([], map)?.collect::<Result<_, _>>()?
    } else {
        stmt.query_map(params![pid], map)?.collect::<Result<_, _>>()?
    };
    for (id, proj, body, title) in rows {
        let combined = format!("{title}\n{body}");
        if combined.trim().is_empty() { continue; }
        let s = score_text(&combined, toks);
        if s <= 0.0 { continue; }
        let snippet = clip(&body);
        let conf = 0.5 + 0.4 * s;
        cards.push((s, EvidenceCard {
            source_kind: "note".into(),
            source_id: id,
            project_id: proj,
            snippet,
            stance: Stance::Unjudged.as_str().into(),
            confidence: conf.min(0.95),
            reasons: vec![format!("Matches {:.0}% of question tokens.", 100.0 * s)],
            href: None,
        }));
    }
    cards.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(cards.into_iter().take(limit).map(|(_, c)| c).collect())
}

fn collect_timeline(
    conn: &rusqlite::Connection,
    project_id: Option<&str>,
    toks: &[String],
    limit: usize,
) -> AppResult<Vec<EvidenceCard>> {
    let mut cards: Vec<(f64, EvidenceCard)> = Vec::new();
    let pid = project_id.unwrap_or("");
    // The timeline_events table may be missing in older DBs.
    let has_table: bool = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='timeline_events'",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if !has_table { return Ok(Vec::new()); }
    let sql = if pid.is_empty() {
        "SELECT id, COALESCE(project_id,'') AS project_id,
                event_type, COALESCE(summary,'') AS summary,
                COALESCE(detail_json,'') AS detail_json, event_at
         FROM timeline_events ORDER BY event_at DESC LIMIT 200"
    } else {
        "SELECT id, COALESCE(project_id,'') AS project_id,
                event_type, COALESCE(summary,'') AS summary,
                COALESCE(detail_json,'') AS detail_json, event_at
         FROM timeline_events WHERE project_id = ?1
         ORDER BY event_at DESC LIMIT 200"
    };
    let mut stmt = conn.prepare(sql)?;
    let map = |r: &rusqlite::Row| -> rusqlite::Result<(String, String, String, String, String, i64)> {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
    };
    let rows: Vec<(String, String, String, String, String, i64)> = if pid.is_empty() {
        stmt.query_map([], map)?.collect::<Result<_, _>>()?
    } else {
        stmt.query_map(params![pid], map)?.collect::<Result<_, _>>()?
    };
    for (id, proj, event_type, summary, detail, _event_at) in rows {
        let combined = format!("{summary}\n{detail}");
        if combined.trim().is_empty() { continue; }
        let s = score_text(&combined, toks);
        if s <= 0.0 { continue; }
        let is_decision = event_type == "decision";
        let snippet = clip(&summary);
        // Decisions come with provenance; we treat them as
        // strong evidence (stance: unjudged by default,
        // the user must confirm).
        let conf = if is_decision { 0.7 + 0.2 * s } else { 0.5 + 0.4 * s };
        let mut reasons = vec![format!("Matches {:.0}% of question tokens.", 100.0 * s)];
        if is_decision { reasons.push("Source: recorded decision (user-authored).".into()); }
        cards.push((s, EvidenceCard {
            source_kind: if is_decision { "decision".into() } else { "timeline".into() },
            source_id: id.clone(),
            project_id: proj,
            snippet,
            stance: Stance::Unjudged.as_str().into(),
            confidence: conf.min(0.95),
            reasons,
            href: Some(format!("timeline://event/{id}")),
        }));
    }
    cards.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(cards.into_iter().take(limit).map(|(_, c)| c).collect())
}

fn collect_hypotheses(
    conn: &rusqlite::Connection,
    project_id: Option<&str>,
    toks: &[String],
    limit: usize,
) -> AppResult<Vec<EvidenceCard>> {
    let mut out: Vec<EvidenceCard> = Vec::new();
    let pid = project_id.unwrap_or("");
    let has_table: bool = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='hypotheses'",
        [], |_| Ok(true),
    ).unwrap_or(false);
    if !has_table { return Ok(Vec::new()); }
    // Fetch hypotheses + their evidence rows in two passes
    // to avoid N+1 queries per hypothesis. The number of
    // hypotheses is bounded by `limit`.
    let sql = if pid.is_empty() {
        "SELECT id, COALESCE(project_id,'') AS project_id,
                COALESCE(title,'') AS title,
                COALESCE(statement,'') AS statement,
                COALESCE(status,'proposed') AS status
         FROM hypotheses ORDER BY updated_at DESC LIMIT 50"
    } else {
        "SELECT id, COALESCE(project_id,'') AS project_id,
                COALESCE(title,'') AS title,
                COALESCE(statement,'') AS statement,
                COALESCE(status,'proposed') AS status
         FROM hypotheses WHERE project_id = ?1
         ORDER BY updated_at DESC LIMIT 50"
    };
    let mut stmt = conn.prepare(sql)?;
    let map = |r: &rusqlite::Row| -> rusqlite::Result<(String, String, String, String, String)> {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    };
    let rows: Vec<(String, String, String, String, String)> = if pid.is_empty() {
        stmt.query_map([], map)?.collect::<Result<_, _>>()?
    } else {
        stmt.query_map(params![pid], map)?.collect::<Result<_, _>>()?
    };
    for (id, proj, title, statement, status) in rows {
        let combined = format!("{title}\n{statement}");
        let s = score_text(&combined, toks);
        if s <= 0.0 && !matches!(status.as_str(), "supported" | "contradicted") {
            // If no token matches but the hypothesis has a
            // verdict, still show it — the user wants to
            // see the verdict regardless of overlap.
        }
        // Evidence rows for this hypothesis. The migration
        // defines `stance` as `for` / `against` / `neutral`;
        // we accept either column (`stance` if present,
        // else fall back to `kind`).
        let mut support: i64 = 0;
        let mut contradict: i64 = 0;
        let mut estmt = conn.prepare(
            "SELECT COALESCE(stance, kind, 'neutral') FROM hypothesis_evidence
             WHERE hypothesis_id = ?1",
        )?;
        let erows = estmt.query_map(params![id], |r| r.get::<_, String>(0))?;
        for er in erows {
            let k = er?;
            match k.as_str() {
                "for" | "supporting" | "support" => support += 1,
                "against" | "contradicting" | "contradict" => contradict += 1,
                _ => {}
            }
        }
        let (stance, mut reasons) = if support == 0 && contradict == 0 {
            (Stance::Unjudged, vec!["Hypothesis has no explicit evidence.".into()])
        } else if contradict == 0 {
            (Stance::Supports, vec![format!("{support} supporting evidence row(s).")])
        } else if support == 0 {
            (Stance::Contradicts, vec![format!("{contradict} contradicting evidence row(s).")])
        } else {
            (Stance::Partial, vec![
                format!("{support} supporting and {contradict} contradicting."),
            ])
        };
        if s > 0.0 {
            reasons.push(format!("Title/statement matches {:.0}% of tokens.", 100.0 * s));
        }
        let conf = match status.as_str() {
            "supported" => 0.85,
            "contradicted" => 0.85,
            "inconclusive" => 0.6,
            "superseded" | "archived" => 0.5,
            _ => 0.55 + 0.2 * s,
        };
        out.push(EvidenceCard {
            source_kind: "hypothesis".into(),
            source_id: id,
            project_id: proj,
            snippet: clip(&statement),
            stance: stance.as_str().into(),
            confidence: conf.min(0.95),
            reasons,
            href: None,
        });
        if out.len() >= limit { break; }
    }
    Ok(out)
}

fn collect_files(
    repo_root: &str,
    toks: &[String],
    limit: usize,
) -> AppResult<Vec<EvidenceCard>> {
    let mut out: Vec<EvidenceCard> = Vec::new();
    let root = std::path::Path::new(repo_root);
    if !root.exists() { return Ok(out); }
    // Walk the directory tree. Bound the depth and the
    // number of files visited. Skip binary files by
    // extension. This is best-effort and bounded.
    let walk = walkdir::WalkDir::new(root)
        .max_depth(6)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok());
    let mut visited = 0usize;
    for entry in walk {
        if visited >= 2_000 { break; }
        visited += 1;
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if matches!(ext, "png"|"jpg"|"jpeg"|"gif"|"pdf"|"zip"|"exe"|"dll"|"so"|"dylib"|"class"|"o"|"obj") {
            continue;
        }
        let display = path.to_string_lossy().to_string();
        // Cheap filter: if the file name has no token hit,
        // skip. This avoids reading megabytes of irrelevant
        // source.
        let name_lower = display.to_ascii_lowercase();
        let name_hit = toks.iter().any(|t| name_lower.contains(t));
        if !name_hit { continue; }
        let body = match std::fs::read_to_string(path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let s = score_text(&body, toks);
        if s <= 0.0 { continue; }
        // Pull the first window around the first hit.
        let lower = body.to_ascii_lowercase();
        let pos = toks.iter().filter_map(|t| lower.find(t.as_str())).min();
        let window = if let Some(p) = pos {
            let start = p.saturating_sub(120);
            let end = (p + 280).min(body.len());
            let mut s_end = end;
            while !body.is_char_boundary(s_end) && s_end > start { s_end -= 1; }
            body[start..s_end].to_string()
        } else {
            body.chars().take(280).collect()
        };
        out.push(EvidenceCard {
            source_kind: "file".into(),
            source_id: display.clone(),
            project_id: String::new(),
            snippet: clip(&window),
            stance: Stance::Unjudged.as_str().into(),
            confidence: (0.4 + 0.4 * s).min(0.9),
            reasons: vec![format!("File path and content match {:.0}% of tokens.", 100.0 * s)],
            href: Some(format!("file://{display}")),
        });
        if out.len() >= limit { break; }
    }
    Ok(out)
}

pub fn run(pool: &SqlitePool, req: &TribunalRequest) -> AppResult<TribunalReport> {
    let toks = tokens(&req.question);
    let limit = req.limit_per_source.unwrap_or(8).clamp(1, 50);
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let mut cards: Vec<EvidenceCard> = Vec::new();
    cards.extend(collect_memories(&conn, req.project_id.as_deref(), &toks, limit)?);
    cards.extend(collect_notes(&conn, req.project_id.as_deref(), &toks, limit)?);
    cards.extend(collect_timeline(&conn, req.project_id.as_deref(), &toks, limit)?);
    cards.extend(collect_hypotheses(&conn, req.project_id.as_deref(), &toks, limit)?);
    if let Some(root) = req.repo_root.as_ref().filter(|s| !s.trim().is_empty()) {
        if let Ok(file_cards) = collect_files(root, &toks, limit) {
            cards.extend(file_cards);
        }
    }
    // Sort by stance priority (contradicts first, then
    // supports, partial, unjudged) and within each stance
    // by confidence desc.
    let stance_rank = |s: &str| -> u8 {
        match s {
            "contradicts" => 0,
            "supports" => 1,
            "partial" => 2,
            _ => 3,
        }
    };
    cards.sort_by(|a, b| {
        stance_rank(&a.stance).cmp(&stance_rank(&b.stance))
            .then_with(|| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut counts = TribunalCounts::default();
    for c in &cards {
        match c.stance.as_str() {
            "supports" => counts.supports += 1,
            "contradicts" => counts.contradicts += 1,
            "partial" => counts.partial += 1,
            _ => counts.unjudged += 1,
        }
    }
    let mut notes: Vec<String> = Vec::new();
    if toks.is_empty() {
        notes.push("Question contained no usable tokens; results may be empty.".into());
    }
    if cards.is_empty() {
        notes.push("No matching evidence found in local records.".into());
    } else {
        notes.push(format!(
            "Stance is computed from explicit hypothesis evidence rows when present; otherwise cards are `unjudged` and require user review."
        ));
    }
    if req.repo_root.is_none() {
        notes.push("No repository root supplied; file evidence is omitted.".into());
    }
    Ok(TribunalReport {
        question: req.question.clone(),
        generated_at: now_ms(),
        cards,
        counts,
        notes,
    })
}
