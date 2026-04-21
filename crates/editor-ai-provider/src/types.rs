//! Shared request, message, and stream event types.

use serde::{Deserialize, Serialize};

use crate::error::ProviderError;

/// Outgoing chat request (all providers map this to their wire format).
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub stop: Vec<String>,
    pub stream: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    User { content: Vec<ContentBlock> },
    Assistant { content: Vec<ContentBlock> },
    ToolResult { tool_call_id: String, content: String, is_error: bool },
}

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Events emitted by [`AiProvider::chat`](crate::provider::AiProvider::chat).
#[derive(Debug, Clone)]
pub enum ChatEvent {
    TextDelta(String),
    ToolCall { id: String, name: String, input: serde_json::Value },
    Done { usage: Usage, stop_reason: StopReason },
    Error(ProviderError),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl Usage {
    pub const ZERO: Self = Self { input_tokens: 0, output_tokens: 0 };

    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            input_tokens: self.input_tokens.saturating_add(other.input_tokens),
            output_tokens: self.output_tokens.saturating_add(other.output_tokens),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    Other(String),
}

impl StopReason {
    pub fn from_anthropic(s: &str) -> Self {
        match s {
            "end_turn" => StopReason::EndTurn,
            "max_tokens" => StopReason::MaxTokens,
            "stop_sequence" => StopReason::StopSequence,
            "tool_use" => StopReason::ToolUse,
            other => StopReason::Other(other.to_owned()),
        }
    }

    pub fn from_openai_finish(reason: Option<&str>, choice_reason: Option<&str>) -> Self {
        let s = choice_reason.or(reason).unwrap_or("");
        match s {
            "stop" => StopReason::EndTurn,
            "length" => StopReason::MaxTokens,
            "content_filter" => StopReason::Other("content_filter".into()),
            "tool_calls" => StopReason::ToolUse,
            _ if s.is_empty() => StopReason::EndTurn,
            other => StopReason::Other(other.to_owned()),
        }
    }
}

/// Lightweight catalog entry for [`AiProvider::available_models`](crate::provider::AiProvider::available_models).
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
}
