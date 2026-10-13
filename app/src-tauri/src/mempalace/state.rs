//! Runtime state for the memory backend. The selected backend + the
//! configuration are kept here so commands can reconfigure the active
//! backend without a restart.

use std::sync::Arc;

use parking_lot::RwLock;

use super::adapter::*;
use super::native::NativeBackend;
use super::sidecar::{MemPalaceBackend, MemPalaceConfig};

pub struct MemoryBackendState {
    pub set: RwLock<BackendSet>,
}

impl MemoryBackendState {
    pub fn new(pool: crate::db::SqlitePool) -> Arc<Self> {
        Arc::new(Self {
            set: RwLock::new(build_default(pool)),
        })
    }

    pub fn current(&self) -> BackendKind {
        self.set.read().active
    }

    pub fn activate_mempalace(&self, cfg: MemPalaceConfig) {
        let mut g = self.set.write();
        g.mempalace = Some(Box::new(MemPalaceBackend::new(cfg)));
        g.active = BackendKind::MemPalace;
    }

    pub fn activate_native(&self) {
        self.set.write().active = BackendKind::Native;
    }

    pub fn disable(&self) {
        self.set.write().active = BackendKind::Disabled;
    }

    pub fn search(&self, query: &str, project_id: Option<&str>, limit: usize) -> Vec<AdapterMemory> {
        self.set.read().search_fallback(query, project_id, limit)
    }
}

/// Helper used by tests to construct a state with the native backend only.
#[allow(dead_code)]
pub fn native_only(pool: crate::db::SqlitePool) -> Arc<MemoryBackendState> {
    let _ = NativeBackend::new(pool.clone());
    MemoryBackendState::new(pool)
}
