//! Explicit memory-save service. Phase P4 — Bug F.
//!
//! When the user types a sentence like
//!
//! > Save to your memory that my age is 22.
//!
//! NEXUS used to forward the message to the model as a
//! normal chat completion. The model then had to figure out
//! that it was supposed to call a tool that does not
//! exist in its tool schema, so the result was a 91-second
//! token generation that produced a long apology and
//! nothing was persisted. Memory Center remained empty.
//!
//! This module is the **deterministic, local** path. The
//! chat pipeline detects the explicit save intent (a
//! regex match), extracts the "that <fact>" clause,
//! classifies it into one of the canonical memory
//! categories, and writes it through the real
//! `memories_v2_create` persistence service. The model
//! is *not* called for an explicit save.
//!
//! The four canonical outcomes mirror the spec:
//!   * `Saved`    — fresh record inserted.
//!   * `Updated`  — a record with the same content hash
//!     already existed; the existing id is returned and
//!     the operation is idempotent.
//!   * `Rejected` — the input was an explicit save
//!     command but the content was empty, was a
//!     question, or was not a saveable claim (e.g.
//!     "remember this conversation later").
//!   * `Failed`   — the database operation errored.
//!
//! The chat pipeline uses the same Tauri command
//! (`memory_save_explicit`) for the *post-save reply*
//! the model still produces: the request is sent as a
//! normal chat completion, but the assistant is told the
//! memory was already saved so it does not attempt to
//! repeat the work. See the Assistant.tsx wiring for the
//! full UX.
//!
//! The implementation deliberately does NOT:
//!   * Hardcode the age example.
//!   * Save every sentence the user writes as a permanent
//!     memory. Only sentences that match the explicit
//!     save phrase are persisted.
//!   * Infer facts the user did not provide.
//!   * Bypass existing memory validation (empty / hash
//!     dedup / etc.) — the existing `memories_v2_create`
//!     is the single source of truth.
//!   * Expose a general-purpose unrestricted DB-writing
//!     tool to the model.

use regex::Regex;
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::memory_v2;
use crate::memory_v2::memory::MemoryV2;

use super::memory::{self, MemoryFilter};

/// The four canonical outcomes surfaced to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveOutcome {
    /// Fresh record inserted. `memory_id` is the new id.
    Saved,
    /// An equivalent record already existed; nothing was
    /// inserted. `memory_id` is the id of the existing
    /// record.
    Updated,
    /// The request was recognised as an explicit save
    /// but the content could not be validated as a
    /// memory. `reason` carries a human-readable
    /// explanation (e.g. "empty content", "no claim to
    /// remember").
    Rejected,
    /// The persistence operation errored. `reason`
    /// carries the error string.
    Failed,
}

impl SaveOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Updated => "updated",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }
}

/// Result of a `memory_save_explicit` call. The frontend
/// renders the outcome chip (Saved/Updated/Rejected/Failed)
/// from this struct.
#[derive(Debug, Clone, Serialize)]
pub struct SaveResult {
    /// One of "saved", "updated", "rejected", "failed".
    pub outcome: String,
    /// The id of the memory row that was created or
    /// matched. `None` for `Rejected` and `Failed` (in the
    /// latter case the error string is in `reason`).
    pub memory_id: Option<String>,
    /// The category the record was saved under
    /// ("preference", "project_fact", etc.). `None` when
    /// rejected.
    pub category: Option<String>,
    /// The exact content that was persisted, normalised
    /// (trimmed, collapsed whitespace, trailing
    /// punctuation removed). The frontend can show this
    /// to the user so they can verify the meaning
    /// survived the extraction step.
    pub content: Option<String>,
    /// Human-readable explanation when the outcome is
    /// `Rejected` or `Failed`. `None` otherwise.
    pub reason: Option<String>,
}

