# M06 — File I/O: Async Load, mmap, Atomic Save

**Mission ID:** M06
**Prerequisites:** M05 complete. Typing works. `EditorCommand::Save` is wired but currently logs a warning. `WorkerPool` is available.
**Output:** A fully functional `editor-io` crate that opens and saves files without blocking the UI. Large files (>50 MB) use memory mapping. Small files use buffered streaming. Saves are atomic (write-temp-then-rename) so a crash mid-save cannot corrupt the user's file. Line endings from the original are preserved on save. Real file paths work on all three OSes, including Unicode paths on Windows.
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/docs/TEXT_ENGINE.md` — the rope's snapshot API and line-ending handling.
- `/docs/CROSS_PLATFORM.md` — Windows long paths, reserved names, case sensitivity, `\\?\` prefix.
- `/reference/03_GAPS_AND_RISKS.md` §6 — file safety and data integrity; this is the spec for *why* atomic saves matter.
- `https://docs.rs/memmap2/latest/memmap2/` — the mmap API.
- `https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist` — atomic persist semantics and their platform caveats.

---

## The Situation In Plain English

File I/O is where bad editors lose user data. Writing the user's buffer directly on top of their existing file means that if the process crashes, the power goes out, or the disk fills up partway through the write, the user ends up with a half-written file — possibly totally empty. That is unacceptable for a tool developers will use on source code. So we do it right: every save writes to a temporary file in the same directory, flushes it fully to disk, then atomically renames it over the original. If anything goes wrong before the rename, the original file is untouched. This pattern is called "write-temp-then-rename" or "atomic persist," and `tempfile::NamedTempFile::persist` implements it with the correct fsync and rename semantics on Unix and Windows.

Reads are similarly careful. For small files (under some threshold, say 10 MB) we read into a `String` with `std::fs::read_to_string` because it's fastest and simplest. For large files we use `memmap2::Mmap` to avoid copying gigabytes into RAM; the OS pages content in on demand. Mmap on Windows has known quirks (the file is locked against deletion while mapped, and permission semantics differ from POSIX), so we guard the mmap path with careful testing and fall back to streaming reads if mmap fails for any reason.

Neither the read nor the write blocks the main thread. Both are submitted to the `WorkerPool` we built in M05. The UI remains interactive while a 500 MB file loads — we can show a progress indicator, cancel the load, and keep the previous buffer visible until the new one is ready. Cancellation is a first-class concern: if the user hits Ctrl+O again midway through a load, the old job is cancelled and its result discarded.

Line endings get preserved. When we load a file we detect its original line ending style (LF, CRLF, CR, or Mixed — using the `LineEnding` type from M02), normalize to LF internally, and remember the original. On save, we convert back. Files authored on Windows with CRLF stay CRLF on disk. This is one of those things that breaks projects (e.g., shell scripts stop working) when an editor gets it wrong, and we will not get it wrong.

---

## Scope

