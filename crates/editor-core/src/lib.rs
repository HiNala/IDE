//! `editor-core` — the text engine.
//!
//! This crate owns the document model: the rope buffer, cursor, selection, and
//! undo/redo stack. It has no GPU, OS, or async dependency.
//!
//! See `docs/TEXT_ENGINE.md` for the design.

#![forbid(unsafe_code)]

pub mod error;

pub use error::{CoreError, CoreResult};

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a human-readable banner identifying this crate.
#[must_use]
pub fn banner() -> String {
    format!("editor-core v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_crate_name_and_version() {
        let b = banner();
        assert!(b.starts_with("editor-core v"), "banner = {b:?}");
        assert!(b.contains(VERSION), "banner = {b:?}");
    }

    #[test]
    fn version_is_non_empty() {
        assert!(!VERSION.is_empty());
    }
}
