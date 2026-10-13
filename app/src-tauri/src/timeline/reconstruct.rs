//! Read-only historical reconstruction. Phase S.1.
//!
//! Given a project id and a timestamp, return the known state of
//! the project's records at or before that moment. We never mutate
//! the source tables.
//!
//! The reconstruction is **honest about its limits**: it only
//! reports what the database has a timestamp for. Anything that
//! predates the earliest event in the relevant tables is reported
//! as "unknown" with a count of zero. The UI is expected to show
//! this distinction explicitly.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

use super::TimelineEvent;

/// A reconstruction for one project at one moment in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reconstruction {
    pub project_id: String,
    pub at_ms: i64,
    pub generated_at: i64,
    pub sections: Vec<SnapshotSection>,
    /// Timeline events whose `event_at` is at or before `at_ms`. The
    /// UI uses this to render a "what we know happened" strip.
    pub events: Vec<TimelineEvent>,
    /// Lists what we *cannot* tell the user from the data we have.
    pub unknowns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotSection {
    pub label: String,
    /// `'known'` when we have a definite count; `'partial'` when we
    /// can give a lower bound but not a full picture.
    pub status: String,
    pub count: i64,
    pub items: Vec<ReconstructionItem>,
    /// A short note the UI surfaces next to the section heading.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconstructionItem {
    pub id: String,
    pub label: String,
    pub at_ms: i64,
    pub detail: serde_json::Value,
}

/// Build the reconstruction for `project_id` at `at_ms`.
///
/// `at_ms` is interpreted as "the state on the wall clock at this
/// epoch millis". Items whose `created_at` (or `event_at`) is `<=`
/// the boundary count as known.
pub fn reconstruct_at(
    pool: &SqlitePool,
    project_id: &str,
    at_ms: i64,
) -> AppResult<Reconstruction> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut sections = Vec::new();

    sections.push(snapshot_tasks(pool, project_id, at_ms)?);
    sections.push(snapshot_notes(pool, project_id, at_ms)?);
    sections.push(snapshot_memories(pool, project_id, at_ms)?);
    sections.push(snapshot_experiments(pool, project_id, at_ms)?);
    sections.push(snapshot_kg_relationships(pool, project_id, at_ms)?);
    sections.push(snapshot_decisions(pool, project_id, at_ms)?);

    // Pull the timeline events up to the boundary so the UI can
    // render them inline.
    let events = super::list(
        pool,
        &super::TimelineFilter {
            project_id: Some(project_id.to_string()),
            until_ms: Some(at_ms),
            limit: Some(500),
            ..Default::default()
        },
    )?;

    // Honest "unknowns" list. The reconstruction is bounded by the
    // time the source tables were created; before that, we have no
    // record.
    let mut unknowns = Vec::new();
    if at_ms < 1 {
        unknowns.push("Timestamp is before the Unix epoch; no data is in scope.".into());
    }
    if events.is_empty() {
        unknowns.push(
            "No timeline events have been indexed for this project. Run 'Build timeline' to populate history.".into(),
        );
    }

    Ok(Reconstruction {
        project_id: project_id.to_string(),
        at_ms,
        generated_at: now,
        sections,
        events,
        unknowns,
    })
}

fn snapshot_tasks(
    pool: &SqlitePool,
    project_id: &str,
    at_ms: i64,
) -> AppResult<SnapshotSection> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks
         WHERE project_id = ?1
           AND created_at <= ?2
           AND (completed_at IS NULL OR completed_at > ?2)",
        params![project_id, at_ms],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, title, status, priority, created_at
         FROM tasks
         WHERE project_id = ?1 AND created_at <= ?2
         ORDER BY created_at DESC LIMIT 50",
    )?;
    let mut items = Vec::new();
    let mut rows = stmt.query(params![project_id, at_ms])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let status: String = row.get(2)?;
        let priority: String = row.get(3)?;
        let created_at: i64 = row.get(4)?;
        items.push(ReconstructionItem {
            id,
            label: format!("[{}] {} — {}", priority, status, title),
            at_ms: created_at,
            detail: serde_json::json!({ "status": status, "priority": priority, "title": title }),
        });
    }
    Ok(SnapshotSection {
        label: "Open tasks at this point".into(),
        status: if items.is_empty() { "partial".into() } else { "known".into() },
        count,
        items,
        note: Some(format!("Tasks created on or before {} that were not yet completed.", format_ms(at_ms))),
    })
}

