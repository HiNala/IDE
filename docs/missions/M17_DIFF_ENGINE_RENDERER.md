# M17 — Diff Engine & Inline Renderer

**Mission ID:** M17
**Prerequisites:** M16 complete. Search works.
**Output:** A `editor-diff` crate that computes line-based and character-based diffs between two pieces of text. An inline diff renderer that displays added lines in green, removed lines in red, and changed segments with intra-line highlighting. A unified diff view (left-side old, right-side new). The renderer is general-purpose: M18 uses it for git diffs, M23 uses it for AI edit previews. This mission builds the engine once and uses it everywhere.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 2; this is the foundation for agent-edit review in Ring 4.
- `/docs/TEXT_ENGINE.md` — chunk access.
- `https://docs.rs/similar/latest/similar/` — the `similar` crate, Rust's canonical diff library.
- `https://docs.rs/imara-diff/latest/imara_diff/` — alternative, faster on large inputs.

---

## The Situation In Plain English

Diff rendering is the load-bearing primitive for two later missions: M18 (git integration needs diff-vs-HEAD) and M23 (AI edit review needs "here's what the agent proposes"). Building the diff engine now, as a clean reusable library, means both of those missions become much cheaper.

We choose `similar` over `imara-diff` because `similar` ships both line-level and character-level (Myers algorithm) diffing with the same API, and its performance is sufficient for the file sizes we care about. `imara-diff` is 2-3x faster for large files but line-only; if an agent edit proposes a 100 KB diff we still need it to render fast, but that's an optimization we can make later by swapping backends if needed.

The rendering side has two modes. **Inline mode** renders a diff *into* the regular editor view — removed lines shown with red background, added lines with green, unchanged lines rendered normally. This is what a reviewer sees when they're asked to approve or reject an AI's proposed edits. **Unified view mode** opens a two-column (old left, new right) view in a separate panel or replacing the editor temporarily; this is what users see when they ask to compare two files or view `git diff`.

For V3 we ship both modes but expect inline to be the primary surface because the AI flow in M23 is the hot path. Unified view is shipped for completeness.

---

## Scope

**In scope:**
- `editor-diff` crate wrapping `similar`.
- `Hunk`, `LineOp` (Equal, Insert, Delete, Replace), `CharOp` for intra-line diffs.
- `compute_line_diff(old: &str, new: &str) -> Vec<Hunk>`.
- `compute_intra_line_diff(old_line: &str, new_line: &str) -> Vec<CharOp>` for Replace hunks.
- Inline renderer that layers diff markup onto an existing editor view.
- Unified diff panel for file-vs-file or file-vs-revision comparison.
- Keyboard navigation between hunks (`Ctrl+Alt+Down` / `Up`).
- Per-hunk accept / reject API (to be consumed by M23).

**Out of scope:**
- Three-way merge (V4+).
- Word-level intra-line diffing (we do character-level; word-level is a V4+ polish).
- Diff minimap (V4+).
- Blame / annotate (V4+; git-specific, part of a deeper git integration than M18).

---

## North Star

Two files. `compute_line_diff` returns a list of hunks. The inline renderer shows the diff: removed lines in red, added lines in green, modified lines with the differing characters highlighted. `Ctrl+Alt+Down` jumps to the next hunk. The same engine powers git-diff-vs-HEAD (M18) and AI-edit review (M23).

---

## TODO List

### 1. Create `editor-diff` crate

- [ ] 1.1. `cargo new --lib crates/editor-diff`. Deps: `similar = "2"`, `editor-core`.
- [ ] 1.2. Commit: `feat(diff): scaffold editor-diff crate`.

### 2. Define diff types