**In scope:**
- `editor-io::FileLoad` — async file load job that returns a `TextBuffer` and metadata (path, original line ending, file mtime, byte size).
- `editor-io::FileSave` — async file save job using atomic write-temp-then-rename.
- Memory-mapped large-file loads with streaming fallback.
- Line-ending detection and preservation.
- Encoding detection: UTF-8 (with BOM) as primary, UTF-16 LE/BE (with BOM) as fallback; everything else is read as UTF-8 with lossy replacement and a warning flag on the load result.
- `EditorCommand::Save` implementation in `editor-app`.
- CLI argument `editor-app <path>` opens the file on startup (replacing the M04/M05 hardcoded fixture).
- `Ctrl+O` command (opens a platform-native file picker — see Decision below).
- Cross-platform path handling, including Windows long paths (`\\?\` prefix automatically applied where needed via `std::path::PathBuf` canonicalization) and Unicode filenames.
- External change detection (mtime check on focus gain — M06 records the feature but full reload-on-external-change UX is V2+).

**Decision: File picker.** For MVP, use the system file picker via `rfd` (Rust File Dialog), a battle-tested cross-platform crate. Alternative is to require path arguments and skip GUI pickers, but Ctrl+O is so universally expected that shipping without it would feel wrong. Add `rfd = "0.14"` (or current) to `editor-app`. The picker is modal and synchronous — acceptable for MVP because loading is the async part.

**Out of scope:**
- File tree / project navigator (post-V2).
- Watching for external file changes (`notify` crate — V2+).
- Saving with a new encoding / line ending (V2+).
- Binary file detection and refusal (post-V2; for MVP, if we can't decode as UTF-8 we warn and open as lossy).
- Multi-tab file management (post-V2).

---

## North Star

At the end of M06:

- `editor-app README.md` opens the README in a window.
- `Ctrl+O` pops up a native file picker. Picking a file loads it, replacing the current buffer.
- `Ctrl+S` saves. The file on disk is byte-identical to what the editor showed (minus the ephemeral IME preedit and cursor).
- Loading a 500 MB file does not hang the UI. The user can move the cursor, scroll, and type into the old buffer while the new one loads.
- A file edited on Windows with CRLF line endings is saved back with CRLF.
- A crash-test: kill the process mid-save (send SIGKILL). The original file is undamaged.

---

## TODO List

### 1. Define the `FileLoad` and `FileSave` types

- [ ] 1.1. `crates/editor-io/src/lib.rs` re-exports `LoadedFile`, `LoadError`, `SaveError`, `LoadProgress`, `save_file`, `load_file`.
- [ ] 1.2. Types:
  ```rust
  pub struct LoadedFile {
      pub buffer: TextBuffer,
      pub path: PathBuf,
      pub line_ending: LineEnding,
      pub encoding: Encoding,        // Utf8, Utf8Bom, Utf16Le, Utf16Be, LossyUtf8
      pub byte_size: u64,
      pub mtime: SystemTime,
      pub was_memory_mapped: bool,
  }

  pub enum LoadProgress {
      Started { total_bytes: u64 },
      Progress { bytes_read: u64 },
      Done(LoadedFile),
      Error(LoadError),
      Cancelled,
  }

  #[derive(thiserror::Error, Debug)]
  pub enum LoadError { /* permission, not-found, invalid-encoding, io */ }

  #[derive(thiserror::Error, Debug)]
  pub enum SaveError { /* permission, disk-full, path-invalid, io */ }
  ```
- [ ] 1.3. `Encoding` enum. For MVP: `Utf8`, `Utf8Bom`, `Utf16Le`, `Utf16Be`, `LossyUtf8`. Use `encoding_rs` for UTF-16 decoding (add as dep).
- [ ] 1.4. Commit: `feat(io): define LoadedFile, LoadProgress, and error types`.

### 2. Implement synchronous `load_file`

- [ ] 2.1. `crates/editor-io/src/load.rs::load_file_sync(path: &Path) -> Result<LoadedFile, LoadError>`. This is the blocking primitive; the async wrapper in step 4 submits it to the WorkerPool.
- [ ] 2.2. Steps:
  1. `std::fs::metadata(path)?` → get file size and mtime. Reject if it's a directory.
  2. If size < `MMAP_THRESHOLD_BYTES` (e.g., 10 MB): `std::fs::read(path)` into a `Vec<u8>`. Otherwise: `File::open`, `unsafe { Mmap::map(&file) }`. If mmap fails with `ErrorKind::Unsupported` or any other error, fall back to streaming the file into a pre-sized `Vec<u8>` with `BufReader`.
  3. Detect BOM at offset 0: `EF BB BF` → UTF-8 BOM; `FF FE` → UTF-16 LE; `FE FF` → UTF-16 BE.
  4. Decode:
     - UTF-8 (no BOM or with BOM): `std::str::from_utf8` on the bytes (skipping the BOM). If that fails, fall back to lossy: `String::from_utf8_lossy` and tag as `LossyUtf8`. Log a warning.
     - UTF-16 LE/BE: use `encoding_rs::UTF_16LE.decode` / `UTF_16BE.decode` and tag accordingly.
  5. Detect line ending via `LineEnding::detect` from M02.
  6. Normalize to LF internally (replace CRLF and CR with LF) using `LineEnding::normalize_to(LineEnding::Lf, ...)`.
  7. Build `TextBuffer::with_line_ending(normalized, detected_le)` (or a variant that takes the *original* line ending as metadata but stores only LF-normalized content).
  8. Return `LoadedFile`.
- [ ] 2.3. Rebuild the `TextBuffer` constructor to match: `TextBuffer::with_original_line_ending(content: &str, original: LineEnding)` stores content (assumed already LF-normalized) and remembers the original for save-time conversion. Update M02's exported API accordingly; bump internal versions.
- [ ] 2.4. Unit tests: small file, large file (fixture in `testdata/`; ensure it's gitignored or under 1 MB if committed), file with BOM, UTF-16 file, file with CRLF, empty file, file with just a BOM, nonexistent file (returns NotFound error), directory path (returns error).
- [ ] 2.5. Commit: `feat(io): implement load_file_sync with mmap and encoding detection`.

### 3. Implement synchronous `save_file`

- [ ] 3.1. `crates/editor-io/src/save.rs::save_file_sync(path: &Path, buffer: &TextBufferSnapshot, original_le: LineEnding) -> Result<(), SaveError>`.
- [ ] 3.2. Steps:
  1. Get the parent directory of `path`. If `path` has no parent (root or relative with no prefix), error — we need a parent to place the temp file.
  2. Create a `NamedTempFile` in the parent directory via `tempfile::NamedTempFile::new_in(parent)`.
  3. Wrap in `BufWriter` with a 64 KB buffer.
  4. Iterate the `TextBufferSnapshot`'s chunks. For each chunk:
     - Write the chunk bytes.
     - If `original_le == Crlf`, pre-transform: replace `\n` with `\r\n` as we write. (The LF-to-CRLF conversion happens in the writer layer, not the rope, so the rope stays clean.)
     - If `original_le == Cr`, similarly replace `\n` with `\r`.
     - If `original_le == Lf` or `Mixed`, write bytes directly.
  5. Flush the BufWriter. Call `sync_all()` on the underlying file. This fsyncs the content to disk before we rename.
  6. Call `temp.persist(path)?`. This atomically renames `temp` → `path`, replacing the original.
  7. On success, update the `mtime` we remember for `LoadedFile` (optional; or just leave it to the next focus-gain check).
- [ ] 3.3. Windows-specific nuance: `NamedTempFile::persist` documents that atomicity is generally reliable on modern Windows (NTFS) but not guaranteed on all filesystems. Document this in `/docs/CROSS_PLATFORM.md#atomic-writes`.
- [ ] 3.4. Crash-safety test: a unit test that writes a file, then repeatedly saves with increasing content, then kills the saving process at a random point via a fault-injection hook, and verifies the file is always either the old content or the new content — never truncated. Use `#[cfg(test)]` injected points in `save_file_sync` that check an atomic and return early.
- [ ] 3.5. Permission errors (file is read-only, parent directory not writable, disk full) map to `SaveError` variants with clear messages.
- [ ] 3.6. Unit tests: round-trip save+load preserves content byte-for-byte (modulo line-ending conversion); save over existing file replaces content; save to non-writable path errors cleanly; save with each `LineEnding` variant produces correct bytes.
- [ ] 3.7. Commit: `feat(io): implement save_file_sync with atomic persist and line-ending preservation`.

### 4. Implement async wrappers using `WorkerPool`

- [ ] 4.1. `load_file_async(pool: &WorkerPool, path: PathBuf) -> (JobToken, Receiver<LoadProgress>)`.
- [ ] 4.2. The job function inside the worker:
  1. Emit `LoadProgress::Started { total_bytes: metadata.len() }`.
  2. Periodically check `token.is_cancelled()` — if so, emit `Cancelled` and return.
  3. For mmap path, just call `load_file_sync` (no real progress granularity since the OS pages in on demand). For streaming path, emit `Progress { bytes_read }` every ~1 MB of bytes read.
  4. Emit `Done(LoadedFile)` or `Error(...)`.
- [ ] 4.3. `save_file_async(pool: &WorkerPool, path: PathBuf, snapshot: TextBufferSnapshot, original_le: LineEnding) -> (JobToken, Receiver<Result<(), SaveError>>)`.
- [ ] 4.4. `Receiver::try_recv` polling integrates with the frame loop — each frame, we `try_recv` on any outstanding jobs and react accordingly.
- [ ] 4.5. Unit tests covering: concurrent loads don't interfere; cancellation stops a load mid-read; cancellation of a save is accepted but the save completes anyway (save cancellation mid-persist is dangerous; either the rename has happened or it hasn't, and we don't want to leave the temp file lingering — prefer: saves cannot be cancelled once they've reached the flush step).
- [ ] 4.6. Commit: `feat(io): implement async load/save wrappers over WorkerPool`.

