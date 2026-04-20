[← docs/](./) · [README](../README.md)

# File I/O

File I/O lives in `editor-io`. It is the only crate allowed to touch disk.
Real-time subsystems (`editor-core`, `editor-render`, `editor-input`)
communicate with `editor-io` only through bounded channels from
`editor-app`.

## 1. Goals

- **Never block the UI.** All I/O is async on the tokio worker pool.
- **Never lose user data.** Every save is atomic.
- **Scale to large files.** 100 MB+ opens without UI stall; multi-GB is
  acceptable if slower but still non-blocking.
- **Preserve original file shape.** Line endings and encodings survive
  the load / save round-trip unless the user explicitly converts.

## 2. Load Strategy

Two paths, chosen by file size (threshold default 16 MiB, configurable):

### Small / Medium: Streaming Load

```text
1. tokio::fs::File::open(path).await
2. Metadata: length, modified time.
3. Read into a Vec<u8> with ~1 MiB chunks using tokio::io::AsyncReadExt.
4. Detect encoding (UTF-8 vs. UTF-8-BOM in MVP).
5. Detect line endings from the first 64 KiB.
6. Normalize to LF.
7. Send FileLoaded { rope, meta } back to the main thread.
```

The rope is constructed from the final `&str` on the worker thread and
shipped over a bounded channel. Rope construction (via `ropey::Rope::from_str`)
is O(n) but single-threaded; acceptable for files up to ~100 MiB.

### Large: Memory-Mapped Load

```text
1. tokio::task::spawn_blocking for the mmap.
2. memmap2::Mmap::map(&File).
3. Inspect the first 64 KiB for encoding / line-ending detection.
4. Construct a lazy, chunked view that feeds ropey incrementally.
5. Ship a Rope backed by chunks whose underlying memory is the mmap.
```

Memory-mapped files introduce lifetime complexity: the `Mmap` must
outlive the `Rope`. We wrap the pair in an `Arc<dyn Send + Sync>` owned
by `editor-core::Document`. Dropping the document releases the mmap.

**Platform note:** on Windows, `memmap2` uses
`CreateFileMapping`/`MapViewOfFile`; there are no POSIX-specific quirks.
However, reading from a mmap may cause a page fault; we never do this on
the main thread. For rendering, shaping pulls from the rope which is
agnostic to the underlying chunk representation.

## 3. Save Strategy (Atomic)

```text
1. Form the serialized bytes in memory (LF → original line ending).
2. tempfile::NamedTempFile::new_in(target_dir).
3. Write bytes, flush, fsync.
4. persist_noclobber or persist to the final path:
   - POSIX: rename(2) is atomic.
   - Windows: ReplaceFileW is atomic for the replacement; MoveFileExW
     with MOVEFILE_REPLACE_EXISTING is the fallback. tempfile handles
     this internally.
5. fsync the containing directory on POSIX for durability.
```

**Durability on Windows.** There is no direct equivalent to fsync'ing
the containing directory on NTFS; the `FlushFileBuffers` call on the
temp file combined with `ReplaceFileW` is sufficient for power-loss
crash consistency in practice. Documented in the save function's comment.

## 4. Encodings

- **MVP:** UTF-8 and UTF-8-with-BOM only. Any other encoding yields a
  user-visible error: "unsupported encoding". No silent corruption.
- **V2:** still UTF-8 only.
- **Post-V2:** `encoding_rs` for legacy encodings (Windows-1252, Shift_JIS,
  GB18030). Infrastructure is present (crate pinned in `TECH_STACK.md`)
  but no UI surface until later missions.

## 5. Line Ending Detection

- Sample the first 64 KiB.
- Count `\r\n`, `\n`, `\r` (isolated).
- Majority wins. Ties prefer `\n` (safer default).
- Record as `LineEndingKind { LF, CRLF, CR }`.

On save, re-emit the detected kind unless the Document's metadata has
been explicitly converted (not an MVP feature).

## 6. Concurrency Model (Recap)

See `CONCURRENCY.md` §4.

- Load and save both run on the tokio worker pool.
- Results posted back to main via a `crossbeam_channel::bounded(32)`.
- Main thread drains the channel at the start of every frame.

## 7. Error Handling

Every file operation returns an explicit `Result<T, FileError>` with the
`thiserror`-derived enum:

```rust
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("I/O error: {0}")] Io(#[from] std::io::Error),
    #[error("file not found: {0}")] NotFound(std::path::PathBuf),
    #[error("permission denied: {0}")] PermissionDenied(std::path::PathBuf),
    #[error("unsupported encoding for {path}: sniffed bytes {bytes:?}")]
    UnsupportedEncoding { path: std::path::PathBuf, bytes: Vec<u8> },
    #[error("target path exists and overwrite was not requested: {0}")]
    AlreadyExists(std::path::PathBuf),
    #[error("mmap failed for {path}: {source}")]
    Mmap { path: std::path::PathBuf, #[source] source: std::io::Error },
}
```

Errors are shown via the status bar in V2; in MVP they are logged and
the current document is preserved.

## 8. File Paths

- Always `std::path::PathBuf` / `Path`.
- On Windows, handle UNC paths (`\\?\C:\...`) and short names gracefully
  (tempfile + ReplaceFileW handles both).
- On macOS, handle file-system-level case preservation.
- On Linux, treat paths as OS-byte-strings; do not assume UTF-8.

`PathBuf` is not `Send` in all contexts (it is, but this is a common
confusion); clone when crossing threads.

## 9. File Watching (Post-MVP)

Not in scope until post-V2. When added, we will use `notify` for
cross-platform file-system watching. Hooks are defined so future wiring
does not disturb the core.

## 10. Security Considerations

- Refuse to open files larger than a sanity cap (default 2 GiB, user
  overridable).
- Refuse to open obviously binary files (null-byte density > threshold in
  the first 64 KiB) unless the user passes `--binary-ok`.
- Validate paths are not symlinks to devices on POSIX (e.g. `/dev/zero`)
  by statting and checking `FileType`.

## 11. Testing

- **Unit:** line-ending detection on crafted byte sequences.
- **Integration:** write → read round-trip equals original content.
- **Property:** round-trip with random content preserves bytes.
- **Crash injection:** spawn a writer subprocess; `SIGKILL` (or
  `TerminateProcess` on Windows) mid-write; verify the original file is
  intact.
- **Stress (M08):** load a 1 GB file; measure UI frame times during load;
  must stay ≥ 60 fps.

---

*Last updated: M00.*
