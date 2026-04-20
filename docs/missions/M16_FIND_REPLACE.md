# M16 — Find & Replace (In-File and Project-Wide)

**Mission ID:** M16
**Prerequisites:** M15 complete. Syntax highlighting works.
**Output:** Two search surfaces shipped. In-file find/replace (`Ctrl+F` / `Ctrl+H`): a small bar anchored at the top of the editor with regex support, case-sensitivity toggle, whole-word toggle, and next/previous navigation. Project-wide find (`Ctrl+Shift+F`): a panel in the sidebar showing results grouped by file, using `ripgrep`-style fast traversal of the workspace. Both support regex. Both are fast enough to stream results incrementally as the user types.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 2 developer affordances.
- `/docs/WORKSPACE_MODEL.md` — workspace enumeration and ignore handling.
- `/docs/TEXT_ENGINE.md` — `TextBuffer`'s character access.
- `https://docs.rs/regex/` — Rust's canonical regex library.
- `https://docs.rs/grep/` — ripgrep's reusable library crates: `grep-regex`, `grep-searcher`, `grep-printer`.

---

## The Situation In Plain English

Search is table stakes. Every developer hits Ctrl+F dozens of times an hour. M16 builds two complementary search surfaces: one scoped to the active buffer, one scoped to the workspace.

The **in-file find bar** is a small horizontal strip that drops in at the top of the editor when `Ctrl+F` is pressed. It contains a text input, three toggles (regex, case-sensitive, whole-word), counters (`1 of 47`), and three buttons (previous, next, close). While active, `Enter` goes to the next match, `Shift+Enter` goes to the previous. `Ctrl+H` expands it with a second row for replace. All matches in the buffer are highlighted visually. The find bar is non-modal — you can still edit the buffer while it's open, in which case matches re-compute on each edit.

The **project-wide find panel** takes over the sidebar (or opens a new panel — design decision) when `Ctrl+Shift+F` is pressed. Same options (regex, case, word). Hitting Enter kicks off a ripgrep-style parallel search across every non-ignored file in the workspace. Results stream in: `file.rs: 42 matches` appears as soon as the first file with matches completes, and expanding a file shows each matching line with the match highlighted inline. Click a result to jump to that file at that line. Because we use the `grep` crates from ripgrep, the search is *fast* — a 100k-line codebase searches in under a second for typical queries.

