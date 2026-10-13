//! Project / workspace / note / task CRUD. Phase 5.
//!
//! All four tables were created in migration 0004. This module is a thin
//! hand-written DAL: no `diesel`, no `refinery`; we keep each row typed so
//! the rest of the application can use them without re-parsing JSON.

pub mod context;
pub mod notes;
pub mod projects;
pub mod tasks;
pub mod ui_state;
pub mod workspaces;
