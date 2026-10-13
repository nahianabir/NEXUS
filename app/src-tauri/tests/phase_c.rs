//! Phase C: MemPalace adapter. Native backend is always tested; the
//! MemPalace sidecar test is gated behind `#[ignore]` because the
//! Python sidecar cannot be installed in this build environment.

mod common;

use common::tempdir;
use nexus_core::db;
use nexus_core::mempalace::adapter::*;
use nexus_core::mempalace::native::NativeBackend;
use nexus_core::mempalace::sidecar::{MemPalaceBackend, MemPalaceConfig};
use nexus_core::memory_v2;
use ulid::Ulid;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn make_record(content: &str, project: Option<&str>) -> AdapterMemory {
    AdapterMemory {
        id: String::new(),
        backend: "native".into(),
        project_id: project.map(|s| s.to_string()),
        workspace_id: None,
        category: "project_fact".into(),
        content: content.into(),
        source_kind: "user".into(),
        source_id: None,
        source_excerpt: None,
        event_at: None,
        valid_from: None,
        valid_until: None,
        created_at: 0,
        updated_at: 0,
    }
}

#[test]
fn native_backend_health_is_true() {
    let dir = tempdir();
    let pool = pool(&dir);
    let backend = NativeBackend::new(pool);
    let diag = backend.health().expect("health");
    assert!(diag.healthy);
    assert_eq!(diag.backend, "native");
    assert_eq!(backend.kind(), BackendKind::Native);
}

#[test]
fn native_backend_search_after_store_returns_record() {
    let dir = tempdir();
    let pool = pool(&dir);
    let backend = NativeBackend::new(pool.clone());
    let id = backend.store(&make_record("Default backend is native.", None)).expect("store");
    assert!(!id.is_empty());
    let hits = backend.search("backend", None, 5).expect("search");
    assert!(hits.iter().any(|h| h.id == id && h.content.contains("native")));
}

#[test]
fn native_backend_delete_soft_removes() {
    let dir = tempdir();
    let pool = pool(&dir);
    let backend = NativeBackend::new(pool.clone());
    let id = backend.store(&make_record("to be deleted", None)).expect("store");
    backend.delete(&id).expect("delete");
    let hits = backend.search("deleted", None, 5).expect("search");
    assert!(!hits.iter().any(|h| h.id == id));
}

#[test]
fn native_backend_list_collections_returns_projects() {
    let dir = tempdir();
    let pool = pool(&dir);
    let backend = NativeBackend::new(pool.clone());
    nexus_core::projects::projects::create(&pool, None, "Demo", None, None, &[]).unwrap();
    let cols = backend.list_collections().expect("collections");
    assert!(cols.iter().any(|c| c.name == "Demo" && c.backend == "native"));
}

#[test]
fn backend_set_defaults_to_native() {
    let dir = tempdir();
    let pool = pool(&dir);
    let state = nexus_core::mempalace::state::native_only(pool);
    assert_eq!(state.current(), BackendKind::Native);
    let hits = state.search("nothing", None, 5);
    assert!(hits.is_empty());
}

#[test]
fn native_backend_dedupes_via_memory_v2() {
    let dir = tempdir();
    let pool = pool(&dir);
    let backend = NativeBackend::new(pool.clone());
    let id1 = backend.store(&make_record("Always  use  tauri  2 .", None)).expect("store");
    let id2 = backend.store(&make_record("always use tauri 2", None)).expect("store");
    assert_eq!(id1, id2, "normalized content must dedupe");
    let total = memory_v2::memory::list(&pool, &Default::default()).unwrap();
    assert_eq!(total.len(), 1);
    let _ = Ulid::new().to_string(); // touch ulid import
}

// The live MemPalace test. Gated because we cannot install the sidecar
// in this build environment. Run with:
//   `cargo test --test phase_c -- --ignored --nocapture`
#[test]
#[ignore = "requires the MemPalace sidecar; see docs/MEMPALACE_INTEGRATION.md"]
fn live_mempalace_health() {
    let backend = MemPalaceBackend::new(MemPalaceConfig {
        binary: std::env::var("MEMPALACE_BIN").unwrap_or_else(|_| "mempalace".into()),
        cwd: std::env::var("MEMPALACE_CWD").ok(),
        timeout_ms: 5_000,
        extra_args: vec![],
    });
    let diag = backend.health().expect("health");
    if diag.healthy {
        assert_eq!(diag.backend, "mempalace");
    } else {
        // The test is ignored but in the live environment we want to
        // assert that the message contains something useful.
        assert!(!diag.message.is_empty());
    }
}
