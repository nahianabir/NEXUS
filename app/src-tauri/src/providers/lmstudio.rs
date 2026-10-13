//! LM Studio provider. Speaks the OpenAI-compatible dialect that LM Studio
//! exposes when its local server is running. SSE streaming is used for chat;
//! embeddings are read from `/v1/embeddings` when the active model supports
//! them. (Qwen3.5-9B and Qwen2.5-Coder-14B do not; the embedding role is
//! filled by a separate model the user picks.)

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{upstream, AppError, AppResult};
use crate::providers::http::new_client;
use crate::providers::stream::{FrameParser, RawFrame};
use crate::providers::{
    ChatRequest, ChatSummary, ModelInfo, ModelProvider, ProviderHealth, StreamEvent,
    Usage,
};

pub struct LmStudioProvider {
    base_url: String,
    client: Arc<reqwest::Client>,
}

impl LmStudioProvider {
    pub fn new(base_url: impl Into<String>) -> reqwest::Result<Self> {
        Ok(Self { base_url: base_url.into().trim_end_matches('/').to_string(), client: Arc::new(new_client()?) })
    }
}

#[async_trait]
impl ModelProvider for LmStudioProvider {
    fn name(&self) -> &'static str { "lmstudio" }
    fn base_url(&self) -> &str { &self.base_url }

