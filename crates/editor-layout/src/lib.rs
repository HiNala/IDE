//! Flexbox layout for IDE chrome, backed by **taffy** with IDE-facing result types.
//!
//! Callers that need glyph metrics pass a [`TextMeasure`]; the default shell wires
//! `editor_render::TextLayer` in a later step.
//!
//! See `docs/UI_STRATEGY.md` §4.

#![forbid(unsafe_code)]

mod engine;
mod status_bar;

pub mod chrome_shell;

pub use chrome_shell::{
    build_chrome_tree, compute_main_chrome_layout, format_main_chrome_layout_golden,
    main_chrome_to_layout_result, ChromeWidgetId, MainChromeLayout, MainChromeParams,
};
pub use engine::{LayoutEngine, LayoutError};
pub use status_bar::layout_status_bar_row;

/// Axis-aligned rectangle in **physical** or **logical** pixels (callers must be consistent per tree).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayoutRect {
    #[must_use]
    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
}

/// One laid-out region tagged with a stable widget id (maps to `FrameChrome` / hit tests).
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutItem {
    pub widget_id: u64,
    pub rect: LayoutRect,
}

/// Flattened result of a layout pass, suitable for paint and hit testing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutResult {
    pub items: Vec<LayoutItem>,
    /// Size reported by taffy for the laid-out root (width, height).
    pub root_width: f32,
    pub root_height: f32,
}

/// Text measurement for intrinsic leaf sizing (implemented by `editor-render` in M30).
pub trait TextMeasure {
    /// Returns pixel width, pixel height, and whether the string was truncated to `max_width_px`.
    fn measure(&self, text: &str, max_width_px: f32) -> (f32, f32, bool);
}

/// Deterministic stub for unit tests; each codepoint contributes a fixed width.
#[derive(Debug, Clone, Copy)]
pub struct MonospaceWidthMeasure {
    /// Advance per character in px (e.g. 8.0 for 8px monospace).
    pub char_width: f32,
    pub line_height: f32,
}

impl MonospaceWidthMeasure {
    /// Single-line: width = `text.chars().count() * char_width`, min with max_width, ellipsis not modeled.
    #[must_use]
    pub const fn new(char_width: f32, line_height: f32) -> Self {
        Self { char_width, line_height }
    }
}

impl TextMeasure for MonospaceWidthMeasure {
    fn measure(&self, text: &str, max_width_px: f32) -> (f32, f32, bool) {
        let w = text.chars().count() as f32 * self.char_width;
        let (w, truncated) =
            if w > max_width_px && max_width_px > 0.0 { (max_width_px, true) } else { (w, false) };
        (w, self.line_height, truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monospace_measure_respects_max_width() {
        let m = MonospaceWidthMeasure::new(8.0, 16.0);
        let (w, h, trunc) = m.measure("hello", 32.0);
        assert!((w - 32.0).abs() < f32::EPSILON);
        assert!((h - 16.0).abs() < f32::EPSILON);
        assert!(trunc);
    }
}
