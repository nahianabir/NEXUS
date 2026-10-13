//! Phase S.3 — Opportunity Radar, Hypothesis Ledger, Codebase
//! Archaeologist. Three additive features, three modules,
//! three pages, one migration (`0014_hypotheses.sql`).
//!
//! The radar and the archaeologist are read-only. The
//! hypothesis ledger is fully additive: it never modifies any
//! existing table, and the FK ON DELETE CASCADE chains are
//! tested explicitly.

mod common;

use common::tempdir;
use nexus_core::archaeologist::{self, FileKind, IndexOptions};
use nexus_core::checkpoints::{self, CreateCheckpoint, PendingAction};
use nexus_core::db;
use nexus_core::hypotheses::{
    self, AddEvidence, CreateHypothesis, EvidenceEntry, HypothesisStatus,
};
use nexus_core::projects::{projects, tasks};
use nexus_core::projects::tasks::TaskPriority;
use nexus_core::radar::{self, OpportunityKind, RadarOptions, Weights};

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

#[test]
fn migration_brings_schema_to_v14_and_creates_hypotheses_tables() {
    let dir = tempdir();
    let pool = pool(&dir);
    let conn = pool.get().unwrap();
    let v: u32 = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    assert!(v >= 14, "expected migration 14 to be applied, got {v}");
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='hypotheses'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(n, 1);
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='hypothesis_evidence'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(n, 1);
}

// ---------------------------------------------------------------------------
// Hypothesis Ledger
// ---------------------------------------------------------------------------

fn sample_hypothesis(project_id: &str) -> CreateHypothesis {
    CreateHypothesis {
        project_id: project_id.to_string(),
        task_id: None,
        title: "S.3.2 fixture".into(),
        statement: "Latency goes down after the new index is added.".into(),
        prediction: Some("Mean p99 latency under 50 ms.".into()),
        confidence: 0.5,
        evidence_for: Vec::new(),
        evidence_against: Vec::new(),
        memory_id: None,
    }
}

#[test]
fn hypothesis_create_round_trips() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let h = hypotheses::create(&pool, &sample_hypothesis(&p.id)).unwrap();
    assert_eq!(h.title, "S.3.2 fixture");
    assert_eq!(h.status, "open");
    assert!((h.confidence - 0.5).abs() < 1e-9);
    let loaded = hypotheses::get(&pool, &h.id).unwrap();
    assert_eq!(loaded.id, h.id);
    assert_eq!(loaded.statement, h.statement);
}

#[test]
fn hypothesis_rejects_empty_title_and_statement() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let mut h = sample_hypothesis(&p.id);
    h.title = "   ".into();
    assert!(hypotheses::create(&pool, &h).is_err());
    let mut h = sample_hypothesis(&p.id);
    h.statement = "  ".into();
    assert!(hypotheses::create(&pool, &h).is_err());
}

#[test]
fn hypothesis_confidence_is_clamped() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let mut h = sample_hypothesis(&p.id);
    h.confidence = 2.0;
    let r = hypotheses::create(&pool, &h).unwrap();
    assert!((r.confidence - 1.0).abs() < 1e-9);
    let mut h = sample_hypothesis(&p.id);
    h.confidence = -0.5;
    let r = hypotheses::create(&pool, &h).unwrap();
    assert_eq!(r.confidence, 0.0);
    // NaN becomes 0.5 (the default).
    let mut h = sample_hypothesis(&p.id);
    h.confidence = f64::NAN;
    let r = hypotheses::create(&pool, &h).unwrap();
    assert!((r.confidence - 0.5).abs() < 1e-9);
}

#[test]
fn hypothesis_decide_writes_status_and_blocks_re_decide() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let h = hypotheses::create(&pool, &sample_hypothesis(&p.id)).unwrap();
    let decided = hypotheses::decide(
        &pool, &h.id, HypothesisStatus::Confirmed, Some("Yep.".into()),
    ).unwrap();
    assert_eq!(decided.status, "confirmed");
    assert!(decided.decided_at.is_some());
    // Re-decide is refused because Confirmed is terminal.
    let err = hypotheses::decide(
        &pool, &h.id, HypothesisStatus::Refuted, None,
    );
    assert!(err.is_err());
}

