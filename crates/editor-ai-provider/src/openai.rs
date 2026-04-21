//! OpenAI `chat/completions` provider.

use std::sync::Arc;

use url::Url;

use crate::error::ProviderError;
use crate::openai_compat::{chat_completions_stream, chat_completions_url_from_base};
use crate::provider::{AiProvider, ChatEventStream};
use crate::rate_limit::MinuteRateLimit;
use crate::types::{ChatRequest, ModelInfo};

fn default_base() -> Url {
    Url::parse("https://api.openai.com").expect("valid")
}

#[derive(Clone)]
pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: Url,
    rate: Arc<MinuteRateLimit>,
}

impl std::fmt::Debug for OpenAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiProvider")
            .field("base_url", &self.base_url)
            .field("rate", &self.rate)
            .finish_non_exhaustive()
    }
}

impl OpenAiProvider {
    pub fn new(
        api_key: impl Into<String>,
        base_url: Option<Url>,
        rate: Arc<MinuteRateLimit>,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("editor-ai-provider/0.1")
                .build()
                .map_err(|e| ProviderError::message(e.to_string()))?,
            api_key: api_key.into(),
            base_url: base_url.unwrap_or_else(default_base),
            rate,
        })
    }

    fn completions_url(&self) -> Result<String, ProviderError> {
        chat_completions_url_from_base(&self.base_url)
    }
}

#[async_trait::async_trait]
impl AiProvider for OpenAiProvider {
    fn id(&self) -> &str {
        "openai"
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "o4-mini"]
            .into_iter()
            .map(|id| ModelInfo { id: id.into(), label: id.into() })
            .collect()
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatEventStream, ProviderError> {
        let url = self.completions_url()?;
        chat_completions_stream(&self.client, &url, Some(&self.api_key), req, self.rate.clone())
            .await
    }
}
