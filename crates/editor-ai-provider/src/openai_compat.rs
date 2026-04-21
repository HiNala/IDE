//! Shared OpenAI-compatible chat completions streaming (`POST .../v1/chat/completions`).

use std::pin::Pin;
use std::sync::Arc;

use futures::stream::Stream;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use url::Url;

use crate::error::ProviderError;
use crate::http::post_json_expect_success;
use crate::provider::ChatEventStream;
use crate::rate_limit::MinuteRateLimit;
use crate::types::{ChatEvent, ChatRequest, ContentBlock, Message};

/// Resolves `POST …/v1/chat/completions` whether `base` is `https://host`, `https://host/v1`, or already `…/v1/chat/completions`.
pub(crate) fn chat_completions_url_from_base(base: &Url) -> Result<String, ProviderError> {
    let s = base.as_str().trim_end_matches('/');
    let out = if s.ends_with("/v1/chat/completions") {
        s.to_string()
    } else if s.ends_with("/v1") {
        format!("{s}/chat/completions")
    } else {
        format!("{s}/v1/chat/completions")
    };
    Ok(out)
}

pub(crate) async fn chat_completions_stream(
    client: &reqwest::Client,
    url: &str,
    bearer_token: Option<&str>,
    req: ChatRequest,
    rate: Arc<MinuteRateLimit>,
) -> Result<ChatEventStream, ProviderError> {
    if !req.stream {
        return Err(ProviderError::message(
            "OpenAI-compatible providers support stream=true only in this crate",
        ));
    }
    rate.acquire().await;
    let body = build_chat_completions_body(&req)?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(t) = bearer_token {
        let v = format!("Bearer {t}");
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&v)
                .map_err(|e| ProviderError::message(format!("invalid Authorization: {e}")))?,
        );
    }
    let response = post_json_expect_success(client, url, headers, body).await?;
    let byte_stream = response.bytes_stream();
    Ok(Box::pin(openai_sse_to_events(byte_stream)))
}

fn build_chat_completions_body(req: &ChatRequest) -> Result<serde_json::Value, ProviderError> {
    let mut body = serde_json::Map::new();
    body.insert("model".into(), json!(req.model));
    body.insert("stream".into(), json!(true));
    body.insert("max_tokens".into(), json!(req.max_tokens));
    if let Some(t) = req.temperature {
        body.insert("temperature".into(), json!(t));
    }
    if !req.stop.is_empty() {
        body.insert("stop".into(), json!(&req.stop));
    }
    if let Some(s) = &req.system {
        // OpenAI-style: prepend as developer or system — use system message prefix in messages array.
        let mut msgs = vec![json!({"role": "system", "content": s})];
        msgs.extend(messages_to_openai(&req.messages)?);
        body.insert("messages".into(), json!(msgs));
    } else {
        body.insert("messages".into(), json!(messages_to_openai(&req.messages)?));
    }
    if !req.tools.is_empty() {
        let tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                        "strict": true,
                    }
                })
            })
            .collect();
        body.insert("tools".into(), json!(tools));
        body.insert("tool_choice".into(), json!("auto"));
    }
    Ok(serde_json::Value::Object(body))
}

fn messages_to_openai(messages: &[Message]) -> Result<Vec<serde_json::Value>, ProviderError> {
    let mut out = Vec::new();
    for m in messages {
        match m {
            Message::User { content } => {
                let parts = user_content_to_openai(content)?;
                out.push(json!({"role": "user", "content": parts}));
            }
            Message::Assistant { content } => {
                let (text, tool_calls) = assistant_content_to_openai(content)?;
                if tool_calls.is_empty() {
                    out.push(json!({"role": "assistant", "content": text}));
                } else {
                    let mut o = json!({"role": "assistant", "content": serde_json::Value::Null});
                    if let Some(obj) = o.as_object_mut() {
                        if !text.is_empty() {
                            obj.insert("content".into(), json!(text));
                        }
                        obj.insert("tool_calls".into(), json!(tool_calls));
                    }
                    out.push(o);
                }
            }
            Message::ToolResult { tool_call_id, content, .. } => {
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": content,
                }));
            }
        }
    }
    Ok(out)
}

