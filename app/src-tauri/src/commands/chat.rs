//! AI Assistant commands. Each `chat_send` call:
//!   1. Loads the conversation and its recent messages.
//!   2. Resolves the active provider + model profile.
//!   3. Optionally retrieves context from the Knowledge Vault (FTS5 + vector
//!      hybrid) and stitches the excerpts into the system prompt; the cited
//!      chunks are persisted to `message_sources` after the model replies.
//!   4. Streams a chat completion, persisting the assistant message incrementally.
//!   5. Emits `nexus:chat.delta`, `nexus:chat.reasoning`, `nexus:chat.error`,
//!      and `nexus:chat.done` events with a `conversation_id` so the UI can route.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::conversations::{self, Message};
use nexus_core::error::{AppError, AppResult};
use nexus_core::model_profiles;
use nexus_core::providers::{ChatMessage, ChatRequest, StreamEvent};

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub conversation_id: String,
    pub text: String,
    /// Optional override; if absent we use the conversation's profile or the
    /// most recent 'general' profile.
    pub model_profile_id: Option<String>,
    /// When true, NEXUS searches the Knowledge Vault for context and
    /// includes the top hits in the system prompt. Citations are persisted
    /// on the assistant message.
    #[serde(default)]
    pub use_vault: bool,
    /// When set, overrides the default top-K for vault retrieval.
    pub vault_limit: Option<usize>,
    /// Optional list of in-memory attachment payloads. Each payload is
    /// the text of a file the user attached in the composer (already
    /// read by `chat_read_attachment`). The text is folded into the
    /// user message before persistence so the conversation history
    /// remains self-contained. We deliberately do not persist the
    /// attachment metadata; a future release can re-link source
    /// documents via the existing Knowledge Vault ingestion path.
    #[serde(default)]
    pub attachments: Vec<AttachedText>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AttachedText {
    pub filename: String,
    /// `text` | `code` | `image` | `pdf` | `docx` | `unknown`.
    pub kind: String,
    pub mime_type: String,
    /// For text/code attachments: the extracted text. For image
    /// attachments: a `data:<mime>;base64,<...>` URI. For PDF /
    /// DOCX: the extracted plain text (the same as the text kind).
    pub text: String,
    /// True if the file was larger than the read cap and we only
    /// included the head of it.
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChatDeltaEvent {
    pub conversation_id: String,
    pub message_id: String,
    pub text: String,
    pub kind: String, // "token" | "reasoning" | "error" | "done" | "cancelled"
    pub finish_reason: Option<String>,
    pub usage: Option<nexus_core::providers::Usage>,
    /// Stable request id (ULID) for the in-flight chat
    /// request. Phase P4 — Bug L. The frontend can use this
    /// to:
    ///   * Match deltas to a specific request even when
    ///     the user switched conversations while a stream
    ///     was running.
    ///   * Detect late events from a stale request and
    ///     ignore them so they cannot overwrite the state
    ///     of a newer active request.
    ///   * Rehydrate the running state after a page
    ///     navigation (the request registry is the source
    ///     of truth; the UI just queries it on mount).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Wall-clock millis of the moment the chat request was
    /// dispatched to the model. Present on every event for a
    /// given request; the frontend subtracts from its own
    /// `performance.now()` to render an elapsed-time clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatched_at_ms: Option<i64>,
    /// Wall-clock millis of the first token received from the
    /// model. Only set on `kind == "token"` once and on `kind ==
    /// "done"`. The frontend uses `(first_token - dispatched_at)`
    /// as the time-to-first-token metric.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_token_at_ms: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChatCitationsEvent {
    pub conversation_id: String,
    pub message_id_user: String,
    pub citations: Vec<nexus_core::ingestion::citation::Citation>,
    pub used_vector: bool,
}

#[tauri::command]
pub fn chat_list_conversations(state: State<'_, Arc<AppState>>, archived: Option<bool>) -> AppResult<Vec<conversations::Conversation>> {
    conversations::list(&state.pool, archived.unwrap_or(false))
}

#[tauri::command]
pub fn chat_create_conversation(
    state: State<'_, Arc<AppState>>,
    title: Option<String>,
    system_prompt: Option<String>,
    model_profile_id: Option<String>,
    project_id: Option<String>,
) -> AppResult<conversations::Conversation> {
    let mut c = conversations::create(&state.pool, title, system_prompt, model_profile_id)?;
    if let Some(pid) = project_id.as_deref() {
        if !pid.is_empty() {
            // Validate the project exists; ignore otherwise so the user can
            // always create a conversation even if the active project was
            // deleted in another tab.
            if nexus_core::projects::projects::get(&state.pool, pid).is_ok() {
                conversations::set_project(&state.pool, &c.id, Some(pid))?;
                c.project_id = Some(pid.to_string());
            }
        }
    }
    audit::log(&state.pool, "user", "conversation.create", Some(&c.id), serde_json::json!({"title": c.title, "project_id": c.project_id}))?;
    Ok(c)
}

#[tauri::command]
pub fn chat_get_conversation(state: State<'_, Arc<AppState>>, id: String) -> AppResult<conversations::Conversation> {
    conversations::get(&state.pool, &id)
}

#[tauri::command]
pub fn chat_rename_conversation(state: State<'_, Arc<AppState>>, id: String, title: String) -> AppResult<()> {
    conversations::rename(&state.pool, &id, &title)?;
    audit::log(&state.pool, "user", "conversation.rename", Some(&id), serde_json::json!({"title": title}))?;
    Ok(())
}

#[tauri::command]
pub fn chat_archive_conversation(state: State<'_, Arc<AppState>>, id: String, archived: bool) -> AppResult<()> {
    conversations::set_archived(&state.pool, &id, archived)?;
    audit::log(&state.pool, "user", if archived { "conversation.archive" } else { "conversation.unarchive" }, Some(&id), serde_json::json!({}))?;
    Ok(())
}

#[tauri::command]
pub fn chat_delete_conversation(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    conversations::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "conversation.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

#[tauri::command]
pub fn chat_list_messages(state: State<'_, Arc<AppState>>, conversation_id: String, limit: Option<i64>) -> AppResult<Vec<conversations::Message>> {
    conversations::list_messages(&state.pool, &conversation_id, limit.unwrap_or(200))
}

/// Return a snapshot of every active chat request.
///
/// Phase P4 — Bug E + L. The Assistant page calls this on
/// mount so it can rehydrate the in-flight state when the
/// user has navigated away and come back (or after a page
/// refresh). For each active request, the UI:
///
///   * Marks the assistant message as `streaming` in its
///     local state.
///   * Re-subscribes to the `nexus:chat.delta` event for
///     that conversation.
///   * Re-starts its elapsed-time clock (the backend
///     already tracks wall-clock; the UI just renders).
///   * Re-attaches a listener filter that ignores any
///     delta whose `request_id` does not match the active
///     request, so a stale late event from an old request
///     cannot overwrite the new state.
#[tauri::command]
pub fn chat_request_list_active(state: State<'_, Arc<AppState>>) -> AppResult<Vec<nexus_core::app_state::ActiveRequestSnapshot>> {
    Ok(state.list_active()
        .into_iter()
        .filter(|s| s.kind == "chat")
        .collect())
}

/// Cancel a single chat request by id. Used by the
/// Assistant page's per-request Stop control (if/when
/// added). Returns `true` if a matching request was
/// found and cancelled.
#[tauri::command]
pub fn chat_request_cancel(state: State<'_, Arc<AppState>>, request_id: String) -> AppResult<bool> {
    Ok(state.cancel_request(&request_id))
}

#[tauri::command]
pub fn chat_set_system_prompt(state: State<'_, Arc<AppState>>, id: String, system_prompt: Option<String>) -> AppResult<()> {
    conversations::set_system_prompt(&state.pool, &id, system_prompt.as_deref())?;
    Ok(())
}

#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    request: SendRequest,
) -> AppResult<String> {
    let s = state.inner().clone();

    // Resolve the model profile. Priority: explicit > conversation > active > first general.
    let profile = resolve_profile(&s, &request.conversation_id, request.model_profile_id.as_deref())?;
    let model_id = profile.model_id.clone();
    let profile_id = profile.id.clone();
    let _context_len = profile.context_len as u32;
    let temperature = profile.temperature as f32;
    let top_p = profile.top_p as f32;
    let max_tokens = profile.max_tokens.max(64) as u32;

    // Vision capability gate (Phase P3 — Bug D fix).
    //
    // The previous implementation unconditionally built a
    // multimodal `image_url` content part for any image
    // attachment and forwarded it to the active model. If
    // the model did not advertise vision support, the
    // LM Studio / OpenAI-compatible endpoint returned a
    // 4xx, the chat stream emitted an `error` event, and
    // the UI displayed a hard error. Worse, the in-flight
    // request was sometimes torn down mid-stream, leaving
    // the chat composer in a "stuck" state.
    //
    // We now refuse the request up front with a clear,
    // recoverable error if the user has attached an image
    // but the active model does not advertise vision
    // support. The check is intentionally strict —
    // `supports_vision = true` must be set in Model
    // Manager — because automatic vision-capability
    // detection across providers is not reliable, and
    // silently degrading the image to a text marker would
    // be dishonest. The error message tells the user how
    // to fix the situation (switch model or enable vision
    // on the current profile).
    let has_image = request.attachments.iter().any(|a| a.kind == "image");
    if has_image && !profile.supports_vision {
        return Err(AppError::Invalid(format!(
            "the active model \"{model_id}\" does not advertise vision support. \
             Switch to a vision-capable model in Model Manager, or enable \"Supports vision\" on this profile, \
             and resend the request without the image."
        )));
    }

    // Load the conversation, then the last N messages.
    let conv = conversations::get(&s.pool, &request.conversation_id)?;
    let history = conversations::list_messages(&s.pool, &request.conversation_id, 200)?;

    // Persist the user message. If the request carries attachment
    // payloads, fold them into the persisted content as fenced
    // blocks so the conversation history is self-contained and
    // re-renderable in the future without re-reading the file.
    let now = chrono::Utc::now().timestamp_millis();
    let user_id = Ulid::new().to_string();
    let persisted_text = if request.attachments.is_empty() {
        request.text.clone()
    } else {
        build_user_text_with_attachments(&request.text, &request.attachments)
    };
    let user_msg = Message {
        id: user_id.clone(),
        conversation_id: conv.id.clone(),
        role: "user".into(),
        content: persisted_text,
        status: "complete".into(),
        token_count: None,
        latency_ms: None,
        model_profile_id: Some(profile_id.clone()),
        created_at: now,
    };
    conversations::add_message(&s.pool, &user_msg)?;
    conversations::touch(&s.pool, &conv.id)?;

    // Phase P4 — Bug F: intercept explicit memory-
    // save requests before the model is invoked. The
    // deterministic local service (see
    // `memory_v2::explicit_save`) handles "Save to your
    // memory that X" without ever calling the model. If
    // the request is not an explicit save we fall
    // through to the normal chat pipeline.
    if let Some(save_result) = nexus_core::memory_v2::explicit_save::try_handle(
        &s.pool,
        &request.text,
        Some(&conv.id),
        conv.project_id.as_deref(),
    )? {
        return handle_explicit_save(
            &s,
            &app,
            &conv,
            save_result,
        ).await;
    }

    // Optional Knowledge Vault retrieval. The retrieval is best-effort:
    // failures (no chunks indexed, provider not reachable for embeddings,
    // etc.) are reported to the UI as a `vault.citation` event with an
    // empty hit list, never as an error that aborts the chat.
    let mut citations: Vec<nexus_core::ingestion::citation::Citation> = Vec::new();
    if request.use_vault {
        let limit = request.vault_limit.unwrap_or(5).clamp(1, 20);
        let res = nexus_core::ingestion::search::search_hybrid(
            s.pool.clone(), &request.text, None, limit,
        ).await.unwrap_or_else(|e| {
            log::warn!("vault retrieval: {e}");
            nexus_core::ingestion::search::SearchResult {
                hits: Vec::new(),
                used_vector: false,
                used_docname_hint: false,
                docname_hint_doc_ids: Vec::new(),
            }
        });
        // Track the actual retrieval outcome so the user
        // can see why the model has (or does not have)
        // vault context. We persist this on the assistant
        // message for the frontend to surface in the
        // activity timeline and in the citation panel.
        if res.used_docname_hint {
            log::info!(
                "vault retrieval: docname hint matched docs {:?} for query {:?}",
                res.docname_hint_doc_ids, request.text
            );
        }
        citations = super::vault::build_citations_from_hits(&res.hits);
        let _ = app.emit("nexus:chat.citations", ChatCitationsEvent {
            conversation_id: conv.id.clone(),
            message_id_user: user_id.clone(),
            citations: citations.clone(),
            used_vector: res.used_vector,
        });
    }

    // Build the messages list. We pass the system prompt if the conversation
    // has one; otherwise we send a NEXUS default.
    let mut req_messages: Vec<ChatMessage> = Vec::new();
    for m in &history {
        if m.role == "system" { continue; } // we pass the system prompt separately
        // History messages are always text (the assistant never
        // emitted multimodal content from a previous turn). Wrap
        // the string in `serde_json::Value::String` so the
        // provider can serialise it as a plain `"content": "..."`
        // field.
        req_messages.push(ChatMessage {
            role: m.role.clone(),
            content: serde_json::Value::String(m.content.clone()),
        });
    }
    // The current user message. If it carries image
    // attachments, build an OpenAI-style multimodal content
    // array (text + image_url) so vision-capable models can
    // see the image. Otherwise fall back to plain text.
    let user_content = build_user_message_content(
        &build_user_text_with_attachments(&request.text, &request.attachments),
        &request.attachments,
    );
    req_messages.push(ChatMessage { role: "user".into(), content: user_content });

    let mut system_prompt = conv.system_prompt.clone().or_else(|| Some(default_system_prompt()));

    // Project context: instructions, goals, and the most recent notes for
    // the project this conversation belongs to. The model is told to honour
    // the project instructions as if they were explicit user preferences.
    if let Some(project_id) = conv.project_id.as_deref() {
        if let Ok(block) = super::projects::build_project_context_block(&s.pool, project_id) {
            if !block.is_empty() {
                system_prompt = Some(match system_prompt {
                    Some(s) => format!("{}{}", s, block),
                    None => block,
                });
            }
        }
    }

    // Unified context (Phase E). Pulls approved memories, vault, graph,
    // MemPalace (best-effort) into a single budgeted block. Failures
    // here are non-fatal: the assistant still answers without context.
    match nexus_core::context::build(&s.pool, &s.memory_backends, &nexus_core::context::BuildRequest {
        workspace_id: None,
        project_id: conv.project_id.clone(),
        conversation_id: Some(conv.id.clone()),
        user_message: build_user_text_with_attachments(&request.text, &request.attachments),
        max_tokens: 1500,
        pinned: vec![],
    }) {
        Ok(bundle) => {
            if !bundle.system_block.is_empty() {
                system_prompt = Some(match system_prompt {
                    Some(s) => format!("{}\n\n{}", s, bundle.system_block),
                    None => bundle.system_block,
                });
            }
        }
        Err(_) => {
            // Fallback to the v1 memory block when the unified builder fails.
            if let Ok(mem_block) = super::projects::build_memory_context_block(&s.pool, conv.project_id.as_deref(), Some(&conv.id)) {
                if !mem_block.is_empty() {
                    system_prompt = Some(match system_prompt {
                        Some(s) => format!("{}{}", s, mem_block),
                        None => mem_block,
                    });
                }
            }
        }
    }

    if !citations.is_empty() {
        system_prompt = Some(match system_prompt {
            Some(s) => format!("{}{}", s, super::vault::build_context_block(&citations)),
            None => super::vault::build_context_block(&citations),
        });
    }

    // Allocate the assistant message row up front so the UI can show it as
    // "streaming" while the tokens arrive.
    let assistant_id = Ulid::new().to_string();
    let assistant_msg = Message {
        id: assistant_id.clone(),
        conversation_id: conv.id.clone(),
        role: "assistant".into(),
        content: String::new(),
        status: "streaming".into(),
        token_count: None,
        latency_ms: None,
        model_profile_id: Some(profile_id.clone()),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    conversations::add_message(&s.pool, &assistant_msg)?;

    let cancel = CancellationToken::new();
    let conv_id = conv.id.clone();
    // Phase P4 — Bug L: every chat request is now
    // tagged with a stable request id, a category ("chat"),
    // the conversation id, and the assistant message id
    // it is writing to. The frontend can recover the
    // in-flight state via `chat_request_list_active` after
    // a page navigation, and `cancel_inflight` (Stop button)
    // only cancels chat requests, not Model Manager test
    // prompts.
    let request_id = ulid::Ulid::new().to_string();
    s.register_request(
        &request_id,
        "chat",
        Some(&conv_id),
        Some(&assistant_msg.id),
        cancel.clone(),
    );

    let provider = s.providers.read().active();
    let req = ChatRequest {
        model: model_id.clone(),
        messages: req_messages,
        system: system_prompt,
        temperature: Some(temperature),
        top_p: Some(top_p),
        max_tokens: Some(max_tokens),
        stop: None,
        tools: None,
        tool_choice: None,
    };

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);
    let provider_for_stream = provider.clone();
    let cancel_for_stream = cancel.clone();
    let app_for_stream = app.clone();
    let assistant_id_for_task = assistant_id.clone();
    let pool_for_task = s.pool.clone();
    let request_id_for_task = request_id.clone();
    let s_for_unregister = s.clone();

    let task = tokio::spawn(async move {
        provider_for_stream.stream_chat(req, tx, cancel_for_stream).await
    });

    let mut content = String::new();
    let mut last_emit = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(180);
    let started = std::time::Instant::now();
    // Latency telemetry: capture the moment the request was
    // dispatched, and the moment the first token arrives. The
    // frontend uses these to render a time-to-first-token
    // indicator without inventing numbers.
    let dispatched_at_ms: i64 = chrono::Utc::now().timestamp_millis();
    let mut first_token_at_ms: Option<i64> = None;
    // Compute the absolute deadline *once* and sleep against it
    // on every loop iteration. Previously the code did
    // `tokio::time::sleep(timeout)` inside the `select!`, which
    // constructs a new 180 s timer on every loop iteration and
    // effectively never fires (the timer is reset before it can
    // elapse). The real deadline is computed here and compared
    // against `Instant::now()` each iteration.
    let deadline = started + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            // 180 s budget exhausted. Cancel the in-flight
            // provider task and break out so the post-loop
            // cleanup emits the terminal `error` event.
            cancel.cancel();
            break;
        }
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = conversations::update_message(&pool_for_task, &assistant_id_for_task, &content, "cancelled");
                let _ = app_for_stream.emit("nexus:chat.delta", ChatDeltaEvent {
                    conversation_id: conv_id.clone(),
                    message_id: assistant_id_for_task.clone(),
                    text: content.clone(),
                    kind: "cancelled".into(),
                    finish_reason: Some("cancelled".into()),
                    usage: None,
                    dispatched_at_ms: Some(dispatched_at_ms),
                    first_token_at_ms,
                    request_id: Some(request_id_for_task.clone()),
                });
                s_for_unregister.unregister_request(&request_id_for_task);
                return Err(AppError::Cancelled);
            }
            _ = tokio::time::sleep(remaining) => {
                // Wall-clock budget exhausted.
                cancel.cancel();
                break;
            }
            maybe = rx.recv() => {
                let Some(ev) = maybe else { break };
                match ev.kind.as_str() {
                    "token" => {
                        if let Some(t) = ev.text { content.push_str(&t); }
                        if first_token_at_ms.is_none() {
                            first_token_at_ms = Some(chrono::Utc::now().timestamp_millis());
                        }
                    }
                    "reasoning" => {
                        if let Some(t) = ev.text {
                            let _ = app_for_stream.emit("nexus:chat.reasoning", ChatDeltaEvent {
                                conversation_id: conv_id.clone(),
                                message_id: assistant_id_for_task.clone(),
                                text: t,
                                kind: "reasoning".into(),
                                finish_reason: None,
                                usage: None,
                                dispatched_at_ms: Some(dispatched_at_ms),
                                first_token_at_ms,
                                request_id: Some(request_id_for_task.clone()),
                            });
                        }
                    }
                    "error" => {
                        let msg = ev.error.unwrap_or_else(|| "unknown error".into());
                        let _ = conversations::update_message(&pool_for_task, &assistant_id_for_task, &content, "error");
                        let _ = app_for_stream.emit("nexus:chat.delta", ChatDeltaEvent {
                            conversation_id: conv_id.clone(),
                            message_id: assistant_id_for_task.clone(),
                            text: msg.clone(),
                            kind: "error".into(),
                            finish_reason: None,
                            usage: None,
                            dispatched_at_ms: Some(dispatched_at_ms),
                            first_token_at_ms,
                            request_id: Some(request_id_for_task.clone()),
                        });
                        s_for_unregister.unregister_request(&request_id_for_task);
                        return Err(AppError::Upstream(msg));
                    }
                    "done" => { break; }
                    _ => {}
                }
                if last_emit.elapsed() > std::time::Duration::from_millis(250) {
                    // Phase P4 — Bug E: persist the partial
                    // content to the DB and emit a delta so
                    // that any late rehydration of the chat
                    // page (after the user has navigated away
                    // and back) sees the most recent streamed
                    // text. Throttled to 250 ms (was 50 ms) so
                    // a chatty local model does not hammer the
                    // SQLite write path; the assistant
                    // message is *always* persisted in full on
                    // the terminal path (done / error /
                    // cancelled / wall-clock timeout).
                    let _ = conversations::update_message(&pool_for_task, &assistant_id_for_task, &content, "streaming");
                    let _ = app_for_stream.emit("nexus:chat.delta", ChatDeltaEvent {
                        conversation_id: conv_id.clone(),
                        message_id: assistant_id_for_task.clone(),
                        text: content.clone(),
                        kind: "token".into(),
                        finish_reason: None,
                        usage: None,
                        dispatched_at_ms: Some(dispatched_at_ms),
                        first_token_at_ms,
                        request_id: Some(request_id_for_task.clone()),
                    });
                    last_emit = std::time::Instant::now();
                }
            }
        }
    }

    let summary = task.await.map_err(|e| AppError::Internal(format!("join: {e}")))?;
    let elapsed = started.elapsed().as_millis() as i64;
    match summary {
        Ok(s) => {
            let final_text = if s.content.is_empty() { content } else { s.content };
            let usage_tokens = s.usage.total_tokens.map(|v| v as i64);
            persist_final(&pool_for_task, &assistant_id_for_task, &final_text, "complete", Some(elapsed), usage_tokens)?;
            if !citations.is_empty() {
                let _ = super::vault::save_citations(&pool_for_task, &assistant_id_for_task, &citations);
            }
            let _ = app_for_stream.emit("nexus:chat.delta", ChatDeltaEvent {
                conversation_id: conv_id.clone(),
                message_id: assistant_id_for_task.clone(),
                text: final_text,
                kind: "done".into(),
                finish_reason: s.finish_reason.clone(),
                usage: Some(s.usage.clone()),
                dispatched_at_ms: Some(dispatched_at_ms),
                first_token_at_ms,
                request_id: Some(request_id_for_task.clone()),
            });
            s_for_unregister.unregister_request(&request_id_for_task);
            // Auto-title: fire-and-forget background task
            // that renames the conversation if it still
            // has the default title. Idempotent: a
            // subsequent call is a no-op once a meaningful
            // title is in place. The task uses its own
            // pool + provider clones; nothing here can
            // fail the chat response. See
            // `super::auto_title::trigger_auto_title` for
            // the full flow.
            let title_pool = pool_for_task.clone();
            let title_provider = provider.clone();
            let title_conv = conv_id.clone();
            let title_app = app_for_stream.clone();
            tokio::spawn(async move {
                super::auto_title::trigger_auto_title(title_pool, title_provider, title_conv, title_app).await;
            });
            Ok(assistant_id_for_task)
        }
        Err(e) => {
            // Preserve any partial content the user already
            // received. The `error` event is the terminal
            // signal — we surface the failure cause to the UI
            // but do not overwrite the visible content. The
            // frontend treats an `error` event arriving after
            // streaming text as "the response was interrupted".
            let status = if content.is_empty() { "error" } else { "interrupted" };
            let _ = conversations::update_message(&pool_for_task, &assistant_id_for_task, &content, status);
            let _ = app_for_stream.emit("nexus:chat.delta", ChatDeltaEvent {
                conversation_id: conv_id.clone(),
                message_id: assistant_id_for_task.clone(),
                // Send the *partial* content (not the error
                // string) so the UI keeps the response that
                // was already streamed to the bubble. The error
                // reason is appended as a footer below.
                text: content.clone(),
                kind: "error".into(),
                finish_reason: None,
                usage: None,
                dispatched_at_ms: Some(dispatched_at_ms),
                first_token_at_ms,
                request_id: Some(request_id_for_task.clone()),
            });
            s_for_unregister.unregister_request(&request_id_for_task);
            Err(e)
        }
    }
}

