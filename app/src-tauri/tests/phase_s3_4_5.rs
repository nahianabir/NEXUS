//! Phase S.3.4 — Serendipity (read-only) and S.3.5 — Impact
//! Simulator (read-only).
//!
//! Both modules never write to any table. The serendipity
//! tests verify the surprise-score formula and the
//! read-only invariant. The impact tests verify the
//! dependent counts, the destructive flag, and the
//! read-only invariant.

mod common;

use common::tempdir;
use nexus_core::checkpoints::{self, CreateCheckpoint, EvidenceRef};
use nexus_core::db;
use nexus_core::hypotheses;
use nexus_core::impact::{self, ImpactAction, ImpactRequest};
use nexus_core::projects::{projects, tasks};
use nexus_core::projects::tasks::TaskPriority;
use nexus_core::serendipity::{self, SerendipityOptions};

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

// ---------------------------------------------------------------------------
// Serendipity
// ---------------------------------------------------------------------------

fn write_note(pool: &nexus_core::db::SqlitePool, project_id: &str, title: &str, days_ago: i64) -> String {
    let id = format!("note-{}", uuid::Uuid::new_v4());
    let updated_at = now_ms() - days_ago * 86_400_000;
    pool.get().unwrap().execute(
        "INSERT INTO notes (id, project_id, title, body, created_at, updated_at)
         VALUES (?1, ?2, ?3, '', ?4, ?4)",
        rusqlite::params![id, project_id, title, updated_at],
    ).unwrap();
    id
}

#[test]
fn serendipity_surfaces_dormant_note() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let _ = write_note(&pool, &p.id, "Very old note about the schema", 90);
    let r = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    let n = r.items.iter().find(|i| i.title == "Very old note about the schema");
    assert!(n.is_some(), "expected dormant note in: {:#?}", r.items);
    let n = n.unwrap();
    assert!(n.days_dormant > 80.0);
    assert!(n.score > 0.0);
}

#[test]
fn serendipity_surfaces_forgotten_hypothesis_with_low_confidence() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let h = hypotheses::create(&pool, &hypotheses::CreateHypothesis {
        project_id: p.id.clone(),
        task_id: None,
        title: "A shaky claim".into(),
        statement: "Maybe.".into(),
        prediction: None,
        confidence: 0.1,
        evidence_for: Vec::new(),
        evidence_against: Vec::new(),
        memory_id: None,
    }).unwrap();
    let long_ago = now_ms() - 60 * 86_400_000;
    pool.get().unwrap().execute(
        "UPDATE hypotheses SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![long_ago, h.id],
    ).unwrap();
    let r = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    let item = r.items.iter().find(|i| i.id == h.id);
    assert!(item.is_some(), "expected forgotten hypothesis; got {:#?}", r.items);
    let item = item.unwrap();
    assert!(item.days_dormant > 50.0);
    // The score for a forgotten low-confidence hypothesis
    // should exceed the score for a forgotten high-confidence
    // one (the formula weights by 1 - confidence).
    let h2 = hypotheses::create(&pool, &hypotheses::CreateHypothesis {
        project_id: p.id.clone(),
        task_id: None,
        title: "A confident claim".into(),
        statement: "Definitely.".into(),
        prediction: None,
        confidence: 0.95,
        evidence_for: Vec::new(),
        evidence_against: Vec::new(),
        memory_id: None,
    }).unwrap();
    pool.get().unwrap().execute(
        "UPDATE hypotheses SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![long_ago, h2.id],
    ).unwrap();
    let r2 = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    let item_low = r2.items.iter().find(|i| i.id == h.id).unwrap();
    let item_high = r2.items.iter().find(|i| i.id == h2.id).unwrap();
    assert!(item_low.score > item_high.score,
        "low-confidence ({}) should outrank high ({})",
        item_low.score, item_high.score);
}

#[test]
fn serendipity_surfaces_cold_archived_checkpoint() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
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
        pending_actions: Vec::new(),
        evidence_refs: Vec::new(),
        created_by: Some("user".into()),
        model_hint: None,
    }).unwrap();
    let _ = checkpoints::set_status(&pool, &c.id, checkpoints::CheckpointStatus::Archived).unwrap();
    let long_ago = now_ms() - 100 * 86_400_000;
    pool.get().unwrap().execute(
        "UPDATE checkpoints SET created_at = ?1 WHERE id = ?2",
        rusqlite::params![long_ago, c.id],
    ).unwrap();
    let r = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    let item = r.items.iter().find(|i| i.id == c.id);
    assert!(item.is_some(), "expected cold checkpoint; got {:#?}", r.items);
}

