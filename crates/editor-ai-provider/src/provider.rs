//! [`AiProvider`] trait — uniform surface for Claude, OpenAI, Ollama, and OpenAI-compatible HTTP APIs.

use std::pin::Pin;

use futures::Stream;

use crate::error::ProviderError;
use crate::types::{ChatEvent, ChatRequest, ModelInfo};

/// Boxed asynchronous stream of [`ChatEvent`] results.
pub type ChatEventStream =
    Pin<Box<dyn Stream<Item = Result<ChatEvent, ProviderError>> + Send + 'static>>;

#[async_trait::async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;

    fn available_models(&self) -> Vec<ModelInfo>;

    async fn chat(&self, req: ChatRequest) -> Result<ChatEventStream, ProviderError>;
}
