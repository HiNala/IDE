//! Chat engine integration for the main event loop (M23).
//!
//! All AI-specific App methods live here so main.rs stays focused on the
//! frame loop and input routing.
//!
//! Sub-modules:
//!  `tools/` — one file per tool category: fs, edit, shell, metadata, tasks, skills.
//!
//! Phases implemented:
//!  Phase 1 — streaming text (TextDelta / Done / Error events)
//!  Phase 2 — tool execution loop (ToolCall → execute → resume stream)
//!  Phase 6 — skills system (SkillRegistry augments system prompt on each submit)
//!  Phase 7 — metadata sidecar (history entry written on session Done)

pub(crate) mod tools;

use std::path::PathBuf;
use std::sync::Arc;

use editor_chat::{ChatRole, Conversation, EngineEvent};
use editor_metadata::{blank_sidecar, HistoryEntry};
use editor_ui::AgentSessionStatus;
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::App;

// Re-export the public dispatch function so main.rs / tests can call it.
pub(crate) use tools::execute_tool;

/// Tools that permanently alter or destroy files / execute arbitrary code.
/// These are blocked unless `auto_approve = true` in `.ide/tools.toml`.
const DESTRUCTIVE_TOOLS: &[&str] = &["delete_file", "move_file", "run_shell"];

fn is_destructive_tool(name: &str) -> bool {
    DESTRUCTIVE_TOOLS.contains(&name)
}

fn auto_approve_enabled(workspace_root: &std::path::Path) -> bool {
    editor_ai_tools::ToolConfig::load_from_workspace_root(workspace_root)
        .map(|c| c.auto_approve)
        .unwrap_or(false)
}

impl App {
    // ── Event polling ─────────────────────────────────────────────────────────

    /// Drain all pending [`EngineEvent`]s and update conversation state.
    /// Returns `true` when at least one event was received (redraw needed).
    pub(crate) fn poll_chat_events(&mut self) -> bool {
        let events: Vec<EngineEvent> = self.chat_engine.events().try_iter().collect();
        if events.is_empty() {
            return false;
        }

        let workspace_root: PathBuf = self
            .workspace
            .as_ref()
            .map(|w| w.root().to_path_buf())
            .or_else(|| self.open_path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let skill_arc = Arc::clone(&self.skill_registry);
        let metadata_store = self.metadata_store.clone();

        for ev in events {
            match ev {
                EngineEvent::TextDelta { session_id, message_id, delta } => {
                    self.chat_last_delta_at = Some(std::time::Instant::now());
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.append_text(message_id, &delta);
                    }
                }

                EngineEvent::Done { session_id, message_id, stop_reason, tokens_out } => {
                    self.chat_last_delta_at = None;
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.finish_streaming(message_id, stop_reason, tokens_out);
                    }
                    self.set_session_status(session_id, AgentSessionStatus::Done);
                    // Phase 7: persist AI reasoning to sidecar.
                    if let Some(ref store) = metadata_store {
                        self.write_session_history_entry(session_id, store);
                    }
                }

                EngineEvent::Error { session_id, message_id, message } => {
                    self.chat_last_delta_at = None;
                    tracing::warn!(session_id, %message, "AI stream error");
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.append_text(message_id, &format!("\n⚠ {message}"));
                        conv.finish_streaming(message_id, Some("error".into()), 0);
                    }
                    self.set_session_status(session_id, AgentSessionStatus::Done);
                }

                EngineEvent::Cancelled { session_id, message_id } => {
                    self.chat_last_delta_at = None;
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.append_text(message_id, "\n[cancelled]");
                        conv.finish_streaming(message_id, Some("cancelled".into()), 0);
                    }
                    self.set_session_status(session_id, AgentSessionStatus::Done);
                }

