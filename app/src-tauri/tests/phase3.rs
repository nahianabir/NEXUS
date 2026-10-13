//! Phase 3 tests: conversation and message persistence, plus profile resolution.

mod common;

use common::tempdir;
use nexus_core::conversations;
use nexus_core::db;
use nexus_core::model_profiles::{self, ModelProfile};

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn seed_profile(pool: &nexus_core::db::SqlitePool) -> ModelProfile {
    let p = model_profiles::default_general("qwen3.5-9b-test".to_string(), "lmstudio".to_string());
    model_profiles::upsert(pool, &p).unwrap();
    p
}

#[test]
fn conversation_crud_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    let c = conversations::create(&pool, Some("Test".into()), Some("You are a test assistant.".into()), None).unwrap();
    assert_eq!(c.title, "Test");
    assert_eq!(c.system_prompt.as_deref(), Some("You are a test assistant."));

    // Add a user + assistant message.
    let now = chrono::Utc::now().timestamp_millis();
    let u = conversations::Message {
        id: nexus_core::conversations::new_id(),
        conversation_id: c.id.clone(),
        role: "user".into(),
        content: "Hello".into(),
        status: "complete".into(),
        token_count: None, latency_ms: None, model_profile_id: None,
        created_at: now,
    };
    conversations::add_message(&pool, &u).unwrap();
    let a = conversations::Message {
        id: nexus_core::conversations::new_id(),
        conversation_id: c.id.clone(),
        role: "assistant".into(),
        content: "Hi there".into(),
        status: "complete".into(),
        token_count: Some(8), latency_ms: Some(120), model_profile_id: None,
        created_at: now + 1,
    };
    conversations::add_message(&pool, &a).unwrap();

    let list = conversations::list_messages(&pool, &c.id, 100).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].role, "user");
    assert_eq!(list[1].role, "assistant");

    // Rename and re-fetch.
    conversations::rename(&pool, &c.id, "Renamed").unwrap();
    let re = conversations::get(&pool, &c.id).unwrap();
    assert_eq!(re.title, "Renamed");

    // Archive and verify it disappears from the default list.
    conversations::set_archived(&pool, &c.id, true).unwrap();
    let visible = conversations::list(&pool, false).unwrap();
    assert!(visible.iter().all(|x| x.id != c.id));
    let hidden = conversations::list(&pool, true).unwrap();
    assert!(hidden.iter().any(|x| x.id == c.id));

    // Delete and confirm.
    conversations::delete(&pool, &c.id).unwrap();
    let after = conversations::list(&pool, true).unwrap();
    assert!(after.iter().all(|x| x.id != c.id));
}

#[test]
fn update_message_status_and_content() {
    let dir = tempdir();
    let pool = pool(&dir);
    let c = conversations::create(&pool, None, None, None).unwrap();
    let m = conversations::Message {
        id: nexus_core::conversations::new_id(),
        conversation_id: c.id.clone(),
        role: "assistant".into(),
        content: String::new(),
        status: "streaming".into(),
        token_count: None, latency_ms: None, model_profile_id: None,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    conversations::add_message(&pool, &m).unwrap();
    conversations::update_message(&pool, &m.id, "partial", "streaming").unwrap();
    conversations::update_message(&pool, &m.id, "final answer", "complete").unwrap();
    let list = conversations::list_messages(&pool, &c.id, 10).unwrap();
    assert_eq!(list[0].content, "final answer");
    assert_eq!(list[0].status, "complete");
}

#[test]
fn profile_resolution_picks_general_first() {
    let dir = tempdir();
    let pool = pool(&dir);
    let g = seed_profile(&pool);
    let c = conversations::create(&pool, None, None, None).unwrap();
    let list = model_profiles::list(&pool).unwrap();
    let resolved = list.iter().find(|p| p.role == "general" && p.enabled).unwrap();
    assert_eq!(resolved.id, g.id);
    let _ = c;
}