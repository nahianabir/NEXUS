//! Capability registry. Phase F.
//!
//! Each `ModelProfile` in `model_profiles` is extended with a row in
//! `router_capabilities` describing what tasks it can do. The router
//! reads the capability table at startup and refreshes it on every
//! profile change.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub profile_id: String,
    pub supports_chat: bool,
    pub supports_code: bool,
    pub supports_structured_output: bool,
    pub supports_tool_calling: bool,
    pub supports_streaming: bool,
    pub is_embedding: bool,
    pub approx_ctx_tokens: i64,
    pub approx_memory_mb: i64,
    pub verified: bool,
    pub notes: Option<String>,
    pub updated_at: i64,
}

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn get(pool: &SqlitePool, profile_id: &str) -> AppResult<Capabilities> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT profile_id, supports_chat, supports_code, supports_structured_output, supports_tool_calling, supports_streaming, is_embedding, approx_ctx_tokens, approx_memory_mb, verified, notes, updated_at
         FROM router_capabilities WHERE profile_id = ?1",
        params![profile_id],
        |r| Ok(Capabilities {
            profile_id: r.get(0)?,
            supports_chat: r.get::<_, i64>(1)? != 0,
            supports_code: r.get::<_, i64>(2)? != 0,
            supports_structured_output: r.get::<_, i64>(3)? != 0,
            supports_tool_calling: r.get::<_, i64>(4)? != 0,
            supports_streaming: r.get::<_, i64>(5)? != 0,
            is_embedding: r.get::<_, i64>(6)? != 0,
            approx_ctx_tokens: r.get(7)?,
            approx_memory_mb: r.get(8)?,
            verified: r.get::<_, i64>(9)? != 0,
            notes: r.get(10)?,
            updated_at: r.get(11)?,
        }),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("capabilities for profile {profile_id}")),
        other => AppError::Db(other),
    })
}

pub fn list_all(pool: &SqlitePool) -> AppResult<Vec<Capabilities>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT profile_id, supports_chat, supports_code, supports_structured_output, supports_tool_calling, supports_streaming, is_embedding, approx_ctx_tokens, approx_memory_mb, verified, notes, updated_at
         FROM router_capabilities",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Capabilities {
            profile_id: r.get(0)?,
            supports_chat: r.get::<_, i64>(1)? != 0,
            supports_code: r.get::<_, i64>(2)? != 0,
            supports_structured_output: r.get::<_, i64>(3)? != 0,
            supports_tool_calling: r.get::<_, i64>(4)? != 0,
            supports_streaming: r.get::<_, i64>(5)? != 0,
            is_embedding: r.get::<_, i64>(6)? != 0,
            approx_ctx_tokens: r.get(7)?,
            approx_memory_mb: r.get(8)?,
            verified: r.get::<_, i64>(9)? != 0,
            notes: r.get(10)?,
            updated_at: r.get(11)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}

pub fn upsert(pool: &SqlitePool, cap: &Capabilities) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO router_capabilities (
            profile_id, supports_chat, supports_code, supports_structured_output, supports_tool_calling, supports_streaming, is_embedding, approx_ctx_tokens, approx_memory_mb, verified, notes, updated_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
         ON CONFLICT(profile_id) DO UPDATE SET
            supports_chat=excluded.supports_chat,
            supports_code=excluded.supports_code,
            supports_structured_output=excluded.supports_structured_output,
            supports_tool_calling=excluded.supports_tool_calling,
            supports_streaming=excluded.supports_streaming,
            is_embedding=excluded.is_embedding,
            approx_ctx_tokens=excluded.approx_ctx_tokens,
            approx_memory_mb=excluded.approx_memory_mb,
            verified=excluded.verified,
            notes=excluded.notes,
            updated_at=excluded.updated_at",
        params![
            cap.profile_id, cap.supports_chat as i64, cap.supports_code as i64,
            cap.supports_structured_output as i64, cap.supports_tool_calling as i64,
            cap.supports_streaming as i64, cap.is_embedding as i64,
            cap.approx_ctx_tokens, cap.approx_memory_mb, cap.verified as i64,
            cap.notes, cap.updated_at,
        ],
    )?;
    Ok(())
}

/// Infer a default capability row from the existing `model_profiles`
/// row. The user can override the inferred values via the UI.
pub fn infer_from_profile(pool: &SqlitePool, profile_id: &str) -> AppResult<Capabilities> {
    let profile = crate::model_profiles::list(pool)?
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| AppError::NotFound(format!("profile {profile_id}")))?;
    let now = chrono::Utc::now().timestamp_millis();
    let is_embed = profile.role == "embedding";
    Ok(Capabilities {
        profile_id: profile.id.clone(),
        supports_chat: !is_embed,
        supports_code: profile.role == "coding" || profile.role == "general",
        supports_structured_output: !is_embed,
        supports_tool_calling: !is_embed,
        supports_streaming: !is_embed,
        is_embedding: is_embed,
        approx_ctx_tokens: profile.context_len.max(0),
        approx_memory_mb: estimate_memory_mb(&profile),
        verified: false,
        notes: Some("inferred from profile".into()),
        updated_at: now,
    })
}

fn estimate_memory_mb(p: &crate::model_profiles::ModelProfile) -> i64 {
    // Conservative: 2 bytes per parameter at Q4; assume 0 if no GGUF.
    if let Some(path) = &p.gguf_path {
        if let Ok(meta) = std::fs::metadata(path) {
            // File size is a reasonable proxy for the model footprint.
            return ((meta.len() as f64) / 1_048_576.0).ceil() as i64;
        }
    }
    // Fallback estimate based on the configured context.
    ((p.context_len as f64) * 0.05).ceil() as i64
}
