//! Right-side agent panel: session tabs, AI chat input, and terminal section.
//!
//! Design (v3 — matching Antigravity IDE v3 design spec):
//!   ┌── session tab row (37 px) ────────────────────────────────┐
//!   ├── chat textarea (fills remaining space) ──────────────────┤
//!   │  (user types here; conversation shows in center editor)   │
//!   ├── bottom row: model selector + [⌘↵] [Send] (36 px) ──────┤
//!   ├── drag handle (5 px) ──────────────────────────────────────┤
//!   │  Terminal (terminal_fraction of panel height)             │
//!   └────────────────────────────────────────────────────────────┘
//!
//! Conversations are shown in the center area as an "Agent" tab when a session
//! tab is clicked; the right panel stays focused on input + terminal.

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::theme::palette as pal;

// ── Layout constants (logical px) ──────────────────────────────────────────

pub const AGENT_PANEL_WIDTH: f32 = 480.0;
pub const AGENT_PANEL_MIN_WIDTH: f32 = 300.0;
pub const AGENT_PANEL_MAX_WIDTH: f32 = 720.0;

const SESSION_TAB_ROW_H: f32 = 37.0;
const BOTTOM_ROW_H: f32 = 36.0;
const DRAG_H: f32 = 5.0;
const TERM_HEADER_H: f32 = 30.0;
/// Approximate logical px per character in monospace layout.
const CHAR_W: f32 = 7.2;
/// Logical px per line of chat text.
const LINE_H: f32 = 14.0;

// ── Chat display types (used by center agent view builder in main.rs) ───────

/// Minimal role tag for display purposes. Decoupled from `editor-chat` so
/// `editor-ui` stays free of AI/tokio dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatDisplayRole {
    User,
    Assistant,
    /// Tool invocation line (read-only, shown monospace).
    Tool,
    /// Inline note / warning from the app itself.
    Note,
}

/// A single message ready for rendering (used by center view builder).
#[derive(Debug, Clone)]
pub struct ChatDisplayMsg {
    pub role: ChatDisplayRole,
    pub text: String,
    pub is_streaming: bool,
}

// ── Session types ───────────────────────────────────────────────────────────

/// Running / queued / finished state for an agent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSessionStatus {
    /// Completed — green dot.
    Done,
    /// Actively streaming — violet dot.
    Running,
    /// Waiting to start — dim dot.
    Queued,
}

/// A single agent conversation session tracked in the panel.
#[derive(Debug, Clone)]
pub struct AgentSession {
    pub id: u64,
    pub label: String,
    pub status: AgentSessionStatus,
}

impl AgentSession {
    #[must_use]
    pub fn new(id: u64, label: impl Into<String>, status: AgentSessionStatus) -> Self {
        Self { id, label: label.into(), status }
    }
}

// ── Hit regions ─────────────────────────────────────────────────────────────

/// Hit region for a session tab click.
#[derive(Debug, Clone)]
pub struct AgentTabHit {
    pub session_idx: usize,
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    pub is_close: bool,
}

/// All click-able sub-regions returned from [`AgentPanel::paint`].
#[derive(Debug, Clone, Default)]
pub struct AgentPanelHits {
    pub tab_hits: Vec<AgentTabHit>,
    /// Physical rect of the Send button (x0, y0, x1, y1).
    pub send_button: Option<[f32; 4]>,
    /// Physical rect of the chat textarea.
    pub input_area: Option<[f32; 4]>,
    /// Physical rect of the "+ new session" button.
    pub new_session_btn: Option<[f32; 4]>,
    /// Physical Y range [y0, y1] of the input/terminal drag handle.
    pub drag_handle: Option<[f32; 2]>,
}

// ── Panel state ─────────────────────────────────────────────────────────────

/// Persistent state for the agent panel.
#[derive(Debug)]
pub struct AgentPanel {
    pub width: f32,
    pub visible: bool,
    pub terminal_fraction: f32,
    pub sessions: Vec<AgentSession>,
    pub active_session: usize,
    next_session_id: u64,
}

impl Default for AgentPanel {
    fn default() -> Self {
        let mut panel = Self {
            width: AGENT_PANEL_WIDTH,
            visible: true,
            terminal_fraction: 0.35,
            sessions: Vec::new(),
            active_session: 0,
            next_session_id: 1,
        };
        panel.add_session("New Chat", AgentSessionStatus::Queued);
        panel
    }
}

impl AgentPanel {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Physical pixel width (or 0 when hidden).
    #[must_use]
    pub fn width_px(&self, scale: f32) -> f32 {
        if self.visible { self.width * scale } else { 0.0 }
    }