    async fn health(&self) -> ProviderHealth {
        let started = Instant::now();
        let url = format!("{}/models", self.base_url);
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
        let url = format!("{}/models", self.base_url);
        let res = self.client.get(&url).send().await.map_err(upstream)?;
        if !res.status().is_success() {
            return Err(AppError::Upstream(format!("GET /models -> HTTP {}", res.status())));
        }
        #[derive(Deserialize)]
        struct List { data: Vec<Entry> }
        #[derive(Deserialize)]
        struct Entry { id: String, #[serde(default)] context: Option<u32> }
        let parsed: List = res.json().await.map_err(upstream)?;
        Ok(parsed.data.into_iter().map(|e| ModelInfo {
            id: e.id.clone(),
            display_name: e.id,
            provider: self.name().into(),
            context_len: e.context,
            supports_tools: true, // LM Studio advertises OpenAI-compatible tools; we still validate.
            gguf_path: None,
        }).collect())
    }

    async fn stream_chat(
        &self,
        req: ChatRequest,
        tx: mpsc::Sender<StreamEvent>,
        cancel: CancellationToken,
    ) -> AppResult<ChatSummary> {
        let url = format!("{}/chat/completions", self.base_url);
        let started = Instant::now();

        // Build the request body in the OpenAI shape, threading our system
        // prompt into the messages list because LM Studio honours both.
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
            "temperature": req.temperature.unwrap_or(0.7),
            "top_p": req.top_p.unwrap_or(0.95),
        });
        if let Some(m) = req.max_tokens { body["max_tokens"] = serde_json::json!(m); }
        if let Some(s) = req.stop { body["stop"] = serde_json::json!(s); }
        if let Some(t) = req.tools { body["tools"] = serde_json::json!(t); }
        if let Some(tc) = req.tool_choice { body["tool_choice"] = serde_json::json!(tc); }

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
        let mut tool_calls: Vec<serde_json::Value> = Vec::new();
        let mut usage = Usage::default();
        let mut finish_reason: Option<String> = None;
        // `data: [DONE]` is OpenAI's standard end-of-stream sentinel.
        // Some servers (LM Studio, llama.cpp's OpenAI shim) emit
        // `[DONE]` and then keep the HTTP connection open until the
        // client closes it. If we treat `[DONE]` as a no-op and
        // keep waiting for the next chunk, the stream hangs forever
        // and the chat UI stays stuck on "Waiting for model". The
        // fix: when we see `[DONE]`, break out of the loop so the
        // `done` event is sent to the consumer and the request
        // reaches its terminal state.
        let mut sse_done_seen = false;

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
                        match frame {
                            RawFrame::SseDone => {
                                // OpenAI's standard end-of-stream
                                // sentinel. Treat as authoritative
                                // end of stream and stop reading the
                                // HTTP body. Without this, some
                                // servers keep the socket open and
                                // the request hangs forever.
                                sse_done_seen = true;
                                break;
                            }
                            RawFrame::SseData(s) => {
                                #[derive(Deserialize)]
                                struct Choice { delta: Delta, finish_reason: Option<String> }
                                #[derive(Deserialize)]
                                struct Delta {
                                    content: Option<String>,
                                    reasoning_content: Option<String>,
                                    tool_calls: Option<Vec<serde_json::Value>>,
                                }
                                #[derive(Deserialize)]
                                struct StreamResp { choices: Vec<Choice>, usage: Option<Usage> }
                                match serde_json::from_str::<StreamResp>(&s) {
                                Ok(r) => {
                                    if let Some(c) = r.choices.first() {
                                        if let Some(text) = &c.delta.content {
                                            if !text.is_empty() {
                                                content.push_str(text);
                                                let _ = tx.send(StreamEvent { kind: "token".into(), text: Some(text.clone()), tool_call: None, error: None, finish_reason: None, usage: None }).await;
                                            }
                                        }
                                        if let Some(reason) = &c.delta.reasoning_content {
                                            if !reason.is_empty() {
                                                let _ = tx.send(StreamEvent { kind: "reasoning".into(), text: Some(reason.clone()), tool_call: None, error: None, finish_reason: None, usage: None }).await;
                                            }
                                        }
                                        if let Some(tcs) = &c.delta.tool_calls {
                                            for tc in tcs {
                                                tool_calls.push(tc.clone());
                                                let _ = tx.send(StreamEvent { kind: "tool_call".into(), text: None, tool_call: Some(tc.clone()), error: None, finish_reason: None, usage: None }).await;
                                            }
                                        }
                                        if c.finish_reason.is_some() {
                                            finish_reason = c.finish_reason.clone();
                                        }
                                    }
                                    if let Some(u) = r.usage { usage = u; }
                                }
                                    Err(e) => {
                                        log::warn!("lmstudio: malformed SSE frame: {e}; body={s}");
                                    }
                                }
                            }
                            RawFrame::NdjsonLine(_) | RawFrame::Other(_) | RawFrame::Eof => {}
                        }
                    }
                    if sse_done_seen {
                        // Drain any remaining bytes in the response
                        // body and break the outer loop. We do not
                        // rely on the connection closing because
                        // some servers keep the socket open after
                        // sending `data: [DONE]`.
                        break;
                    }
                }
            }
            if sse_done_seen { break; }
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
            tool_calls,
            usage,
            latency_ms: started.elapsed().as_millis() as u64,
            finish_reason,
        })
    }

    async fn embeddings(&self, model: &str, inputs: &[String]) -> AppResult<Vec<Vec<f32>>> {
        let url = format!("{}/embeddings", self.base_url);
        let body = serde_json::json!({ "model": model, "input": inputs });
        let res = self.client.post(&url).json(&body).send().await.map_err(upstream)?;
        if !res.status().is_success() {
            return Err(AppError::Upstream(format!("embeddings HTTP {}", res.status())));
        }
        #[derive(Deserialize)]
        struct Resp { data: Vec<EmbedItem> }
        #[derive(Deserialize)]
        struct EmbedItem { embedding: Vec<f32> }
        let parsed: Resp = res.json().await.map_err(upstream)?;
        Ok(parsed.data.into_iter().map(|e| e.embedding).collect())
    }

    /// Non-streaming chat completion for short auxiliary
    /// requests (auto-title generation, intent
    /// classification, etc.). We pass `stream: false` and
    /// parse the single JSON response. This is significantly
    /// cheaper than opening an SSE channel just to read
    /// ~24 tokens.
    async fn chat_complete(
        &self,
        req: ChatRequest,
        cancel: CancellationToken,
    ) -> AppResult<ChatSummary> {
        let url = format!("{}/chat/completions", self.base_url);
        let started = Instant::now();
        // Same payload shape as the streaming path, with
        // `stream: false` to opt out of SSE.
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
            "temperature": req.temperature.unwrap_or(0.3),
            "max_tokens": req.max_tokens.unwrap_or(64),
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
            choices: Vec<Choice>,
            #[serde(default)]
            usage: Option<UsageDto>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: ChoiceMessage,
            #[serde(default)]
            finish_reason: Option<String>,
        }
        #[derive(Deserialize)]
        struct ChoiceMessage { content: String }
        #[derive(Deserialize)]
        struct UsageDto {
            #[serde(default)] prompt_tokens: Option<u32>,
            #[serde(default)] completion_tokens: Option<u32>,
            #[serde(default)] total_tokens: Option<u32>,
        }
        let parsed: Resp = res.json().await.map_err(upstream)?;
        let first = parsed.choices.into_iter().next();
        let content = first.as_ref().map(|c| c.message.content.clone()).unwrap_or_default();
        let finish_reason = first.as_ref().and_then(|c| c.finish_reason.clone());
        let usage = parsed.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }).unwrap_or(Usage { prompt_tokens: None, completion_tokens: None, total_tokens: None });
        let elapsed = started.elapsed();
        Ok(ChatSummary {
            model: req.model,
            content,
            tool_calls: Vec::new(),
            usage,
            latency_ms: elapsed.as_millis() as u64,
            finish_reason,
        })
    }
}

// Touch the unused Duration import so the linter is quiet.
#[allow(dead_code)]
fn _keep_duration_import() -> Duration { Duration::from_secs(0) }
