//! Model Manager commands. Phase 2 surfaces provider health, list, and a
//! streaming test-prompt action. The actual chat with conversation history
//! lives in `commands::chat` (Phase 3).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::{AppError, AppResult};
use nexus_core::model_profiles::{self, ModelProfile};
use nexus_core::providers::{self, ChatRequest, ProviderHealth, ProviderKind, StreamEvent};

#[derive(Debug, Serialize)]
pub struct ProviderProbe {
    pub kind: String,
    pub health: ProviderHealth,
    pub models: Vec<nexus_core::providers::ModelInfo>,
}

async fn probe_kind(state: &AppState, kind: ProviderKind) -> ProviderProbe {
    let p = state.providers.read().by_kind(kind);
    let health = p.health().await;
    let models = if health.reachable { p.list_models().await.unwrap_or_default() } else { Vec::new() };
    ProviderProbe { kind: kind.as_str().to_string(), health, models }
}

#[tauri::command]
pub async fn providers_probe(state: State<'_, Arc<AppState>>) -> AppResult<Vec<ProviderProbe>> {
    let s = state.inner().clone();
    let mut out = Vec::new();
    out.push(probe_kind(&s, ProviderKind::LmStudio).await);
    out.push(probe_kind(&s, ProviderKind::Ollama).await);
    out.push(probe_kind(&s, ProviderKind::LlamaCpp).await);
    Ok(out)
}

#[tauri::command]
pub async fn providers_set_active(
    state: State<'_, Arc<AppState>>,
    kind: String,
) -> AppResult<()> {
    let s = state.inner().clone();
    let k = ProviderKind::from_str(&kind);
    s.providers.write().set_active(k);
    audit::log(&s.pool, "user", "provider.activate", Some(&kind), serde_json::json!({}))?;
    Ok(())
}

#[tauri::command]
pub fn models_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<ModelProfile>> {
    model_profiles::list(&state.pool)
}

#[tauri::command]
pub fn models_upsert(state: State<'_, Arc<AppState>>, profile: ModelProfile) -> AppResult<ModelProfile> {
    let mut p = profile;
    p.updated_at = chrono::Utc::now().timestamp_millis();
    if p.id.is_empty() { p.id = model_profiles::new_id(); p.created_at = p.updated_at; }
    model_profiles::upsert(&state.pool, &p)?;
    audit::log(&state.pool, "user", "model.upsert", Some(&p.id), serde_json::to_value(&p)?)?;
    Ok(p)
}

#[tauri::command]
pub fn models_delete(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    model_profiles::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "model.delete", Some(&id), serde_json::json!({}))?;
    Ok(())
}

#[tauri::command]
pub fn models_set_active(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    let list = model_profiles::list(&state.pool)?;
    let Some(p) = list.into_iter().find(|p| p.id == id) else {
        return Err(AppError::NotFound(format!("profile {id}")));
    };
    *state.active_model.write() = Some(p.model_id.clone());
    audit::log(&state.pool, "user", "model.activate", Some(&id), serde_json::json!({"model_id": p.model_id}))?;
    Ok(())
}

#[tauri::command]
pub async fn providers_start_lmstudio(app: AppHandle) -> AppResult<String> {
    // Use the user's `lms` CLI when it is on PATH. We only ever invoke the
    // server lifecycle on explicit user opt-in (`Settings -> Model Manager`).
    let lms = which::which("lms").map_err(|_| AppError::NotFound("lms CLI not on PATH".into()))?;
    let mut child = std::process::Command::new(lms);
    child.args(["server", "start"]);
    let out = child.output().map_err(AppError::Io)?;
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    audit::log(&app.state::<Arc<AppState>>().pool, "user", "lms.start", Some("lms"), serde_json::json!({"ok": out.status.success()}))?;
    Ok(text)
}

