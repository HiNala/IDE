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
/// Design: deep ink base + raised chrome + electric violet accents (Cursor / 2025 dark).
/// Surfaces: shell #08080d → canvas ~#08080c → panels #0c0c14 → inputs #12121c.
pub mod palette {
    // === Editor surface ===
    /// Main editor canvas — near-black (reads “true dark” on OLED-style UIs).
    pub const EDITOR_BG: [f32; 4] = rgba_u8(0x08, 0x08, 0x0c, 0xff);
    /// Editor body text — near-white (#f4f4fa).
    pub const EDITOR_FG: [u8; 3] = [0xf4, 0xf4, 0xfa];
    /// Dim text: line numbers, inactive hints (cool gray).
    pub const EDITOR_FG_DIM: [u8; 3] = [0x78, 0x7b, 0x92];

    // === Activity bar (retained for layout but width=0 in this design) ===
    pub const ACTIVITY_BG: [f32; 4] = rgba_u8(0x08, 0x08, 0x0d, 0xff);
    pub const ACTIVITY_FG_ACTIVE: [u8; 3] = [0xdd, 0xd2, 0xff];
    pub const ACTIVITY_FG_INACTIVE: [u8; 3] = [0x4a, 0x4c, 0x5e];

    // === Sidebar ===
    /// Sidebar — half-step above canvas.
    pub const SIDEBAR_BG: [f32; 4] = rgba_u8(0x0c, 0x0c, 0x14, 0xff);
    /// Hovered row tint.
    pub const SIDEBAR_ROW_HOVER: [f32; 4] = rgba_u8(0x16, 0x16, 0x22, 0xff);
    /// Focused / selected row (violet wash).
    pub const SIDEBAR_ROW_FOCUS: [f32; 4] = rgba_u8(0xa8, 0x55, 0xf7, 0x2a);
    /// Sidebar active accent left bar.
    pub const SIDEBAR_ACCENT: [f32; 4] = rgba_u8(0xbf, 0x6b, 0xff, 0xff);
    /// Sidebar header label.
    pub const SIDEBAR_HEADER_FG: [u8; 3] = [0x6a, 0x6c, 0x82];
    /// Sidebar row text.
    pub const SIDEBAR_ROW_FG: [u8; 3] = [0x9a, 0x9c, 0xb2];
    /// Hairline border (~8% white) — clean separation.
    pub const SIDEBAR_BORDER: [f32; 4] = rgba_u8(0xff, 0xff, 0xff, 0x14);
    /// Sidebar git: modified file — amber.
    pub const SIDEBAR_GIT_MODIFIED: [u8; 3] = [0xf5, 0xa6, 0x23];
    /// Sidebar git: untracked file — soft green.
    pub const SIDEBAR_GIT_UNTRACKED: [u8; 3] = [0x7e, 0xc6, 0x99];
    /// Sidebar git: newly added/staged file — bright green.
    pub const SIDEBAR_GIT_ADDED: [u8; 3] = [0x5d, 0xdd, 0x8e];

    // === Tab strip ===
    pub const TAB_STRIP_BG: [f32; 4] = rgba_u8(0x0c, 0x0c, 0x14, 0xff);
    /// Inactive tab fill.
    pub const TAB_INACTIVE_BG: [f32; 4] = rgba_u8(0x0c, 0x0c, 0x14, 0xff);
    /// Active tab — same as editor canvas.
    pub const TAB_ACTIVE_BG: [f32; 4] = EDITOR_BG;
    /// Global hairline: `rgba(255,255,255,0.06)` (reference shell).
    pub const HAIRLINE: [f32; 4] = [1.0, 1.0, 1.0, 0.06];
    /// Tab separator / bottom hairline.
    pub const TAB_SEPARATOR: [f32; 4] = rgba_u8(0xff, 0xff, 0xff, 0x14);
    /// Active tab text (#eeeef8).
    pub const TAB_ACTIVE_FG: [u8; 3] = [0xee, 0xee, 0xf8];
    /// Inactive tab label — softer than body text.
    pub const TAB_INACTIVE_FG: [u8; 3] = [0x6e, 0x72, 0x8a];
    /// Tab close-button icon color when dim.
    pub const TAB_CLOSE_DIM: [u8; 3] = [0x5a, 0x5c, 0x6e];

    // === Status bar ===
    /// [`STATUS_BAR_BG`] / active strip (Cursor reference #080810).
    pub const STATUS_BAR_BG: [f32; 4] = rgba_u8(0x08, 0x08, 0x10, 0xff);
    /// Deepest shell strip — below panels.
    pub const STATUS_BAR_BG_ACTIVE: [f32; 4] = STATUS_BAR_BG;
    pub const STATUS_BAR_BG_IDLE: [f32; 4] = STATUS_BAR_BG;
    /// Muted label text (readability on #08080d).
    pub const STATUS_BAR_FG: [u8; 3] = [0x8a, 0x8c, 0xa3];