- [ ] 2.1. `crates/editor-diff/src/types.rs`:
  ```rust
  pub enum LineOp {
      Equal { old_range: Range<usize>, new_range: Range<usize> },
      Insert { new_range: Range<usize> },
      Delete { old_range: Range<usize> },
      Replace {
          old_range: Range<usize>,
          new_range: Range<usize>,
          intra_line: Vec<IntraLineDiff>,
      },
  }
  pub struct IntraLineDiff {
      pub old_line_idx: usize,     // relative to the Replace hunk's old_range
      pub new_line_idx: usize,     // relative to the Replace hunk's new_range
      pub char_ops: Vec<CharOp>,
  }
  pub enum CharOp {
      Equal(Range<usize>),
      Insert(Range<usize>),
      Delete(Range<usize>),
  }
  pub struct Hunk {
      pub header: HunkHeader,      // old/new start + count
      pub ops: Vec<LineOp>,
  }
  ```
- [ ] 2.2. Ranges are in line indices (for LineOp) or character indices within a line (for CharOp).
- [ ] 2.3. Commit: `feat(diff): diff type definitions`.

### 3. Line-level diff

- [ ] 3.1. `pub fn compute_line_diff(old: &str, new: &str) -> Vec<Hunk>`:
  - Use `similar::TextDiff::from_lines(old, new)` which returns a `TextDiff`.
  - Iterate `text_diff.iter_all_changes()` and group consecutive changes into hunks (grouped if within 3 lines of each other — the standard "context radius").
- [ ] 3.2. For `Replace` hunks (Delete + Insert pairs), run intra-line diffing on each old/new line pair.
- [ ] 3.3. Target: 1 MB old vs 1 MB new diff in under 100 ms.
- [ ] 3.4. Unit tests: identical text (single Equal hunk), fully replaced (single Replace hunk), interleaved, empty old, empty new, single-char changes, newline-only changes.
- [ ] 3.5. Commit: `feat(diff): line-level diff via similar`.

### 4. Intra-line diff

- [ ] 4.1. `pub fn compute_intra_line_diff(old_line: &str, new_line: &str) -> Vec<CharOp>`:
  - `similar::TextDiff::from_chars(old_line, new_line)`.
  - Group consecutive equal / insert / delete chars.
- [ ] 4.2. Fall back to whole-line diff if the similarity score is too low (< 30%) — at that point character-level comparison is noise.
- [ ] 4.3. Commit: `feat(diff): intra-line character diff with similarity fallback`.

### 5. Inline rendering: extend `editor-render`

- [ ] 5.1. `editor-render::DiffOverlay` layer:
  ```rust
  pub struct DiffOverlay {
      pub mode: DiffMode,   // None | Inline { hunks } | Unified { old_hunks, new_hunks }
  }
  ```
- [ ] 5.2. Rendered as quads beneath the text (same pipeline as `SelectionLayer` from M09):
  - For each Delete line in visible viewport: full-width red tint quad (rgba ~(0xFF, 0x50, 0x50, 0x40)).
  - For each Insert line: full-width green tint quad (~(0x50, 0xFF, 0x80, 0x40)).
  - For each Replace → intra-line: character-level red/green spans within the Delete/Insert lines.
- [ ] 5.3. Left gutter gets per-line markers: `+`, `-`, `~` for Insert, Delete, Replace.
- [ ] 5.4. The editor's normal text layer renders on top — colors visible through the translucent hunks.
- [ ] 5.5. Commit: `feat(render): DiffOverlay for inline diff rendering`.

### 6. Inline mode application

- [ ] 6.1. When a buffer enters diff mode (e.g., showing diff vs disk or diff vs HEAD), the `BufferState` gains a `diff_overlay: Option<DiffOverlay>`.
- [ ] 6.2. Content shown is the *combined* view — both the Delete and Insert lines appear in the buffer simultaneously (Delete lines above Insert lines for each Replace hunk). This is how `git diff` shows changes; visually it means the buffer's visible line count temporarily exceeds the file's line count.
- [ ] 6.3. The underlying buffer is *not* modified — diff view is a rendering layer. Editing is disabled while in diff view (or at minimum, edits show a warning banner — V3 disables editing to avoid surprises).
- [ ] 6.4. Commit: `feat(app): buffer-level diff mode with read-only state`.

