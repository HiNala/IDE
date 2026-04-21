//! `editor-io` — file I/O (mmap load, atomic save, encoding + line endings).
//!
//! This crate is the only place in the workspace allowed to touch disk for
//! editor buffers. The only `unsafe` is in [`load`](crate::load) for `memmap2`.
//!
//! See `docs/missions/M06_FILE_IO.md`.

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_debug_implementations)]

mod load;
mod paths;
mod save;
mod types;

pub use load::{load_file_async, load_file_sync, MMAP_THRESHOLD_BYTES};
pub use save::{save_file_async, save_file_sync};
pub use types::{Encoding, LoadError, LoadProgress, LoadedFile, SaveError};

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
    use std::io::Write;
    use std::path::PathBuf;

    use editor_core::TextBuffer;
    use tempfile::tempdir;

    #[test]
    fn banner_contains_crate_name_and_version() {
        let b = banner();
        assert!(b.starts_with("editor-io v"), "banner = {b:?}");
        assert!(b.contains(VERSION), "banner = {b:?}");
    }

    #[test]
    fn roundtrip_utf8_crlf() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let p = dir.path().join("t.txt");
        std::fs::write(&p, b"a\r\nb\r\n")?;
        let loaded = load_file_sync(&p)?;
        assert_eq!(loaded.buffer.original_line_ending(), editor_core::LineEnding::Crlf);
        let snap = loaded.buffer.snapshot();
        save_file_sync(&p, &snap, loaded.buffer.original_line_ending(), Encoding::Utf8)?;
        let bytes = std::fs::read(&p)?;
        assert_eq!(bytes, b"a\r\nb\r\n");
        Ok(())
    }

    #[test]
    fn utf8_bom_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let p = dir.path().join("bom.txt");
        let mut f = std::fs::File::create(&p)?;
        f.write_all(&[0xEF, 0xBB, 0xBF])?;
        f.write_all(b"hi\n")?;
        drop(f);
        let loaded = load_file_sync(&p)?;
        assert_eq!(loaded.encoding, Encoding::Utf8Bom);
        let snap = loaded.buffer.snapshot();
        save_file_sync(&p, &snap, loaded.buffer.original_line_ending(), Encoding::Utf8Bom)?;
        let bytes = std::fs::read(&p)?;
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        Ok(())
    }

    #[test]
    fn directory_errors() {
        let dir = tempdir().unwrap();
        let err = load_file_sync(dir.path()).unwrap_err();
        assert!(matches!(err, LoadError::NotAFile));
    }

    #[test]
    fn empty_file() -> Result<(), LoadError> {
        let dir = tempfile::tempdir().map_err(LoadError::Io)?;
        let p = dir.path().join("empty.txt");
        std::fs::write(&p, b"").map_err(LoadError::Io)?;
        let loaded = load_file_sync(&p)?;
        assert_eq!(loaded.buffer.len_bytes(), 0);
        Ok(())
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn reserved_name_errors() {
        let p = PathBuf::from("CON");
        let b = TextBuffer::from_str("x");
        let snap = b.snapshot();
        let e = save_file_sync(&p, &snap, b.original_line_ending(), Encoding::Utf8).unwrap_err();
        assert!(matches!(e, SaveError::ReservedName(_)));
    }
}
