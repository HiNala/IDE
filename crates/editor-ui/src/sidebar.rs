//! Collapsible project file tree (M14).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use editor_workspace::entry::{FileEntry, FileKind};
use editor_workspace::BufferManager;

use crate::chrome::{ChromeQuad, FrameChrome};

/// Default sidebar width (logical px).
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;
/// Row height in logical pixels (before scale).
pub const ROW_LINE_HEIGHT: f32 = 22.0;
/// Logical height of the header strip above rows.
pub const HEADER_HEIGHT: f32 = 32.0;
const INDENT_PER_DEPTH: f32 = 14.0;
const LEFT_PAD: f32 = 12.0;
// VS Code Dark+ palette.
const BG_RGBA: [f32; 4] = [0.145, 0.145, 0.149, 1.0]; // #252526
const ROW_HL_RGBA: [f32; 4] = [0.165, 0.178, 0.184, 1.0]; // #2a2d2e
const ROW_FOCUS_RGBA: [f32; 4] = [0.024, 0.31, 0.54, 0.85]; // #04558a (focused row on sidebar)
const HEADER_BG_RGBA: [f32; 4] = [0.145, 0.145, 0.149, 1.0];
const HEADER_RGB: [u8; 3] = [0xBB, 0xBB, 0xBB];
const TEXT_RGB: [u8; 3] = [0xCC, 0xCC, 0xCC];
const TEXT_DIM: [u8; 3] = [0x85, 0x85, 0x85];
const ACCENT: [u8; 3] = [0xFF, 0xFF, 0xFF];

/// One visible row in the flattened tree.
#[derive(Debug, Clone)]
pub struct FlatRow {
    pub rel: PathBuf,
    pub depth: u16,
    pub is_dir: bool,
    /// File or directory name (last segment).
    pub label: String,
    pub has_children: bool,
}

/// Sidebar state: width, scroll, expanded dirs, keyboard highlight.
#[derive(Debug)]
pub struct Sidebar {
    pub width: f32,
    pub visible: bool,
    pub scroll_y: f32,
    pub expanded_dirs: HashSet<PathBuf>,
    /// Keyboard focus (arrow navigation).
    pub highlighted: Option<PathBuf>,
    pub focused: bool,
    flat_rows: Vec<FlatRow>,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl Sidebar {
    #[must_use]
    pub fn new() -> Self {
        let mut expanded_dirs = HashSet::new();
        expanded_dirs.insert(PathBuf::new());
        Self {
            width: DEFAULT_SIDEBAR_WIDTH,
            visible: false,
            scroll_y: 0.0,
            expanded_dirs,
            highlighted: None,
            focused: false,
            flat_rows: Vec::new(),
        }
    }

    /// Expand ancestors so `rel` can appear; does not toggle focus.
    pub fn reveal_path(&mut self, rel: impl AsRef<Path>) {
        let rel = rel.as_ref();
        let mut p = rel.to_path_buf();
        while let Some(parent) = p.parent() {
            if parent.as_os_str().is_empty() {
                break;
            }
            self.expanded_dirs.insert(parent.to_path_buf());
            p = parent.to_path_buf();
        }
        self.expanded_dirs.insert(PathBuf::new());
    }

    fn row_visible(rel: &Path, expanded: &HashSet<PathBuf>) -> bool {
        let mut pb = rel.to_path_buf();
        while let Some(p) = pb.parent() {
            if p.as_os_str().is_empty() {
                break;
            }
            let key = p.to_path_buf();
            if !expanded.contains(&key) {
                return false;
            }
            pb = key;
        }
        true
    }

    /// Rebuild [`Self::flat_rows`] from workspace scan results.
    pub fn rebuild_flat(&mut self, entries: &[FileEntry]) {
        self.flat_rows.clear();
        let mut v: Vec<&FileEntry> = entries.iter().collect();
        v.sort_by(|a, b| a.relative.to_string_lossy().cmp(&b.relative.to_string_lossy()));

        for e in v {
            if e.kind == FileKind::Symlink {
                continue;
            }
            if !Self::row_visible(&e.relative, &self.expanded_dirs) {
                continue;
            }
            let depth = e.relative.components().count().saturating_sub(1) as u16;
            let label = e
                .relative
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| e.relative.to_string_lossy().into_owned());
            let has_children = e.kind == FileKind::Directory
                && entries.iter().any(|o| {
                    o.relative.starts_with(&e.relative)
                        && o.relative != e.relative
                        && o.relative
                            .strip_prefix(&e.relative)
                            .ok()
                            .and_then(|p| p.components().next())
                            .is_some()
                });
            self.flat_rows.push(FlatRow {
                rel: e.relative.clone(),
                depth,
                is_dir: e.kind == FileKind::Directory,
                label,
                has_children,
            });
        }
    }

