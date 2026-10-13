//! Phase D: knowledge graph. CRUD, alias resolution, evidence,
//! bounded traversal, merge history, and source-record resolution.

mod common;

use common::tempdir;
use nexus_core::db;
use nexus_core::kg;
use nexus_core::kg::evidence::CreateEvidence;
use nexus_core::projects::projects;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

#[test]
fn entity_crud_and_alias_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    let e = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None,
        entity_type: "model".into(),
        canonical_name: "Qwen 2.5 Coder 14B".into(),
        description: Some("Coding assistant GGUF".into()),
        metadata_json: None,
        aliases: vec!["qwen-coder".into()],
    }).unwrap();
    let got = kg::graph::get(&pool, &e.id).unwrap();
    assert_eq!(got.canonical_name, "Qwen 2.5 Coder 14B");
    // The canonical name normalises to "qwen 25 coder 14b" (punctuation
    // stripped, no whitespace inserted between digits).
    let found = kg::graph::find_by_normalized(&pool, "qwen 25 coder 14b", "model").unwrap();
    assert!(found.is_some(), "canonical-name lookup must succeed");
    // The alias "qwen-coder" normalises to "qwencoder".
    let found2 = kg::graph::find_by_normalized(&pool, "qwencoder", "model").unwrap();
    assert!(found2.is_some(), "alias lookup must succeed");
    // The alias with case folding also matches.
    let found3 = kg::graph::find_by_normalized(&pool, "QWEN-CODER", "model").unwrap();
    assert!(found3.is_some(), "case folding must not block the lookup");
}

#[test]
fn relationship_upserts_and_evidence_persists() {
    let dir = tempdir();
    let pool = pool(&dir);
    let a = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "model".into(), canonical_name: "Llama 3".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    let b = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "dataset".into(), canonical_name: "WikiText".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    let r = kg::relationship::create(&pool, &kg::CreateRelationship {
        source_entity_id: a.id.clone(),
        relationship_type: "USES_DATASET".into(),
        target_entity_id: b.id.clone(),
        evidence: vec![CreateEvidence {
            source_kind: "document".into(),
            source_id: Some("paper-1".into()),
            source_location: Some("p.3".into()),
            excerpt: Some("We trained on WikiText-103.".into()),
        }],
        ..Default::default()
    }).unwrap();
    let ev = kg::evidence::list_for(&pool, &r.id).unwrap();
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].source_kind, "document");
    // Calling create again should upsert, not create a duplicate.
    let r2 = kg::relationship::create(&pool, &kg::CreateRelationship {
        source_entity_id: a.id.clone(),
        relationship_type: "USES_DATASET".into(),
        target_entity_id: b.id.clone(),
        evidence: vec![],
        ..Default::default()
    }).unwrap();
    assert_eq!(r.id, r2.id);
    let ev2 = kg::evidence::list_for(&pool, &r.id).unwrap();
    assert_eq!(ev2.len(), 1, "re-creating must not duplicate the existing evidence");
}

#[test]
fn neighborhood_is_bounded_and_terminates() {
    let dir = tempdir();
    let pool = pool(&dir);
    let center = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "project".into(), canonical_name: "Center".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    // Build a 3-hop chain.
    let mut prev = center.id.clone();
    for i in 0..10 {
        let n = kg::graph::create(&pool, &kg::CreateEntity {
            workspace_id: None, entity_type: "memory".into(), canonical_name: format!("N{i}"),
            description: None, metadata_json: None, aliases: vec![],
        }).unwrap();
        kg::relationship::create(&pool, &kg::CreateRelationship {
            source_entity_id: prev.clone(), relationship_type: "REFERENCES".into(),
            target_entity_id: n.id.clone(), ..Default::default()
        }).unwrap();
        prev = n.id;
    }
    let hood = kg::queries::neighborhood(&pool, &center.id, kg::Bounded { max_depth: 2, max_nodes: 50 }).unwrap();
    assert!(hood.nodes.len() <= 50);
    assert!(hood.nodes.iter().all(|n| n.id == center.id || hood.edges.iter().any(|e| e.source_entity_id == n.id || e.target_entity_id == n.id)));
}