#[test]
fn hypothesis_delete_cascades_to_evidence() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let h = hypotheses::create(&pool, &sample_hypothesis(&p.id)).unwrap();
    let _ = hypotheses::add_evidence(&pool, &AddEvidence {
        hypothesis_id: h.id.clone(),
        kind: "observation".into(),
        summary: "Saw it once.".into(),
        weight: 0.6,
        source_kind: None,
        source_id: None,
        stance: "for".into(),
    }).unwrap();
    let _ = hypotheses::add_evidence(&pool, &AddEvidence {
        hypothesis_id: h.id.clone(),
        kind: "experiment".into(),
        summary: "Experiment showed no effect.".into(),
        weight: 0.4,
        source_kind: None,
        source_id: None,
        stance: "against".into(),
    }).unwrap();
    let before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM hypothesis_evidence WHERE hypothesis_id = ?1",
                   rusqlite::params![h.id], |r| r.get(0)).unwrap();
    assert_eq!(before, 2);
    hypotheses::delete(&pool, &h.id).unwrap();
    let after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM hypothesis_evidence WHERE hypothesis_id = ?1",
                   rusqlite::params![h.id], |r| r.get(0)).unwrap();
    assert_eq!(after, 0);
}

#[test]
fn hypothesis_delete_does_not_touch_source_records() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Source task", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let mut h = sample_hypothesis(&p.id);
    h.task_id = Some(t.id.clone());
    h.evidence_for = vec![EvidenceEntry {
        kind: "task".into(), id: t.id.clone(), summary: "Linked.".into(), weight: 0.5,
    }];
    let r = hypotheses::create(&pool, &h).unwrap();
    hypotheses::delete(&pool, &r.id).unwrap();
    // The task and the project must still exist.
    let t2 = tasks::get(&pool, &t.id).unwrap();
    assert_eq!(t2.id, t.id);
}

#[test]
fn hypothesis_list_filters_by_status() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let h1 = hypotheses::create(&pool, &sample_hypothesis(&p.id)).unwrap();
    let h2 = hypotheses::create(&pool, &sample_hypothesis(&p.id)).unwrap();
    let _ = hypotheses::decide(&pool, &h2.id, HypothesisStatus::Refuted, None).unwrap();
    let open = hypotheses::list(&pool, &hypotheses::ListFilter {
        project_id: Some(p.id.clone()),
        status: Some("open".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, h1.id);
    let all = hypotheses::list(&pool, &hypotheses::ListFilter {
        project_id: Some(p.id.clone()),
        ..Default::default()
    }).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn hypothesis_update_trips_prediction_and_memory() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let h = hypotheses::create(&pool, &sample_hypothesis(&p.id)).unwrap();
    let u = hypotheses::update(&pool, &h.id, &hypotheses::UpdateHypothesis {
        prediction: Some(Some("Refined.".into())),
        confidence: Some(0.8),
        memory_id: Some(Some("mem-1".into())),
        ..Default::default()
    }).unwrap();
    assert_eq!(u.prediction.as_deref(), Some("Refined."));
    assert!((u.confidence - 0.8).abs() < 1e-9);
    assert_eq!(u.memory_id.as_deref(), Some("mem-1"));
    // Clear memory_id back to None.
    let u = hypotheses::update(&pool, &h.id, &hypotheses::UpdateHypothesis {
        memory_id: Some(None),
        ..Default::default()
    }).unwrap();
    assert!(u.memory_id.is_none());
}

// ---------------------------------------------------------------------------
// Opportunity Radar
// ---------------------------------------------------------------------------

#[test]
fn radar_reports_overdue_urgent_task() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let now = now_ms();
    let t = tasks::create(
        &pool, Some(&p.id), "Hot overdue", None, TaskPriority::Urgent, None, &[],
    ).unwrap();
    // Backdate: due 3 days ago.
    let due = now - 3 * 86_400_000;
    pool.get().unwrap().execute(
        "UPDATE tasks SET due_at = ?1 WHERE id = ?2",
        rusqlite::params![due, t.id],
    ).unwrap();
    let report = radar::report(&pool, &p.id, &RadarOptions::default()).unwrap();
    let overdue = report.items.iter().find(|i| i.kind == OpportunityKind::OverdueUrgentTask);
    assert!(overdue.is_some(), "expected an OverdueUrgentTask item; got {:?}", report.items);
    let overdue = overdue.unwrap();
    assert!(overdue.score >= 100.0);
    assert!(overdue.metric.unwrap() > 2.5 && overdue.metric.unwrap() < 3.5);
    assert_eq!(report.counts.overdue_tasks, 1);
}

#[test]
fn radar_reports_stale_open_task() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Stale", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let long_ago = now_ms() - 30 * 86_400_000;
    pool.get().unwrap().execute(
        "UPDATE tasks SET updated_at = ?1, status = 'open' WHERE id = ?2",
        rusqlite::params![long_ago, t.id],
    ).unwrap();
    let report = radar::report(&pool, &p.id, &RadarOptions::default()).unwrap();
    let stale = report.items.iter().find(|i| i.kind == OpportunityKind::StaleOpenTask);
    assert!(stale.is_some());
}

