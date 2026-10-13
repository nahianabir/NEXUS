//! llama.cpp provider. `llama-server -m model.gguf --port 8081 --host 127.0.0.1`
//! exposes the same OpenAI-compatible surface as LM Studio, so we wrap the
//! LM Studio client with a different base URL.

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::providers::lmstudio::LmStudioProvider;
use crate::providers::{ChatRequest, ChatSummary, ModelInfo, ModelProvider, ProviderHealth, StreamEvent};
use crate::error::AppResult;

pub struct LlamaCppProvider {
    inner: LmStudioProvider,
}

impl LlamaCppProvider {
    pub fn new(base_url: impl Into<String>) -> reqwest::Result<Self> {
        Ok(Self { inner: LmStudioProvider::new(base_url)? })
    }
}

#[async_trait]
impl ModelProvider for LlamaCppProvider {
    fn name(&self) -> &'static str { "llamacpp" }
    fn base_url(&self) -> &str { self.inner.base_url() }

    async fn health(&self) -> ProviderHealth {
        self.inner.health().await
    }
    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        let mut info = self.inner.list_models().await?;
        for m in &mut info { m.provider = "llamacpp".into(); }
        Ok(info)
    }
    async fn stream_chat(
        &self,
        req: ChatRequest,
        tx: mpsc::Sender<StreamEvent>,
        cancel: CancellationToken,
    ) -> AppResult<ChatSummary> {
        self.inner.stream_chat(req, tx, cancel).await
    }
    async fn chat_complete(
        &self,
        req: ChatRequest,
        cancel: CancellationToken,
    ) -> AppResult<ChatSummary> {
        self.inner.chat_complete(req, cancel).await
    }
    async fn embeddings(&self, model: &str, inputs: &[String]) -> AppResult<Vec<Vec<f32>>> {
        self.inner.embeddings(model, inputs).await
    }
}