### 7. Unified two-pane view

- [ ] 7.1. `editor-ui::UnifiedDiffView`:
  - Replaces the main editor area with a two-column view.
  - Each column is a minimal `TextLayer` rendering the respective content.
  - Matching lines are aligned vertically; Delete lines leave gaps on the right side, Insert lines leave gaps on the left.
- [ ] 7.2. Keyboard nav is shared between columns (scroll one, the other follows).
- [ ] 7.3. Entered via `Ctrl+K, D` followed by picking a file to compare against (V3 ships diff-vs-disk; git diff comes in M18).
- [ ] 7.4. Commit: `feat(ui): UnifiedDiffView two-column renderer`.

### 8. Keyboard navigation between hunks

- [ ] 8.1. `Ctrl+Alt+Down` / `Ctrl+Alt+Up` jump to next/previous hunk in the current diff view. Cursor (and scroll) moves to the hunk.
- [ ] 8.2. Commit: `feat(input): hunk navigation shortcuts`.

### 9. Per-hunk accept/reject API (for M23)

- [ ] 9.1. `DiffOverlay::apply_hunk(idx: usize, target_buffer: &mut TextBuffer)` — applies only that hunk's changes to the buffer, leaving the rest.
- [ ] 9.2. `DiffOverlay::reject_hunk(idx: usize)` — marks the hunk as rejected, so future M23 operations know not to apply it.
- [ ] 9.3. Each application goes through the standard `TextBuffer::apply_edit`, so undo works.
- [ ] 9.4. For now, wire it into a pair of shortcuts: `Ctrl+Alt+Enter` to accept the current hunk, `Ctrl+Alt+Backspace` to reject. These are placeholder; the AI flow in M23 will have dedicated UI.
- [ ] 9.5. Commit: `feat(diff): per-hunk accept / reject with undo-safe application`.

### 10. Benchmarks

- [ ] 10.1. Compute line diff on 100 KB vs 100 KB: < 10 ms.
- [ ] 10.2. Compute line diff on 1 MB vs 1 MB: < 200 ms.
- [ ] 10.3. Intra-line diff on 200-char lines: < 100 μs per pair.
- [ ] 10.4. Save baseline as `m17-v3`.
- [ ] 10.5. Commit: `bench(diff): line + intra-line diff benchmarks`.

### 11. Quality gates + documentation

- [ ] 11.1. Standard gates.
- [ ] 11.2. Update `/docs/ARCHITECTURE.md` with `editor-diff`.
- [ ] 11.3. Write `/docs/DIFF_RENDERING.md` describing overlay semantics, color choices, the per-hunk API.
- [ ] 11.4. Tag: `git tag -a m17-complete -m "M17 complete: diff engine and inline renderer"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. Compute a diff between two real files; inline rendering is clear and correct.
3. Intra-line highlighting shows character-level changes in Replace hunks.
4. Hunk navigation shortcuts work.
5. No V2 perf regression.
6. `m17-complete` tag pushed.

## Testing Requirements

- Unit tests on LineOp / CharOp decomposition.
- Golden tests: known inputs producing known hunk sequences.
- Benchmark thresholds met.

## Git Commit Strategy

10-12 commits. Push after items 3, 5, 7, 9, 11.

## Handoff to M18

M18 assumes:
- `editor-diff::compute_line_diff` is the one true diff.
- M18 passes `(HEAD version, working-tree version)` through this function.

---

## Standing Orders Reminder

- All diff rendering goes through `DiffOverlay`. Do not reimplement diff rendering in the git or AI mission.
- Diff mode puts the buffer in read-only state. Never allow editing while the diff overlay is active without a very explicit workflow.
- Hunk application must always go through `TextBuffer::apply_edit` so undo works. No shortcut paths.

Go.
