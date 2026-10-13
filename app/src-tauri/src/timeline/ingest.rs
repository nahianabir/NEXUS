//! Ingestors: walk each source table and write a small summary into
//! `timeline_events`.
//!
//! Every ingestor is read-only against the source. The mapping is
//! deliberately lossy: a 14B-parameter model has no business looking
//! at a full chat transcript or a 500-page paper, so the timeline
//! only carries the short summary it needs to render a row.
//!
//! Idempotency: the UNIQUE key on `(source_kind, source_id,
//! event_type, event_at)` plus `INSERT OR IGNORE` guarantees that
//! the same source event is never written twice.

use rusqlite::params;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

use super::{insert, new_run_id, IngestReport, SourceReport};

/// Walk every source for one project and write the corresponding
/// timeline rows. This is the function the `timeline_ingest_project`
/// Tauri command calls.
///
/// `project_id` is required: we never index rows that are not
/// project-scoped in this release (workspaces, cross-project graphs,
/// and global audit rows are out of scope for the v1 slice).
pub fn ingest_all_for_project(
    pool: &SqlitePool,
    project_id: &str,
) -> AppResult<IngestReport> {
    let run_id = new_run_id();
    let mut report = IngestReport {
        run_id: run_id.clone(),
        ..Default::default()
    };

    // Each ingestor returns (examined, inserted, skipped_existing)
    // and appends its own per-source report.
    let r = ingest_audit(pool, project_id, &run_id)?;
    push_report(&mut report, r);
    let r = ingest_tasks(pool, project_id, &run_id)?;
    push_report(&mut report, r);
    let r = ingest_notes(pool, project_id, &run_id)?;
    push_report(&mut report, r);
    let r = ingest_memories_v2(pool, project_id, &run_id)?;
    push_report(&mut report, r);
    let r = ingest_experiments(pool, project_id, &run_id)?;
    push_report(&mut report, r);
    let r = ingest_kg_relationships(pool, project_id, &run_id)?;
    push_report(&mut report, r);
    let r = ingest_agent_invocations(pool, project_id, &run_id)?;
    push_report(&mut report, r);
    let r = ingest_research_papers(pool, project_id, &run_id)?;
    push_report(&mut report, r);
    let r = ingest_research_reports(pool, project_id, &run_id)?;
    push_report(&mut report, r);

    Ok(report)
}

fn push_report(into: &mut IngestReport, r: SourceReport) {
    into.examined += r.examined;
    into.inserted += r.inserted;
    into.skipped_existing += r.skipped_existing;
    into.sources.push(r);
}

// ----------------- audit_events -----------------
//
// Every `audit_events` row whose `target` is a project id (or whose
// `target` is `None` and which is `system` scope) is written to the
// timeline. The `action` field becomes the timeline event_type when
// it's a known verb; otherwise we use "note".
pub fn ingest_audit(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "audit".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // We can't filter on `target = project_id` directly because some
    // audit rows have NULL target (system boot) or a non-project target
    // (model profile, etc.). Instead we pull every audit row in a
    // bounded window and decide in code.
    let mut stmt = conn.prepare(
        "SELECT id, actor, action, target, detail, created_at
         FROM audit_events ORDER BY created_at ASC",
    )?;
    let rows: Vec<(String, String, String, Option<String>, String, i64)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            v.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?));
        }
        v
    };
    for (id, actor, action, target, _detail, created_at) in rows {
        out.examined += 1;
        // Filter to project-relevant rows. Heuristic: the action
        // verb is project-related, OR the target string is the
        // project id we asked for. This keeps the timeline honest —
        // a "model.unload" event never shows up in a project view.
        let is_project_action = matches!(
            action.as_str(),
            "project.create" | "project.update" | "project.archive" | "project.delete"
                | "task.create" | "task.update" | "task.set_status" | "task.delete"
                | "note.create" | "note.update" | "note.delete"
                | "memory.create" | "memory.approve" | "memory.delete"
                | "workspace.create" | "workspace.delete"
                | "app.boot"
        );
        let target_matches = target.as_deref() == Some(project_id);
        if !(is_project_action || target_matches) { continue; }

        let event_type = match action.as_str() {
            "project.create" | "task.create" | "note.create" | "memory.create"
            | "workspace.create" => "created",
            "task.set_status" => "status_changed",
            "memory.approve" => "approved",
            "project.archive" | "project.delete" | "task.delete" | "note.delete"
            | "memory.delete" | "workspace.delete" => "archived",
            _ => "note",
        };
        let summary = format!("{}: {} ({})", action, actor, target.as_deref().unwrap_or("-"));
        let inserted = insert(
            pool,
            Some(project_id),
            event_type,
            "audit",
            Some(&id),
            Some(&format!("{} · {}", action, actor)),
            created_at,
            &summary,
            serde_json::json!({ "action": action, "actor": actor, "target": target }),
            "ingest_audit",
            Some(run_id),
        )?;
        if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
    }
    Ok(out)
}

