//! Chat engine integration for the main event loop.
//!
//! All AI-specific App methods live here so main.rs stays focused on the
//! frame loop and input routing.  Add this file as `mod chat;` in main.rs.
//!
//! Phases implemented:
//!  Phase 1 — streaming text (TextDelta / Done / Error events)
//!  Phase 2 — tool execution loop (ToolCall → execute → resume stream)

use std::path::{Path, PathBuf};

use editor_chat::{ChatRole, Conversation, EngineEvent};
use editor_ui::{AgentSessionStatus, ChatDisplayMsg, ChatDisplayRole};
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::App;

impl App {
    // ── Event polling ────────────────────────────────────────────────────────

    /// Drain all pending [`EngineEvent`]s and update conversation state.
    /// Returns `true` when at least one event was received (redraw needed).
    pub(crate) fn poll_chat_events(&mut self) -> bool {
        let mut got_event = false;

        // Collect first to avoid borrowing `self.chat_engine` while mutating other fields.
        let events: Vec<EngineEvent> = self.chat_engine.events().try_iter().collect();

        for ev in events {
            got_event = true;
            match ev {
                EngineEvent::TextDelta { session_id, message_id, delta } => {
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.append_text(message_id, &delta);
                    }
                    self.agent_panel.chat_scroll_offset = f32::MAX;
                }

                EngineEvent::Done { session_id, message_id, stop_reason, tokens_out } => {
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.finish_streaming(message_id, stop_reason, tokens_out);
                    }
                    self.set_session_status(session_id, AgentSessionStatus::Done);
                }

                EngineEvent::Error { session_id, message_id, message } => {
                    tracing::warn!(session_id, %message, "AI stream error");
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.append_text(message_id, &format!("\n⚠ {message}"));
                        conv.finish_streaming(message_id, Some("error".into()), 0);
                    }
                    self.set_session_status(session_id, AgentSessionStatus::Done);
                }

                EngineEvent::ToolCall { session_id, call_id, name, input_json, result_tx } => {
                    tracing::debug!(session_id, call_id, name, "executing tool call");

                    // Show the invocation inline before executing.
                    let preview = if input_json.len() > 80 {
                        format!("{}…", &input_json[..80])
                    } else {
                        input_json.clone()
                    };
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        conv.push_tool_note(format!("▶ {name}({preview})"));
                    }
                    self.agent_panel.chat_scroll_offset = f32::MAX;

                    // Execute the tool synchronously on the main thread.
                    let workspace_root = self
                        .workspace
                        .as_ref()
                        .map(|w| w.root().to_path_buf())
                        .or_else(|| {
                            self.open_path
                                .as_ref()
                                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                        })
                        .unwrap_or_else(|| {
                            std::env::current_dir()
                                .unwrap_or_else(|_| PathBuf::from("."))
                        });

                    let (result_content, is_error) =
                        execute_tool_fs(&name, &input_json, &workspace_root);

                    // Show result note inline.
                    let result_preview = if result_content.len() > 120 {
                        format!("{}…", &result_content[..120])
                    } else {
                        result_content.clone()
                    };
                    if let Some(conv) = self.chat_conversations.get_mut(&session_id) {
                        let prefix = if is_error { "✗" } else { "✔" };
                        conv.push_tool_note(format!("{prefix} {result_preview}"));
                    }
                    self.agent_panel.chat_scroll_offset = f32::MAX;

