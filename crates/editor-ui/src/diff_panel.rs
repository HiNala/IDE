//! Diff-vs-HEAD floating panel (M18: Ctrl+Shift+D).
//!
//! Displays a unified diff of the active buffer's working content against its
//! last-committed HEAD version. Dismissed with Escape or toggled with the same
//! key chord.

use editor_diff::{Hunk, LineOp};

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::theme::palette as pal;

const CARD_W: f32 = 700.0;
const ROW_H: f32 = 14.0;
const VISIBLE_ROWS: usize = 28;
const PADDING: f32 = 12.0;
const HEADER_H: f32 = 28.0;

/// Kind of line rendered in the diff panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    HunkHeader,
    Added,
    Removed,
    Context,
}

/// One visual row in the diff panel.
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

/// Floating overlay showing a unified diff of the active buffer vs HEAD (M18).
#[derive(Debug, Default)]
pub struct DiffPanel {
    pub visible: bool,
    pub title: String,
    pub lines: Vec<DiffLine>,
    pub scroll: usize,
}

impl DiffPanel {
    /// Build panel content from a completed diff computation.
    ///
    /// `before_lines` and `after_lines` are the HEAD and working-copy lines
    /// respectively (as returned by `str::lines()`). `hunks` come from
    /// `editor_diff::compute_line_diff`.
    pub fn from_diff(title: &str, before: &str, after: &str, hunks: &[Hunk]) -> Self {
        let bl: Vec<&str> = before.lines().collect();
        let al: Vec<&str> = after.lines().collect();

        let mut lines: Vec<DiffLine> = Vec::new();

        if hunks.is_empty() {
            lines.push(DiffLine {
                kind: DiffLineKind::Context,
                text: "  (no changes vs HEAD)".into(),
            });
        }

        for hunk in hunks {
            let h = &hunk.header;
            lines.push(DiffLine {
                kind: DiffLineKind::HunkHeader,
                text: format!(
                    "@@ -{},{} +{},{} @@",
                    h.old_start, h.old_lines, h.new_start, h.new_lines
                ),
            });

            for op in &hunk.ops {
                match op {
                    LineOp::Equal { old_range, .. } => {
                        for i in old_range.clone() {
                            if let Some(t) = bl.get(i) {
                                lines.push(DiffLine {
                                    kind: DiffLineKind::Context,
                                    text: format!("  {t}"),
                                });
                            }
                        }
                    }
                    LineOp::Delete { old_range } => {
                        for i in old_range.clone() {
                            if let Some(t) = bl.get(i) {
                                lines.push(DiffLine {
                                    kind: DiffLineKind::Removed,
                                    text: format!("- {t}"),
                                });
                            }
                        }
                    }
                    LineOp::Insert { new_range } => {
                        for i in new_range.clone() {
                            if let Some(t) = al.get(i) {
                                lines.push(DiffLine {
                                    kind: DiffLineKind::Added,
                                    text: format!("+ {t}"),
                                });
                            }
                        }
                    }
                    LineOp::Replace { old_range, new_range, .. } => {
                        for i in old_range.clone() {
                            if let Some(t) = bl.get(i) {
                                lines.push(DiffLine {
                                    kind: DiffLineKind::Removed,
                                    text: format!("- {t}"),
                                });
                            }
                        }
                        for i in new_range.clone() {
                            if let Some(t) = al.get(i) {
                                lines.push(DiffLine {
                                    kind: DiffLineKind::Added,
                                    text: format!("+ {t}"),
                                });
                            }
                        }
                    }
                }
            }
        }

        Self { visible: true, title: title.to_string(), lines, scroll: 0 }
    }

    /// Scroll by `delta` rows, clamped to valid range.
    pub fn scroll_by(&mut self, delta: isize) {
        let max = self.lines.len().saturating_sub(VISIBLE_ROWS);
        self.scroll = (self.scroll as isize + delta).clamp(0, max as isize) as usize;
    }