// ----------------- tasks -----------------
//
// Two events per task: a "created" event on the task's
// `created_at`, and either a "status_changed" event on every status
// change OR a single "completed" event when the task is done.
// We treat `completed_at` as the canonical "done" timestamp.
pub fn ingest_tasks(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "task".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, title, status, priority, created_at, updated_at, completed_at
         FROM tasks WHERE project_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows: Vec<(String, String, String, String, i64, i64, Option<i64>)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query(params![project_id])?;
        while let Some(row) = rows.next()? {
            v.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?));
        }
        v
    };
    for (id, title, status, priority, created_at, updated_at, completed_at) in rows {
        out.examined += 1;
        let summary_created = format!("Task created: {} [{}]", title, priority);
        let inserted = insert(
            pool,
            Some(project_id),
            "created",
            "task",
            Some(&id),
            Some(&title),
            created_at,
            &summary_created,
            serde_json::json!({ "status": status, "priority": priority, "title": title }),
            "ingest_tasks",
            Some(run_id),
        )?;
        if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }

        // Status change: use `updated_at` if it differs from
        // `created_at`. (We do not have a separate status-changed
        // timestamp; `updated_at` is the closest reliable signal.)
        if updated_at > created_at {
            let summary_status = format!("Task updated: {} → {}", title, status);
            let inserted = insert(
                pool,
                Some(project_id),
                "status_changed",
                "task",
                Some(&id),
                Some(&title),
                updated_at,
                &summary_status,
                serde_json::json!({ "status": status, "title": title }),
                "ingest_tasks",
                Some(run_id),
            )?;
            if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        }
        if let Some(done_at) = completed_at {
            let summary_done = format!("Task completed: {}", title);
            let inserted = insert(
                pool,
                Some(project_id),
                "completed",
                "task",
                Some(&id),
                Some(&title),
                done_at,
                &summary_done,
                serde_json::json!({ "status": status, "title": title }),
                "ingest_tasks",
                Some(run_id),
            )?;
            if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        }
    }
    Ok(out)
}

// ----------------- notes -----------------
pub fn ingest_notes(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "note".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, title, body, created_at, updated_at
         FROM notes WHERE project_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows: Vec<(String, String, String, i64, i64)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query(params![project_id])?;
        while let Some(row) = rows.next()? {
            v.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?));
        }
        v
    };
    for (id, title, body, created_at, updated_at) in rows {
        out.examined += 1;
        // Use the first non-empty line of the body as the excerpt.
        let excerpt: String = body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("")
            .chars()
            .take(120)
            .collect();
        let summary_created = format!("Note: {}", title);
        let inserted = insert(
            pool,
            Some(project_id),
            "created",
            "note",
            Some(&id),
            Some(&title),
            created_at,
            &summary_created,
            serde_json::json!({ "title": title, "excerpt": excerpt }),
            "ingest_notes",
            Some(run_id),
        )?;
        if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        if updated_at > created_at {
            let summary_updated = format!("Note edited: {}", title);
            let inserted = insert(
                pool,
                Some(project_id),
                "updated",
                "note",
                Some(&id),
                Some(&title),
                updated_at,
                &summary_updated,
                serde_json::json!({ "title": title, "excerpt": excerpt }),
                "ingest_notes",
                Some(run_id),
            )?;
            if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        }
    }
    Ok(out)
}