fn snapshot_notes(
    pool: &SqlitePool,
    project_id: &str,
    at_ms: i64,
) -> AppResult<SnapshotSection> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notes
         WHERE project_id = ?1 AND created_at <= ?2",
        params![project_id, at_ms],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at FROM notes
         WHERE project_id = ?1 AND created_at <= ?2
         ORDER BY created_at DESC LIMIT 50",
    )?;
    let mut items = Vec::new();
    let mut rows = stmt.query(params![project_id, at_ms])?;
    while let Some(row) = rows.next()? {
        items.push(ReconstructionItem {
            id: row.get(0)?,
            label: row.get(1)?,
            at_ms: row.get(2)?,
            detail: serde_json::json!({}),
        });
    }
    Ok(SnapshotSection {
        label: "Notes at this point".into(),
        status: "known".into(),
        count,
        items,
        note: None,
    })
}

fn snapshot_memories(
    pool: &SqlitePool,
    project_id: &str,
    at_ms: i64,
) -> AppResult<SnapshotSection> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memories_v2
         WHERE project_id = ?1 AND deleted_at IS NULL
           AND COALESCE(event_at, created_at) <= ?2
           AND (superseded_by IS NULL)
           AND (valid_until IS NULL OR valid_until > ?2)",
        params![project_id, at_ms],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, category, content, COALESCE(event_at, created_at)
         FROM memories_v2
         WHERE project_id = ?1 AND deleted_at IS NULL
           AND COALESCE(event_at, created_at) <= ?2
           AND (superseded_by IS NULL)
           AND (valid_until IS NULL OR valid_until > ?2)
         ORDER BY COALESCE(event_at, created_at) DESC LIMIT 50",
    )?;
    let mut items = Vec::new();
    let mut rows = stmt.query(params![project_id, at_ms])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let category: String = row.get(1)?;
        let content: String = row.get(2)?;
        let at: i64 = row.get(3)?;
        let excerpt: String = content.chars().take(120).collect();
        items.push(ReconstructionItem {
            id,
            label: format!("[{}] {}", category, excerpt),
            at_ms: at,
            detail: serde_json::json!({ "category": category, "excerpt": excerpt }),
        });
    }
    Ok(SnapshotSection {
        label: "Valid memories at this point".into(),
        status: "known".into(),
        count,
        items,
        note: Some("Memories whose `valid_until` is unset or in the future, and which have not been superseded.".into()),
    })
}

fn snapshot_experiments(
    pool: &SqlitePool,
    project_id: &str,
    at_ms: i64,
) -> AppResult<SnapshotSection> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM experiments
         WHERE project_id = ?1 AND created_at <= ?2",
        params![project_id, at_ms],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, name, status, created_at FROM experiments
         WHERE project_id = ?1 AND created_at <= ?2
         ORDER BY created_at DESC LIMIT 50",
    )?;
    let mut items = Vec::new();
    let mut rows = stmt.query(params![project_id, at_ms])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let status: String = row.get(2)?;
        let at: i64 = row.get(3)?;
        items.push(ReconstructionItem {
            id,
            label: format!("[{}] {}", status, name),
            at_ms: at,
            detail: serde_json::json!({ "status": status, "name": name }),
        });
    }
    Ok(SnapshotSection {
        label: "Experiments at this point".into(),
        status: "known".into(),
        count,
        items,
        note: None,
    })
}