A critical design point: both searches operate on *on-disk content* for project-wide (for speed — we don't want to load every file into memory), but on *buffer content* for in-file (because the user might have unsaved changes and expects find to match those). For unsaved modified buffers, project-wide search reads the in-memory content, not disk.

---

## Scope

**In scope:**
- `editor-ui::FindBar` — horizontal bar with input, toggles, navigation.
- `editor-ui::ReplaceBar` — find bar + replace input + replace / replace-all buttons.
- `editor-ui::SearchPanel` — project-wide results panel.
- `editor-search` crate — the search engine, independent of UI.
- In-file search: incremental computation on every query keystroke.
- Match highlighting via the existing `SelectionLayer` (different color; stacked visually with the selection).
- Project-wide search: parallel traversal using `ignore::WalkParallel` + `grep-searcher`.
- Streaming results on a channel; UI polls each frame.
- Regex, case-sensitive, whole-word options.
- Replace and replace-all (with per-match confirmation).
- Cancellation: typing a new character cancels the in-flight search.

**Out of scope:**
- Replace across project (V4+; very easy to misuse, needs UX care).
- Search history (V4+).
- Saved searches (V4+).
- Excluding specific paths / globs per search (V4+; we use `.gitignore` rules for now).
- Incremental indexing for faster search (V4+; ripgrep is already fast enough for sub-second on most projects).
- Symbol-aware search (e.g., "find all functions named X") — V4+; requires LSP or Tree-sitter query work.

---

## North Star

Hit `Ctrl+F`. Type a word. Every occurrence in the current file highlights. Counter shows `3 of 12`. `Enter` cycles forward, `Shift+Enter` back. Hit `Esc`. Back to normal editing.

Hit `Ctrl+Shift+F`. Type a query. The sidebar turns into a search panel. Files with matches stream in. Click one. The file opens at the matching line, centered, with the match highlighted. The whole thing feels like VS Code's search, but snappier.

---

## TODO List

### 1. Create `editor-search` crate

- [ ] 1.1. `cargo new --lib crates/editor-search`. Deps: `regex = "1"`, `grep-regex = "0.1"`, `grep-searcher = "0.1"`, `ignore = "0.4"`, `crossbeam-channel`, `editor-core`, `editor-workspace`.
- [ ] 1.2. Commit: `feat(search): scaffold editor-search crate`.

### 2. In-file search API

- [ ] 2.1. `crates/editor-search/src/in_file.rs`:
  ```rust
  pub struct InFileSearch {
      pub query: String,
      pub is_regex: bool,
      pub case_sensitive: bool,
      pub whole_word: bool,
  }
  pub struct InFileMatch {
      pub byte_range: Range<usize>,
      pub line: usize,
      pub col_start: usize,
  }
  pub fn search_buffer(
      params: &InFileSearch,
      snapshot: &TextBufferSnapshot,
  ) -> Result<Vec<InFileMatch>, SearchError>;
  ```
- [ ] 2.2. Implementation: compile the regex (or literal-escape the query if `!is_regex`). For `whole_word`, wrap in `\b...\b`. For case-insensitive, set the regex flag. Walk the buffer's `chunks` iterator and apply the regex to each chunk, offsetting match positions.
- [ ] 2.3. Cap results at 5000 matches (performance / UI). Return a flag when capped.
- [ ] 2.4. Target: 10k matches in a 10 MB file under 50 ms.
- [ ] 2.5. Unit tests: literal vs regex, case sensitivity, word boundary, empty query (returns no matches), regex with unicode.
- [ ] 2.6. Commit: `feat(search): in-file search with regex + options`.

### 3. In-file replace API

- [ ] 3.1. `pub fn replace_one(buffer: &mut TextBuffer, match_idx: usize, matches: &[InFileMatch], replacement: &str) -> Result<(), SearchError>`.
- [ ] 3.2. Applies one edit through the normal `TextBuffer::apply_edit` so undo works.
- [ ] 3.3. Note: after replacing, all subsequent byte offsets in `matches` are stale. Return the delta so the UI can adjust.
- [ ] 3.4. `pub fn replace_all(buffer: &mut TextBuffer, matches: &[InFileMatch], replacement: &str) -> Result<usize, SearchError>` — applies in reverse order (high byte offsets first) so earlier offsets remain valid. Returns count replaced.
- [ ] 3.5. Commit: `feat(search): in-file replace with correct offset handling`.

### 4. FindBar UI

- [ ] 4.1. `crates/editor-ui/src/find_bar.rs`:
  ```rust
  pub struct FindBar {
      pub visible: bool,
      pub query: String,
      pub cursor: usize,
      pub is_regex: bool,
      pub case_sensitive: bool,
      pub whole_word: bool,
      pub matches: Vec<InFileMatch>,
      pub current_match: Option<usize>,
  }
  ```
- [ ] 4.2. Rendered as a horizontal strip ~32 logical px tall, at the top of the editor area below any tab strip. Dark background quad, input text, small toggle pills, counter, nav buttons.
- [ ] 4.3. Toggle buttons use Unicode char icons: `.*` for regex, `Aa` for case, `ab` (crossed out) for whole-word. Click to toggle; hover highlights.
- [ ] 4.4. Input cursor blinks (reuse M04 logic).
- [ ] 4.5. Commit: `feat(ui): FindBar rendering`.

### 5. FindBar input

- [ ] 5.1. `Ctrl+F` shows find bar with current selection as default query (if selection non-empty). Focus shifts to find bar.
- [ ] 5.2. While visible, typing characters / backspace / arrows edit the query, not the buffer. `Enter` jumps to next match. `Shift+Enter` jumps to previous. `Esc` hides and returns focus to buffer (selection stays on last match so user can type from there).
- [ ] 5.3. On query change: rerun `search_buffer`; update `matches` and `current_match`.
- [ ] 5.4. Jumping to a match: move cursor to the match, ensure visible (reuse auto-scroll from M04).
- [ ] 5.5. Commit: `feat(input): FindBar input and match navigation`.

### 6. Match highlighting in the editor

- [ ] 6.1. Extend `SelectionLayer` from M09 to render *two* layers: user selection (blue) and search matches (amber with lower opacity, drawn beneath selection).
- [ ] 6.2. Active / current match gets a brighter amber highlight.
- [ ] 6.3. `FrameInput` gets a new `search_matches: &[InFileMatch]` field.
- [ ] 6.4. Commit: `feat(render): match-highlight layer stacked below selection`.

### 7. ReplaceBar UI

- [ ] 7.1. `ReplaceBar` extends `FindBar` with a second row: replace input, Replace button, Replace All button. Height grows to ~64 logical px when active.
- [ ] 7.2. `Ctrl+H` toggles between find and find+replace modes.
- [ ] 7.3. `Enter` in the replace input triggers "Replace" (replace current match and move to next); `Ctrl+Enter` triggers Replace All.
- [ ] 7.4. Commit: `feat(ui): ReplaceBar with per-match and replace-all`.

### 8. Project-wide search API

- [ ] 8.1. `crates/editor-search/src/project.rs`:
  ```rust
  pub struct ProjectSearch {
      pub query: String,
      pub is_regex: bool,
      pub case_sensitive: bool,
      pub whole_word: bool,
  }
  pub struct ProjectMatch {
      pub path: PathBuf,
      pub line: usize,
      pub col_start: usize,
      pub byte_range: Range<usize>,     // within the file
      pub line_content: String,         // the full line of the match for display
  }
  pub struct SearchJob {
      pub rx: Receiver<SearchEvent>,
      token: JobToken,
  }
  pub enum SearchEvent {
      FileStarted(PathBuf),
      Match(ProjectMatch),
      FileFinished { path: PathBuf, match_count: usize },
      Done { total_files_searched: usize, total_matches: usize },
      Error(SearchError),
  }
  pub fn start_project_search(
      params: ProjectSearch,
      workspace: &Workspace,
      open_buffers: &BufferManager,
      pool: &WorkerPool,
  ) -> SearchJob;
  ```
- [ ] 8.2. Implementation: use `ignore::WalkParallel` to iterate non-ignored files. For each file, use `grep-searcher::Searcher` with a `grep-regex::RegexMatcher` to find matches. Emit `SearchEvent`s on the channel.
- [ ] 8.3. For any file that corresponds to an open, dirty buffer in `BufferManager`, use the in-memory content rather than re-reading from disk — this keeps unsaved changes visible in search.
- [ ] 8.4. `token.cancel()` stops the search; workers check between files.
- [ ] 8.5. Commit: `feat(search): project-wide search via ripgrep crates`.

### 9. SearchPanel UI

- [ ] 9.1. `crates/editor-ui/src/search_panel.rs`:
  ```rust
  pub struct SearchPanel {
      pub visible: bool,
      pub query: String,
      pub is_regex: bool,
      pub case_sensitive: bool,
      pub whole_word: bool,
      pub current_job: Option<SearchJob>,
      pub results: BTreeMap<PathBuf, Vec<ProjectMatch>>,
      pub expanded_files: HashSet<PathBuf>,
      pub scroll: f32,
      pub highlighted: Option<(PathBuf, usize)>,  // (file, match index)
  }
  ```
- [ ] 9.2. Replaces the sidebar content when active (share the same screen real estate). `Ctrl+Shift+E` switches back to the file tree.
- [ ] 9.3. Top: the same toggles as FindBar (regex, case, word), query input.
- [ ] 9.4. Below: a scrolling list showing each file as a header (collapsible, with match count) and each match as a row showing the line content with the matched span highlighted.
- [ ] 9.5. On each frame, drain `current_job.rx` and insert events into `results`.
- [ ] 9.6. Click a match → `buffers.open_file(path)` + move cursor to match position.
- [ ] 9.7. Click a file header → toggle `expanded_files`.
- [ ] 9.8. Commit: `feat(ui): SearchPanel with streaming results`.

### 10. Search panel input

- [ ] 10.1. `Ctrl+Shift+F` shows the search panel with current selection as default query.
- [ ] 10.2. `Enter` in the query input starts the search (cancel any in-flight).
- [ ] 10.3. Arrow keys navigate between matches in the result list; Enter opens.
- [ ] 10.4. `Esc` returns focus to editor (panel stays visible but unfocused).
- [ ] 10.5. Commit: `feat(input): search panel keybindings`.

### 11. Cancellation + debouncing

- [ ] 11.1. Typing a character while a search is running cancels the old search and starts a new one 200 ms later (debounce). This avoids spamming parallel searches on every keystroke.
- [ ] 11.2. For in-file search: no debounce — in-file is fast enough to re-run on each keystroke even for 10 MB files.
- [ ] 11.3. Commit: `feat(search): debounced project-wide search`.

### 12. Benchmarks

- [ ] 12.1. In-file search on 10 MB file with query hitting 1000 matches: < 50 ms.
- [ ] 12.2. Project-wide search on a 10k-file project for a common query: first result arrives in < 200 ms; full results in < 2 seconds.
- [ ] 12.3. Save baseline as `m16-v3`.
- [ ] 12.4. Commit: `bench(search): in-file and project-wide benchmarks`.

### 13. Quality gates + documentation

- [ ] 13.1. Standard gates.
- [ ] 13.2. Manual: open a real codebase, search for a few things, verify correctness and speed.
- [ ] 13.3. Update `/docs/ARCHITECTURE.md` with `editor-search`.
- [ ] 13.4. Tag: `git tag -a m16-complete -m "M16 complete: find & replace + project-wide search"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. `Ctrl+F` / `Ctrl+H` / `Ctrl+Shift+F` all work with their respective keybindings.
3. Regex, case, whole-word toggles work.
4. In-file match highlighting visible and correct.
5. Project-wide search streams results in under 200 ms TTFR on 10k-file project.
6. Replace and replace-all work, respecting undo.
7. No V2 perf regression.
8. `m16-complete` tag pushed.

## Testing Requirements

- Unit tests on regex / literal / word variants.
- Integration test: project-wide search on a known fixture tree.
- Benchmark: throughput on a large buffer.

## Git Commit Strategy

12-14 commits. Push after items 5, 7, 9, 11, 13.

## Handoff to M17

M17 assumes:
- `editor-search` is stable. M17's diff engine is independent but lives nearby in the codebase organization.
- The SearchPanel pattern (sidebar-replacing panel with streaming results) will be reused for git-status and AI-chat panels in later missions.

---

## Standing Orders Reminder

- Unsaved buffer content must be searchable. Always cross-reference `BufferManager` before hitting disk.
- Cancellation is the most-broken feature in search UIs. Cancel aggressively and always; never let an old search stomp on new results.
- Regex compile errors are user errors; surface them inline in the find bar (red border + tooltip), don't crash.

Go.
