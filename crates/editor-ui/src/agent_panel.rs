//! Right-side agent panel: session tabs, AI chat input, and terminal controls.
//!
//! Layout (top → bottom):
//!   ┌─ session tab row (37 px) ───────────────────────────────┐
//!   │  [M21 Sidecar ✓] [M25 Fixes ●] [M28 Settings ○] [+]    │
//!   ├─ context pills row (28 px) ─────────────────────────────┤
//!   │  [sidecar.rs ×] [+]                                     │
//!   ├─ chat textarea (flex) ──────────────────────────────────┤
//!   ├─ model selector · [⌘↵] [Send] row (36 px) ─────────────┤
//!   ├─ drag handle (5 px) ────────────────────────────────────┤
//!   │  Terminal header                                         │
//!   └──────────────────────────────────────────────────────────┘

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::theme::palette as pal;

/// Default logical width of the agent panel.
pub const AGENT_PANEL_WIDTH: f32 = 440.0;
/// Minimum logical width.
pub const AGENT_PANEL_MIN_WIDTH: f32 = 280.0;
/// Maximum logical width.
pub const AGENT_PANEL_MAX_WIDTH: f32 = 720.0;

const SESSION_TAB_ROW_H: f32 = 37.0;
const CONTEXT_ROW_H: f32 = 28.0;
const BOTTOM_ROW_H: f32 = 36.0;
const DRAG_H: f32 = 5.0;
const TERM_HEADER_H: f32 = 30.0;

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
    /// Stable handle (monotone counter).
    pub id: u64,
    /// Short display label shown in the tab.
    pub label: String,
    /// Current execution state.
    pub status: AgentSessionStatus,
}

impl AgentSession {
    #[must_use]
    pub fn new(id: u64, label: impl Into<String>, status: AgentSessionStatus) -> Self {
        Self { id, label: label.into(), status }
    }
}

/// Hit region for a session tab click.
#[derive(Debug, Clone)]
pub struct AgentTabHit {
    /// Index into [`AgentPanel::sessions`].
    pub session_idx: usize,
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    /// True when the pointer hit the close (×) button.
    pub is_close: bool,
}

/// Persistent state for the agent panel.
#[derive(Debug)]
pub struct AgentPanel {
    /// Logical pixel width of the panel.
    pub width: f32,
    /// Whether the panel is currently shown.
    pub visible: bool,
    /// Fraction of panel height allocated to the terminal section (0..1).
    pub terminal_fraction: f32,
    /// Agent conversation sessions — displayed as tabs at the top of the panel.
    pub sessions: Vec<AgentSession>,
    /// Index of the currently-selected session.
    pub active_session: usize,
    /// Next session id to assign.
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
        // Seed with the three canonical V3 sessions from the design.
        panel.add_session("M21 Sidecar", AgentSessionStatus::Done);
        panel.add_session("M25 Fixes", AgentSessionStatus::Running);
        panel.add_session("M28 Settings", AgentSessionStatus::Queued);
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

