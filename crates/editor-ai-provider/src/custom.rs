//! User-configured OpenAI-compatible HTTP endpoint (vLLM, LM Studio, TGI, etc.).

use std::sync::Arc;

use url::Url;

use crate::error::ProviderError;
use crate::openai_compat::{chat_completions_stream, chat_completions_url_from_base};
use crate::provider::{AiProvider, ChatEventStream};
use crate::rate_limit::MinuteRateLimit;
use crate::types::{ChatRequest, ModelInfo};

#[derive(Clone)]
pub struct CustomHttpProvider {
    logical_id: String,
    client: reqwest::Client,
    base_url: Url,
    api_key: Option<String>,
    models: Vec<String>,
    rate: Arc<MinuteRateLimit>,
}

impl std::fmt::Debug for CustomHttpProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomHttpProvider")
            .field("logical_id", &self.logical_id)
            .field("base_url", &self.base_url)
            .field("rate", &self.rate)
            .finish_non_exhaustive()
    }
}

impl CustomHttpProvider {
    pub fn new(
        id: impl Into<String>,
        base_url: Url,
        api_key: Option<String>,
        models: Vec<String>,
        rate: Arc<MinuteRateLimit>,
    ) -> Result<Self, ProviderError> {
        let label: String = id.into();
        Ok(Self {
            logical_id: format!("custom:{label}"),
            client: reqwest::Client::builder()
                .user_agent("editor-ai-provider/0.1")
                .build()
                .map_err(|e| ProviderError::message(e.to_string()))?,
            base_url,
            api_key,
            models,
            rate,
        })
    }

    fn completions_url(&self) -> Result<String, ProviderError> {
        chat_completions_url_from_base(&self.base_url)
    }
}

#[async_trait::async_trait]
impl AiProvider for CustomHttpProvider {
    fn id(&self) -> &str {
        &self.logical_id
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        self.models.iter().cloned().map(|id| ModelInfo { label: id.clone(), id }).collect()
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatEventStream, ProviderError> {
        let url = self.completions_url()?;
        chat_completions_stream(&self.client, &url, self.api_key.as_deref(), req, self.rate.clone())
            .await
    }
}

impl CustomHttpProvider {
    /// Config table key (`my-vllm` in `[providers.custom.my-vllm]`).
    pub fn registry_key(&self) -> &str {
        self.logical_id.strip_prefix("custom:").unwrap_or(self.logical_id.as_str())
    }
}
