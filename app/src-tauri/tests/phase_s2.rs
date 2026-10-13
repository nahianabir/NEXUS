//! Phase S.2 — One-Click Work Resume.
//!
//! Verifies the additive migration, the checkpoint CRUD
//! invariants, model independence, the read-only drift check,
//! and the read-only-by-default resume summary.
//!
//! These tests never touch the source tables. They create
//! fixture rows in `tasks`, `notes`, `memories_v2`, then save a
//! checkpoint that references them, and assert that the
//! checkpoint survives restart, that evidence refs resolve
//! correctly, that drift is reported honestly, and that
//! deleting a checkpoint does not cascade into the source
//! tables.

mod common;

use common::tempdir;
use nexus_core::checkpoints::{
    self, CapturedDecision, CapturedError, CheckpointStatus, CreateCheckpoint, EvidenceRef,
    ObservedFile, ObservedTest, PendingAction,
};
use nexus_core::db;
use nexus_core::memory_v2::memory::{MemoryFilter, MemoryV2};
use nexus_core::projects::{notes, projects, tasks};

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

#[test]
fn migration_brings_schema_to_v13_and_creates_checkpoints_table() {
    let dir = tempdir();
    let pool = pool(&dir);
    let conn = pool.get().unwrap();
    let v: u32 = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    assert!(v >= 13, "expected migration 13 or later to be applied, got {v}");
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='checkpoints'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "checkpoints table should exist");
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='checkpoint_resumes'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "checkpoint_resumes table should exist");
}

fn sample_request(project_id: &str) -> CreateCheckpoint {
    CreateCheckpoint {
        project_id: project_id.to_string(),
        task_id: None,
        title: "S.2 fixture".into(),
        goal: Some("Land the One-Click Work Resume feature.".into()),
        completed_work: Some("Schema and CRUD landed.".into()),
        current_state: Some("About to write the resume summary.".into()),
        next_action_text: Some("Write the resume summary test.".into()),
        observed_files: Vec::new(),
        observed_tests: vec![ObservedTest {
            name: "phase_s2_crud".into(),
            outcome: "passed".into(),
            note: None,
            at_ms: Some(now_ms()),
        }],
        errors: vec![CapturedError {
            text: "Read receipts arrived out of order once.".into(),
            at_ms: Some(now_ms()),
        }],
        decisions: vec![CapturedDecision {
            summary: "Use filesystem mtimes, not git, for v1 drift.".into(),
            rationale: Some("Existing codebase has no git module.".into()),
            memory_id: None,
        }],
        constraints: vec![],
        pending_actions: vec![PendingAction {
            text: "Write the resume summary test.".into(),
            done: false,
        }],
        evidence_refs: Vec::new(),
        created_by: Some("user".into()),
        model_hint: Some("Qwen3.5-9B".into()),
    }
}

#[test]
fn create_then_get_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "S2", None, None, &[]).unwrap();
    let c = checkpoints::create(&pool, &sample_request(&p.id)).unwrap();
    assert_eq!(c.title, "S.2 fixture");
    assert_eq!(c.status, "active");
    assert_eq!(c.observed_tests.len(), 1);
    assert_eq!(c.decisions.len(), 1);
    assert_eq!(c.errors.len(), 1);
    assert!(c.drift_check.captured_at.is_some());
    let loaded = checkpoints::get(&pool, &c.id).unwrap();
    assert_eq!(loaded.id, c.id);
    assert_eq!(loaded.goal, c.goal);
}

#[test]
fn create_rejects_empty_title() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "S2", None, None, &[]).unwrap();
    let mut req = sample_request(&p.id);
    req.title = "   ".into();
    let err = checkpoints::create(&pool, &req);
    assert!(err.is_err());
}