    /// Update width from a drag (physical pixels, dragging the left edge leftward = wider).
    pub fn apply_drag(&mut self, delta_px: f32, scale: f32) {
        let delta_logical = delta_px / scale;
        self.width =
            (self.width - delta_logical).clamp(AGENT_PANEL_MIN_WIDTH, AGENT_PANEL_MAX_WIDTH);
    }

    /// Add a new session, returning its id.
    pub fn add_session(&mut self, label: impl Into<String>, status: AgentSessionStatus) -> u64 {
        let id = self.next_session_id;
        self.next_session_id += 1;
        self.sessions.push(AgentSession::new(id, label, status));
        id
    }

    /// Remove a session by index, clamping `active_session` if needed.
    pub fn remove_session(&mut self, idx: usize) {
        if idx < self.sessions.len() {
            self.sessions.remove(idx);
            if self.active_session >= self.sessions.len() && !self.sessions.is_empty() {
                self.active_session = self.sessions.len() - 1;
            }
        }
    }

    /// Left X (physical px) of the resize drag edge.
    #[must_use]
    pub fn resize_edge_x_px(&self, scale: f32, window_width_px: f32) -> Option<f32> {
        if !self.visible { return None; }
        Some(window_width_px - self.width_px(scale))
    }

    /// Y coordinate (physical px) of the input/terminal drag handle.
    #[must_use]
    pub fn drag_handle_y_px(
        &self,
        scale: f32,
        panel_top_px: f32,
        panel_height_px: f32,
        terminal_visible: bool,
    ) -> Option<f32> {
        if !self.visible || !terminal_visible { return None; }
        let header_h = SESSION_TAB_ROW_H * scale;
        let bottom_row_h = BOTTOM_ROW_H * scale;
        let term_h = panel_height_px * self.terminal_fraction;
        let drag_h = DRAG_H * scale;
        let input_h = (panel_height_px - header_h - bottom_row_h - drag_h - term_h).max(40.0 * scale);
        Some(panel_top_px + header_h + input_h + bottom_row_h)
    }

    // ── Paint ───────────────────────────────────────────────────────────────

