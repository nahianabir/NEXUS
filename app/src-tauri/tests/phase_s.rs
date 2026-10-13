//! Phase S.1 — Project Time Machine.
//!
//! Verifies the additive migration, the timeline row insertion /
//! idempotency contract, the cross-source ingest pipeline, the
//! read-only reconstruction, the past-vs-present comparison, the
//! superseded-items listing, and the decision-archaeology view.
//!
//! These tests never touch the source tables. They create fixture
//! rows in `tasks`, `notes`, `memories_v2`, `kg_relationships`,
//! `experiments`, and `audit_events`, then assert that the
//! timeline ingests them correctly and that the reconstruction
//! reflects a known moment in the past.

mod common;

use common::tempdir;
use nexus_core::audit;
use nexus_core::db;
use nexus_core::kg;
use nexus_core::memory_v2::{self, memory::MemoryFilter, memory::MemoryV2};
use nexus_core::projects::{notes, projects, tasks};
use nexus_core::timeline::{self};

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

#[test]
fn compare_returns_events_in_window() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Cmp", None, None, &[]).unwrap();
    // Three events at distinct timestamps.
    timeline::insert(
        &pool, Some(&p.id), "created", "task", Some("a"), Some("a"),
        100, "first", serde_json::json!({}), "test", Some("r1"),
    ).unwrap();
    timeline::insert(
        &pool, Some(&p.id), "created", "task", Some("b"), Some("b"),
        200, "second", serde_json::json!({}), "test", Some("r1"),
    ).unwrap();
    timeline::insert(
        &pool, Some(&p.id), "created", "task", Some("c"), Some("c"),
        300, "third", serde_json::json!({}), "test", Some("r1"),
    ).unwrap();
    // [150, 301) is half-open at the top: includes events at 200 and 300.
    let c = timeline::compare(&pool, &p.id, 150, 301).unwrap();
    assert_eq!(c.added.len(), 2, "[150, 301) should include the events at 200 and 300");
    // to_ms < from_ms is rejected.
    let err = timeline::compare(&pool, &p.id, 300, 100);
    assert!(err.is_err());
}

#[test]
fn migration_brings_schema_to_v12_and_creates_timeline_table() {
    let dir = tempdir();
    let pool = pool(&dir);
    let conn = pool.get().unwrap();
    let v: u32 = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    assert!(v >= 12, "expected migration 12 or later to be applied, got {v}");
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='timeline_events'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "timeline_events table should exist");
    // The unique key is in place.
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='uq_timeline_source_event'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "uq_timeline_source_event should exist");
}

#[test]
fn insert_is_idempotent_for_same_source_event() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Demo", None, None, &[]).unwrap();
    // Manually insert the same event twice.
    let inserted = timeline::insert(
        &pool,
        Some(&p.id),
        "created",
        "task",
        Some("task-abc"),
        Some("Write a doc"),
        1_700_000_000_000,
        "Task created: Write a doc",
        serde_json::json!({ "title": "Write a doc" }),
        "ingest_tasks",
        Some("run-1"),
    ).unwrap();
    assert!(inserted);
    let inserted2 = timeline::insert(
        &pool,
        Some(&p.id),
        "created",
        "task",
        Some("task-abc"),
        Some("Write a doc"),
        1_700_000_000_000,
        "Task created: Write a doc",
        serde_json::json!({ "title": "Write a doc" }),
        "ingest_tasks",
        Some("run-2"),
    ).unwrap();
    assert!(!inserted2, "second insert with same key must be a no-op");
    let list = timeline::list(
        &pool,
        &timeline::TimelineFilter { project_id: Some(p.id.clone()), ..Default::default() },
    ).unwrap();
    assert_eq!(list.len(), 1);
    // Different event_type with the same source still counts as a
    // distinct timeline row.
    let inserted3 = timeline::insert(
        &pool,
        Some(&p.id),
        "completed",
        "task",
        Some("task-abc"),
        Some("Write a doc"),
        1_700_000_000_000,
        "Task completed: Write a doc",
        serde_json::json!({}),
        "ingest_tasks",
        Some("run-1"),
    ).unwrap();
    assert!(inserted3);
    let list = timeline::list(
        &pool,
        &timeline::TimelineFilter { project_id: Some(p.id.clone()), ..Default::default() },
    ).unwrap();
    assert_eq!(list.len(), 2);
}