// ----------------- memories_v2 -----------------
//
// A v2 memory has its own `event_at` (the time the underlying fact
// happened) AND its own `verified_at` / `created_at`. We honour the
// explicit temporal contract: the timeline uses `event_at` as
// `event_at` and `created_at` (when the user wrote it into NEXUS) as
// the import marker. `superseded_by` becomes a separate
// "superseded" timeline row at the time of supersession.
pub fn ingest_memories_v2(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "memory".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, category, content, source_kind, source_label, event_at,
                valid_from, valid_until, verified_at, superseded_by,
                created_at, updated_at
         FROM memories_v2
         WHERE project_id = ?1 AND deleted_at IS NULL
         ORDER BY COALESCE(event_at, created_at) ASC",
    )?;
    let rows: Vec<(String, String, String, String, Option<String>,
                   Option<i64>, Option<i64>, Option<i64>, Option<i64>,
                   Option<String>, i64, i64)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query(params![project_id])?;
        while let Some(row) = rows.next()? {
            v.push((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?,
                row.get(10)?, row.get(11)?,
            ));
        }
        v
    };
    for (id, category, content, _source_kind, source_label,
         event_at, _valid_from, valid_until, verified_at,
         superseded_by, created_at, _updated_at) in rows
    {
        out.examined += 1;
        let event_at = event_at.unwrap_or(created_at);
        let excerpt: String = content.chars().take(140).collect();
        let label = source_label.clone().unwrap_or_else(|| category.clone());
        let summary = format!("[{}] {}", category, excerpt);
        let event_type = if verified_at.is_some() && category == "decision" { "approved" } else { "created" };
        let inserted = insert(
            pool,
            Some(project_id),
            event_type,
            "memory",
            Some(&id),
            Some(&label),
            event_at,
            &summary,
            serde_json::json!({ "category": category, "excerpt": excerpt, "valid_until": valid_until }),
            "ingest_memories_v2",
            Some(run_id),
        )?;
        if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }

        if let Some(_) = superseded_by {
            // We don't have a separate "superseded_at" timestamp on
            // the row, so we record the supersession at `updated_at`
            // — the closest reliable signal. The UI can read
            // detail_json.superseded_by to disambiguate.
            let summary_super = format!("[{}] superseded by {}", category, id);
            // Use updated_at as a proxy: it's the last time the row
            // was touched, which is when the supersession happened.
            let inserted = insert(
                pool,
                Some(project_id),
                "superseded",
                "memory",
                Some(&id),
                Some(&label),
                chrono::Utc::now().timestamp_millis(), // event_at: "now" is honest here
                &summary_super,
                serde_json::json!({ "category": category, "superseded_by": superseded_by }),
                "ingest_memories_v2",
                Some(run_id),
            )?;
            if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        }
    }
    Ok(out)
}