                    // Send the result back to the engine (unblocks the streaming task).
                    let _ = result_tx.send((result_content, is_error));
                }
            }
        }

        got_event
    }

    // ── Submission ───────────────────────────────────────────────────────────

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
            let conv =
                self.chat_conversations.entry(session_id).or_insert_with(Conversation::new);
            conv.push_user(prompt.clone());
            conv.push_note(
                "⚠ No AI provider configured. Set ANTHROPIC_API_KEY in your environment, \
                 then restart — or open Settings (Ctrl+,) to configure a key.",
            );
            self.clear_chat_input();
            self.agent_panel.chat_scroll_offset = f32::MAX;
            return;
        }

        // Build history from prior turns (tool / note messages excluded).
        let history: Vec<(ChatRole, String)> = self
            .chat_conversations
            .get(&session_id)
            .map(|c| {
                c.messages()
                    .iter()
                    .filter_map(|m| match m.role {
                        editor_chat::ChatRole::User => Some((ChatRole::User, m.text.clone())),
                        editor_chat::ChatRole::Assistant => {
                            Some((ChatRole::Assistant, m.text.clone()))
                        }
                        editor_chat::ChatRole::Tool { .. } | editor_chat::ChatRole::Note => None,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let conv = self.chat_conversations.entry(session_id).or_insert_with(Conversation::new);
        conv.push_user(prompt.clone());
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
        self.agent_panel.chat_scroll_offset = f32::MAX;
    }

    // ── Keyboard input for the textarea ─────────────────────────────────────

    /// Handle a key press while the agent panel textarea has focus.
    /// Returns `true` when the event was consumed.
    pub(crate) fn handle_agent_panel_key(&mut self, event: &KeyEvent) -> bool {
        if !self.agent_panel_focused {
            return false;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        let ctrl = self.modifiers.control_key() || self.modifiers.super_key();

        match code {
            KeyCode::Escape => {
                self.agent_panel_focused = false;
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
                let to_newline = after.find('\n').unwrap_or(after.len());
                self.chat_input_cursor += to_newline;
                return true;
            }
            _ => {}
        }

        // Printable characters — only when ctrl is NOT held.
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

    // ── Display message conversion ───────────────────────────────────────────

    /// Build the display message list for the currently active session.
    pub(crate) fn active_session_display_msgs(&self) -> Vec<ChatDisplayMsg> {
        let Some(session) = self.agent_panel.sessions.get(self.agent_panel.active_session) else {
            return Vec::new();
        };
        let Some(conv) = self.chat_conversations.get(&session.id) else {
            return Vec::new();
        };
        conv.messages()
            .iter()
            .map(|m| ChatDisplayMsg {
                role: match m.role {
                    editor_chat::ChatRole::User => ChatDisplayRole::User,
                    editor_chat::ChatRole::Assistant => ChatDisplayRole::Assistant,
                    editor_chat::ChatRole::Tool { .. } => ChatDisplayRole::Tool,
                    editor_chat::ChatRole::Note => ChatDisplayRole::Note,
                },
                text: m.text.clone(),
                is_streaming: m.is_streaming,
            })
            .collect()
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn set_session_status(&mut self, session_id: u64, status: AgentSessionStatus) {
        for s in &mut self.agent_panel.sessions {
            if s.id == session_id {
                s.status = status;
                break;
            }
        }
    }

    fn clear_chat_input(&mut self) {
        self.chat_input.clear();
        self.chat_input_cursor = 0;
    }
}

// ── Tool execution (filesystem, no buffer manager required) ──────────────────

/// Execute a named tool synchronously using direct filesystem operations.
///
/// Returns `(result_content, is_error)`.  Write operations go straight to
/// disk; the IDE's existing file-change watcher will detect and reload them.
fn execute_tool_fs(
    name: &str,
    input_json: &str,
    workspace_root: &Path,
) -> (String, bool) {
    let input: serde_json::Value = match serde_json::from_str(input_json) {
        Ok(v) => v,
        Err(e) => return (format!("Invalid tool input JSON: {e}"), true),
    };

    match name {
        "read_file" => tool_read_file(&input, workspace_root),
        "list_directory" => tool_list_directory(&input, workspace_root),
        "find_files" => tool_find_files(&input, workspace_root),
        "grep" => tool_grep(&input, workspace_root),
        "edit_lines" => tool_edit_lines(&input, workspace_root),
        "insert_at" => tool_insert_at(&input, workspace_root),
        "append_to" => tool_append_to(&input, workspace_root),
        "create_file" => tool_create_file(&input, workspace_root),
        "delete_file" => tool_delete_file(&input, workspace_root),
        "move_file" => tool_move_file(&input, workspace_root),
        other => (format!("Unknown tool: {other}"), true),
    }
}

fn resolve_path(input: &serde_json::Value, key: &str, root: &Path) -> Result<PathBuf, String> {
    let s = input[key]
        .as_str()
        .ok_or_else(|| format!("Missing required field '{key}'"))?;
    let p = Path::new(s);
    let abs = if p.is_absolute() { p.to_path_buf() } else { root.join(p) };
    // Security: keep path under root (for relative paths) or just normalise absolute.
    Ok(abs)
}

fn tool_read_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    match resolve_path(input, "path", root) {
        Err(e) => (e, true),
        Ok(path) => match std::fs::read_to_string(&path) {
            Ok(content) => {
                let lines = content.lines().count();
                (format!("File: {} ({} lines)\n---\n{}", path.display(), lines, content), false)
            }
            Err(e) => (format!("Cannot read {}: {e}", path.display()), true),
        },
    }
}

fn tool_list_directory(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let recursive = input["recursive"].as_bool().unwrap_or(false);
    match resolve_path(input, "path", root) {
        Err(e) => (e, true),
        Ok(path) => {
            if !path.is_dir() {
                return (format!("{} is not a directory", path.display()), true);
            }
            let mut entries: Vec<String> = Vec::new();
            if recursive {
                collect_entries_recursive(&path, &path, &mut entries, 0, 5);
            } else {
                match std::fs::read_dir(&path) {
                    Err(e) => return (format!("Cannot list {}: {e}", path.display()), true),
                    Ok(rd) => {
                        let mut names: Vec<String> = rd
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                                let n = e.file_name().to_string_lossy().to_string();
                                if is_dir { format!("{n}/") } else { n }
                            })
                            .collect();
                        names.sort();
                        entries = names;
                    }
                }
            }
            if entries.is_empty() {
                return (format!("{} is empty", path.display()), false);
            }
            (entries.join("\n"), false)
        }
    }
}

fn collect_entries_recursive(
    root: &Path,
    dir: &Path,
    out: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let rel = entry
            .path()
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());
        if is_dir {
            out.push(format!("{rel}/"));
            collect_entries_recursive(root, &entry.path(), out, depth + 1, max_depth);
        } else {
            out.push(rel);
        }
    }
}