fn persist_final(pool: &nexus_core::db::SqlitePool, id: &str, content: &str, status: &str, latency: Option<i64>, tokens: Option<i64>) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE messages SET content = ?1, status = ?2, latency_ms = ?3, token_count = ?4 WHERE id = ?5",
        rusqlite::params![content, status, latency, tokens, id],
    )?;
    Ok(())
}

fn resolve_profile(state: &AppState, conversation_id: &str, override_id: Option<&str>) -> AppResult<nexus_core::model_profiles::ModelProfile> {
    if let Some(id) = override_id {
        if let Some(p) = model_profiles::list(&state.pool)?.into_iter().find(|p| p.id == id) {
            return Ok(p);
        }
    }
    if let Ok(conv) = conversations::get(&state.pool, conversation_id) {
        if let Some(pid) = conv.model_profile_id {
            if let Some(p) = model_profiles::list(&state.pool)?.into_iter().find(|p| p.id == pid) {
                return Ok(p);
            }
        }
    }
    if let Some(active) = state.active_model.read().clone() {
        if let Some(p) = model_profiles::list(&state.pool)?.into_iter().find(|p| p.model_id == active && p.role == "general") {
            return Ok(p);
        }
    }
    let list = model_profiles::list(&state.pool)?;
    list.into_iter()
        .find(|p| p.role == "general" && p.enabled)
        .ok_or_else(|| AppError::NotFound("no model profile available; create one in Model Manager".into()))
}