    /// Paint the agent panel chrome into `frame`.
    ///
    /// Returns hit regions for session tabs (for click routing in main.rs).
    ///
    /// * `left_px`    — physical X of the panel's left edge.
    /// * `top_px`     — physical Y of the panel's top (usually 0, below the title bar if any).
    /// * `height_px`  — physical height available (above status bar).
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        chrome: &mut FrameChrome,
        scale: f32,
        left_px: f32,
        top_px: f32,
        height_px: f32,
        terminal_visible: bool,
    ) -> Vec<AgentTabHit> {
        if !self.visible || height_px <= 0.5 {
            return Vec::new();
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

        // ── Session tab row ──────────────────────────────────────────────────
        let tab_row_h = SESSION_TAB_ROW_H * scale;
        let tab_hits = self.paint_session_tabs(chrome, scale, left_px, top_px, w, tab_row_h);

        // Tab row bottom border.
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: top_px + tab_row_h - scale,
            width: w,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });

        // ── Context pills row ────────────────────────────────────────────────
        let ctx_row_top = top_px + tab_row_h;
        let ctx_row_h = CONTEXT_ROW_H * scale;
        self.paint_context_row(chrome, scale, left_px, ctx_row_top, w, ctx_row_h);

        // Context row bottom border.
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: ctx_row_top + ctx_row_h - scale,
            width: w,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });

        // ── Input area layout ────────────────────────────────────────────────
        let term_h = if terminal_visible { height_px * self.terminal_fraction } else { 0.0 };
        let drag_h = if terminal_visible { DRAG_H * scale } else { 0.0 };
        let term_header_h = if terminal_visible { TERM_HEADER_H * scale } else { 0.0 };
        let bottom_row_h = BOTTOM_ROW_H * scale;
        let header_h = tab_row_h + ctx_row_h;
        let input_h = (height_px - header_h - bottom_row_h - drag_h - term_h).max(60.0 * scale);
        let input_top = top_px + header_h;

        // Textarea background.
        let pad = 12.0 * scale;
        chrome.push_quad(ChromeQuad {
            left: left_px + scale,
            top: input_top,
            width: w - scale,
            height: input_h,
            rgba: pal::AGENT_INPUT_BG,
        });

        // Placeholder text.
        chrome.push_line(
            left_px + pad + scale,
            input_top + 10.0 * scale,
            "Ask anything, or describe what to build\u{2026}".to_string(),
            pal::AGENT_HEADER_FG,
        );

        // ── Bottom row: model selector + send button ─────────────────────────
        let bottom_top = input_top + input_h;
        chrome.push_quad(ChromeQuad {
            left: left_px + scale,
            top: bottom_top,
            width: w - scale,
            height: bottom_row_h,
            rgba: pal::AGENT_BG,
        });
        // Separator above bottom row.
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: bottom_top,
            width: w,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });

        // Send button (right-aligned).
        let btn_h = 26.0 * scale;
        let btn_w = 58.0 * scale;
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
            btn_x + 14.0 * scale,
            btn_y + (btn_h - 10.0 * scale) / 2.0,
            "Send".to_string(),
            [0xff, 0xff, 0xff],
        );

        // ⌘↵ hint left of the send button.
        chrome.push_line(
            btn_x - 38.0 * scale,
            btn_y + (btn_h - 9.0 * scale) / 2.0,
            "\u{2318}\u{21b5}".to_string(),
            pal::AGENT_HEADER_FG,
        );

        // Model selector (left side).
        chrome.push_line(
            left_px + pad + scale,
            bottom_top + (bottom_row_h - 9.0 * scale) / 2.0,
            "Claude Opus 4.7".to_string(),
            pal::AGENT_HEADER_FG,
        );

        if !terminal_visible {
            return tab_hits;
        }

        // ── Drag handle ──────────────────────────────────────────────────────
        let drag_top = bottom_top + bottom_row_h;
        chrome.push_quad(ChromeQuad {
            left: left_px + scale,
            top: drag_top + drag_h * 0.5 - 0.5 * scale,
            width: w - scale,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });

        // ── Terminal section header ──────────────────────────────────────────
        let th_top = drag_top + drag_h;
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: th_top,
            width: w,
            height: term_header_h,
            rgba: pal::AGENT_BG,
        });
        chrome.push_quad(ChromeQuad {
            left: left_px,
            top: th_top + term_header_h - scale,
            width: w,
            height: scale,
            rgba: pal::AGENT_BORDER,
        });
        chrome.push_line(
            left_px + 14.0 * scale,
            th_top + (term_header_h - 9.0 * scale) / 2.0,
            "Terminal".to_string(),
            pal::AGENT_HEADER_FG,
        );

        tab_hits
    }

    /// Paint the session tab row and return hit regions.
    fn paint_session_tabs(
        &self,
        chrome: &mut FrameChrome,
        scale: f32,
        left_px: f32,
        top_px: f32,
        panel_w: f32,
        row_h: f32,
    ) -> Vec<AgentTabHit> {
        let mut hits = Vec::new();
        let mut x = left_px + scale; // start just after the left border

        let tab_pad_h = 10.0 * scale; // horizontal padding inside each tab
        let dot_r = 4.0 * scale; // status dot radius (drawn as a small square)
        let close_w = 18.0 * scale; // width of the × close zone
        let min_tab_w = 80.0 * scale;
        let new_btn_w = 26.0 * scale;
        let new_btn_right = left_px + panel_w - scale - new_btn_w;

        for (i, session) in self.sessions.iter().enumerate() {
            let is_active = i == self.active_session;

            // Estimate tab width from label length (monospace approximation).
            let label_w = session.label.len() as f32 * 7.0 * scale;
            let tab_w = (tab_pad_h + dot_r * 2.0 + 6.0 * scale + label_w + close_w + tab_pad_h)
                .max(min_tab_w);

            // Don't overflow into the + button area.
            if x + tab_w > new_btn_right - 4.0 * scale {
                break;
            }

            // Tab background: active tabs slightly lighter, inactive = panel bg.
            let tab_bg = if is_active { pal::AGENT_INPUT_BG } else { pal::AGENT_BG };
            chrome.push_quad(ChromeQuad {
                left: x,
                top: top_px,
                width: tab_w,
                height: row_h,
                rgba: tab_bg,
            });

            // Active tab: violet underline at the bottom.
            if is_active {
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
            let dot_color = status_dot_color(session.status);
            chrome.push_quad(ChromeQuad {
                left: dot_x,
                top: dot_y,
                width: dot_r * 2.0,
                height: dot_r * 2.0,
                rgba: dot_color,
            });

            // Tab label.
            let label_x = dot_x + dot_r * 2.0 + 6.0 * scale;
            let label_y = top_px + (row_h - 9.0 * scale) / 2.0;
            let fg = if is_active { pal::EDITOR_FG } else { pal::SIDEBAR_ROW_FG };
            chrome.push_line(label_x, label_y, session.label.clone(), fg);

            // Close (×) button zone.
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
            // Also track the close button as a separate entry with is_close=true.
            hits.push(AgentTabHit {
                session_idx: i,
                x0: close_x,
                x1: x + tab_w,
                y0: top_px,
                y1: top_px + row_h,
                is_close: true,
            });

            // Right separator between tabs.
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
            new_btn_right + 7.0 * scale,
            top_px + (row_h - 10.0 * scale) / 2.0,
            "+".to_string(),
            pal::AGENT_HEADER_FG,
        );

        hits
    }

    /// Paint the context pills row (file context pins).
    fn paint_context_row(
        &self,
        chrome: &mut FrameChrome,
        scale: f32,
        left_px: f32,
        top_px: f32,
        panel_w: f32,
        row_h: f32,
    ) {
        let pad = 8.0 * scale;
        let mut x = left_px + scale + pad;

        // "Context:" label.
        chrome.push_line(
            x,
            top_px + (row_h - 9.0 * scale) / 2.0,
            "Context:".to_string(),
            pal::AGENT_HEADER_FG,
        );
        x += 56.0 * scale;

        // Sample context pills — in V3 these will come from the active session's context list.
        for pill_label in &["sidecar.rs", "atomic.rs"] {
            let pill_w = (pill_label.len() as f32 * 7.0 * scale + 20.0 * scale).max(60.0 * scale);
            if x + pill_w > left_px + panel_w - pad {
                break;
            }
            // Pill background.
            chrome.push_quad(ChromeQuad {
                left: x,
                top: top_px + 5.0 * scale,
                width: pill_w,
                height: row_h - 10.0 * scale,
                rgba: pal::ACCENT_TINT,
            });
            chrome.push_line(
                x + 6.0 * scale,
                top_px + (row_h - 9.0 * scale) / 2.0,
                (*pill_label).to_string(),
                pal::ACCENT_TEXT,
            );
            x += pill_w + 4.0 * scale;
        }

        // "+" to add more context.
        chrome.push_line(
            x + 4.0 * scale,
            top_px + (row_h - 9.0 * scale) / 2.0,
            "+".to_string(),
            pal::AGENT_HEADER_FG,
        );
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
        let header_h = (SESSION_TAB_ROW_H + CONTEXT_ROW_H) * scale;
        let bottom_row_h = BOTTOM_ROW_H * scale;
        let term_h = panel_height_px * self.terminal_fraction;
        let drag_h = DRAG_H * scale;
        let input_h =
            (panel_height_px - header_h - bottom_row_h - drag_h - term_h).max(60.0 * scale);
        Some(panel_top_px + header_h + input_h + bottom_row_h)
    }

    /// Left X (physical px) of the resize drag edge.
    #[must_use]
    pub fn resize_edge_x_px(&self, scale: f32, window_width_px: f32) -> Option<f32> {
        if !self.visible {
            return None;
        }
        Some(window_width_px - self.width_px(scale))
    }
}

/// RGBA color for a session status dot.
fn status_dot_color(status: AgentSessionStatus) -> [f32; 4] {
    match status {
        AgentSessionStatus::Done => pal::DIFF_ADDED,     // green
        AgentSessionStatus::Running => pal::ACCENT_BLUE, // violet
        AgentSessionStatus::Queued => {
            [0x3a as f32 / 255.0, 0x3a as f32 / 255.0, 0x52 as f32 / 255.0, 1.0]
        }
    }
}

/// Paint a 1-px border rectangle as 4 edge quads.
#[allow(dead_code)]
fn paint_rect_border(
    chrome: &mut FrameChrome,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale: f32,
    rgba: [f32; 4],
) {
    let t = scale;
    chrome.push_quad(ChromeQuad { left: x, top: y, width: w, height: t, rgba });
    chrome.push_quad(ChromeQuad { left: x, top: y + h - t, width: w, height: t, rgba });
    chrome.push_quad(ChromeQuad { left: x, top: y, width: t, height: h, rgba });
    chrome.push_quad(ChromeQuad { left: x + w - t, top: y, width: t, height: h, rgba });
}
