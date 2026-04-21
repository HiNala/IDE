//! Local Ollama via OpenAI-compatible `/v1/chat/completions` plus `/api/tags` model lists.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use url::Url;

use crate::error::ProviderError;
use crate::openai_compat::{chat_completions_stream, chat_completions_url_from_base};
use crate::provider::{AiProvider, ChatEventStream};
use crate::rate_limit::MinuteRateLimit;
use crate::types::{ChatRequest, ModelInfo};

fn default_base() -> Url {
    Url::parse("http://localhost:11434").expect("valid")
}

#[derive(Clone)]
pub struct OllamaProvider {
    client: reqwest::Client,
    base_url: Url,
    rate: Arc<MinuteRateLimit>,
}

impl std::fmt::Debug for OllamaProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OllamaProvider")
            .field("base_url", &self.base_url)
            .field("rate", &self.rate)
            .finish_non_exhaustive()
    }
}

impl OllamaProvider {
    pub fn new(base_url: Option<Url>, rate: Arc<MinuteRateLimit>) -> Result<Self, ProviderError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("editor-ai-provider/0.1")
                .build()
                .map_err(|e| ProviderError::message(e.to_string()))?,
            base_url: base_url.unwrap_or_else(default_base),
            rate,
        })
    }

    fn completions_url(&self) -> Result<String, ProviderError> {
        chat_completions_url_from_base(&self.base_url)
    }

    fn tags_url(&self) -> Result<String, ProviderError> {
        Ok(self
            .base_url
            .join("api/tags")
            .map_err(|e| ProviderError::message(e.to_string()))?
            .as_str()
            .to_owned())
    }
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

#[async_trait::async_trait]
impl AiProvider for OllamaProvider {
    fn id(&self) -> &str {
        "ollama"
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        let url = match self.tags_url() {
            Ok(u) => u,
            Err(_) => return fallback_models(),
        };
        let client =
            match reqwest::blocking::Client::builder().timeout(Duration::from_secs(3)).build() {
                Ok(c) => c,
                Err(_) => return fallback_models(),
            };
        let resp = match client.get(&url).send() {
            Ok(r) => r,
            Err(_) => return fallback_models(),
        };
        if !resp.status().is_success() {
            return fallback_models();
        }
        let Ok(body) = resp.json::<TagsResponse>() else {
            return fallback_models();
        };
        body.models.into_iter().map(|m| ModelInfo { label: m.name.clone(), id: m.name }).collect()
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatEventStream, ProviderError> {
        let url = self.completions_url()?;
        chat_completions_stream(&self.client, &url, None, req, self.rate.clone()).await
    }
}

fn fallback_models() -> Vec<ModelInfo> {
    vec![ModelInfo {
        id: "llama3.2".into(),
        label: "llama3.2 (ollama not reachable — using placeholder)".into(),
    }]
}
