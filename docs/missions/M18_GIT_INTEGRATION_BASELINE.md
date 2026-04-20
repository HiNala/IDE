# M18 — Git Integration Baseline

**Mission ID:** M18
**Prerequisites:** M17 complete. Diff engine works.
**Output:** The editor becomes git-aware. File tree shows modified / untracked / ignored status with color-coded indicators. Status bar shows the current branch. Any file can be diffed against HEAD using M17's inline diff renderer (`Ctrl+Shift+D` on an open file). No write operations — V3 cannot commit, push, pull, stage, or stash. Read-only git awareness, nothing more.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 2 developer affordances.
- `/docs/WORKSPACE_MODEL.md` — the workspace from M13.
- `https://docs.rs/gix/` — `gix` is the pure-Rust git implementation; preferred in 2026.
- `https://docs.rs/git2/` — `git2` is the libgit2 binding; fallback if a specific feature isn't in gix yet.

---

## The Situation In Plain English

Developers expect their editor to know what git knows. When a file is modified since last commit, the sidebar should say so at a glance. When we're on a feature branch, the status bar should show it. When we want to see "what did I change?", one keypress should show the diff against HEAD. These are small, cheap affordances that make an editor feel serious.

What we're *not* doing: staging hunks, committing, branch switching, history visualization, merge conflict resolution, remote operations. Those are individually large features — each a mission of its own in V4+ — and rushing them now would either produce broken stubs or force design decisions we shouldn't make yet. V3 gives us read-only awareness. If the user wants to commit, they Alt-Tab to their terminal like always.

The preferred library is `gix` (also called `gitoxide`): a pure-Rust reimplementation of git that has matured substantially by 2026, is about 10x faster than libgit2 for many operations, and avoids the C dependency `git2` drags in. For operations where `gix` doesn't yet have the right API, we fall back to shelling out to `git` (the CLI) — boring but reliable. `git2` is our secondary option if both fail.

---

## Scope

**In scope:**
- New `editor-git` crate.
- Repository detection on workspace open (walk up from root looking for `.git`).
- File status: modified / untracked / ignored / clean, cached, refreshed on file watcher events.
- Current branch name, refreshed on `.git/HEAD` changes.
- Diff-vs-HEAD rendering using M17's `DiffOverlay` in inline mode.
- File tree color coding: modified = amber, untracked = green, ignored = subdued grey.
- Status bar branch indicator: `main · 3 modified` style.
- `Ctrl+Shift+D` opens diff-vs-HEAD on the active buffer.

**Out of scope:**
- Staging, committing, pushing, pulling, fetching, merging, rebasing — anything that writes to the repo.
- Branch switcher UI.
- History / log view.
- Blame / annotate.
- Merge conflict markers / resolution tools.
- Remote operations of any kind.
- Submodules (V4+; they add a lot of complexity).
- LFS awareness (V4+).

---

## North Star

Open a project that's a git repo. Status bar shows `main · 2 modified`. File tree shows one file in amber (modified), one file in green (untracked). Hit `Ctrl+Shift+D` on a modified file — the editor swaps into diff mode showing changes vs HEAD in red and green. Hit `Esc` to exit. Edit the file more, save, then `Ctrl+Shift+D` again — fresh diff reflecting the latest disk state.

---

## TODO List

### 1. Create `editor-git` crate

- [ ] 1.1. `cargo new --lib crates/editor-git`. Deps: `gix = "0.70"` (or latest stable; use `features` to include only needed components to keep build time down), `editor-core`, `editor-workspace`, `editor-diff`, `crossbeam-channel`.
- [ ] 1.2. Consider a fallback feature flag `gix-fallback-git-cli` that shells out to `git` for anything unsupported. For V3, we lean on gix primarily; CLI fallback is reserved for edge cases.
- [ ] 1.3. Commit: `feat(git): scaffold editor-git crate with gix`.

### 2. Repository detection

- [ ] 2.1. `Repository::discover(root: &Path) -> Option<Repository>` — walks up from `root` looking for a `.git` directory (or `.git` file in the case of submodules / worktrees). Returns `None` if not inside a git repo.
- [ ] 2.2. `Repository` holds: the open `gix::Repository`, the worktree path, a `RwLock<FileStatusCache>`, the current branch.
- [ ] 2.3. Workspaces without git still work — every git feature gracefully no-ops when `Repository::discover` returned None.
- [ ] 2.4. Commit: `feat(git): repository discovery`.

### 3. File status cache

- [ ] 3.1. `FileStatusCache`:
  ```rust
  pub struct FileStatusCache {
      map: HashMap<PathBuf, FileStatus>,
      last_refreshed: Instant,
  }
  pub enum FileStatus {
      Clean,
      Modified,
      Untracked,
      Ignored,
      Added,      // staged but we report as if tracked
      Deleted,
  }
  ```
- [ ] 3.2. Initial status computation: gix provides `Repository::status()` (modern API) which enumerates worktree vs HEAD vs index differences. Run on workspace open, on a worker thread for large repos.
- [ ] 3.3. For 10k-file repos: target under 500 ms for initial status.
- [ ] 3.4. Commit: `feat(git): compute file status via gix::Repository::status`.

### 4. Incremental refresh

