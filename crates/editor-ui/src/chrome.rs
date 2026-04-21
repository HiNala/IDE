//! GPU-friendly chrome primitives (sidebar, tab strip, quick open) — layout only, no wgpu.

/// Monospace text line at window pixel coordinates (Y-down).
#[derive(Debug, Clone)]
pub struct ChromeTextLine {
    pub left: f32,
    pub top: f32,
    pub text: String,
    pub rgb: [u8; 3],
}

/// Premultiplied-ish RGBA axis-aligned quad in window pixels.
#[derive(Debug, Clone, Copy)]
pub struct ChromeQuad {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub rgba: [f32; 4],
}

/// Bundles vectors built each frame for [`editor_render::FrameInput`].
#[derive(Debug, Default, Clone)]
pub struct FrameChrome {
    pub lines: Vec<ChromeTextLine>,
    pub quads: Vec<ChromeQuad>,
}

impl FrameChrome {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.quads.clear();
    }

    pub fn push_quad(&mut self, q: ChromeQuad) {
        self.quads.push(q);
    }

    pub fn push_line(&mut self, left: f32, top: f32, text: impl Into<String>, rgb: [u8; 3]) {
        self.lines.push(ChromeTextLine { left, top, text: text.into(), rgb });
    }
}
