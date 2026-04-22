//! Rect-based monochrome icon primitives.
//!
//! We do not ship an icon font. Each icon is defined as a small set of
//! normalized rectangles (0..1 space, origin = top-left) that get rasterized
//! by [`FrameChrome::push_quad`](crate::FrameChrome::push_quad). This keeps
//! the renderer free of path / SVG code and guarantees crisp edges at every
//! DPI and font size.
//!
//! Paint order for a single icon:
//! 1. [`paint_icon`] sizes the icon into a logical `size × size` square
//!    centered at `(x, y)`.
//! 2. Each rect is pushed as a colored quad.
//!
//! The pixel grid each icon is drawn on is 16×16 logical units. This matches
//! the Lucide / Codicon baseline and keeps math simple.

use crate::chrome::{ChromeQuad, FrameChrome};

/// Icon grid resolution. All icon rects are authored in integer 0..16 units.
const GRID: f32 = 16.0;

/// The set of icons the app currently paints. This list is additive — new icons
/// get a new variant + a rule in [`icon_rects`] and nothing else changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// File explorer (stacked document lines).
    Explorer,
    /// Search (magnifier body + handle).
    Search,
    /// Source control (branch: two nodes + connector).
    SourceControl,
    /// Run / play (right-pointing triangle staircase).
    Run,
    /// Settings (gear, 4 notches + center ring).
    Settings,
    /// AI / chat (speech balloon).
    Chat,
    /// Close (X).
    Close,
    /// Chevron pointing right (collapsed dir).
    ChevronRight,
    /// Chevron pointing down (expanded dir).
    ChevronDown,
    /// Small dot (marker).
    Dot,
}

/// Describe one icon as an iterator of rectangles in the 16×16 unit grid.
/// Rect tuple = `(x, y, w, h)` in grid units.
fn icon_rects(icon: Icon) -> &'static [(f32, f32, f32, f32)] {
    match icon {
        // 3 stacked horizontal lines (document title + two content lines).
        Icon::Explorer => &[(3.0, 3.0, 10.0, 2.0), (3.0, 7.0, 10.0, 1.5), (3.0, 10.0, 7.0, 1.5)],
        // Magnifier: 4 sides of a square ring + diagonal handle drawn as
        // three offset rects to approximate a 45° stroke.
        Icon::Search => &[
            // Ring (top/bottom/left/right of the 7×7 circle approximation).
            (3.0, 3.0, 7.0, 1.5),
            (3.0, 8.5, 7.0, 1.5),
            (3.0, 3.0, 1.5, 7.0),
            (8.5, 3.0, 1.5, 7.0),
            // Handle.
            (10.0, 10.0, 2.0, 1.5),
            (11.5, 11.5, 2.0, 1.5),
            (13.0, 13.0, 1.5, 1.5),
        ],
        // Branch motif: two dots connected by a vertical + diagonal stub.
        Icon::SourceControl => &[
            (4.0, 2.5, 2.5, 2.5),
            (4.0, 11.0, 2.5, 2.5),
            (4.9, 5.0, 1.0, 6.0),
            (6.5, 6.0, 3.0, 1.0),
            (9.0, 7.0, 2.5, 2.5),
        ],
        // Right-pointing triangle approximated by a staircase of 5 rects.
        Icon::Run => &[
            (4.5, 3.5, 2.0, 9.0),
            (6.5, 4.5, 2.0, 7.0),
            (8.5, 5.5, 2.0, 5.0),
            (10.5, 6.5, 1.5, 3.0),
            (12.0, 7.5, 1.0, 1.0),
        ],
        // Gear: 4 radial notches + central square (ring).
        Icon::Settings => &[
            // Center ring: 4 sides of an inner square.
            (6.0, 5.5, 4.0, 1.3),
            (6.0, 9.2, 4.0, 1.3),
            (6.0, 5.5, 1.3, 5.0),
            (8.7, 5.5, 1.3, 5.0),
            // Notches: top / bottom / left / right.
            (7.4, 2.0, 1.4, 2.0),
            (7.4, 12.0, 1.4, 2.0),
            (2.0, 7.4, 2.0, 1.4),
            (12.0, 7.4, 2.0, 1.4),
        ],
        // Speech balloon: rounded rect approximated by 3 full-width slats +
        // a small tail at bottom-left.
        Icon::Chat => &[
            (2.5, 3.0, 11.0, 1.5),
            (2.5, 5.5, 11.0, 1.5),
            (2.5, 8.0, 8.0, 1.5),
            (3.0, 10.0, 2.0, 2.0),
        ],
        // X: two diagonal strokes approximated by 7 grid cells each.
        Icon::Close => &[
            (3.5, 3.5, 1.5, 1.5),
            (5.0, 5.0, 1.5, 1.5),
            (6.5, 6.5, 1.5, 1.5),
            (8.0, 8.0, 1.5, 1.5),
            (9.5, 9.5, 1.5, 1.5),
            (11.0, 11.0, 1.5, 1.5),
            (11.0, 3.5, 1.5, 1.5),
            (9.5, 5.0, 1.5, 1.5),
            (8.0, 6.5, 1.5, 1.5),
            (5.0, 9.5, 1.5, 1.5),
            (3.5, 11.0, 1.5, 1.5),
        ],
        // Chevron right (>): staircase of rects angled to the right.
        Icon::ChevronRight => &[
            (6.0, 4.0, 1.5, 1.5),
            (7.5, 5.5, 1.5, 1.5),
            (9.0, 7.0, 1.5, 1.5),
            (7.5, 8.5, 1.5, 1.5),
            (6.0, 10.0, 1.5, 1.5),
        ],
        // Chevron down (v): staircase angled down.
        Icon::ChevronDown => &[
            (4.0, 6.0, 1.5, 1.5),
            (5.5, 7.5, 1.5, 1.5),
            (7.0, 9.0, 1.5, 1.5),
            (8.5, 7.5, 1.5, 1.5),
            (10.0, 6.0, 1.5, 1.5),
        ],
        // 4×4 dot centered.
        Icon::Dot => &[(6.0, 6.0, 4.0, 4.0)],
    }
}

