//! Central design tokens: palette, spacing, typography.
//!
//! Every other UI module pulls its colors from this table so that swapping
//! themes (e.g. high-contrast, light) happens in one place. Values come from
//! the VS Code Dark+ palette with minor tuning for our bundled font.
//!
//! Colors are stored in two shapes:
//!   - `[f32; 4]` linear-ish sRGB + alpha for [`ChromeQuad`](crate::ChromeQuad)
//!   - `[u8; 3]` 8-bit sRGB for [`ChromeTextLine`](crate::ChromeTextLine)
//!
//! The helper [`rgba_u8`] converts 8-bit sRGB+alpha to the quad format so
//! that the same literal hex can be used for both without duplicating it.

#![allow(clippy::excessive_precision)]

/// Convert an 8-bit sRGB+alpha tuple into the `[f32; 4]` form used by
/// [`ChromeQuad`](crate::ChromeQuad). Division is by 255.0 and clamped to 1.0.
#[must_use]
pub const fn rgba_u8(r: u8, g: u8, b: u8, a: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0]
}

/// Background, foreground, and accent palette for chrome surfaces.
///
/// Design: obsidian dark + soft violet (`#9580ff`).
/// Surfaces: s0=#07070b (deepest) → s1=#0c0c12 → s2=#090910 → s3=#101018 → s4=#14141e
pub mod palette {
    // === Editor surface ===
    /// Main editor background — obsidian (#090910).
    pub const EDITOR_BG: [f32; 4] = rgba_u8(0x09, 0x09, 0x10, 0xff);
    /// Editor body text — near-white (#eeeef8).
    pub const EDITOR_FG: [u8; 3] = [0xee, 0xee, 0xf8];
    /// Dim text: line numbers, inactive hints (#70708a).
    pub const EDITOR_FG_DIM: [u8; 3] = [0x70, 0x70, 0x8a];

    // === Activity bar (retained for layout but width=0 in this design) ===
    pub const ACTIVITY_BG: [f32; 4] = rgba_u8(0x07, 0x07, 0x0b, 0xff);
    pub const ACTIVITY_FG_ACTIVE: [u8; 3] = [0xc4, 0xb4, 0xff];
    pub const ACTIVITY_FG_INACTIVE: [u8; 3] = [0x3a, 0x3a, 0x52];

    // === Sidebar ===
    /// Sidebar panel background (#0c0c12).
    pub const SIDEBAR_BG: [f32; 4] = rgba_u8(0x0c, 0x0c, 0x12, 0xff);
    /// Hovered row tint.
    pub const SIDEBAR_ROW_HOVER: [f32; 4] = rgba_u8(0x14, 0x14, 0x20, 0xff);
    /// Focused / selected row (violet tint).
    pub const SIDEBAR_ROW_FOCUS: [f32; 4] = rgba_u8(0x95, 0x80, 0xff, 0x26);
    /// Sidebar active accent left bar.
    pub const SIDEBAR_ACCENT: [f32; 4] = rgba_u8(0x95, 0x80, 0xff, 0xff);
    /// Sidebar header label (#3a3a52).
    pub const SIDEBAR_HEADER_FG: [u8; 3] = [0x3a, 0x3a, 0x52];
    /// Sidebar row text (#70708a).
    pub const SIDEBAR_ROW_FG: [u8; 3] = [0x70, 0x70, 0x8a];
    /// Sidebar border (#ffffff0f).
    pub const SIDEBAR_BORDER: [f32; 4] = rgba_u8(0xff, 0xff, 0xff, 0x0f);