fn default_system_prompt() -> String {
    "You are NEXUS, a local AI assistant running entirely on the user's PC. \
     You may use retrieval-augmented context when it is provided. You must not \
     pretend to perform actions, fabricate citations, or invent file paths. \
     If you are uncertain, say so plainly. Keep answers concise unless asked \
     to be thorough."
        .to_string()
}

// Touch the unused `ChatRequest` import for downstream helpers.
#[allow(dead_code)]
fn _typecheck(_: ChatRequest) {}

/// Build the user-message content for the model request. If the
/// request carries image attachments, emit an OpenAI-style
/// multimodal content array (`[{type:text,text:...},
/// {type:image_url,image_url:{url:data:...}}]`). Otherwise
/// return the text wrapped as a plain string so the provider can
/// serialise it as `"content": "..."`.
///
/// We deliberately do NOT inline the text-extraction of every
/// attachment here; the text-block helper (`build_user_text_with_attachments`)
/// already produced the relevant visible text. The image
/// attachments are sent as *additional* content parts, so the
/// model receives both the text summary and the raw image bytes.
fn build_user_message_content(user_text: &str, attachments: &[AttachedText]) -> serde_json::Value {
    let image_atts: Vec<&AttachedText> = attachments.iter().filter(|a| a.kind == "image").collect();
    if image_atts.is_empty() {
        return serde_json::Value::String(user_text.to_string());
    }
    let mut parts: Vec<serde_json::Value> = Vec::with_capacity(1 + image_atts.len());
    parts.push(serde_json::json!({ "type": "text", "text": user_text }));
    for a in &image_atts {
        // Validate the data URI before forwarding it.
        // A malformed URI (e.g. produced by a corrupted
        // upload or a non-image file tagged as image by
        // a buggy extension match) would crash the JSON
        // serialiser downstream; we replace it with an
        // explicit error marker so the request still
        // reaches the model and the user can see
        // something useful in the response.
        let url = sanitise_image_data_uri(&a.text, &a.mime_type);
        parts.push(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": url }
        }));
    }
    serde_json::Value::Array(parts)
}

