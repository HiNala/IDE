//! `ChatEngine` — drives AI streaming in a background tokio task.
//!
//! The engine owns a tokio runtime. The winit event loop calls
//! [`ChatEngine::submit`] synchronously; the engine spawns an async task
//! that calls the provider and sends [`EngineEvent`]s back via a
//! `crossbeam_channel::Sender`. The caller polls the receiver after each
//! winit event and redraws when events arrive.
//!
//! Tool execution loop:
//!   - When the model requests a tool call, a `ToolCall` event is sent with
//!     a `result_tx: oneshot::Sender<(String, bool)>`.
//!   - The caller must execute the tool and send `(result_json, is_error)`.
//!   - The engine then re-submits with the tool result and continues streaming.
//!   - Up to [`MAX_TOOL_ROUNDS`] rounds per turn.

use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use tracing::{debug, warn};

use editor_ai_provider::{
    AiProvider, ChatEvent as ProviderChatEvent, ChatRequest, ContentBlock, Message,
    ProviderRegistry, ToolDef,
};

use crate::conversation::{ChatRole, MessageId};
use crate::error::{ChatError, Result};

/// Maximum tool-call rounds per conversation turn before the stream is aborted.
const MAX_TOOL_ROUNDS: usize = 15;

/// Events flowing from the background AI task to the UI thread.
#[derive(Debug)]
pub enum EngineEvent {
    /// A text delta arrived; append to the streaming message.
    TextDelta { session_id: u64, message_id: MessageId, delta: String },
    /// The stream finished cleanly.
    Done { session_id: u64, message_id: MessageId, stop_reason: Option<String>, tokens_out: u32 },
    /// An error terminated the stream.
    Error { session_id: u64, message_id: MessageId, message: String },
    /// The model requested a tool call.
    ///
    /// The receiver **must** send `(result_content, is_error)` back via `result_tx`
    /// to unblock the engine and continue the conversation.  If the channel is
    /// dropped the engine reports an error for that session.
    ToolCall {
        session_id: u64,
        call_id: String,
        name: String,
        input_json: String,
        /// Channel for the tool result.  Send `(result_text, is_error)`.
        result_tx: tokio::sync::oneshot::Sender<(String, bool)>,
    },
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
    /// Tool schemas exposed to the model.  Empty = no tools.
    pub tools: Vec<ToolDef>,
}

impl Default for ChatEngineConfig {
    fn default() -> Self {
        Self {
            default_model: "claude-opus-4-7".into(),
            max_tokens: 8192,
            system_prompt: "You are an expert coding assistant integrated into a GPU-rendered \
                            Rust IDE called Antigravity. You have access to file-system tools \
                            to read, search, and edit code. Be concise and precise. Reference \
                            exact file paths and line numbers. When editing files prefer \
                            edit_lines over full rewrites."
                .into(),
            tools: default_tool_defs(),
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

    /// Replace the tool schemas exposed to the model (called on workspace open).
    pub fn set_tools(&mut self, tools: Vec<ToolDef>) {
        self.config.tools = tools;
    }

    /// Update the system prompt (e.g. after loading a workspace's skill registry).
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.config.system_prompt = prompt;
    }

    /// Read-only view of the current engine config (e.g. to read system_prompt).
    pub fn config(&self) -> &ChatEngineConfig {
        &self.config
    }

    /// Borrow the event receiver; the caller drains it each frame.
    pub fn events(&self) -> &Receiver<EngineEvent> {
        &self.event_rx
    }

    /// Submit a user prompt for streaming.
    ///
    /// The caller passes conversation history as `(role, text)` pairs excluding
    /// the current user turn.  Tool / Note messages are skipped on the wire.
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

        // Build initial messages from history.
        let mut messages: Vec<Message> = history
            .into_iter()
            .filter_map(|(role, text)| match role {
                ChatRole::User => Some(Message::User { content: vec![ContentBlock::Text(text)] }),
                ChatRole::Assistant => {
                    Some(Message::Assistant { content: vec![ContentBlock::Text(text)] })
                }
                ChatRole::Tool { .. } | ChatRole::Note => None,
            })
            .collect();
        messages.push(Message::User { content: vec![ContentBlock::Text(prompt)] });

        self.rt.spawn(async move {
            run_stream(session_id, message_id, messages, provider, config, tx).await;
        });
        Ok(())
    }
}

