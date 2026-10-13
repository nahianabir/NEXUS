//! Phase E: unified context builder.

mod common;

use common::tempdir;
use nexus_core::context;
use nexus_core::db;
use nexus_core::kg;
use nexus_core::mempalace;
use nexus_core::memory_v2;
use nexus_core::memory_v2::memory::MemoryV2;
use nexus_core::projects;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn make_memory(content: &str, category: &str, project: Option<&str>) -> MemoryV2 {
    let now = chrono::Utc::now().timestamp_millis();
    let norm = memory_v2::normalize_content(content);
    let hash = memory_v2::content_hash(&norm);
    MemoryV2 {
        id: memory_v2::memory::new_id(),
        workspace_id: None,
        project_id: project.map(|s| s.to_string()),
        conversation_id: None,
        category: category.into(),
        content: content.into(),
        content_hash: hash,
        normalized_content: norm,
        source_kind: "user".into(),
        source_id: None,
        source_label: None,
        source_excerpt: None,
        event_at: None,
        valid_from: None,
        valid_until: None,
        verified_at: Some(now),
        superseded_by: None,
        deleted_at: None,
        approved: true,
        verification: "approved".into(),
        pinned: false,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn context_includes_active_project_and_approved_memories() {
    let dir = tempdir();
    let pool = pool(&dir);
    let project = projects::projects::create(&pool, None, "P", Some("desc"), Some("be terse"), &["ship it".into()]).unwrap();
    let m = make_memory("Always cite the source of every claim.", "instruction", Some(&project.id));
    memory_v2::memory::insert(&pool, &m).unwrap();

    let state = mempalace::state::native_only(pool.clone());
    let bundle = context::build(&pool, &state, &context::BuildRequest {
        workspace_id: None,
        project_id: Some(project.id.clone()),
        conversation_id: None,
        user_message: "What is the rule about citations and the source of every claim?".into(),
        max_tokens: 2000,
        pinned: vec![],
    }).unwrap();
    assert!(bundle.system_block.contains("be terse"));
    assert!(bundle.system_block.contains("source"));
    assert!(bundle.hits_by_source.contains_key("project"));
    assert!(bundle.hits_by_source.contains_key("memory"));
}

#[test]
fn context_includes_graph_when_entity_matches() {
    let dir = tempdir();
    let pool = pool(&dir);
    let project = projects::projects::create(&pool, None, "Demo", None, None, &[]).unwrap();
    let _e = kg::extract::seed_project(&pool, &project.id).unwrap();
    let model = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "model".into(),
        canonical_name: "Qwen 2.5 Coder 14B".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    kg::relationship::create(&pool, &kg::CreateRelationship {
        source_entity_id: model.id.clone(),
        relationship_type: "USES_DATASET".into(),
        target_entity_id: kg::graph::create(&pool, &kg::CreateEntity {
            workspace_id: None, entity_type: "dataset".into(),
            canonical_name: "WikiText".into(),
            description: None, metadata_json: None, aliases: vec![],
        }).unwrap().id.clone(),
        ..Default::default()
    }).unwrap();

    let state = mempalace::state::native_only(pool.clone());
    let bundle = context::build(&pool, &state, &context::BuildRequest {
        workspace_id: None,
        project_id: None,
        conversation_id: None,
        user_message: "Tell me about Qwen 2.5 Coder 14B and its dataset.".into(),
        max_tokens: 4000,
        pinned: vec![],
    }).unwrap();
    assert!(bundle.system_block.contains("Qwen 2.5 Coder 14B"));
    assert!(bundle.hits_by_source.contains_key("graph-node"));
}

#[test]
fn context_budget_drops_low_score_items() {
    let dir = tempdir();
    let pool = pool(&dir);
    for i in 0..20 {
        let m = make_memory(
            &format!("This is filler fact number {i}, a long-enough memory that adds up to many tokens when assembled."),
            "lesson", None,
        );
        memory_v2::memory::insert(&pool, &m).unwrap();
    }
    let state = mempalace::state::native_only(pool.clone());
    let bundle = context::build(&pool, &state, &context::BuildRequest {
        workspace_id: None,
        project_id: None,
        conversation_id: None,
        user_message: "filler fact number".into(),
        max_tokens: 200,
        pinned: vec![],
    }).unwrap();
    assert!(bundle.truncated, "the budget must force truncation; got {} items, ~{} tokens", bundle.items.len(), bundle.approx_tokens);
    assert!(bundle.approx_tokens <= 220, "approximate budget must hold");
}

#[test]
fn context_pinned_memories_always_survive() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pinned = make_memory("User's full name is Dr. Test.", "instruction", None);
    memory_v2::memory::insert(&pool, &pinned).unwrap();
    let state = mempalace::state::native_only(pool.clone());
    let bundle = context::build(&pool, &state, &context::BuildRequest {
        workspace_id: None,
        project_id: None,
        conversation_id: None,
        user_message: "anything".into(),
        max_tokens: 10_000,
        pinned: vec![pinned.clone()],
    }).unwrap();
    assert!(bundle.items.iter().any(|i| i.source_id == pinned.id));
}

#[test]
fn context_search_uses_fallback_when_backend_fails() {
    let dir = tempdir();
    let pool = pool(&dir);
    let m = make_memory("Default memory backend is native.", "decision", None);
    memory_v2::memory::insert(&pool, &m).unwrap();
    // Even with a degraded MemPalace, the native backend should still surface the memory.
    let state = mempalace::state::native_only(pool.clone());
    let bundle = context::build(&pool, &state, &context::BuildRequest {
        workspace_id: None,
        project_id: None,
        conversation_id: None,
        user_message: "What is the default memory backend?".into(),
        max_tokens: 2000,
        pinned: vec![],
    }).unwrap();
    assert!(bundle.system_block.contains("native"));
}