/// Validate an image data URI before sending it to the
/// model. Returns the original URI when it looks well
/// formed; otherwise returns a `data:text/plain,...`
/// URI containing a short error message so the request
/// can still go out (the model will see the error
/// marker and respond sensibly).
fn sanitise_image_data_uri(uri: &str, mime: &str) -> String {
    use base64::Engine as _;
    let trimmed = uri.trim();
    if trimmed.len() > 64 {
        // Cheap pre-check: a valid data URI is at
        // least "data:<mime>;base64,<one byte>".
        if let Some(rest) = trimmed.strip_prefix("data:") {
            if let Some((header, payload)) = rest.split_once(',') {
                if header.contains(";base64") && !payload.is_empty() {
                    // Looks like a real base64 data URI.
                    return trimmed.to_string();
                }
            }
        }
        log::warn!("image attachment has malformed data URI (mime {mime}); sending error marker instead");
        let err = "image attachment could not be encoded as a data URI; please re-attach the image";
        return format!("data:text/plain;base64,{}", base64::engine::general_purpose::STANDARD.encode(err.as_bytes()));
    }
    log::warn!("image attachment too short to be a valid data URI (mime {mime})");
    let err = "image attachment is empty or too short";
    format!("data:text/plain;base64,{}", base64::engine::general_purpose::STANDARD.encode(err.as_bytes()))
}

