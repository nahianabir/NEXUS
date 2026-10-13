//! Phase F — Opportunity Radar feedback (dismiss / snooze / pin / done / false).
//! Phase G — Evidence Tribunal (read-only, no model).
//! Phase J — Knowledge Vault export (read-only, no destructive import).

mod common;

use common::tempdir;
use nexus_core::db::{self, SqlitePool};
use nexus_core::radar::{self, RadarOptions};
use nexus_core::radar_feedback::{self, FeedbackKind, UpsertFeedback};
use nexus_core::tribunal::{self, TribunalRequest};
use nexus_core::vault_export::{self, ExportRequest, ARCHIVE_FORMAT_VERSION};
use serde_json::json;

fn pool(dir: &std::path::Path) -> SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

fn seed_project(pool: &SqlitePool, name: &str) -> String {
    let conn = pool.get().unwrap();
    let id: String = ulid::Ulid::new().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO projects (id, name, archived, created_at, updated_at)
         VALUES (?1, ?2, 0, ?3, ?3)",
        rusqlite::params![id, name, now],
    ).unwrap();
    id
}

fn insert_task(pool: &SqlitePool, project_id: &str, title: &str, days_overdue: i64) {
    let conn = pool.get().unwrap();
    let id: String = ulid::Ulid::new().to_string();
    let now = now_ms();
    let due = now - days_overdue * 86_400_000;
    conn.execute(
        "INSERT INTO tasks
            (id, project_id, title, status, priority, due_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'open', 'urgent', ?4, ?5, ?5)",
        rusqlite::params![id, project_id, title, due, now],
    ).unwrap();
}

fn insert_note(pool: &SqlitePool, project_id: &str, body: &str) {
    let conn = pool.get().unwrap();
    let id: String = ulid::Ulid::new().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO notes (id, project_id, title, body, created_at, updated_at)
         VALUES (?1, ?2, 'note', ?3, ?4, ?4)",
        rusqlite::params![id, project_id, body, now],
    ).unwrap();
}

fn insert_memory(pool: &SqlitePool, project_id: &str, body: &str) {
    let conn = pool.get().unwrap();
    let id: String = ulid::Ulid::new().to_string();
    let now = now_ms();
    let hash = format!("{:x}", sha2_dummy(body));
    let normalized = body.to_lowercase();
    conn.execute(
        "INSERT INTO memories_v2
            (id, project_id, category, content, content_hash, normalized_content,
             source_kind, created_at, updated_at)
         VALUES (?1, ?2, 'lesson', ?3, ?4, ?5, 'user', ?6, ?6)",
        rusqlite::params![id, project_id, body, hash, normalized, now],
    ).unwrap();
}

fn insert_timeline(pool: &SqlitePool, project_id: &str, kind: &str, summary: &str) {
    let conn = pool.get().unwrap();
    let id: String = ulid::Ulid::new().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO timeline_events
            (id, project_id, event_type, source_kind, event_at, imported_at,
             summary, detail_json, created_at)
         VALUES (?1, ?2, ?3, 'manual', ?4, ?4, ?5, '{}', ?4)",
        rusqlite::params![id, project_id, kind, now, summary],
    ).unwrap();
}

fn sha2_dummy(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

// ---------------- Phase F ----------------

#[test]
fn radar_feedback_round_trip() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");

    let fb = radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(),
        key: "overdue_urgent_task:abc".into(),
        kind: FeedbackKind::Dismiss,
        title: Some("Overdue task X".into()),
        note: Some("not actionable right now".into()),
        until_ms: None,
    }).expect("upsert dismiss");
    assert_eq!(fb.kind, "dismiss");
    assert_eq!(fb.title.as_deref(), Some("Overdue task X"));

    // Idempotent re-upsert: a second call for the same key
    // updates the existing row instead of inserting a
    // duplicate.
    let fb2 = radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(),
        key: "overdue_urgent_task:abc".into(),
        kind: FeedbackKind::Pin,
        title: None, note: None, until_ms: None,
    }).expect("upsert pin");
    assert_eq!(fb2.id, fb.id);
    assert_eq!(fb2.kind, "pin");

    let list = radar_feedback::list(&pool, &radar_feedback::ListFilter {
        project_id: Some(pid.clone()),
        ..Default::default()
    }).expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, "pin");

    radar_feedback::delete(&pool, &fb.id).expect("delete");
    let after = radar_feedback::list(&pool, &radar_feedback::ListFilter {
        project_id: Some(pid.clone()),
        ..Default::default()
    }).expect("list2");
    assert!(after.is_empty());
}

#[test]
fn radar_feedback_rejects_empty_key() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    let r = radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid,
        key: "   ".into(),
        kind: FeedbackKind::Dismiss,
        title: None, note: None, until_ms: None,
    });
    assert!(r.is_err());
}

