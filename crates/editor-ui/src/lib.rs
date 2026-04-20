//! `editor-ui` — minimal UI composition (gutter, status bar in V2).
//!
//! This crate stays free of GPU and windowing types. It owns layout for chrome
//! around the text canvas and will consume snapshots from `editor-core` in
//! later missions.

#![forbid(unsafe_code)]

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a human-readable banner identifying this crate.
#[must_use]
pub fn banner() -> String {
    format!("editor-ui v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_crate_name_and_version() {
        let b = banner();
        assert!(b.starts_with("editor-ui v"), "banner = {b:?}");
        assert!(b.contains(VERSION), "banner = {b:?}");
    }
}