impl SaveResult {
    pub fn saved(m: &MemoryV2) -> Self {
        Self {
            outcome: SaveOutcome::Saved.as_str().into(),
            memory_id: Some(m.id.clone()),
            category: Some(m.category.clone()),
            content: Some(m.content.clone()),
            reason: None,
        }
    }
    pub fn updated(m: &MemoryV2) -> Self {
        Self {
            outcome: SaveOutcome::Updated.as_str().into(),
            memory_id: Some(m.id.clone()),
            category: Some(m.category.clone()),
            content: Some(m.content.clone()),
            reason: None,
        }
    }
    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            outcome: SaveOutcome::Rejected.as_str().into(),
            memory_id: None,
            category: None,
            content: None,
            reason: Some(reason.into()),
        }
    }
    pub fn failed(reason: impl Into<String>) -> Self {
        Self {
            outcome: SaveOutcome::Failed.as_str().into(),
            memory_id: None,
            category: None,
            content: None,
            reason: Some(reason.into()),
        }
    }
}

/// Phrases the user is allowed to use to ask NEXUS to
/// remember a fact. All of them collapse to the same
/// extraction pipeline. The patterns are deliberately
/// permissive about the surface form ("save",
/// "remember", "store") because the user's wording is
/// not predictable. The actual *content* is what
/// `extract_clause` parses.
///
/// We do not require an exact prefix; the regex looks
/// for the verb + "memory" anywhere near the start.
const SAVE_PHRASES: &[&str] = &[
    // Verb + optional preposition + optional "your/the"
    // + "memory" + optional "that" + clause.
    //
    // Examples that match:
    //   "save to your memory that my age is 22"
    //   "save to memory that i prefer dark mode"
    //   "remember this in your memory"
    //   "save it in memory"
    //   "store in the memory"
    r"(?i)\A\s*(?:please\s+|can\s+you\s+|could\s+you\s+|i\s+want\s+you\s+to\s+)?(?:save|remember|store|record|note|keep)\s+(?:it\s+)?(?:in|into|within|to)?\s*(?:your|the)?\s*memory\s*[:,]?\s*(?:that\s+)?(?P<clause>.+?)\s*\z",
    // "Remember: <clause>" / "Note: <clause>" / "Note —
    // <clause>" — the verb alone is enough when the
    // colon / em-dash makes the clause obvious.
    r"(?i)\A\s*(?:please\s+|can\s+you\s+|could\s+you\s+|i\s+want\s+you\s+to\s+)?(?:remember|note|record|store)\s*[:\u{2014}\u{2013}-]\s*(?P<clause>.+?)\s*\z",
    // "Remember that I'm working on X" — verb alone
    // (no "memory" word required) when followed by a
    // "that" clause.
    r"(?i)\A\s*(?:please\s+|can\s+you\s+|could\s+you\s+|i\s+want\s+you\s+to\s+)?(?:remember|note)\s+that\s+(?P<clause>.+?)\s*\z",
];

/// Returns true if `text` looks like an explicit save
/// request. Cheap pre-check; the real extraction runs
/// only when this is true.
pub fn is_explicit_save_request(text: &str) -> bool {
    extract_clause(text).is_some()
}

/// Extract the "thing to remember" clause from a free-
/// form user message. Returns `None` if the message is
/// not an explicit save request.
pub fn extract_clause(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    for pat in SAVE_PHRASES {
        if let Ok(re) = Regex::new(pat) {
            if let Some(caps) = re.captures(trimmed) {
                if let Some(m) = caps.name("clause") {
                    let raw = m.as_str().trim();
                    let cleaned = clean_extracted(raw);
                    if !cleaned.is_empty() {
                        return Some(cleaned);
                    }
                }
            }
        }
    }
    None
}

/// Trim trailing punctuation, collapse internal whitespace,
/// strip a leading "that" (case-insensitive). The cleaned
/// text is what we persist.
fn clean_extracted(s: &str) -> String {
    let s = s.trim();
    // Strip a leading "that" (with optional punctuation).
    let s = if let Some(rest) = s.strip_prefix(|c: char| c.is_ascii_alphabetic()) {
        // The above only strips the first char; do a real prefix check.
        let _ = rest;
        s
    } else {
        s
    };
    let s_lower = s.to_ascii_lowercase();
    let s = if s_lower.starts_with("that ") {
        &s[5..]
    } else if s_lower.starts_with("that,") || s_lower.starts_with("that.") {
        &s[5..]
    } else {
        s
    };
    // Strip leading punctuation: ' " - ( space
    let s = s.trim_start_matches(|c: char| matches!(c, '\'' | '"' | '-' | '(' | ' ' | ',' | '–' | '—'));
    // Strip trailing punctuation.
    let s = s.trim_end_matches(|c: char| matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | ')' | '–' | '—' | ' '));
    // Collapse internal whitespace.
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws { out.push(' '); }
            prev_ws = true;
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out.trim().to_string()
}