    /// Paint the full panel chrome into `frame`.
    ///
    /// Returns [`AgentPanelHits`] with all click regions for use by the main event loop.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        chrome: &mut FrameChrome,
        scale: f32,
        left_px: f32,
        top_px: f32,
        height_px: f32,
        terminal_visible: bool,
        chat_input: &str,
        chat_input_cursor: usize,
        input_focused: bool,
        blink_on: bool,
        // Display label for the active AI model (e.g. "claude-sonnet-4-6").
        active_model: &str,
    ) -> AgentPanelHits {
        if !self.visible || height_px <= 0.5 {
            return AgentPanelHits::default();
        }
        let w = self.width * scale;

        // Panel background.
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: top_px,
            width: w,
            height: height_px,
            rgba: pal::AGENT_BG,
        });
        // Left border.
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: top_px,
            width: scale,
            height: height_px,
            rgba: pal::AGENT_BORDER,
        });

        // ── Session tab row ───────────────────────────────────────────────
        let tab_row_h = SESSION_TAB_ROW_H * scale;
        let (tab_hits, new_btn_rect) =
            self.paint_session_tabs(chrome, scale, left_px, top_px, w, tab_row_h);
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: top_px + tab_row_h - scale,
            width: w,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });

        // ── Geometry ──────────────────────────────────────────────────────
        let term_h = if terminal_visible { height_px * self.terminal_fraction } else { 0.0 };
        let drag_h = if terminal_visible { DRAG_H * scale } else { 0.0 };
        let bottom_row_h = BOTTOM_ROW_H * scale;
        let input_area_h =
            (height_px - tab_row_h - bottom_row_h - drag_h - term_h).max(40.0 * scale);

        let input_top = top_px + tab_row_h;
        let bottom_top = input_top + input_area_h;

        // ── Chat textarea (fills all available space) ─────────────────────
        // Input area background.
        chrome.push_quad(ChromeQuad {
            left: left_px + scale,
            top: input_top,
            width: w - scale,
            height: input_area_h,
            rgba: pal::AGENT_INPUT_BG,
        });

        // Focus ring on left edge.
        if input_focused {
            chrome.push_quad(ChromeQuad {
                left: left_px,
                top: input_top,
                width: 2.0 * scale,
                height: input_area_h,
                rgba: pal::ACCENT_BLUE,
            });
        }

        let pad = 14.0 * scale;
        if chat_input.is_empty() {
            // Placeholder text.
            chrome.push_line(
                left_px + pad + scale,
                input_top + 14.0 * scale,
                "Ask anything, or describe what to build\u{2026}".to_string(),
                pal::AGENT_HEADER_FG,
            );
        } else {
            let max_chars_f = (self.width - 32.0) / CHAR_W;
            self.paint_input_text(
                chrome,
                scale,
                left_px + pad + scale,
                input_top + 12.0 * scale,
                max_chars_f,
                chat_input,
                chat_input_cursor,
                input_focused && blink_on,
            );
        }

        // ── Bottom row ────────────────────────────────────────────────────
        chrome.push_quad(ChromeQuad {
            left: left_px + scale,
            top: bottom_top,
            width: w - scale,
            height: bottom_row_h,
            rgba: pal::AGENT_BG,
        });
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: bottom_top,
            width: w,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });

        // Send button (right-aligned).
        let btn_h = 26.0 * scale;
        let btn_w = 64.0 * scale;
        let btn_x = left_px + w - pad - btn_w;
        let btn_y = bottom_top + (bottom_row_h - btn_h) / 2.0;
        chrome.push_quad(ChromeQuad {
            left: btn_x,
            top: btn_y,
            width: btn_w,
            height: btn_h,
            rgba: pal::AGENT_SEND_BG,
        });
        chrome.push_line(
            btn_x + 18.0 * scale,
            btn_y + (btn_h - 10.0 * scale) / 2.0,
            "Send".to_string(),
            [0xff, 0xff, 0xff],
        );

        // ⌘↵ hint.
        chrome.push_line(
            btn_x - 42.0 * scale,
            btn_y + (btn_h - 9.0 * scale) / 2.0,
            "\u{2318}\u{21b5}".to_string(),
            pal::AGENT_HEADER_FG,
        );

        // Model selector (left side of bottom row).
        let model_label = if active_model.is_empty() { "no model set" } else { active_model };
        chrome.push_line(
            left_px + pad + scale,
            bottom_top + (bottom_row_h - 9.0 * scale) / 2.0,
            model_label.to_string(),
            pal::AGENT_HEADER_FG,
        );

        // ── Terminal section ──────────────────────────────────────────────
        let mut drag_handle_y = None;
        if terminal_visible {
            let drag_top = bottom_top + bottom_row_h;
            drag_handle_y = Some([drag_top, drag_top + drag_h]);
            chrome.push_quad(ChromeQuad {
                left: left_px + scale,
                top: drag_top + drag_h * 0.5 - 0.5 * scale,
                width: w - scale,
                height: scale,
                rgba: pal::AGENT_BORDER,
            });
            let th_top = drag_top + drag_h;
            chrome.push_quad(ChromeQuad {
                left: left_px,
                top: th_top,
                width: w,
                height: TERM_HEADER_H * scale,
                rgba: pal::AGENT_BG,
            });
            chrome.push_quad(ChromeQuad {
                left: left_px,
                top: th_top + TERM_HEADER_H * scale - scale,
                width: w,
                height: scale,
                rgba: pal::AGENT_BORDER,
            });
            chrome.push_line(
                left_px + 14.0 * scale,
                th_top + (TERM_HEADER_H * scale - 9.0 * scale) / 2.0,
                "Terminal".to_string(),
                pal::AGENT_HEADER_FG,
            );
        }

        AgentPanelHits {
            tab_hits,
            send_button: Some([btn_x, btn_y, btn_x + btn_w, btn_y + btn_h]),
            input_area: Some([
                left_px + scale,
                input_top,
                left_px + w,
                input_top + input_area_h,
            ]),
            new_session_btn: new_btn_rect,
            drag_handle: drag_handle_y,
        }
    }

    // ── Internal renderers ───────────────────────────────────────────────────

    /// Render the multi-line chat input with a text cursor.
    fn paint_input_text(
        &self,
        chrome: &mut FrameChrome,
        scale: f32,
        text_left: f32,
        text_top: f32,
        max_chars_f: f32,
        text: &str,
        cursor_byte: usize,
        show_cursor: bool,
    ) {
        let line_h = LINE_H * scale;
        let max_c = (max_chars_f as usize).max(8);
        let lines: Vec<&str> = text.split('\n').collect();
        let mut byte_count = 0usize;
        let mut cursor_drawn = false;
        for (li, line) in lines.iter().enumerate() {
            let y = text_top + li as f32 * line_h;
            let display = if line.len() > max_c { &line[..max_c] } else { line };
            chrome.push_line(text_left, y, display.to_string(), pal::EDITOR_FG);
            if show_cursor && !cursor_drawn {
                let line_end = byte_count + line.len();
                let line_start = byte_count;
                if cursor_byte >= line_start && cursor_byte <= line_end {
                    let col = cursor_byte - line_start;
                    let cx = text_left + col as f32 * CHAR_W * scale - 0.5 * scale;
                    chrome.push_quad(ChromeQuad {
                        left: cx,
                        top: y,
                        width: scale,
                        height: line_h,
                        rgba: pal::ACCENT_BLUE,
                    });
                    cursor_drawn = true;
                }
            }
            byte_count += line.len() + 1;
        }
    }

    fn paint_session_tabs(
        &self,
        chrome: &mut FrameChrome,
        scale: f32,
        left_px: f32,
        top_px: f32,
        panel_w: f32,
        row_h: f32,
    ) -> (Vec<AgentTabHit>, Option<[f32; 4]>) {
        let mut hits = Vec::new();
        let mut x = left_px + scale;

        let tab_pad_h = 10.0 * scale;
        let dot_r = 4.0 * scale;
        let close_w = 18.0 * scale;
        let min_tab_w = 80.0 * scale;
        let new_btn_w = 28.0 * scale;
        let new_btn_right = left_px + panel_w - scale - new_btn_w;

        for (i, session) in self.sessions.iter().enumerate() {
            let is_active = i == self.active_session;

            let label_w = session.label.len() as f32 * 7.0 * scale;
            let tab_w =
                (tab_pad_h + dot_r * 2.0 + 6.0 * scale + label_w + close_w + tab_pad_h)
                    .max(min_tab_w);

            if x + tab_w > new_btn_right - 4.0 * scale {
                break;
            }

            let tab_bg = if is_active { pal::AGENT_INPUT_BG } else { pal::AGENT_BG };
            chrome.push_quad(ChromeQuad {
                left: x,
                top: top_px,
                width: tab_w,
                height: row_h,
                rgba: tab_bg,
            });
            if is_active {
                // Active tab: violet underline.
                chrome.push_quad(ChromeQuad {
                    left: x,
                    top: top_px + row_h - 2.0 * scale,
                    width: tab_w,
                    height: 2.0 * scale,
                    rgba: pal::ACCENT_BLUE,
                });
            }

            // Status dot.
            let dot_x = x + tab_pad_h;
            let dot_y = top_px + (row_h - dot_r * 2.0) / 2.0;
            chrome.push_quad(ChromeQuad {
                left: dot_x,
                top: dot_y,
                width: dot_r * 2.0,
                height: dot_r * 2.0,
                rgba: status_dot_color(session.status),
            });

            let label_x = dot_x + dot_r * 2.0 + 6.0 * scale;
            let label_y = top_px + (row_h - 9.0 * scale) / 2.0;
            let fg = if is_active { pal::EDITOR_FG } else { pal::SIDEBAR_ROW_FG };
            chrome.push_line(label_x, label_y, session.label.clone(), fg);

            let close_x = x + tab_w - close_w;
            chrome.push_line(
                close_x + 4.0 * scale,
                label_y,
                "\u{00d7}".to_string(),
                pal::AGENT_HEADER_FG,
            );

            hits.push(AgentTabHit {
                session_idx: i,
                x0: x,
                x1: x + tab_w,
                y0: top_px,
                y1: top_px + row_h,
                is_close: false,
            });
            hits.push(AgentTabHit {
                session_idx: i,
                x0: close_x,
                x1: x + tab_w,
                y0: top_px,
                y1: top_px + row_h,
                is_close: true,
            });

            // Tab separator.
            chrome.push_quad(ChromeQuad {
                left: x + tab_w - scale,
                top: top_px,
                width: scale,
                height: row_h,
                rgba: pal::AGENT_BORDER,
            });

            x += tab_w;
        }

        // "+ New session" button.
        chrome.push_quad(ChromeQuad {
            left: new_btn_right,
            top: top_px,
            width: new_btn_w,
            height: row_h,
            rgba: pal::AGENT_BG,
        });
        chrome.push_line(
            new_btn_right + 8.0 * scale,
            top_px + (row_h - 10.0 * scale) / 2.0,
            "+".to_string(),
            pal::AGENT_HEADER_FG,
        );
        let new_btn_rect = Some([new_btn_right, top_px, new_btn_right + new_btn_w, top_px + row_h]);

        (hits, new_btn_rect)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// RGBA color for a session status dot.
fn status_dot_color(status: AgentSessionStatus) -> [f32; 4] {
    match status {
        AgentSessionStatus::Done => pal::DIFF_ADDED,
        AgentSessionStatus::Running => pal::ACCENT_BLUE,
        AgentSessionStatus::Queued => pal::rgba_u8(0x3a, 0x3a, 0x52, 0xff),
    }
}

/// Wrap `text` at `max_chars` characters per line (word-aware).
pub fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let max_c = max_chars.max(8);
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in paragraph.split_ascii_whitespace() {
            if line.is_empty() {
                if word.len() > max_c {
                    out.push(word[..max_c].to_string());
                } else {
                    line.push_str(word);
                }
            } else if line.len() + 1 + word.len() <= max_c {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(std::mem::take(&mut line));
                if word.len() > max_c {
                    out.push(word[..max_c].to_string());
                } else {
                    line.push_str(word);
                }
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}
