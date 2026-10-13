//! Phase 5B native long-term memory v2. CRUD, dedupe, supersession, FTS5,
//! candidates, evidence, and the v1 → v2 migration.

mod common;

use common::tempdir;
use nexus_core::db;
use nexus_core::memory_v2;
use nexus_core::memory_v2::candidates;
use nexus_core::memory_v2::evidence;
use nexus_core::memory_v2::memory::{MemoryFilter, MemoryV2};
use nexus_core::memory_v2::session;
use nexus_core::projects::projects;
use ulid::Ulid;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn make_memory(content: &str, category: &str, project_id: Option<&str>) -> MemoryV2 {
    let now = chrono::Utc::now().timestamp_millis();
    let norm = memory_v2::normalize_content(content);
    let hash = memory_v2::content_hash(&norm);
    MemoryV2 {
        id: memory_v2::memory::new_id(),
        workspace_id: None,
        project_id: project_id.map(|s| s.to_string()),
        conversation_id: None,
        category: category.into(),
        content: content.into(),
        content_hash: hash,
        normalized_content: norm,
        source_kind: "user".into(),
        source_id: None,
        source_label: None,
        source_excerpt: None,
        event_at: None,
        valid_from: None,
        valid_until: None,
        verified_at: Some(now),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "approved".into(),
        pinned: false,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn memory_v2_crud_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    let m = make_memory("User prefers concise answers.", "preference", None);
    memory_v2::memory::insert(&pool, &m).unwrap();
    let got = memory_v2::memory::get(&pool, &m.id).unwrap();
    assert_eq!(got.content, m.content);
    assert_eq!(got.category, "preference");
    assert!(got.approved);

    // Update the content; FTS must reflect it.
    let updated = memory_v2::memory::update(
        &pool,
        &m.id,
        Some("User prefers short, structured answers."),
        None,
        None,
        None, None, None, None,
    ).unwrap();
    assert!(updated.content.contains("structured"));
    let hits = memory_v2::search::search_fts(&pool, "structured", 10).unwrap();
    assert!(hits.iter().any(|(id, _)| id == &m.id));
    let misses = memory_v2::search::search_fts(&pool, "concise", 10).unwrap();
    assert!(misses.is_empty());
}

#[test]
fn memory_v2_dedupes_on_content_hash() {
    let dir = tempdir();
    let pool = pool(&dir);
    let m1 = make_memory("Always cite page numbers in summaries.", "instruction", None);
    let m2 = make_memory("always cite   page numbers in summaries.", "instruction", None);
    memory_v2::memory::insert(&pool, &m1).unwrap();
    let existing = memory_v2::memory::find_by_hash(&pool, &m2.content_hash).unwrap();
    assert!(existing.is_some(), "dedupe should find m1 when m2 is normalised");
    let norm2 = memory_v2::normalize_content(&m2.content);
    let h2 = memory_v2::content_hash(&norm2);
    assert_eq!(h2, m1.content_hash, "normalization must be consistent");
}

#[test]
fn memory_v2_supersession_chain() {
    let dir = tempdir();
    let pool = pool(&dir);
    let m1 = make_memory("Default embedding model is MiniLM-L6.", "decision", None);
    let m2 = make_memory("Default embedding model is Nomic v1.5.", "decision", None);
    memory_v2::memory::insert(&pool, &m1).unwrap();
    memory_v2::memory::insert(&pool, &m2).unwrap();
    memory_v2::memory::supersede(&pool, &m1.id, &m2.id).unwrap();
    let got = memory_v2::memory::get(&pool, &m1.id).unwrap();
    assert_eq!(got.superseded_by.as_deref(), Some(m2.id.as_str()));
    let filter = MemoryFilter { approved_only: true, include_superseded: false, ..Default::default() };
    let active = memory_v2::memory::list(&pool, &filter).unwrap();
    assert!(active.iter().any(|x| x.id == m2.id));
    assert!(active.iter().all(|x| x.id != m1.id));
    let filter_with_super = MemoryFilter { approved_only: true, include_superseded: true, ..Default::default() };
    let all = memory_v2::memory::list(&pool, &filter_with_super).unwrap();
    assert!(all.iter().any(|x| x.id == m1.id && x.superseded_by.is_some()));
}

#[test]
fn memory_v2_soft_delete_preserves_row() {
    let dir = tempdir();
    let pool = pool(&dir);
    let m = make_memory("to be deleted", "lesson", None);
    memory_v2::memory::insert(&pool, &m).unwrap();
    memory_v2::memory::soft_delete(&pool, &m.id).unwrap();
    let filter = MemoryFilter::default();
    let list = memory_v2::memory::list(&pool, &filter).unwrap();
    assert!(list.iter().all(|x| x.id != m.id));
    let filter_with_deleted = MemoryFilter { include_deleted: true, ..Default::default() };
    let all = memory_v2::memory::list(&pool, &filter_with_deleted).unwrap();
    assert!(all.iter().any(|x| x.id == m.id && x.deleted_at.is_some()));
}

#[test]
fn memory_v2_evidence_chain() {
    let dir = tempdir();
    let pool = pool(&dir);
    let m = make_memory("Use SQLite for storage.", "decision", None);
    memory_v2::memory::insert(&pool, &m).unwrap();
    let e = evidence::add(&pool, &m.id, "document", Some("doc-1"), Some("p.3"), Some("...we will use SQLite...")).unwrap();
    assert!(!e.verified);
    let list = evidence::list_for_memory(&pool, &m.id).unwrap();
    assert_eq!(list.len(), 1);
    evidence::verify(&pool, &e.id).unwrap();
    let list = evidence::list_for_memory(&pool, &m.id).unwrap();
    assert!(list[0].verified);
}

#[test]
fn memory_v2_candidate_lifecycle() {
    let dir = tempdir();
    let pool = pool(&dir);
    let c = candidates::propose(
        &pool, "project_fact", "Project target hardware is a Ryzen 7 7700.",
        "auto", None, None, 0.7, None, None,
    ).unwrap();
    assert_eq!(c.status, "pending");
    let pending = candidates::list(&pool, "pending", 100).unwrap();
    assert_eq!(pending.len(), 1);
    let promoted = candidates::approve(&pool, &c.id).unwrap();
    assert!(promoted.approved);
    assert_eq!(promoted.category, "project_fact");
    let still_pending = candidates::list(&pool, "pending", 100).unwrap();
    assert!(still_pending.is_empty());
}

#[test]
fn memory_v2_candidate_dedup_preserves_existing() {
    let dir = tempdir();
    let pool = pool(&dir);
    let m = make_memory("Project uses Tauri 2.", "project_fact", None);
    memory_v2::memory::insert(&pool, &m).unwrap();
    let c = candidates::propose(
        &pool, "project_fact", "project uses tauri 2",
        "auto", None, None, 0.5, None, None,
    ).unwrap();
    let returned = candidates::approve(&pool, &c.id).unwrap();
    assert_eq!(returned.id, m.id, "approve must return the existing approved row, not create a duplicate");
}

#[test]
fn memory_v2_session_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    let now = chrono::Utc::now().timestamp_millis();
    let project_id = projects::create(&pool, None, "Session Test", None, None, &[]).unwrap().id;
    let s = session::Session {
        id: session::new_id(),
        workspace_id: None,
        project_id: Some(project_id.clone()),
        conversation_id: None,
        title: "Phase 5B session".into(),
        summary: "Wired memories_v2, evidence, candidates, sessions.".into(),
        work_completed: Some("Added 0006 migration; v2 module; commands; page.".into()),
        decisions_made: Some("Keep v1 surface intact; new code uses v2.".into()),
        files_changed: Some("migrations/0006, src/memory_v2/*, commands/memory_v2.rs, pages/MemoryCenter.tsx".into()),
        experiments_performed: None,
        errors_encountered: Some("Initial parse error: `Statement` cannot be dereferenced; fixed.".into()),
        open_questions: None,
        next_actions: Some("Begin MemPalace adapter; then knowledge graph.".into()),
        approved: true,
        started_at: now - 60_000,
        ended_at: now,
        created_at: now,
    };
    session::insert(&pool, &s).unwrap();
    let list = session::list_for_project(&pool, &project_id, 50).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, s.id);
}

