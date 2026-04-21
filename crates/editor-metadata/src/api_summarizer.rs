//! Cheap remote summarization: OpenAI-compatible or Anthropic Messages (non-streaming).

use std::path::Path;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::SummarizerError;
use crate::prompt::{self, build_summarizer_user_message};
use crate::schema::{parse, Sidecar};
use crate::session::SessionLog;
use crate::summarizer::Summarizer;
use url::Url;

/// Provider kind for [`ApiSummarizer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiProviderKind {
    OpenAiCompatible,
    Anthropic,
}

#[derive(Clone, Debug)]
pub struct ApiSummarizer {
    client: reqwest::Client,
    kind: ApiProviderKind,
    url: String,
    model: String,
    auth_header: Option<(String, String)>,
}

impl ApiSummarizer {
    pub fn openai_compatible(
        api_base: &str,
        bearer_token: &str,
        model: impl Into<String>,
    ) -> Result<Self, SummarizerError> {
        let u = Url::parse(api_base.trim_end_matches('/'))
            .map_err(|e| SummarizerError::Message(e.to_string()))?;
        let url = u
            .join("v1/chat/completions")
            .map_err(|e| SummarizerError::Message(e.to_string()))?
            .to_string();
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("editor-metadata/0.1")
                .build()
                .map_err(|e| SummarizerError::Http(e.to_string()))?,
            kind: ApiProviderKind::OpenAiCompatible,
            url,
            model: model.into(),
            auth_header: Some(("Authorization".into(), format!("Bearer {bearer_token}"))),
        })
    }

    pub fn anthropic(
        api_base: Option<&str>,
        api_key: &str,
        model: impl Into<String>,
    ) -> Result<Self, SummarizerError> {
        let u = Url::parse(api_base.unwrap_or("https://api.anthropic.com").trim_end_matches('/'))
            .map_err(|e| SummarizerError::Message(e.to_string()))?;
        let url =
            u.join("v1/messages").map_err(|e| SummarizerError::Message(e.to_string()))?.to_string();
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("editor-metadata/0.1")
                .build()
                .map_err(|e| SummarizerError::Http(e.to_string()))?,
            kind: ApiProviderKind::Anthropic,
            url,
            model: model.into(),
            auth_header: Some(("x-api-key".into(), api_key.to_string())),
        })
    }
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Option<Vec<OpenAiChoice>>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiMsg>,
}

#[derive(Deserialize)]
struct OpenAiMsg {
    content: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Option<Vec<AnthropicBlock>>,
}

#[derive(Deserialize)]
struct AnthropicBlock {
    text: Option<String>,
}

#[async_trait]
impl Summarizer for ApiSummarizer {
    async fn summarize(
        &self,
        file_path: &Path,
        prior_sidecar: Option<&Sidecar>,
        session: &SessionLog,
        current_source: &str,
    ) -> Result<Sidecar, SummarizerError> {
        let user = build_summarizer_user_message(file_path, prior_sidecar, session, current_source);
        let text = match self.kind {
            ApiProviderKind::OpenAiCompatible => {
                let mut req =
                    self.client.post(&self.url).header("Content-Type", "application/json");
                if let Some((k, v)) = &self.auth_header {
                    req = req.header(k.as_str(), v.as_str());
                }
                let body = json!({
                    "model": self.model,
                    "stream": false,
                    "max_tokens": 1000,
                    "messages": [
                        {"role": "system", "content": prompt::SUMMARIZE_SYSTEM},
                        {"role": "user", "content": user},
                    ]
                });
                let resp = req
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| SummarizerError::Http(e.to_string()))?;
                if !resp.status().is_success() {
                    let status = resp.status();
                    let t = resp.text().await.unwrap_or_default();
                    return Err(SummarizerError::Http(format!("api HTTP {}: {}", status, t)));
                }
                let p: OpenAiChatResponse =
                    resp.json().await.map_err(|e| SummarizerError::Http(e.to_string()))?;
                p.choices
                    .and_then(|c| c.into_iter().next())
                    .and_then(|x| x.message)
                    .and_then(|m| m.content)
            }
            ApiProviderKind::Anthropic => {
                let mut req = self
                    .client
                    .post(&self.url)
                    .header("Content-Type", "application/json")
                    .header("anthropic-version", "2023-06-01");
                if let Some((k, v)) = &self.auth_header {
                    req = req.header(k.as_str(), v.as_str());
                }
                let body = json!({
                    "model": self.model,
                    "max_tokens": 1000,
                    "stream": false,
                    "system": prompt::SUMMARIZE_SYSTEM,
                    "messages": [{"role": "user", "content": user}],
                });
                let resp = req
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| SummarizerError::Http(e.to_string()))?;
                if !resp.status().is_success() {
                    let status = resp.status();
                    let t = resp.text().await.unwrap_or_default();
                    return Err(SummarizerError::Http(format!("anthropic HTTP {}: {}", status, t)));
                }
                let p: AnthropicResponse =
                    resp.json().await.map_err(|e| SummarizerError::Http(e.to_string()))?;
                p.content.map(|blocks| {
                    blocks.into_iter().filter_map(|b| b.text).collect::<Vec<_>>().join("\n")
                })
            }
        };
        let text = text.filter(|s| !s.trim().is_empty()).ok_or(SummarizerError::EmptyResponse)?;
        parse(&prompt::strip_model_markdown_fence(&text)).map_err(Into::into)
    }
}