/// Combine the user-visible prompt with any attached file payloads
/// into a single string. Code files are wrapped in a fenced
/// ```` ```lang ```` block so the model can see the language; text
/// files are wrapped in a generic ```text block. We deliberately
/// cap each attachment at 64 KB of *display* text (post-prefix) so
/// a single huge file cannot blow out the prompt budget. Attachments
/// live in this string only; the original files stay on disk and
/// are not auto-ingested into the Knowledge Vault.
fn build_user_text_with_attachments(user_text: &str, attachments: &[AttachedText]) -> String {
    if attachments.is_empty() {
        return user_text.to_string();
    }
    let mut out = String::new();
    out.push_str(user_text);
    out.push_str("\n\n---\nAttached files (read-only context, not persisted as documents):\n");
    for a in attachments {
        // Image attachments are routed to the multimodal
        // content path in `build_user_message_content`. We
        // deliberately do not inline the base64 data URI
        // here — the encoded bytes are too large and would
        // just bloat the prompt. A short marker tells the
        // model that the image is attached as a separate
        // payload.
        if a.kind == "image" {
            out.push_str(&format!(
                "\n### {} (image attached via vision path; mime: {})\n",
                a.filename, a.mime_type
            ));
            continue;
        }
        let lang = match a.kind.as_str() {
            "code" => guess_lang(&a.filename, &a.mime_type),
            _ => "text",
        };
        out.push_str(&format!("\n### {}{}\n", a.filename, if a.truncated { " (truncated)" } else { "" }));
        // Cap displayed text per attachment.
        let cap = 64 * 1024;
        let text: &str = if a.text.len() > cap { &a.text[..cap] } else { &a.text };
        out.push_str(&format!("```{}\n{}\n```\n", lang, text));
    }
    out
}

