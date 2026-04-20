//! `editor-render` — GPU rendering (`wgpu`; `glyphon` in M04).
//!
//! This crate owns all `wgpu` state. No other crate holds a `wgpu::Device`.
//!
//! See `docs/RENDERING_PIPELINE.md`.

#![forbid(unsafe_code)]

mod editor_renderer;
mod error;
mod gpu;
mod text_layer;

pub use editor_renderer::{EditorRenderer, FrameInput};
pub use error::RenderError;
pub use gpu::{dry_run_headless, GpuContext};
pub use text_layer::TextLayer;

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a human-readable banner identifying this crate.
#[must_use]
pub fn banner() -> String {
    format!("editor-render v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_crate_name_and_version() {
        let b = banner();
        assert!(b.starts_with("editor-render v"), "banner = {b:?}");
        assert!(b.contains(VERSION), "banner = {b:?}");
    }

    #[test]
    fn dry_run_headless_smoke() {
        // GitHub Actions Linux runners may not expose a usable adapter; allow failure.
        let _ = dry_run_headless();
    }
}
