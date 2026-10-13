//! The memory backend trait and the cross-backend record types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::db::SqlitePool;
use crate::error::AppResult;
#[allow(unused_imports)]
use super::native::NativeBackend;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("unsupported operation: {0}")]
    Unsupported(&'static str),
    #[error("upstream failure: {0}")]
    Upstream(String),
    #[error("not initialised")]
    NotInitialised,
    #[error("io: {0}")]
    Io(String),
}

pub type AdapterResult<T> = Result<T, AdapterError>;

/// The cross-backend memory record. We deliberately keep this small and
/// stable so the native store and the MemPalace store can both surface
/// the same shape to the context builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterMemory {
    pub id: String,
    pub backend: String,         // 'native' | 'mempalace'
    pub project_id: Option<String>,
    pub workspace_id: Option<String>,
    pub category: String,
    pub content: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_excerpt: Option<String>,
    pub event_at: Option<i64>,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterCollection {
    pub id: String,
    pub name: String,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterDiagnostics {
    pub backend: String,
    pub healthy: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind { Native, MemPalace, Disabled }

impl BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::MemPalace => "mempalace",
            Self::Disabled => "disabled",
        }
    }
}

/// A memory backend. Implementations must be cheap to construct and
/// thread-safe (`Send + Sync`).
pub trait MemoryBackend: Send + Sync {
    fn kind(&self) -> BackendKind;

    /// Light-touch health probe. The native backend always returns true;
    /// the MemPalace backend invokes the sidecar's `health` RPC.
    fn health(&self) -> AdapterResult<AdapterDiagnostics>;

    /// Search by keyword. `project_id` is an optional filter; `limit`
    /// bounds the response.
    fn search(&self, query: &str, project_id: Option<&str>, limit: usize) -> AdapterResult<Vec<AdapterMemory>>;

    /// Store a record. Returns the canonical id assigned by the backend.
    fn store(&self, record: &AdapterMemory) -> AdapterResult<String>;

    /// Delete by id. Returns Ok(()) if the record was removed or was
    /// already absent.
    fn delete(&self, id: &str) -> AdapterResult<()>;

    /// List collections. The native backend returns the project list as
    /// a stand-in; MemPalace returns the real collection list.
    fn list_collections(&self) -> AdapterResult<Vec<AdapterCollection>> {
        Err(AdapterError::Unsupported("list_collections"))
    }
}

/// The selected backend + any configuration. Held in app state so the
/// runtime can swap it without re-opening the database.
pub struct BackendSet {
    pub active: BackendKind,
    pub native: Box<dyn MemoryBackend>,
    pub mempalace: Option<Box<dyn MemoryBackend>>,
}

impl BackendSet {
    pub fn current(&self) -> &dyn MemoryBackend {
        match self.active {
            BackendKind::Native => &*self.native,
            BackendKind::MemPalace => match self.mempalace.as_deref() {
                Some(b) => b as &dyn MemoryBackend,
                None => &*self.native,
            },
            BackendKind::Disabled => &*self.native,
        }
    }

    /// Convenience: a search that prefers the active backend and falls
    /// back to native if the active backend is unavailable.
    pub fn search_fallback(&self, query: &str, project_id: Option<&str>, limit: usize) -> Vec<AdapterMemory> {
        match self.current().search(query, project_id, limit) {
            Ok(v) => v,
            Err(_) => self.native.search(query, project_id, limit).unwrap_or_default(),
        }
    }
}

/// Helper: build a BackendSet with the native backend always present.
pub fn build_default(pool: SqlitePool) -> BackendSet {
    BackendSet {
        active: BackendKind::Native,
        native: Box::new(NativeBackend::new(pool)),
        mempalace: None,
    }
}

pub fn with_mempalace(mut set: BackendSet, backend: Box<dyn MemoryBackend>) -> BackendSet {
    set.mempalace = Some(backend);
    set
}

#[allow(dead_code)]
fn _typecheck(_: AppResult<()>) {}