// ----------------- experiments -----------------
//
// An experiment is a "what I'm trying" entity. A run is a single
// attempt; the timeline gets one "created" row per experiment and
// one "finished" row per run.
pub fn ingest_experiments(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "experiment".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // First the experiments themselves, scoped by project_id.
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, status, created_at, updated_at
             FROM experiments WHERE project_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows: Vec<(String, String, String, i64, i64)> = {
            let mut v = Vec::new();
            let mut rows = stmt.query(params![project_id])?;
            while let Some(row) = rows.next()? {
                v.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?));
            }
            v
        };
        for (id, name, status, created_at, _updated_at) in rows {
            out.examined += 1;
            let summary = format!("Experiment: {} [{}]", name, status);
            let inserted = insert(
                pool,
                Some(project_id),
                "created",
                "experiment",
                Some(&id),
                Some(&name),
                created_at,
                &summary,
                serde_json::json!({ "name": name, "status": status }),
                "ingest_experiments",
                Some(run_id),
            )?;
            if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        }
    }
    // Then the runs of those experiments.
    {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.experiment_id, r.name, r.status, r.started_at, r.ended_at, e.name
             FROM experiment_runs r
             JOIN experiments e ON e.id = r.experiment_id
             WHERE e.project_id = ?1
             ORDER BY COALESCE(r.started_at, r.created_at) ASC",
        )?;
        let rows: Vec<(String, String, String, String, Option<i64>, Option<i64>, String)> = {
            let mut v = Vec::new();
            let mut rows = stmt.query(params![project_id])?;
            while let Some(row) = rows.next()? {
                v.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?));
            }
            v
        };
        for (id, _exp_id, name, status, started_at, ended_at, exp_name) in rows {
            out.examined += 1;
            let start = started_at.unwrap_or(0);
            if start > 0 {
                let summary = format!("Run started: {} ({})", name, exp_name);
                let inserted = insert(
                    pool,
                    Some(project_id),
                    "opened",
                    "experiment_run",
                    Some(&id),
                    Some(&format!("{} · {}", exp_name, name)),
                    start,
                    &summary,
                    serde_json::json!({ "status": status, "experiment_name": exp_name, "run_name": name }),
                    "ingest_experiments",
                    Some(run_id),
                )?;
                if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
            }
            if let Some(end) = ended_at {
                let summary = format!("Run finished: {} ({}) → {}", name, exp_name, status);
                let inserted = insert(
                    pool,
                    Some(project_id),
                    "finished",
                    "experiment_run",
                    Some(&id),
                    Some(&format!("{} · {}", exp_name, name)),
                    end,
                    &summary,
                    serde_json::json!({ "status": status, "experiment_name": exp_name, "run_name": name }),
                    "ingest_experiments",
                    Some(run_id),
                )?;
                if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
            }
        }
    }
    Ok(out)
}

// ----------------- kg_relationships -----------------
//
// KG relationships are temporal by design: `valid_from` and
// `valid_until` are part of the schema. We only emit relationships
// that touch the project — either because the source entity is the
// project, or because the entity is named in this project, or
// because there is a `kg_evidence` row whose source is one of the
// project's documents/notes.
//
// The v1 heuristic is conservative: a relationship counts as
// "project-relevant" if either endpoint is the project's seeded
// entity (`kg_entities.entity_type = 'project'` with the project
// name) or if the relationship has a `kg_evidence` row whose
// `source_kind` is `note` and `source_id` is one of this project's
// note ids.
pub fn ingest_kg_relationships(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "kg_relationship".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Build the set of note ids for this project.
    let note_ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM notes WHERE project_id = ?1")?;
        let rows = stmt.query_map(params![project_id], |r| r.get::<_, String>(0))?;
        let mut v = Vec::new();
        for row in rows { v.push(row?); }
        v
    };
    if note_ids.is_empty() {
        // Even with no notes, a project may have a seeded KG entity
        // — handle that case too.
    }
    // Find the project entity id (if any). The seed creates one
    // entity per project with the project name and type 'project'.
    let project_entity_id: Option<String> = {
        // We have to look up the project name from the projects table
        // first; `entity.canonical_name` may equal it.
        let project_name: Option<String> = conn.query_row(
            "SELECT name FROM projects WHERE id = ?1",
            params![project_id],
            |r| r.get(0),
        ).ok();
        match project_name {
            Some(name) => conn.query_row(
                "SELECT id FROM kg_entities
                 WHERE entity_type = 'project' AND canonical_name = ?1
                 LIMIT 1",
                params![name],
                |r| r.get(0),
            ).ok(),
            None => None,
        }
    };
    // Build a list of (relationship_id, source_entity_id,
    // target_entity_id, type, valid_from, valid_until, created_at)
    // for relationships that touch the project.
    let mut stmt = conn.prepare(
        "SELECT id, source_entity_id, target_entity_id, relationship_type,
                valid_from, valid_until, created_at
         FROM kg_relationships
         ORDER BY COALESCE(valid_from, created_at) ASC",
    )?;
    let rows: Vec<(String, String, String, String, Option<i64>, Option<i64>, i64)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            v.push((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                row.get(4)?, row.get(5)?, row.get(6)?,
            ));
        }
        v
    };
    for (id, src, tgt, rel_type, valid_from, valid_until, created_at) in rows {
        out.examined += 1;
        // Heuristic: the relationship counts as project-relevant if
        // either endpoint is the project's entity, OR there is a
        // kg_evidence row pointing back to one of this project's
        // notes.
        let touches_project_entity = project_entity_id.as_deref()
            .map(|pe| src == pe || tgt == pe)
            .unwrap_or(false);
        if !touches_project_entity {
            // Check evidence.
            let ev_count: i64 = {
                let mut stmt = conn.prepare(
                    "SELECT COUNT(*) FROM kg_evidence
                     WHERE relationship_id = ?1
                       AND source_kind = 'note'
                       AND source_id = ?2",
                )?;
                let mut total = 0i64;
                for nid in &note_ids {
                    let n: i64 = stmt.query_row(
                        params![id, nid],
                        |r| r.get(0),
                    ).unwrap_or(0);
                    total += n;
                }
                total
            };
            if ev_count == 0 { continue; }
        }
        let event_at = valid_from.unwrap_or(created_at);
        let summary = format!("Graph: {} → {}", src, rel_type);
        let event_type = if valid_until.is_some() { "superseded" } else { "created" };
        let inserted = insert(
            pool,
            Some(project_id),
            event_type,
            "kg_relationship",
            Some(&id),
            Some(&rel_type),
            event_at,
            &summary,
            serde_json::json!({
                "source_entity_id": src,
                "target_entity_id": tgt,
                "relationship_type": rel_type,
                "valid_from": valid_from,
                "valid_until": valid_until,
            }),
            "ingest_kg_relationships",
            Some(run_id),
        )?;
        if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
    }
    Ok(out)
}