/// Paint one icon into `chrome` centered at (`center_x`, `center_y`) and
/// inscribed inside a logical `size × size` square. `rgba` is the fill color.
pub fn paint_icon(
    chrome: &mut FrameChrome,
    icon: Icon,
    center_x: f32,
    center_y: f32,
    size: f32,
    rgba: [f32; 4],
) {
    let unit = size / GRID;
    let left = center_x - size / 2.0;
    let top = center_y - size / 2.0;
    for (x, y, w, h) in icon_rects(icon) {
        chrome.push_quad(ChromeQuad {
            left: left + *x * unit,
            top: top + *y * unit,
            width: (*w * unit).max(1.0),
            height: (*h * unit).max(1.0),
            rgba,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_produces_at_least_one_rect() {
        let icons = [
            Icon::Explorer,
            Icon::Search,
            Icon::SourceControl,
            Icon::Run,
            Icon::Settings,
            Icon::Chat,
            Icon::Close,
            Icon::ChevronRight,
            Icon::ChevronDown,
            Icon::Dot,
        ];
        for icon in icons {
            assert!(!icon_rects(icon).is_empty(), "{icon:?} has no rects");
        }
    }

    #[test]
    fn paint_icon_pushes_quads_sized_within_bounds() {
        let mut chrome = FrameChrome::new();
        paint_icon(&mut chrome, Icon::Explorer, 24.0, 24.0, 16.0, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(chrome.quads.len(), icon_rects(Icon::Explorer).len());
        for q in &chrome.quads {
            // Every rect must land inside the 16×16 square centered at (24,24):
            // left ∈ [16, 40], top ∈ [16, 40].
            assert!(q.left >= 16.0 - 0.001 && q.left <= 40.0 + 0.001, "left={}", q.left);
            assert!(q.top >= 16.0 - 0.001 && q.top <= 40.0 + 0.001, "top={}", q.top);
            assert!(q.width > 0.0);
            assert!(q.height > 0.0);
        }
    }

    #[test]
    fn paint_icon_scales_with_size() {
        let mut c_small = FrameChrome::new();
        let mut c_large = FrameChrome::new();
        // Use sizes large enough that the per-quad 1px floor doesn't dominate.
        paint_icon(&mut c_small, Icon::Close, 10.0, 10.0, 16.0, [1.0; 4]);
        paint_icon(&mut c_large, Icon::Close, 10.0, 10.0, 32.0, [1.0; 4]);
        assert_eq!(c_small.quads.len(), c_large.quads.len());
        let w_small: f32 = c_small.quads.iter().map(|q| q.width).sum();
        let w_large: f32 = c_large.quads.iter().map(|q| q.width).sum();
        // Doubling the icon size should roughly double total rect width.
        assert!(
            w_large > w_small * 1.5,
            "expected at least 1.5× scaling, got small={w_small} large={w_large}"
        );
    }
}