fn tool_find_files(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let pattern = match input["pattern"].as_str() {
        Some(p) => p,
        None => return ("Missing required field 'pattern'".into(), true),
    };
    let search_root = if let Some(p) = input["path"].as_str() {
        root.join(p)
    } else {
        root.to_path_buf()
    };

    let mut matches: Vec<String> = Vec::new();
    let glob_pat = if search_root != *root && !pattern.starts_with("**/") {
        format!("**/{pattern}")
    } else {
        pattern.to_string()
    };

    walk_and_match(&search_root, &search_root, &glob_pat, &mut matches, 0, 8);
    matches.sort();
    matches.truncate(200);

    if matches.is_empty() {
        (format!("No files match '{pattern}'"), false)
    } else {
        (matches.join("\n"), false)
    }
}

fn walk_and_match(
    root: &Path,
    dir: &Path,
    pattern: &str,
    out: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.filter_map(|e| e.ok()) {
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        // Skip hidden dirs
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') && name != "." {
            continue;
        }
        if is_dir {
            walk_and_match(root, &path, pattern, out, depth + 1, max_depth);
        } else {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());
            if glob_match(pattern, &rel) || glob_match(pattern, &name) {
                out.push(rel);
            }
        }
    }
}

fn glob_match(pattern: &str, s: &str) -> bool {
    // Simple glob: `**` matches any path segments, `*` matches within a segment.
    // Use the `glob` crate if available; otherwise fall back to basic matching.
    let pat_parts: Vec<&str> = pattern.split('/').collect();
    let s_parts: Vec<&str> = s.split('/').collect();
    glob_match_parts(&pat_parts, &s_parts)
}

