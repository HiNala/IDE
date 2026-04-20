//! Blocking file load (mmap or read, encoding + line endings).

use std::fs::File;
use std::io::Read;
use std::path::Path;

use editor_core::{LineEnding, TextBuffer};
use memmap2::Mmap;
use tracing::{instrument, warn};

use crate::types::{Encoding, LoadError, LoadedFile};

/// Files this size or larger use mmap (with read fallback).
pub const MMAP_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;

/// Read full file into memory, using mmap for large files when possible.
#[instrument(fields(path = %path.display()), skip(path))]
pub fn load_file_sync(path: &Path) -> Result<LoadedFile, LoadError> {
    let meta = std::fs::metadata(path)?;
    if meta.is_dir() {
        return Err(LoadError::NotAFile);
    }
    let byte_size = meta.len();
    let mtime = meta.modified()?;

    let (bytes, was_memory_mapped) = if byte_size >= MMAP_THRESHOLD_BYTES {
        match read_via_mmap(path) {
            Ok(b) => (b, true),
            Err(e) => {
                warn!(error = %e, "mmap failed; falling back to streaming read");
                (read_file_streaming(path)?, false)
            }
        }
    } else {
        (std::fs::read(path)?, false)
    };

    let (buffer, encoding) = decode_to_buffer(&bytes)?;
    Ok(LoadedFile {
        buffer,
        path: path.to_path_buf(),
        encoding,
        byte_size,
        mtime,
        was_memory_mapped,
    })
}

fn read_via_mmap(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let file = File::open(path)?;
    // SAFETY: We map the file read-only, copy into a `Vec` before unmapping, and do not mutate
    // the file while mapped. See memmap2 docs: mapping is valid for the file's length at map time.
    let mmap = unsafe { Mmap::map(&file)? };
    Ok(mmap.to_vec())
}

fn read_file_streaming(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len() as usize;
    let mut v = Vec::with_capacity(size.min(1usize << 30));
    let mut buf = [0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        v.extend_from_slice(&buf[..n]);
    }
    Ok(v)
}

fn decode_to_buffer(bytes: &[u8]) -> Result<(TextBuffer, Encoding), LoadError> {
    if bytes.is_empty() {
        return Ok((TextBuffer::new(), Encoding::Utf8));
    }

    // UTF-8 with BOM
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return decode_utf8_slice(&bytes[3..], Encoding::Utf8Bom);
    }

    // UTF-16 LE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        return decode_utf16(&bytes[2..], encoding_rs::UTF_16LE, Encoding::Utf16Le);
    }

    // UTF-16 BE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return decode_utf16(&bytes[2..], encoding_rs::UTF_16BE, Encoding::Utf16Be);
    }

    // UTF-8 (strict, then lossy)
    decode_utf8_slice(bytes, Encoding::Utf8)
}

fn decode_utf8_slice(bytes: &[u8], enc: Encoding) -> Result<(TextBuffer, Encoding), LoadError> {
    match std::str::from_utf8(bytes) {
        Ok(s) => {
            let le = LineEnding::detect(s);
            let normalized = LineEnding::normalize_to(LineEnding::Lf, s);
            Ok((TextBuffer::with_line_ending(&normalized, le), enc))
        }
        Err(_) => {
            let s = String::from_utf8_lossy(bytes);
            warn!("file decoded with UTF-8 replacement characters");
            let le = LineEnding::detect(s.as_ref());
            let normalized = LineEnding::normalize_to(LineEnding::Lf, s.as_ref());
            Ok((TextBuffer::with_line_ending(&normalized, le), Encoding::LossyUtf8))
        }
    }
}

fn decode_utf16(
    bytes: &[u8],
    dec: &'static encoding_rs::Encoding,
    enc: Encoding,
) -> Result<(TextBuffer, Encoding), LoadError> {
    let (cow, _, had_errors) = dec.decode(bytes);
    if had_errors {
        warn!("UTF-16 decode reported errors; output may be lossy");
    }
    let s = cow.into_owned();
    let le = LineEnding::detect(&s);
    let normalized = LineEnding::normalize_to(LineEnding::Lf, &s);
    Ok((TextBuffer::with_line_ending(&normalized, le), enc))
}
