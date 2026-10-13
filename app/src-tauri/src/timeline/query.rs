//! Read-only timeline queries shared by the Tauri command layer
//! and the test suite.
//!
//! The Tauri command layer lives in the binary crate (so it can
//! use the `#[tauri::command]` proc-macro), but the *data* layer is
//! pure functions over `SqlitePool`. The Tauri commands call into
//! this module; the test suite calls into this module directly.
//! There is no behavioural difference between the two paths.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

use super::TimelineEvent;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Superseded {
    pub memories: Vec<SupersededItem>,
    pub kg_relationships: Vec<SupersededItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupersededItem {
    pub id: String,
    pub label: String,
    pub at_ms: i64,
    pub valid_until: Option<i64>,
    pub superseded_by: Option<String>,
    pub detail: serde_json::Value,
}

/// List memories and KG relationships that have been superseded
/// (`superseded_by` non-null or `valid_until` set), scoped to a
/// project. The KG filter is conservative: a relationship counts
/// only if it touches the project's KG entity.
pub fn list_superseded(pool: &SqlitePool, project_id: &str) -> AppResult<Superseded> {
    let mut out = Superseded::default();
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Memories.
    {
        let mut stmt = conn.prepare(
            "SELECT id, category, content, valid_until, superseded_by, COALESCE(event_at, created_at)
             FROM memories_v2
             WHERE project_id = ?1 AND deleted_at IS NULL
               AND (superseded_by IS NOT NULL OR valid_until IS NOT NULL)
             ORDER BY COALESCE(event_at, created_at) DESC LIMIT 200",
        )?;
        let mut rows = stmt.query(params![project_id])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let category: String = row.get(1)?;
            let content: String = row.get(2)?;
            let valid_until: Option<i64> = row.get(3)?;
            let superseded_by: Option<String> = row.get(4)?;
            let at: i64 = row.get(5)?;
            let excerpt: String = content.chars().take(120).collect();
            out.memories.push(SupersededItem {
                id: id.clone(),
                label: format!("[{}] {}", category, excerpt),
                at_ms: at,
                valid_until,
                superseded_by: superseded_by.clone(),
                detail: serde_json::json!({
                    "category": category, "excerpt": excerpt, "valid_until": valid_until,
                }),
            });
        }
    }
    // KG relationships: valid_until IS NOT NULL, touching the project entity.
    {
        let project_name: Option<String> = conn.query_row(
            "SELECT name FROM projects WHERE id = ?1",
            params![project_id],
            |r| r.get(0),
        ).ok();
        let project_entity_id: Option<String> = match project_name {
            Some(name) => conn.query_row(
                "SELECT id FROM kg_entities
                 WHERE entity_type = 'project' AND canonical_name = ?1 LIMIT 1",
                params![name],
                |r| r.get(0),
            ).ok(),
            None => None,
        };
        let mut stmt = conn.prepare(
            "SELECT id, source_entity_id, target_entity_id, relationship_type,
                    valid_until, COALESCE(valid_from, created_at)
             FROM kg_relationships
             WHERE valid_until IS NOT NULL
             ORDER BY COALESCE(valid_from, created_at) DESC LIMIT 200",
        )?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let src: String = row.get(1)?;
            let tgt: String = row.get(2)?;
            let rel_type: String = row.get(3)?;
            let valid_until: Option<i64> = row.get(4)?;
            let at: i64 = row.get(5)?;
            let touches = project_entity_id.as_deref()
                .map(|pe| src == pe || tgt == pe)
                .unwrap_or(false);
            if !touches { continue; }
            out.kg_relationships.push(SupersededItem {
                id: id.clone(),
                label: format!("{} → {}", src, rel_type),
                at_ms: at,
                valid_until,
                superseded_by: None,
                detail: serde_json::json!({
                    "source": src, "target": tgt, "type": rel_type,
                }),
            });
        }
    }
    Ok(out)
}

/// Synthesise decision-archaeology timeline events from
/// `memories_v2` rows whose category is 'decision'. We do not write
/// a new timeline row here — the decision archaeology view is a
/// *query* over the existing memory table. The returned events
/// carry `ingested_by = "synthesised"` so the UI can label them
/// honestly as derived.
pub fn list_decisions(pool: &SqlitePool, project_id: &str) -> AppResult<Vec<TimelineEvent>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, content, COALESCE(event_at, created_at), verified_at
         FROM memories_v2
         WHERE project_id = ?1 AND deleted_at IS NULL
           AND category = 'decision'
           AND (superseded_by IS NULL)
         ORDER BY COALESCE(event_at, created_at) DESC LIMIT 200",
    )?;
    let mut out = Vec::new();
    let mut rows = stmt.query(params![&project_id])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let content: String = row.get(1)?;
        let event_at: i64 = row.get(2)?;
        let verified_at: Option<i64> = row.get(3)?;
        let excerpt: String = content.chars().take(240).collect();
        let summary = format!("[decision] {}", excerpt);
        out.push(TimelineEvent {
            id: format!("synth:decision:{}", id),
            project_id: Some(project_id.to_string()),
            event_type: if verified_at.is_some() { "approved".into() } else { "created".into() },
            source_kind: "memory".into(),
            source_id: Some(id),
            source_label: Some("decision".into()),
            event_at,
            imported_at: verified_at.unwrap_or(event_at),
            summary,
            detail_json: serde_json::json!({ "category": "decision", "excerpt": excerpt }),
            ingested_by: "synthesised".into(),
            ingest_run_id: None,
            created_at: event_at,
        });
    }
    Ok(out)
}