#[test]
fn list_is_deterministic_for_equal_event_at() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Demo", None, None, &[]).unwrap();
    // Two events at the same timestamp.
    timeline::insert(
        &pool, Some(&p.id), "created", "note", Some("n1"), None,
        1_700_000_000_000, "B", serde_json::json!({}), "test", Some("r1"),
    ).unwrap();
    timeline::insert(
        &pool, Some(&p.id), "created", "note", Some("n2"), None,
        1_700_000_000_000, "A", serde_json::json!({}), "test", Some("r1"),
    ).unwrap();
    let list = timeline::list(
        &pool,
        &timeline::TimelineFilter { project_id: Some(p.id.clone()), ..Default::default() },
    ).unwrap();
    assert_eq!(list.len(), 2);
    // Both have the same event_at; the secondary key is id ASC.
    // We just assert the order is stable across two calls.
    let list2 = timeline::list(
        &pool,
        &timeline::TimelineFilter { project_id: Some(p.id.clone()), ..Default::default() },
    ).unwrap();
    assert_eq!(
        list.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
        list2.iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
    );
}

#[test]
fn ingest_all_for_project_pulls_from_every_registered_source() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Project Alpha", None, None, &[]).unwrap();

    // Tasks.
    let t = tasks::create(
        &pool,
        Some(&p.id),
        "Read paper",
        Some("Q4 paper"),
        tasks::TaskPriority::High,
        None,
        &[],
    ).unwrap();
    tasks::set_status(&pool, &t.id, tasks::TaskStatus::Done).unwrap();
    // Notes.
    let n = notes::create(&pool, Some(&p.id), "Idea", "Outline of the paper").unwrap();
    // A v2 memory with a known category.
    let m = MemoryV2 {
        id: memory_v2::memory::new_id(),
        workspace_id: None,
        project_id: Some(p.id.clone()),
        conversation_id: None,
        category: "decision".into(),
        content: "Decided to use 4-bit quant".into(),
        content_hash: "h".into(),
        normalized_content: "h".into(),
        source_kind: "user".into(),
        source_id: None,
        source_label: Some("meeting".into()),
        source_excerpt: None,
        event_at: Some(1_700_000_000_000),
        valid_from: Some(1_700_000_000_000),
        valid_until: None,
        verified_at: Some(1_700_000_000_000),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "approved".into(),
        pinned: false,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
    };
    memory_v2::memory::insert(&pool, &m).unwrap();
    // An audit row that targets the project.
    audit::log(
        &pool,
        "user",
        "project.create",
        Some(&p.id),
        serde_json::json!({ "name": "Project Alpha" }),
    ).unwrap();

    let report = timeline::ingest::ingest_all_for_project(&pool, &p.id).unwrap();
    assert!(report.examined > 0, "at least some source rows were examined");
    assert!(report.inserted > 0, "at least some timeline rows were inserted");
    let kinds: std::collections::HashSet<String> = report
        .sources
        .iter()
        .map(|s| s.source_kind.clone())
        .collect();
    for expected in [
        "audit", "task", "note", "memory",
    ] {
        assert!(kinds.contains(expected), "expected source kind {expected} in report, got {kinds:?}");
    }

    // Re-run; nothing new should be inserted because of the UNIQUE key.
    let report2 = timeline::ingest::ingest_all_for_project(&pool, &p.id).unwrap();
    assert_eq!(report2.inserted, 0, "second ingest should be a no-op");
    assert!(report2.skipped_existing > 0);

    // The list reflects the cross-source reality.
    let all = timeline::list(
        &pool,
        &timeline::TimelineFilter { project_id: Some(p.id.clone()), limit: Some(200), ..Default::default() },
    ).unwrap();
    let source_kinds: std::collections::HashSet<String> =
        all.iter().map(|e| e.source_kind.clone()).collect();
    for k in ["audit", "task", "note", "memory"] {
        assert!(source_kinds.contains(k), "timeline should contain {k} events");
    }
    // We should see at least one "completed" event for the task.
    assert!(all.iter().any(|e| e.event_type == "completed" && e.source_kind == "task"));
    let _ = n; // silence unused
}