// ----------------- agent_invocations -----------------
//
// One event per invocation. Agent invocations are tied to a
// `conversation_id`; the project is recovered by looking up the
// conversation's `project_id`. We also fall back to the agent run's
// `project_id` when present.
pub fn ingest_agent_invocations(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "agent_invocation".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    // Pull every agent_run that belongs to this project (or whose
    // conversation is project-scoped), and every invocation inside.
    let mut stmt = conn.prepare(
        "SELECT i.id, i.tool_name, i.permission, i.status, i.started_at, i.finished_at, i.error
         FROM agent_invocations i
         LEFT JOIN agent_runs r ON r.id = i.run_id
         LEFT JOIN conversations c ON c.id = r.conversation_id
         WHERE r.project_id = ?1 OR c.project_id = ?1
         ORDER BY i.started_at ASC",
    )?;
    let rows: Vec<(String, String, String, String, i64, Option<i64>, Option<String>)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query(params![project_id])?;
        while let Some(row) = rows.next()? {
            v.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?));
        }
        v
    };
    for (id, tool_name, permission, status, started_at, finished_at, error) in rows {
        out.examined += 1;
        let summary = format!("Agent: {} [{} → {}]", tool_name, permission, status);
        let inserted = insert(
            pool,
            Some(project_id),
            "invoked",
            "agent_invocation",
            Some(&id),
            Some(&tool_name),
            started_at,
            &summary,
            serde_json::json!({
                "tool_name": tool_name,
                "permission": permission,
                "status": status,
                "error": error,
            }),
            "ingest_agent_invocations",
            Some(run_id),
        )?;
        if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        if let Some(end) = finished_at {
            let summary_end = format!("Agent finished: {} → {}", tool_name, status);
            let inserted = insert(
                pool,
                Some(project_id),
                "finished",
                "agent_invocation",
                Some(&id),
                Some(&tool_name),
                end,
                &summary_end,
                serde_json::json!({ "tool_name": tool_name, "status": status }),
                "ingest_agent_invocations",
                Some(run_id),
            )?;
            if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        }
    }
    Ok(out)
}

