//! Phase H: experiment tracking.

mod common;

use common::tempdir;
use nexus_core::db;
use nexus_core::experiments;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

#[test]
fn experiment_crud() {
    let dir = tempdir();
    let pool = pool(&dir);
    let e = experiments::experiments::create(&pool, &experiments::Experiment {
        id: String::new(),
        project_id: None,
        name: "alpha".into(),
        description: Some("alpha run".into()),
        status: "active".into(),
        tags: vec!["llm".into(), "rag".into()],
        created_at: 0, updated_at: 0,
    }).unwrap();
    assert!(!e.id.is_empty());
    let fetched = experiments::experiments::get(&pool, &e.id).unwrap();
    assert_eq!(fetched.tags, vec!["llm".to_string(), "rag".to_string()]);
    let list = experiments::experiments::list(&pool, None, None, 50).unwrap();
    assert_eq!(list.len(), 1);
    experiments::experiments::update(&pool, &experiments::Experiment {
        status: "done".into(),
        ..fetched
    }).unwrap();
    experiments::experiments::delete(&pool, &e.id).unwrap();
    assert!(experiments::experiments::list(&pool, None, None, 50).unwrap().is_empty());
}

#[test]
fn run_crud_and_metric_history() {
    let dir = tempdir();
    let pool = pool(&dir);
    let e = experiments::experiments::create(&pool, &experiments::Experiment {
        id: String::new(), project_id: None, name: "e".into(), description: None, status: "active".into(), tags: vec![], created_at: 0, updated_at: 0,
    }).unwrap();
    let r1 = experiments::runs::create(&pool, &experiments::ExperimentRun {
        id: String::new(),
        experiment_id: e.id.clone(),
        name: "r1".into(),
        status: "completed".into(),
        hyperparams: serde_json::json!({"lr": 0.001, "batch": 16}),
        metrics: serde_json::json!({"loss": 0.42, "accuracy": 0.85}),
        code_revision: Some("abc123".into()),
        code_path: None,
        hardware: Some("RTX 4090".into()),
        notes: None,
        started_at: Some(0), ended_at: Some(1), created_at: 0, updated_at: 0,
    }).unwrap();
    let r2 = experiments::runs::create(&pool, &experiments::ExperimentRun {
        id: String::new(),
        experiment_id: e.id.clone(),
        name: "r2".into(),
        status: "completed".into(),
        hyperparams: serde_json::json!({"lr": 0.01, "batch": 32}),
        metrics: serde_json::json!({"loss": 0.30, "accuracy": 0.90}),
        code_revision: None, code_path: None, hardware: None, notes: None,
        started_at: Some(0), ended_at: Some(1), created_at: 0, updated_at: 0,
    }).unwrap();
    let list = experiments::runs::list_for(&pool, &e.id).unwrap();
    assert_eq!(list.len(), 2);
    // Update metrics.
    let mut r1_updated = r1.clone();
    r1_updated.metrics = serde_json::json!({"loss": 0.20, "accuracy": 0.92});
    let saved = experiments::runs::update(&pool, &r1_updated).unwrap();
    assert_eq!(saved.metrics["loss"].as_f64().unwrap(), 0.20);
    // History returns a row per (run, metric).
    let hist = experiments::runs::metric_history(&pool, &e.id).unwrap();
    // 2 runs × 2 metrics = 4 rows after the update.
    assert_eq!(hist.len(), 4);
    let losses: Vec<_> = hist.iter().filter(|p| p.metric == "loss").map(|p| p.value).collect();
    assert!(losses.contains(&0.20));
    assert!(losses.contains(&0.30));
    experiments::runs::delete(&pool, &r2.id).unwrap();
    let list2 = experiments::runs::list_for(&pool, &e.id).unwrap();
    assert_eq!(list2.len(), 1);
}
