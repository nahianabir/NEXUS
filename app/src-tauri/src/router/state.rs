//! Runtime state for the router: the currently loaded model and a
//! hard "one heavyweight at a time" guard.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use super::lifecycle::{self, State};

pub struct RouterState {
    /// id of the profile currently considered loaded. May be None.
    pub active: Mutex<Option<String>>,
}

impl RouterState {
    pub fn new() -> Arc<Self> { Arc::new(Self { active: Mutex::new(None) }) }

    /// Returns the active profile, or None if no profile is loaded.
    pub fn current(&self) -> Option<String> { self.active.lock().clone() }

    /// Atomic-ish switch. If a different heavyweight model is currently
    /// active, this returns the existing active id and a hint to unload
    /// it first; the caller is expected to apply the lifecycle
    /// transitions on the backend.
    pub fn try_acquire(&self, candidate: &str) -> Result<Option<String>, AppError> {
        let mut g = self.active.lock();
        if let Some(current) = g.as_ref() {
            if current == candidate { return Ok(None); } // already ours
            return Ok(Some(current.clone())); // caller must unload first
        }
        *g = Some(candidate.to_string());
        Ok(None)
    }

    pub fn release(&self, profile_id: &str) {
        let mut g = self.active.lock();
        if g.as_deref() == Some(profile_id) { *g = None; }
    }
}

/// Convenience: record a transition and update the active state.
pub fn activate(pool: &SqlitePool, state: &RouterState, profile_id: &str) -> AppResult<()> {
    if let Some(previous) = state.try_acquire(profile_id)? {
        // Mark the previous as unloading.
        let _ = lifecycle::transition(pool, &previous, State::Unloading, None);
        let _ = lifecycle::transition(pool, &previous, State::Unloaded, None);
        state.release(&previous);
        // Try again now that the previous is released.
        let _ = state.try_acquire(profile_id)?;
    }
    let _ = lifecycle::transition(pool, profile_id, State::Loading, None);
    let _ = lifecycle::transition(pool, profile_id, State::Ready, None);
    Ok(())
}