fn glob_match_parts(pat: &[&str], s: &[&str]) -> bool {
    if pat.is_empty() {
        return s.is_empty();
    }
    if pat[0] == "**" {
        // `**` can match 0 or more path segments
        if pat.len() == 1 {
            return true;
        }
        for i in 0..=s.len() {
            if glob_match_parts(&pat[1..], &s[i..]) {
                return true;
            }
        }
        return false;
    }
    if s.is_empty() {
        return false;
    }
    if wildcard_match(pat[0], s[0]) {
        return glob_match_parts(&pat[1..], &s[1..]);
    }
    false
}

fn wildcard_match(pattern: &str, s: &str) -> bool {
    let (mut pi, mut si) = (0usize, 0usize);
    let pb = pattern.as_bytes();
    let sb = s.as_bytes();
    let (mut star_pi, mut star_si) = (usize::MAX, usize::MAX);
    while si < sb.len() {
        if pi < pb.len() && (pb[pi] == b'?' || pb[pi] == sb[si]) {
            pi += 1;
            si += 1;
        } else if pi < pb.len() && pb[pi] == b'*' {
            star_pi = pi;
            star_si = si;
            pi += 1;
        } else if star_pi != usize::MAX {
            star_si += 1;
            si = star_si;
            pi = star_pi + 1;
        } else {
            return false;
        }
    }
    while pi < pb.len() && pb[pi] == b'*' {
        pi += 1;
    }
    pi == pb.len()
}

fn tool_grep(input: &serde_json::Value, root: &Path) -> (String, bool) {

    let pattern_str = match input["pattern"].as_str() {
        Some(p) => p,
        None => return ("Missing required field 'pattern'".into(), true),
    };
    let search_root = if let Some(p) = input["path"].as_str() {
        root.join(p)
    } else {
        root.to_path_buf()
    };
    let file_pattern = input["file_pattern"].as_str().unwrap_or("*");
    let context = input["context_lines"].as_u64().unwrap_or(2) as usize;

    // Use simple case-sensitive substring for now (no regex overhead).
    let mut results: Vec<String> = Vec::new();
    let mut file_count = 0usize;

    grep_dir(
        &search_root,
        &search_root,
        pattern_str,
        file_pattern,
        context,
        &mut results,
        &mut file_count,
        0,
        6,
    );

    results.truncate(500);

    if results.is_empty() {
        (format!("No matches for '{pattern_str}'"), false)
    } else {
        (results.join("\n"), false)
    }
}

#[allow(clippy::too_many_arguments)]
fn grep_dir(
    root: &Path,
    dir: &Path,
    pattern: &str,
    file_pattern: &str,
    context: usize,
    out: &mut Vec<String>,
    file_count: &mut usize,
    depth: usize,
    max_depth: usize,
) {
    use std::io::{BufRead, BufReader};

    if depth > max_depth || *file_count > 100 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if is_dir {
            grep_dir(root, &path, pattern, file_pattern, context, out, file_count, depth + 1, max_depth);
        } else if wildcard_match(file_pattern, &name) {
            let Ok(f) = std::fs::File::open(&path) else { continue };
            *file_count += 1;
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());
            let reader = BufReader::new(f);
            let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
            let mut i = 0usize;
            while i < lines.len() {
                if lines[i].contains(pattern) {
                    let start = i.saturating_sub(context);
                    let end = (i + context + 1).min(lines.len());
                    out.push(format!("{}:{}", rel, i + 1));
                    for (j, l) in lines[start..end].iter().enumerate() {
                        let lnum = start + j + 1;
                        let marker = if start + j == i { ">" } else { " " };
                        out.push(format!("{marker}{lnum:4}: {l}"));
                    }
                    out.push(String::new());
                    i = end;
                } else {
                    i += 1;
                }
            }
        }
    }
}

