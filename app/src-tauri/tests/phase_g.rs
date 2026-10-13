//! Phase G: research studio.

mod common;

use common::tempdir;
use nexus_core::db;
use nexus_core::research;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn new_project(p: &nexus_core::db::SqlitePool, title: &str) -> research::ResearchProject {
    let now = chrono::Utc::now().timestamp_millis();
    research::projects::create(p, &research::ResearchProject {
        id: String::new(),
        workspace_id: None,
        title: title.into(),
        question: "Does X improve Y?".into(),
        description: None,
        status: "active".into(),
        created_at: now,
        updated_at: now,
    }).unwrap()
}

#[test]
fn project_create_list_get_update_delete() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = new_project(&pool, "My review");
    assert!(!p.id.is_empty());
    let list = research::projects::list(&pool, None, None, 50).unwrap();
    assert_eq!(list.len(), 1);
    let fetched = research::projects::get(&pool, &p.id).unwrap();
    assert_eq!(fetched.title, "My review");
    let updated = research::projects::update(&pool, &research::ResearchProject {
        status: "archived".into(),
        ..fetched.clone()
    }).unwrap();
    assert_eq!(updated.status, "archived");
    research::projects::delete(&pool, &p.id).unwrap();
    let list2 = research::projects::list(&pool, None, None, 50).unwrap();
    assert_eq!(list2.len(), 0);
}

#[test]
fn paper_crud_and_position() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = new_project(&pool, "P");
    let paper = research::papers::add(&pool, &research::ResearchPaper {
        id: String::new(),
        research_project_id: p.id.clone(),
        document_id: None,
        title: "Paper A".into(),
        authors: Some("A. Author".into()),
        venue: Some("NeurIPS".into()),
        year: Some(2024),
        doi: Some("10.1/abc".into()),
        url: None,
        notes: None,
        position: 0,
        created_at: 0,
    }).unwrap();
    let paper2 = research::papers::add(&pool, &research::ResearchPaper {
        id: String::new(),
        research_project_id: p.id.clone(),
        document_id: None,
        title: "Paper B".into(),
        authors: None,
        venue: None,
        year: None,
        doi: None,
        url: None,
        notes: None,
        position: 1,
        created_at: 0,
    }).unwrap();
    let list = research::papers::list_for(&pool, &p.id).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, paper.id);
    assert_eq!(list[1].id, paper2.id);
    research::papers::delete(&pool, &paper.id).unwrap();
    let list2 = research::papers::list_for(&pool, &p.id).unwrap();
    assert_eq!(list2.len(), 1);
}

#[test]
fn claim_grouped_by_category() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = new_project(&pool, "P");
    let paper = research::papers::add(&pool, &research::ResearchPaper {
        id: String::new(),
        research_project_id: p.id.clone(),
        document_id: None,
        title: "Paper A".into(),
        authors: None, venue: None, year: None, doi: None, url: None, notes: None,
        position: 0, created_at: 0,
    }).unwrap();
    research::claims::add(&pool, &research::PaperClaim {
        id: String::new(),
        research_project_id: p.id.clone(),
        paper_id: Some(paper.id.clone()),
        document_id: None,
        claim: "X improves Y by 30%".into(),
        category: "finding".into(),
        evidence: Some("p. 4, Fig. 2".into()),
        confidence: 0.9,
        kg_entity_id: None,
        created_at: 0,
    }).unwrap();
    research::claims::add(&pool, &research::PaperClaim {
        id: String::new(),
        research_project_id: p.id.clone(),
        paper_id: None,
        document_id: None,
        claim: "Open question: scaling".into(),
        category: "question".into(),
        evidence: None,
        confidence: 0.6,
        kg_entity_id: None,
        created_at: 0,
    }).unwrap();
    let findings = research::claims::list_for(&pool, &p.id, Some("finding"), None).unwrap();
    assert_eq!(findings.len(), 1);
    let all = research::claims::list_for(&pool, &p.id, None, None).unwrap();
    assert_eq!(all.len(), 2);
    let only_paper = research::claims::list_for(&pool, &p.id, None, Some(&paper.id)).unwrap();
    assert_eq!(only_paper.len(), 1);
}

