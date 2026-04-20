//! Atomic save (temp file + rename) with encoding and line-ending preservation.

use std::io::{BufWriter, Write};
use std::path::Path;

use editor_core::{LineEnding, TextBufferSnapshot};
use ropey::Rope;
use tempfile::NamedTempFile;
use tracing::instrument;

use crate::paths::is_windows_reserved_path;
use crate::types::{Encoding, SaveError};

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

fn write_body<W: Write>(
    w: &mut W,
    rope: &Rope,
    le: LineEnding,
    encoding: Encoding,
) -> Result<(), SaveError> {
    match encoding {
        Encoding::Utf8 | Encoding::LossyUtf8 => write_utf8(w, rope, le),
        Encoding::Utf8Bom => {
            w.write_all(&[0xEF, 0xBB, 0xBF])?;
            write_utf8(w, rope, le)
        }
        Encoding::Utf16Le => {
            w.write_all(&[0xFF, 0xFE])?;
            write_utf16(w, rope, le, true)
        }
        Encoding::Utf16Be => {
            w.write_all(&[0xFE, 0xFF])?;
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
    w.write_all(out.as_bytes()).map_err(SaveError::Io)
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
        w.write_all(&b).map_err(SaveError::Io)?;
    }
    Ok(())
}