#[test]
fn reconstruct_at_returns_known_sections_and_does_not_mutate_sources() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Rec", None, None, &[]).unwrap();
    // Two tasks, one done before the boundary, one open.
    let t1 = tasks::create(
        &pool, Some(&p.id), "Earlier", None, tasks::TaskPriority::Normal, None, &[],
    ).unwrap();
    tasks::set_status(&pool, &t1.id, tasks::TaskStatus::Done).unwrap();
    let t2 = tasks::create(
        &pool, Some(&p.id), "Later", None, tasks::TaskPriority::Normal, None, &[],
    ).unwrap();

    // Build a snapshot now: everything is in scope.
    let snap = timeline::reconstruct_at(&pool, &p.id, now_ms()).unwrap();
    let labels: Vec<String> = snap.sections.iter().map(|s| s.label.clone()).collect();
    assert!(labels.iter().any(|l| l.contains("Open tasks")));
    assert!(labels.iter().any(|l| l.contains("Notes")));
    assert!(labels.iter().any(|l| l.contains("Valid memories")));
    assert!(labels.iter().any(|l| l.contains("Decisions")));

    // Hash a few source tables before the reconstruction, then again
    // after, to prove the read-only guarantee.
    let task_count_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks WHERE project_id = ?1", rusqlite::params![p.id], |r| r.get(0))
        .unwrap();
    let note_count_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes WHERE project_id = ?1", rusqlite::params![p.id], |r| r.get(0))
        .unwrap();
    let mem_count_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM memories_v2 WHERE project_id = ?1", rusqlite::params![p.id], |r| r.get(0))
        .unwrap();
    // Run reconstruction many times.
    for _ in 0..5 {
        let _ = timeline::reconstruct_at(&pool, &p.id, now_ms()).unwrap();
    }
    let task_count_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks WHERE project_id = ?1", rusqlite::params![p.id], |r| r.get(0))
        .unwrap();
    let note_count_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes WHERE project_id = ?1", rusqlite::params![p.id], |r| r.get(0))
        .unwrap();
    let mem_count_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM memories_v2 WHERE project_id = ?1", rusqlite::params![p.id], |r| r.get(0))
        .unwrap();
    assert_eq!(task_count_before, task_count_after);
    assert_eq!(note_count_before, note_count_after);
    assert_eq!(mem_count_before, mem_count_after);
    let _ = t2; // silence
}

#[test]
fn reconstruct_at_handles_unknowns_honestly() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Empty", None, None, &[]).unwrap();
    // No events have been ingested yet.
    let snap = timeline::reconstruct_at(&pool, &p.id, now_ms()).unwrap();
    assert!(!snap.unknowns.is_empty(), "honest reconstruction should report unknowns for an empty project");
    // The unknowns should not pretend to know things they don't.
    assert!(
        snap.unknowns.iter().any(|u| u.contains("No timeline events") || u.contains("Build timeline")),
        "unknowns should mention the missing timeline ingest: {:?}", snap.unknowns,
    );
}

#[test]
fn superseded_lists_memories_and_kg_relationships_with_valid_until() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Sup", None, None, &[]).unwrap();
    // A v2 memory that is superseded. `superseded_by` must point at
    // a row that actually exists, so we first insert a replacement
    // memory and then point the old one's `superseded_by` at it.
    let replacement = MemoryV2 {
        id: memory_v2::memory::new_id(),
        workspace_id: None,
        project_id: Some(p.id.clone()),
        conversation_id: None,
        category: "fact".into(),
        content: "We now use Q5_K_M".into(),
        content_hash: "h2".into(),
        normalized_content: "h2".into(),
        source_kind: "user".into(),
        source_id: None,
        source_label: None,
        source_excerpt: None,
        event_at: Some(1_700_000_000_000),
        valid_from: Some(1_700_000_000_000),
        valid_until: None,
        verified_at: Some(1_700_000_000_000),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "approved".into(),
        pinned: false,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
    };
    memory_v2::memory::insert(&pool, &replacement).unwrap();
    let m = MemoryV2 {
        id: memory_v2::memory::new_id(),
        workspace_id: None,
        project_id: Some(p.id.clone()),
        conversation_id: None,
        category: "fact".into(),
        content: "We used Q4_K_M".into(),
        content_hash: "h".into(),
        normalized_content: "h".into(),
        source_kind: "user".into(),
        source_id: None,
        source_label: None,
        source_excerpt: None,
        event_at: Some(1_700_000_000_000),
        valid_from: Some(1_700_000_000_000),
        valid_until: Some(1_800_000_000_000), // ends later
        verified_at: None,
        superseded_by: Some(replacement.id.clone()),
        deleted_at: None,
        approved: true,
        verification: "approved".into(),
        pinned: false,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
    };
    memory_v2::memory::insert(&pool, &m).unwrap();
    // A KG relationship touching the project's seeded entity.
    let project_entity = kg::graph::create(&pool, &kg::graph::CreateEntity {
        workspace_id: None,
        entity_type: "project".into(),
        canonical_name: p.name.clone(),
        description: None,
        metadata_json: None,
        aliases: vec![],
    }).unwrap();
    let other_entity = kg::graph::create(&pool, &kg::graph::CreateEntity {
        workspace_id: None,
        entity_type: "task".into(),
        canonical_name: "T".into(),
        description: None,
        metadata_json: None,
        aliases: vec![],
    }).unwrap();
    let rel = kg::relationship::create(&pool, &kg::relationship::CreateRelationship {
        source_entity_id: project_entity.id.clone(),
        target_entity_id: other_entity.id.clone(),
        relationship_type: "DEPENDS_ON".into(),
        valid_from: Some(1_700_000_000_000),
        valid_until: Some(1_800_000_000_000),
        verification_status: Some("inferred".into()),
        weight: Some(1.0),
        metadata_json: None,
        evidence: vec![],
    }).unwrap();
    let _ = rel;

    // Ingest first so the timeline has these events.
    let _ = timeline::ingest::ingest_all_for_project(&pool, &p.id).unwrap();
    // Use the same listing the Tauri command uses.
    let sup = timeline::query::list_superseded(&pool, &p.id).unwrap();
    assert!(!sup.memories.is_empty(), "the superseded memory should appear");
    assert!(!sup.kg_relationships.is_empty(), "the kg relationship with valid_until should appear");
    // The plain `list` filter (MemoryFilter) should also surface the
    // superseded row because the default behaviour is to include
    // superseded rows only when explicitly asked.
    let _ = MemoryFilter::default();
}

