//! `editor-input` — OS input → editor operations.
//!
//! This crate translates raw `winit::event::WindowEvent` values into
//! high-level editor `Action`s that `editor-app` applies against
//! `editor-core`. It never performs edits itself.
//!
//! Mission status:
//! - **M01 (current):** crate scaffolded, builds, one smoke test.
//! - **M05:** full translator, key-binding table, primary-cursor motion.
//! - **V2 (M09):** selection, clipboard shortcuts, word-level navigation.
//!
//! See `docs/INPUT_PIPELINE.md` for the design.

#![forbid(unsafe_code)]

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a human-readable banner identifying this crate.
#[must_use]
pub fn banner() -> String {
    format!("editor-input v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_crate_name_and_version() {
        let b = banner();
        assert!(b.starts_with("editor-input v"), "banner = {b:?}");
        assert!(b.contains(VERSION), "banner = {b:?}");
    }
}
