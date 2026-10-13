//! Server-side auto-title for conversations — pure helpers.
//!
//! The Tauri-aware glue (event emit) lives in
//! `src/commands/auto_title.rs` so this module can be
//! compiled into the lib crate without pulling in
//! `WebView2Loader.dll`. The lib crate must stay
//! testable on its own; mixing the Tauri runtime into
//! it causes the test binaries to fail to load.
//!
//! The previous implementation lived in the frontend
//! (`Assistant.tsx::maybeAutoTitle`) and was best-effort:
//! it could race with the user switching conversations, get
//! stuck if the websocket dropped, and produce a title
//! derived from a *literal* substring of the first message
//! rather than an actual summary.
//!
//! This module runs entirely in the Tauri command layer
//! after a successful chat completion. It asks the local
//! model to summarise the first user message in 3-8 words,
//! validates the result, and hands the title to the caller
//! for persistence. The whole flow is idempotent:
//! re-running it on a conversation with a real title is a
//! no-op.

use std::sync::Arc;

use crate::conversations;
use crate::db::SqlitePool;
use crate::error::AppResult;
use crate::model_profiles;
use crate::providers::{ChatMessage, ChatRequest, ModelProvider};
use tokio_util::sync::CancellationToken;

/// `true` when the conversation title still looks like a
/// default placeholder. The string match is
/// case-insensitive and tolerates the variations the
/// conversation creation path has produced historically
/// (`"New conversation"`, `"new conversation"`, empty
/// string).
pub fn is_default_title(title: Option<&str>) -> bool {
    match title {
        None => true,
        Some(t) => {
            let s = t.trim();
            s.is_empty() || s.eq_ignore_ascii_case("new conversation")
        }
    }
}

/// Heuristic fallback when the model is unreachable.
/// Produces a deterministic 1-8 word title from the first
/// user message. Never returns an empty string.
pub fn heuristic_title(message: &str, max_len: usize) -> String {
    let mut out = String::new();
    let mut count = 0usize;
    for raw in message.split_whitespace() {
        let cleaned: String = raw.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if cleaned.is_empty() { continue; }
        if !out.is_empty() { out.push(' '); }
        out.push_str(&cleaned);
        count += 1;
        if count >= 8 { break; }
        if out.len() + cleaned.len() + 1 > max_len { break; }
    }
    if out.is_empty() { return "New conversation".into(); }
    out
}

/// Normalise a model-generated title: collapse whitespace,
/// strip leading/trailing punctuation, and clip to a
/// sensible length. Returns `None` if the result is empty
/// or unusable.
pub fn normalise_title(raw: &str) -> Option<String> {
    let cleaned: String = raw.chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() { return None; }
    let stripped = trimmed.trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '«' | '»' | '“' | '”'));
    if stripped.is_empty() { return None; }
    let lowered = stripped.to_ascii_lowercase();
    for prefix in ["title:", "conversation title:", "here is a title:", "title is:", "subject:"] {
        if lowered.starts_with(prefix) {
            return None;
        }
    }
    let collapsed: String = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > 64 {
        // The clip is on char count, not byte count, so
        // multibyte characters cannot slice in the middle
        // of a code point. We use `char_indices` to find
        // the byte offset of the 64th char and slice
        // there.
        let end = collapsed
            .char_indices()
            .nth(64)
            .map(|(i, _)| i)
            .unwrap_or_else(|| collapsed.len());
        return Some(format!("{}…", &collapsed[..end]));
    }
    Some(collapsed)
}

/// Pick a model id for the title-generation request.
/// Priority:
///
/// 1. The first enabled general model profile.
/// 2. The first enabled model profile (any role).
pub fn derive_model_for_title(pool: &SqlitePool) -> Option<String> {
    if let Ok(list) = model_profiles::list(pool) {
        for p in &list {
            if p.role == "general" && p.enabled {
                return Some(p.model_id.clone());
            }
        }
        for p in &list {
            if p.enabled {
                return Some(p.model_id.clone());
            }
        }
    }
    None
}

/// Outcome of the auto-title attempt. The Tauri glue in
/// the binary crate converts this into either a database
/// write + event emit (when `kind` is `Model` or
/// `Heuristic`) or a no-op (when `kind` is
/// `AlreadyTitled` or `EmptyConversation`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoTitleOutcome {
    /// Conversation was already named; nothing to do.
    AlreadyTitled,
    /// No user messages yet; nothing to title.
    EmptyConversation,
    /// Title came from the local model.
    Model(String),
    /// Title came from the local heuristic (model
    /// unavailable or returned junk).
    Heuristic(String),
}

impl AutoTitleOutcome {
    pub fn title(&self) -> Option<&str> {
        match self {
            Self::AlreadyTitled | Self::EmptyConversation => None,
            Self::Model(t) | Self::Heuristic(t) => Some(t.as_str()),
        }
    }
    pub fn from_model(&self) -> bool {
        matches!(self, Self::Model(_))
    }
}

/// Drive the auto-title flow (Tauri-free). The caller is
/// responsible for persistence and event emission; this
/// function only computes the desired title.
pub async fn compute_auto_title(
    pool: &SqlitePool,
    provider: Arc<dyn ModelProvider>,
    conv_id: &str,
) -> AppResult<AutoTitleOutcome> {
    let conv = conversations::get(pool, conv_id)?;
    if !is_default_title(Some(&conv.title)) {
        return Ok(AutoTitleOutcome::AlreadyTitled);
    }
    let history = conversations::list_messages(pool, conv_id, 50)?;
    let first_user_text = match history.iter().find(|m| m.role == "user") {
        Some(m) if !m.content.trim().is_empty() => m.content.clone(),
        _ => return Ok(AutoTitleOutcome::EmptyConversation),
    };
    let title_prompt = format!(
        "Summarise the following user message as a short, 3-8 word conversation title. \
         Output ONLY the title, no quotes, no leading labels, no punctuation at the end.\n\n\
         User message: {first_user_text}\n\nTitle:"
    );
    let model_id = derive_model_for_title(pool).unwrap_or_default();
    let req = ChatRequest {
        model: model_id,
        messages: vec![ChatMessage::text("user", title_prompt)],
        system: Some(
            "You are a helpful assistant that generates concise conversation titles. \
             Always output only the title, nothing else.".into(),
        ),
        temperature: Some(0.3),
        top_p: Some(0.95),
        max_tokens: Some(24),
        stop: None,
        tools: None,
        tool_choice: None,
    };
    let cancel = CancellationToken::new();
    let outcome = match provider.chat_complete(req, cancel).await {
        Ok(summary) => match normalise_title(&summary.content) {
            Some(t) => AutoTitleOutcome::Model(t),
            None => AutoTitleOutcome::Heuristic(heuristic_title(&first_user_text, 48)),
        },
        Err(_) => AutoTitleOutcome::Heuristic(heuristic_title(&first_user_text, 48)),
    };
    Ok(outcome)
}
