# M13 — Workspace Model & Multi-Buffer Foundation

**Mission ID:** M13
**Prerequisites:** M12 complete. Window behaves natively.
**Output:** A new `editor-workspace` crate that models "a project" (a root directory, its file tree, its ignore rules, a file watcher). A new `BufferManager` that owns multiple open `TextBuffer`s simultaneously, each with independent cursor/scroll/undo state. No UI — this is the headless foundation. M14 will add the visible panels.
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 1 polished substrate.
- `/docs/ARCHITECTURE.md` — how a new crate slots into the workspace.
- `/docs/TEXT_ENGINE.md` — `TextBuffer` / `TextBufferSnapshot` semantics.
- `https://docs.rs/notify/latest/notify/` — cross-platform file watching.
- `https://docs.rs/ignore/latest/ignore/` — the `ignore` crate (powers ripgrep) for gitignore-aware traversal.
- `https://docs.rs/globset/latest/globset/` — for custom pattern matching.

---

## The Situation In Plain English

Up to V2, the editor was a single-file viewer-editor. You could open one file at a time; `Ctrl+O` replaced the current buffer. That's fine for a demo, impossible for real work. M13 turns the editor into a *project*-aware tool. It introduces two concepts that didn't exist before: a `Workspace` (the top-level directory being worked on, plus everything beneath it that isn't ignored) and a `BufferManager` (the set of currently-open files, any subset of which the user — or, later, an agent — can have in memory at once).

These are both *headless* in this mission. No UI. The goal is a clean data model, tested and benchmarked, that M14 will wrap in a tree panel and a tab strip, and that M19-M23 will expose to LLMs through a structured tool API. Getting the model right matters more than any pixel — every subsequent mission depends on it. A bad workspace API now is a week of pain later.

The `Workspace` is the `.git` / `.ide` / `Cargo.toml`-containing folder the user pointed at. It watches files with `notify` (the standard Rust cross-platform file watcher — polling on macOS where native APIs are unreliable, native inotify on Linux, ReadDirectoryChangesW on Windows). It respects `.gitignore` (and `.ignore`, and `.git/info/exclude`) using the `ignore` crate, which is the same library ripgrep uses and is battle-tested on millions of repos. It exposes a lazy, cached tree view with async enumeration so opening a workspace containing 100,000 files doesn't block the UI.

The `BufferManager` holds N `TextBuffer`s, each with its own `BufferId`. Each buffer tracks the same state `EditorState` tracked in V2: cursor, selection, undo stack, scroll offset, dirty flag, absolute path, original line ending. Switching buffers is a cheap operation (just swap the `BufferId` the renderer is pointing at). Closing a buffer with unsaved changes *does* trigger the save-prompt flow — but since we don't have a dialog system yet in V3, we keep the unsaved buffer around and surface it via the banner system from M08.

---

## Scope

**In scope:**
- New `editor-workspace` crate.
- `Workspace` type: project root, watcher, ignore rules, lazy tree enumeration.
- `FileEntry`: path, size, mtime, file type (regular / symlink / directory), is-ignored flag, is-binary heuristic.
- `FileSystemEvent` stream: Created, Modified, Removed, Renamed — de-duplicated and coalesced.
- `BufferManager`: `HashMap<BufferId, BufferState>`, `active_buffer: Option<BufferId>`.
- Each `BufferState` owns: `TextBuffer`, `Cursor`, `Selection`, `UndoStack`, `ScrollOffset`, `Option<PathBuf>`, `LineEnding`, `Encoding`, `dirty: bool`, `external_mtime: SystemTime`.
- `BufferManager::open_file`, `save_file`, `close_buffer`, `switch_to`, `next_buffer`, `prev_buffer`.
- CLI: `editor-app [path]` — if `path` is a directory, open it as a workspace; if a file, open the file and treat its parent as the workspace root.

**Out of scope:**
- UI for any of this (M14).
- Watching for changes to the `.gitignore` file itself and reloading rules (V4+).
- Very-large-project optimizations beyond what `ignore` gives us (V4+).
- Fuzzy matching over the file list (M14's quick-open does that).
- Global (multi-workspace) operations (V4+).

---

## North Star

`editor-app ~/code/my-project` opens. Behind the scenes: `Workspace::new` walks the directory with `ignore::WalkBuilder`, respecting all `.gitignore` files on the way down. A `notify` watcher is installed. The `BufferManager` is empty (no files open yet). The CLI prints a summary to stdout: "Workspace: ~/code/my-project, 1247 files tracked, 892 ignored." No UI. That's M14. But from here, every other mission has a rich, queryable, event-driven project model to build on.

---

## TODO List

### 1. Create the `editor-workspace` crate

- [ ] 1.1. `cargo new --lib crates/editor-workspace`. Add to workspace `Cargo.toml` members.
- [ ] 1.2. Dependencies: `notify` (file watcher), `ignore` (gitignore-aware walker), `globset`, `serde` + `serde_json` (for config), `thiserror`, `anyhow`, `tracing`, `crossbeam-channel`, `editor-core` (for `TextBuffer` types referenced in `BufferManager`), `editor-io` (for loading/saving). Pin versions.
- [ ] 1.3. Commit: `feat(workspace): scaffold editor-workspace crate`.

### 2. Design the `Workspace` type

- [ ] 2.1. `crates/editor-workspace/src/workspace.rs`:
  ```rust
  pub struct Workspace {
      root: PathBuf,
      ignore_config: Arc<IgnoreConfig>,
      watcher: RecommendedWatcher,
      events_rx: Receiver<FileSystemEvent>,
      tree_cache: Arc<RwLock<TreeCache>>,
  }
  impl Workspace {
      pub fn open(root: PathBuf) -> Result<Self, WorkspaceError>;
      pub fn root(&self) -> &Path;
      pub fn iter_entries(&self) -> impl Iterator<Item = FileEntry>;
      pub fn poll_events(&self) -> Vec<FileSystemEvent>;
      pub fn is_ignored(&self, path: &Path) -> bool;
  }
  ```
- [ ] 2.2. `FileEntry`:
  ```rust
  pub struct FileEntry {
      pub path: PathBuf,            // absolute
      pub relative: PathBuf,        // relative to workspace root
      pub file_type: FileType,      // Regular, Directory, Symlink
      pub size_bytes: u64,
      pub mtime: SystemTime,
      pub is_binary_heuristic: bool,
  }
  pub enum FileType { Regular, Directory, Symlink }
  ```
- [ ] 2.3. `is_binary_heuristic`: check the first 8 KB for NUL bytes. Standard trick, ~99% accurate, sub-millisecond per file.
- [ ] 2.4. Commit: `feat(workspace): Workspace and FileEntry types`.

### 3. Implement workspace enumeration

- [ ] 3.1. `Workspace::open` builds an `ignore::WalkBuilder::new(root)`, configures: `.git/info/exclude`, all `.gitignore` files, `.ignore` files. Produces a `WalkParallel` iterator.
- [ ] 3.2. Walk the tree on a worker thread from `WorkerPool` (from M05). Collect entries into `TreeCache`. Emit a completion event on a channel so the UI (eventually) can know enumeration finished.
- [ ] 3.3. For projects > 10,000 files, emit progressive "discovered N files" events every 1000 entries so the UI can show progress.
- [ ] 3.4. Unit tests: open a test fixture directory with mixed ignored/not-ignored files, assert correct filtering.
- [ ] 3.5. Commit: `feat(workspace): gitignore-aware tree enumeration via ignore crate`.

### 4. Set up the file watcher

- [ ] 4.1. Use `notify::recommended_watcher(|res: Result<Event>| ...)` — this picks inotify / FSEvents / ReadDirectoryChangesW automatically.
- [ ] 4.2. Add `notify-debouncer-full` (or equivalent) to coalesce bursts of events — many editors save files via write-temp-then-rename, which produces Create + Remove pairs that should be coalesced into a single Modified event.
- [ ] 4.3. `FileSystemEvent`:
  ```rust
  pub enum FileSystemEvent {
      Created(PathBuf),
      Modified(PathBuf),
      Removed(PathBuf),
      Renamed { from: PathBuf, to: PathBuf },
  }
  ```
- [ ] 4.4. Filter out events on ignored paths (don't spam the UI about `.git/` changes).
- [ ] 4.5. `poll_events` drains the receiver into a Vec and returns it. The UI calls this each frame; the watcher runs on its own thread.
- [ ] 4.6. Unit test: create a tempdir, start a workspace, write a file externally, assert the Created event arrives within 500 ms.
- [ ] 4.7. Commit: `feat(workspace): cross-platform file watcher with debouncing`.

### 5. Design `BufferManager` and `BufferState`

- [ ] 5.1. `crates/editor-workspace/src/buffers.rs`:
  ```rust
  #[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
  pub struct BufferId(u64);

  pub struct BufferState {
      pub buffer: TextBuffer,
      pub cursor: Cursor,
      pub selection: Selection,
      pub undo: UndoStack,
      pub scroll: ScrollOffset,
      pub path: Option<PathBuf>,       // None for untitled buffers
      pub line_ending: LineEnding,
      pub encoding: Encoding,
      pub dirty: bool,
      pub external_mtime: Option<SystemTime>,
      pub ime_preedit: Option<(String, Option<(usize, usize)>)>,
  }

  pub struct BufferManager {
      buffers: HashMap<BufferId, BufferState>,
      order: Vec<BufferId>,           // MRU order for tab strip
      active: Option<BufferId>,
      next_id: u64,
  }

  impl BufferManager {
      pub fn new() -> Self;
      pub fn open_file(&mut self, path: PathBuf, pool: &WorkerPool) -> BufferId;
      pub fn new_untitled(&mut self) -> BufferId;
      pub fn save(&mut self, id: BufferId, path: Option<PathBuf>) -> Result<(), SaveError>;
      pub fn close(&mut self, id: BufferId, force: bool) -> Result<(), CloseError>;
      pub fn switch_to(&mut self, id: BufferId);
      pub fn active(&self) -> Option<BufferId>;
      pub fn get(&self, id: BufferId) -> Option<&BufferState>;
      pub fn get_mut(&mut self, id: BufferId) -> Option<&mut BufferState>;
      pub fn iter(&self) -> impl Iterator<Item = (BufferId, &BufferState)>;
      pub fn find_by_path(&self, path: &Path) -> Option<BufferId>;
  }
  ```
- [ ] 5.2. `next_id` increments monotonically; BufferIds are unique forever within a session.
- [ ] 5.3. `order` tracks most-recently-active; switching bumps to front. This is what the tab strip will use for ordering.
- [ ] 5.4. `close` returns `Err(CloseError::UnsavedChanges)` if `dirty` and `!force`. Caller decides whether to force, prompt, or abort.
- [ ] 5.5. Commit: `feat(workspace): BufferManager and BufferState`.

### 6. Integrate `open_file` with async loading

- [ ] 6.1. `open_file` returns the `BufferId` immediately with an empty `TextBuffer`. It kicks off an async load via `editor-io::load_file_async` on the `WorkerPool`. Polling the load receiver happens in the host's frame loop (to be wired in the `editor-app` integration step).
- [ ] 6.2. If the path is already open (`find_by_path`), return the existing `BufferId` rather than opening a duplicate.
- [ ] 6.3. Load progress events update the `BufferState` in place. When the load completes, the `TextBuffer` is swapped in.
- [ ] 6.4. Commit: `feat(workspace): async file open via BufferManager`.

### 7. Integrate `save` with the line-ending preservation from M06

- [ ] 7.1. `save(id, None)` → use the buffer's existing path; error if untitled.
- [ ] 7.2. `save(id, Some(new_path))` → Save As; update the buffer's path.
- [ ] 7.3. Line-ending and encoding preserved from when the file was loaded.
- [ ] 7.4. Commit: `feat(workspace): save through BufferManager with preserved formatting`.

### 8. Wire `EditorState` in `editor-app` to use `BufferManager`

- [ ] 8.1. The M10 `EditorState` stored a single buffer. Replace it with a reference to the active buffer in `BufferManager`. `App` gains `buffers: BufferManager` and `workspace: Option<Workspace>`.
- [ ] 8.2. Every place that read `state.buffer`, `state.cursor`, etc., now reads `self.buffers.get(self.buffers.active()?)`. Commands that mutate go through `get_mut`.
- [ ] 8.3. `FrameInput` is assembled from the active buffer's state.
- [ ] 8.4. Commands not supported without an active buffer (insert text when no file is open) are no-ops. Handle the `Option<BufferId> == None` case cleanly.
- [ ] 8.5. Commit: `refactor(app): route through BufferManager for all buffer state`.

### 9. CLI: accept a directory as the workspace root

- [ ] 9.1. `editor-app <path>`:
  - If `path` is a directory: `Workspace::open(path)`, no initial buffer.
  - If `path` is a file: `Workspace::open(path.parent()?)`, `buffers.open_file(path)`.
  - If `path` is a dot (`.`): same as current directory.
  - No arg: open no workspace; `buffers.new_untitled()` for a blank buffer.
- [ ] 9.2. Update the window title to reflect workspace + active buffer: `"* main.rs — my-project — IDE"`.
- [ ] 9.3. Commit: `feat(app): CLI accepts workspace directory or file path`.

### 10. Keybindings for buffer switching

- [ ] 10.1. `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle through open buffers in MRU order.
- [ ] 10.2. `Ctrl+W` closes the active buffer (prompts on dirty — for V3, just logs a warning and refuses to close; the dialog version is V4+).
- [ ] 10.3. `Ctrl+N` creates a new untitled buffer.
- [ ] 10.4. Add commands `NextBuffer`, `PrevBuffer`, `CloseBuffer`, `NewBuffer` to `EditorCommand`.
- [ ] 10.5. Commit: `feat(input): buffer-switching keybindings`.

### 11. External modification detection per buffer

- [ ] 11.1. The workspace's `FileSystemEvent` stream is polled each frame. On `Modified(path)`, if any buffer has that path open, compare the on-disk mtime with the buffer's `external_mtime`; if changed, set `external_modified` flag.
- [ ] 11.2. The status bar (M10) already shows this flag for the active buffer.
- [ ] 11.3. On `Removed(path)`, log a warning and leave the buffer with stale content (do not auto-close).
- [ ] 11.4. On `Renamed { from, to }`, if a buffer had `from` as its path, update to `to`.
- [ ] 11.5. Commit: `feat(workspace): react to external file changes per buffer`.

### 12. Benchmarks

- [ ] 12.1. `Workspace::open` on a 10,000-file project: target < 500 ms on reference hardware.
- [ ] 12.2. `Workspace::open` on a 100,000-file project: target < 3 seconds (with progressive events firing so UI can show progress).
- [ ] 12.3. `BufferManager::switch_to`: target < 10 μs (just a HashMap lookup + `active = id`).
- [ ] 12.4. `poll_events` when no events pending: target < 1 μs.
- [ ] 12.5. Save baseline as `m13-v3`.
- [ ] 12.6. Commit: `bench(workspace): open, switch, poll benchmarks`.

### 13. Stress test: many buffers

- [ ] 13.1. Open 100 buffers, verify memory stays bounded (target: < 50 MB of editor RAM for 100 small files).
- [ ] 13.2. Rapid-switch between buffers in a loop; verify no leaks.
- [ ] 13.3. Close all buffers; verify memory returns to baseline.
- [ ] 13.4. Commit: `test(workspace): many-buffers stress test`.

### 14. Documentation & handoff

- [ ] 14.1. New `/docs/WORKSPACE_MODEL.md` documenting the Workspace / BufferManager architecture.
- [ ] 14.2. Update `/docs/ARCHITECTURE.md` with the new crate.
- [ ] 14.3. Update `/docs/STATUS.md`: M13 complete, M14 next.
- [ ] 14.4. Tag: `git tag -a m13-complete -m "M13 complete: workspace + multi-buffer foundation"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. `editor-app ~/some-project` opens the workspace, prints a tree summary, and idles.
3. `Ctrl+N`, `Ctrl+Tab`, `Ctrl+W` work as expected.
4. External file modifications trigger the external-modified flag.
5. Enumerating a 10k-file project completes under 500 ms.
6. All V2 acceptance numbers preserved — the user-visible experience of editing a single file is unchanged.
7. `m13-complete` tag pushed.

## Testing Requirements

- Unit tests on Workspace enumeration, file watcher, BufferManager transitions.
- Stress test for many-buffers.
- Benchmarks captured.

## Git Commit Strategy

12-14 commits. Push after items 3, 6, 8, 10, 14.

## Handoff to M14

M14 assumes:
- `Workspace` and `BufferManager` are stable and well-tested.
- No UI exists yet for either.
- Adding a sidebar, tab strip, and quick-open palette is purely a rendering / input extension on top of the data model.

---

## Standing Orders Reminder

- The workspace crate is headless. Do not add any rendering, input, or UI concerns. Those belong to `editor-ui` or `editor-render`.
- Buffer identity must be stable across operations. Never reuse a BufferId.
- The file watcher runs on its own thread. Never call back into `BufferManager` from it directly — use the channel.
- `ignore` crate semantics match git exactly. Do not roll your own gitignore parser; it will be subtly wrong on edge cases.

Go.
