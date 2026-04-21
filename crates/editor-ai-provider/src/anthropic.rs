//! Anthropic Messages API (`/v1/messages`) with SSE streaming.

use std::sync::Arc;

use futures::stream::Stream;
use futures::StreamExt;
use serde::Serialize;
use serde_json::json;
use url::Url;

use crate::error::ProviderError;
use crate::http::post_json_expect_success;
use crate::provider::{AiProvider, ChatEventStream};
use crate::rate_limit::MinuteRateLimit;
use crate::types::{ChatEvent, ChatRequest, ContentBlock, Message, ModelInfo, StopReason};

const ANTHROPIC_VERSION: &str = "2023-06-01";

fn default_base_url() -> Url {
    Url::parse("https://api.anthropic.com").expect("valid url")
}

#[derive(Clone)]
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: Url,
    rate: Arc<MinuteRateLimit>,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("base_url", &self.base_url)
            .field("rate", &self.rate)
            .finish_non_exhaustive()
    }
}

impl AnthropicProvider {
    pub fn new(
        api_key: impl Into<String>,
        base_url: Option<Url>,
        rate: Arc<MinuteRateLimit>,
    ) -> Result<Self, ProviderError> {
        let base_url = base_url.unwrap_or_else(default_base_url);
        Ok(Self {
            client: reqwest::Client::builder()
                .user_agent("editor-ai-provider/0.1")
                .build()
                .map_err(|e| ProviderError::message(e.to_string()))?,
            api_key: api_key.into(),
            base_url,
            rate,
        })
    }

    fn url_messages(&self) -> Result<String, ProviderError> {
        Ok(self
            .base_url
            .join("v1/messages")
            .map_err(|e| ProviderError::message(e.to_string()))?
            .to_string())
    }
}

#[async_trait::async_trait]
impl AiProvider for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        [
            "claude-opus-4-7",
            "claude-sonnet-4-6",
            "claude-3-5-sonnet-20241022",
            "claude-3-5-haiku-20241022",
        ]
        .into_iter()
        .map(|id| ModelInfo { id: id.into(), label: id.into() })
        .collect()
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatEventStream, ProviderError> {
        self.rate.acquire().await;
        if !req.stream {
            return Err(ProviderError::message(
                "AnthropicProvider currently supports stream=true only",
            ));
        }
        let url = self.url_messages()?;
        let body = build_anthropic_body(&req)?;
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-api-key",
            reqwest::header::HeaderValue::from_str(&self.api_key)
                .map_err(|e| ProviderError::message(format!("invalid api key header: {e}")))?,
        );
        headers.insert(
            "anthropic-version",
            reqwest::header::HeaderValue::from_static(ANTHROPIC_VERSION),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let response = post_json_expect_success(&self.client, &url, headers, body).await?;
        let byte_stream = response.bytes_stream();
        let stream = anthropic_sse_to_events(byte_stream);
        Ok(Box::pin(stream))
    }
}

fn build_anthropic_body(req: &ChatRequest) -> Result<serde_json::Value, ProviderError> {
    #[derive(Serialize)]
    struct ToolWire<'a> {
        name: &'a str,
        description: &'a str,
        input_schema: &'a serde_json::Value,
        strict: bool,
    }

    let mut body = serde_json::Map::new();
    body.insert("model".into(), json!(req.model));
    body.insert("max_tokens".into(), json!(req.max_tokens));
    body.insert("stream".into(), json!(true));
    if let Some(ref s) = req.system {
        body.insert("system".into(), json!(s));
    }
    if !req.stop.is_empty() {
        body.insert("stop_sequences".into(), json!(&req.stop));
    }
    if let Some(t) = req.temperature {
        body.insert("temperature".into(), json!(t));
    }
    body.insert("messages".into(), json!(messages_to_anthropic(&req.messages)?));
    if !req.tools.is_empty() {
        let tools: Vec<ToolWire<'_>> = req
            .tools
            .iter()
            .map(|t| ToolWire {
                name: &t.name,
                description: &t.description,
                input_schema: &t.input_schema,
                strict: true,
            })
            .collect();
        body.insert("tools".into(), serde_json::to_value(&tools)?);
    }
    Ok(serde_json::Value::Object(body))
}

fn messages_to_anthropic(messages: &[Message]) -> Result<Vec<serde_json::Value>, ProviderError> {
    let mut out = Vec::new();
    for m in messages {
        match m {
            Message::User { content } => {
                out.push(json!({"role": "user", "content": content_blocks_user(content)?}));
            }
            Message::Assistant { content } => {
                out.push(
                    json!({"role": "assistant", "content": content_blocks_assistant(content)?}),
                );
            }
            Message::ToolResult { tool_call_id, content, is_error } => {
                let mut block =
                    json!({"type":"tool_result","tool_use_id": tool_call_id,"content": content});
                if *is_error {
                    block.as_object_mut().expect("object").insert("is_error".into(), json!(true));
                }
                out.push(json!({"role": "user", "content": vec![block]}));
            }
        }
    }
    Ok(out)
}

