//! Ctrl+Shift+P command palette: a discoverable, searchable list of every
//! in-app action.
//!
//! Data flow: the app populates the palette once at startup with
//! `[CommandEntry]` entries (title + optional keybinding hint + opaque id).
//! When visible, the palette fuzzy-filters entries with the same ranker used
//! by quick-open, returns the selected id on `Enter`, and gets dismissed on
//! `Escape`.
//!
//! The palette stays UI-only: it does not know what commands *do* — the caller
//! dispatches whatever identifier comes back out.

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::icons::{paint_icon, Icon};
use crate::quick_open::QuickOpenRanker;
use crate::theme::palette;

const MAX_RESULTS: usize = 200;
const ROW_H: f32 = 22.0;
const CARD_W: f32 = 560.0;
const CARD_MAX_ROWS: usize = 14;

/// One entry in the command palette.
///
/// `id` is an opaque discriminator the app maps back to its `EditorCommand`
/// variant. `title` is the user-visible label. `hint` is an optional
/// keybinding string (e.g. `"Ctrl+S"`) rendered right-aligned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry {
    pub id: &'static str,
    pub title: String,
    pub hint: Option<String>,
}

/// State + layout for the Ctrl+Shift+P palette.
#[derive(Debug)]
pub struct CommandPalette {
    pub visible: bool,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    entries: Vec<CommandEntry>,
    search_strings: Vec<String>,
    ranker: QuickOpenRanker,
    ranked_indices: Vec<usize>,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    #[must_use]
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            selected: 0,
            scroll: 0,
            entries: Vec::new(),
            search_strings: Vec::new(),
            ranker: QuickOpenRanker::new(),
            ranked_indices: Vec::new(),
        }
    }

    /// Seed the palette with the app's full command inventory.
    ///
    /// Safe to call more than once; the palette simply re-ranks against the
    /// current query.
    pub fn set_entries(&mut self, entries: Vec<CommandEntry>) {
        self.search_strings = entries
            .iter()
            .map(|e| {
                // Include the hint in the search corpus so users can type a
                // keybinding like "Ctrl+S" and find the matching command.
                match &e.hint {
                    Some(h) => format!("{} {}", e.title, h),
                    None => e.title.clone(),
                }
            })
            .collect();
        self.entries = entries;
        self.rerank();
    }

    /// Number of entries registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.selected = 0;
        self.scroll = 0;
        self.rerank();
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.scroll = 0;
        self.rerank();
    }

    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.selected = 0;
        self.scroll = 0;
        self.rerank();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.selected = 0;
        self.scroll = 0;
        self.rerank();
    }

    fn rerank(&mut self) {
        if self.entries.is_empty() {
            self.ranked_indices.clear();
            return;
        }
        if self.query.is_empty() {
            // No query: show every entry in registration order.
            self.ranked_indices = (0..self.entries.len()).collect();
            return;
        }
        self.ranked_indices =
            self.ranker.rank_paths(&self.query, &self.search_strings, MAX_RESULTS);
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.ranked_indices.is_empty() {
            return;
        }
        let n = self.ranked_indices.len();
        let i = (self.selected as isize + delta).clamp(0, n as isize - 1) as usize;
        self.selected = i;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + CARD_MAX_ROWS {
            self.scroll = self.selected + 1 - CARD_MAX_ROWS;
        }
    }

    /// The command id currently highlighted, if any. Returns `None` when the
    /// palette is visible but no entries match the query.
    #[must_use]
    pub fn selected_id(&self) -> Option<&'static str> {
        let i = *self.ranked_indices.get(self.selected)?;
        Some(self.entries[i].id)
    }

    /// Dim overlay + centered card + rows. Safe to call when `!visible`
    /// (returns immediately).
    pub fn paint(&self, chrome: &mut FrameChrome, scale: f32, physical_w: f32, physical_h: f32) {
        if !self.visible {
            return;
        }
        chrome.push_quad(ChromeQuad {
            left: 0.0,
            top: 0.0,
            width: physical_w,
            height: physical_h,
            rgba: [0.0, 0.0, 0.0, 0.45],
        });

        let cw = CARD_W * scale;
        let vis = CARD_MAX_ROWS;
        let ch = (40.0 + vis as f32 * ROW_H) * scale;
        let cx = (physical_w - cw) * 0.5;
        let cy = physical_h * 0.12;
        chrome.push_quad(ChromeQuad {
            left: cx,
            top: cy,
            width: cw,
            height: ch,
            rgba: palette::OVERLAY_BG,
        });
        // Hairline border.
        chrome.push_quad(ChromeQuad {
            left: cx,
            top: cy,
            width: cw,
            height: scale.max(1.0),
            rgba: palette::OVERLAY_BORDER,
        });
        chrome.push_quad(ChromeQuad {
            left: cx,
            top: cy + ch - scale.max(1.0),
            width: cw,
            height: scale.max(1.0),
            rgba: palette::OVERLAY_BORDER,
        });

        // Chevron icon next to the query, then the query text.
        let chevron_size = 14.0 * scale;
        let icon_cx = cx + (12.0 + chevron_size / 2.0) * scale - scale * 2.0;
        let icon_cy = cy + 18.0 * scale;
        paint_icon(
            chrome,
            Icon::ChevronRight,
            icon_cx,
            icon_cy,
            chevron_size,
            [0.92, 0.92, 0.95, 1.0],
        );
        let q_text = if self.query.is_empty() {
            "Type a command...".to_string()
        } else {
            self.query.clone()
        };
        chrome.push_line(
            cx + 30.0 * scale,
            cy + 10.0 * scale,
            q_text,
            if self.query.is_empty() { [0x7a, 0x7a, 0x7a] } else { [0xe8, 0xe8, 0xec] },
        );

        let row_h = ROW_H * scale;
        let mut y = cy + 36.0 * scale;
        let slice = self.ranked_indices.get(self.scroll..).unwrap_or(&[]);
        for (i, entry_idx) in slice.iter().take(vis).enumerate() {
            let global = self.scroll + i;
            let sel = global == self.selected;
            let Some(entry) = self.entries.get(*entry_idx) else { continue };
            if sel {
                chrome.push_quad(ChromeQuad {
                    left: cx + 4.0 * scale,
                    top: y - 2.0 * scale,
                    width: cw - 8.0 * scale,
                    height: row_h,
                    rgba: palette::SIDEBAR_ROW_FOCUS,
                });
            }
            let text_rgb = if sel { [0xff, 0xff, 0xff] } else { [0xc8, 0xc8, 0xd0] };
            chrome.push_line(cx + 14.0 * scale, y, entry.title.clone(), text_rgb);
            if let Some(hint) = entry.hint.as_deref() {
                // Right-align the hint — approximate char width for monospace.
                let hint_w = hint.chars().count() as f32 * 7.2 * scale;
                chrome.push_line(
                    cx + cw - hint_w - 16.0 * scale,
                    y,
                    hint.to_string(),
                    [0x85, 0x85, 0x85],
                );
            }
            y += row_h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &'static str, title: &str, hint: Option<&str>) -> CommandEntry {
        CommandEntry { id, title: title.into(), hint: hint.map(String::from) }
    }

    fn seed() -> CommandPalette {
        let mut p = CommandPalette::new();
        p.set_entries(vec![
            entry("save", "File: Save", Some("Ctrl+S")),
            entry("save_as", "File: Save As", None),
            entry("find", "Edit: Find", Some("Ctrl+F")),
            entry("quit", "Quit", Some("Ctrl+Q")),
        ]);
        p
    }

    #[test]
    fn new_hidden_and_empty() {
        let p = CommandPalette::new();
        assert!(!p.visible);
        assert!(p.is_empty());
        assert!(p.selected_id().is_none());
    }

    #[test]
    fn show_ranks_every_entry_when_query_empty() {
        let mut p = seed();
        p.show();
        assert!(p.visible);
        assert_eq!(p.selected_id(), Some("save"));
        assert_eq!(p.len(), 4);
    }

    #[test]
    fn query_narrows_results() {
        let mut p = seed();
        p.show();
        for c in "find".chars() {
            p.push_char(c);
        }
        assert_eq!(p.selected_id(), Some("find"));
    }

    #[test]
    fn keybind_hint_is_searchable() {
        let mut p = seed();
        p.show();
        for c in "Ctrl+Q".chars() {
            p.push_char(c);
        }
        assert_eq!(p.selected_id(), Some("quit"));
    }

    #[test]
    fn selection_moves_and_clamps() {
        let mut p = seed();
        p.show();
        p.move_selection(1);
        assert_eq!(p.selected_id(), Some("save_as"));
        p.move_selection(100);
        // Last entry in registration order is "quit".
        assert_eq!(p.selected_id(), Some("quit"));
        p.move_selection(-999);
        assert_eq!(p.selected_id(), Some("save"));
    }

    #[test]
    fn backspace_shrinks_query() {
        let mut p = seed();
        p.show();
        p.push_char('s');
        p.push_char('a');
        p.backspace();
        assert_eq!(p.query, "s");
    }
}
