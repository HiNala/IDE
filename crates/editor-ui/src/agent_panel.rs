//! Right-side agent panel: session tabs, AI chat input, and terminal section.
//!
//! Design (v5 — Cursor-style right rail):
//!   ┌── session tab row (37 px) ────────────────────────────────┐
//!   │  optional context file chips (e.g. active buffer name)     │
//!   │  Transcript (tail scroll) + compose + model row + term      │
//!   ├── multiline input (fixed min height) ─────────────────────┤
//!   ├── bottom row: model selector + [⌘↵] [Send] (36 px) ────────┤
//!   ├── drag handle (5 px) ──────────────────────────────────────┤
//!   │  Terminal (terminal_fraction of panel height)             │
//!   └────────────────────────────────────────────────────────────┘
//!
//! Conversation text is drawn in the transcript region above the input box.

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::icons::{paint_icon, Icon};
use crate::text_fit;
use crate::theme::palette as pal;

// ── Layout constants (logical px) ──────────────────────────────────────────

pub const AGENT_PANEL_WIDTH: f32 = 360.0;
pub const AGENT_PANEL_MIN_WIDTH: f32 = 320.0;
pub const AGENT_PANEL_MAX_WIDTH: f32 = 640.0;

const SESSION_TAB_ROW_H: f32 = 37.0;
const BOTTOM_ROW_H: f32 = 36.0;
const DRAG_H: f32 = 5.0;
const TERM_HEADER_H: f32 = 30.0;
/// Approximate logical px per character in monospace layout.
const CHAR_W: f32 = 7.2;
/// Logical px per line of chat text.
const LINE_H: f32 = 14.0;
/// Minimum height of the compose box (multiline input) in logical px.
const TYPED_INPUT_MIN_LOGICAL: f32 = 72.0;
/// Row height for CONTEXT label + file chips (logical px).
const CONTEXT_CHIP_ROW_H: f32 = 36.0;
/// Horizontal+vertical inset for transcript & compose (logical px, × scale at paint).
const PANEL_INNER_PAD: f32 = 18.0;

/// One @-file style row item: basename + dot color (reference: orange / purple / …).
#[derive(Debug, Clone)]
pub struct ContextChip {
    pub label: String,
    pub dot_rgba: [f32; 4],
}

