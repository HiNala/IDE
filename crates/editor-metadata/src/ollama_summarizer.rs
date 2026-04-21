//! Local Ollama summarization via OpenAI-compatible `/v1/chat/completions` (non-streaming).

use std::path::Path;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use url::Url;

use crate::error::SummarizerError;
use crate::prompt::{self, build_summarizer_user_message};
use crate::schema::{parse, Sidecar};
use crate::session::SessionLog;
use crate::summarizer::Summarizer;

#[derive(Clone, Debug)]
pub struct OllamaSummarizer {
    client: reqwest::Client,
    chat_url: String,
    model: String,
}

impl OllamaSummarizer {
    pub fn new(base_url: Option<&str>, model: impl Into<String>) -> Result<Self, SummarizerError> {
        let base = base_url
            .map(|s| s.trim_end_matches('/').to_string())
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let u = Url::parse(&base).map_err(|e| SummarizerError::Message(e.to_string()))?;
        let chat_url = u
            .join("v1/chat/completions")
            .map_err(|e| SummarizerError::Message(e.to_string()))?
            .to_string();
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("editor-metadata/0.1")
                .build()
                .map_err(|e| SummarizerError::Http(e.to_string()))?,
            chat_url,
            model: model.into(),
        })
    }

    pub fn default_model() -> &'static str {
        "llama3.2:3b"
    }
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    choices: Option<Vec<OllamaChoice>>,
}

#[derive(Deserialize)]
struct OllamaChoice {
    message: Option<OllamaMsg>,
}

#[derive(Deserialize)]
struct OllamaMsg {
    content: Option<String>,
}

#[async_trait]
impl Summarizer for OllamaSummarizer {
    async fn summarize(
        &self,
        file_path: &Path,
        prior_sidecar: Option<&Sidecar>,
        session: &SessionLog,
        current_source: &str,
    ) -> Result<Sidecar, SummarizerError> {
        let user = build_summarizer_user_message(file_path, prior_sidecar, session, current_source);
        let req = json!({
            "model": self.model,
            "stream": false,
            "max_tokens": 1000,
            "messages": [
                {"role": "system", "content": prompt::SUMMARIZE_SYSTEM},
                {"role": "user", "content": user}
            ]
        });
        let resp = self
            .client
            .post(&self.chat_url)
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| SummarizerError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let t = resp.text().await.unwrap_or_default();
            return Err(SummarizerError::Http(format!("ollama HTTP {}: {}", status, t)));
        }
        let parsed: OllamaChatResponse =
            resp.json().await.map_err(|e| SummarizerError::Http(e.to_string()))?;
        let text = parsed
            .choices
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .filter(|s| !s.trim().is_empty())
            .ok_or(SummarizerError::EmptyResponse)?;
        parse(&prompt::strip_model_markdown_fence(&text)).map_err(Into::into)
    }
}
