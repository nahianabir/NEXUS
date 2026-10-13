//! Shared application state, owned by the Tauri runtime.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use tokio_util::sync::CancellationToken;

use crate::audit;
use crate::agent;
use crate::config::AppConfig;
use crate::db::{self, SqlitePool};
use crate::error::{AppError, AppResult};
use crate::mempalace::state::MemoryBackendState;
use crate::model_profiles::{self, ModelProfile};
use crate::monitor::SystemMonitor;
use crate::providers::{self, ProviderKind, ProviderSet};

pub struct AppState {
    pub data_dir: PathBuf,
    pub config: RwLock<AppConfig>,
    pub pool: SqlitePool,
    pub providers: RwLock<ProviderSet>,
    pub active_model: RwLock<Option<String>>,
    /// Per-request cancel registry (Phase P4 — Bug L).
    ///
    /// The previous `Vec<CancellationToken>` design had three
    /// independent failure modes:
    ///   1. A `cancel_inflight` (Stop button) call would cancel
    ///      **every** active token, including unrelated
    ///      concurrent requests (Model Manager test prompt,
    ///      other chat requests).
    ///   2. There was no way to identify a specific request
    ///      from the outside, so the frontend could not
    ///      cancel only the request it wanted to stop.
    ///   3. A request that registered a token and then
    ///      finished never explicitly removed its token,
    ///      so the vector grew unboundedly.
    ///
    /// The new registry keys each token by a stable
    /// `request_id` (ULID) emitted by the chat pipeline, with
    /// a human-readable `kind` so callers (typically
    /// `cancel_inflight`) can target a category ("chat" vs
    /// "test") and a `conversation_id` so we can stop a
    /// specific chat without affecting another one. The
    /// registry is also accessible to the frontend via
    /// `chat_request_list_active` and `chat_request_cancel`,
    /// which lets the chat UI recover the in-flight state
    /// after a navigation/reload.
    pub active_cancellations: parking_lot::Mutex<HashMap<String, ActiveCancel>>,
    pub memory_backends: Arc<MemoryBackendState>,
    /// Long-lived system monitor. Reusing a single instance across
    /// `dashboard_summary` calls keeps the CPU-time delta samples
    /// stable; constructing a fresh `SystemMonitor` per request resets
    /// the prime counter and the dashboard would perpetually show
    /// the un-primed first value.
    pub monitor: parking_lot::Mutex<SystemMonitor>,
}

impl AppState {
    pub fn initialize(data_dir: &Path) -> AppResult<Self> {
        // Boot trace — append a single line per stage so we can localise
        // the crash when the GUI subsystem swallows stderr.
        fn trace(msg: &str) {
            let dir = std::env::var("APPDATA")
                .map(|p| std::path::PathBuf::from(p).join("ai.nexus.workbench"))
                .unwrap_or_else(|_| std::env::temp_dir().join("ai.nexus.workbench"));
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join("boot.log");
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                let _ = writeln!(f, "{} {}", chrono::Utc::now().to_rfc3339(), msg);
            }
        }
        trace("initialize: enter");
        std::fs::create_dir_all(data_dir)?;
        trace("initialize: data_dir ensured");
        crate::logging::ensure_log_dir(data_dir)?;
        trace("initialize: log dir ensured");

        let db_path = db::default_db_path(data_dir);
        let pool = db::open_pool(&db_path)?;

        {
            let conn = pool
                .get()
                .map_err(|e| AppError::Internal(format!("pool: {e}")))?;
            let mut conn = conn;
            db::migrations::run(&mut conn)?;
        }

        let mut config = AppConfig::load(data_dir)?;
        if config.general.data_dir.as_os_str().is_empty() {
            config.general.data_dir = data_dir.to_path_buf();
        }
        config.save(data_dir)?;