fn guess_lang(filename: &str, mime: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    let ext = std::path::Path::new(&lower)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match ext {
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "rb" => "ruby",
        "c" | "h" => "c",
        "cpp" | "hpp" | "cc" | "hh" => "cpp",
        "cs" => "csharp",
        "sh" | "bash" => "bash",
        "ps1" => "powershell",
        "sql" => "sql",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "xml" => "xml",
        "html" | "htm" | "xhtml" => "html",
        "css" => "css",
        "md" | "markdown" => "markdown",
        _ => {
            if mime.contains("python") { "python" }
            else if mime.contains("rust") { "rust" }
            else { "text" }
        }
    }
}

// ---------------------------------------------------------------------------
// Attachments (Phase 1 P9)
// ---------------------------------------------------------------------------
//
// The chat composer can attach small text / source files. The Rust core
// reads the file, tolerates non-UTF-8 bytes (see
// `nexus_core::ingestion::extractors`), and returns a plain text payload
// that the frontend splices into the user message as fenced blocks. We
// deliberately do NOT automatically embed the file into the Knowledge
// Vault — the user must explicitly register a source for that. We also
// cap each attachment at `MAX_ATTACHMENT_BYTES` to avoid OOM on huge
// files. Attachments are not persisted; they live only for the duration
// of the next `chat_send` call.