    // === Accent (shared) ===
    /// Primary accent — violet-500 class (#A855F7), matches reference IDEs.
    pub const ACCENT_BLUE: [f32; 4] = rgba_u8(0xa8, 0x55, 0xf7, 0xff);
    /// Focus rings / list wash.
    pub const ACCENT_TINT: [f32; 4] = rgba_u8(0xa8, 0x55, 0xf7, 0x33);
    /// On-accent and bright labels.
    pub const ACCENT_TEXT: [u8; 3] = [0xec, 0xdd, 0xff];
    /// Diff added / success green (#5ddd8e).
    pub const DIFF_ADDED: [f32; 4] = rgba_u8(0x5d, 0xdd, 0x8e, 0xff);
    /// Diff modified / warning amber (#f0b454).
    pub const DIFF_MODIFIED: [f32; 4] = rgba_u8(0xf0, 0xb4, 0x54, 0xff);
    /// Diff removed / error red (#e85c6e).
    pub const DIFF_REMOVED: [f32; 4] = rgba_u8(0xe8, 0x5c, 0x6e, 0xff);

    // === Chrome overlay (palettes / modals) ===
    pub const OVERLAY_BG: [f32; 4] = rgba_u8(0x12, 0x12, 0x1c, 0xfa);
    pub const OVERLAY_BORDER: [f32; 4] = rgba_u8(0xff, 0xff, 0xff, 0x18);
    /// Overlay row text (#70708a).
    pub const OVERLAY_FG: [u8; 3] = [0x70, 0x70, 0x8a];

    // === Syntax — high-contrast on ink ===
    /// Keyword (fn, if, let, struct).
    pub const SYNTAX_KEYWORD: [u8; 3] = [0xda, 0xae, 0xff];
    /// Control-flow keyword (return, break, match) — same as keyword.
    pub const SYNTAX_CONTROL: [u8; 3] = [0xd0, 0x9f, 0xff];
    /// String literal — bright green (#72e898).
    pub const SYNTAX_STRING: [u8; 3] = [0x72, 0xe8, 0x98];
    /// Numeric literal — amber (#ffbe6a).
    pub const SYNTAX_NUMBER: [u8; 3] = [0xff, 0xbe, 0x6a];
    /// Comment — cool slate (reads on #08080c).
    pub const SYNTAX_COMMENT: [u8; 3] = [0x5c, 0x5e, 0x78];
    /// Type name (struct, enum, trait) — sky blue (#90c4ff).
    pub const SYNTAX_TYPE: [u8; 3] = [0x90, 0xc4, 0xff];
    /// Function call / definition — warm yellow (#ffd57e).
    pub const SYNTAX_FUNCTION: [u8; 3] = [0xff, 0xd5, 0x7e];
    /// Attribute / macro / lifetime — coral (#f08080).
    pub const SYNTAX_ATTRIBUTE: [u8; 3] = [0xf0, 0x80, 0x80];
    /// Operator / punctuation — muted (#80809c).
    pub const SYNTAX_OPERATOR: [u8; 3] = [0x80, 0x80, 0x9c];

    // === Agent panel surface (Cursor reference: #0a0a14 / #13131c) ===
    pub const AGENT_PANEL_BG: [f32; 4] = rgba_u8(0x0a, 0x0a, 0x14, 0xff);
    pub const AGENT_COMPOSER_BG: [f32; 4] = rgba_u8(0x13, 0x13, 0x1c, 0xff);
    /// Transcript scroll area (reference #0b0b14).
    pub const AGENT_TRANSCRIPT_BG: [f32; 4] = rgba_u8(0x0b, 0x0b, 0x14, 0xff);
    /// Embedded terminal body (reference #0a0a10).
    pub const AGENT_TERMINAL_BG: [f32; 4] = rgba_u8(0x0a, 0x0a, 0x10, 0xff);
    /// Main rail fill.
    pub const AGENT_BG: [f32; 4] = AGENT_PANEL_BG;
    /// Compose / model fields — raised surface.
    pub const AGENT_INPUT_BG: [f32; 4] = AGENT_COMPOSER_BG;
    pub const AGENT_BORDER: [f32; 4] = rgba_u8(0xff, 0xff, 0xff, 0x12);
    pub const AGENT_HEADER_FG: [u8; 3] = [0x6a, 0x6c, 0x84];
    /// Send — same family as `ACCENT_BLUE`, slightly bright for a primary CTA.
    pub const AGENT_SEND_BG: [f32; 4] = rgba_u8(0xb4, 0x6a, 0xff, 0xff);
    /// Queued session dot (#3a3a52).
    pub const AGENT_QUEUED_DOT: [f32; 4] = rgba_u8(0x3a, 0x3a, 0x52, 0xff);

    // === Diff background ===
    pub const DIFF_BG: [f32; 4] = rgba_u8(0x0a, 0x0a, 0x10, 0xff);

    // === Accent body text ===
    /// Readable on dark bg — links to [`ACCENT_TEXT`].
    pub const ACCENT_BLUE_TEXT: [u8; 3] = ACCENT_TEXT;

    /// Re-export so callers can build one-off colours without a separate import.
    pub use super::rgba_u8;
}

/// Logical pixel spacing used by multiple chrome modules.
pub mod spacing {
    /// Context file chip height (logical px; row may add padding).
    pub const CONTEXT_CHIP_HEIGHT: f32 = 22.0;
    /// Gap between context chips (logical px).
    pub const CONTEXT_CHIP_GAP: f32 = 8.0;
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
        // Focus wash uses ~0x2a alpha for focused sidebar rows.
        assert!((palette::SIDEBAR_ROW_FOCUS[3] - 0x2a as f32 / 255.0).abs() < 1e-6);
        // Overlay background near-opaque 0xfa.
        assert!((palette::OVERLAY_BG[3] - 0xfa as f32 / 255.0).abs() < 1e-6);
    }
}