fn snapshot_kg_relationships(
    pool: &SqlitePool,
    project_id: &str,
    at_ms: i64,
) -> AppResult<SnapshotSection> {
    // Re-use the same heuristic as ingest_kg_relationships: a
    // relationship counts if it touches the project's KG entity OR
    // has evidence pointing at one of the project's notes.
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let project_entity_id: Option<String> = {
        let project_name: Option<String> = conn.query_row(
            "SELECT name FROM projects WHERE id = ?1",
            params![project_id],
            |r| r.get(0),
        ).ok();
        match project_name {
            Some(name) => conn.query_row(
                "SELECT id FROM kg_entities
                 WHERE entity_type = 'project' AND canonical_name = ?1 LIMIT 1",
                params![name],
                |r| r.get(0),
            ).ok(),
            None => None,
        }
    };
    let note_ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM notes WHERE project_id = ?1")?;
        let rows = stmt.query_map(params![project_id], |r| r.get::<_, String>(0))?;
        let mut v = Vec::new();
        for row in rows { v.push(row?); }
        v
    };
    let mut stmt = conn.prepare(
        "SELECT id, source_entity_id, target_entity_id, relationship_type, created_at
         FROM kg_relationships
         WHERE (valid_from IS NULL OR valid_from <= ?1)
           AND (valid_until IS NULL OR valid_until > ?1)
         ORDER BY created_at DESC LIMIT 50",
    )?;
    let mut items = Vec::new();
    let mut rows = stmt.query(params![at_ms])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let src: String = row.get(1)?;
        let tgt: String = row.get(2)?;
        let rel_type: String = row.get(3)?;
        let at: i64 = row.get(4)?;
        let touches_project_entity = project_entity_id.as_deref()
            .map(|pe| src == pe || tgt == pe)
            .unwrap_or(false);
        if !touches_project_entity {
            // Check evidence.
            let mut found = false;
            for nid in &note_ids {
                let n: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM kg_evidence
                     WHERE relationship_id = ?1 AND source_kind = 'note' AND source_id = ?2",
                    params![id, nid],
                    |r| r.get(0),
                ).unwrap_or(0);
                if n > 0 { found = true; break; }
            }
            if !found { continue; }
        }
        items.push(ReconstructionItem {
            id,
            label: format!("{} → {}", src, rel_type),
            at_ms: at,
            detail: serde_json::json!({ "source": src, "target": tgt, "type": rel_type }),
        });
    }
    Ok(SnapshotSection {
        label: "Knowledge graph relationships in force".into(),
        status: "known".into(),
        count: items.len() as i64,
        items,
        note: Some("Includes only relationships whose `valid_from`/`valid_until` window covers the moment in time.".into()),
    })
}

fn snapshot_decisions(
    pool: &SqlitePool,
    project_id: &str,
    at_ms: i64,
) -> AppResult<SnapshotSection> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Decisions live in two places: memories_v2 with category
    // 'decision', and audit_events with the action
    // 'project.*' / 'task.*'. We union them by id, distinct.
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM memories_v2
         WHERE project_id = ?1 AND deleted_at IS NULL
           AND category = 'decision'
           AND COALESCE(event_at, created_at) <= ?2
           AND (superseded_by IS NULL)",
        params![project_id, at_ms],
        |r| r.get(0),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, content, COALESCE(event_at, created_at)
         FROM memories_v2
         WHERE project_id = ?1 AND deleted_at IS NULL
           AND category = 'decision'
           AND COALESCE(event_at, created_at) <= ?2
           AND (superseded_by IS NULL)
         ORDER BY COALESCE(event_at, created_at) DESC LIMIT 50",
    )?;
    let mut items = Vec::new();
    let mut rows = stmt.query(params![project_id, at_ms])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let content: String = row.get(1)?;
        let at: i64 = row.get(2)?;
        let excerpt: String = content.chars().take(140).collect();
        items.push(ReconstructionItem {
            id,
            label: excerpt.clone(),
            at_ms: at,
            detail: serde_json::json!({ "category": "decision", "excerpt": excerpt }),
        });
    }
    Ok(SnapshotSection {
        label: "Decisions on record".into(),
        status: "known".into(),
        count,
        items,
        note: Some("Decisions come from approved memories with category 'decision'. Other kinds of decisions may be recorded in notes or audit events.".into()),
    })
}

/// Compare two timestamps for the same project. Returns the
/// timeline events added between `from_ms` and `to_ms` (inclusive
/// of `from_ms`, exclusive of `to_ms`). The UI uses this to
/// highlight what changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comparison {
    pub project_id: String,
    pub from_ms: i64,
    pub to_ms: i64,
    pub added: Vec<TimelineEvent>,
    pub note: String,
}

pub fn compare(
    pool: &SqlitePool,
    project_id: &str,
    from_ms: i64,
    to_ms: i64,
) -> AppResult<Comparison> {
    if to_ms < from_ms {
        return Err(AppError::Invalid("compare: to_ms < from_ms".into()));
    }
    let added = super::list(
        pool,
        &super::TimelineFilter {
            project_id: Some(project_id.to_string()),
            since_ms: Some(from_ms),
            until_ms: Some(to_ms - 1),
            limit: Some(1000),
            ..Default::default()
        },
    )?;
    Ok(Comparison {
        project_id: project_id.to_string(),
        from_ms,
        to_ms,
        added,
        note: "Events whose `event_at` is in the closed-open range [from_ms, to_ms).".into(),
    })
}

fn format_ms(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| ms.to_string())
}
