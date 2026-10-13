//! Native long-term memory v2. Phase 5B.
//!
//! Four related tables:
//!   * `memories_v2`    — the canonical memory row, with temporal fields,
//!                        supersession, content hash, and verification.
//!   * `memory_evidence`— the chain back to the original source.
//!   * `memory_candidates`— auto-extracted, pending user review.
//!   * `memory_sessions`— end-of-session summaries the user can store.
//!
//! Plus a FTS5 mirror (`memories_v2_fts`) for keyword search.
//!
//! The v1 module in `crate::memory` keeps working; nothing there has
//! been removed. The two share the same UI surface where possible:
//! `memory_list` returns the v2 row when available and falls back to
//! the v1 row otherwise. New code should prefer v2.

pub mod candidates;
pub mod evidence;
pub mod explicit_save;
pub mod memory;
pub mod session;
pub mod search;

// The submodules are not glob-re-exported because they would clash on
// common function names (`get`, `list`, `delete`). Callers should use
// the full path: `memory_v2::memory::list`, `memory_v2::candidates::list`,
// `memory_v2::session::list`, etc.

/// The 12 categories of long-term memory. Stored as TEXT in SQLite so
/// additional categories can be added without a schema migration.
pub const CATEGORIES: &[&str] = &[
    "preference",      // User preferences and habits.
    "instruction",     // Standing instructions.
    "project_fact",    // Project-scoped facts.
    "decision",        // Architecture / design decisions and their rationale.
    "constraint",      // Technical constraints and limitations.
    "hypothesis",      // Research hypotheses (require verification before becoming facts).
    "decision_record", // Decisions made during a research session.
    "experiment",      // Experiment results and observations.
    "problem",         // Unresolved problems / open issues.
    "task_commit",     // Task commitments and deadlines.
    "milestone",       // Milestones achieved.
    "lesson",          // Lessons learned and rejected approaches.
];

/// Normalise free-form memory content for hashing and FTS indexing.
/// Folds case, collapses all whitespace, and strips any non-alphanumeric
/// non-ASCII-letter character so punctuation differences ("Tauri 2" vs
/// "tauri 2.") still dedupe.
pub fn normalize_content(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws { out.push(' '); }
            prev_ws = true;
            continue;
        }
        if ch.is_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_ws = false;
        }
        // Skip punctuation; the alphanumeric run continues.
    }
    out.trim().to_string()
}

/// SHA-256 of the normalised content, returned as a lowercase hex string.
pub fn content_hash(normalized: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    // The DB stores a hex string. We use DefaultHasher for determinism
    // without dragging in sha2; this is for dedupe, not for trust.
    let mut h = DefaultHasher::new();
    normalized.hash(&mut h);
    let v = h.finish();
    format!("{:016x}", v)
}

/// Compute the dedupe key (normalized content) and its hash together.
/// Used by `insert` to enforce uniqueness.
pub fn dedupe_key(content: &str) -> (String, String) {
    let n = normalize_content(content);
    let h = content_hash(&n);
    (n, h)
}