- [ ] 4.1. When the workspace file watcher (M13) fires a `Modified(path)` or `Created(path)`, re-query the status for that single path via gix — much cheaper than full repo rescan.
- [ ] 4.2. When `.git/HEAD` changes (user ran `git checkout` externally), refetch branch name and trigger full status recompute.
- [ ] 4.3. When `.git/index` changes (user ran `git add`), full status recompute.
- [ ] 4.4. Coalesce bursts: if 50 files change in a directory within 1 second, do one batch recompute instead of 50 individual ones.
- [ ] 4.5. Commit: `feat(git): incremental status refresh on file watcher events`.

### 5. Branch detection

- [ ] 5.1. `Repository::current_branch() -> Option<String>`: read `HEAD`, follow symbolic ref to get branch name, or return `(detached HEAD at abc1234)` if detached.
- [ ] 5.2. Cache; refresh on `.git/HEAD` change.
- [ ] 5.3. Commit: `feat(git): current branch detection`.

### 6. Sidebar integration

- [ ] 6.1. Extend the `Sidebar::FlatEntry` from M14 with `git_status: Option<FileStatus>`.
- [ ] 6.2. When rendering a row: if status is Modified, render text in amber (~#F5A623); Untracked in green (~#7EC699); Ignored in subdued grey (~#5C5C5C, maybe only visible when user toggles show-ignored, to be added in V4+).
- [ ] 6.3. Commit: `feat(ui): sidebar shows git status colors`.

### 7. Status bar branch display

- [ ] 7.1. Extend `StatusBarInfo` from M10 with `git_branch: Option<String>`, `git_modified_count: usize`.
- [ ] 7.2. Left side of status bar (after file info) shows `· main · 3 modified`. Clicking (V4+) could open a branch picker; V3 has no action, just display.
- [ ] 7.3. Commit: `feat(ui): status bar branch and modified count`.

### 8. Diff-vs-HEAD

- [ ] 8.1. `Repository::blob_at_head(path: &Path) -> Option<Vec<u8>>` — returns the file contents as they exist at HEAD (the latest committed version on the current branch). Uses gix's tree lookup.
- [ ] 8.2. For a file not yet in HEAD (new file), returns empty — the resulting diff is "everything is added."
- [ ] 8.3. `pub fn diff_vs_head(repo: &Repository, path: &Path, working_content: &str) -> Result<Vec<Hunk>, DiffError>`:
  - Fetch HEAD blob.
  - Pass `(head_str, working_content)` through `editor-diff::compute_line_diff`.
  - Return hunks.
- [ ] 8.4. Commit: `feat(git): diff-vs-HEAD via editor-diff`.

### 9. `Ctrl+Shift+D` keybinding

- [ ] 9.1. Command `DiffActiveBufferVsHead` bound to `Ctrl+Shift+D`. Triggers:
  1. Fetch current buffer's working content.
  2. Call `diff_vs_head`.
  3. Construct a `DiffOverlay` from the hunks.
  4. Put the active `BufferState` into diff mode (read-only + overlay).
- [ ] 9.2. `Esc` while in diff mode exits back to normal editing.
- [ ] 9.3. Commit: `feat(input): Ctrl+Shift+D diff vs HEAD`.

### 10. Handle repo edge cases

- [ ] 10.1. Bare repos: `discover` returns None (we require a worktree).
- [ ] 10.2. Worktrees (`git worktree add`): supported as long as gix sees them as normal repos.
- [ ] 10.3. Submodules: detected as separate repos, but V3 treats submodule contents as ignored from the parent workspace's perspective.
- [ ] 10.4. Very large repos (> 100k files tracked): status computation can be slow. Cap the initial computation at 2 seconds; if exceeded, fall back to on-demand per-file status and emit a log warning.
- [ ] 10.5. Commit: `feat(git): repo edge case handling`.

### 11. Benchmarks

- [ ] 11.1. Initial status on 10k-file repo: < 500 ms.
- [ ] 11.2. Per-file status check on watcher event: < 10 ms.
- [ ] 11.3. Diff-vs-HEAD on a 10 KB file: < 20 ms.
- [ ] 11.4. Save baseline as `m18-v3`.
- [ ] 11.5. Commit: `bench(git): status + diff benchmarks`.

### 12. Quality gates + documentation

- [ ] 12.1. Standard gates.
- [ ] 12.2. Manual test: open the IDE project itself, modify some files, verify colors + status bar + diff.
- [ ] 12.3. Update `/docs/ARCHITECTURE.md`.
- [ ] 12.4. Tag: `git tag -a m18-complete -m "M18 complete: git status, branch, diff-vs-HEAD"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. Opening a git repo populates file statuses within 500 ms on a 10k-file repo.
3. Sidebar colors reflect status correctly.
4. Status bar shows branch.
5. `Ctrl+Shift+D` diffs against HEAD using M17's renderer.
6. External `git checkout` refreshes the branch display within a couple seconds.
7. `m18-complete` tag pushed.

## Testing Requirements

- Unit tests on status enum mapping.
- Integration test: fixture repo, modify file, assert correct status.
- Benchmark.

## Git Commit Strategy

10-12 commits. Push after items 4, 7, 9, 12.

## Handoff to M19

M19 is the first AI-facing mission. After M18, the developer-facing half of V3 is substantially complete (window, workspace, UI, syntax, search, diff, git). M19 starts the agent substrate.

---

## Standing Orders Reminder

- No write operations to the git repo. Period. If a user wants to commit, they use the terminal.
- `.git/index` and `.git/HEAD` are watched via `notify`. Don't poll.
- `gix` is under active development; pin an exact version and re-test on bumps.
- When in doubt about whether a feature belongs in M18, the answer is no — defer to V4+.

Go.