        let lmstudio = Arc::new(
            providers::lmstudio::LmStudioProvider::new(config.providers.lmstudio_base_url.clone())
                .map_err(|e| AppError::Internal(format!("http client: {e}")))?,
        );
        let llamacpp = Arc::new(
            providers::llamacpp::LlamaCppProvider::new(config.providers.llamacpp_base_url.clone())
                .map_err(|e| AppError::Internal(format!("http client: {e}")))?,
        );
        let ollama = Arc::new(
            providers::ollama::OllamaProvider::new(config.providers.ollama_base_url.clone())
                .map_err(|e| AppError::Internal(format!("http client: {e}")))?,
        );
        let providers = ProviderSet::new(lmstudio, llamacpp, ollama, ProviderKind::LmStudio);

        ensure_default_profiles(&pool, &providers)?;

        // Seed the controlled agent's tool registry.
        if let Err(e) = agent::ensure_seed_tools(&pool) {
            log::warn!("agent tool seed failed: {e}");
        }

        let active = model_profiles::list(&pool)?
            .into_iter()
            .filter(|p| p.role == "general" && p.enabled)
            .max_by_key(|p| p.updated_at)
            .map(|p| p.model_id);

        audit::log(
            &pool,
            "system",
            "app.boot",
            None,
            serde_json::json!({ "data_dir": data_dir, "db_path": db_path, "active_model": active }),
        )?;

        // Honour the audit_retention_days + audit_prune_on_boot settings.
        if config.agent.audit_prune_on_boot && config.agent.audit_retention_days > 0 {
            match audit::prune_by_retention(&pool, config.agent.audit_retention_days) {
                Ok(n) if n > 0 => {
                    log::info!("pruned {n} audit events older than {} days", config.agent.audit_retention_days);
                }
                Ok(_) => {}
                Err(e) => log::warn!("audit prune failed: {e}"),
            }
        }

