//! Phase I: controlled agent.

mod common;

use common::tempdir;
use nexus_core::agent;
use nexus_core::db;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

#[test]
fn seed_tools_creates_known_set() {
    let dir = tempdir();
    let pool = pool(&dir);
    agent::ensure_seed_tools(&pool).unwrap();
    let tools = agent::registry::list(&pool, None, false).unwrap();
    let names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
    assert!(names.iter().any(|n| n == "vault.search"));
    assert!(names.iter().any(|n| n == "kg.search"));
    assert!(names.iter().any(|n| n == "memory.recall"));
    assert!(names.iter().any(|n| n == "shell.run"));
    assert!(names.iter().any(|n| n == "git.status"));
    assert!(names.iter().any(|n| n == "settings.get"));
    assert!(names.iter().any(|n| n == "settings.set"));
}

#[test]
fn policy_always_runs_without_approval() {
    let dir = tempdir();
    let pool = pool(&dir);
    agent::ensure_seed_tools(&pool).unwrap();
    let decision = agent::policy::evaluate(&pool, "vault.search").unwrap();
    assert!(matches!(decision.decision, agent::policy::PolicyDecision::Allow));
    let decision2 = agent::policy::evaluate(&pool, "shell.run").unwrap();
    assert!(matches!(decision2.decision, agent::policy::PolicyDecision::RequireApproval));
}

#[test]
fn disabled_tool_is_denied() {
    let dir = tempdir();
    let pool = pool(&dir);
    agent::ensure_seed_tools(&pool).unwrap();
    agent::registry::set_enabled(&pool, "vault.search", false).unwrap();
    let d = agent::policy::evaluate(&pool, "vault.search").unwrap();
    assert!(matches!(d.decision, agent::policy::PolicyDecision::Deny));
}

#[test]
fn run_and_invocation_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    agent::ensure_seed_tools(&pool).unwrap();
    let run = agent::runs::create(&pool, &agent::AgentRun {
        id: String::new(), conversation_id: None, project_id: None,
        title: "test".into(), status: "running".into(),
        created_at: 0, updated_at: 0, finished_at: None,
    }).unwrap();
    let inv = agent::invocations::create(&pool, &agent::Invocation {
        id: String::new(),
        run_id: run.id.clone(),
        tool_name: "vault.search".into(),
        permission: "always".into(),
        input: serde_json::json!({ "query": "hello" }),
        output: None,
        status: "pending".into(),
        error: None,
        started_at: 0,
        finished_at: None,
    }).unwrap();
    let fetched = agent::invocations::get(&pool, &inv.id).unwrap();
    assert_eq!(fetched.status, "pending");
    let list = agent::invocations::list_for_run(&pool, &run.id).unwrap();
    assert_eq!(list.len(), 1);
    agent::invocations::update_status(&pool, &inv.id, "executed", Some(&serde_json::json!({"chunks": 0})), None, true).unwrap();
    let after = agent::invocations::get(&pool, &inv.id).unwrap();
    assert_eq!(after.status, "executed");
    assert!(after.finished_at.is_some());
    agent::runs::set_status(&pool, &run.id, "completed", true).unwrap();
    let run2 = agent::runs::get(&pool, &run.id).unwrap();
    assert_eq!(run2.status, "completed");
    assert!(run2.finished_at.is_some());
}

#[test]
fn allowlist_rejects_unknown_path_and_command() {
    let dir = tempdir();
    let pool = pool(&dir);
    agent::ensure_seed_tools(&pool).unwrap();
    // Create a real file so canonicalize succeeds.
    let real = dir.join("ok.txt");
    std::fs::write(&real, b"hello").unwrap();
    let allow: Vec<String> = vec![dir.to_string_lossy().to_string()];
    let ok = agent::policy::check_path_allowed(&real.to_string_lossy(), &allow);
    assert!(ok.is_ok(), "expected {real:?} to be allowed");
    let bad = agent::policy::check_path_allowed("/etc/passwd", &allow);
    assert!(bad.is_err());
    let cmd_ok = agent::policy::check_command_allowed("git", &vec!["git".into(), "echo".into()]);
    assert!(cmd_ok.is_ok());
    let cmd_bad = agent::policy::check_command_allowed("rm", &vec!["git".into()]);
    assert!(cmd_bad.is_err());
}
