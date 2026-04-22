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

// Palette references — source of truth lives in `crate::theme::palette`.
use crate::theme::palette as pal;
const BG_RGBA: [f32; 4] = pal::SIDEBAR_BG;
const ROW_HL_RGBA: [f32; 4] = pal::SIDEBAR_ROW_HOVER;
const ROW_FOCUS_RGBA: [f32; 4] = pal::SIDEBAR_ROW_FOCUS;
const HEADER_BG_RGBA: [f32; 4] = pal::SIDEBAR_BG;
const HEADER_RGB: [u8; 3] = pal::SIDEBAR_HEADER_FG;
const TEXT_RGB: [u8; 3] = pal::SIDEBAR_ROW_FG;
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

    /// Index of the currently-highlighted row, if any.
    #[must_use]
    pub fn highlighted_index(&self) -> Option<usize> {
        let rel = self.highlighted.as_ref()?;
        self.flat_rows.iter().position(|r| &r.rel == rel)
    }

    /// Move the keyboard highlight by `delta` rows (positive = down). Clamps to the
    /// visible row range. If nothing is highlighted yet, seeds at the first row.
    ///
    /// Returns the new index when it changed, `None` when no movement was possible
    /// (empty tree or already clamped at the requested edge).
    pub fn move_highlight(&mut self, delta: isize) -> Option<usize> {
        if self.flat_rows.is_empty() {
            self.highlighted = None;
            return None;
        }
        let last = self.flat_rows.len() - 1;
        let current = self.highlighted_index();
        let next = match current {
            Some(i) => {
                let target = (i as isize + delta).clamp(0, last as isize) as usize;
                if target == i {
                    return None;
                }
                target
            }
            None => {
                if delta >= 0 {
                    0
                } else {
                    last
                }
            }
        };
        self.highlighted = Some(self.flat_rows[next].rel.clone());
        Some(next)
    }

    /// Jump to the first / last visible row. Returns the new index if it changed.
    pub fn highlight_first(&mut self) -> Option<usize> {
        let first = self.flat_rows.first()?;
        let idx = 0;
        if self.highlighted_index() == Some(idx) {
            return None;
        }
        self.highlighted = Some(first.rel.clone());
        Some(idx)
    }

    /// Jump to the last visible row. Returns the new index if it changed.
    pub fn highlight_last(&mut self) -> Option<usize> {
        let last_idx = self.flat_rows.len().checked_sub(1)?;
        if self.highlighted_index() == Some(last_idx) {
            return None;
        }
        self.highlighted = Some(self.flat_rows[last_idx].rel.clone());
        Some(last_idx)
    }

    /// Expand the highlighted directory (or no-op for files / already-expanded).
    /// Returns `true` if an expansion actually happened.
    pub fn expand_highlighted(&mut self) -> bool {
        let Some(rel) = self.highlighted.as_ref() else { return false };
        let Some(row) = self.flat_rows.iter().find(|r| &r.rel == rel) else { return false };
        if !row.is_dir || self.is_expanded(&row.rel) {
            return false;
        }
        self.expanded_dirs.insert(row.rel.clone());
        true
    }

    /// Collapse the highlighted directory, or move the highlight to the parent when
    /// the row is already collapsed / is a file. Returns the new highlighted path
    /// when the highlight moved, or `None` when the action was a collapse or no-op.
    pub fn collapse_or_parent(&mut self) -> Option<PathBuf> {
        let rel = self.highlighted.as_ref()?.clone();
        let row_is_open_dir = self
            .flat_rows
            .iter()
            .find(|r| r.rel == rel)
            .is_some_and(|r| r.is_dir && self.is_expanded(&r.rel));
        if row_is_open_dir {
            self.expanded_dirs.remove(&rel);
            return None;
        }
        // Move to parent directory if there is one.
        let parent = rel.parent()?.to_path_buf();
        if parent.as_os_str().is_empty() {
            return None;
        }
        self.highlighted = Some(parent.clone());
        Some(parent)
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

    fn row(rel: &str, is_dir: bool, depth: u16) -> FlatRow {
        FlatRow {
            rel: PathBuf::from(rel),
            depth,
            is_dir,
            label: rel.rsplit('/').next().unwrap_or(rel).to_string(),
            has_children: is_dir,
        }
    }

    fn sb_with_rows() -> Sidebar {
        let mut s = Sidebar::new();
        s.flat_rows = vec![
            row("a", true, 0),
            row("a/b.rs", false, 1),
            row("c", true, 0),
            row("c/d.rs", false, 1),
        ];
        // Expand "a" + "c" so their children appear as real rows in this fixture.
        s.expanded_dirs.insert(PathBuf::from("a"));
        s.expanded_dirs.insert(PathBuf::from("c"));
        s
    }

    #[test]
    fn move_highlight_seeds_from_top_on_first_down() {
        let mut s = sb_with_rows();
        assert_eq!(s.move_highlight(1), Some(0));
        assert_eq!(s.highlighted, Some(PathBuf::from("a")));
    }

    #[test]
    fn move_highlight_seeds_from_bottom_on_first_up() {
        let mut s = sb_with_rows();
        assert_eq!(s.move_highlight(-1), Some(3));
        assert_eq!(s.highlighted, Some(PathBuf::from("c/d.rs")));
    }

    #[test]
    fn move_highlight_clamps_at_edges() {
        let mut s = sb_with_rows();
        s.highlighted = Some(PathBuf::from("a"));
        // Already at top: no move.
        assert_eq!(s.move_highlight(-1), None);
        s.highlighted = Some(PathBuf::from("c/d.rs"));
        // Already at bottom: no move.
        assert_eq!(s.move_highlight(1), None);
    }

    #[test]
    fn move_highlight_steps_by_delta() {
        let mut s = sb_with_rows();
        s.highlighted = Some(PathBuf::from("a"));
        assert_eq!(s.move_highlight(2), Some(2));
        assert_eq!(s.highlighted, Some(PathBuf::from("c")));
    }

    #[test]
    fn first_and_last_helpers() {
        let mut s = sb_with_rows();
        assert_eq!(s.highlight_last(), Some(3));
        assert_eq!(s.highlighted, Some(PathBuf::from("c/d.rs")));
        assert_eq!(s.highlight_first(), Some(0));
        assert_eq!(s.highlighted, Some(PathBuf::from("a")));
        // No-op when already there.
        assert_eq!(s.highlight_first(), None);
    }

    #[test]
    fn expand_only_applies_to_collapsed_dirs() {
        let mut s = sb_with_rows();
        // Seed with a collapsed directory: remove "a" from expanded set.
        s.expanded_dirs.remove(&PathBuf::from("a"));
        s.highlighted = Some(PathBuf::from("a"));
        assert!(s.expand_highlighted());
        assert!(s.is_expanded(Path::new("a")));
        // Second call is a no-op because the dir is already expanded.
        assert!(!s.expand_highlighted());
        // Files never expand.
        s.highlighted = Some(PathBuf::from("a/b.rs"));
        assert!(!s.expand_highlighted());
    }

    #[test]
    fn collapse_or_parent_collapses_open_dir_then_walks_up() {
        let mut s = sb_with_rows();
        s.highlighted = Some(PathBuf::from("a"));
        // First call collapses "a" and keeps highlight.
        assert_eq!(s.collapse_or_parent(), None);
        assert!(!s.is_expanded(Path::new("a")));
        // Second call on an already-collapsed top-level dir has no parent to walk to.
        assert_eq!(s.collapse_or_parent(), None);
        // On a file, walks up to the parent directory.
        s.highlighted = Some(PathBuf::from("c/d.rs"));
        assert_eq!(s.collapse_or_parent(), Some(PathBuf::from("c")));
        assert_eq!(s.highlighted, Some(PathBuf::from("c")));
    }
}