async fn run_stream(
    session_id: u64,
    message_id: MessageId,
    mut messages: Vec<Message>,
    provider: Arc<dyn AiProvider>,
    config: ChatEngineConfig,
    tx: Sender<EngineEvent>,
) {
    let mut total_tokens_out = 0u32;

    for _round in 0..MAX_TOOL_ROUNDS {
        let request = ChatRequest {
            model: config.default_model.clone(),
            system: Some(config.system_prompt.clone()),
            messages: messages.clone(),
            tools: config.tools.clone(),
            max_tokens: config.max_tokens,
            temperature: None,
            stop: vec![],
            stream: true,
        };

        let stream = match provider.chat(request).await {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(EngineEvent::Error {
                    session_id,
                    message_id,
                    message: e.to_string(),
                });
                return;
            }
        };

        let mut stream = Box::pin(stream);
        let mut assistant_text = String::new();
        // Collected tool calls for this streaming turn.
        let mut tool_calls: Vec<(String, String, serde_json::Value)> = Vec::new();
        let mut stop_reason: Option<String> = None;

        while let Some(item) = stream.next().await {
            let event = match item {
                Ok(e) => e,
                Err(e) => {
                    warn!(session_id, error = %e, "stream error");
                    let _ = tx.send(EngineEvent::Error {
                        session_id,
                        message_id,
                        message: e.to_string(),
                    });
                    return;
                }
            };
            match event {
                ProviderChatEvent::TextDelta(delta) => {
                    debug!(session_id, len = delta.len(), "text delta");
                    assistant_text.push_str(&delta);
                    let _ =
                        tx.send(EngineEvent::TextDelta { session_id, message_id, delta });
                }
                ProviderChatEvent::Done { usage, stop_reason: sr } => {
                    total_tokens_out =
                        total_tokens_out.saturating_add(usage.output_tokens);
                    stop_reason = Some(format!("{sr:?}"));
                    break;
                }
                ProviderChatEvent::Error(e) => {
                    warn!(session_id, error = %e, "stream error event");
                    let _ = tx.send(EngineEvent::Error {
                        session_id,
                        message_id,
                        message: e.to_string(),
                    });
                    return;
                }
                ProviderChatEvent::ToolCall { id, name, input } => {
                    tool_calls.push((id, name, input));
                }
            }
        }

        if tool_calls.is_empty() {
            // Clean finish — no tools called this round.
            let _ = tx.send(EngineEvent::Done {
                session_id,
                message_id,
                stop_reason,
                tokens_out: total_tokens_out,
            });
            return;
        }

        // Build the assistant message: accumulated text + tool_use blocks.
        let mut assistant_content: Vec<ContentBlock> = Vec::new();
        if !assistant_text.is_empty() {
            assistant_content.push(ContentBlock::Text(assistant_text));
        }
        for (id, name, input) in &tool_calls {
            assistant_content.push(ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            });
        }
        messages.push(Message::Assistant { content: assistant_content });

        // Execute each tool: emit event, await result via oneshot channel.
        for (call_id, name, input_val) in tool_calls {
            let input_json = input_val.to_string();
            let (result_tx, result_rx) = tokio::sync::oneshot::channel::<(String, bool)>();
            let _ = tx.send(EngineEvent::ToolCall {
                session_id,
                call_id: call_id.clone(),
                name,
                input_json,
                result_tx,
            });
            let (content, is_error) = match tokio::time::timeout(
                Duration::from_secs(60),
                result_rx,
            )
            .await
            {
                Ok(Ok(r)) => r,
                Ok(Err(_)) => ("Tool result channel closed.".to_string(), true),
                Err(_) => {
                    ("Tool execution timed out after 60 seconds.".to_string(), true)
                }
            };
            messages.push(Message::ToolResult {
                tool_call_id: call_id,
                content,
                is_error,
            });
        }
        // Loop: next API call carries the tool results.
    }

    // Exceeded MAX_TOOL_ROUNDS.
    let _ = tx.send(EngineEvent::Error {
        session_id,
        message_id,
        message: format!("Exceeded {MAX_TOOL_ROUNDS} tool-call rounds."),
    });
}

// ── Built-in tool schemas ──────────────────────────────────────────────────────

