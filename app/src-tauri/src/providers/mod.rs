//! Model provider abstraction. Each backend speaks its own dialect of HTTP;
//! we normalize to a single streaming surface that the rest of NEXUS uses.
//!
//! In Phase 2 we ship:
//!   - `LmStudioProvider` (OpenAI-compatible, /v1/chat/completions SSE)
//!   - `LlamaCppProvider` (same dialect, default port 8081)
//!   - `OllamaProvider`   (Ollama's own /api/chat NDJSON dialect)
//!
//! Native tool calling is exposed as `tools_supported`; callers must still
//! validate every tool call against the registry before executing it.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

pub mod http;
pub mod lmstudio;
pub mod llamacpp;
pub mod ollama;
pub mod stream;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub context_len: Option<u32>,
    pub supports_tools: bool,
    pub gguf_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub reachable: bool,
    pub base_url: String,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    /// Text or multimodal content. For text-only messages this is
    /// a plain string. For multimodal messages (text + image)
    /// this is an array of content parts in OpenAI's shape:
    /// `{"type": "text", "text": "..."}` and
    /// `{"type": "image_url", "image_url": {"url": "data:image/png;base64,..."}}`.
    /// We keep this as `serde_json::Value` so the call site can
    /// pick the right shape without forcing every call site to
    /// branch.
    pub content: serde_json::Value,
}

impl ChatMessage {
    /// Helper to construct a plain text message. Use this when
    /// the message is text-only.
    pub fn text(role: impl Into<String>, text: impl Into<String>) -> Self {
        Self { role: role.into(), content: serde_json::Value::String(text.into()) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub system: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop: Option<Vec<String>>,
    /// OpenAI-compatible tool definitions. The model may or may not honour them.
    pub tools: Option<Vec<serde_json::Value>>,
    pub tool_choice: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// "token" (user-visible text) | "reasoning" (chain-of-thought) | "tool_call" | "error" | "done" | "cancelled"
    pub kind: String,
    pub text: Option<String>,
    pub tool_call: Option<serde_json::Value>,
    pub error: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSummary {
    pub model: String,
    pub content: String,
    pub tool_calls: Vec<serde_json::Value>,
    pub usage: Usage,
    pub latency_ms: u64,
    pub finish_reason: Option<String>,
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn base_url(&self) -> &str;
    async fn health(&self) -> ProviderHealth;
    async fn list_models(&self) -> AppResult<Vec<ModelInfo>>;
    async fn stream_chat(
        &self,
        req: ChatRequest,
        tx: tokio::sync::mpsc::Sender<StreamEvent>,
        cancel: CancellationToken,
    ) -> AppResult<ChatSummary>;
    /// Non-streaming chat completion. Returns the full
    /// assistant text in a single `ChatSummary` and does
    /// not emit intermediate events. Used for short
    /// auxiliary tasks (auto-title generation, intent
    /// classification, etc.) where streaming is
    /// unnecessary and would add overhead.
    async fn chat_complete(
        &self,
        req: ChatRequest,
        cancel: CancellationToken,
    ) -> AppResult<ChatSummary>;
    async fn embeddings(&self, model: &str, inputs: &[String]) -> AppResult<Vec<Vec<f32>>>;
}

/// Holds the *active* provider, switched at runtime.
pub struct ProviderSet {
    pub lmstudio: Arc<dyn ModelProvider>,
    pub llamacpp: Arc<dyn ModelProvider>,
    pub ollama: Arc<dyn ModelProvider>,
    active: Mutex<ProviderKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    LmStudio,
    LlamaCpp,
    Ollama,
}

impl ProviderKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "llamacpp" | "llama.cpp" => Self::LlamaCpp,
            "ollama" => Self::Ollama,
            _ => Self::LmStudio,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LmStudio => "lmstudio",
            Self::LlamaCpp => "llamacpp",
            Self::Ollama => "ollama",
        }
    }
}

impl ProviderSet {
    pub fn new(
        lmstudio: Arc<dyn ModelProvider>,
        llamacpp: Arc<dyn ModelProvider>,
        ollama: Arc<dyn ModelProvider>,
        initial: ProviderKind,
    ) -> Self {
        Self { lmstudio, llamacpp, ollama, active: Mutex::new(initial) }
    }

    pub fn active_kind(&self) -> ProviderKind {
        *self.active.lock()
    }

    pub fn set_active(&self, k: ProviderKind) {
        *self.active.lock() = k;
    }

    pub fn active(&self) -> Arc<dyn ModelProvider> {
        match self.active_kind() {
            ProviderKind::LmStudio => self.lmstudio.clone(),
            ProviderKind::LlamaCpp => self.llamacpp.clone(),
            ProviderKind::Ollama => self.ollama.clone(),
        }
    }

    pub fn by_kind(&self, k: ProviderKind) -> Arc<dyn ModelProvider> {
        match k {
            ProviderKind::LmStudio => self.lmstudio.clone(),
            ProviderKind::LlamaCpp => self.llamacpp.clone(),
            ProviderKind::Ollama => self.ollama.clone(),
        }
    }
}

/// Try to discover a reachable provider, in priority order.
pub async fn discover_first_reachable(
    set: &ProviderSet,
) -> Option<ProviderKind> {
    for k in [ProviderKind::LmStudio, ProviderKind::Ollama, ProviderKind::LlamaCpp] {
        let p = set.by_kind(k);
        if p.health().await.reachable {
            return Some(k);
        }
    }
    None
}

pub fn require<T>(v: Option<T>, ctx: &str) -> AppResult<T> {
    v.ok_or_else(|| AppError::Upstream(format!("missing {ctx}")))
}