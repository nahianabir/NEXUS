//! Phase S.3.4 (Serendipity) and S.3.5 (Impact Simulator).
//! Both modules are read-only. The impact simulator is
//! audit-logged; the serendipity module is plain read.

mod common;

use common::tempdir;
use nexus_core::checkpoints::{self, CreateCheckpoint, EvidenceRef, PendingAction};
use nexus_core::db;
use nexus_core::hypotheses::{self, CreateHypothesis};
use nexus_core::impact::{self, ImpactAction, ImpactRequest};
use nexus_core::projects::{notes, projects, tasks};
use nexus_core::projects::tasks::TaskPriority;
use nexus_core::serendipity::{self, SerendipityOptions};
use rusqlite::params;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

fn backdate(pool: &nexus_core::db::SqlitePool, table: &str, id: &str, ms: i64) {
    let conn = pool.get().unwrap();
    let _ = conn.execute(
        &format!("UPDATE {table} SET updated_at = ?1, created_at = ?1 WHERE id = ?2"),
        params![ms, id],
    );
}

// ---------------------------------------------------------------------------
// Serendipity
// ---------------------------------------------------------------------------

#[test]
fn serendipity_reports_dormant_note_and_dormant_memory() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let n = notes::create(&pool, Some(&p.id), "An old note", "Body").unwrap();
    // Create a memory row directly. The memories_v2 schema
    // does not have an `importance` column; the serendipity
    // module detects the column and falls back to a
    // 0-importance score.
    let mem_id = "mem-test-1";
    let now = now_ms();
    let ninety_days_ago = now - 90 * 86_400_000;
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO memories_v2
            (id, project_id, category, content, content_hash, normalized_content,
             source_kind, source_id, event_at, created_at, updated_at,
             deleted_at, approved, verification, pinned)
         VALUES (?1, ?2, 'lesson', 'dormant memory body',
                 'deadbeef', 'dormant memory body',
                 'agent', 'run-1', ?3, ?4, ?5, NULL, 1, 'derived', 0)",
        params![mem_id, p.id, ninety_days_ago, ninety_days_ago, ninety_days_ago],
    ).unwrap();
    backdate(&pool, "notes", &n.id, ninety_days_ago);
    let r = serendipity::report(&pool, &p.id, &SerendipityOptions {
        dormant_days: 30,
        ..Default::default()
    }).unwrap();
    let kinds: Vec<&str> = r.items.iter().map(|i| i.kind.as_str()).collect();
    assert!(kinds.contains(&"dormant_note"), "expected dormant_note; got {:?}", kinds);
    assert!(kinds.contains(&"dormant_memory"), "expected dormant_memory; got {:?}", kinds);
    let memory_item = r.items.iter().find(|i| i.kind.as_str() == "dormant_memory").unwrap();
    assert!(memory_item.score > 0.0);
    assert!(memory_item.days_dormant > 80.0);
}

#[test]
fn serendipity_reports_forgotten_hypothesis_and_cold_checkpoint() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let h = hypotheses::create(&pool, &CreateHypothesis {
        project_id: p.id.clone(),
        task_id: None,
        title: "Will we ever know?".into(),
        statement: "Some claim".into(),
        prediction: None,
        confidence: 0.4,
        evidence_for: Vec::new(),
        evidence_against: Vec::new(),
        memory_id: None,
    }).unwrap();
    let c = checkpoints::create(&pool, &CreateCheckpoint {
        project_id: p.id.clone(),
        task_id: None,
        title: "Cold checkpoint".into(),
        goal: None, completed_work: None, current_state: None, next_action_text: None,
        observed_files: Vec::new(),
        observed_tests: Vec::new(),
        errors: Vec::new(),
        decisions: Vec::new(),
        constraints: Vec::new(),
        pending_actions: vec![PendingAction { text: "do a thing".into(), done: false }],
        evidence_refs: vec![EvidenceRef { kind: "hypothesis".into(), id: h.id.clone() }],
        created_by: Some("user".into()),
        model_hint: None,
    }).unwrap();
    let _ = checkpoints::set_status(&pool, &c.id, checkpoints::CheckpointStatus::Archived).unwrap();
    let long_ago = now_ms() - 90 * 86_400_000;
    backdate(&pool, "hypotheses", &h.id, long_ago);
    backdate(&pool, "checkpoints", &c.id, long_ago);
    let r = serendipity::report(&pool, &p.id, &SerendipityOptions {
        dormant_days: 30,
        ..Default::default()
    }).unwrap();
    let kinds: Vec<&str> = r.items.iter().map(|i| i.kind.as_str()).collect();
    assert!(kinds.contains(&"forgotten_hypothesis"), "got {:?}", kinds);
    assert!(kinds.contains(&"cold_checkpoint"), "got {:?}", kinds);
}

#[test]
fn serendipity_recent_items_are_not_surfaced() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let _n = notes::create(&pool, Some(&p.id), "Brand new", "Body").unwrap();
    let r = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    assert!(r.items.is_empty(), "expected no dormant items, got {:?}", r.items);
}

#[test]
fn serendipity_is_read_only() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let n = notes::create(&pool, Some(&p.id), "Old", "Body").unwrap();
    let long_ago = now_ms() - 90 * 86_400_000;
    backdate(&pool, "notes", &n.id, long_ago);
    let before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    let _ = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    let _ = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    let after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    assert_eq!(before, after, "serendipity must not insert or delete notes");
}

