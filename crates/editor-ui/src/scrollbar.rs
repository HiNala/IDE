//! Thin vertical scrollbar painted on the right edge of the editor viewport.
//!
//! Minimal MVP: a translucent track + an opaque thumb sized by the viewport
//! / document ratio. No interactivity yet — dragging the thumb and scroll-to-
//! click are reserved for a later pass (would live here, consuming mouse
//! coordinates returned in [`ScrollbarMetrics`]).
//!
//! The painter is a pure function over scroll state. It owns no state.

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::theme::palette;

/// Default scrollbar width in logical pixels.
pub const SCROLLBAR_WIDTH: f32 = 12.0;
/// Minimum thumb height so it stays draggable on huge documents.
const MIN_THUMB_H: f32 = 24.0;

/// Geometry for the thumb in physical px — returned so mouse routing can
/// hit-test it without rederiving the math.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollbarMetrics {
    pub track_left: f32,
    pub track_top: f32,
    pub track_width: f32,
    pub track_height: f32,
    pub thumb_top: f32,
    pub thumb_height: f32,
}

/// Inputs required by [`paint_scrollbar`]. `content_*` values are in physical px.
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarInput {
    /// Total number of logical rows in the document.
    pub total_lines: usize,
    /// Current scroll offset (document pixels hidden above the viewport).
    pub scroll_y_px: f32,
    /// Row height in physical px.
    pub line_height_px: f32,
    /// Right edge of the canvas in physical px (usually the window width).
    pub content_right_px: f32,
    /// Top of the editor canvas in physical px (below chrome).
    pub content_top_px: f32,
    /// Bottom of the editor canvas in physical px (above status bar / terminal).
    pub content_bottom_px: f32,
    /// Scale factor — applied to width + min-thumb-height.
    pub scale: f32,
}

/// Paint the scrollbar track + thumb into `chrome`. Returns `None` when the
/// viewport is large enough to show the whole document (no scrollbar needed).
pub fn paint_scrollbar(
    chrome: &mut FrameChrome,
    input: ScrollbarInput,
) -> Option<ScrollbarMetrics> {
    let ScrollbarInput {
        total_lines,
        scroll_y_px,
        line_height_px,
        content_right_px,
        content_top_px,
        content_bottom_px,
        scale,
    } = input;

    let track_height = (content_bottom_px - content_top_px).max(0.0);
    if track_height <= 0.0 || line_height_px <= 0.0 || total_lines == 0 {
        return None;
    }

    let doc_height_px = total_lines as f32 * line_height_px;
    if doc_height_px <= track_height {
        // Document fits; no scrollbar needed.
        return None;
    }

    let track_width = SCROLLBAR_WIDTH * scale;
    let track_left = (content_right_px - track_width).max(0.0);

    // Track: faint translucent rail.
    chrome.push_quad(ChromeQuad {
        left: track_left,
        top: content_top_px,
        width: track_width,
        height: track_height,
        rgba: [palette::SIDEBAR_BG[0], palette::SIDEBAR_BG[1], palette::SIDEBAR_BG[2], 0.35],
    });

    let min_thumb_h = MIN_THUMB_H * scale;
    let raw_thumb = (track_height * (track_height / doc_height_px)).max(min_thumb_h);
    let thumb_h = raw_thumb.min(track_height);
    let max_scroll = (doc_height_px - track_height).max(1.0);
    let clamped_scroll = scroll_y_px.clamp(0.0, max_scroll);
    let max_thumb_top = track_height - thumb_h;
    let thumb_top = content_top_px + (clamped_scroll / max_scroll) * max_thumb_top;

    // Thumb: brighter solid fill, 2px inset from track edges so the track
    // reads as a rail.
    let inset = (1.0 * scale).min(track_width / 2.0);
    chrome.push_quad(ChromeQuad {
        left: track_left + inset,
        top: thumb_top,
        width: track_width - inset * 2.0,
        height: thumb_h,
        rgba: [
            palette::EDITOR_FG_DIM[0] as f32 / 255.0,
            palette::EDITOR_FG_DIM[1] as f32 / 255.0,
            palette::EDITOR_FG_DIM[2] as f32 / 255.0,
            0.55,
        ],
    });

    Some(ScrollbarMetrics {
        track_left,
        track_top: content_top_px,
        track_width,
        track_height,
        thumb_top,
        thumb_height: thumb_h,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input(total: usize, scroll: f32) -> ScrollbarInput {
        ScrollbarInput {
            total_lines: total,
            scroll_y_px: scroll,
            line_height_px: 20.0,
            content_right_px: 800.0,
            content_top_px: 40.0,
            content_bottom_px: 440.0, // 400px tall viewport.
            scale: 1.0,
        }
    }

    #[test]
    fn hidden_when_document_fits() {
        let mut c = FrameChrome::new();
        // 400px viewport vs 400px doc (20 lines × 20) — fits exactly.
        assert!(paint_scrollbar(&mut c, base_input(20, 0.0)).is_none());
        assert_eq!(c.quads.len(), 0);
    }

    #[test]
    fn hidden_when_line_height_zero() {
        let mut c = FrameChrome::new();
        let mut input = base_input(100, 0.0);
        input.line_height_px = 0.0;
        assert!(paint_scrollbar(&mut c, input).is_none());
    }

    #[test]
    fn thumb_placed_at_top_when_scroll_zero() {
        let mut c = FrameChrome::new();
        // 100 lines × 20 = 2000px doc, 400px viewport.
        let m = paint_scrollbar(&mut c, base_input(100, 0.0)).expect("should paint");
        assert_eq!(c.quads.len(), 2, "track + thumb");
        assert_eq!(m.thumb_top, m.track_top);
    }

    #[test]
    fn thumb_slides_to_bottom_when_scroll_maxed() {
        let mut c = FrameChrome::new();
        // max scroll = 2000 - 400 = 1600.
        let m = paint_scrollbar(&mut c, base_input(100, 1600.0)).expect("should paint");
        let bottom = m.thumb_top + m.thumb_height;
        assert!((bottom - (m.track_top + m.track_height)).abs() < 0.01, "m={m:?}");
    }

    #[test]
    fn thumb_has_minimum_height_on_huge_documents() {
        let mut c = FrameChrome::new();
        // 1 million lines — thumb would otherwise be sub-pixel.
        let m = paint_scrollbar(&mut c, base_input(1_000_000, 0.0)).expect("should paint");
        assert!(m.thumb_height >= MIN_THUMB_H - 0.001, "thumb_height={}", m.thumb_height);
    }

    #[test]
    fn thumb_stays_inside_track_when_scroll_overshoots() {
        let mut c = FrameChrome::new();
        // scroll_y beyond doc size should clamp, not panic.
        let m = paint_scrollbar(&mut c, base_input(50, 999_999.0)).expect("should paint");
        assert!(m.thumb_top >= m.track_top);
        assert!(m.thumb_top + m.thumb_height <= m.track_top + m.track_height + 0.01);
    }
}