fn content_blocks_user(blocks: &[ContentBlock]) -> Result<serde_json::Value, ProviderError> {
    let mut v = Vec::new();
    for b in blocks {
        match b {
            ContentBlock::Text(s) => v.push(json!({"type":"text","text": s})),
            ContentBlock::ToolUse { .. } => {
                return Err(ProviderError::config(
                    "user messages must not contain ToolUse blocks in this API",
                ));
            }
        }
    }
    Ok(json!(v))
}

fn content_blocks_assistant(blocks: &[ContentBlock]) -> Result<serde_json::Value, ProviderError> {
    let mut v = Vec::new();
    for b in blocks {
        match b {
            ContentBlock::Text(s) => v.push(json!({"type":"text","text": s})),
            ContentBlock::ToolUse { id, name, input } => {
                v.push(json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input,
                }));
            }
        }
    }
    Ok(json!(v))
}

struct ToolAccum {
    id: String,
    name: String,
    input_json: String,
}

fn anthropic_sse_to_events<S>(
    byte_stream: S,
) -> impl Stream<Item = Result<ChatEvent, ProviderError>> + Send + 'static
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    async_stream::stream! {
        let mut payloads = crate::sse::iter_data_payloads(byte_stream);
        let mut tools: std::collections::HashMap<usize, ToolAccum> =
            std::collections::HashMap::new();
        let mut last_usage: Option<crate::types::Usage> = None;
        let mut last_stop: Option<crate::types::StopReason> = None;

        while let Some(payload) = payloads.next().await {
            let payload = match payload {
                Ok(p) => p,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };
            let v: serde_json::Value = match serde_json::from_str(&payload) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error = %e, "skipping malformed Anthropic SSE JSON");
                    continue;
                }
            };
                let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match ty {
                    "content_block_delta" => {
                        let index = v.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        let delta = match v.get("delta") {
                            Some(d) => d,
                            None => continue,
                        };
                        let dty = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if dty == "text_delta" {
                            if let Some(s) = delta.get("text").and_then(|t| t.as_str()) {
                                if !s.is_empty() {
                                    yield Ok(ChatEvent::TextDelta(s.to_owned()));
                                }
                            }
                        } else if dty == "input_json_delta" {
                            let partial = delta
                                .get("partial_json")
                                .and_then(|p| p.as_str())
                                .unwrap_or("");
                            let entry = tools.entry(index).or_insert(ToolAccum {
                                id: String::new(),
                                name: String::new(),
                                input_json: String::new(),
                            });
                            entry.input_json.push_str(partial);
                        }
                    }
                    "content_block_start" => {
                        let index = v.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        if let Some(block) = v.get("content_block") {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                let id = block
                                    .get("id")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("")
                                    .to_owned();
                                let name = block
                                    .get("name")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("")
                                    .to_owned();
                                tools.insert(
                                    index,
                                    ToolAccum {
                                        id,
                                        name,
                                        input_json: String::new(),
                                    },
                                );
                            }
                        }
                    }
                    "content_block_stop" => {
                        let index = v.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        if let Some(accum) = tools.remove(&index) {
                            if !accum.name.is_empty() {
                                let input: serde_json::Value = if accum.input_json.trim().is_empty() {
                                    json!({})
                                } else {
                                    match serde_json::from_str(&accum.input_json) {
                                        Ok(v) => v,
                                        Err(e) => {
                                            tracing::warn!(error = %e, "tool_use JSON parse; using null");
                                            serde_json::Value::Null
                                        }
                                    }
                                };
                                yield Ok(ChatEvent::ToolCall {
                                    id: accum.id,
                                    name: accum.name,
                                    input,
                                });
                            }
                        }
                    }
                    "message_delta" => {
                        if let Some(delta) = v.get("delta") {
                            if let Some(sr) = delta.get("stop_reason").and_then(|s| s.as_str()) {
                                last_stop = Some(StopReason::from_anthropic(sr));
                            }
                        }
                        if let Some(u) = v.get("usage") {
                            let input = u
                                .get("input_tokens")
                                .and_then(|x| x.as_u64())
                                .unwrap_or(0) as u32;
                            let output = u
                                .get("output_tokens")
                                .and_then(|x| x.as_u64())
                                .unwrap_or(0) as u32;
                            last_usage = Some(crate::types::Usage {
                                input_tokens: input,
                                output_tokens: output,
                            });
                        }
                    }
                    "message_stop" => {
                        let usage = last_usage.unwrap_or(crate::types::Usage::ZERO);
                        let stop = last_stop
                            .clone()
                            .unwrap_or(crate::types::StopReason::EndTurn);
                        yield Ok(ChatEvent::Done { usage, stop_reason: stop });
                        return;
                    }
                    "error" => {
                        let msg = v
                            .pointer("/error/message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Anthropic stream error");
                        yield Err(ProviderError::message(msg));
                        return;
                    }
                    _ => {}
                }
        }
        yield Err(ProviderError::incomplete_stream());
    }
}
