//! Tauri-side glue for the auto-title flow. Lives in
//! the binary crate so the lib stays free of
//! `WebView2Loader.dll` dependencies and the unit tests
//! can keep running without a Tauri runtime.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use nexus_core::audit;
use nexus_core::auto_title::{compute_auto_title, AutoTitleOutcome};
use nexus_core::conversations;
use nexus_core::db::SqlitePool;
use nexus_core::providers::ModelProvider;

/// Event payload emitted on the `nexus:chat.title_updated`
/// channel so the sidebar can refresh the conversation
/// list.
#[derive(Debug, Clone, Serialize)]
pub struct ChatTitleUpdatedEvent {
    pub conversation_id: String,
    pub title: String,
    /// True if the title came from the local model;
    /// false if it was a heuristic fallback.
    pub from_model: bool,
}

/// Compute the auto-title for `conv_id` and, if a real
/// title is produced, persist it and emit
/// `nexus:chat.title_updated`. This is the Tauri-aware
/// counterpart of `nexus_core::auto_title::compute_auto_title`
/// and is the function the chat pipeline should call.
///
/// The whole flow is **idempotent** and silent on
/// failure (logs at `warn` level). It is *never* allowed
/// to surface an error to the chat pipeline — an
/// unreachable model must not block the real response.
pub async fn trigger_auto_title(
    pool: SqlitePool,
    provider: Arc<dyn ModelProvider>,
    conv_id: String,
    app: AppHandle,
) {
    let (from_model, title) = match compute_auto_title(&pool, provider, &conv_id).await {
        Ok(AutoTitleOutcome::Model(t)) => (true, Some(t)),
        Ok(AutoTitleOutcome::Heuristic(t)) => (false, Some(t)),
        Ok(AutoTitleOutcome::AlreadyTitled) | Ok(AutoTitleOutcome::EmptyConversation) => {
            return; // No work; do not emit anything.
        }
        Err(e) => {
            log::warn!("auto-title: compute failed: {e}");
            return;
        }
    };
    let title = match title {
        Some(t) if !t.is_empty() => t,
        _ => return,
    };
    if let Err(e) = conversations::rename(&pool, &conv_id, &title) {
        log::warn!("auto-title: rename failed: {e}");
        return;
    }
    let _ = audit::log(
        &pool,
        "system",
        "conversation.auto_title",
        Some(&conv_id),
        serde_json::json!({"title": title, "from_model": from_model}),
    );
    let _ = app.emit("nexus:chat.title_updated", ChatTitleUpdatedEvent {
        conversation_id: conv_id,
        title,
        from_model,
    });
}