### 5. Integrate file loading into `editor-app`

- [ ] 5.1. CLI arg: `editor-app <path>` (simple `env::args().nth(1)`). If the path is given, kick off an async load during `resumed`; in the meantime, show an empty buffer and a subtle "Loading..." indicator (status bar comes in M09; for now, just render the word "Loading..." via `TextLayer` at the top-left).
- [ ] 5.2. On each frame, poll the load receiver. When `Done(loaded)` arrives, replace `state.buffer` with `loaded.buffer`, remember `loaded.path`, `loaded.line_ending`, `loaded.mtime`. Trigger a redraw.
- [ ] 5.3. `Ctrl+O`: pops up `rfd::FileDialog::new().pick_file()`. On result, kick off a load, replacing the current buffer on completion. Warn in the log if the user had unsaved changes (full "do you want to save?" modal is V2+).
- [ ] 5.4. `Ctrl+S`: if we have a path, kick off a save with the current buffer's snapshot and the remembered `line_ending`. If we don't have a path, pop up a `rfd::FileDialog::new().save_file()` for Save As.
- [ ] 5.5. Track `state.dirty: bool` — set to true on every `apply_edit`, cleared when a save completes successfully.
- [ ] 5.6. Commit: `feat(app): wire async load/save into EditorState with CLI arg, Ctrl+O, Ctrl+S`.

