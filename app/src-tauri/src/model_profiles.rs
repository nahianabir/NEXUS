//! CRUD over `model_profiles`. Phase 2 stores profiles in SQLite; later phases
//! surface the editor on the Model Manager page.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: String,
    pub role: String,            // 'general' | 'coding' | 'embedding' | 'reranker'
    pub display_name: String,
    pub provider: String,
    pub model_id: String,
    pub gguf_path: Option<String>,
    pub context_len: i64,
    pub gpu_layers: Option<i64>,
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: i64,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Embedding vector dimension. `None` for non-embedding
    /// profiles. Populated automatically by
    /// `models_test_embedding` and stored in the database so
    /// the backfill command can route to the right
    /// `chunk_vectors_<dim>` vec0 table without re-probing
    /// every time. This field was added in migration 0016.
    #[serde(default)]
    pub dimension: Option<u32>,
    /// Stable identity of the embedding model. Derived from
    /// the provider base URL + the model id, or set by the
    /// application. Used to detect when two profiles use
    /// *different* models that happen to share a dimension
    /// (e.g. two 768-dim models that produce semantically
    /// incompatible vectors). `None` for non-embedding
    /// profiles and for legacy rows that predate this field.
    #[serde(default)]
    pub model_identity: Option<String>,
    /// True if the model advertises vision capability and
    /// can accept OpenAI-style `image_url` content parts.
    /// Defaults to `false` so the field is strictly
    /// opt-in; an existing row that pre-dates migration
    /// 0017 will not claim vision capability. Users opt-in
    /// by editing the model profile in Model Manager. The
    /// chat pipeline checks this field before sending
    /// image attachments so a text-only model is never
    /// asked to interpret an image.
    #[serde(default)]
    pub supports_vision: bool,
}

pub fn list(pool: &SqlitePool) -> AppResult<Vec<ModelProfile>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT id, role, display_name, provider, model_id, gguf_path, context_len, gpu_layers,
                temperature, top_p, max_tokens, enabled, created_at, updated_at,
                dimension, model_identity, supports_vision
         FROM model_profiles ORDER BY role, display_name",
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(ModelProfile {
            id: row.get(0)?,
            role: row.get(1)?,
            display_name: row.get(2)?,
            provider: row.get(3)?,
            model_id: row.get(4)?,
            gguf_path: row.get(5)?,
            context_len: row.get(6)?,
            gpu_layers: row.get(7)?,
            temperature: row.get(8)?,
            top_p: row.get(9)?,
            max_tokens: row.get(10)?,
            enabled: row.get::<_, i64>(11)? != 0,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
            dimension: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
            model_identity: row.get(15)?,
            supports_vision: row.get::<_, i64>(16)? != 0,
        });
    }
    Ok(out)
}

pub fn upsert(pool: &SqlitePool, p: &ModelProfile) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "INSERT INTO model_profiles (
            id, role, display_name, provider, model_id, gguf_path, context_len, gpu_layers,
            temperature, top_p, max_tokens, enabled, created_at, updated_at,
            dimension, model_identity, supports_vision
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
         ON CONFLICT(id) DO UPDATE SET
            role=excluded.role, display_name=excluded.display_name, provider=excluded.provider,
            model_id=excluded.model_id, gguf_path=excluded.gguf_path, context_len=excluded.context_len,
            gpu_layers=excluded.gpu_layers, temperature=excluded.temperature, top_p=excluded.top_p,
            max_tokens=excluded.max_tokens, enabled=excluded.enabled, updated_at=excluded.updated_at,
            dimension=excluded.dimension, model_identity=excluded.model_identity,
            supports_vision=excluded.supports_vision",
        params![
            p.id, p.role, p.display_name, p.provider, p.model_id, p.gguf_path,
            p.context_len, p.gpu_layers, p.temperature, p.top_p, p.max_tokens,
            p.enabled as i64, p.created_at, p.updated_at,
            p.dimension.map(|v| v as i64),
            p.model_identity,
            p.supports_vision as i64,
        ],
    )?;
    Ok(())
}

pub fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute("DELETE FROM model_profiles WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn new_id() -> String {
    Ulid::new().to_string()
}

pub fn default_general(model_id: impl Into<String>, provider: impl Into<String>) -> ModelProfile {
    let now = chrono::Utc::now().timestamp_millis();
    ModelProfile {
        id: new_id(),
        role: "general".into(),
        display_name: "General Assistant".into(),
        provider: provider.into(),
        model_id: model_id.into(),
        gguf_path: None,
        context_len: 4096,
        gpu_layers: None,
        temperature: 0.7,
        top_p: 0.95,
        max_tokens: 1024,
        enabled: true,
        created_at: now,
        updated_at: now,
        dimension: None,
        model_identity: None,
        supports_vision: false,
    }
}

pub fn default_coding(model_id: impl Into<String>, provider: impl Into<String>) -> ModelProfile {
    let mut p = default_general(model_id, provider);
    p.role = "coding".into();
    p.display_name = "Coding Specialist".into();
    p.context_len = 8192;
    p.max_tokens = 2048;
    p
}

pub fn default_embedding(model_id: impl Into<String>, provider: impl Into<String>) -> ModelProfile {
    let mut p = default_general(model_id, provider);
    p.role = "embedding".into();
    p.display_name = "Embedding Model".into();
    p.context_len = 2048;
    p.max_tokens = 0;
    p
}