#[test]
fn radar_feedback_rules_for_classifies_pins_and_dismisses() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    let now = now_ms();
    let in_future = now + 7 * 86_400_000;
    let in_past   = now - 7 * 86_400_000;

    radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(), key: "k1".into(), kind: FeedbackKind::Dismiss,
        title: None, note: None, until_ms: None,
    }).unwrap();
    radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(), key: "k2".into(), kind: FeedbackKind::Pin,
        title: None, note: None, until_ms: None,
    }).unwrap();
    radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(), key: "k3".into(), kind: FeedbackKind::Snooze,
        title: None, note: None, until_ms: Some(in_future),
    }).unwrap();
    radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(), key: "k4".into(), kind: FeedbackKind::Snooze,
        title: None, note: None, until_ms: Some(in_past),
    }).unwrap();
    radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(), key: "k5".into(), kind: FeedbackKind::Done,
        title: None, note: None, until_ms: None,
    }).unwrap();
    let (exclude, pinned) = radar_feedback::rules_for(&pool, &pid, now).unwrap();
    assert!(exclude.contains("k1"));
    assert!(exclude.contains("k3"));
    assert!(exclude.contains("k5"));
    assert!(!exclude.contains("k4")); // expired snooze
    assert!(pinned.contains("k2"));
    assert!(!pinned.contains("k1"));
}

#[test]
fn radar_applies_feedback_dismiss_and_pin() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    insert_task(&pool, &pid, "task A", 5);
    insert_task(&pool, &pid, "task B", 10);

    let report = radar::report(&pool, &pid, &RadarOptions::default()).unwrap();
    assert!(report.items.len() >= 2, "two opportunities expected");
    for o in &report.items {
        let _ = radar_feedback::opportunity_key(&o.kind.as_str(), &o.refs[0].id);
        assert!(!o.key.is_empty());
    }

    let target_key = report.items[0].key.clone();
    radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(), key: target_key.clone(), kind: FeedbackKind::Dismiss,
        title: Some(report.items[0].title.clone()), note: None, until_ms: None,
    }).unwrap();

    let report2 = radar::report(&pool, &pid, &RadarOptions::default()).unwrap();
    assert!(report2.items.iter().all(|o| o.key != target_key));

    let remaining = report2.items[0].key.clone();
    radar_feedback::upsert(&pool, &UpsertFeedback {
        project_id: pid.clone(), key: remaining.clone(), kind: FeedbackKind::Pin,
        title: None, note: None, until_ms: None,
    }).unwrap();
    let report3 = radar::report(&pool, &pid, &RadarOptions::default()).unwrap();
    if let Some(first) = report3.items.first() {
        assert_eq!(first.key, remaining);
    }
}

#[test]
fn radar_feedback_kind_parse() {
    for (s, k) in [("dismiss", FeedbackKind::Dismiss),
                   ("snooze", FeedbackKind::Snooze),
                   ("pin", FeedbackKind::Pin),
                   ("done", FeedbackKind::Done),
                   ("false", FeedbackKind::False)] {
        assert_eq!(FeedbackKind::parse(s).unwrap(), k);
    }
    assert!(FeedbackKind::parse("nope").is_err());
}

// ---------------- Phase G ----------------

#[test]
fn tribunal_empty_when_nothing_matches() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    let report = tribunal::run(&pool, &TribunalRequest {
        question: "completely unrelated zzzqqqxxx".into(),
        project_id: Some(pid),
        repo_root: None,
        limit_per_source: Some(5),
    }).unwrap();
    assert!(report.cards.is_empty());
    assert_eq!(report.counts.unjudged, 0);
    assert!(!report.notes.is_empty());
}

#[test]
fn tribunal_collects_from_notes_and_memories() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    insert_note(&pool, &pid, "We chose Rust because of memory safety.");
    insert_note(&pool, &pid, "Apples and oranges are unrelated.");
    insert_memory(&pool, &pid, "Decision memory: use Rust for the backend.");
    let report = tribunal::run(&pool, &TribunalRequest {
        question: "Why did we choose Rust?".into(),
        project_id: Some(pid),
        repo_root: None,
        limit_per_source: Some(5),
    }).unwrap();
    assert!(report.cards.len() >= 2, "expected >=2 cards, got {}", report.cards.len());
    let kinds: std::collections::HashSet<_> = report.cards.iter()
        .map(|c| c.source_kind.clone()).collect();
    assert!(kinds.contains("memory"));
    for c in &report.cards { assert_eq!(c.stance, "unjudged"); }
}

#[test]
fn tribunal_decision_card_is_stronger() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    insert_timeline(&pool, &pid, "decision", "Adopted Rust for the desktop backend.");
    insert_note(&pool, &pid, "Some unrelated note about apples.");
    let report = tribunal::run(&pool, &TribunalRequest {
        question: "Rust".into(),
        project_id: Some(pid),
        repo_root: None,
        limit_per_source: Some(5),
    }).unwrap();
    let dec: Vec<&tribunal::EvidenceCard> = report.cards.iter()
        .filter(|c| c.source_kind == "decision")
        .collect();
    assert!(!dec.is_empty(), "decision card missing");
    let n: Vec<&tribunal::EvidenceCard> = report.cards.iter()
        .filter(|c| c.source_kind == "note")
        .collect();
    if !n.is_empty() {
        assert!(dec[0].confidence >= n[0].confidence - 0.001);
    }
}

