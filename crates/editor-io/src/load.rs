//! Blocking file load (mmap or read, encoding + line endings).

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crossbeam_channel::Sender;
use editor_core::{JobToken, LineEnding, TextBuffer, WorkerPool};
use memmap2::Mmap;
use tracing::{instrument, warn};

use crate::types::{Encoding, LoadError, LoadProgress, LoadedFile};

/// Files this size or larger use mmap (with read fallback).
pub const MMAP_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;

const PROGRESS_STRIDE_BYTES: u64 = 1024 * 1024;

/// Read full file into memory, using mmap for large files when possible.
#[instrument(fields(path = %path.display()), skip(path))]
pub fn load_file_sync(path: &Path) -> Result<LoadedFile, LoadError> {
    load_file_inner(path, None, None)
}

/// Background load with cooperative cancellation and optional progress (streaming reads).
///
/// Poll [`crossbeam_channel::Receiver::try_recv`] on the main thread; terminal messages are
/// [`LoadProgress::Done`], [`LoadProgress::Error`], or [`LoadProgress::Cancelled`].
#[must_use]
pub fn load_file_async(
    pool: &WorkerPool,
    path: PathBuf,
) -> (JobToken, crossbeam_channel::Receiver<LoadProgress>) {
    let (tx, rx) = crossbeam_channel::bounded::<LoadProgress>(128);
    let (token, done_rx) = pool.spawn(move |token| {
        if token.is_cancelled() {
            let _ = tx.send(LoadProgress::Cancelled);
            return;
        }
        match load_file_inner(&path, Some(token), Some(&tx)) {
            Ok(l) => {
                let _ = tx.send(LoadProgress::Done(l));
            }
            Err(LoadError::Cancelled) => {
                let _ = tx.send(LoadProgress::Cancelled);
            }
            Err(e) => {
                let _ = tx.send(LoadProgress::Error(e));
            }
        }
        drop(tx);
    });
    drop(done_rx);
    (token, rx)
}

fn load_file_inner(
    path: &Path,
    token: Option<&JobToken>,
    progress: Option<&Sender<LoadProgress>>,
) -> Result<LoadedFile, LoadError> {
    if let Some(t) = token {
        if t.is_cancelled() {
            return Err(LoadError::Cancelled);
        }
    }

    let meta = std::fs::metadata(path)?;
    if meta.is_dir() {
        return Err(LoadError::NotAFile);
    }
    let byte_size = meta.len();
    let mtime = meta.modified()?;

    if let Some(tx) = progress {
        let _ = tx.send(LoadProgress::Started { total_bytes: byte_size });
    }

    if let Some(t) = token {
        if t.is_cancelled() {
            return Err(LoadError::Cancelled);
        }
    }

    let (bytes, was_memory_mapped) = if byte_size >= MMAP_THRESHOLD_BYTES {
        match read_via_mmap(path) {
            Ok(b) => (b, true),
            Err(e) => {
                warn!(error = %e, "mmap failed; falling back to streaming read");
                (read_file_streaming(path, byte_size, token, progress)?, false)
            }
        }
    } else {
        (std::fs::read(path)?, false)
    };

    if let Some(t) = token {
        if t.is_cancelled() {
            return Err(LoadError::Cancelled);
        }
    }

    let (buffer, encoding) = decode_to_buffer(&bytes)?;
    let line_ending = buffer.original_line_ending();
    Ok(LoadedFile {
        buffer,
        path: path.to_path_buf(),
        line_ending,
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

fn read_file_streaming(
    path: &Path,
    total_hint: u64,
    token: Option<&JobToken>,
    progress: Option<&Sender<LoadProgress>>,
) -> Result<Vec<u8>, LoadError> {
    let mut f = File::open(path)?;
    let size = f.metadata()?.len() as usize;
    let mut v = Vec::with_capacity(size.min(1usize << 30));
    let mut buf = [0u8; 1 << 20];
    let mut read_total = 0u64;
    let mut next_progress = PROGRESS_STRIDE_BYTES.min(total_hint).max(1);
    loop {
        if let Some(t) = token {
            if t.is_cancelled() {
                return Err(LoadError::Cancelled);
            }
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        v.extend_from_slice(&buf[..n]);
        read_total += n as u64;
        if let Some(tx) = progress {
            if read_total >= next_progress {
                let _ = tx.send(LoadProgress::Progress { bytes_read: read_total });
                next_progress = read_total.saturating_add(PROGRESS_STRIDE_BYTES);
            }
        }
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
