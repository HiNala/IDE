//! Conversation data model: messages, roles, streaming state.

use std::fmt;

/// Stable handle for a message within a [`Conversation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(pub u64);

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "msg-{}", self.0)
    }
}

/// Who authored the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    /// Tool invocation (call request + result) displayed inline in the chat log.
    Tool { call_id: u64 },
    /// Non-fatal inline note (e.g. "no provider configured").
    Note,
}

/// A single turn in the conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: MessageId,
    pub role: ChatRole,
    /// Accumulated text content. For assistant messages this grows as tokens stream in.
    pub text: String,
    /// True while tokens are still streaming for this message.
    pub is_streaming: bool,
    /// Non-empty if the provider returned a stop reason.
    pub stop_reason: Option<String>,
    /// Approximate token count (input=0 for user messages).
    pub tokens_out: u32,
}

impl ChatMessage {
    pub fn user(id: MessageId, text: impl Into<String>) -> Self {
        Self {
            id,
            role: ChatRole::User,
            text: text.into(),
            is_streaming: false,
            stop_reason: None,
            tokens_out: 0,
        }
    }

    pub fn assistant_streaming(id: MessageId) -> Self {
        Self {
            id,
            role: ChatRole::Assistant,
            text: String::new(),
            is_streaming: true,
            stop_reason: None,
            tokens_out: 0,
        }
    }
}

/// Ordered list of messages for one agent session.
#[derive(Debug, Clone, Default)]
pub struct Conversation {
    messages: Vec<ChatMessage>,
    next_id: u64,
}

impl Conversation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a user message, returning its id.
    pub fn push_user(&mut self, text: impl Into<String>) -> MessageId {
        let id = self.alloc_id();
        self.messages.push(ChatMessage::user(id, text));
        id
    }

    /// Push an empty streaming assistant message, returning its id.
    pub fn push_assistant_streaming(&mut self) -> MessageId {
        let id = self.alloc_id();
        self.messages.push(ChatMessage::assistant_streaming(id));
        id
    }

    /// Append a text delta to an existing message.
    pub fn append_text(&mut self, id: MessageId, delta: &str) {
        if let Some(m) = self.messages.iter_mut().find(|m| m.id == id) {
            m.text.push_str(delta);
        }
    }

    /// Mark a message as finished streaming.
    pub fn finish_streaming(
        &mut self,
        id: MessageId,
        stop_reason: Option<String>,
        tokens_out: u32,
    ) {
        if let Some(m) = self.messages.iter_mut().find(|m| m.id == id) {
            m.is_streaming = false;
            m.stop_reason = stop_reason;
            m.tokens_out = tokens_out;
        }
    }

    /// All messages in order.
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Most recent N messages (for display).
    pub fn last_n(&self, n: usize) -> &[ChatMessage] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }

    /// Push a completed tool-call line (call name + abbreviated args).
    pub fn push_tool_note(&mut self, text: impl Into<String>) -> MessageId {
        let id = self.alloc_id();
        self.messages.push(ChatMessage {
            id,
            role: ChatRole::Tool { call_id: id.0 },
            text: text.into(),
            is_streaming: false,
            stop_reason: None,
            tokens_out: 0,
        });
        id
    }

    /// Push a short inline note (warning / error surface to the user).
    pub fn push_note(&mut self, text: impl Into<String>) -> MessageId {
        let id = self.alloc_id();
        self.messages.push(ChatMessage {
            id,
            role: ChatRole::Note,
            text: text.into(),
            is_streaming: false,
            stop_reason: None,
            tokens_out: 0,
        });
        id
    }

    /// Total messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn alloc_id(&mut self) -> MessageId {
        let id = MessageId(self.next_id);
        self.next_id += 1;
        id
    }
}