#[test]
fn serendipity_cross_project_panel_pairs_dormant_notes_by_trigrams() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p1 = projects::create(&pool, None, "P1", None, None, &[]).unwrap();
    let p2 = projects::create(&pool, None, "P2", None, None, &[]).unwrap();
    let a = notes::create(&pool, Some(&p1.id), "rust async runtime design choices", "").unwrap();
    let b = notes::create(&pool, Some(&p2.id), "rust async runtime scheduler internals", "").unwrap();
    let long_ago = now_ms() - 90 * 86_400_000;
    backdate(&pool, "notes", &a.id, long_ago);
    backdate(&pool, "notes", &b.id, long_ago);
    let r = serendipity::report(&pool, &p1.id, &SerendipityOptions {
        dormant_days: 30,
        cross_project_limit: 10,
        ..Default::default()
    }).unwrap();
    assert!(!r.cross_project.is_empty(),
        "expected at least one cross-project pair, got nothing (left={:?}, right={:?})",
        r.items.iter().map(|i| (&i.id, &i.summary)).collect::<Vec<_>>(),
        Vec::<&str>::new(),
    );
    let first = &r.cross_project[0];
    assert!(first.left.project_id == p1.id || first.right.project_id == p1.id);
    assert!(first.shared_count >= 1);
}

#[test]
fn serendipity_does_not_break_without_optional_tables() {
    // Use the full migration; the serendipity module should
    // silently skip kinds whose source tables are missing or
    // whose columns are absent. Here we use a freshly created
    // project with no rows.
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let r = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    assert!(r.items.is_empty());
    assert!(r.cross_project.is_empty());
}

// ---------------------------------------------------------------------------
// Impact Simulator
// ---------------------------------------------------------------------------

#[test]
fn impact_simulate_delete_note_counts_timeline_events() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let n = notes::create(&pool, Some(&p.id), "Source", "Body").unwrap();
    let conn = pool.get().unwrap();
    for kind in ["created", "updated"] {
        let id = format!("te-{kind}");
        conn.execute(
            "INSERT INTO timeline_events
                (id, project_id, event_type, source_kind, source_id,
                 event_at, imported_at, summary, created_at)
             VALUES (?1, ?2, ?3, 'note', ?4, 0, 0, 'x', 0)",
            params![id, p.id, kind, n.id],
        ).unwrap();
    }
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteNote, target_id: n.id.clone(),
    }).unwrap();
    let count = r.counts.iter().find(|c| c.table == "timeline_events")
        .expect("timeline_events count present");
    assert_eq!(count.count, 2);
    assert!(r.destructive);
    assert_eq!(r.affected.len(), 2);
}

#[test]
fn impact_simulate_delete_task_counts_checkpoints_and_hypotheses() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Task", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let _c = checkpoints::create(&pool, &CreateCheckpoint {
        project_id: p.id.clone(),
        task_id: Some(t.id.clone()),
        title: "Linked checkpoint".into(),
        goal: None, completed_work: None, current_state: None, next_action_text: None,
        observed_files: Vec::new(),
        observed_tests: Vec::new(),
        errors: Vec::new(),
        decisions: Vec::new(),
        constraints: Vec::new(),
        pending_actions: Vec::new(),
        evidence_refs: vec![EvidenceRef { kind: "task".into(), id: t.id.clone() }],
        created_by: Some("user".into()),
        model_hint: None,
    }).unwrap();
    let _h = hypotheses::create(&pool, &CreateHypothesis {
        project_id: p.id.clone(),
        task_id: Some(t.id.clone()),
        title: "Linked hypothesis".into(),
        statement: "claim".into(),
        prediction: None,
        confidence: 0.5,
        evidence_for: Vec::new(),
        evidence_against: Vec::new(),
        memory_id: None,
    }).unwrap();
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteTask, target_id: t.id.clone(),
    }).unwrap();
    let ck = r.counts.iter().find(|c| c.table == "checkpoints").unwrap();
    assert_eq!(ck.count, 1);
    let hy = r.counts.iter().find(|c| c.table == "hypotheses").unwrap();
    assert_eq!(hy.count, 1);
    assert!(r.destructive);
}

#[test]
fn impact_simulate_delete_project_walks_all_dependent_tables() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let _t = tasks::create(
        &pool, Some(&p.id), "t1", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let _n = notes::create(&pool, Some(&p.id), "n1", "b").unwrap();
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteProject, target_id: p.id.clone(),
    }).unwrap();
    let tasks_count = r.counts.iter().find(|c| c.table == "tasks").unwrap();
    let notes_count = r.counts.iter().find(|c| c.table == "notes").unwrap();
    assert_eq!(tasks_count.count, 1);
    assert_eq!(notes_count.count, 1);
    assert!(r.destructive);
}

#[test]
fn impact_simulate_complete_task_emits_a_warning() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "t1", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::CompleteTask, target_id: t.id.clone(),
    }).unwrap();
    assert!(!r.destructive);
    assert!(!r.warnings.is_empty(),
        "completing a task should warn about open checkpoint actions");
}

#[test]
fn impact_simulate_unknown_action_rejected() {
    let dir = tempdir();
    let pool = pool(&dir);
    let err = ImpactAction::parse("delete_everything");
    assert!(err.is_err());
    // `simulate` on a valid action with a target id that
    // doesn't exist should still return a well-formed report
    // with all counts at zero.
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteNote, target_id: "does-not-exist".into(),
    }).unwrap();
    for c in &r.counts {
        assert_eq!(c.count, 0, "{} should be zero for a missing target", c.table);
    }
    assert!(r.affected.is_empty());
}

#[test]
fn impact_simulate_is_read_only() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let n = notes::create(&pool, Some(&p.id), "n1", "b").unwrap();
    let tasks_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
    let notes_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    let _ = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteNote, target_id: n.id.clone(),
    }).unwrap();
    let _ = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteProject, target_id: p.id.clone(),
    }).unwrap();
    let tasks_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
    let notes_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    assert_eq!(tasks_before, tasks_after);
    assert_eq!(notes_before, notes_after);
}