#[test]
fn memory_v2_list_for_context_respects_project_and_pin() {
    let dir = tempdir();
    let pool = pool(&dir);
    let project_id = projects::create(&pool, None, "Context Test", None, None, &[]).unwrap().id;
    let a = make_memory("pinned fact", "instruction", Some(&project_id));
    let mut pinned = a.clone();
    pinned.pinned = true;
    memory_v2::memory::insert(&pool, &pinned).unwrap();
    let b = make_memory("regular fact", "lesson", Some(&project_id));
    memory_v2::memory::insert(&pool, &b).unwrap();
    let c = make_memory("unrelated", "lesson", None);
    memory_v2::memory::insert(&pool, &c).unwrap();

    let ctx = memory_v2::memory::list_for_context(&pool, Some(&project_id), None, 10).unwrap();
    let ids: Vec<&str> = ctx.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&pinned.id.as_str()));
    assert!(ids.contains(&b.id.as_str()));
    assert!(!ids.contains(&c.id.as_str()));
    // Pinned comes first.
    assert_eq!(ctx.first().unwrap().id, pinned.id);
}

#[test]
fn memory_v2_v1_migration_is_idempotent() {
    let dir = tempdir();
    let pool = pool(&dir);
    // Insert a v1 row directly.
    {
        let conn = pool.get().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let id = Ulid::new().to_string();
        conn.execute(
            "INSERT INTO memories (id, scope, project_id, conversation_id, content, source, approved, created_at, updated_at)
             VALUES (?1, 'project', NULL, NULL, 'Migrated v1 fact', 'user', 1, ?2, ?2)",
            rusqlite::params![id, now],
        ).unwrap();
    }
    let n1 = memory_v2::memory::migrate_from_v1(&pool).unwrap();
    assert_eq!(n1, 1);
    // Second call is a no-op (dedupe).
    let n2 = memory_v2::memory::migrate_from_v1(&pool).unwrap();
    assert_eq!(n2, 0);
    let all = memory_v2::memory::list(&pool, &MemoryFilter { include_superseded: true, ..Default::default() }).unwrap();
    assert!(all.iter().any(|m| m.content == "Migrated v1 fact" && m.approved && m.category == "lesson"));
}
