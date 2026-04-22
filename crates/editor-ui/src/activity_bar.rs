//! Narrow icon column on the far-left edge (VS Code convention).
//!
//! Minimal visual bar only — no interactive surface yet. Icons are Unicode glyphs
//! because the app does not bundle an icon font yet. Pixel dimensions follow VS
//! Code's defaults: a 48-logical-px column with ~28 px tall square icon slots.

use crate::chrome::{ChromeQuad, FrameChrome};

/// Logical width of the activity bar column.
pub const ACTIVITY_BAR_WIDTH: f32 = 48.0;
/// Logical icon slot height.
pub const ACTIVITY_ICON_HEIGHT: f32 = 48.0;

const BG_RGBA: [f32; 4] = [0.20, 0.20, 0.21, 1.0];
const ACTIVE_BAR_RGBA: [f32; 4] = [0.0, 0.48, 0.80, 1.0];
const ICON_ACTIVE_RGB: [u8; 3] = [0xff, 0xff, 0xff];
const ICON_DIM_RGB: [u8; 3] = [0x85, 0x85, 0x85];

/// One icon slot on the activity bar.
#[derive(Debug, Clone, Copy)]
pub struct ActivityIcon {
    /// Unicode glyph to show in the slot.
    pub glyph: &'static str,
    /// True → paint the left accent bar + bright fg (the current surface).
    pub active: bool,
}

impl ActivityIcon {
    #[must_use]
    pub const fn new(glyph: &'static str, active: bool) -> Self {
        Self { glyph, active }
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
    chrome.push_quad(ChromeQuad { left: 0.0, top: 0.0, width: w, height: h, rgba: BG_RGBA });

    let slot_h = ACTIVITY_ICON_HEIGHT * scale;
    let mut y = 0.0;
    for icon in icons {
        if y + slot_h > h {
            break;
        }
        if icon.active {
            // 2px blue accent bar on the left edge.
            chrome.push_quad(ChromeQuad {
                left: 0.0,
                top: y,
                width: 2.0 * scale,
                height: slot_h,
                rgba: ACTIVE_BAR_RGBA,
            });
        }
        let rgb = if icon.active { ICON_ACTIVE_RGB } else { ICON_DIM_RGB };
        // Centered-ish: the glyph font is 14pt monospace, so y offset ~ 14px.
        chrome.push_line(
            (ACTIVITY_BAR_WIDTH / 2.0 - 7.0) * scale,
            y + (slot_h - 14.0 * scale) / 2.0,
            icon.glyph.to_string(),
            rgb,
        );
        y += slot_h;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_background_and_accent_for_active() {
        let mut chrome = FrameChrome::new();
        let icons = [ActivityIcon::new("\u{1F5C0}", true), ActivityIcon::new("?", false)];
        paint_activity_bar(&mut chrome, 1.0, 300.0, &icons);
        // 1 bg quad + 1 accent quad for active icon = 2 quads.
        assert_eq!(chrome.quads.len(), 2);
        // 2 icon text lines.
        assert_eq!(chrome.lines.len(), 2);
    }
}
