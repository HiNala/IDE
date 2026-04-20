//! `editor-io` — file I/O (async, mmap, atomic save).
//!
//! This crate is the only place in the workspace allowed to touch disk.
//! All operations happen off the main thread; results are delivered back via
//! bounded channels owned by `editor-app`.
//!
//! Mission status:
//! - **M01 (current):** crate scaffolded, builds, one smoke test.
//! - **M06:** async streaming load, memory-mapped large-file load,
//!   atomic save, line-ending / encoding detection.
//!
//! See `docs/FILE_IO.md` for the design.

#![forbid(unsafe_code)]

/// Crate version string, sourced from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a human-readable banner identifying this crate.
#[must_use]
pub fn banner() -> String {
    format!("editor-io v{VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_crate_name_and_version() {
        let b = banner();
        assert!(b.starts_with("editor-io v"), "banner = {b:?}");
        assert!(b.contains(VERSION), "banner = {b:?}");
    }
}