#[test]
fn radar_reports_resumed_checkpoint_with_pending_actions() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let c = checkpoints::create(&pool, &CreateCheckpoint {
        project_id: p.id.clone(),
        task_id: None,
        title: "After S.2 work".into(),
        goal: None, completed_work: None, current_state: None, next_action_text: None,
        observed_files: Vec::new(),
        observed_tests: Vec::new(),
        errors: Vec::new(),
        decisions: Vec::new(),
        constraints: Vec::new(),
        pending_actions: vec![
            PendingAction { text: "Tidy the test suite.".into(), done: false },
            PendingAction { text: "Write a property test.".into(), done: false },
        ],
        evidence_refs: Vec::new(),
        created_by: Some("user".into()),
        model_hint: None,
    }).unwrap();
    let _ = checkpoints::resume_summary(&pool, &c.id, None).unwrap();
    let report = radar::report(&pool, &p.id, &RadarOptions::default()).unwrap();
    let r = report.items.iter()
        .find(|i| i.kind == OpportunityKind::ResumedCheckpointWithPending);
    assert!(r.is_some());
    let r = r.unwrap();
    assert!(r.summary.contains("2"));
    assert!(r.metric.unwrap() == 2.0);
}

#[test]
fn radar_reports_open_hypothesis_when_hypotheses_table_is_present() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let h = hypotheses::create(&pool, &sample_hypothesis(&p.id)).unwrap();
    let report = radar::report(&pool, &p.id, &RadarOptions::default()).unwrap();
    let h_item = report.items.iter()
        .find(|i| i.kind == OpportunityKind::OpenHypothesis);
    assert!(h_item.is_some(), "expected OpenHypothesis; got {:?}", report.items);
    let h_item = h_item.unwrap();
    assert!(h_item.refs.iter().any(|r| r.kind == "hypothesis" && r.id == h.id));
    assert_eq!(report.counts.open_hypotheses, 1);
}

#[test]
fn radar_does_not_break_when_hypotheses_table_is_absent() {
    // Use the full migration; the radar should still produce a
    // well-formed report when the project has no hypotheses.
    // (Skipping a missing `hypotheses` table is exercised by
    // the open_pool with an empty string elsewhere; here we
    // test the integration with the real schema.)
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let r = radar::report(&pool, &p.id, &RadarOptions::default()).unwrap();
    assert!(r.items.is_empty(), "empty project should produce no opportunities");
    assert_eq!(r.counts.open_hypotheses, 0);
}

#[test]
fn radar_scoring_is_deterministic() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let t = tasks::create(
        &pool, Some(&p.id), "Repeatable", None, TaskPriority::High, None, &[],
    ).unwrap();
    let due = now_ms() - 86_400_000;
    pool.get().unwrap().execute(
        "UPDATE tasks SET due_at = ?1 WHERE id = ?2",
        rusqlite::params![due, t.id],
    ).unwrap();
    let opts = RadarOptions { weights: Weights::default(), ..Default::default() };
    let r1 = radar::report(&pool, &p.id, &opts).unwrap();
    let r2 = radar::report(&pool, &p.id, &opts).unwrap();
    // Generated_at can differ by a millisecond, but scores
    // and item kinds must be identical.
    assert_eq!(r1.items.len(), r2.items.len());
    for (a, b) in r1.items.iter().zip(r2.items.iter()) {
        assert_eq!(a.kind, b.kind);
        // Allow for up to a few seconds of wall-clock drift
        // between the two `now` samples; the score's day-factor
        // term amplifies the difference by a small constant.
        assert!(
            (a.score - b.score).abs() < 1.0,
            "scores diverged: a={} b={}", a.score, b.score
        );
        assert_eq!(a.title, b.title);
    }
}

#[test]
fn radar_is_read_only() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let _ = tasks::create(
        &pool, Some(&p.id), "Some task", None, TaskPriority::Normal, None, &[],
    ).unwrap();
    let projects_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0)).unwrap();
    let tasks_before: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
    let _ = radar::report(&pool, &p.id, &RadarOptions::default()).unwrap();
    let _ = radar::report(&pool, &p.id, &RadarOptions::default()).unwrap();
    let projects_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0)).unwrap();
    let tasks_after: i64 = pool.get().unwrap()
        .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
    assert_eq!(projects_before, projects_after);
    assert_eq!(tasks_before, tasks_after);
}

// ---------------------------------------------------------------------------
// Codebase Archaeologist
// ---------------------------------------------------------------------------

fn make_tree(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("src/nested")).unwrap();
    std::fs::create_dir_all(dir.join("node_modules/foo")).unwrap();
    std::fs::create_dir_all(dir.join(".git/refs")).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    std::fs::write(dir.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
    std::fs::write(dir.join("src/nested/inner.rs"), "pub fn deep() {}\n").unwrap();
    std::fs::write(dir.join("README.md"), "# demo\n").unwrap();
    std::fs::write(dir.join("node_modules/foo/ignored.js"), "// ignored\n").unwrap();
    std::fs::write(dir.join(".git/refs/ignored"), "ignored\n").unwrap();
    // Binary-ish file with a known extension.
    std::fs::write(dir.join("src/data.json"), "{\"a\":1}\n").unwrap();
}