        let memory_backends = MemoryBackendState::new(pool.clone());
        Ok(Self {
            data_dir: data_dir.to_path_buf(),
            config: RwLock::new(config),
            pool,
            providers: RwLock::new(providers),
            active_model: RwLock::new(active),
            active_cancellations: parking_lot::Mutex::new(HashMap::new()),
            memory_backends,
            monitor: parking_lot::Mutex::new(SystemMonitor::new()),
        })
    }

    pub fn config(&self) -> Arc<AppConfig> {
        let guard = self.config.read();
        Arc::new(guard.clone())
    }

    /// Backwards-compatible: register an un-tagged token.
    /// Used by the Model Manager test prompt which doesn't
    /// yet have a request id.
    pub fn register_cancel(&self, cancel: CancellationToken) {
        let mut map = self.active_cancellations.lock();
        map.retain(|_, v| !v.cancel.is_cancelled());
        let key = format!("legacy-{}", uuid::Uuid::new_v4());
        map.insert(key, ActiveCancel {
            request_id: String::new(),
            kind: "legacy".into(),
            conversation_id: None,
            message_id: None,
            cancel,
        });
    }

    /// Register a tagged cancel token. Prefer this over
    /// `register_cancel`.
    pub fn register_request(
        &self,
        request_id: &str,
        kind: &str,
        conversation_id: Option<&str>,
        message_id: Option<&str>,
        cancel: CancellationToken,
    ) {
        let mut map = self.active_cancellations.lock();
        map.retain(|_, v| !v.cancel.is_cancelled());
        map.insert(request_id.to_string(), ActiveCancel {
            request_id: request_id.to_string(),
            kind: kind.to_string(),
            conversation_id: conversation_id.map(|s| s.to_string()),
            message_id: message_id.map(|s| s.to_string()),
            cancel,
        });
    }

    /// Unregister a request by id. Idempotent.
    pub fn unregister_request(&self, request_id: &str) {
        let mut map = self.active_cancellations.lock();
        map.remove(request_id);
    }

    /// Cancel every active request. Used by the Stop
    /// button in the chat UI for backwards compatibility.
    pub fn cancel_all(&self) {
        let mut map = self.active_cancellations.lock();
        for v in map.values() { v.cancel.cancel(); }
        map.clear();
    }

    /// Cancel a single request by id. Returns true if the
    /// id was found and cancelled.
    pub fn cancel_request(&self, request_id: &str) -> bool {
        let mut map = self.active_cancellations.lock();
        if let Some(v) = map.remove(request_id) {
            v.cancel.cancel();
            return true;
        }
        false
    }

    /// Cancel every active request of a given kind.
    /// Currently used by the Stop button to cancel only
    /// chat requests, leaving e.g. test prompts alone.
    pub fn cancel_kind(&self, kind: &str) -> usize {
        let mut map = self.active_cancellations.lock();
        let mut n = 0;
        let keys: Vec<String> = map
            .iter()
            .filter_map(|(k, v)| if v.kind == kind { Some(k.clone()) } else { None })
            .collect();
        for k in keys {
            if let Some(v) = map.remove(&k) {
                v.cancel.cancel();
                n += 1;
            }
        }
        n
    }

    /// Cancel every active request belonging to a single
    /// conversation.
    pub fn cancel_conversation(&self, conversation_id: &str) -> usize {
        let mut map = self.active_cancellations.lock();
        let mut n = 0;
        let keys: Vec<String> = map
            .iter()
            .filter_map(|(k, v)| match &v.conversation_id {
                Some(cid) if cid == conversation_id => Some(k.clone()),
                _ => None,
            })
            .collect();
        for k in keys {
            if let Some(v) = map.remove(&k) {
                v.cancel.cancel();
                n += 1;
            }
        }
        n
    }

    /// Snapshot the active requests. The frontend uses
    /// this to rehydrate the in-flight state after a
    /// navigation or page reload.
    pub fn list_active(&self) -> Vec<ActiveRequestSnapshot> {
        let map = self.active_cancellations.lock();
        map.values()
            .map(|v| ActiveRequestSnapshot {
                request_id: v.request_id.clone(),
                kind: v.kind.clone(),
                conversation_id: v.conversation_id.clone(),
                message_id: v.message_id.clone(),
            })
            .collect()
    }
}

/// A single entry in the per-request cancel registry.
#[derive(Debug, Clone)]
pub struct ActiveCancel {
    /// Stable request id (ULID). The chat pipeline emits
    /// this in every `nexus:chat.delta` event so the
    /// frontend can correlate deltas with a specific
    /// in-flight request.
    pub request_id: String,
    /// Human-readable category, e.g. `"chat"`, `"test"`.
    /// Used by `cancel_inflight` to target the right
    /// tokens.
    pub kind: String,
    /// The conversation this request belongs to, if any.
    pub conversation_id: Option<String>,
    /// The assistant message id this request is writing to.
    pub message_id: Option<String>,
    /// The cancellation token. Cancel this to stop the
    /// in-flight request.
    pub cancel: CancellationToken,
}

/// Lightweight, serialisable description of an active
/// request. Returned by `chat_request_list_active` so the
/// frontend can rehydrate its in-flight state.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ActiveRequestSnapshot {
    pub request_id: String,
    pub kind: String,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
}

fn ensure_default_profiles(pool: &SqlitePool, _p: &ProviderSet) -> AppResult<()> {
    let existing = model_profiles::list(pool)?;
    if !existing.is_empty() {
        return Ok(());
    }
    // We deliberately do NOT call into the providers here: the backend may
    // not be running, the user may not have started the LM Studio server, and
    // a synchronous boot must never block on a network call. The first time
    // the user runs the Model Manager page, `providers_probe` will discover
    // any reachable model and the user can create a profile from there.
    Ok(())
}

// Force the compiler to keep the import of `ModelProfile` so the type is
// available to downstream modules without an extra `use`.
#[allow(dead_code)]
fn _typecheck(_: ModelProfile) {}