pub const MAX_ATTACHMENT_BYTES: u64 = 256 * 1024; // 256 KB per file

#[derive(Debug, Deserialize)]
pub struct AttachmentRequest {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct AttachmentPayload {
    pub path: String,
    pub filename: String,
    pub kind: String,           // "text" | "code" | "unknown"
    pub mime_type: String,
    pub size_bytes: u64,
    pub text: String,
    pub truncated: bool,
    pub non_utf8_replaced: bool,
    pub line_count: Option<usize>,
}

#[tauri::command]
pub fn chat_read_attachment(req: AttachmentRequest) -> AppResult<AttachmentPayload> {
    use std::path::Path;
    let p = Path::new(&req.path);
    if !p.exists() {
        return Err(AppError::NotFound(format!("file: {}", req.path)));
    }
    let meta = std::fs::metadata(p).map_err(AppError::Io)?;
    if !meta.is_file() {
        return Err(AppError::Invalid(format!("not a regular file: {}", req.path)));
    }
    // Sniff the first few bytes so we can route to the
    // correct cap and extractor. The size check below
    // uses the appropriate cap depending on the file kind.
    let head = {
        use std::io::Read;
        let mut f = std::fs::File::open(p).map_err(AppError::Io)?;
        let mut buf = vec![0u8; 16.min(meta.len() as usize)];
        let _ = f.read(&mut buf).ok();
        buf
    };
    let is_image = looks_like_image(p, &head).is_some();
    let cap = if is_image { MAX_IMAGE_ATTACHMENT_BYTES } else { MAX_ATTACHMENT_BYTES };
    if meta.len() > cap {
        // Cap exceeded. For images, refuse outright — a partial
        // image is not a valid input. For text, read the head so
        // the user gets a useful preview.
        if is_image {
            return Err(AppError::Invalid(format!(
                "image attachment too large: {} bytes (max {})",
                meta.len(), cap
            )));
        }
        let mut f = std::fs::File::open(p).map_err(AppError::Io)?;
        use std::io::Read;
        let mut buf = vec![0u8; cap as usize];
        let n = f.read(&mut buf).map_err(AppError::Io)?;
        buf.truncate(n);
        let (text, kind, mime, non_utf8, line_count) = decode_payload(p, &buf);
        return Ok(AttachmentPayload {
            path: req.path.clone(),
            filename: p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
            kind,
            mime_type: mime,
            size_bytes: meta.len(),
            text,
            truncated: true,
            non_utf8_replaced: non_utf8,
            line_count,
        });
    }
    let bytes = std::fs::read(p).map_err(AppError::Io)?;
    let size = bytes.len() as u64;
    let (text, kind, mime, non_utf8, line_count) = decode_payload(p, &bytes);
    Ok(AttachmentPayload {
        path: req.path.clone(),
        filename: p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
        kind,
        mime_type: mime,
        size_bytes: size,
        text,
        truncated: false,
        non_utf8_replaced: non_utf8,
        line_count,
    })
}

/// Max size for a single image attachment, in bytes. 20 MB
/// is large enough for high-resolution screenshots without
/// being able to DoS the local model.
pub const MAX_IMAGE_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// Return true if the path's extension (or sniffed magic
/// bytes) looks like an image. Used to route images into the
/// vision payload path instead of the text-extraction path.
fn looks_like_image(p: &std::path::Path, bytes: &[u8]) -> Option<&'static str> {
    if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
        let lower = ext.to_ascii_lowercase();
        for known in IMAGE_EXTENSIONS {
            if lower == *known {
                return Some(match lower.as_str() {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    _ => "image/png",
                });
            }
        }
    }
    // Sniff magic bytes. PNG: 89 50 4E 47. JPEG: FF D8 FF. GIF: 47 49 46 38.
    // WebP: 52 49 46 46 ... 57 45 42 50.
    if bytes.len() >= 4 {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            return Some("image/png");
        }
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some("image/jpeg");
        }
        if bytes.starts_with(b"GIF8") {
            return Some("image/gif");
        }
        if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            return Some("image/webp");
        }
    }
    None
}

fn decode_payload(p: &std::path::Path, bytes: &[u8]) -> (String, String, String, bool, Option<usize>) {
    use nexus_core::ingestion::extractors;
    // Image attachments: encode as a data: URI so the LM Studio
    // client can ship the raw bytes through the OpenAI-compatible
    // multimodal message structure. The frontend uses `kind: image`
    // to render a thumbnail and the backend uses the same flag to
    // route the bytes to the vision payload path.
    if let Some(mime) = looks_like_image(p, bytes) {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let uri = format!("data:{};base64,{}", mime, b64);
        return (uri, "image".into(), mime.into(), false, None);
    }
    // Try the existing extractor first; it knows about the file kind and
    // can add line numbers for source code. Fall back to a raw lossy
    // decode for files the extractor does not recognise.
    if let Some(f) = extractors::extractor_for(p) {
        if let Ok(ext) = f(p) {
            let line_count = ext.metadata.get("line_count").and_then(|v| v.as_u64()).map(|v| v as usize);
            let non_utf8 = ext.metadata.get("non_utf8_replaced").and_then(|v| v.as_bool()).unwrap_or(false);
            return (ext.text, "code".into(), ext.mime_type, non_utf8, line_count);
        }
    }
    let (text, non_utf8) = match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), true),
    };
    let line_count = text.lines().count();
    (text, "text".into(), "text/plain".into(), non_utf8, Some(line_count))
}