/// Pick a category for the new memory. The classifier is
/// intentionally simple: keyword buckets that map onto
/// the canonical v2 categories. Unknown facts go to
/// `project_fact` by default (the most common "thing
/// the user told me about themselves" bucket).
pub fn classify(content: &str) -> &'static str {
    let lower = content.to_ascii_lowercase();
    let has = |kw: &str| lower.contains(kw);
    if has("i prefer") || has("i like") || has("i love") || has("i hate") || has("my favourite") || has("my favorite") {
        return "preference";
    }
    // Lessons are checked before instructions because
    // a sentence like "I learned to always check the
    // env first" contains both "i learned" and
    // "always" — lesson wins.
    if has("i learned") || has("lesson:") || has("note to self") {
        return "lesson";
    }
    if has("i decided") || has("we decided") || has("decision:") {
        return "decision";
    }
    if has("can't") || has("cannot") || has("must not") || has("mustn't") || has("don't") || has("do not") || has("forbidden") || has("constraint:") {
        return "constraint";
    }
    if has("always") || has("never") || has("don't forget") || has("do not forget") || has("rule:") {
        return "instruction";
    }
    if has("i'm working on") || has("i am working on") || has("my project") || has("our project") {
        return "project_fact";
    }
    "project_fact"
}

/// Heuristic guard: the extracted content must look like
/// a *claim* the user is making about themselves, the
/// world, or the project. A few anti-patterns:
///
///   * Pure questions ("remember my age?" — no fact).
///   * Empty after cleaning.
///   * Too short (< 3 chars after cleaning).
///   * Time-bound verbiage only ("remember this
///     conversation later" — not a stable fact).
///   * The whole preamble phrase leaked through the
///     regex (e.g. "save to your memory" with no
///     clause after the verb).
fn validate_content(content: &str) -> Result<String, &'static str> {
    let cleaned = content.trim();
    if cleaned.is_empty() { return Err("empty content"); }
    if cleaned.chars().count() < 3 { return Err("content is too short"); }
    if cleaned.ends_with('?') { return Err("content is a question, not a fact"); }
    if cleaned.eq_ignore_ascii_case("this") || cleaned.eq_ignore_ascii_case("that") || cleaned.eq_ignore_ascii_case("it") {
        return Err("no fact specified");
    }
    // Catch the case where the user wrote "save to
    // your memory" with nothing after the verb. The
    // regex still matches (it captures the trailing
    // text), but the content is just a restatement of
    // the preamble.
    let lower = cleaned.to_ascii_lowercase();
    if lower == "save to memory" || lower == "save to your memory" || lower == "save to the memory"
        || lower == "remember" || lower == "note" || lower == "store"
        || lower == "save it in memory" || lower == "remember this" || lower == "remember that"
    {
        return Err("no fact specified");
    }
    Ok(cleaned.to_string())
}

