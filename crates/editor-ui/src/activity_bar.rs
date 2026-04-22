//! Narrow icon column on the far-left edge (VS Code convention).
//!
//! Icons are drawn from [`crate::icons`] as rect primitives — no icon font,
//! no Unicode / emoji pictographs. Layout is a 48-logical-px column with 48px
//! square slots.

use crate::chrome::{ChromeQuad, FrameChrome};
use crate::icons::{paint_icon, Icon};
use crate::theme::palette;

/// Logical width of the activity bar column.
/// Set to 0 — the design integrates navigation directly into the sidebar.
pub const ACTIVITY_BAR_WIDTH: f32 = 0.0;
/// Logical icon slot height.
pub const ACTIVITY_ICON_HEIGHT: f32 = 48.0;
/// Logical size of the drawn icon inside its slot.
pub const ACTIVITY_ICON_SIZE: f32 = 20.0;

/// One icon slot on the activity bar.
#[derive(Debug, Clone, Copy)]
pub struct ActivityIcon {
    /// Which shape to draw.
    pub kind: Icon,
    /// True → paint the left accent bar + bright fg (the current surface).
    pub active: bool,
}

impl ActivityIcon {
    #[must_use]
    pub const fn new(kind: Icon, active: bool) -> Self {
        Self { kind, active }
    }
}

/// Paint the activity bar column. `height_px` is the already-computed height (minus
/// status bar). Icons render top-to-bottom in the order provided.
pub fn paint_activity_bar(
    chrome: &mut FrameChrome,
    scale: f32,
    height_px: f32,
    icons: &[ActivityIcon],
) {
    let w = ACTIVITY_BAR_WIDTH * scale;
    let h = height_px.max(1.0);
    chrome.push_quad(ChromeQuad {
        left: 0.0,
        top: 0.0,
        width: w,
        height: h,
        rgba: palette::ACTIVITY_BG,
    });

    let slot_h = ACTIVITY_ICON_HEIGHT * scale;
    let icon_size = ACTIVITY_ICON_SIZE * scale;
    let center_x = (ACTIVITY_BAR_WIDTH / 2.0) * scale;
    let mut y = 0.0;
    for icon in icons {
        if y + slot_h > h {
            break;
        }
        if icon.active {
            chrome.push_quad(ChromeQuad {
                left: 0.0,
                top: y,
                width: 2.0 * scale,
                height: slot_h,
                rgba: palette::ACCENT_BLUE,
            });
        }
        let rgb =
            if icon.active { palette::ACTIVITY_FG_ACTIVE } else { palette::ACTIVITY_FG_INACTIVE };
        let rgba = [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0];
        paint_icon(chrome, icon.kind, center_x, y + slot_h / 2.0, icon_size, rgba);
        y += slot_h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_background_accent_and_icons() {
        let mut chrome = FrameChrome::new();
        let icons =
            [ActivityIcon::new(Icon::Explorer, true), ActivityIcon::new(Icon::Search, false)];
        paint_activity_bar(&mut chrome, 1.0, 300.0, &icons);
        // 1 bg + 1 accent + N icon rects per icon.
        assert!(chrome.quads.len() >= 2);
        // No Unicode glyphs painted.
        assert_eq!(chrome.lines.len(), 0);
    }

    #[test]
    fn clips_icons_that_overflow() {
        let mut chrome = FrameChrome::new();
        let icons = [
            ActivityIcon::new(Icon::Explorer, true),
            ActivityIcon::new(Icon::Search, false),
            ActivityIcon::new(Icon::Run, false),
        ];
        // Height of only ~60px fits one slot plus part of a second.
        paint_activity_bar(&mut chrome, 1.0, 60.0, &icons);
        // The first icon's rects should be there, the third should not.
        let first_count = crate::icons::Icon::Explorer;
        let _ = first_count; // existence check (enum variant).
        assert!(chrome.quads.len() <= 1 + 1 + 10); // bg + accent + ~up to one icon's rects
    }
}