// ---------------------------- Bug F ----------------------------
//
// Phase P4 — Bug F. Persist a memory extracted from an
// explicit save request, then synthesise a short
// assistant reply that the user can read in the chat.
// The model is NOT invoked, so there is no 91-second
// token-generation hang, no false "I'll remember that",
// and no silent failure.

#[allow(clippy::too_many_arguments)]
async fn handle_explicit_save(
    s: &Arc<AppState>,
    app: &tauri::AppHandle,
    conv: &nexus_core::conversations::Conversation,
    save: nexus_core::memory_v2::explicit_save::SaveResult,
) -> AppResult<String> {
    use nexus_core::conversations::{self, Message};
    use tokio_util::sync::CancellationToken;
    use tauri::Emitter;

    let started = std::time::Instant::now();
    let dispatched_at_ms = chrono::Utc::now().timestamp_millis();

    // Create the assistant message row up front so the
    // user sees a real (non-streaming) reply tied to a
    // stable id. The frontend rehydrates via
    // `listMessages` and renders the bubble from the
    // same row, so there is no orphan state.
    let now = chrono::Utc::now().timestamp_millis();
    let assistant_id = conversations::new_id();
    let reply_text = match save.outcome.as_str() {
        "saved" => format!(
            "Saved: “{}” (category: {}). The fact is now visible in Memory Center and available to subsequent chat requests.",
            save.content.clone().unwrap_or_default(),
            save.category.clone().unwrap_or_else(|| "project_fact".into()),
        ),
        "updated" => format!(
            "Already remembered: “{}” (category: {}). No duplicate record was created.",
            save.content.clone().unwrap_or_default(),
            save.category.clone().unwrap_or_else(|| "project_fact".into()),
        ),
        "rejected" => format!(
            "I couldn’t save that to memory. Reason: {}. If you meant to remember a fact, try: “Save to your memory that …”.",
            save.reason.clone().unwrap_or_else(|| "unknown".into()),
        ),
        // "failed" — never claim success.
        _ => format!(
            "I couldn’t save that to memory. Reason: {}. The original message is still in the conversation; please retry.",
            save.reason.clone().unwrap_or_else(|| "persistence error".into()),
        ),
    };
    let assistant_msg = Message {
        id: assistant_id.clone(),
        conversation_id: conv.id.clone(),
        role: "assistant".into(),
        content: reply_text.clone(),
        status: "complete".into(),
        token_count: None,
        latency_ms: Some(started.elapsed().as_millis() as i64),
        model_profile_id: None,
        created_at: now,
    };
    conversations::add_message(&s.pool, &assistant_msg)?;
    conversations::touch(&s.pool, &conv.id)?;

    // Emit the synthetic delta. We register a
    // transient cancel token so the request registry
    // shows a chat request was active; the registry
    // entry is removed on every exit path.
    let cancel = CancellationToken::new();
    let request_id = ulid::Ulid::new().to_string();
    s.register_request(
        &request_id,
        "chat",
        Some(&conv.id),
        Some(&assistant_id),
        cancel.clone(),
    );

    // Emit a "token" delta with the reply text, then a
    // "done" event so the chat UI sees the same
    // streaming sequence it would for a normal model
    // reply. The frontend listener patches the
    // assistant message in place; the final rehydrate
    // via `listMessages` shows the persisted content.
    let _ = app.emit("nexus:chat.delta", ChatDeltaEvent {
        conversation_id: conv.id.clone(),
        message_id: assistant_id.clone(),
        text: reply_text.clone(),
        kind: "token".into(),
        finish_reason: None,
        usage: None,
        dispatched_at_ms: Some(dispatched_at_ms),
        first_token_at_ms: Some(dispatched_at_ms),
        request_id: Some(request_id.clone()),
    });
    let _ = app.emit("nexus:chat.delta", ChatDeltaEvent {
        conversation_id: conv.id.clone(),
        message_id: assistant_id.clone(),
        text: reply_text,
        kind: "done".into(),
        finish_reason: Some("memory_save_explicit".into()),
        usage: None,
        dispatched_at_ms: Some(dispatched_at_ms),
        first_token_at_ms: Some(dispatched_at_ms),
        request_id: Some(request_id.clone()),
    });
    s.unregister_request(&request_id);

    // Surface the explicit-save outcome on a dedicated
    // event so the Memory Center page can refresh its
    // list without polling.
    #[derive(serde::Serialize, Clone)]
    struct MemorySavedEvent {
        outcome: String,
        memory_id: Option<String>,
        category: Option<String>,
        content: Option<String>,
        reason: Option<String>,
        conversation_id: String,
    }
    let _ = app.emit("nexus:memory.saved", MemorySavedEvent {
        outcome: save.outcome.clone(),
        memory_id: save.memory_id.clone(),
        category: save.category.clone(),
        content: save.content.clone(),
        reason: save.reason.clone(),
        conversation_id: conv.id.clone(),
    });

    Ok(assistant_id)
}