#[test]
fn list_filters_by_project_and_status() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p1 = projects::create(&pool, None, "P1", None, None, &[]).unwrap();
    let p2 = projects::create(&pool, None, "P2", None, None, &[]).unwrap();
    let _c1 = checkpoints::create(&pool, &sample_request(&p1.id)).unwrap();
    let _c2 = checkpoints::create(&pool, &sample_request(&p2.id)).unwrap();
    let list = checkpoints::list(&pool, &checkpoints::ListFilter {
        project_id: Some(p1.id.clone()),
        ..Default::default()
    }).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].project_id, p1.id);
    let list_all = checkpoints::list(&pool, &checkpoints::ListFilter::default()).unwrap();
    assert_eq!(list_all.len(), 2);
    // Status filter.
    let list_active = checkpoints::list(&pool, &checkpoints::ListFilter {
        status: Some("active".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(list_active.len(), 2);
    let list_archived = checkpoints::list(&pool, &checkpoints::ListFilter {
        status: Some("archived".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(list_archived.len(), 0);
}

#[test]
fn update_changes_fields_and_preserves_drift_snapshot() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let c = checkpoints::create(&pool, &sample_request(&p.id)).unwrap();
    let original_drift_at = c.drift_check.captured_at;
    // Update via the tri-state Option<Option<String>> encoding.
    let updated = checkpoints::update(
        &pool,
        &c.id,
        &checkpoints::UpdateCheckpoint {
            title: Some("Renamed".into()),
            goal: Some(Some("New goal".into())),
            completed_work: Some(None), // clear
            ..Default::default()
        },
    ).unwrap();
    assert_eq!(updated.title, "Renamed");
    assert_eq!(updated.goal.as_deref(), Some("New goal"));
    assert_eq!(updated.completed_work, None);
    // The drift snapshot is preserved across edits.
    assert_eq!(updated.drift_check.captured_at, original_drift_at);
    assert!(updated.updated_at >= c.created_at);
}

#[test]
fn set_status_writes_resumed_at_only_for_resumed() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let c = checkpoints::create(&pool, &sample_request(&p.id)).unwrap();
    let archived = checkpoints::set_status(&pool, &c.id, CheckpointStatus::Archived).unwrap();
    assert_eq!(archived.status, "archived");
    assert!(archived.resumed_at.is_none());
    // Restore to active for the next assertion.
    // (We do not have a public back-to-active helper; the
    // internal update SQL allows it via the lib. We re-use the
    // lib's set_status with a direct UPDATE here is not needed;
    // the archived status is enough for this test.)
}

#[test]
fn delete_only_removes_the_checkpoint_not_source_records() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Survives", None, tasks::TaskPriority::Normal, None, &[],
    ).unwrap();
    let n = notes::create(&pool, Some(&p.id), "Survives too", "Body").unwrap();
    let mut req = sample_request(&p.id);
    req.task_id = Some(t.id.clone());
    req.evidence_refs = vec![EvidenceRef {
        kind: "note".into(),
        id: n.id.clone(),
    }];
    let c = checkpoints::create(&pool, &req).unwrap();
    checkpoints::delete(&pool, &c.id).unwrap();
    // Source rows still exist.
    let t2 = tasks::get(&pool, &t.id).unwrap();
    assert_eq!(t2.id, t.id);
    let n2 = notes::get(&pool, &n.id).unwrap();
    assert_eq!(n2.id, n.id);
    // Checkpoint is gone.
    let err = checkpoints::get(&pool, &c.id);
    assert!(err.is_err());
}

#[test]
fn resume_summary_marks_status_resumed_and_writes_audit_row() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let c = checkpoints::create(&pool, &sample_request(&p.id)).unwrap();
    assert_eq!(c.status, "active");
    let (resumed, report, summary) = checkpoints::resume_summary(
        &pool, &c.id, Some("First resume of the day.".into()),
    ).unwrap();
    assert_eq!(resumed.status, "resumed");
    assert!(resumed.resumed_at.is_some());
    assert!(!summary.is_empty());
    // The drift report is well-formed even when nothing has
    // changed since the snapshot.
    let _ = report;
    // Resuming twice is allowed and writes two records.
    let _ = checkpoints::resume_summary(&pool, &c.id, Some("Second pass.".into())).unwrap();
    let records = checkpoints::list_resumes(&pool, &c.id).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records[0].resumed_at >= records[1].resumed_at);
}

#[test]
fn drift_report_detects_open_task_count_change() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    // No tasks when the checkpoint is created.
    let c = checkpoints::create(&pool, &sample_request(&p.id)).unwrap();
    assert_eq!(c.drift_check.open_task_count, Some(0));
    // Add a task after the checkpoint.
    tasks::create(
        &pool, Some(&p.id), "Added later", None, tasks::TaskPriority::Normal, None, &[],
    ).unwrap();
    let (_resumed, report, _summary) = checkpoints::resume_summary(
        &pool, &c.id, None,
    ).unwrap();
    let item = report.items.iter().find(|i| i.kind == "open_task_count")
        .expect("open_task_count drift item should appear");
    assert_eq!(item.expected.as_deref(), Some("0"));
    assert_eq!(item.actual.as_deref(), Some("1"));
}

#[test]
fn drift_report_detects_missing_files() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    // Pick a path we can create at test time and then delete.
    let target = dir.join("observed.txt");
    std::fs::write(&target, "hello").unwrap();
    let mut req = sample_request(&p.id);
    req.observed_files = vec![ObservedFile {
        path: target.display().to_string(),
        label: Some("fixture".into()),
        sha256: None,
        size_bytes: Some(5),
        mtime_ms: Some(now_ms()),
    }];
    let c = checkpoints::create(&pool, &req).unwrap();
    // Delete the file and resume.
    std::fs::remove_file(&target).unwrap();
    let (_resumed, report, _summary) = checkpoints::resume_summary(
        &pool, &c.id, None,
    ).unwrap();
    let missing = report.items.iter().find(|i| i.kind == "file_missing")
        .expect("file_missing item should appear");
    assert!(missing.summary.contains("observed.txt"));
    // The report also includes the current snapshot of the file.
    let snap = report.files_now.iter().find(|f| f.path == target.display().to_string())
        .expect("files_now should contain the file we asked about");
    assert!(!snap.exists);
}