#[tauri::command]
pub async fn providers_stop_lmstudio(app: AppHandle) -> AppResult<String> {
    let lms = which::which("lms").map_err(|_| AppError::NotFound("lms CLI not on PATH".into()))?;
    let mut child = std::process::Command::new(lms);
    child.args(["server", "stop"]);
    let out = child.output().map_err(AppError::Io)?;
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    audit::log(&app.state::<Arc<AppState>>().pool, "user", "lms.stop", Some("lms"), serde_json::json!({"ok": out.status.success()}))?;
    Ok(text)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestPromptRequest {
    pub model: String,
    pub prompt: String,
    pub system: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[tauri::command]
pub async fn models_test_prompt(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    request: TestPromptRequest,
) -> AppResult<String> {
    let s = state.inner().clone();
    let provider = s.providers.read().active();
    let cancel = CancellationToken::new();
    s.register_cancel(cancel.clone());

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);
    let req = ChatRequest {
        model: request.model.clone(),
        messages: vec![nexus_core::providers::ChatMessage::text("user", request.prompt.clone())],
        system: request.system.clone(),
        temperature: request.temperature,
        top_p: None,
        max_tokens: request.max_tokens,
        stop: None,
        tools: None,
        tool_choice: None,
    };
    let provider_for_stream = provider.clone();
    let cancel_for_stream = cancel.clone();
    let app_for_stream = app.clone();
    let task = tokio::spawn(async move {
        let _ = provider_for_stream.stream_chat(req, tx, cancel_for_stream).await;
    });

    let mut full = String::new();
    let iter_started = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(180);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => { break; }
            _ = tokio::time::sleep(timeout) => {
                cancel.cancel();
                break;
            }
            maybe = rx.recv() => {
                match maybe {
                    Some(ev) => {
                        match ev.kind.as_str() {
                            "token" => if let Some(t) = ev.text { full.push_str(&t); }
                            "reasoning" => {
                                // Forward reasoning as its own event so the UI can show it
                                // separately. We do not mix it into the final answer.
                                if let Some(t) = ev.text {
                                    let _ = app_for_stream.emit("nexus:test.reasoning", t);
                                }
                            }
                            "error" => { if let Some(e) = ev.error { let _ = app_for_stream.emit("nexus:test.error", e); } }
                            "done" | "cancelled" => { break; }
                            _ => {}
                        }
                        if last_emit.elapsed() > std::time::Duration::from_millis(60) {
                            let _ = app_for_stream.emit("nexus:test.delta", &full);
                            last_emit = std::time::Instant::now();
                        }
                    }
                    None => break,
                }
                if iter_started.elapsed() > timeout { cancel.cancel(); break; }
            }
        }
    }
    let _ = task.await;
    let _ = app.emit("nexus:test.done", &full);
    Ok(full)
}

/// Cancel every in-flight request. Phase P4 — Bug L.
///
/// The Stop button on the Assistant page now cancels
/// **only** chat requests (so the user cannot accidentally
/// kill a Model Manager test prompt they kicked off in
/// another tab). Legacy callers — e.g. the Model Manager
/// "Stop test" button — still get the full cancel
/// behaviour they had before by passing
/// `cancel_all = true` (or by calling the legacy
/// `cancel_all` backend). For backwards compatibility the
/// default behaviour matches the new contract (chat-only).
#[tauri::command]
pub fn cancel_inflight(
    state: State<'_, Arc<AppState>>,
    cancel_all: Option<bool>,
) -> AppResult<usize> {
    if cancel_all.unwrap_or(false) {
        let v = state.active_cancellations.lock();
        let n = v.len();
        drop(v);
        state.cancel_all();
        Ok(n)
    } else {
        Ok(state.cancel_kind("chat"))
    }
}

// --------------------- embedding test ----------------------
//
// `models_test_embedding` calls the active provider with a
// short probe string and returns the embedding dimension. The
// frontend uses this to populate the `ModelProfile.dimension`
// field the first time the user runs backfill. The probe
// string is intentionally tiny so the call is fast and
// inexpensive.

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingTestRequest {
    pub model: String,
    /// Optional custom probe text. The default is a short,
    /// well-known ASCII string.
    pub probe: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingTestResponse {
    pub dimension: u32,
    /// Echo of the model id that produced the dimension.
    pub model: String,
    /// First 4 values of the embedding, useful for the
    /// "embeddings look right" smoke test in the UI.
    pub preview: Vec<f32>,
}

#[tauri::command]
pub async fn models_test_embedding(
    state: State<'_, Arc<AppState>>,
    request: EmbeddingTestRequest,
) -> AppResult<EmbeddingTestResponse> {
    let s = state.inner().clone();
    let provider = s.providers.read().active();
    let probe = request.probe.unwrap_or_else(|| "hello world".to_string());
    let vectors = provider
        .embeddings(&request.model, &[probe])
        .await?;
    let v = vectors.into_iter().next().ok_or_else(|| {
        AppError::Upstream(format!(
            "embedding test: provider returned no vectors for model {}",
            request.model
        ))
    })?;
    if v.is_empty() {
        return Err(AppError::Upstream(format!(
            "embedding test: provider returned a zero-length vector for model {}",
            request.model
        )));
    }
    let preview: Vec<f32> = v.iter().take(4).copied().collect();
    Ok(EmbeddingTestResponse {
        dimension: v.len() as u32,
        model: request.model,
        preview,
    })
}

// Mark `providers` as used even though we reach the trait through `ModelProvider`.
#[allow(dead_code)]
fn _typecheck() {
    let _ = providers::ProviderKind::LmStudio;
}