// ── Chat display types (for future rich bubbles / context chips) ─────────────

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
        if self.visible {
            self.width * scale
        } else {
            0.0
        }
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
        if !self.visible {
            return None;
        }
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
        if !self.visible || !terminal_visible {
            return None;
        }
        let header_h = SESSION_TAB_ROW_H * scale;
        let bottom_row_h = BOTTOM_ROW_H * scale;
        let term_h = panel_height_px * self.terminal_fraction;
        let drag_h = DRAG_H * scale;
        let input_h =
            (panel_height_px - header_h - bottom_row_h - drag_h - term_h).max(40.0 * scale);
        Some(panel_top_px + header_h + input_h + bottom_row_h)
    }

    // ── Paint ───────────────────────────────────────────────────────────────

    /// Paint the full panel chrome into `frame`.
    ///
    /// `context_chips` are optional file labels (e.g. the active buffer basename) shown as pills
    /// under the session tabs, similar to Cursor’s @-file chips.
    ///
    /// Returns [`AgentPanelHits`] with all click regions for use by the main event loop.
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
        // Files in context (e.g. active + another open file); max ~4 short chips.
        context_chips: &[ContextChip],
        transcript: &[(String, [u8; 3])],
    ) -> AgentPanelHits {
        if !self.visible || height_px <= 0.5 {
            return AgentPanelHits::default();
        }
        let w = self.width * scale;
        let panel_clip = [left_px, top_px, left_px + w, top_px + height_px];

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
            self.paint_session_tabs(chrome, scale, left_px, top_px, w, tab_row_h, panel_clip);
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: top_px + tab_row_h - scale,
            width: w,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });

        let chip_row_h = if context_chips.is_empty() { 0.0 } else { CONTEXT_CHIP_ROW_H * scale };
        if chip_row_h > 0.0 {
            let chip_y = top_px + tab_row_h;
            self.paint_context_chip_row(
                chrome,
                scale,
                left_px,
                chip_y,
                w,
                chip_row_h,
                context_chips,
                panel_clip,
            );
            chrome.push_quad(ChromeQuad {
                left: left_px,
                top: chip_y + chip_row_h - scale,
                width: w,
                height: scale,
                rgba: pal::AGENT_BORDER,
            });
        }

        // ── Geometry: transcript + compose + bottom + drag + terminal ─────
        let term_h = if terminal_visible { height_px * self.terminal_fraction } else { 0.0 };
        let drag_h = if terminal_visible { DRAG_H * scale } else { 0.0 };
        let bottom_row_h = BOTTOM_ROW_H * scale;
        let chat_column_h =
            (height_px - tab_row_h - chip_row_h - bottom_row_h - drag_h - term_h).max(40.0 * scale);
        let typed_h = (TYPED_INPUT_MIN_LOGICAL * scale).min(chat_column_h * 0.42).max(48.0 * scale);
        let transcript_h = (chat_column_h - typed_h).max(0.0);

        let transcript_top = top_px + tab_row_h + chip_row_h;
        let input_top = transcript_top + transcript_h;
        let bottom_top = input_top + typed_h;

        // Transcript (same fill as the main editor for readable contrast).
        chrome.push_quad(ChromeQuad {
            left: left_px + scale,
            top: transcript_top,
            width: w - scale,
            height: transcript_h,
            rgba: pal::AGENT_TRANSCRIPT_BG,
        });
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: transcript_top + transcript_h - scale,
            width: w,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });

        let pad = PANEL_INNER_PAD * scale;
        let max_chars = ((self.width - 2.0 * PANEL_INNER_PAD - 8.0) / CHAR_W).max(8.0) as usize;
        let line_h = LINE_H * scale;
        let inner_top = transcript_top + 8.0 * scale;
        let inner_h = (transcript_h - 16.0 * scale).max(line_h);
        let max_lines = (inner_h / line_h).floor().max(1.0) as usize;
        let flat = Self::flatten_transcript_rich(transcript, max_chars);
        let start = flat.len().saturating_sub(max_lines);
        for (i, (text, rgb)) in flat[start..].iter().enumerate() {
            let y = inner_top + i as f32 * line_h;
            if y + line_h > transcript_top + transcript_h {
                break;
            }
            if !text.is_empty() {
                chrome.push_line_clipped(left_px + pad + scale, y, text.clone(), *rgb, panel_clip);
            }
        }

        // Compose box.
        chrome.push_quad(ChromeQuad {
            left: left_px + scale,
            top: input_top,
            width: w - scale,
            height: typed_h,
            rgba: pal::AGENT_INPUT_BG,
        });

        if input_focused {
            chrome.push_quad(ChromeQuad {
                left: left_px,
                top: input_top,
                width: 2.0 * scale,
                height: typed_h,
                rgba: pal::ACCENT_BLUE,
            });
        }

        if chat_input.is_empty() {
            chrome.push_line_clipped(
                left_px + pad + scale,
                input_top + 10.0 * scale,
                "Ask anything, or describe what to build\u{2026}".to_string(),
                pal::AGENT_HEADER_FG,
                panel_clip,
            );
        } else {
            let max_chars_f = (self.width - 2.0 * PANEL_INNER_PAD - 4.0) / CHAR_W;
            self.paint_input_text(
                chrome,
                scale,
                left_px + pad + scale,
                input_top + 8.0 * scale,
                max_chars_f,
                chat_input,
                chat_input_cursor,
                input_focused && blink_on,
                panel_clip,
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

        // Send (Cursor-style: violet pill + ➤ + label).
        let btn_h = 28.0 * scale;
        let btn_w = 88.0 * scale;
        let btn_x = left_px + w - pad - btn_w;
        let btn_y = bottom_top + (bottom_row_h - btn_h) / 2.0;
        chrome.push_quad(ChromeQuad {
            left: btn_x,
            top: btn_y,
            width: btn_w,
            height: btn_h,
            rgba: pal::AGENT_SEND_BG,
        });
        // Airplane (U+2708) + Send — reference-style “paper plane” affordance.
        chrome.push_line_clipped(
            btn_x + 10.0 * scale,
            btn_y + (btn_h - 10.0 * scale) / 2.0,
            "\u{2708}  Send".to_string(),
            [0xff, 0xff, 0xff],
            panel_clip,
        );

        // ⌘↵ hint.
        chrome.push_line_clipped(
            btn_x - 42.0 * scale,
            btn_y + (btn_h - 9.0 * scale) / 2.0,
            "\u{2318}\u{21b5}".to_string(),
            pal::AGENT_HEADER_FG,
            panel_clip,
        );

        // Model selector + star (reference: favorited / default model).
        let model_line = if active_model.is_empty() {
            "no model set".to_string()
        } else {
            format!("{active_model} \u{2605}")
        };
        chrome.push_line_clipped(
            left_px + pad + scale,
            bottom_top + (bottom_row_h - 9.0 * scale) / 2.0,
            model_line,
            if active_model.is_empty() { pal::AGENT_HEADER_FG } else { pal::ACCENT_TEXT },
            panel_clip,
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
            crate::terminal_header::paint_terminal_title_tabs(
                chrome,
                scale,
                left_px,
                th_top,
                w,
                TERM_HEADER_H * scale,
                0.0,
            );
        }

        AgentPanelHits {
            tab_hits,
            send_button: Some([btn_x, btn_y, btn_x + btn_w, btn_y + btn_h]),
            input_area: Some([left_px + scale, input_top, left_px + w, input_top + typed_h]),
            new_session_btn: new_btn_rect,
            drag_handle: drag_handle_y,
        }
    }

    // ── Internal renderers ───────────────────────────────────────────────────

    /// Cursor-style file pills under session tabs.
    fn paint_context_chip_row(
        &self,
        chrome: &mut FrameChrome,
        scale: f32,
        left_px: f32,
        top_px: f32,
        panel_w: f32,
        row_h: f32,
        chips: &[ContextChip],
        clip: [f32; 4],
    ) {
        chrome.push_quad(ChromeQuad {
            left: left_px + scale,
            top: top_px,
            width: panel_w - scale,
            height: row_h,
            rgba: pal::AGENT_BG,
        });
        let y_mid = top_px + (row_h - 9.0 * scale) / 2.0;
        let mut x = left_px + 10.0 * scale;
        if !chips.is_empty() {
            // Section label (reference: "CONTEXT" beside file chips).
            chrome.push_line_clipped(x, y_mid, "CONTEXT".to_string(), pal::SIDEBAR_HEADER_FG, clip);
            x += 56.0 * scale;
        }
        let y_chip = y_mid;
        let max_x = left_px + panel_w - 10.0 * scale;
        for ch in chips.iter().take(4) {
            if ch.label.is_empty() {
                continue;
            }
            let inner_pad = 8.0 * scale;
            let dot_w = 6.0 * scale;
            let close_afford = 14.0 * scale;
            // Fit label in remaining width; cap max chip width for dense rails.
            let max_chip_w = (max_x - x).min(232.0 * scale);
            let text_budget =
                (max_chip_w - dot_w - 4.0 * scale - 2.0 * inner_pad - close_afford - 2.0 * scale)
                    .max(8.0 * scale);
            let display = text_fit::ellipsize_mono(&ch.label, text_budget, scale, 6.8);
            let chip_w = (dot_w
                + 4.0 * scale
                + (display.chars().count() as f32) * 6.8 * scale
                + 2.0 * inner_pad
                + close_afford)
                .min(232.0 * scale);
            if x + chip_w > max_x {
                break;
            }
            let chip_h = row_h - 10.0 * scale;
            let chip_top = top_px + 5.0 * scale;
            // Border
            chrome.push_quad(ChromeQuad {
                left: x,
                top: chip_top,
                width: chip_w,
                height: chip_h,
                rgba: pal::AGENT_BORDER,
            });
            chrome.push_quad(ChromeQuad {
                left: x + scale,
                top: chip_top + scale,
                width: chip_w - 2.0 * scale,
                height: chip_h - 2.0 * scale,
                rgba: pal::AGENT_INPUT_BG,
            });
            // Top hairline — soft “lit edge” on dark chips.
            let hair = scale.max(1.0);
            chrome.push_quad(ChromeQuad {
                left: x + 2.0 * scale,
                top: chip_top + scale,
                width: chip_w - 4.0 * scale,
                height: hair,
                rgba: pal::rgba_u8(0xff, 0xff, 0xff, 0x0b),
            });
            // Violet left accent (reference: small purple marker on chips)
            chrome.push_quad(ChromeQuad {
                left: x + scale,
                top: chip_top + scale,
                width: 2.0 * scale,
                height: chip_h - 2.0 * scale,
                rgba: pal::ACCENT_BLUE,
            });
            paint_icon(
                chrome,
                Icon::Dot,
                x + 2.0 * scale + inner_pad + dot_w / 2.0,
                chip_top + chip_h / 2.0,
                dot_w,
                ch.dot_rgba,
            );
            chrome.push_line_clipped(
                x + 2.0 * scale + inner_pad + dot_w + 4.0 * scale,
                y_chip,
                display,
                pal::ACCENT_TEXT,
                clip,
            );
            let rgb = pal::AGENT_HEADER_FG;
            paint_icon(
                chrome,
                Icon::Close,
                x + chip_w - scale - close_afford * 0.45,
                chip_top + chip_h * 0.5,
                9.0 * scale,
                [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0],
            );
            x += chip_w + 6.0 * scale;
        }
    }

    fn flatten_transcript_rich(
        rows: &[(String, [u8; 3])],
        max_chars: usize,
    ) -> Vec<(String, [u8; 3])> {
        let mut out = Vec::new();
        for (s, c) in rows {
            for part in wrap_text(s, max_chars) {
                out.push((part, *c));
            }
        }
        out
    }

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
        clip: [f32; 4],
    ) {
        let line_h = LINE_H * scale;
        let max_c = (max_chars_f as usize).max(8);
        let lines: Vec<&str> = text.split('\n').collect();
        let mut byte_count = 0usize;
        let mut cursor_drawn = false;
        for (li, line) in lines.iter().enumerate() {
            let y = text_top + li as f32 * line_h;
            let display = if line.len() > max_c { &line[..max_c] } else { line };
            chrome.push_line_clipped(text_left, y, display.to_string(), pal::EDITOR_FG, clip);
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
        clip: [f32; 4],
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
            let tab_w = (tab_pad_h + dot_r * 2.0 + 6.0 * scale + label_w + close_w + tab_pad_h)
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

            // Status dot — active session = green (reference), else by state.
            let dot_x = x + tab_pad_h;
            let dot_y = top_px + (row_h - dot_r * 2.0) / 2.0;
            let dot_rgba =
                if is_active { pal::DIFF_ADDED } else { status_dot_color(session.status) };
            chrome.push_quad(ChromeQuad {
                left: dot_x,
                top: dot_y,
                width: dot_r * 2.0,
                height: dot_r * 2.0,
                rgba: dot_rgba,
            });

            let label_x = dot_x + dot_r * 2.0 + 6.0 * scale;
            let label_y = top_px + (row_h - 9.0 * scale) / 2.0;
            let fg = if is_active { pal::EDITOR_FG } else { pal::SIDEBAR_ROW_FG };
            chrome.push_line_clipped(label_x, label_y, session.label.clone(), fg, clip);

            let close_x = x + tab_w - close_w;
            chrome.push_line_clipped(
                close_x + 4.0 * scale,
                label_y,
                "\u{00d7}".to_string(),
                pal::AGENT_HEADER_FG,
                clip,
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
        chrome.push_line_clipped(
            new_btn_right + 8.0 * scale,
            top_px + (row_h - 10.0 * scale) / 2.0,
            "+".to_string(),
            pal::AGENT_HEADER_FG,
            clip,
        );
        let new_btn_rect = Some([new_btn_right, top_px, new_btn_right + new_btn_w, top_px + row_h]);

        (hits, new_btn_rect)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// RGBA color for a session status dot.
fn status_dot_color(status: AgentSessionStatus) -> [f32; 4] {
    match status {
        // Mockup: active session = green; idle tab = dim.
        AgentSessionStatus::Done | AgentSessionStatus::Running => pal::DIFF_ADDED,
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
