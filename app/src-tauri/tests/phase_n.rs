//! Phase N — Operations & policy surface.
//!
//! Tests the boot path used by the real Tauri binary:
//!   * `AppState::initialize` migrates the schema and writes the
//!     default `config.toml`.
//!   * `audit::log` + `audit::recent` round-trip.
//!   * The new `audit_retention_days` + `audit_prune_on_boot` settings
//!     actually prune on the next `initialize`.
//!   * The new `file_allowlist` and `shell_allowlist` survive a
//!     config round-trip.

mod common;

use common::tempdir;
use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::config::AppConfig;
use nexus_core::db;

#[test]
fn app_state_initialize_migrates_and_persists_config() {
    let dir = tempdir();
    let state = AppState::initialize(&dir).expect("init");
    // DB exists.
    let db_path = db::default_db_path(&dir);
    assert!(db_path.exists(), "sqlite db should exist after initialize");
    // Config file is written.
    let cfg_path = AppConfig::path_for(&dir);
    assert!(cfg_path.exists(), "config.toml should be written");
    // Default schema is at v11 (last migration is 0011_agent.sql).
    let conn = state.pool.get().unwrap();
    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(version >= 11, "schema should be at v11+, got {version}");
}

#[test]
fn audit_log_round_trip() {
    let dir = tempdir();
    let state = AppState::initialize(&dir).expect("init");
    audit::log(
        &state.pool,
        "user",
        "test.event",
        Some("target"),
        serde_json::json!({ "k": 1 }),
    )
    .unwrap();
    let recent = audit::recent(&state.pool, 10).unwrap();
    assert!(recent.iter().any(|e| e.action == "test.event" && e.actor == "user"));
}

#[test]
fn boot_time_audit_prune_removes_old_events() {
    let dir = tempdir();

    // First boot: write config with retention = 30 days, prune on boot.
    {
        let mut cfg = AppConfig::load(&dir).unwrap();
        cfg.agent.audit_retention_days = 30;
        cfg.agent.audit_prune_on_boot = true;
        cfg.save(&dir).unwrap();
    }

    // Initialize once to materialise the DB and insert the boot event.
    let state = AppState::initialize(&dir).expect("init");

    // Insert one "ancient" event directly.
    let conn = state.pool.get().unwrap();
    let ancient: i64 = 1; // 1970-01-01
    conn.execute(
        "INSERT INTO audit_events (id, actor, action, target, detail, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params!["ancient", "user", "old", "t", "{}", ancient],
    )
    .unwrap();
    drop(conn);

    // Re-initialize with the same config — the boot-time prune should
    // remove the ancient row but keep the boot event.
    let _ = AppState::initialize(&dir).expect("init2");
    let recent = audit::recent(&state.pool, 100).unwrap();
    assert!(
        !recent.iter().any(|e| e.id == "ancient"),
        "ancient event should have been pruned by retention"
    );
    assert!(
        recent.iter().any(|e| e.action == "app.boot"),
        "boot event should remain"
    );
}

#[test]
fn file_and_shell_allowlists_round_trip() {
    let dir = tempdir();
    let mut cfg = AppConfig::load(&dir).unwrap();
    cfg.agent.file_allowlist = vec!["C:\\notes".into(), "D:\\datasets".into()];
    cfg.agent.shell_allowlist = vec!["git".into(), "rg".into()];
    cfg.save(&dir).unwrap();

    let _ = AppState::initialize(&dir).expect("init");
    let reloaded = AppConfig::load(&dir).unwrap();
    assert_eq!(reloaded.agent.file_allowlist, cfg.agent.file_allowlist);
    assert_eq!(reloaded.agent.shell_allowlist, cfg.agent.shell_allowlist);
}
