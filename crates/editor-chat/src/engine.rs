//! `ChatEngine` — drives AI streaming in a background tokio task.
//!
//! The engine owns a tokio runtime. The winit event loop calls
//! [`ChatEngine::submit`] synchronously; the engine spawns an async task
//! that calls the provider and sends [`EngineEvent`]s back via a
//! `crossbeam_channel::Sender`. The caller polls the receiver after each
//! winit event and redraws when events arrive.

use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use tracing::{debug, warn};

use editor_ai_provider::{
    AiProvider, ChatEvent as ProviderChatEvent, ChatRequest, ContentBlock, Message,
    ProviderRegistry,
};

use crate::conversation::MessageId;
use crate::error::{ChatError, Result};

/// Events flowing from the background AI task to the UI thread.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// A text delta arrived; append to the streaming message.
    TextDelta { session_id: u64, message_id: MessageId, delta: String },
    /// The stream finished.
    Done { session_id: u64, message_id: MessageId, stop_reason: Option<String>, tokens_out: u32 },
    /// An error terminated the stream.
    Error { session_id: u64, message_id: MessageId, message: String },
}

/// Configuration for [`ChatEngine`].
#[derive(Debug, Clone)]
pub struct ChatEngineConfig {
    /// Default model id to use when no session-level override is set.
    pub default_model: String,
    /// Maximum tokens to request per turn.
    pub max_tokens: u32,
    /// System prompt injected at every turn.
    pub system_prompt: String,
}

impl Default for ChatEngineConfig {
    fn default() -> Self {
        Self {
            default_model: "claude-opus-4-7".into(),
            max_tokens: 4096,
            system_prompt: "You are an expert coding assistant integrated into a GPU-rendered \
                            Rust IDE. Be concise, correct, and reference exact file paths and \
                            line numbers when discussing code."
                .into(),
        }
    }
}

/// Manages the tokio runtime and dispatches streaming AI requests.
pub struct ChatEngine {
    rt: Arc<tokio::runtime::Runtime>,
    registry: Option<Arc<ProviderRegistry>>,
    config: ChatEngineConfig,
    event_tx: Sender<EngineEvent>,
    event_rx: Receiver<EngineEvent>,
}

impl std::fmt::Debug for ChatEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatEngine").field("has_registry", &self.registry.is_some()).finish()
    }
}

impl ChatEngine {
    /// Create the engine. Call [`ChatEngine::set_registry`] before submitting any requests.
    #[must_use]
    pub fn new(config: ChatEngineConfig) -> Self {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("ide-chat")
            .build()
            .expect("failed to start chat tokio runtime");
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        Self { rt: Arc::new(rt), registry: None, config, event_tx, event_rx }
    }

    /// Install a provider registry (built from the user's settings/keyring).
    pub fn set_registry(&mut self, registry: ProviderRegistry) {
        self.registry = Some(Arc::new(registry));
    }

    /// True if a registry with at least one provider has been set.
    pub fn has_provider(&self) -> bool {
        self.registry.as_ref().map(|r| r.has_active()).unwrap_or(false)
    }

    /// Borrow the event receiver; the caller drains it each frame.
    pub fn events(&self) -> &Receiver<EngineEvent> {
        &self.event_rx
    }

    /// Submit a user prompt for streaming. Returns the assistant `MessageId` allocated
    /// for this turn so the caller can track which message to update.
    ///
    /// The engine will send [`EngineEvent`]s asynchronously; the caller should redraw
    /// when events arrive.
    pub fn submit(
        &self,
        session_id: u64,
        message_id: MessageId,
        history: Vec<(ChatRole, String)>,
        prompt: String,
    ) -> Result<()> {
        let Some(registry) = self.registry.clone() else {
            return Err(ChatError::NoProvider);
        };
        let provider = registry.active().ok_or(ChatError::NoProvider)?;
        let config = self.config.clone();
        let tx = self.event_tx.clone();

        self.rt.spawn(async move {
            run_stream(session_id, message_id, history, prompt, provider, config, tx).await;
        });
        Ok(())
    }
}

/// Role for history reconstruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

async fn run_stream(
    session_id: u64,
    message_id: MessageId,
    history: Vec<(ChatRole, String)>,
    prompt: String,
    provider: Arc<dyn AiProvider>,
    config: ChatEngineConfig,
    tx: Sender<EngineEvent>,
) {
    // Build messages from history + current prompt.
    let mut messages: Vec<Message> = history
        .into_iter()
        .map(|(role, text)| match role {
            ChatRole::User => Message::User { content: vec![ContentBlock::Text(text)] },
            ChatRole::Assistant => Message::Assistant { content: vec![ContentBlock::Text(text)] },
        })
        .collect();
    messages.push(Message::User { content: vec![ContentBlock::Text(prompt)] });

    let request = ChatRequest {
        model: config.default_model,
        system: Some(config.system_prompt),
        messages,
        tools: vec![],
        max_tokens: config.max_tokens,
        temperature: None,
        stop: vec![],
        stream: true,
    };

    let stream = match provider.chat(request).await {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(EngineEvent::Error { session_id, message_id, message: e.to_string() });
            return;
        }
    };

    let mut stream = Box::pin(stream);
    let mut tokens_out = 0u32;
    let mut stop_reason: Option<String> = None;

    while let Some(item) = stream.next().await {
        let event = match item {
            Ok(e) => e,
            Err(e) => {
                warn!(session_id, error = %e, "stream error");
                let _ =
                    tx.send(EngineEvent::Error { session_id, message_id, message: e.to_string() });
                return;
            }
        };
        match event {
            ProviderChatEvent::TextDelta(delta) => {
                debug!(session_id, delta = %delta, "stream delta");
                let _ = tx.send(EngineEvent::TextDelta { session_id, message_id, delta });
            }
            ProviderChatEvent::Done { usage, stop_reason: sr } => {
                tokens_out = usage.output_tokens;
                stop_reason = Some(format!("{:?}", sr));
                break;
            }
            ProviderChatEvent::Error(e) => {
                warn!(session_id, error = %e, "stream error");
                let _ =
                    tx.send(EngineEvent::Error { session_id, message_id, message: e.to_string() });
                return;
            }
            ProviderChatEvent::ToolCall { .. } => {
                // Tool calls are not yet executed — just skip for now.
            }
        }
    }

    let _ = tx.send(EngineEvent::Done { session_id, message_id, stop_reason, tokens_out });
}