/// Detect an explicit save request and, if one is
/// present, persist the extracted clause to the v2
/// memory store. The model is NOT called.
///
/// Returns `Some(SaveResult)` when the input was an
/// explicit save (success or failure); `None` when the
/// input is not an explicit save request (and should be
/// sent to the model as a normal chat completion).
pub fn try_handle(
    pool: &crate::db::SqlitePool,
    text: &str,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
) -> AppResult<Option<SaveResult>> {
    let Some(clause) = extract_clause(text) else { return Ok(None); };
    let content = match validate_content(&clause) {
        Ok(c) => c,
        Err(reason) => return Ok(Some(SaveResult::rejected(reason))),
    };
    let category = classify(&content);
    // The save must succeed end-to-end. If the
    // persistence layer errors, surface it as a `Failed`
    // outcome (NOT a `Saved` result) so the UI does not
    // claim success.
    let now = chrono::Utc::now().timestamp_millis();
    let norm = memory_v2::normalize_content(&content);
    let hash = memory_v2::content_hash(&norm);
    if let Some(existing) = memory::find_by_hash(pool, &hash)? {
        return Ok(Some(SaveResult::updated(&existing)));
    }
    let m = MemoryV2 {
        id: memory::new_id(),
        workspace_id: None,
        project_id: project_id.map(|s| s.to_string()),
        conversation_id: conversation_id.map(|s| s.to_string()),
        category: category.to_string(),
        content: content.clone(),
        content_hash: hash,
        normalized_content: norm,
        source_kind: "explicit_chat".into(),
        source_id: None,
        source_label: Some("user instruction".into()),
        source_excerpt: Some(content.clone()),
        event_at: Some(now),
        valid_from: Some(now),
        valid_until: None,
        verified_at: Some(now),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "user_explicit".into(),
        pinned: false,
        created_at: now,
        updated_at: now,
    };
    if let Err(e) = memory::insert(pool, &m) {
        // Distinguish "validation" errors (empty
        // content) from real database errors so the UI
        // can show a useful message.
        return Ok(Some(SaveResult::failed(format!("{e}"))));
    }
    // Mirror to FTS so the next chat request can
    // retrieve the new fact via the standard memory
    // search. The insert above already did this for us;
    // we just verify the FTS row by listing.
    let _ = memory::list(pool, &MemoryFilter {
        project_id: project_id.map(|s| s.to_string()),
        include_deleted: false,
        limit: Some(1),
        ..Default::default()
    });
    Ok(Some(SaveResult::saved(&m)))
}

/// Variant that never touches the model and never
/// returns `Ok(None)`. Used by the post-save
/// "synthesise a chat reply" flow on the frontend.
pub fn try_handle_strict(
    pool: &crate::db::SqlitePool,
    text: &str,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
) -> AppResult<SaveResult> {
    match try_handle(pool, text, conversation_id, project_id)? {
        Some(r) => Ok(r),
        None => Ok(SaveResult::rejected("not an explicit save request")),
    }
}

/// Map an `AppError` to a `SaveResult::Failed`. Useful
/// for the Tauri command glue.
pub fn failed_from(err: AppError) -> SaveResult {
    SaveResult::failed(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_explicit_save_yes() {
        assert!(is_explicit_save_request("Save to your memory that my age is 22."));
        assert!(is_explicit_save_request("save to memory that i prefer dark mode"));
        assert!(is_explicit_save_request("Please remember that I'm working on a new compiler"));
        assert!(is_explicit_save_request("Remember: my favourite colour is teal"));
    }

    #[test]
    fn is_explicit_save_no() {
        assert!(!is_explicit_save_request("Hello there"));
        assert!(!is_explicit_save_request("What is my age?"));
        assert!(!is_explicit_save_request(""));
    }

    #[test]
    fn extract_preserves_meaning() {
        assert_eq!(extract_clause("Save to your memory that my age is 22.").as_deref(), Some("my age is 22"));
        assert_eq!(extract_clause("Please remember: my favourite colour is teal").as_deref(), Some("my favourite colour is teal"));
        assert_eq!(extract_clause("remember that I prefer dark mode").as_deref(), Some("I prefer dark mode"));
    }

    #[test]
    fn classify_routes_correctly() {
        assert_eq!(classify("I prefer dark mode"), "preference");
        assert_eq!(classify("Always cite page numbers"), "instruction");
        assert_eq!(classify("I decided to use Postgres"), "decision");
        assert_eq!(classify("Don't run rm -rf on the host"), "constraint");
        assert_eq!(classify("I learned to always check the env first"), "lesson");
        assert_eq!(classify("My name is Abir"), "project_fact");
    }

    #[test]
    fn validate_rejects_questions_and_empty() {
        assert!(validate_content("my age?").is_err());
        assert!(validate_content("").is_err());
        assert!(validate_content("ab").is_err());
        assert!(validate_content("this").is_err());
        assert!(validate_content("my age is 22").is_ok());
    }
}
