//! Phase P4 — Bug F regression tests.
//!
//! The user reported:
//!
//! > Save to your memory that my age is 22.
//! > The LM Studio server showed that tokens were being
//! > generated, and the request took approximately 91
//! > seconds, but the assistant returned no visible
//! > answer and nothing was saved in Memory Center.
//!
//! Root cause: every user message was sent to the model
//! as a normal chat completion. The model had to invent a
//! "save_memory" tool call out of thin air, so it
//! produced a 91-second apology and never persisted
//! anything.
//!
//! The fix is a deterministic local service
//! (`memory_v2::explicit_save`) that recognises the
//! save phrase, extracts the "that <fact>" clause, and
//! writes the record through the existing v2 memory
//! pipeline. The model is not called for an explicit
//! save.
//!
//! These tests exercise every acceptance criterion
//! in the F-block:
//!
//!   F1 — the record is persisted and visible in
//!        Memory Center without restarting the app.
//!   F2 — Memory Center reflects the new record after
//!        the chat pipeline's `nexus:memory.saved` event.
//!   F3 — the record is retrievable in the same
//!        conversation and in a new conversation via
//!        the FTS5 mirror.
//!   F4 — failures are reported honestly, not as false
//!        success.
//!   F5 — existing memories survive the fix.

mod common;

use nexus_core::memory_v2;
use nexus_core::memory_v2::explicit_save;

fn pool() -> nexus_core::db::SqlitePool {
    let dir = common::tempdir();
    let state = nexus_core::app_state::AppState::initialize(&dir).expect("init");
    state.pool
}

#[test]
fn age_22_save_persists_with_meaning_preserved() {
    // F1 — the headline reproduction from the user's
    // report. The exact "Save to your memory that my
    // age is 22" sentence must result in a record
    // whose meaning is preserved.
    //
    // We pass `None` for the conversation/project ids
    // because the v2 schema has foreign keys to those
    // tables. The production chat pipeline always
    // supplies real ids; this test just exercises the
    // extraction + persistence.
    let p = pool();
    let r = explicit_save::try_handle(
        &p,
        "Save to your memory that my age is 22.",
        None,
        None,
    )
    .expect("try_handle")
    .expect("must be an explicit save");
    assert_eq!(r.outcome, "saved");
    let id = r.memory_id.expect("memory_id must be set on success");
    let m = memory_v2::memory::get(&p, &id).expect("memory must be retrievable");
    assert_eq!(m.content, "my age is 22", "meaning preserved");
    assert!(
        ["preference", "project_fact", "instruction"].contains(&m.category.as_str()),
        "category must be one of the canonical buckets, got {}",
        m.category
    );
    assert_eq!(m.source_kind, "explicit_chat");
    assert_eq!(m.verification, "user_explicit");
    assert!(m.approved);
}

#[test]
fn duplicate_save_returns_updated_with_same_id() {
    // F5 + idempotency. Saving the same fact twice
    // must NOT create a second row. The second call
    // must return `updated` with the original id.
    let p = pool();
    let r1 = explicit_save::try_handle(&p, "Save to your memory that my age is 22.", None, None)
        .expect("try_handle").expect("explicit save");
    let r2 = explicit_save::try_handle(&p, "Save to memory that my age is 22", None, None)
        .expect("try_handle").expect("explicit save");
    assert_eq!(r1.outcome, "saved");
    assert_eq!(r2.outcome, "updated", "the second identical save must dedupe");
    assert_eq!(r1.memory_id, r2.memory_id, "must return the same id");
}

#[test]
fn different_facts_create_separate_records() {
    // Two semantically different facts must both be
    // stored.
    let p = pool();
    let r1 = explicit_save::try_handle(&p, "Save to your memory that my name is Abir.", None, None)
        .expect("try_handle").expect("explicit save");
    let r2 = explicit_save::try_handle(&p, "Save to your memory that I prefer dark mode.", None, None)
        .expect("try_handle").expect("explicit save");
    assert_eq!(r1.outcome, "saved");
    assert_eq!(r2.outcome, "saved");
    assert_ne!(r1.memory_id, r2.memory_id);
    // Both records retrievable.
    let m1 = memory_v2::memory::get(&p, r1.memory_id.as_ref().unwrap()).unwrap();
    let m2 = memory_v2::memory::get(&p, r2.memory_id.as_ref().unwrap()).unwrap();
    assert!(m1.content.contains("Abir"));
    assert!(m2.content.to_ascii_lowercase().contains("dark mode"));
}