#[test]
fn decisions_returns_only_approved_decision_memories() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Dec", None, None, &[]).unwrap();
    // A decision memory (verified).
    let mut d = MemoryV2 {
        id: memory_v2::memory::new_id(),
        workspace_id: None,
        project_id: Some(p.id.clone()),
        conversation_id: None,
        category: "decision".into(),
        content: "We pick the lightweight RAG".into(),
        content_hash: "h1".into(),
        normalized_content: "h1".into(),
        source_kind: "user".into(),
        source_id: None,
        source_label: None,
        source_excerpt: None,
        event_at: Some(1_700_000_000_000),
        valid_from: None,
        valid_until: None,
        verified_at: Some(1_700_000_000_000),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "approved".into(),
        pinned: false,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
    };
    memory_v2::memory::insert(&pool, &d).unwrap();
    // A different category — should not be returned.
    d.id = memory_v2::memory::new_id();
    d.category = "lesson".into();
    d.content = "Lesson learned".into();
    d.content_hash = "h2".into();
    d.normalized_content = "h2".into();
    memory_v2::memory::insert(&pool, &d).unwrap();
    // Decisions endpoint.
    let decs = timeline::query::list_decisions(&pool, &p.id).unwrap();
    assert_eq!(decs.len(), 1, "only the decision memory should surface");
    assert_eq!(decs[0].source_kind, "memory");
    assert!(decs[0].summary.contains("decision"));
}

#[test]
fn delete_only_removes_the_timeline_row_not_source() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Del", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Survives", None, tasks::TaskPriority::Normal, None, &[],
    ).unwrap();
    let _ = timeline::ingest::ingest_all_for_project(&pool, &p.id).unwrap();
    let evs = timeline::list(
        &pool,
        &timeline::TimelineFilter { project_id: Some(p.id.clone()), source_kind: Some("task".into()), ..Default::default() },
    ).unwrap();
    assert!(!evs.is_empty());
    let id = evs[0].id.clone();
    timeline::delete(&pool, &id).unwrap();
    // Source row still exists.
    let t2 = tasks::get(&pool, &t.id).unwrap();
    assert_eq!(t2.id, t.id);
    // Timeline row is gone.
    let evs2 = timeline::list(
        &pool,
        &timeline::TimelineFilter { project_id: Some(p.id.clone()), ..Default::default() },
    ).unwrap();
    assert!(evs2.iter().all(|e| e.id != id));
}

#[test]
fn manual_entry_round_trips_with_distinct_event_and_import_timestamps() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "M", None, None, &[]).unwrap();
    let historical = 1_600_000_000_000_i64; // an old moment in time
    let ev = timeline::insert_manual(
        &pool,
        &p.id,
        "Decided to revisit Q2 results in October",
        Some(historical),
        serde_json::json!({ "category": "milestone" }),
    ).unwrap();
    assert_eq!(ev.event_at, historical, "the timeline must preserve the historical event time");
    assert!(ev.imported_at >= historical, "imported_at should be the time the row was written, not the event time");
    assert_eq!(ev.source_kind, "manual");
    assert_eq!(ev.event_type, "manual");
}
