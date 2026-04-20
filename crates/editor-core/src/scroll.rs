//! Vertical scroll offset for viewing the document buffer.
//!
//! Values are in **physical pixels** at the current scale factor.

/// Y-offset in pixels (downward is positive).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScrollOffset {
    /// Vertical scroll in physical pixels.
    pub y_px: f32,
}

impl ScrollOffset {
    /// Creates a scroll offset with the given Y position.
    #[must_use]
    pub const fn new(y_px: f32) -> Self {
        Self { y_px }
    }
}