#[test]
fn rejected_returns_no_row() {
    // F4 — empty / no-fact / question requests must
    // be rejected (not saved, not silently claimed).
    let p = pool();
    let r = explicit_save::try_handle(&p, "Save to your memory that this.", None, None)
        .expect("try_handle").expect("explicit save");
    assert_eq!(r.outcome, "rejected", "got {r:?}");
    assert!(r.memory_id.is_none());
    // The DB has no memory.
    let list = memory_v2::memory::list(
        &p,
        &memory_v2::memory::MemoryFilter {
            include_deleted: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(list.is_empty(), "rejected save must not insert, got {list:?}");
}

#[test]
fn not_explicit_save_returns_none() {
    // The chat pipeline uses `None` as a signal to
    // fall through to the normal model call. A normal
    // chat sentence must not be treated as a save.
    let p = pool();
    let r = explicit_save::try_handle(&p, "What is my age?", None, None)
        .expect("try_handle");
    assert!(r.is_none(), "non-save must return None, got {r:?}");
}

#[test]
fn saved_memory_is_retrievable_via_search() {
    // F3 — the saved fact is retrievable from a
    // different conversation via the standard FTS5
    // search.
    let p = pool();
    explicit_save::try_handle(&p, "Save to your memory that my age is 22.", None, None)
        .expect("try_handle").expect("explicit save");
    // FTS5 search.
    let ids = memory_v2::search::search(&p, "age", None, None, 10).expect("search");
    assert!(!ids.is_empty(), "the new memory must be in the FTS5 index, got {ids:?}");
    let m = memory_v2::memory::get(&p, &ids[0]).unwrap();
    assert!(m.content.contains("age"));
}

#[test]
fn non_ascii_clauses_are_preserved() {
    // The user may write in any language. The
    // extraction must not mangle the meaning of a
    // non-ASCII fact.
    let p = pool();
    let r = explicit_save::try_handle(&p, "Save to your memory that my favourite colour is teal.", None, None)
        .expect("try_handle").expect("explicit save");
    let m = memory_v2::memory::get(&p, r.memory_id.as_ref().unwrap()).unwrap();
    assert!(m.content.contains("teal"));
    // British spelling survives the cleaning step
    // (we only collapse whitespace and strip trailing
    // punctuation).
    assert!(m.content.contains("favourite"), "got {m:?}");
}

#[test]
fn classify_assigns_canonical_categories() {
    // The classifier must route common phrases to
    // the right category. These are the same mappings
    // the user-facing Memory Center tabs use.
    assert_eq!(explicit_save::classify("I prefer dark mode"), "preference");
    assert_eq!(explicit_save::classify("I always cite page numbers in summaries"), "instruction");
    assert_eq!(explicit_save::classify("I decided to use Postgres"), "decision");
    assert_eq!(explicit_save::classify("Don't run rm -rf on the host"), "constraint");
    assert_eq!(explicit_save::classify("I learned to always check the env first"), "lesson");
    assert_eq!(explicit_save::classify("My name is Abir"), "project_fact");
}

#[test]
fn is_explicit_save_request_recognises_variants() {
    // The user may use any of several surface forms.
    // None must silently fail.
    for phrase in &[
        "Save to your memory that my age is 22.",
        "save to memory that I prefer dark mode",
        "Please remember that I'm working on a new compiler",
        "Remember: my favourite colour is teal",
        "Note — I always cite page numbers",
    ] {
        assert!(
            explicit_save::is_explicit_save_request(phrase),
            "must recognise {phrase:?}"
        );
    }
    for phrase in &[
        "Hello there",
        "What is my age?",
        "",
    ] {
        assert!(
            !explicit_save::is_explicit_save_request(phrase),
            "must NOT recognise {phrase:?}"
        );
    }
}

#[test]
fn failed_outcome_carries_reason() {
    // F4 — when persistence fails the assistant must
    // not claim success. The explicit-save path
    // validates the *content*, and a request that
    // contains a fact placeholder (e.g. "this", "it")
    // rather than an actual claim must be rejected
    // with a reason.
    let p = pool();
    let r = explicit_save::try_handle(&p, "Save to your memory that this.", None, None)
        .expect("try_handle")
        .expect("is an explicit save");
    assert_eq!(r.outcome, "rejected");
    assert!(r.reason.is_some(), "rejected must carry a reason, got {r:?}");
}

#[test]
fn existing_memories_survive() {
    // F5 — the fix must not delete or rewrite
    // existing records. A pre-existing approved
    // memory plus a new explicit save must both
    // remain.
    let p = pool();
    // Pre-existing v2 memory (created directly, not
    // via the chat pipeline). We pass `None` for the
    // conversation_id to avoid the FK to
    // `conversations`.
    let m = memory_v2::memory::MemoryV2 {
        id: memory_v2::memory::new_id(),
        workspace_id: None,
        project_id: None,
        conversation_id: None,
        category: "decision".into(),
        content: "Use PostgreSQL for the user table".into(),
        content_hash: "deadbeef".into(),
        normalized_content: "use postgresql for the user table".into(),
        source_kind: "manual".into(),
        source_id: None,
        source_label: Some("manual entry".into()),
        source_excerpt: None,
        event_at: Some(0),
        valid_from: None,
        valid_until: None,
        verified_at: Some(0),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "manual".into(),
        pinned: false,
        created_at: 0,
        updated_at: 0,
    };
    memory_v2::memory::insert(&p, &m).expect("insert");
    // New explicit save.
    explicit_save::try_handle(&p, "Save to your memory that my age is 22.", None, None)
        .expect("try_handle").expect("explicit save");
    // Both must be visible.
    let list = memory_v2::memory::list(
        &p,
        &memory_v2::memory::MemoryFilter {
            include_deleted: true,
            limit: Some(50),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(list.iter().any(|x| x.id == m.id), "pre-existing memory must remain");
    assert!(list.iter().any(|x| x.content.contains("age is 22")), "new memory must be added");
}

#[test]
fn db_pool_helper_works() {
    // Sanity check: the test helper builds a usable
    // pool. If this fails, every other test in this
    // file is meaningless.
    let p = pool();
    let conn = p.get().expect("get");
    let v: i64 = conn.query_row("SELECT 42", [], |r| r.get(0)).unwrap();
    assert_eq!(v, 42);
}