fn tool_edit_lines(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let start_line = match input["start_line"].as_u64() {
        Some(n) if n >= 1 => n as usize,
        _ => return ("'start_line' must be a positive integer".into(), true),
    };
    let end_line = match input["end_line"].as_u64() {
        Some(n) if n >= start_line as u64 => n as usize,
        _ => return ("'end_line' must be >= start_line".into(), true),
    };
    let new_content = match input["new_content"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'new_content'".into(), true),
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return (format!("Cannot read {}: {e}", path.display()), true),
    };
    let mut lines: Vec<&str> = content.lines().collect();
    if end_line > lines.len() {
        return (
            format!("end_line {end_line} > file length {}", lines.len()),
            true,
        );
    }
    let new_lines: Vec<&str> = new_content.lines().collect();
    lines.splice((start_line - 1)..end_line, new_lines);
    let new_file = lines.join("\n") + if content.ends_with('\n') { "\n" } else { "" };
    match std::fs::write(&path, &new_file) {
        Ok(()) => (
            format!("Replaced lines {start_line}–{end_line} in {}", path.display()),
            false,
        ),
        Err(e) => (format!("Cannot write {}: {e}", path.display()), true),
    }
}

fn tool_insert_at(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let line_number = match input["line_number"].as_u64() {
        Some(n) if n >= 1 => n as usize,
        _ => return ("'line_number' must be a positive integer".into(), true),
    };
    let content_str = match input["content"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'content'".into(), true),
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return (format!("Cannot read {}: {e}", path.display()), true),
    };
    let mut lines: Vec<&str> = content.lines().collect();
    let insert_at = (line_number - 1).min(lines.len());
    let new_lines: Vec<&str> = content_str.lines().collect();
    for (i, l) in new_lines.into_iter().enumerate() {
        lines.insert(insert_at + i, l);
    }
    let new_file = lines.join("\n") + if content.ends_with('\n') { "\n" } else { "" };
    match std::fs::write(&path, &new_file) {
        Ok(()) => (format!("Inserted {} line(s) at line {line_number} in {}", content_str.lines().count(), path.display()), false),
        Err(e) => (format!("Cannot write {}: {e}", path.display()), true),
    }
}

fn tool_append_to(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let content_str = match input["content"].as_str() {
        Some(s) => s,
        None => return ("Missing required field 'content'".into(), true),
    };
    use std::io::Write;
    match std::fs::OpenOptions::new().append(true).create(true).open(&path) {
        Ok(mut f) => match f.write_all(content_str.as_bytes()) {
            Ok(()) => (format!("Appended {} bytes to {}", content_str.len(), path.display()), false),
            Err(e) => (format!("Write error on {}: {e}", path.display()), true),
        },
        Err(e) => (format!("Cannot open {} for append: {e}", path.display()), true),
    }
}

fn tool_create_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    if path.exists() {
        return (format!("{} already exists", path.display()), true);
    }
    let content_str = input["content"].as_str().unwrap_or("");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, content_str) {
        Ok(()) => (format!("Created {} ({} bytes)", path.display(), content_str.len()), false),
        Err(e) => (format!("Cannot create {}: {e}", path.display()), true),
    }
}

fn tool_delete_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let path = match resolve_path(input, "path", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    match std::fs::remove_file(&path) {
        Ok(()) => (format!("Deleted {}", path.display()), false),
        Err(e) => (format!("Cannot delete {}: {e}", path.display()), true),
    }
}

fn tool_move_file(input: &serde_json::Value, root: &Path) -> (String, bool) {
    let from = match resolve_path(input, "from", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    let to = match resolve_path(input, "to", root) {
        Err(e) => return (e, true),
        Ok(p) => p,
    };
    if to.exists() {
        return (format!("Destination {} already exists", to.display()), true);
    }
    match std::fs::rename(&from, &to) {
        Ok(()) => (format!("Moved {} → {}", from.display(), to.display()), false),
        Err(e) => (format!("Cannot move: {e}"), true),
    }
}