#[test]
fn shortest_path_finds_known_chain() {
    let dir = tempdir();
    let pool = pool(&dir);
    let a = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "project".into(), canonical_name: "A".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    let b = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "project".into(), canonical_name: "B".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    let c = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "project".into(), canonical_name: "C".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    kg::relationship::create(&pool, &kg::CreateRelationship {
        source_entity_id: a.id.clone(), relationship_type: "REFERENCES".into(), target_entity_id: b.id.clone(),
        ..Default::default()
    }).unwrap();
    kg::relationship::create(&pool, &kg::CreateRelationship {
        source_entity_id: b.id.clone(), relationship_type: "REFERENCES".into(), target_entity_id: c.id.clone(),
        ..Default::default()
    }).unwrap();
    let path = kg::queries::shortest_path(&pool, &a.id, &c.id, 4).unwrap();
    assert_eq!(path, vec![a.id.clone(), b.id.clone(), c.id.clone()]);
    let none = kg::queries::shortest_path(&pool, &a.id, &c.id, 1).unwrap();
    assert!(none.is_empty(), "max_depth=1 must not find a 2-hop path");
}

#[test]
fn merge_migrates_relationships_and_records_audit() {
    let dir = tempdir();
    let pool = pool(&dir);
    let winner = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "model".into(), canonical_name: "Winner".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    let loser = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "model".into(), canonical_name: "Loser".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    let dataset = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "dataset".into(), canonical_name: "Data".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    let r = kg::relationship::create(&pool, &kg::CreateRelationship {
        source_entity_id: loser.id.clone(), relationship_type: "USES_DATASET".into(),
        target_entity_id: dataset.id.clone(), ..Default::default()
    }).unwrap();
    let winner2 = kg::graph::merge(&pool, &kg::MergeRequest { winner_id: winner.id.clone(), loser_id: loser.id.clone(), reason: Some("same model".into()) }).unwrap();
    assert_eq!(winner2.id, winner.id);
    // The relationship must now point at the winner.
    let merged = kg::relationship::get(&pool, &r.id).unwrap();
    assert_eq!(merged.source_entity_id, winner.id);
    // The loser is preserved with `merged_into`.
    let loser_after = kg::graph::get(&pool, &loser.id).unwrap();
    assert_eq!(loser_after.merged_into.as_deref(), Some(winner.id.as_str()));
    // List with include_merged=false omits the loser.
    let live = kg::graph::list(&pool, &kg::ListFilter { include_merged: false, ..Default::default() }).unwrap();
    assert!(live.iter().all(|e| e.id != loser.id));
}

#[test]
fn seed_project_creates_entity_with_metadata() {
    let dir = tempdir();
    let pool = pool(&dir);
    let project = projects::create(&pool, None, "Demo Project", None, None, &[]).unwrap();
    let e = kg::extract::seed_project(&pool, &project.id).unwrap();
    assert_eq!(e.entity_type, "project");
    assert_eq!(e.canonical_name, "Demo Project");
    let meta = e.metadata_json.unwrap();
    assert!(meta.contains(&project.id));
    // Re-seeding is idempotent.
    let e2 = kg::extract::seed_project(&pool, &project.id).unwrap();
    assert_eq!(e.id, e2.id);
}

#[test]
fn projects_using_entity_returns_only_projects() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "P", None, None, &[]).unwrap();
    let e = kg::extract::seed_project(&pool, &p.id).unwrap();
    let dataset = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "dataset".into(), canonical_name: "Data".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    kg::relationship::create(&pool, &kg::CreateRelationship {
        source_entity_id: e.id.clone(), relationship_type: "USES_DATASET".into(),
        target_entity_id: dataset.id.clone(), ..Default::default()
    }).unwrap();
    let projects = kg::queries::projects_using_entity(&pool, &dataset.id, 10).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, e.id);
}

#[test]
fn stats_counts_omit_merged() {
    let dir = tempdir();
    let pool = pool(&dir);
    let a = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "x".into(), canonical_name: "A".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    let b = kg::graph::create(&pool, &kg::CreateEntity {
        workspace_id: None, entity_type: "x".into(), canonical_name: "B".into(),
        description: None, metadata_json: None, aliases: vec![],
    }).unwrap();
    kg::graph::merge(&pool, &kg::MergeRequest { winner_id: a.id.clone(), loser_id: b.id.clone(), reason: None }).unwrap();
    let s = kg::queries::stats(&pool).unwrap();
    assert_eq!(s.entities, 1, "merged entities should not be counted");
}
