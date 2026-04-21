//! Atomic save (temp file + rename) with encoding and line-ending preservation.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crossbeam_channel::Receiver;
use editor_core::{JobToken, LineEnding, TextBufferSnapshot, WorkerPool};
use ropey::Rope;
use tempfile::NamedTempFile;
use tracing::instrument;

use crate::paths::is_windows_reserved_path;
use crate::types::{Encoding, SaveError};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

/// After this many bytes written to the temp file, abort with an I/O error (tests only).
#[cfg(test)]
static SAVE_FAULT_AFTER_BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
static SAVE_BYTES_WRITTEN: AtomicUsize = AtomicUsize::new(0);

/// Test hook: `None` disables. Resets the per-save byte counter on next [`save_file_sync`].
#[cfg(test)]
pub fn save_fault_set_abort_after_bytes(n: Option<usize>) {
    SAVE_FAULT_AFTER_BYTES.store(n.unwrap_or(0), Ordering::SeqCst);
    SAVE_BYTES_WRITTEN.store(0, Ordering::SeqCst);
}

/// Write `snapshot` to `path` using a temp file in the same directory, then rename.
#[instrument(
    fields(path = %path.display(), snapshot_version = snapshot.version()),
    skip(path, snapshot)
)]
pub fn save_file_sync(
    path: &Path,
    snapshot: &TextBufferSnapshot,
    original_le: LineEnding,
    encoding: Encoding,
) -> Result<(), SaveError> {
    #[cfg(test)]
    SAVE_BYTES_WRITTEN.store(0, Ordering::SeqCst);

    if is_windows_reserved_path(path) {
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            return Err(SaveError::ReservedName(name.to_string()));
        }
    }
    let parent =
        path.parent().filter(|p| !p.as_os_str().is_empty()).ok_or(SaveError::NoParentDir)?;
    let mut tmp = NamedTempFile::new_in(parent)?;
    {
        let mut w = BufWriter::with_capacity(64 * 1024, tmp.as_file_mut());
        write_body(&mut w, snapshot.rope(), original_le, encoding)?;
        w.flush()?;
    }
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| SaveError::Persist(e.error))?;
    Ok(())
}

/// Queue a save on [`WorkerPool`] (same semantics as [`save_file_sync`]).
#[must_use]
#[allow(dead_code)] // wired from the app frame loop in a later milestone
pub fn save_file_async(
    pool: &WorkerPool,
    path: PathBuf,
    snapshot: TextBufferSnapshot,
    original_le: LineEnding,
    encoding: Encoding,
) -> (JobToken, Receiver<Result<(), SaveError>>) {
    pool.spawn(move |_t| save_file_sync(&path, &snapshot, original_le, encoding))
}

fn write_body<W: Write>(
    w: &mut W,
    rope: &Rope,
    le: LineEnding,
    encoding: Encoding,
) -> Result<(), SaveError> {
    match encoding {
        Encoding::Utf8 | Encoding::LossyUtf8 => write_utf8(w, rope, le),
        Encoding::Utf8Bom => {
            write_all_fault_inject(w, &[0xEF, 0xBB, 0xBF])?;
            write_utf8(w, rope, le)
        }
        Encoding::Utf16Le => {
            write_all_fault_inject(w, &[0xFF, 0xFE])?;
            write_utf16(w, rope, le, true)
        }
        Encoding::Utf16Be => {
            write_all_fault_inject(w, &[0xFE, 0xFF])?;
            write_utf16(w, rope, le, false)
        }
    }
}

/// Rope stores LF-only newlines; expand to the on-disk line convention in one pass.
fn lf_to_disk_string(rope: &Rope, le: LineEnding) -> String {
    let s = rope.to_string();
    match le {
        LineEnding::Crlf => s.replace('\n', "\r\n"),
        LineEnding::Cr => s.replace('\n', "\r"),
        LineEnding::Lf | LineEnding::Mixed => s,
    }
}

fn write_utf8<W: Write>(w: &mut W, rope: &Rope, le: LineEnding) -> Result<(), SaveError> {
    let out = lf_to_disk_string(rope, le);
    write_all_fault_inject(w, out.as_bytes())
}

#[cfg(test)]
fn fault_check(len: usize) -> Result<(), SaveError> {
    let max = SAVE_FAULT_AFTER_BYTES.load(Ordering::SeqCst);
    if max == 0 {
        return Ok(());
    }
    let cur = SAVE_BYTES_WRITTEN.fetch_add(len, Ordering::SeqCst) + len;
    if cur > max {
        return Err(SaveError::Io(std::io::Error::other(
            "save fault injection (abort before persist)",
        )));
    }
    Ok(())
}

#[cfg(not(test))]
fn fault_check(_len: usize) -> Result<(), SaveError> {
    Ok(())
}

fn write_all_fault_inject<W: Write>(w: &mut W, buf: &[u8]) -> Result<(), SaveError> {
    fault_check(buf.len())?;
    w.write_all(buf).map_err(SaveError::Io)
}

fn write_utf16<W: Write>(
    w: &mut W,
    rope: &Rope,
    le: LineEnding,
    little_endian: bool,
) -> Result<(), SaveError> {
    let s = lf_to_disk_string(rope, le);
    for u in s.encode_utf16() {
        let b = if little_endian { u.to_le_bytes() } else { u.to_be_bytes() };
        write_all_fault_inject(w, &b)?;
    }
    Ok(())
}

#[cfg(test)]
mod fault_tests {
    use std::fs;

    use editor_core::TextBuffer;

    use super::*;

    #[test]
    fn abort_before_persist_leaves_original_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("t.txt");
        fs::write(&p, b"ORIGINAL").expect("write");
        let buf = TextBuffer::from_str("new content that is long");
        let snap = buf.snapshot();
        save_fault_set_abort_after_bytes(Some(4));
        let r = save_file_sync(&p, &snap, LineEnding::Lf, Encoding::Utf8);
        assert!(r.is_err(), "expected fault injection error");
        save_fault_set_abort_after_bytes(None);
        assert_eq!(fs::read_to_string(&p).expect("read"), "ORIGINAL");
    }
}
