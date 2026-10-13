//! Portable Personal Knowledge Vault — export. Phase J.
//!
//! Produces a single JSON archive that the user owns. The
//! archive contains:
//!
//! - a manifest (format version, schema version, app
//!   version, export timestamp, record counts, file hashes)
//! - a `records` object keyed by kind
//!   (`projects`, `notes`, `tasks`, `memories_v2`,
//!    `timeline_events`, `checkpoints`, `hypotheses`,
//!    `hypothesis_evidence`, `graph_entities`,
//!    `graph_relationships`)
//!
//! Each record is the canonical column set for the table.
//! No binary blobs are inlined. The user can inspect the
//! archive with any JSON viewer.
//!
//! **No destructive import is exposed in this module.**
//! Importing is a separate decision: it would write to
//! a fresh database or a selected target. This module
//! only emits archives.
//!
//! The user selects which kinds to include. The default
//! is everything except `audit_log` (which is
//! operational, not knowledge).

use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::db::SqlitePool;
use crate::error::AppResult;

pub const ARCHIVE_FORMAT_VERSION: &str = "nexus-vault-1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportRequest {
    /// Restrict export to a single project. `None` means
    /// every project the user owns.
    pub project_id: Option<String>,
    /// Optional list of kinds to include. `None` means
    /// the default (everything except `audit_log`).
    pub kinds: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub format: String,
    pub schema_version: i64,
    pub app_version: String,
    pub exported_at_ms: i64,
    pub project_id: Option<String>,
    pub kinds: Vec<String>,
    pub record_counts: BTreeMap<String, usize>,
    /// sha256 of the canonical JSON of the `records`
    /// object, hex-encoded. Verifying the integrity of
    /// the archive is a one-line operation.
    pub records_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportArchive {
    pub manifest: ExportManifest,
    pub records: serde_json::Value,
}

const DEFAULT_KINDS: &[&str] = &[
    "projects",
    "notes",
    "tasks",
    "memories_v2",
    "timeline_events",
    "checkpoints",
    "hypotheses",
    "hypothesis_evidence",
    "graph_entities",
    "graph_relationships",
];

fn table_exists(conn: &rusqlite::Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        params![name],
        |_| Ok(true),
    ).unwrap_or(false)
}

fn rows_of(
    conn: &rusqlite::Connection,
    table: &str,
    project_id: Option<&str>,
) -> AppResult<Vec<serde_json::Value>> {
    // The simplest correct path: build a SELECT with `*`
    // and let rusqlite produce a `Value` per cell. We
    // avoid a per-table column allowlist because the
    // schema evolves. If a column is added in a future
    // migration it is automatically included.
    if !table_exists(conn, table) { return Ok(Vec::new()); }
    let pid_filter = project_id
        .filter(|_| table_has_project_id(conn, table))
        .map(|_| " WHERE project_id = ?")
        .unwrap_or("");
    let sql = format!("SELECT * FROM {table}{pid_filter} ORDER BY rowid");
    let mut stmt = conn.prepare(&sql)?;
    let cols: Vec<String> = stmt.column_names().into_iter().map(|c| c.to_string()).collect();
    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<serde_json::Value> {
        let mut obj = serde_json::Map::new();
        for (i, name) in cols.iter().enumerate() {
            let v: rusqlite::types::Value = r.get(i)?;
            obj.insert(name.clone(), sqlite_to_json(v));
        }
        Ok(serde_json::Value::Object(obj))
    };
    let mut out: Vec<serde_json::Value> = Vec::new();
    if let Some(pid) = project_id.filter(|_| table_has_project_id(conn, table)) {
        let rows = stmt.query_map(params![pid], map_row)?;
        for r in rows { out.push(r?); }
    } else {
        let rows = stmt.query_map([], map_row)?;
        for r in rows { out.push(r?); }
    }
    Ok(out)
}

fn table_has_project_id(conn: &rusqlite::Connection, table: &str) -> bool {
    // pragma_table_info is the canonical introspection.
    let sql = format!("PRAGMA table_info({table})");
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(1)) {
            for r in rows { if let Ok(name) = r { if name == "project_id" { return true; } } }
        }
    }
    false
}

fn sqlite_to_json(v: rusqlite::types::Value) -> serde_json::Value {
    use rusqlite::types::Value as V;
    match v {
        V::Null => serde_json::Value::Null,
        V::Integer(i) => serde_json::Value::from(i),
        V::Real(f) => serde_json::Value::from(f),
        V::Text(s) => serde_json::Value::String(s),
        V::Blob(b) => serde_json::json!({
            "kind": "blob",
            "size": b.len(),
            "sha256_hex": hex_sha256(&b),
        }),
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out { s.push_str(&format!("{b:02x}")); }
    s
}

fn current_schema_version(pool: &SqlitePool) -> AppResult<i64> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    // schema_version is set by the migration runner; it is
    // also written into the first migration as a meta row.
    // If missing, return 0.
    let v: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    ).unwrap_or(0);
    Ok(v)
}

fn app_version() -> String {
    // The crate version is the cleanest source. This is
    // not a runtime check; it is a build-time constant.
    env!("CARGO_PKG_VERSION").to_string()
}

pub fn export(pool: &SqlitePool, req: &ExportRequest) -> AppResult<ExportArchive> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let kinds: Vec<String> = match &req.kinds {
        Some(list) => list.iter().map(|s| s.to_ascii_lowercase()).collect(),
        None => DEFAULT_KINDS.iter().map(|s| s.to_string()).collect(),
    };
    let mut records = serde_json::Map::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for kind in &kinds {
        let rows = rows_of(&conn, kind, req.project_id.as_deref())?;
        counts.insert(kind.clone(), rows.len());
        records.insert(kind.clone(), serde_json::Value::Array(rows));
    }
    let records_value = serde_json::Value::Object(records);
    let canonical = serde_json::to_vec(&records_value)
        .map_err(|e| crate::error::AppError::Internal(format!("serialise: {e}")))?;
    let records_sha = hex_sha256(&canonical);
    let manifest = ExportManifest {
        format: ARCHIVE_FORMAT_VERSION.into(),
        schema_version: current_schema_version(pool)?,
        app_version: app_version(),
        exported_at_ms: chrono::Utc::now().timestamp_millis(),
        project_id: req.project_id.clone(),
        kinds,
        record_counts: counts,
        records_sha256: records_sha,
    };
    Ok(ExportArchive { manifest, records: records_value })
}

/// Convenience: serialise the archive to pretty JSON.
pub fn to_pretty_json(archive: &ExportArchive) -> AppResult<String> {
    serde_json::to_string_pretty(archive)
        .map_err(|e| crate::error::AppError::Internal(format!("serialise: {e}")))
}
