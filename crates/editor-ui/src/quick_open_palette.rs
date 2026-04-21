//! Centered Ctrl+P fuzzy file picker (M14).

use std::path::{Path, PathBuf};

use editor_workspace::entry::FileEntry;
use editor_workspace::entry::FileKind;
use editor_workspace::Workspace;

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::quick_open::QuickOpenRanker;

const MAX_RESULTS: usize = 100;
const ROW_H: f32 = 22.0;
const CARD_W: f32 = 500.0;

/// Modal quick-open state.
#[derive(Debug)]
pub struct QuickOpenPalette {
    pub visible: bool,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    ranker: QuickOpenRanker,
    /// Workspace-relative posix paths.
    paths: Vec<String>,
    ranked_indices: Vec<usize>,
}

impl Default for QuickOpenPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl QuickOpenPalette {
    #[must_use]
    pub fn new() -> Self {
        Self {
            visible: false,
            query: String::new(),
            selected: 0,
            scroll: 0,
            ranker: QuickOpenRanker::new(),
            paths: Vec::new(),
            ranked_indices: Vec::new(),
        }
    }

    pub fn set_workspace_files(&mut self, _workspace: &Workspace, entries: &[FileEntry]) {
        self.paths = entries
            .iter()
            .filter(|e| e.kind == FileKind::Regular && !e.is_binary_heuristic)
            .map(|e| e.relative.to_string_lossy().replace('\\', "/"))
            .collect();
        self.paths.sort();
        self.rerank();
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

    pub fn set_query(&mut self, q: String) {
        self.query = q;
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
        if self.paths.is_empty() {
            self.ranked_indices.clear();
            return;
        }
        self.ranked_indices = self.ranker.rank_paths(&self.query, &self.paths, MAX_RESULTS);
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.ranked_indices.is_empty() {
            return;
        }
        let n = self.ranked_indices.len();
        let i = (self.selected as isize + delta).clamp(0, n as isize - 1) as usize;
        self.selected = i;
        let vis = 12usize;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + vis {
            self.scroll = self.selected + 1 - vis;
        }
    }

    #[must_use]
    pub fn selected_path(&self) -> Option<PathBuf> {
        let i = self.ranked_indices.get(self.selected)?;
        self.paths.get(*i).map(PathBuf::from)
    }

    #[must_use]
    pub fn selected_absolute(&self, workspace_root: &Path) -> Option<PathBuf> {
        let rel = self.selected_path()?;
        Some(workspace_root.join(rel))
    }

    /// Dim overlay + card + result rows (`physical_h` for layout).
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
        let vis = 12usize;
        let ch = (40.0 + vis as f32 * ROW_H) * scale;
        let cx = (physical_w - cw) * 0.5;
        let cy = physical_h * 0.15;
        chrome.push_quad(ChromeQuad {
            left: cx,
            top: cy,
            width: cw,
            height: ch,
            rgba: [0.12, 0.12, 0.14, 1.0],
        });
        let q = format!("> {}", self.query);
        chrome.push_line(cx + 10.0 * scale, cy + 8.0 * scale, q, [0xe8, 0xe8, 0xec]);
        let row_h = ROW_H * scale;
        let mut y = cy + 36.0 * scale;
        let slice = self.ranked_indices.get(self.scroll..).unwrap_or(&[]);
        for (i, file_idx) in slice.iter().take(vis).enumerate() {
            let global = self.scroll + i;
            let sel = global == self.selected;
            let path = &self.paths[*file_idx];
            if sel {
                chrome.push_quad(ChromeQuad {
                    left: cx + 4.0 * scale,
                    top: y - 2.0 * scale,
                    width: cw - 8.0 * scale,
                    height: row_h,
                    rgba: [0.2, 0.35, 0.75, 0.35],
                });
            }
            chrome.push_line(
                cx + 10.0 * scale,
                y,
                path.clone(),
                if sel { [0xff, 0xff, 0xff] } else { [0xc8, 0xc8, 0xd0] },
            );
            y += row_h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_hidden() {
        let p = QuickOpenPalette::new();
        assert!(!p.visible);
    }
}