#[test]
fn tribunal_hypothesis_uses_explicit_evidence() {
    use nexus_core::hypotheses::{self, AddEvidence, CreateHypothesis};
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    let h = hypotheses::create(&pool, &CreateHypothesis {
        project_id: pid.clone(),
        task_id: None,
        title: "Hypothesis: Qwen3.5 outperforms Qwen2.5".into(),
        statement: "Qwen3.5 produces more concise answers on the eval set.".into(),
        prediction: Some("Qwen3.5 will be preferred on >60% of prompts.".into()),
        confidence: 0.5,
        evidence_for: vec![],
        evidence_against: vec![],
        memory_id: None,
    }).unwrap();
    hypotheses::add_evidence(&pool, &AddEvidence {
        hypothesis_id: h.id.clone(),
        kind: "experiment".into(),
        summary: "Eval run 1, 12/20 prompts preferred.".into(),
        weight: 0.6,
        source_kind: Some("experiment".into()),
        source_id: None,
        stance: "for".into(),
    }).unwrap();
    hypotheses::add_evidence(&pool, &AddEvidence {
        hypothesis_id: h.id.clone(),
        kind: "experiment".into(),
        summary: "Eval run 2, 8/20 prompts preferred.".into(),
        weight: 0.6,
        source_kind: Some("experiment".into()),
        source_id: None,
        stance: "against".into(),
    }).unwrap();

    let report = tribunal::run(&pool, &TribunalRequest {
        question: "Qwen3.5 conciseness".into(),
        project_id: Some(pid),
        repo_root: None,
        limit_per_source: Some(5),
    }).unwrap();
    let h_card = report.cards.iter().find(|c| c.source_kind == "hypothesis")
        .expect("hypothesis card");
    assert_eq!(h_card.stance, "partial",
        "1 for + 1 against should be partial");
    let _ = json!({ "ok": true });
}

#[test]
fn tribunal_blank_question_is_handled() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    let report = tribunal::run(&pool, &TribunalRequest {
        question: "???".into(),
        project_id: Some(pid),
        repo_root: None,
        limit_per_source: Some(5),
    }).unwrap();
    let _ = report.counts;
}

// ---------------- Phase J ----------------

#[test]
fn vault_export_writes_archive_with_manifest() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    insert_note(&pool, &pid, "Note 1");
    insert_task(&pool, &pid, "Task 1", 3);

    let archive = vault_export::export(&pool, &ExportRequest::default()).unwrap();
    assert_eq!(archive.manifest.format, ARCHIVE_FORMAT_VERSION);
    assert!(archive.manifest.schema_version >= 1);
    assert!(!archive.manifest.app_version.is_empty());
    let counts = &archive.manifest.record_counts;
    assert!(counts.get("projects").copied().unwrap_or(0) >= 1);
    assert!(counts.get("notes").copied().unwrap_or(0) >= 1);
    assert!(counts.get("tasks").copied().unwrap_or(0) >= 1);
    assert_eq!(archive.manifest.records_sha256.len(), 64);
    let json = vault_export::to_pretty_json(&archive).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["manifest"]["format"], archive.manifest.format);
}

#[test]
fn vault_export_respects_kind_filter() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    insert_note(&pool, &pid, "Note 1");
    insert_task(&pool, &pid, "Task 1", 3);

    let archive = vault_export::export(&pool, &ExportRequest {
        project_id: None,
        kinds: Some(vec!["projects".into(), "tasks".into()]),
    }).unwrap();
    let counts = &archive.manifest.record_counts;
    assert!(counts.contains_key("projects"));
    assert!(counts.contains_key("tasks"));
    assert!(!counts.contains_key("notes"));
    assert!(archive.records.get("notes").is_none()
        || archive.records["notes"].as_array().map(|a| a.is_empty()).unwrap_or(true));
}

#[test]
fn vault_export_respects_project_filter() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid_a = seed_project(&pool, "A");
    let pid_b = seed_project(&pool, "B");
    insert_note(&pool, &pid_a, "A note");
    insert_note(&pool, &pid_b, "B note");

    let archive = vault_export::export(&pool, &ExportRequest {
        project_id: Some(pid_a.clone()),
        kinds: None,
    }).unwrap();
    let notes = archive.records["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["project_id"], json!(pid_a));
}

#[test]
fn vault_export_hash_is_stable_for_same_data() {
    let dir = tempdir();
    let pool = pool(&dir);
    let pid = seed_project(&pool, "demo");
    insert_note(&pool, &pid, "Stable content");

    let a = vault_export::export(&pool, &ExportRequest::default()).unwrap();
    let b = vault_export::export(&pool, &ExportRequest::default()).unwrap();
    assert_eq!(a.manifest.records_sha256, b.manifest.records_sha256);
    assert!(a.manifest.exported_at_ms <= b.manifest.exported_at_ms);
}