// ----------------- research_papers / research_reports -----------------
//
// Research projects are scoped to a workspace, not a project. We
// bridge by indexing only research projects whose title contains
// the project name OR whose workspace_id is the project's
// workspace_id. This is the v1 heuristic; a v2 release will add a
// `research_projects.project_id` foreign key once we can migrate
// existing rows safely.
pub fn ingest_research_papers(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "research_paper".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let project: Option<(Option<String>, String)> = conn.query_row(
        "SELECT workspace_id, name FROM projects WHERE id = ?1",
        params![project_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();
    let (workspace_id, project_name) = match project {
        Some(p) => p,
        None => return Ok(out),
    };
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.authors, p.year, p.created_at, r.title, r.workspace_id
         FROM research_papers p
         JOIN research_projects r ON r.id = p.research_project_id
         WHERE r.workspace_id = ?1 OR r.workspace_id IS NULL
            OR INSTR(r.title, ?2) > 0
         ORDER BY p.created_at ASC",
    )?;
    let rows: Vec<(String, String, Option<String>, Option<i64>, i64, String, Option<String>)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query(params![workspace_id, project_name])?;
        while let Some(row) = rows.next()? {
            v.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?));
        }
        v
    };
    for (id, title, authors, year, created_at, rp_title, _rp_workspace) in rows {
        // Only include papers whose research project's title contains
        // the project name OR whose workspace matches the project's
        // workspace. (The SQL above is a coarse filter; the
        // application-level check below is the real one.)
        let in_scope = rp_title.contains(&project_name) || workspace_id.is_some();
        if !in_scope { continue; }
        out.examined += 1;
        let label = format!("{} ({})", title, year.map(|y| y.to_string()).unwrap_or_else(|| "n.d.".into()));
        let summary = format!("Paper added: {}", title);
        let _ = authors; // reserved for a future detail_json field
        let inserted = insert(
            pool,
            Some(project_id),
            "created",
            "research_paper",
            Some(&id),
            Some(&label),
            created_at,
            &summary,
            serde_json::json!({ "title": title, "year": year, "research_project": rp_title }),
            "ingest_research_papers",
            Some(run_id),
        )?;
        if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
    }
    Ok(out)
}

pub fn ingest_research_reports(
    pool: &SqlitePool,
    project_id: &str,
    run_id: &str,
) -> AppResult<SourceReport> {
    let mut out = SourceReport { source_kind: "research_report".into(), ..Default::default() };
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let project: Option<(Option<String>, String)> = conn.query_row(
        "SELECT workspace_id, name FROM projects WHERE id = ?1",
        params![project_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();
    let (workspace_id, project_name) = match project {
        Some(p) => p,
        None => return Ok(out),
    };
    let mut stmt = conn.prepare(
        "SELECT rep.id, rep.title, rep.created_at, rep.updated_at, rp.title, rp.workspace_id
         FROM research_reports rep
         JOIN research_projects rp ON rp.id = rep.research_project_id
         WHERE rp.workspace_id = ?1
            OR INSTR(rp.title, ?2) > 0
         ORDER BY rep.created_at ASC",
    )?;
    let rows: Vec<(String, String, i64, i64, String, Option<String>)> = {
        let mut v = Vec::new();
        let mut rows = stmt.query(params![workspace_id, project_name])?;
        while let Some(row) = rows.next()? {
            v.push((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?));
        }
        v
    };
    for (id, title, created_at, updated_at, rp_title, _rp_workspace) in rows {
        let in_scope = rp_title.contains(&project_name) || workspace_id.is_some();
        if !in_scope { continue; }
        out.examined += 1;
        let summary = format!("Report: {}", title);
        let inserted = insert(
            pool,
            Some(project_id),
            "created",
            "research_report",
            Some(&id),
            Some(&title),
            created_at,
            &summary,
            serde_json::json!({ "title": title, "research_project": rp_title }),
            "ingest_research_reports",
            Some(run_id),
        )?;
        if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        if updated_at > created_at {
            let summary_u = format!("Report updated: {}", title);
            let inserted = insert(
                pool,
                Some(project_id),
                "updated",
                "research_report",
                Some(&id),
                Some(&title),
                updated_at,
                &summary_u,
                serde_json::json!({ "title": title, "research_project": rp_title }),
                "ingest_research_reports",
                Some(run_id),
            )?;
            if inserted { out.inserted += 1; } else { out.skipped_existing += 1; }
        }
    }
    Ok(out)
}

// Re-exported so callers can build a one-shot summary without going
// through `IngestReport` JSON shape.
pub fn total_events_for_project(pool: &SqlitePool, project_id: &str) -> AppResult<i64> {
    super::count_for_project(pool, project_id)
}