    /// Paint the floating diff panel into `chrome`.
    pub fn paint(&self, chrome: &mut FrameChrome, scale: f32, viewport_w: f32, viewport_h: f32) {
        if !self.visible {
            return;
        }

        let card_w = CARD_W * scale;
        let row_h = ROW_H * scale;
        let header_h = HEADER_H * scale;
        let pad = PADDING * scale;

        let visible_count = VISIBLE_ROWS.min(self.lines.len());
        let card_h = header_h + pad + visible_count as f32 * row_h + pad;

        // Center the card.
        let left = ((viewport_w - card_w) / 2.0).max(0.0);
        let top = ((viewport_h - card_h) / 3.0).max(0.0);

        // Dim backdrop.
        chrome.push_quad(ChromeQuad {
            left: 0.0,
            top: 0.0,
            width: viewport_w,
            height: viewport_h,
            rgba: [0.0, 0.0, 0.0, 0.5],
        });

        // Card background.
        chrome.push_quad(ChromeQuad {
            left,
            top,
            width: card_w,
            height: card_h,
            rgba: pal::OVERLAY_BG,
        });

        // Card border.
        chrome.push_quad(ChromeQuad {
            left,
            top,
            width: card_w,
            height: 1.0 * scale,
            rgba: pal::OVERLAY_BORDER,
        });

        // Header background strip.
        chrome.push_quad(ChromeQuad {
            left,
            top,
            width: card_w,
            height: header_h,
            rgba: [0.08, 0.08, 0.12, 1.0],
        });

        // Title text.
        chrome.push_line(
            left + pad,
            top + (header_h - 10.0 * scale) / 2.0,
            self.title.clone(),
            [0xcc, 0xcc, 0xdd],
        );

        // Hint text — right-aligned.
        let hint = "Esc to close  ↑↓ scroll";
        chrome.push_line(
            left + card_w - pad - hint.len() as f32 * 5.5 * scale,
            top + (header_h - 10.0 * scale) / 2.0,
            hint.to_string(),
            pal::OVERLAY_FG,
        );

        // Diff lines.
        let content_top = top + header_h + pad;
        let visible = self.lines.iter().skip(self.scroll).take(VISIBLE_ROWS);
        for (i, dl) in visible.enumerate() {
            let y = content_top + i as f32 * row_h;

            let (bg, fg): (Option<[f32; 4]>, [u8; 3]) = match dl.kind {
                DiffLineKind::HunkHeader => (Some([0.18, 0.18, 0.28, 1.0]), [0x88, 0x88, 0xcc]),
                DiffLineKind::Added => (Some([0.05, 0.22, 0.09, 0.55]), [0x5d, 0xdd, 0x8e]),
                DiffLineKind::Removed => (Some([0.22, 0.05, 0.07, 0.55]), [0xe8, 0x5c, 0x6e]),
                DiffLineKind::Context => (None, [0x88, 0x88, 0x99]),
            };

            if let Some(bg_rgba) = bg {
                chrome.push_quad(ChromeQuad {
                    left,
                    top: y,
                    width: card_w,
                    height: row_h,
                    rgba: bg_rgba,
                });
            }

            // Truncate long lines at card width.
            let max_chars = ((card_w - pad * 2.0) / (6.5 * scale)) as usize;
            let text = if dl.text.len() > max_chars {
                format!("{}\u{2026}", &dl.text[..max_chars.saturating_sub(1)])
            } else {
                dl.text.clone()
            };
            chrome.push_line(left + pad, y + 1.0 * scale, text, fg);
        }

        // Scroll indicator — a subtle right-edge bar when there are more lines.
        if self.lines.len() > VISIBLE_ROWS {
            let total = self.lines.len() as f32;
            let frac_top = self.scroll as f32 / total;
            let frac_h = VISIBLE_ROWS as f32 / total;
            let track_h = visible_count as f32 * row_h;
            let bar_top = content_top + frac_top * track_h;
            let bar_h = (frac_h * track_h).max(20.0 * scale);
            chrome.push_quad(ChromeQuad {
                left: left + card_w - 3.0 * scale,
                top: bar_top,
                width: 3.0 * scale,
                height: bar_h,
                rgba: [0.4, 0.4, 0.6, 0.7],
            });
        }
    }
}