#[test]
fn matrix_crud_and_cell_upsert() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = new_project(&pool, "P");
    let paper_a = research::papers::add(&pool, &research::ResearchPaper {
        id: String::new(),
        research_project_id: p.id.clone(),
        document_id: None,
        title: "A".into(),
        authors: None, venue: None, year: None, doi: None, url: None, notes: None,
        position: 0, created_at: 0,
    }).unwrap();
    let paper_b = research::papers::add(&pool, &research::ResearchPaper {
        id: String::new(),
        research_project_id: p.id.clone(),
        document_id: None,
        title: "B".into(),
        authors: None, venue: None, year: None, doi: None, url: None, notes: None,
        position: 1, created_at: 0,
    }).unwrap();
    let m = research::matrices::create_matrix(&pool, &research::ComparisonMatrix {
        id: String::new(),
        research_project_id: p.id.clone(),
        title: "Methods".into(),
        description: None,
        created_at: 0,
    }).unwrap();
    let c1 = research::matrices::add_column(&pool, &research::MatrixColumn {
        id: String::new(),
        matrix_id: m.id.clone(),
        label: "Sample size".into(),
        kind: "number".into(),
        position: 0,
    }).unwrap();
    let c2 = research::matrices::add_column(&pool, &research::MatrixColumn {
        id: String::new(),
        matrix_id: m.id.clone(),
        label: "Open source".into(),
        kind: "bool".into(),
        position: 1,
    }).unwrap();
    let cell1 = research::matrices::set_cell(&pool, &research::MatrixCell {
        id: String::new(),
        matrix_id: m.id.clone(),
        paper_id: paper_a.id.clone(),
        column_id: c1.id.clone(),
        value_text: None, value_number: Some(1000.0), value_bool: None,
        updated_at: 0,
    }).unwrap();
    let cell1_again = research::matrices::set_cell(&pool, &research::MatrixCell {
        id: String::new(),
        matrix_id: m.id.clone(),
        paper_id: paper_a.id.clone(),
        column_id: c1.id.clone(),
        value_text: None, value_number: Some(2500.0), value_bool: None,
        updated_at: 0,
    }).unwrap();
    // Upsert should reuse the same id.
    assert_eq!(cell1.id, cell1_again.id);
    assert_eq!(cell1_again.value_number, Some(2500.0));
    let cell2 = research::matrices::set_cell(&pool, &research::MatrixCell {
        id: String::new(),
        matrix_id: m.id.clone(),
        paper_id: paper_b.id.clone(),
        column_id: c2.id.clone(),
        value_text: None, value_number: None, value_bool: Some(true),
        updated_at: 0,
    }).unwrap();
    assert_eq!(cell2.value_bool, Some(true));
    let all = research::matrices::list_cells(&pool, &m.id).unwrap();
    assert_eq!(all.len(), 2);
    let cols = research::matrices::list_columns(&pool, &m.id).unwrap();
    assert_eq!(cols.len(), 2);
    research::matrices::delete_column(&pool, &c1.id).unwrap();
    let cols2 = research::matrices::list_columns(&pool, &m.id).unwrap();
    assert_eq!(cols2.len(), 1);
}

#[test]
fn report_save_and_build() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = new_project(&pool, "P");
    research::papers::add(&pool, &research::ResearchPaper {
        id: String::new(),
        research_project_id: p.id.clone(),
        document_id: None,
        title: "Paper A".into(),
        authors: Some("Author".into()),
        venue: None, year: Some(2024), doi: None, url: None, notes: None,
        position: 0, created_at: 0,
    }).unwrap();
    research::claims::add(&pool, &research::PaperClaim {
        id: String::new(),
        research_project_id: p.id.clone(),
        paper_id: None, document_id: None,
        claim: "Key finding".into(),
        category: "finding".into(), evidence: None, confidence: 0.5, kg_entity_id: None,
        created_at: 0,
    }).unwrap();
    let saved = research::reports::save(&pool, &research::ResearchReport {
        id: String::new(),
        research_project_id: p.id.clone(),
        title: "Snapshot 1".into(),
        body_md: "# body".into(),
        paper_ids: vec![],
        claim_ids: vec![],
        created_at: 0, updated_at: 0,
    }).unwrap();
    assert_eq!(saved.title, "Snapshot 1");
    let list = research::reports::list_for(&pool, &p.id).unwrap();
    assert_eq!(list.len(), 1);
    // Build the deterministic export.
    let md = research::markdown::build_project_report(&pool, &p.id).unwrap();
    assert!(md.contains("# P"));
    assert!(md.contains("Paper A"));
    assert!(md.contains("Key finding"));
    assert!(md.contains("## Claims"));
    research::reports::delete(&pool, &saved.id).unwrap();
    let list2 = research::reports::list_for(&pool, &p.id).unwrap();
    assert!(list2.is_empty());
}