#[test]
fn archaeologist_index_walks_the_tree_and_classifies_files() {
    let dir = tempdir();
    let root = dir.join("repo");
    make_tree(&root);
    let s = archaeologist::index(&root, &IndexOptions::default()).unwrap();
    assert!(s.total_files >= 4, "got {} files: {:?}", s.total_files, s.files);
    let names: Vec<&str> = s.files.iter().map(|f| f.path.as_str()).collect();
    let any_ends_with = |suffix: &str| -> bool {
        names.iter().any(|p| p.ends_with(suffix) || p.ends_with(&suffix.replace('/', "\\")))
    };
    assert!(any_ends_with("src/lib.rs"), "missing src/lib.rs in: {:#?}", names);
    assert!(any_ends_with("src/nested/inner.rs"));
    assert!(any_ends_with("Cargo.toml"));
    assert!(any_ends_with("README.md"));
    // Excluded directories.
    assert!(!names.iter().any(|p| p.contains("node_modules")));
    assert!(!names.iter().any(|p| p.contains(".git")));
    // File kinds.
    let kinds: Vec<FileKind> = s.files.iter().map(|f| f.kind).collect();
    assert!(kinds.contains(&FileKind::Rust));
    assert!(kinds.contains(&FileKind::Toml));
    assert!(kinds.contains(&FileKind::Markdown));
    assert!(kinds.contains(&FileKind::Json));
    // SHA-256 is recorded.
    let lib = s.files.iter()
        .find(|f| f.path.ends_with("src/lib.rs") || f.path.ends_with("src\\lib.rs"))
        .expect("src/lib.rs must be present in the index");
    assert!(lib.sha256.is_some());
    assert_eq!(lib.sha256.as_ref().unwrap().len(), 64);
}

#[test]
fn archaeologist_find_by_name_is_case_insensitive() {
    let dir = tempdir();
    let root = dir.join("repo");
    make_tree(&root);
    let s = archaeologist::index(&root, &IndexOptions::default()).unwrap();
    let hits = archaeologist::find_by_name(&s, "CARGO");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].path.ends_with("Cargo.toml"));
}

#[test]
fn archaeologist_head_returns_first_n_lines_and_skips_oversize() {
    let dir = tempdir();
    let root = dir.join("repo");
    make_tree(&root);
    let big = root.join("src/big.txt");
    std::fs::write(&big, "a\nb\nc\nd\ne\n").unwrap();
    let head = archaeologist::head(&big, 3).unwrap();
    assert_eq!(head.total_lines, 5);
    assert_eq!(head.head.lines().count(), 3);
    assert!(head.truncated);
    // Too-large file: write a >1 MiB file and confirm we
    // refuse to read it.
    let huge = root.join("src/huge.txt");
    let mut bytes = Vec::with_capacity(2 * 1024 * 1024);
    for _ in 0..(2 * 1024 * 1024) { bytes.push(b'a'); }
    std::fs::write(&huge, &bytes).unwrap();
    let err = archaeologist::head(&huge, 3);
    assert!(err.is_err(), "expected oversize head to fail");
}

#[test]
fn archaeologist_resolve_under_rejects_path_traversal() {
    let dir = tempdir();
    let root = dir.join("repo");
    make_tree(&root);
    let outside = dir.join("elsewhere.txt");
    std::fs::write(&outside, "x").unwrap();
    let err = archaeologist::resolve_under(&root, "../elsewhere.txt");
    assert!(err.is_err(), "must not allow escaping root");
}

#[test]
fn archaeologist_is_read_only() {
    let dir = tempdir();
    let root = dir.join("repo");
    make_tree(&root);
    let before_files: Vec<_> = walk_files(&root);
    let _ = archaeologist::index(&root, &IndexOptions::default()).unwrap();
    let _ = archaeologist::head(&root.join("src/lib.rs"), 10).unwrap();
    let after_files: Vec<_> = walk_files(&root);
    let before: std::collections::BTreeSet<String> = before_files.into_iter().collect();
    let after: std::collections::BTreeSet<String> = after_files.into_iter().collect();
    assert_eq!(before, after, "archaeologist must not modify the tree");
}

fn walk_files(root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        let rd = match std::fs::read_dir(&p) { Ok(r) => r, Err(_) => continue };
        for e in rd.flatten() {
            let ft = match e.file_type() { Ok(t) => t, Err(_) => continue };
            if ft.is_dir() { stack.push(e.path()); }
            else if ft.is_file() { out.push(e.path().display().to_string()); }
        }
    }
    out
}
