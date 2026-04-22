//! Thin path strip painted under the tab strip showing the active buffer's
//! workspace-relative location as clickable segments separated by chevrons.
//!
//! Today the paint path only renders the segments — mouse interaction
//! (clicking a parent directory to navigate there) is reserved for a future
//! mission and would be layered on top of this module without changing its
//! public API.

use std::path::{Component, Path, PathBuf};

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::icons::{paint_icon, Icon};
use crate::theme::palette;

/// Logical height of the breadcrumbs strip.
pub const BREADCRUMBS_HEIGHT: f32 = 24.0;
/// Logical horizontal pad between text and separator.
const SEGMENT_GAP: f32 = 4.0;
const HPAD: f32 = 12.0;

/// Hit region for one crumb segment. `full_path` is the sub-path that the
/// crumb represents (relative to the workspace root); mouse code may wire
/// click-to-navigate on top of this in a future change.
#[derive(Debug, Clone)]
pub struct BreadcrumbHit {
    pub full_path: PathBuf,
    pub x0: f32,
    pub x1: f32,
}

/// Split a relative path into its component labels. Returns the segments in
/// display order (root-most first). Any non-normal component (Windows prefix,
/// root dir, `..`) is discarded because crumbs only make sense relative to a
/// workspace root.
pub fn crumb_segments(rel: &Path) -> Vec<String> {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect()
}

/// Paint the breadcrumbs strip. `rel` is the buffer's workspace-relative path
/// (or its file name when no workspace is open). Returns per-segment hit
/// regions in paint order.
pub fn paint_breadcrumbs(
    chrome: &mut FrameChrome,
    scale: f32,
    origin_x: f32,
    origin_y: f32,
    strip_width_px: f32,
    rel: Option<&Path>,
) -> Vec<BreadcrumbHit> {
    let h = BREADCRUMBS_HEIGHT * scale;
    // Strip background (slightly darker than tab strip so it reads as its own band).
    chrome.push_quad(ChromeQuad {
        left: origin_x,
        top: origin_y,
        width: strip_width_px,
        height: h,
        rgba: palette::EDITOR_BG,
    });
    // 1px hairline separator at the bottom of the strip.
    chrome.push_quad(ChromeQuad {
        left: origin_x,
        top: origin_y + h - scale.max(1.0),
        width: strip_width_px,
        height: scale.max(1.0),
        rgba: palette::TAB_SEPARATOR,
    });

    let Some(rel) = rel else { return Vec::new() };
    let segments = crumb_segments(rel);
    if segments.is_empty() {
        return Vec::new();
    }

    let chev_size = 10.0 * scale;
    let text_y = origin_y + 6.0 * scale;
    let separator_y = origin_y + h / 2.0;
    let mut x = origin_x + HPAD * scale;
    let mut hits = Vec::with_capacity(segments.len());
    let mut accumulated = PathBuf::new();
    let text_rgb = palette::EDITOR_FG_DIM;
    let last_rgb = palette::EDITOR_FG;

    for (i, seg) in segments.iter().enumerate() {
        accumulated.push(seg);
        let is_last = i + 1 == segments.len();
        // Measure with the same 7.2px-per-char approximation used elsewhere.
        let w_text = seg.chars().count() as f32 * 7.2 * scale;
        let seg_w = w_text;

        // Stop drawing once we would overflow the strip; honor a small right-side gutter.
        if x + seg_w > origin_x + strip_width_px - HPAD * scale {
            break;
        }

        chrome.push_line(x, text_y, seg.clone(), if is_last { last_rgb } else { text_rgb });
        let x0 = x;
        x += seg_w;
        hits.push(BreadcrumbHit { full_path: accumulated.clone(), x0, x1: x });

        if !is_last {
            x += SEGMENT_GAP * scale;
            let chev_x = x + chev_size / 2.0;
            if chev_x + chev_size / 2.0 > origin_x + strip_width_px - HPAD * scale {
                break;
            }
            paint_icon(
                chrome,
                Icon::ChevronRight,
                chev_x,
                separator_y,
                chev_size,
                [
                    text_rgb[0] as f32 / 255.0,
                    text_rgb[1] as f32 / 255.0,
                    text_rgb[2] as f32 / 255.0,
                    1.0,
                ],
            );
            x += chev_size + SEGMENT_GAP * scale;
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_keep_only_normal_components() {
        let segs = crumb_segments(Path::new("a/b/c.rs"));
        assert_eq!(segs, vec!["a", "b", "c.rs"]);
        // Windows absolute path: prefix + root dir are skipped.
        if cfg!(windows) {
            let w = crumb_segments(Path::new(r"C:\x\y"));
            assert_eq!(w, vec!["x", "y"]);
        }
    }

    #[test]
    fn paint_emits_background_and_one_text_per_segment() {
        let mut chrome = FrameChrome::new();
        let hits = paint_breadcrumbs(
            &mut chrome,
            1.0,
            0.0,
            0.0,
            400.0,
            Some(Path::new("crates/editor-ui/src/lib.rs")),
        );
        // 1 strip bg + 1 bottom hairline = 2 quads from the strip itself;
        // chevrons between segments add more.
        assert!(chrome.quads.len() >= 2);
        // 4 segment text lines.
        assert_eq!(chrome.lines.len(), 4);
        assert_eq!(hits.len(), 4);
        assert_eq!(hits[0].full_path, PathBuf::from("crates"));
        assert_eq!(hits.last().unwrap().full_path, PathBuf::from("crates/editor-ui/src/lib.rs"));
        // Monotonic x progression.
        for pair in hits.windows(2) {
            assert!(pair[0].x1 <= pair[1].x0 + 0.001);
        }
    }

    #[test]
    fn paint_is_noop_when_path_is_none() {
        let mut chrome = FrameChrome::new();
        let hits = paint_breadcrumbs(&mut chrome, 1.0, 0.0, 0.0, 400.0, None);
        // Still paints the background + hairline.
        assert_eq!(chrome.quads.len(), 2);
        assert!(chrome.lines.is_empty());
        assert!(hits.is_empty());
    }

    #[test]
    fn overflow_clips_trailing_segments() {
        let mut chrome = FrameChrome::new();
        // Narrow strip — only a couple of segments can fit.
        let hits = paint_breadcrumbs(
            &mut chrome,
            1.0,
            0.0,
            0.0,
            120.0,
            Some(Path::new("a-very-long-dir-name/another-long-dir/another/deep.rs")),
        );
        assert!(hits.len() < 4, "clip should drop trailing crumbs, got {}", hits.len());
    }
}
