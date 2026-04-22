//! Thin header strip sitting at the top of the integrated terminal pane.
//!
//! Deliberately minimal: a "Terminal" label on the left, a close button on
//! the right, a 1px separator underneath. No tabs, no kebab menu — the pane
//! becomes recognizable chrome without crowding the viewport.
//!
//! The painter is a pure function: it pushes quads + text into [`FrameChrome`]
//! and returns a hit region for the close glyph so mouse routing in the app
//! can consume clicks on it.

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::icons::{paint_icon, Icon};
use crate::theme::palette;

/// Logical height of the terminal header strip.
pub const TERMINAL_HEADER_HEIGHT: f32 = 28.0;
/// Logical size of the close button icon (drawn via `icons::Icon::Close`).
pub const TERMINAL_CLOSE_ICON_SIZE: f32 = 12.0;
/// Logical right padding between the close icon and the right edge.
const RIGHT_PAD: f32 = 8.0;
/// Logical padding inside the close button rect (so the hit target is bigger
/// than the icon itself, following Fitts's law).
const CLOSE_BUTTON_PAD: f32 = 6.0;
/// Logical left pad for the "Terminal" label.
const LEFT_PAD: f32 = 12.0;

/// Hit region for the close button in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalHeaderHits {
    pub close_x0: f32,
    pub close_x1: f32,
    pub close_y0: f32,
    pub close_y1: f32,
    /// Full header rect — used by mouse routing to swallow clicks so the
    /// terminal pane doesn't steal focus when the user is clicking chrome.
    pub header_x0: f32,
    pub header_x1: f32,
    pub header_y0: f32,
    pub header_y1: f32,
}

impl TerminalHeaderHits {
    /// True when the given physical-pixel pointer is on the close button.
    #[must_use]
    pub fn pointer_on_close(&self, x: f32, y: f32) -> bool {
        x >= self.close_x0 && x <= self.close_x1 && y >= self.close_y0 && y <= self.close_y1
    }

    /// True when the given physical-pixel pointer is anywhere inside the strip.
    #[must_use]
    pub fn pointer_on_header(&self, x: f32, y: f32) -> bool {
        x >= self.header_x0 && x <= self.header_x1 && y >= self.header_y0 && y <= self.header_y1
    }
}

/// Paint the header above the terminal pane.
///
/// `origin_x` / `origin_y` are the top-left of the **terminal pane itself**
/// (the full-width strip whose top is `window_h - status_bar_h - pane_h`).
/// `pane_width_px` is the horizontal extent (usually window width minus the
/// activity bar — kept configurable so sidebar-aware layouts are possible).
pub fn paint_terminal_header(
    chrome: &mut FrameChrome,
    scale: f32,
    origin_x: f32,
    origin_y: f32,
    pane_width_px: f32,
) -> TerminalHeaderHits {
    let h = TERMINAL_HEADER_HEIGHT * scale;
    // Header background — slightly lighter than the terminal body so the split
    // is obvious without a heavy border.
    chrome.push_quad(ChromeQuad {
        left: origin_x,
        top: origin_y,
        width: pane_width_px,
        height: h,
        rgba: palette::SIDEBAR_BG,
    });
    // 1px top separator — the visible border between editor and terminal.
    chrome.push_quad(ChromeQuad {
        left: origin_x,
        top: origin_y,
        width: pane_width_px,
        height: scale.max(1.0),
        rgba: palette::TAB_SEPARATOR,
    });
    // 1px bottom separator so the label doesn't touch the PTY rows.
    chrome.push_quad(ChromeQuad {
        left: origin_x,
        top: origin_y + h - scale.max(1.0),
        width: pane_width_px,
        height: scale.max(1.0),
        rgba: palette::TAB_SEPARATOR,
    });

    // Label. Vertical center: the font ascent lives about 9 logical px below
    // the strip top for a 13px font at this line height.
    chrome.push_line(
        origin_x + LEFT_PAD * scale,
        origin_y + 7.0 * scale,
        "Terminal",
        palette::SIDEBAR_HEADER_FG,
    );

    // Close button: centered inside a padded hit target so there's breathing
    // room on trackpads / touch.
    let btn_size = (TERMINAL_CLOSE_ICON_SIZE + CLOSE_BUTTON_PAD * 2.0) * scale;
    let close_x1 = origin_x + pane_width_px - RIGHT_PAD * scale;
    let close_x0 = close_x1 - btn_size;
    let close_y0 = origin_y + (h - btn_size) / 2.0;
    let close_y1 = close_y0 + btn_size;
    let icon_rgb = palette::TAB_CLOSE_DIM;
    paint_icon(
        chrome,
        Icon::Close,
        close_x0 + btn_size / 2.0,
        close_y0 + btn_size / 2.0,
        TERMINAL_CLOSE_ICON_SIZE * scale,
        [icon_rgb[0] as f32 / 255.0, icon_rgb[1] as f32 / 255.0, icon_rgb[2] as f32 / 255.0, 1.0],
    );

    TerminalHeaderHits {
        close_x0,
        close_x1,
        close_y0,
        close_y1,
        header_x0: origin_x,
        header_x1: origin_x + pane_width_px,
        header_y0: origin_y,
        header_y1: origin_y + h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_background_separators_and_label() {
        let mut c = FrameChrome::new();
        let hits = paint_terminal_header(&mut c, 1.0, 0.0, 500.0, 1024.0);
        // bg + top separator + bottom separator + close-icon quads
        assert!(c.quads.len() >= 3, "expected >=3 quads, got {}", c.quads.len());
        assert_eq!(c.lines.len(), 1);
        assert_eq!(c.lines[0].text, "Terminal");
        // Close rect sits on the right half of the pane.
        assert!(hits.close_x0 > 500.0);
        assert!(hits.close_x1 > hits.close_x0);
        assert!(hits.close_y1 > hits.close_y0);
    }

    #[test]
    fn pointer_hit_tests_are_correct() {
        let mut c = FrameChrome::new();
        let hits = paint_terminal_header(&mut c, 1.0, 0.0, 0.0, 1000.0);
        // Close center should be inside the close rect.
        let cx = (hits.close_x0 + hits.close_x1) / 2.0;
        let cy = (hits.close_y0 + hits.close_y1) / 2.0;
        assert!(hits.pointer_on_close(cx, cy));
        assert!(hits.pointer_on_header(cx, cy));
        // Far-left point is on the header but not on the close button.
        assert!(hits.pointer_on_header(50.0, 10.0));
        assert!(!hits.pointer_on_close(50.0, 10.0));
        // Below the header strip shouldn't match either.
        assert!(!hits.pointer_on_header(50.0, 100.0));
    }

    #[test]
    fn scales_up_with_scale_factor() {
        let mut a = FrameChrome::new();
        let mut b = FrameChrome::new();
        let ha = paint_terminal_header(&mut a, 1.0, 0.0, 0.0, 800.0);
        let hb = paint_terminal_header(&mut b, 2.0, 0.0, 0.0, 800.0);
        assert_eq!(a.quads.len(), b.quads.len());
        // Close target grows roughly 2x.
        let size_a = ha.close_x1 - ha.close_x0;
        let size_b = hb.close_x1 - hb.close_x0;
        assert!(size_b > size_a * 1.8, "size_a={size_a} size_b={size_b}");
    }
}