/// Returns the default set of tool schemas that the engine exposes to the model.
fn default_tool_defs() -> Vec<ToolDef> {
    use serde_json::json;
    vec![
        ToolDef {
            name: "read_file".into(),
            description: "Read the complete text contents of a file.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative or absolute path." }
                }
            }),
        },
        ToolDef {
            name: "list_directory".into(),
            description: "List the direct children of a directory (files and sub-directories).".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Directory path (workspace-relative)." },
                    "recursive": { "type": "boolean", "description": "If true, walk all sub-directories." }
                }
            }),
        },
        ToolDef {
            name: "find_files".into(),
            description: "Find files matching a glob pattern under a directory.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern, e.g. '**/*.rs'." },
                    "path": { "type": "string", "description": "Search root (default: workspace root)." }
                }
            }),
        },
        ToolDef {
            name: "grep".into(),
            description: "Search for a regex pattern across files in a directory tree.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "pattern": { "type": "string", "description": "Regular expression to search for." },
                    "path": { "type": "string", "description": "Root path to search (default: workspace)." },
                    "file_pattern": { "type": "string", "description": "Only search files matching this glob, e.g. '*.rs'." },
                    "context_lines": { "type": "integer", "description": "Lines of context around each match (default 2)." }
                }
            }),
        },
        ToolDef {
            name: "edit_lines".into(),
            description: "Replace a range of lines in a file.  Lines are 1-indexed.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "start_line", "end_line", "new_content"],
                "properties": {
                    "path": { "type": "string" },
                    "start_line": { "type": "integer", "description": "First line to replace (1-indexed, inclusive)." },
                    "end_line": { "type": "integer", "description": "Last line to replace (1-indexed, inclusive)." },
                    "new_content": { "type": "string", "description": "Replacement text (may include newlines)." }
                }
            }),
        },
        ToolDef {
            name: "insert_at".into(),
            description: "Insert text before a given 1-indexed line number.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "line_number", "content"],
                "properties": {
                    "path": { "type": "string" },
                    "line_number": { "type": "integer", "description": "Insert before this line (1-indexed)." },
                    "content": { "type": "string" }
                }
            }),
        },
        ToolDef {
            name: "append_to".into(),
            description: "Append text to the end of a file.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "content"],
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                }
            }),
        },
        ToolDef {
            name: "create_file".into(),
            description: "Create a new file with the given content.  Fails if the file already exists.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "content"],
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                }
            }),
        },
        ToolDef {
            name: "delete_file".into(),
            description: "Permanently delete a file from the workspace.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string" }
                }
            }),
        },
        ToolDef {
            name: "move_file".into(),
            description: "Move or rename a file within the workspace.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["from", "to"],
                "properties": {
                    "from": { "type": "string" },
                    "to": { "type": "string" }
                }
            }),
        },
        ToolDef {
            name: "replace_in_file".into(),
            description: "Replace a literal string (or all occurrences) inside a file. Use edit_lines for line-range changes; use this for precise text substitutions.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "old_text", "new_text"],
                "properties": {
                    "path": { "type": "string" },
                    "old_text": { "type": "string", "description": "Exact text to find." },
                    "new_text": { "type": "string", "description": "Replacement text." },
                    "occurrence": { "type": "integer", "description": "1-based occurrence index to replace. 0 = replace all (default 1)." }
                }
            }),
        },
        ToolDef {
            name: "run_shell".into(),
            description: "Run a shell command in the workspace. Requires shell to be enabled in .ide/tools.toml. Use for npm/cargo/git/npx commands. Always prefer specific tools (read_file, edit_lines) over shell when possible.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": { "type": "string", "description": "The shell command to run. Example: 'npx create-next-app@latest my-app --ts --no-git'." },
                    "cwd": { "type": "string", "description": "Working directory (workspace-relative). Default: workspace root." },
                    "timeout_seconds": { "type": "integer", "description": "Max seconds to wait (default 60, max 300)." }
                }
            }),
        },
        ToolDef {
            name: "read_metadata".into(),
            description: "Read the .ide/meta/ sidecar for a file — contains prior AI reasoning, history, and notes.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative source file path." }
                }
            }),
        },
        ToolDef {
            name: "write_metadata_note".into(),
            description: "Append a note to the .ide/meta/ sidecar for a file. Use this to record reasoning, design decisions, or important findings about the file.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "note"],
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative source file path." },
                    "note": { "type": "string", "description": "Note text to append." }
                }
            }),
        },
        ToolDef {
            name: "list_tasks".into(),
            description: "List all project tasks from .ide/tasks.md.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolDef {
            name: "add_task".into(),
            description: "Add a new task to .ide/tasks.md.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["summary"],
                "properties": {
                    "summary": { "type": "string" },
                    "notes": { "type": "string" }
                }
            }),
        },
        ToolDef {
            name: "complete_task".into(),
            description: "Mark a task as done in .ide/tasks.md.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string", "description": "Task id from list_tasks." }
                }
            }),
        },
        ToolDef {
            name: "update_task".into(),
            description: "Update a task's status or notes in .ide/tasks.md.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "status": { "type": "string", "enum": ["open", "in_progress", "done", "cancelled"] },
                    "notes": { "type": "string" }
                }
            }),
        },
        ToolDef {
            name: "load_skill".into(),
            description: "Load the full body of a named skill (IDE convention or language guide). Call list_skills first if unsure of the name.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" }
                }
            }),
        },
        ToolDef {
            name: "list_skills".into(),
            description: "List all available skills with their names and descriptions.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}
