//! Phase 5 tests: projects, notes, tasks, memory, and project-context for chat.
//!
//! All unit tests. Live tests live in `phase5_live.rs`.

mod common;

use common::tempdir;
use nexus_core::conversations;
use nexus_core::db;
use nexus_core::memory::{self, MemoryScope};
use nexus_core::projects::{notes, projects, tasks, ui_state, workspaces};

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

#[test]
fn workspace_crud_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    let w = workspaces::create(&pool, "Personal", Some("home lab")).unwrap();
    assert_eq!(w.name, "Personal");
    workspaces::rename(&pool, &w.id, "Personal II").unwrap();
    let list = workspaces::list(&pool).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Personal II");
    workspaces::delete(&pool, &w.id).unwrap();
    assert!(workspaces::list(&pool).unwrap().is_empty());
}

#[test]
fn project_crud_with_goals_and_instructions() {
    let dir = tempdir();
    let pool = pool(&dir);
    let w = workspaces::create(&pool, "Lab", None).unwrap();
    let p = projects::create(
        &pool,
        Some(&w.id),
        "Paper Reading",
        Some("weekly paper review"),
        Some("Always cite page numbers and quote at least one sentence verbatim."),
        &["finish the review by Friday".into(), "summarise in 200 words".into()],
    ).unwrap();
    assert_eq!(p.goals.len(), 2);

    let updated = projects::update(
        &pool,
        &p.id,
        Some("Paper Reading 2"),
        None,
        None,
        Some(&["finish by Friday".into()]),
    ).unwrap();
    assert_eq!(updated.name, "Paper Reading 2");
    assert_eq!(updated.goals, vec!["finish by Friday".to_string()]);

    projects::set_archived(&pool, &p.id, true).unwrap();
    let all = projects::list(&pool, true).unwrap();
    assert!(all.iter().any(|p| p.id == p.id && p.archived));
    let visible = projects::list(&pool, false).unwrap();
    assert!(visible.iter().all(|p| p.id != p.id));
}

#[test]
fn notes_crud_and_project_filter() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Demo", None, None, &[]).unwrap();
    let n1 = notes::create(&pool, Some(&p.id), "First", "Body A").unwrap();
    let n2 = notes::create(&pool, Some(&p.id), "Second", "Body B").unwrap();
    let n3 = notes::create(&pool, None, "Unattached", "Body C").unwrap();

    let in_project = notes::list(&pool, Some(&p.id)).unwrap();
    assert_eq!(in_project.len(), 2);
    let all = notes::list(&pool, None).unwrap();
    assert_eq!(all.len(), 3);

    let updated = notes::update(&pool, &n1.id, Some("Renamed"), None).unwrap();
    assert_eq!(updated.title, "Renamed");
    let _ = (n2, n3);
}

#[test]
fn tasks_crud_with_status_checklist_due() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "T", None, None, &[]).unwrap();
    let due = chrono::Utc::now().timestamp_millis() + 86_400_000;
    let checklist = vec![
        tasks::ChecklistItem { text: "first".into(), done: false },
        tasks::ChecklistItem { text: "second".into(), done: true },
    ];
    let t = tasks::create(
        &pool,
        Some(&p.id),
        "Ship Phase 5",
        Some("lots of small things"),
        tasks::TaskPriority::High,
        Some(due),
        &checklist,
    ).unwrap();
    assert_eq!(t.status, "open");
    assert_eq!(t.priority, "high");
    assert_eq!(t.checklist.len(), 2);

    let in_progress = tasks::set_status(&pool, &t.id, tasks::TaskStatus::InProgress).unwrap();
    assert_eq!(in_progress.status, "in_progress");

    let open = tasks::list(&pool, Some(&p.id), None, false).unwrap();
    assert_eq!(open.len(), 1);
    let with_done = tasks::list(&pool, Some(&p.id), None, true).unwrap();
    assert_eq!(with_done.len(), 1);

    let due_range = tasks::list_due_in_range(&pool, due - 1000, due + 1000).unwrap();
    assert!(due_range.iter().any(|x| x.id == t.id));
}

#[test]
fn memory_crud_and_approval_gating() {
    let dir = tempdir();
    let pool = pool(&dir);
    let m_user = memory::create(&pool, MemoryScope::User, None, None, "Always respond in English.", Some("user"), true).unwrap();
    let m_auto = memory::create(&pool, MemoryScope::Auto, None, None, "User prefers dark mode.", Some("auto"), false).unwrap();

    let only_approved = memory::list(&pool, None, true).unwrap();
    assert!(only_approved.iter().any(|m| m.id == m_user.id));
    assert!(only_approved.iter().all(|m| m.id != m_auto.id));

    let approved = memory::approve(&pool, &m_auto.id).unwrap();
    assert!(approved.approved);

    let ctx = memory::list_for_context(&pool, None, None).unwrap();
    assert!(ctx.iter().any(|m| m.id == m_user.id));
    assert!(ctx.iter().any(|m| m.id == approved.id));
}

#[test]
fn ui_state_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    let v: Option<String> = ui_state::get(&pool, "active_project").unwrap();
    assert!(v.is_none());
    ui_state::set(&pool, "active_project", &"abc123".to_string()).unwrap();
    let v: Option<String> = ui_state::get(&pool, "active_project").unwrap();
    assert_eq!(v.as_deref(), Some("abc123"));
    ui_state::unset(&pool, "active_project").unwrap();
    let v: Option<String> = ui_state::get(&pool, "active_project").unwrap();
    assert!(v.is_none());
}

#[test]
fn conversation_attaches_to_project() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Attached", None, None, &[]).unwrap();
    let c = conversations::create(&pool, Some("Chat".into()), None, None).unwrap();
    assert!(c.project_id.is_none());
    conversations::set_project(&pool, &c.id, Some(&p.id)).unwrap();
    let fetched = conversations::get(&pool, &c.id).unwrap();
    assert_eq!(fetched.project_id.as_deref(), Some(p.id.as_str()));
}

#[test]
fn project_context_block_surfaces_goals_and_notes() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(
        &pool,
        None,
        "Paper Reading",
        None,
        Some("Always cite page numbers."),
        &["finish by Friday".into()],
    ).unwrap();
    let _ = notes::create(&pool, Some(&p.id), "today's note", "Read the PDF and capture key claims.").unwrap();
    let block = nexus_core::projects::context::build_project_context_block(&pool, &p.id).unwrap();
    assert!(block.contains("Paper Reading"), "block: {block}");
    assert!(block.contains("Always cite page numbers"));
    assert!(block.contains("finish by Friday"));
    assert!(block.contains("today's note"));
}

#[test]
fn memory_context_block_includes_approved_only() {
    let dir = tempdir();
    let pool = pool(&dir);
    let p = projects::create(&pool, None, "Demo", None, None, &[]).unwrap();
    let _ = memory::create(&pool, MemoryScope::User, None, None, "Loves concise answers.", Some("user"), true).unwrap();
    let _ = memory::create(&pool, MemoryScope::Project, Some(&p.id), None, "Project is about the Roman Empire.", Some("user"), true).unwrap();
    let _ = memory::create(&pool, MemoryScope::Project, Some(&p.id), None, "Should not appear.", Some("auto"), false).unwrap();
    let block = nexus_core::projects::context::build_memory_context_block(&pool, Some(&p.id), None).unwrap();
    assert!(block.contains("Loves concise answers"));
    assert!(block.contains("Roman Empire"));
    assert!(!block.contains("Should not appear"));
}