#[test]
fn serendipity_is_read_only() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let notes_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes WHERE project_id = ?1",
                   rusqlite::params![p.id], |r| r.get(0)).unwrap();
    let tasks_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks WHERE project_id = ?1",
                   rusqlite::params![p.id], |r| r.get(0)).unwrap();
    let ck_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoints WHERE project_id = ?1",
                   rusqlite::params![p.id], |r| r.get(0)).unwrap();
    let _ = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    let _ = serendipity::report(&pool, &p.id, &SerendipityOptions::default()).unwrap();
    let notes_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes WHERE project_id = ?1",
                   rusqlite::params![p.id], |r| r.get(0)).unwrap();
    let tasks_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks WHERE project_id = ?1",
                   rusqlite::params![p.id], |r| r.get(0)).unwrap();
    let ck_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoints WHERE project_id = ?1",
                   rusqlite::params![p.id], |r| r.get(0)).unwrap();
    assert_eq!(notes_before, notes_after);
    assert_eq!(tasks_before, tasks_after);
    assert_eq!(ck_before, ck_after);
}

#[test]
fn serendipity_handles_empty_project() {
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
fn impact_delete_task_reports_checkpoints_and_hypotheses() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "About to be deleted", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let c = checkpoints::create(&pool, &CreateCheckpoint {
        project_id: p.id.clone(),
        task_id: Some(t.id.clone()),
        title: "Cites the task".into(),
        goal: None, completed_work: None, current_state: None, next_action_text: None,
        observed_files: Vec::new(),
        observed_tests: Vec::new(),
        errors: Vec::new(),
        decisions: Vec::new(),
        constraints: Vec::new(),
        pending_actions: Vec::new(),
        evidence_refs: vec![EvidenceRef {
            kind: "task".into(), id: t.id.clone(),
        }],
        created_by: Some("user".into()),
        model_hint: None,
    }).unwrap();
    let _h = hypotheses::create(&pool, &hypotheses::CreateHypothesis {
        project_id: p.id.clone(),
        task_id: Some(t.id.clone()),
        title: "Linked to the task".into(),
        statement: "X.".into(),
        prediction: None,
        confidence: 0.5,
        evidence_for: Vec::new(),
        evidence_against: Vec::new(),
        memory_id: None,
    }).unwrap();
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteTask, target_id: t.id.clone(),
    }).unwrap();
    assert!(r.destructive);
    let ck_count = r.counts.iter().find(|c| c.table == "checkpoints").unwrap().count;
    assert_eq!(ck_count, 1);
    let h_count = r.counts.iter().find(|c| c.table == "hypotheses").unwrap().count;
    assert_eq!(h_count, 1);
    assert!(r.affected.iter().any(|a| a.id == c.id));
}

#[test]
fn impact_complete_task_is_not_destructive() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "About to be completed", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::CompleteTask, target_id: t.id.clone(),
    }).unwrap();
    assert!(!r.destructive);
    assert!(!r.warnings.is_empty(), "complete should emit at least one warning");
}

#[test]
fn impact_delete_project_fans_out_to_every_table() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Inside project", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteProject, target_id: p.id.clone(),
    }).unwrap();
    assert!(r.destructive);
    let t_count = r.counts.iter().find(|c| c.table == "tasks").map(|c| c.count).unwrap_or(0);
    assert_eq!(t_count, 1);
    assert!(r.affected.iter().any(|a| a.id == t.id));
}

#[test]
fn impact_is_read_only() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Survives impact", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let notes_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    let tasks_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
    let checkpoints_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoints", [], |r| r.get(0)).unwrap();
    let _ = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteTask, target_id: t.id.clone(),
    }).unwrap();
    let _ = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteProject, target_id: p.id.clone(),
    }).unwrap();
    let notes_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    let tasks_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
    let checkpoints_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoints", [], |r| r.get(0)).unwrap();
    assert_eq!(notes_before, notes_after);
    assert_eq!(tasks_before, tasks_after);
    assert_eq!(checkpoints_before, checkpoints_after);
    // The target task and project are still there.
    let t2 = tasks::get(&pool, &t.id).unwrap();
    assert_eq!(t2.id, t.id);
}

#[test]
fn impact_unknown_action_is_rejected() {
    let dir = tempdir();
    let pool = pool(&dir);
    let err = ImpactAction::parse("delete_universe");
    assert!(err.is_err());
}

#[test]
fn impact_delete_checkpoint_counts_resumes() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let c = checkpoints::create(&pool, &CreateCheckpoint {
        project_id: p.id.clone(),
        task_id: None,
        title: "About to be deleted".into(),
        goal: None, completed_work: None, current_state: None, next_action_text: None,
        observed_files: Vec::new(),
        observed_tests: Vec::new(),
        errors: Vec::new(),
        decisions: Vec::new(),
        constraints: Vec::new(),
        pending_actions: Vec::new(),
        evidence_refs: Vec::new(),
        created_by: Some("user".into()),
        model_hint: None,
    }).unwrap();
    let _ = checkpoints::resume_summary(&pool, &c.id, None).unwrap();
    let _ = checkpoints::resume_summary(&pool, &c.id, None).unwrap();
    let r = impact::simulate(&pool, &ImpactRequest {
        action: ImpactAction::DeleteCheckpoint, target_id: c.id.clone(),
    }).unwrap();
    assert!(r.destructive);
    let n = r.counts.iter().find(|c| c.table == "checkpoint_resumes").unwrap().count;
    assert_eq!(n, 2);
}