fn user_content_to_openai(blocks: &[ContentBlock]) -> Result<serde_json::Value, ProviderError> {
    let mut parts = Vec::new();
    for b in blocks {
        match b {
            ContentBlock::Text(s) => parts.push(json!({"type": "text", "text": s})),
            ContentBlock::ToolUse { .. } => {
                return Err(ProviderError::config("user content cannot contain ToolUse"));
            }
        }
    }
    if parts.is_empty() {
        return Ok(json!(""));
    }
    if parts.len() == 1 {
        return Ok(parts[0]["text"].as_str().map(|s| json!(s)).unwrap_or(json!("")));
    }
    Ok(json!(parts))
}

fn assistant_content_to_openai(
    blocks: &[ContentBlock],
) -> Result<(String, Vec<serde_json::Value>), ProviderError> {
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for b in blocks {
        match b {
            ContentBlock::Text(s) => text.push_str(s),
            ContentBlock::ToolUse { id, name, input } => {
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(input)?,
                    }
                }));
            }
        }
    }
    Ok((text, tool_calls))
}

#[derive(Default, Clone)]
struct ToolAccum {
    id: String,
    name: String,
    arguments: String,
}

fn openai_sse_to_events(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
) -> impl Stream<Item = Result<ChatEvent, ProviderError>> + Send + 'static {
    let byte_stream: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>> =
        Box::pin(byte_stream);
    async_stream::stream! {
        let mut payloads = crate::sse::iter_data_payloads(byte_stream);
        let mut tools: std::collections::BTreeMap<u32, ToolAccum> =
            std::collections::BTreeMap::new();
        let mut last_usage: Option<crate::types::Usage> = None;
        let mut last_finish: Option<String> = None;

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
                    tracing::warn!(error = %e, "skipping malformed OpenAI SSE JSON");
                    continue;
                }
            };
                if let Some(u) = v.get("usage") {
                    let input = u.get("prompt_tokens").or_else(|| u.get("input_tokens"))
                        .and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                    let output = u.get("completion_tokens").or_else(|| u.get("output_tokens"))
                        .and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                    last_usage = Some(crate::types::Usage { input_tokens: input, output_tokens: output });
                }
                let choice = match v.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first()) {
                    Some(c) => c,
                    None => continue,
                };
                if let Some(fr) = choice.get("finish_reason").and_then(|s| s.as_str()) {
                    last_finish = Some(fr.to_owned());
                }
                let delta = match choice.get("delta") {
                    Some(d) => d,
                    None => continue,
                };
                if let Some(c) = delta.get("content").and_then(|x| x.as_str()) {
                    if !c.is_empty() {
                        yield Ok(ChatEvent::TextDelta(c.to_owned()));
                    }
                }
                if let Some(tc) = delta.get("tool_calls").and_then(|x| x.as_array()) {
                    for part in tc {
                        let idx = part.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
                        let entry = tools.entry(idx).or_default();
                        if let Some(id) = part.get("id").and_then(|s| s.as_str()) {
                            entry.id = id.to_owned();
                        }
                        if let Some(f) = part.get("function") {
                            if let Some(n) = f.get("name").and_then(|s| s.as_str()) {
                                entry.name = n.to_owned();
                            }
                            if let Some(a) = f.get("arguments").and_then(|s| s.as_str()) {
                                entry.arguments.push_str(a);
                            }
                        }
                    }
                }
        }
        if last_finish.as_deref() == Some("tool_calls") {
            for (_i, acc) in tools {
                if acc.name.is_empty() {
                    continue;
                }
                let input: serde_json::Value = if acc.arguments.trim().is_empty() {
                    json!({})
                } else {
                    match serde_json::from_str(&acc.arguments) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(error = %e, "tool arguments JSON");
                            serde_json::Value::Null
                        }
                    }
                };
                yield Ok(ChatEvent::ToolCall {
                    id: acc.id,
                    name: acc.name,
                    input,
                });
            }
        }
        let usage = last_usage.unwrap_or(crate::types::Usage::ZERO);
        let stop = crate::types::StopReason::from_openai_finish(None, last_finish.as_deref());
        yield Ok(ChatEvent::Done { usage, stop_reason: stop });
    }
}