                EngineEvent::ToolCall { session_id, call_id, name, input_json, result_tx } => {
                    tracing::debug!(session_id, call_id, %name, "executing tool call");

                    // Safety gate: block destructive tools unless auto_approve is enabled.
                    if is_destructive_tool(&name) && !auto_approve_enabled(&workspace_root) {
                        let preview = if input_json.len() > 120 {
                            format!("{}…", &input_json[..120])
                        } else {
                            input_json.clone()
                        };
                        if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                            conv.push_tool_note(format!(
                                "🔒 Blocked destructive tool `{name}`: {preview}\n   \
                                 To allow shell/delete/move tools, set `auto_approve = true` \
                                 in .ide/tools.toml."
                            ));
                        }
                        let _ = result_tx.send((
                            format!(
                                "Tool '{name}' requires user confirmation. \
                                 Set auto_approve = true in .ide/tools.toml to enable it."
                            ),
                            true,
                        ));
                        continue;
                    }

                    let preview = if input_json.len() > 80 {
                        format!("{}…", &input_json[..80])
                    } else {
                        input_json.clone()
                    };
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.push_tool_note(format!("▶ {name}({preview})"));
                    }

                    let skill_guard = skill_arc.read().expect("skill lock poisoned");
                    let (result_content, is_error) = execute_tool(
                        &name,
                        &input_json,
                        &workspace_root,
                        Some(&*skill_guard),
                        metadata_store.as_ref(),
                    );

                    let result_preview = if result_content.len() > 160 {
                        format!("{}…", &result_content[..160])
                    } else {
                        result_content.clone()
                    };
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        let prefix = if is_error { "✗" } else { "✔" };
                        conv.push_tool_note(format!("{prefix} {result_preview}"));
                    }
                    let _ = result_tx.send((result_content, is_error));
                }
            }
        }

        true
    }

    // ── Submission ────────────────────────────────────────────────────────────

    /// Submit `self.chat_input` to the active session's AI stream.
    pub(crate) fn submit_chat_input(&mut self) {
        let prompt = self.chat_input.trim().to_string();
        if prompt.is_empty() {
            return;
        }

        let session_id = match self.agent_panel.sessions.get(self.agent_panel.active_session) {
            Some(s) => s.id,
            None => return,
        };

        if !self.chat_engine.has_provider() {
            let conv = self.chat_conversations.entry(session_id).or_insert_with(Conversation::new);
            conv.push_user(prompt.clone());
            conv.push_note(
                "⚠ No AI provider configured. Set ANTHROPIC_API_KEY in your environment, \
                 then restart — or open Settings (Ctrl+,) to configure a key.",
            );
            self.clear_chat_input();
            self.agent_view_active = true;
            return;
        }

        // Sync engine config from current settings (model, max_tokens, temperature).
        {
            let s = self.settings_store.settings();
            let model = s.ai.active_model.clone()
                .or_else(|| {
                    s.ai.active_provider.as_deref()
                        .and_then(|p| s.ai.providers.get(p))
                        .map(|pc| pc.default_model.clone())
                });
            if let Some(m) = model {
                if !m.is_empty() {
                    self.chat_engine.set_model(m);
                }
            }
            self.chat_engine.set_max_tokens(s.ai.max_tokens_default);
            self.chat_engine.set_temperature(s.ai.temperature_default);
        }

        // Phase 6: augment system prompt with loaded skill context.
        {
            let base = self.chat_engine.config().system_prompt.clone();
            let skill_guard = self.skill_registry.read().expect("skill lock");
            if !skill_guard.list().is_empty() {
                let augmented = skill_guard.augment_system_prompt(&base);
                drop(skill_guard);
                self.chat_engine.set_system_prompt(augmented);
            }
        }

        // Build history with a sliding window: keep the most recent exchanges to
        // avoid overflowing the model's context window (~140 k token budget).
        // We estimate ~4 chars/token and cap at 100k chars of history (~25k tokens).
        const HISTORY_CHAR_BUDGET: usize = 100_000;
        let mut history: Vec<(ChatRole, String)> = self
            .chat_conversations
            .get(&session_id)
            .map(|c| {
                c.messages()
                    .iter()
                    .filter_map(|m| match m.role {
                        ChatRole::User => Some((ChatRole::User, m.text.clone())),
                        ChatRole::Assistant => Some((ChatRole::Assistant, m.text.clone())),
                        ChatRole::Tool { .. } | ChatRole::Note => None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Drop oldest messages from the front until under budget.
        let total_chars: usize = history.iter().map(|(_, t)| t.len()).sum();
        if total_chars > HISTORY_CHAR_BUDGET {
            let mut running = total_chars;
            let mut drop_until = 0;
            for (i, (_, text)) in history.iter().enumerate() {
                if running <= HISTORY_CHAR_BUDGET { break; }
                running -= text.len();
                drop_until = i + 1;
            }
            if drop_until > 0 {
                history.drain(..drop_until);
                if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                    conv.push_note(
                        "ℹ Context window: oldest messages dropped to stay under the token limit.",
                    );
                }
            }
        }

        // Estimate token usage for the current context (4 chars ≈ 1 token).
        let prompt_chars: usize = prompt.len()
            + history.iter().map(|(_, t)| t.len()).sum::<usize>()
            + self.chat_engine.config().system_prompt.len();
        let est_tokens = prompt_chars / 4;
        tracing::debug!(est_tokens, "pre-flight token estimate");

        let conv = self.chat_conversations.entry(session_id).or_insert_with(Conversation::new);
        conv.push_user(prompt.clone());
        // Surface a note if the context is getting large (>80k estimated tokens).
        if est_tokens > 80_000 {
            conv.push_note(&format!(
                "⚠ Estimated context: ~{est_tokens} tokens — approaching model limit. \
                 Oldest history has been trimmed."
            ));
        }
        let msg_id = conv.push_assistant_streaming();

        self.set_session_status(session_id, AgentSessionStatus::Running);
        if let Some(s) = self.agent_panel.sessions.get_mut(self.agent_panel.active_session) {
            if s.label == "New Chat" {
                s.label = prompt.chars().take(22).collect();
            }
        }

        match self.chat_engine.submit(session_id, msg_id, history, prompt) {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(error = %e, "chat submit failed");
                if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                    conv.append_text(msg_id, &format!("⚠ {e}"));
                    conv.finish_streaming(msg_id, Some("error".into()), 0);
                }
                self.set_session_status(session_id, AgentSessionStatus::Done);
            }
        }

        self.clear_chat_input();
        self.agent_view_active = true;
    }

    // ── Keyboard input ────────────────────────────────────────────────────────

    /// Handle a key press while the agent panel textarea has focus.
    /// Returns `true` when the event was consumed.
    pub(crate) fn handle_agent_panel_key(&mut self, event: &KeyEvent) -> bool {
        if !self.agent_panel_focused {
            return false;
        }
        let PhysicalKey::Code(code) = event.physical_key else { return false };
        let ctrl = self.modifiers.control_key() || self.modifiers.super_key();

        match code {
            KeyCode::Escape => {
                // If a stream is active, cancel it; otherwise unfocus the panel.
                if self.chat_engine.is_cancelling() || {
                    let running = self.agent_panel.sessions.iter().any(|s| {
                        s.status == AgentSessionStatus::Running
                    });
                    running
                } {
                    self.chat_engine.cancel_stream();
                } else {
                    self.agent_panel_focused = false;
                }
                return true;
            }
            KeyCode::Enter => {
                if ctrl {
                    self.submit_chat_input();
                } else {
                    self.chat_input.insert(self.chat_input_cursor, '\n');
                    self.chat_input_cursor += 1;
                }
                return true;
            }
            KeyCode::Backspace => {
                if self.chat_input_cursor > 0 {
                    let mut new_pos = self.chat_input_cursor - 1;
                    while new_pos > 0 && !self.chat_input.is_char_boundary(new_pos) {
                        new_pos -= 1;
                    }
                    self.chat_input.remove(new_pos);
                    self.chat_input_cursor = new_pos;
                }
                return true;
            }
            KeyCode::Delete => {
                let len = self.chat_input.len();
                if self.chat_input_cursor < len {
                    let mut end = self.chat_input_cursor + 1;
                    while end < len && !self.chat_input.is_char_boundary(end) {
                        end += 1;
                    }
                    self.chat_input.drain(self.chat_input_cursor..end);
                }
                return true;
            }
            KeyCode::ArrowLeft => {
                if self.chat_input_cursor > 0 {
                    let mut p = self.chat_input_cursor - 1;
                    while p > 0 && !self.chat_input.is_char_boundary(p) {
                        p -= 1;
                    }
                    self.chat_input_cursor = p;
                }
                return true;
            }
            KeyCode::ArrowRight => {
                let len = self.chat_input.len();
                if self.chat_input_cursor < len {
                    let mut p = self.chat_input_cursor + 1;
                    while p < len && !self.chat_input.is_char_boundary(p) {
                        p += 1;
                    }
                    self.chat_input_cursor = p;
                }
                return true;
            }
            KeyCode::Home => {
                let before = &self.chat_input[..self.chat_input_cursor];
                self.chat_input_cursor = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
                return true;
            }
            KeyCode::End => {
                let after = self.chat_input[self.chat_input_cursor..].to_owned();
                self.chat_input_cursor += after.find('\n').unwrap_or(after.len());
                return true;
            }
            _ => {}
        }

        if !ctrl {
            if let Some(t) = event.text.as_ref() {
                if !t.is_empty() && t.chars().all(|c| !c.is_control()) {
                    self.chat_input.insert_str(self.chat_input_cursor, t);
                    self.chat_input_cursor += t.len();
                    return true;
                }
            }
        }
        false
    }

    // ── Center agent view ─────────────────────────────────────────────────────

    /// Format the active session's conversation for the center "Agent" overlay.
    pub(crate) fn format_center_agent_lines(&self) -> Vec<String> {
        let Some(session) = self.agent_panel.sessions.get(self.agent_panel.active_session) else {
            return Vec::new();
        };
        let label = &session.label;
        let mut lines = vec![format!("  ◎  {label}"), String::new()];

        let Some(conv) = self.chat_conversations.get(&session.id) else {
            lines.push(
                "  No messages yet. Type in the right panel and press Ctrl+↵.".to_string(),
            );
            return lines;
        };
        if conv.is_empty() {
            lines.push(
                "  No messages yet. Type in the right panel and press Ctrl+↵.".to_string(),
            );
            return lines;
        }

        for msg in conv.messages() {
            match msg.role {
                ChatRole::User => {
                    lines.push(
                        "  ── You ──────────────────────────────────────────────".to_string(),
                    );
                    for line in msg.text.lines() {
                        lines.push(format!("  {line}"));
                    }
                }
                ChatRole::Assistant => {
                    lines.push(
                        "  ── Claude ───────────────────────────────────────────".to_string(),
                    );
                    for line in msg.text.lines() {
                        lines.push(format!("  {line}"));
                    }
                    if msg.is_streaming {
                        lines.push("  ▌".to_string());
                    }
                }
                ChatRole::Tool { .. } => {
                    lines.push(format!("  {}", msg.text));
                }
                ChatRole::Note => {
                    lines.push(format!("  ─ {}", msg.text));
                }
            }
            lines.push(String::new());
        }
        lines
    }

    // ── Metadata sidecar helpers ──────────────────────────────────────────────

    /// Append a history entry to the sidecar of the currently open file.
    /// Called when an AI session finishes (Phase 7).
    fn write_session_history_entry(
        &self,
        session_id: u64,
        store: &editor_metadata::MetadataStore,
    ) {
        let Some(ref file_path) = self.open_path else { return };
        if !file_path.exists() { return }

        let session_label = self
            .agent_panel
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.label.clone())
            .unwrap_or_else(|| format!("session-{session_id}"));

        let assistant_text: String = self
            .chat_conversations
            .get(&session_id)
            .map(|conv| {
                conv.messages()
                    .iter()
                    .filter(|m| matches!(m.role, ChatRole::Assistant))
                    .map(|m| m.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

        if assistant_text.trim().is_empty() {
            return;
        }

        let summary: String = assistant_text.chars().take(200).collect();
        let rel_path = file_path
            .strip_prefix(store.workspace_root())
            .unwrap_or(file_path)
            .to_path_buf();

        let mut sidecar = store.load(file_path).unwrap_or(None).unwrap_or_else(|| {
            let content = std::fs::read_to_string(file_path).unwrap_or_default();
            blank_sidecar(&rel_path, &content, "claude-opus-4-7")
        });

        sidecar.body.history.push(HistoryEntry {
            timestamp: chrono::Utc::now(),
            summary,
            session_id: session_label,
        });
        sidecar.frontmatter.last_updated = chrono::Utc::now();

        if let Err(e) = store.save(&sidecar) {
            tracing::warn!(error = %e, path = %file_path.display(), "failed to write metadata sidecar");
        }
    }

    /// Load the sidecar for the current open file and inject prior reasoning
    /// as a context note into the active conversation. Called on file open.
    pub(crate) fn inject_file_metadata_context(&mut self) {
        let Some(ref file_path) = self.open_path.clone() else { return };
        let Some(ref store) = self.metadata_store.clone() else { return };
        let Ok(Some(sidecar)) = store.load(file_path) else { return };

        let has_reasoning = !sidecar.body.reasoning.trim().is_empty();
        let has_notes = !sidecar.body.notes.trim().is_empty();
        let has_history = !sidecar.body.history.is_empty();
        if !has_reasoning && !has_notes && !has_history {
            return;
        }

        let filename = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.display().to_string());

        let mut note = format!("📎 Prior context for {filename}:");
        if has_reasoning {
            let snippet: String = sidecar.body.reasoning.chars().take(300).collect();
            note.push_str(&format!("\n  Reasoning: {snippet}"));
        }
        if has_history {
            if let Some(last) = sidecar.body.history.last() {
                note.push_str(&format!(
                    "\n  Last session ({}): {}",
                    last.session_id, last.summary
                ));
            }
        }
        if has_notes {
            let snippet: String = sidecar.body.notes.chars().take(200).collect();
            note.push_str(&format!("\n  Notes: {snippet}"));
        }

        let session_id = match self.agent_panel.sessions.get(self.agent_panel.active_session) {
            Some(s) => s.id,
            None => return,
        };
        let conv = self.chat_conversations.entry(session_id).or_insert_with(Conversation::new);
        conv.push_note(note);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    pub(crate) fn set_session_status(&mut self, session_id: u64, status: AgentSessionStatus) {
        for s in &mut self.agent_panel.sessions {
            if s.id == session_id {
                s.status = status;
                break;
            }
        }
    }

    pub(crate) fn clear_chat_input(&mut self) {
        self.chat_input.clear();
        self.chat_input_cursor = 0;
    }

    /// Call once per frame while a stream is active.
    /// Injects a stall warning note after 12 s of silence, and resets so it only fires once.
    /// Returns `true` if the UI needs a redraw.
    pub(crate) fn check_chat_stall(&mut self) -> bool {
        const STALL_SECS: u64 = 12;
        let Some(last) = self.chat_last_delta_at else { return false; };
        if last.elapsed().as_secs() < STALL_SECS { return false; }

        let is_running = self.agent_panel.sessions.iter()
            .any(|s| s.status == AgentSessionStatus::Running);
        if !is_running { return false; }

        // Clear so this fires only once per stall period.
        self.chat_last_delta_at = None;

        let session_id = self.agent_panel.sessions
            .get(self.agent_panel.active_session)
            .map(|s| s.id);
        if let Some(sid) = session_id {
            let conv = self.chat_conversations.entry(sid).or_insert_with(editor_chat::Conversation::new);
            conv.push_note(
                "⏳ Stream has been silent for >12 s. Press Escape to cancel and retry."
            );
        }
        true
    }
}
