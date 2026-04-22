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
pub mod palette {
    use super::rgba_u8;

    // === Editor surface ===
    /// Main editor background (#1e1e1e).
    pub const EDITOR_BG: [f32; 4] = rgba_u8(0x1e, 0x1e, 0x1e, 0xff);
    /// Editor body text (#d4d4d4).
    pub const EDITOR_FG: [u8; 3] = [0xd4, 0xd4, 0xd4];
    /// Dim text: line numbers, inactive hints (#858585).
    pub const EDITOR_FG_DIM: [u8; 3] = [0x85, 0x85, 0x85];

    // === Activity bar ===
    /// Activity bar background (#333334).
    pub const ACTIVITY_BG: [f32; 4] = rgba_u8(0x33, 0x33, 0x34, 0xff);
    /// Active icon foreground.
    pub const ACTIVITY_FG_ACTIVE: [u8; 3] = [0xff, 0xff, 0xff];
    /// Inactive icon foreground.
    pub const ACTIVITY_FG_INACTIVE: [u8; 3] = [0x85, 0x85, 0x85];

    // === Sidebar ===
    /// Sidebar background (#252526).
    pub const SIDEBAR_BG: [f32; 4] = rgba_u8(0x25, 0x25, 0x26, 0xff);
    /// Hovered row tint (#2a2d2e).
    pub const SIDEBAR_ROW_HOVER: [f32; 4] = rgba_u8(0x2a, 0x2d, 0x2e, 0xff);
    /// Focused / selected row (#04558a with alpha).
    pub const SIDEBAR_ROW_FOCUS: [f32; 4] = rgba_u8(0x04, 0x55, 0x8a, 0xd8);
    /// Sidebar header foreground (#bbbbbb).
    pub const SIDEBAR_HEADER_FG: [u8; 3] = [0xbb, 0xbb, 0xbb];
    /// Sidebar row text (#cccccc).
    pub const SIDEBAR_ROW_FG: [u8; 3] = [0xcc, 0xcc, 0xcc];

    // === Tab strip ===
    /// Tab strip background (#2d2d2d).
    pub const TAB_STRIP_BG: [f32; 4] = rgba_u8(0x2d, 0x2d, 0x2d, 0xff);
    /// Inactive tab fill (identical to strip by convention).
    pub const TAB_INACTIVE_BG: [f32; 4] = rgba_u8(0x2d, 0x2d, 0x2d, 0xff);
    /// Active tab fill (matches editor bg).
    pub const TAB_ACTIVE_BG: [f32; 4] = rgba_u8(0x1e, 0x1e, 0x1e, 0xff);
    /// Tab separator line (#191919).
    pub const TAB_SEPARATOR: [f32; 4] = rgba_u8(0x19, 0x19, 0x19, 0xff);
    /// Active tab text.
    pub const TAB_ACTIVE_FG: [u8; 3] = [0xff, 0xff, 0xff];
    /// Inactive tab text (#969696).
    pub const TAB_INACTIVE_FG: [u8; 3] = [0x96, 0x96, 0x96];
    /// Tab close-button icon color when dim (#7a7a7a).
    pub const TAB_CLOSE_DIM: [u8; 3] = [0x7a, 0x7a, 0x7a];

    // === Status bar ===
    /// Status bar background when a workspace is open (#007acc).
    pub const STATUS_BAR_BG_ACTIVE: [f32; 4] = rgba_u8(0x00, 0x7a, 0xcc, 0xff);
    /// Status bar background when idle (#333333).
    pub const STATUS_BAR_BG_IDLE: [f32; 4] = rgba_u8(0x33, 0x33, 0x33, 0xff);
    /// Status bar text (#ffffff).
    pub const STATUS_BAR_FG: [u8; 3] = [0xff, 0xff, 0xff];

    // === Accent (shared) ===
    /// Primary accent: blue (#007acc). Used for tab underlines and focus rings.
    pub const ACCENT_BLUE: [f32; 4] = rgba_u8(0x00, 0x7a, 0xcc, 0xff);
    /// Diff added (#3fb950).
    pub const DIFF_ADDED: [f32; 4] = rgba_u8(0x3f, 0xb9, 0x50, 0xff);
    /// Diff modified (#d29922).
    pub const DIFF_MODIFIED: [f32; 4] = rgba_u8(0xd2, 0x99, 0x22, 0xff);
    /// Diff removed (#f85149).
    pub const DIFF_REMOVED: [f32; 4] = rgba_u8(0xf8, 0x51, 0x49, 0xff);

    // === Chrome overlay (palettes / modals) ===
    /// Overlay panel background (#252526).
    pub const OVERLAY_BG: [f32; 4] = rgba_u8(0x25, 0x25, 0x26, 0xf5);
    /// Overlay border / subtle divider (#3c3c3c).
    pub const OVERLAY_BORDER: [f32; 4] = rgba_u8(0x3c, 0x3c, 0x3c, 0xff);
    /// Overlay row text.
    pub const OVERLAY_FG: [u8; 3] = [0xcc, 0xcc, 0xcc];

    // === Syntax highlight slots (M15). ===
    /// Keyword (fn, if, let, struct). (#c586c0)
    pub const SYNTAX_KEYWORD: [u8; 3] = [0xc5, 0x86, 0xc0];
    /// Control-flow keyword (return, break, match). (#569cd6)
    pub const SYNTAX_CONTROL: [u8; 3] = [0x56, 0x9c, 0xd6];
    /// String literal. (#ce9178)
    pub const SYNTAX_STRING: [u8; 3] = [0xce, 0x91, 0x78];
    /// Numeric literal. (#b5cea8)
    pub const SYNTAX_NUMBER: [u8; 3] = [0xb5, 0xce, 0xa8];
    /// Comment. (#6a9955)
    pub const SYNTAX_COMMENT: [u8; 3] = [0x6a, 0x99, 0x55];
    /// Type name (struct, enum, trait). (#4ec9b0)
    pub const SYNTAX_TYPE: [u8; 3] = [0x4e, 0xc9, 0xb0];
    /// Function call / definition. (#dcdcaa)
    pub const SYNTAX_FUNCTION: [u8; 3] = [0xdc, 0xdc, 0xaa];
    /// Attribute / macro / lifetime. (#9cdcfe)
    pub const SYNTAX_ATTRIBUTE: [u8; 3] = [0x9c, 0xdc, 0xfe];
    /// Operator / punctuation. (#d4d4d4)
    pub const SYNTAX_OPERATOR: [u8; 3] = [0xd4, 0xd4, 0xd4];
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
        assert_eq!(palette::SIDEBAR_ROW_FOCUS[3], 216.0 / 255.0);
        assert_eq!(palette::OVERLAY_BG[3], 0xf5 as f32 / 255.0);
    }
}
