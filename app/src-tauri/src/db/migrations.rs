//! Migration runner. The list is hard-coded so the boot path is predictable.
//!
//! Each entry is `(version, description, sql)`. The runner applies any
//! migration whose version is greater than the highest version recorded in
//! `schema_migrations`. Every migration runs inside a transaction.

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

const MIGRATION_0001: &str = include_str!("../../migrations/0001_core.sql");
const MIGRATION_0002: &str = include_str!("../../migrations/0002_models.sql");
const MIGRATION_0003: &str = include_str!("../../migrations/0003_conversations.sql");
const MIGRATION_0004: &str = include_str!("../../migrations/0004_vault.sql");
const MIGRATION_0005: &str = include_str!("../../migrations/0005_memory.sql");
const MIGRATION_0006: &str = include_str!("../../migrations/0006_memory_v2.sql");
const MIGRATION_0007: &str = include_str!("../../migrations/0007_knowledge_graph.sql");
const MIGRATION_0008: &str = include_str!("../../migrations/0008_router.sql");
const MIGRATION_0009: &str = include_str!("../../migrations/0009_research.sql");
const MIGRATION_0010: &str = include_str!("../../migrations/0010_experiments.sql");
const MIGRATION_0011: &str = include_str!("../../migrations/0011_agent.sql");
const MIGRATION_0012: &str = include_str!("../../migrations/0012_timeline.sql");
const MIGRATION_0013: &str = include_str!("../../migrations/0013_checkpoints.sql");
const MIGRATION_0014: &str = include_str!("../../migrations/0014_hypotheses.sql");
const MIGRATION_0015: &str = include_str!("../../migrations/0015_radar_feedback.sql");
const MIGRATION_0016: &str = include_str!("../../migrations/0016_embedding_profiles.sql");
const MIGRATION_0017: &str = include_str!("../../migrations/0017_vision_capability.sql");

const ALL: &[(u32, &str, &str)] = &[
    (1, "core tables", MIGRATION_0001),
    (2, "model profiles", MIGRATION_0002),
    (3, "conversations", MIGRATION_0003),
    (4, "knowledge vault", MIGRATION_0004),
    (5, "memory & ui_state", MIGRATION_0005),
    (6, "memory v2 & evidence", MIGRATION_0006),
    (7, "knowledge graph", MIGRATION_0007),
    (8, "model router", MIGRATION_0008),
    (9, "research studio", MIGRATION_0009),
    (10, "experiments", MIGRATION_0010),
    (11, "controlled agent", MIGRATION_0011),
    (12, "timeline (continuity engine)", MIGRATION_0012),
    (13, "checkpoints (work resume)", MIGRATION_0013),
    (14, "hypotheses (decision ledger)", MIGRATION_0014),
    (15, "radar feedback (opportunity radar)", MIGRATION_0015),
    (16, "embedding profile dimension awareness", MIGRATION_0016),
    (17, "model profile supports_vision flag", MIGRATION_0017),
];

pub fn ensure_schema_table(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL,
            description TEXT NOT NULL
         );",
    )?;
    Ok(())
}

pub fn current_version(conn: &Connection) -> AppResult<u32> {
    let v: Option<u32> = conn
        .query_row(
            "SELECT MAX(version) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    Ok(v.unwrap_or(0))
}

pub fn run(conn: &mut Connection) -> AppResult<()> {
    ensure_schema_table(conn)?;
    let already = current_version(conn)?;
    let tx = conn.transaction()?;
    for (version, description, sql) in ALL {
        if *version <= already {
            continue;
        }
        // Pre-rename vec0 shadow helper tables for migration
        // 16, which renames the user-facing `chunk_vectors`
        // vec0 table to `chunk_vectors_384`. The vec0
        // extension creates helper tables (e.g.
        // `chunk_vectors_rowids`,
        // `chunk_vectors_vector_chunks00`) that must move in
        // lockstep. SQLite does not support `ALTER TABLE IF
        // EXISTS`, so we discover and rename them here.
        if *version == 16 {
            rename_vec0_shadow_tables(&tx, "chunk_vectors", "chunk_vectors_384")?;
        }
        tx.execute_batch(sql).map_err(|e| {
            AppError::Migration(format!("migration {version} ({description}) failed: {e}"))
        })?;
        let now = chrono::Utc::now().timestamp_millis();
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at, description) VALUES (?1, ?2, ?3)",
            rusqlite::params![version, now, description],
        )?;
        log::info!("migration applied: v{version} {description}");
    }
    tx.commit()?;
    Ok(())
}

/// Rename every SQLite table whose name starts with
/// `<old>_` (the vec0 shadow table convention) to
/// `<new>_<rest>`. Used to migrate the legacy vec0 table and
/// its helper tables to the per-dimension name. Best-effort:
/// unknown shadow tables (if sqlite-vec changes the helper
/// naming) are simply left alone.
fn rename_vec0_shadow_tables(conn: &Connection, old: &str, new: &str) -> AppResult<()> {
    let prefix = format!("{old}_");
    let mut names: Vec<String> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![format!("{prefix}%")], |r| r.get::<_, String>(0))?;
        for r in rows { names.push(r?); }
    }
    for name in names {
        if name == old {
            // The main table itself is renamed by the SQL
            // file, not by this helper.
            continue;
        }
        if !name.starts_with(&prefix) {
            continue;
        }
        let suffix = &name[prefix.len()..];
        let target = format!("{new}_{suffix}");
        let sql = format!("ALTER TABLE {name} RENAME TO {target}");
        if let Err(e) = conn.execute_batch(&sql) {
            log::warn!("rename vec0 shadow table {name} -> {target} failed: {e}");
        }
    }
    Ok(())
}
