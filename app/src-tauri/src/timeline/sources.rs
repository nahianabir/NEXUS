//! Source-kind and event-type vocabulary.
//!
//! Keeping these as constants in one place avoids drift between
//! the migration, the Rust ingestors, the Tauri commands, and the
//! TypeScript UI.

/// Source kinds. Matches the values stored in
/// `timeline_events.source_kind` and `memory_evidence.source_kind`.
pub const SOURCE_KINDS: &[&str] = &[
    "audit",
    "task",
    "note",
    "memory",
    "experiment",
    "experiment_run",
    "kg_relationship",
    "agent_invocation",
    "research_paper",
    "research_report",
    "manual",
];

/// Event types. The set is intentionally small; new values are
/// accepted but the UI groups by these.
pub const EVENT_TYPES: &[&str] = &[
    "created",
    "updated",
    "status_changed",
    "completed",
    "approved",
    "superseded",
    "archived",
    "opened",
    "invoked",
    "finished",
    "note",
    "manual",
];

pub fn is_valid_source(s: &str) -> bool { SOURCE_KINDS.contains(&s) }
pub fn is_valid_event(s: &str) -> bool { EVENT_TYPES.contains(&s) }