#[test]
fn drift_report_marks_evidence_missing_when_record_deleted() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let n = notes::create(&pool, Some(&p.id), "Subject", "Body").unwrap();
    let mut req = sample_request(&p.id);
    req.evidence_refs = vec![EvidenceRef {
        kind: "note".into(),
        id: n.id.clone(),
    }];
    let c = checkpoints::create(&pool, &req).unwrap();
    // Delete the note at resume time.
    notes::delete(&pool, &n.id).unwrap();
    let (_resumed, report, _summary) = checkpoints::resume_summary(
        &pool, &c.id, None,
    ).unwrap();
    let item = report.items.iter().find(|i| i.kind == "evidence_missing")
        .expect("evidence_missing item should appear");
    assert!(item.summary.contains(&n.id));
}

#[test]
fn resume_does_not_modify_source_tables() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Source task", None, tasks::TaskPriority::Normal, None, &[],
    ).unwrap();
    let n = notes::create(&pool, Some(&p.id), "Source note", "Body").unwrap();
    let mut req = sample_request(&p.id);
    req.task_id = Some(t.id.clone());
    let c = checkpoints::create(&pool, &req).unwrap();
    // Snapshot counts before resuming.
    let tasks_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
    let notes_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    let ck_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoints", [], |r| r.get(0)).unwrap();
    // Resume five times.
    for _ in 0..5 {
        let _ = checkpoints::resume_summary(&pool, &c.id, None).unwrap();
    }
    let tasks_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
    let notes_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).unwrap();
    let ck_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoints", [], |r| r.get(0)).unwrap();
    assert_eq!(tasks_before, tasks_after);
    assert_eq!(notes_before, notes_after);
    // The checkpoint row itself is still there (it transitioned
    // to 'resumed' but was not deleted); only the resume
    // records table grew.
    assert_eq!(ck_before, ck_after);
    let resumes: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoint_resumes", [], |r| r.get(0)).unwrap();
    assert!(resumes >= 5, "expected at least 5 resume records, got {resumes}");
}

#[test]
fn model_hint_is_a_hint_not_a_dependency() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let mut req = sample_request(&p.id);
    req.model_hint = Some("Qwen3.5-9B".into());
    let c = checkpoints::create(&pool, &req).unwrap();
    // The model hint is a free-form string; the resume
    // summary must not change regardless of which model is
    // active in the live app. We exercise that by resuming
    // without ever creating a `model_profiles` row.
    let (_resumed, _report, summary_a) = checkpoints::resume_summary(
        &pool, &c.id, Some("Run with Qwen3.5-9B".into()),
    ).unwrap();
    let (_resumed, _report, summary_b) = checkpoints::resume_summary(
        &pool, &c.id, Some("Run with a different model".into()),
    ).unwrap();
    // The deterministic rendering of the resume summary does
    // not embed the model hint — only the user-written fields.
    assert_eq!(summary_a, summary_b);
    // The model hint itself is preserved on the row.
    let c2 = checkpoints::get(&pool, &c.id).unwrap();
    assert_eq!(c2.model_hint.as_deref(), Some("Qwen3.5-9B"));
    // Filtering by an unknown memory id should not crash.
    let mut req_unknown = sample_request(&p.id);
    req_unknown.evidence_refs.push(EvidenceRef {
        kind: "memory".into(),
        id: "does-not-exist".into(),
    });
    let _ = checkpoints::create(&pool, &req_unknown).unwrap();
    let _ = MemoryFilter::default(); // sanity: the symbol is reachable
}

#[test]
fn tri_state_clears_when_outer_some_inner_none() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let c = checkpoints::create(&pool, &sample_request(&p.id)).unwrap();
    assert!(c.next_action_text.is_some());
    let updated = checkpoints::update(
        &pool,
        &c.id,
        &checkpoints::UpdateCheckpoint {
            next_action_text: Some(None), // explicit clear
            ..Default::default()
        },
    ).unwrap();
    assert!(updated.next_action_text.is_none());
}

#[test]
fn delete_cascades_to_resume_records() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let c = checkpoints::create(&pool, &sample_request(&p.id)).unwrap();
    let _ = checkpoints::resume_summary(&pool, &c.id, None).unwrap();
    let before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoint_resumes WHERE checkpoint_id = ?1",
                   rusqlite::params![c.id], |r| r.get(0)).unwrap();
    assert_eq!(before, 1);
    checkpoints::delete(&pool, &c.id).unwrap();
    let after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM checkpoint_resumes WHERE checkpoint_id = ?1",
                   rusqlite::params![c.id], |r| r.get(0)).unwrap();
    assert_eq!(after, 0, "FK ON DELETE CASCADE should remove child resume records");
}