    #[must_use]
    pub fn flat_rows(&self) -> &[FlatRow] {
        &self.flat_rows
    }

    pub fn toggle_dir(&mut self, rel: &Path) {
        let p = rel.to_path_buf();
        if self.expanded_dirs.contains(&p) {
            self.expanded_dirs.remove(&p);
        } else {
            self.expanded_dirs.insert(p);
        }
    }

    #[must_use]
    pub fn is_expanded(&self, rel: &Path) -> bool {
        self.expanded_dirs.contains(&rel.to_path_buf())
    }

    /// Row index at window Y, or None. `origin_y` is the top of the **rows area**
    /// (header already accounted for by the caller).
    pub fn row_index_at_y(&self, y_px: f32, scale: f32, origin_y: f32) -> Option<usize> {
        if !self.visible {
            return None;
        }
        let lh = ROW_LINE_HEIGHT * scale;
        let rel_y = y_px - origin_y + self.scroll_y;
        if rel_y < 0.0 {
            return None;
        }
        let i = (rel_y / lh).floor() as usize;
        (i < self.flat_rows.len()).then_some(i)
    }

    /// Paint sidebar into `chrome` (quads + text).
    ///
    /// `origin_x` is the left edge of the sidebar (usually the activity bar's right edge).
    /// `origin_y` / `viewport_h` cover the full sidebar column including the header.
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        chrome: &mut FrameChrome,
        buffers: &BufferManager,
        workspace_root: Option<&Path>,
        auto_highlight_rel: Option<&Path>,
        scale: f32,
        origin_x: f32,
        origin_y: f32,
        viewport_h: f32,
    ) {
        if !self.visible {
            return;
        }
        let w = self.width * scale;
        let h = viewport_h.max(1.0);
        // Column background.
        chrome.push_quad(ChromeQuad {
            left: origin_x,
            top: origin_y,
            width: w,
            height: h,
            rgba: BG_RGBA,
        });

        // Header strip ("EXPLORER"). Slightly lighter than the column, tiny uppercase label.
        let header_h = HEADER_HEIGHT * scale;
        chrome.push_quad(ChromeQuad {
            left: origin_x,
            top: origin_y,
            width: w,
            height: header_h,
            rgba: HEADER_BG_RGBA,
        });
        let header_label = workspace_root
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_uppercase())
            .unwrap_or_else(|| "EXPLORER".to_string());
        chrome.push_line(
            origin_x + LEFT_PAD * scale,
            origin_y + (header_h - 11.0 * scale) / 2.0,
            header_label,
            HEADER_RGB,
        );

        let rows_top = origin_y + header_h;
        let lh = ROW_LINE_HEIGHT * scale;
        let mut y = rows_top - self.scroll_y;
        for row in &self.flat_rows {
            if y + lh < rows_top {
                y += lh;
                continue;
            }
            if y > origin_y + viewport_h {
                break;
            }
            let x0 = origin_x + LEFT_PAD * scale + row.depth as f32 * INDENT_PER_DEPTH * scale;
            let abs_path = workspace_root.map(|r| r.join(&row.rel));
            let is_open = abs_path.as_ref().and_then(|p| buffers.find_by_path(p)).is_some();

            let is_focused_row =
                self.focused && self.highlighted.as_ref().is_some_and(|p| p == &row.rel);
            let is_hover_row = !self.focused
                && (self.highlighted.as_ref().is_some_and(|p| p == &row.rel)
                    || auto_highlight_rel.is_some_and(|hp| {
                        abs_path.as_ref().map(|a| BufferManager::same_path(a, hp)).unwrap_or(false)
                    }));
            if is_focused_row {
                chrome.push_quad(ChromeQuad {
                    left: origin_x,
                    top: y,
                    width: w,
                    height: lh,
                    rgba: ROW_FOCUS_RGBA,
                });
            } else if is_hover_row {
                chrome.push_quad(ChromeQuad {
                    left: origin_x,
                    top: y,
                    width: w,
                    height: lh,
                    rgba: ROW_HL_RGBA,
                });
            }

            let (icon, rgb) = if row.is_dir {
                let sym = if self.is_expanded(&row.rel) { "▾ " } else { "▸ " };
                (format!("{sym}{}", row.label), if is_open { ACCENT } else { TEXT_RGB })
            } else if is_open {
                (format!("  {}", row.label), ACCENT)
            } else {
                (format!("  {}", row.label), TEXT_DIM)
            };
            chrome.push_line(x0, y + 4.0 * scale, icon, rgb);
            y += lh;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn reveal_expands_parents() {
        let mut s = Sidebar::new();
        s.reveal_path(Path::new("a/b/c.rs"));
        assert!(s.expanded_dirs.contains(&PathBuf::from("a")));
        assert!(s.expanded_dirs.contains(&PathBuf::from("a/b")));
    }
}
