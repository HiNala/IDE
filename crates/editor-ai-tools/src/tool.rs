//! [`Tool`] trait — uniform async surface for LLM tool calls.

use crate::error::{Result, ToolError};
use crate::transaction::WorkspaceTx;
use async_trait::async_trait;

/// Human-readable + optional JSON payload from a tool invocation.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: String,
    pub structured: Option<serde_json::Value>,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn text(s: impl Into<String>) -> Self {
        Self { content: s.into(), structured: None, is_error: false }
    }

    pub fn error(s: impl Into<String>) -> Self {
        Self { content: s.into(), structured: None, is_error: true }
    }

    pub fn json(content: String, value: serde_json::Value) -> Self {
        Self { content, structured: Some(value), is_error: false }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;

    async fn invoke(
        &self,
        input: serde_json::Value,
        tx: &mut WorkspaceTx,
        dry_run: bool,
    ) -> Result<ToolOutput, ToolError>;
}

pub(crate) fn parse_input<T: serde::de::DeserializeOwned>(
    v: serde_json::Value,
) -> Result<T, ToolError> {
    serde_json::from_value(v).map_err(ToolError::Serde)
}

pub(crate) fn schema_value<T: schemars::JsonSchema>() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(T))
        .unwrap_or_else(|_| serde_json::json!({"type": "object"}))
}
