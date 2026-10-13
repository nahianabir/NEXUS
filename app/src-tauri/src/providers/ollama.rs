//! Ollama provider. Different dialect: `/api/chat` streams NDJSON; each line
//! is a `{"message":{"role":"assistant","content":"..."},"done":false}` object.
//! Final frame has `done:true` and may carry `eval_count`/`prompt_eval_count`.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{upstream, AppError, AppResult};
use crate::providers::http::new_client;
use crate::providers::stream::{FrameParser, RawFrame};
use crate::providers::{
    ChatMessage, ChatRequest, ChatSummary, ModelInfo, ModelProvider, ProviderHealth, StreamEvent,
    Usage,
};

pub struct OllamaProvider {
    base_url: String,
    client: Arc<reqwest::Client>,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>) -> reqwest::Result<Self> {
        // Ollama's API is at the host root, not under `/v1`.
        let mut u = base_url.into().trim_end_matches('/').to_string();
        if u.ends_with("/v1") { u.truncate(u.len() - 3); }
        Ok(Self { base_url: u, client: Arc::new(new_client()?) })
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn name(&self) -> &'static str { "ollama" }
    fn base_url(&self) -> &str { &self.base_url }

    async fn health(&self) -> ProviderHealth {
        let started = Instant::now();
        let url = format!("{}/api/tags", self.base_url);
        match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => ProviderHealth {
                reachable: true,
                base_url: self.base_url.clone(),
                latency_ms: Some(started.elapsed().as_millis() as u64),
                error: None,
                version: None,
            },
            Ok(r) => ProviderHealth {
                reachable: false,
                base_url: self.base_url.clone(),
                latency_ms: Some(started.elapsed().as_millis() as u64),
                error: Some(format!("HTTP {}", r.status())),
                version: None,
            },
            Err(e) => ProviderHealth {
                reachable: false,
                base_url: self.base_url.clone(),
                latency_ms: Some(started.elapsed().as_millis() as u64),
                error: Some(e.to_string()),
                version: None,
            },
        }
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);
        let res = self.client.get(&url).send().await.map_err(upstream)?;
        if !res.status().is_success() {
            return Err(AppError::Upstream(format!("HTTP {}", res.status())));
        }
        #[derive(Deserialize)]
        struct List { models: Vec<Entry> }
        #[derive(Deserialize)]
        struct Entry { name: String, #[serde(default)] _size: Option<u64> }
        let parsed: List = res.json().await.map_err(upstream)?;
        Ok(parsed.models.into_iter().map(|e| ModelInfo {
            id: e.name.clone(),
            display_name: e.name,
            provider: "ollama".into(),
            context_len: None,
            supports_tools: false,
            gguf_path: None,
        }).collect())
    }

    async fn stream_chat(
        &self,
        req: ChatRequest,
        tx: mpsc::Sender<StreamEvent>,
        cancel: CancellationToken,
    ) -> AppResult<ChatSummary> {
        let url = format!("{}/api/chat", self.base_url);
        let started = Instant::now();
        let mut messages: Vec<serde_json::Value> = Vec::new();
        if let Some(sys) = &req.system {
            if !sys.trim().is_empty() {
                messages.push(serde_json::json!({ "role": "system", "content": sys }));
            }
        }
        for m in &req.messages {
            messages.push(serde_json::json!({ "role": m.role, "content": m.content }));
        }
        let mut body = serde_json::json!({
            "model": req.model,
            "messages": messages,
            "stream": true,
        });
        if let Some(t) = req.temperature { body["options"] = serde_json::json!({ "temperature": t }); }
        if let Some(m) = req.max_tokens { body["options"] = match body.get("options") { Some(o) => { let mut v = o.clone(); v["num_predict"] = serde_json::json!(m); v }, None => serde_json::json!({ "num_predict": m }) }; }
        let request = self.client.post(&url).json(&body);
        let res = tokio::select! {
            r = request.send() => r.map_err(upstream)?,
            _ = cancel.cancelled() => {
                let _ = tx.send(StreamEvent { kind: "cancelled".into(), text: None, tool_call: None, error: None, finish_reason: Some("cancelled".into()), usage: None }).await;
                return Err(AppError::Cancelled);
            }
        };
        if !res.status().is_success() {
            let status = res.status();
            let txt = res.text().await.unwrap_or_default();
            let _ = tx.send(StreamEvent { kind: "error".into(), text: None, tool_call: None, error: Some(format!("HTTP {status}: {txt}")), finish_reason: None, usage: None }).await;
            return Err(AppError::Upstream(format!("HTTP {status}: {txt}")));
        }

        let mut stream = res.bytes_stream();
        let mut buf = String::new();
        let mut content = String::new();
        let mut usage = Usage::default();
        let mut finish_reason: Option<String> = None;
        #[derive(Deserialize)]
        struct Msg { content: String }
        #[derive(Deserialize)]
        struct Frame { message: Option<Msg>, done: bool, #[serde(default)] done_reason: Option<String>, #[serde(default)] eval_count: Option<u32>, #[serde(default)] prompt_eval_count: Option<u32> }

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    let _ = tx.send(StreamEvent { kind: "cancelled".into(), text: None, tool_call: None, error: None, finish_reason: Some("cancelled".into()), usage: None }).await;
                    return Err(AppError::Cancelled);
                }
                next = stream.next() => {
                    let Some(chunk) = next else { break };
                    let chunk = chunk.map_err(upstream)?;
                    for frame in FrameParser::push(&mut buf, &chunk) {
                        if let RawFrame::NdjsonLine(s) = frame {
                            match serde_json::from_str::<Frame>(&s) {
                                Ok(f) => {
                                    if let Some(m) = f.message {
                                        if !m.content.is_empty() {
                                            content.push_str(&m.content);
                                            let _ = tx.send(StreamEvent { kind: "token".into(), text: Some(m.content), tool_call: None, error: None, finish_reason: None, usage: None }).await;
                                        }
                                    }
                                    if f.done {
                                        finish_reason = f.done_reason.or(Some("stop".into()));
                                        usage.completion_tokens = f.eval_count;
                                        usage.prompt_tokens = f.prompt_eval_count;
                                        if let (Some(a), Some(b)) = (usage.prompt_tokens, usage.completion_tokens) {
                                            usage.total_tokens = Some(a + b);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("ollama: malformed NDJSON line: {e}; body={s}");
                                }
                            }
                        }
                    }
                }
            }
        }

        let _ = tx.send(StreamEvent {
            kind: "done".into(),
            text: None,
            tool_call: None,
            error: None,
            finish_reason: finish_reason.clone(),
            usage: Some(usage.clone()),
        }).await;

        Ok(ChatSummary {
            model: req.model,
            content,
            tool_calls: Vec::new(),
            usage,
            latency_ms: started.elapsed().as_millis() as u64,
            finish_reason,
        })
    }

    async fn embeddings(&self, model: &str, inputs: &[String]) -> AppResult<Vec<Vec<f32>>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let mut out = Vec::with_capacity(inputs.len());
        for input in inputs {
            let body = serde_json::json!({ "model": model, "prompt": input });
            let res = self.client.post(&url).json(&body).send().await.map_err(upstream)?;
            if !res.status().is_success() {
                return Err(AppError::Upstream(format!("embeddings HTTP {}", res.status())));
            }
            #[derive(Deserialize)]
            struct R { embedding: Vec<f32> }
            let r: R = res.json().await.map_err(upstream)?;
            out.push(r.embedding);
        }
        Ok(out)
    }

    /// Non-streaming chat completion. Ollama's `/api/chat`
    /// supports `stream: false` to return a single JSON
    /// object instead of NDJSON events.
    async fn chat_complete(
        &self,
        req: ChatRequest,
        cancel: CancellationToken,
    ) -> AppResult<ChatSummary> {
        let url = format!("{}/api/chat", self.base_url);
        let started = Instant::now();
        let mut messages: Vec<serde_json::Value> = Vec::new();
        if let Some(sys) = &req.system {
            if !sys.trim().is_empty() {
                messages.push(serde_json::json!({ "role": "system", "content": sys }));
            }
        }
        for m in &req.messages {
            messages.push(serde_json::json!({ "role": m.role, "content": m.content }));
        }
        let body = serde_json::json!({
            "model": req.model,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": req.temperature.unwrap_or(0.3),
                "num_predict": req.max_tokens.unwrap_or(64),
            }
        });
        let send_fut = self.client.post(&url).json(&body).send();
        let res = tokio::select! {
            r = send_fut => r.map_err(upstream)?,
            _ = cancel.cancelled() => return Err(AppError::Cancelled),
        };
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!("chat_complete HTTP {status}: {text}")));
        }
        #[derive(Deserialize)]
        struct Resp {
            #[serde(default)]
            message: Option<RespMessage>,
            #[serde(default)]
            done_reason: Option<String>,
            #[serde(default)]
            prompt_eval_count: Option<u32>,
            #[serde(default)]
            eval_count: Option<u32>,
        }
        #[derive(Deserialize)]
        struct RespMessage { content: String }
        let parsed: Resp = res.json().await.map_err(upstream)?;
        let content = parsed.message.map(|m| m.content).unwrap_or_default();
        let usage = Usage {
            prompt_tokens: parsed.prompt_eval_count,
            completion_tokens: parsed.eval_count,
            total_tokens: match (parsed.prompt_eval_count, parsed.eval_count) {
                (Some(p), Some(c)) => Some(p + c),
                _ => None,
            },
        };
        let elapsed = started.elapsed();
        Ok(ChatSummary {
            model: req.model,
            content,
            tool_calls: Vec::new(),
            usage,
            latency_ms: elapsed.as_millis() as u64,
            finish_reason: parsed.done_reason,
        })
    }
}

// Touch the unused ChatMessage import so the linter is quiet.
#[allow(dead_code)]
fn _keep<T>() {}
#[allow(dead_code)]
fn _typecheck(_: ChatMessage) {}