    // === Tab strip ===
    /// Tab strip background (#0c0c12).
    pub const TAB_STRIP_BG: [f32; 4] = rgba_u8(0x0c, 0x0c, 0x12, 0xff);
    /// Inactive tab fill.
    pub const TAB_INACTIVE_BG: [f32; 4] = rgba_u8(0x0c, 0x0c, 0x12, 0xff);
    /// Active tab fill (matches editor bg #090910).
    pub const TAB_ACTIVE_BG: [f32; 4] = rgba_u8(0x09, 0x09, 0x10, 0xff);
    /// Tab separator line.
    pub const TAB_SEPARATOR: [f32; 4] = rgba_u8(0xff, 0xff, 0xff, 0x0f);
    /// Active tab text (#eeeef8).
    pub const TAB_ACTIVE_FG: [u8; 3] = [0xee, 0xee, 0xf8];
    /// Inactive tab text (#70708a).
    pub const TAB_INACTIVE_FG: [u8; 3] = [0x70, 0x70, 0x8a];
    /// Tab close-button icon color when dim.
    pub const TAB_CLOSE_DIM: [u8; 3] = [0x3a, 0x3a, 0x52];

    // === Status bar ===
    /// Status bar background — deepest obsidian (#07070b).
    pub const STATUS_BAR_BG_ACTIVE: [f32; 4] = rgba_u8(0x07, 0x07, 0x0b, 0xff);
    /// Status bar background when idle — same as active.
    pub const STATUS_BAR_BG_IDLE: [f32; 4] = rgba_u8(0x07, 0x07, 0x0b, 0xff);
    /// Status bar text (#70708a).
    pub const STATUS_BAR_FG: [u8; 3] = [0x70, 0x70, 0x8a];

    // === Accent (shared) ===
    /// Primary accent: soft violet (#9580ff).
    pub const ACCENT_BLUE: [f32; 4] = rgba_u8(0x95, 0x80, 0xff, 0xff);
    /// Accent tint for active elements.
    pub const ACCENT_TINT: [f32; 4] = rgba_u8(0x95, 0x80, 0xff, 0x26);
    /// Accent text (#c4b4ff).
    pub const ACCENT_TEXT: [u8; 3] = [0xc4, 0xb4, 0xff];
    /// Diff added / success green (#5ddd8e).
    pub const DIFF_ADDED: [f32; 4] = rgba_u8(0x5d, 0xdd, 0x8e, 0xff);
    /// Diff modified / warning amber (#f0b454).
    pub const DIFF_MODIFIED: [f32; 4] = rgba_u8(0xf0, 0xb4, 0x54, 0xff);
    /// Diff removed / error red (#e85c6e).
    pub const DIFF_REMOVED: [f32; 4] = rgba_u8(0xe8, 0x5c, 0x6e, 0xff);

    // === Chrome overlay (palettes / modals) ===
    /// Overlay panel background (#101018 near-opaque).
    pub const OVERLAY_BG: [f32; 4] = rgba_u8(0x10, 0x10, 0x18, 0xf8);
    /// Overlay border.
    pub const OVERLAY_BORDER: [f32; 4] = rgba_u8(0xff, 0xff, 0xff, 0x1c);
    /// Overlay row text (#70708a).
    pub const OVERLAY_FG: [u8; 3] = [0x70, 0x70, 0x8a];

    // === Syntax — bright & crisp (Antigravity IDE palette) ===
    /// Keyword (fn, if, let, struct) — soft violet (#d09fff).
    pub const SYNTAX_KEYWORD: [u8; 3] = [0xd0, 0x9f, 0xff];
    /// Control-flow keyword (return, break, match) — same as keyword.
    pub const SYNTAX_CONTROL: [u8; 3] = [0xd0, 0x9f, 0xff];
    /// String literal — bright green (#72e898).
    pub const SYNTAX_STRING: [u8; 3] = [0x72, 0xe8, 0x98];
    /// Numeric literal — amber (#ffbe6a).
    pub const SYNTAX_NUMBER: [u8; 3] = [0xff, 0xbe, 0x6a];
    /// Comment — muted purple (#4a4a6a).
    pub const SYNTAX_COMMENT: [u8; 3] = [0x4a, 0x4a, 0x6a];
    /// Type name (struct, enum, trait) — sky blue (#90c4ff).
    pub const SYNTAX_TYPE: [u8; 3] = [0x90, 0xc4, 0xff];
    /// Function call / definition — warm yellow (#ffd57e).
    pub const SYNTAX_FUNCTION: [u8; 3] = [0xff, 0xd5, 0x7e];
    /// Attribute / macro / lifetime — coral (#f08080).
    pub const SYNTAX_ATTRIBUTE: [u8; 3] = [0xf0, 0x80, 0x80];
    /// Operator / punctuation — muted (#80809c).
    pub const SYNTAX_OPERATOR: [u8; 3] = [0x80, 0x80, 0x9c];

