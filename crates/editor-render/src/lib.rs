//! `editor-render` — GPU rendering for the IDE project.
//!
//! This crate owns all `wgpu` state (Instance, Adapter, Device, Queue,
//! Surface) and drives the `glyphon` text renderer. No other crate is
//! permitted to hold a `wgpu::Device`.
//!
//! Mission status:
//! - **M01 (current):** crate scaffolded, builds, one smoke test.
//! - **M03:** wgpu init, surface management, clear-color frame.
//! - **M04:** glyphon integration, visible rope content rendered.
//!
//! See `docs/RENDERING.md` for the design.

#![forbid(unsafe_code)]

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
}