### 6. Handle Windows-specific path corners

- [ ] 6.1. Long paths: our `Cargo.toml` and Windows manifest should already opt into long-path support from M01. Verify by saving a file at a path > 260 chars on Windows.
- [ ] 6.2. Reserved names (`CON`, `NUL`, etc.): if the user tries to save to one, return a `SaveError::ReservedName(..)` with a clear message. Check the final path component's stem (case-insensitive) against the list.
- [ ] 6.3. UNC paths (`\\server\share\file.txt`): should just work via `std::path`. Confirm with a test (if a share is available; otherwise document as untested).
- [ ] 6.4. Unicode filenames: `std::path::Path` uses `OsString` internally which preserves any OS-valid byte sequence. `PathBuf::from("中文.txt")` round-trips correctly on all three OSes. Add a test.
- [ ] 6.5. Commit: `fix(io): handle Windows reserved names, long paths, Unicode filenames`.

### 7. External change detection (foundation only)

- [ ] 7.1. On focus-gain (winit's `WindowEvent::Focused(true)`), call `std::fs::metadata(&path)?.modified()?` and compare to the remembered `mtime`. If it changed, set `state.external_modified = true` and log a warning. Full UX for reloading (prompt, diff, etc.) is V2+.
- [ ] 7.2. Render a small indicator (e.g., a "⚠" unicode char near the top-right) when `external_modified`. Implement as a simple text draw via `TextLayer` at a fixed position. Real status bar comes in M09/M10.
- [ ] 7.3. Commit: `feat(app): detect external file modifications on focus gain`.

### 8. Benchmarks

- [ ] 8.1. `crates/editor-io/benches/file_io.rs`: benchmark `load_file_sync` on small (100 KB), medium (10 MB), and large (100 MB) files. Generate the fixtures on the fly to avoid committing them.
- [ ] 8.2. Benchmark the save path for the same sizes.
- [ ] 8.3. Benchmark CRLF↔LF conversion throughput during save.
- [ ] 8.4. Save baseline as `m06-mvp`.
- [ ] 8.5. Commit: `bench(io): file load and save throughput`.

### 9. Stress testing

- [ ] 9.1. `crates/editor-io/tests/stress_large_file.rs` (ignored by default): generate a 500 MB file of Lorem Ipsum, load it, verify byte count, make some edits, save, reload, verify changes persisted, delete the test file. Document how to run: `cargo test -p editor-io --test stress_large_file -- --ignored`.
- [ ] 9.2. `crates/editor-io/tests/save_atomicity.rs`: use the fault-injection hook from step 3 to prove that a crash mid-save leaves the original file untouched.
- [ ] 9.3. Commit: `test(io): add stress and atomicity tests (ignored by default)`.

### 10. Cross-platform verification

- [ ] 10.1. Windows: open `README.md`, edit, save, confirm CRLF preserved. Open a UTF-16 LE file (Windows Notepad default for "Unicode"), edit, save, confirm UTF-16 LE preserved.
- [ ] 10.2. macOS: open a file with an HFS+ decomposed-form Unicode filename (e.g., `café` as `cafe\u{0301}`) if you can create one. Confirm round-trip.
- [ ] 10.3. Linux: test with a file on an NFS-mounted directory if possible (atomic rename semantics differ on NFS); document any caveats.
- [ ] 10.4. Commit: `fix(io): cross-platform corrections from verification pass`.

### 11. Quality gates

- [ ] 11.1. `cargo fmt --all --check`.
- [ ] 11.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 11.3. `cargo test --workspace` (ignored stress tests can be skipped in CI; run once locally before tagging).
- [ ] 11.4. `cargo bench -p editor-io --no-run`.
- [ ] 11.5. Manual: open a 100 MB file, confirm UI stays interactive, edit, save, reopen, verify.

### 12. Documentation

- [ ] 12.1. Update `/docs/TEXT_ENGINE.md`: line-ending preservation contract with `editor-io`.
- [ ] 12.2. Update `/docs/ARCHITECTURE.md`: the async I/O flow through `WorkerPool`.
- [ ] 12.3. Update `/docs/CROSS_PLATFORM.md`: Windows-specific paths, reserved names, NFS atomicity caveat.
- [ ] 12.4. Update `/docs/STATUS.md`: M06 complete, M07 next.
- [ ] 12.5. Update `/CHANGELOG.md`.
- [ ] 12.6. Tag: `git tag -a m06-complete -m "M06 complete: file I/O with atomic saves and mmap loads"`; push.

---

## Validation / Acceptance Criteria

M06 is complete when:

1. Quality gates pass.
2. CI green on all three OSes.
3. `editor-app README.md` opens the file correctly.
4. `Ctrl+O` pops up the native picker; picking a file loads it.
5. `Ctrl+S` saves; the on-disk file is byte-correct.
6. CRLF files stay CRLF on save; LF stays LF.
7. UTF-8 BOM is detected and preserved.
8. UTF-16 LE/BE files are detected and round-trip as UTF-16.
9. Loading a 100 MB file does not freeze the UI (manual verification on Windows).
10. The crash-injection test proves atomic-save safety.
11. Unicode filenames work on Windows.
12. Benchmarks baseline saved as `m06-mvp`.
13. `m06-complete` tag pushed.

## Testing Requirements

- Unit tests on every public function in `editor-io`.
- Round-trip tests: save + load = original content (modulo line-ending conversion).
- Stress test on 500 MB file (ignored by default).
- Atomicity test with fault injection.
- Cross-platform manual verification.

## Git Commit Strategy

12-16 commits. Push after items 2, 3, 4, 5, 7, 9, 12.

## Handoff to M07

M07 assumes:

- `editor-io` is stable and has benchmarks.
- Load/save work and are instrumented with `tracing` spans.
- The editor is *functionally* complete for the MVP spec; M07 adds observability so we can *prove* performance targets.

---

## Standing Orders Reminder

- Atomicity is not a nice-to-have. Do not accept a save path that writes directly to the target file, even as a fast path for small files. Always write-temp-then-rename.
- `unsafe { Mmap::map(...) }` is the only `unsafe` block allowed in `editor-io`, and it must carry a `// SAFETY:` comment citing memmap2's docs on the preconditions (the file must not be externally modified while mapped).
- Never panic on a save failure. The user's keystrokes got them to this point; a panic loses their work. Surface the error to the UI layer (which will display it in a future mission) and keep the buffer dirty.
- Line-ending mixing: we detect `Mixed` on load and default to LF on save, but log the detection. User-facing choice to normalize-or-preserve comes in V2+.

Go.