    // === Agent panel surface ===
    /// Agent panel background (#0c0c12 — same as sidebar).
    pub const AGENT_BG: [f32; 4] = rgba_u8(0x0c, 0x0c, 0x12, 0xff);
    /// Agent panel input area (#101018).
    pub const AGENT_INPUT_BG: [f32; 4] = rgba_u8(0x10, 0x10, 0x18, 0xff);
    /// Agent panel border (#ffffff0f).
    pub const AGENT_BORDER: [f32; 4] = rgba_u8(0xff, 0xff, 0xff, 0x0f);
    /// Agent panel header / dim text (#3a3a52).
    pub const AGENT_HEADER_FG: [u8; 3] = [0x3a, 0x3a, 0x52];
    /// Agent send button background (violet accent).
    pub const AGENT_SEND_BG: [f32; 4] = rgba_u8(0x95, 0x80, 0xff, 0xff);
    /// Queued session dot (#3a3a52).
    pub const AGENT_QUEUED_DOT: [f32; 4] = rgba_u8(0x3a, 0x3a, 0x52, 0xff);

    // === Diff background ===
    /// Tool-message / diff subtle tint (#0e0e18).
    pub const DIFF_BG: [f32; 4] = rgba_u8(0x0e, 0x0e, 0x18, 0xff);

    // === Accent body text ===
    /// Bright violet readable on dark bg — assistant label (#c4b4ff).
    pub const ACCENT_BLUE_TEXT: [u8; 3] = [0xc4, 0xb4, 0xff];

    /// Re-export so callers can build one-off colours without a separate import.
    pub use super::rgba_u8;
}

/// Logical pixel spacing used by multiple chrome modules.
pub mod spacing {
    /// Horizontal pad between icon and text in rows.
    pub const ROW_GAP: f32 = 6.0;
    /// Vertical pad at the top of overlay modals.
    pub const OVERLAY_PAD: f32 = 8.0;
    /// Height of a one-line overlay entry (quick-open, command palette).
    pub const OVERLAY_ROW: f32 = 22.0;
}

/// Typography scale. Values are logical px; multiply by `scale_factor` at paint time.
pub mod typography {
    /// Editor body font size.
    pub const EDITOR_FONT_PX: f32 = 14.0;
    /// Chrome (tabs, status, sidebar) font size.
    pub const CHROME_FONT_PX: f32 = 13.0;
    /// Small-caps headings (sidebar title).
    pub const SECTION_HEADER_FONT_PX: f32 = 11.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_u8_round_trips_to_unit_floats() {
        assert_eq!(rgba_u8(0, 0, 0, 0), [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(rgba_u8(255, 255, 255, 255), [1.0, 1.0, 1.0, 1.0]);
        let mid = rgba_u8(128, 128, 128, 255);
        // 128 / 255 ≈ 0.5019607...
        assert!((mid[0] - 0.501_960_8).abs() < 1e-6);
    }

    #[test]
    fn palette_alpha_values_are_sane() {
        assert_eq!(palette::EDITOR_BG[3], 1.0);
        // Violet tint 0x26 alpha for focused sidebar rows.
        assert!((palette::SIDEBAR_ROW_FOCUS[3] - 0x26 as f32 / 255.0).abs() < 1e-6);
        // Overlay background near-opaque 0xf8.
        assert!((palette::OVERLAY_BG[3] - 0xf8 as f32 / 255.0).abs() < 1e-6);
    }
}
