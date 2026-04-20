# M15 — Syntax Highlighting via Tree-sitter

**Mission ID:** M15
**Prerequisites:** M14 complete. Multi-buffer UI works.
**Output:** Code displayed in the editor is now colored. Keywords, strings, comments, functions, types, and other syntactic categories are assigned colors from a small, deliberate palette. Highlighting runs incrementally via Tree-sitter — on each keystroke, only the affected nodes re-parse, so typing stays under the 5 ms input-to-pixel latency target even on a 10 MB file. Six core languages ship in V3: Rust, Python, TypeScript, JavaScript, Markdown, JSON, TOML. Adding more languages later is a pure data change (ship a grammar + query file).
**Estimated scope:** 2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — Ring 2 developer affordances.
- `/docs/TEXT_ENGINE.md` — the rope's `edit` API and version counter (Tree-sitter needs incremental updates).
- `/docs/RENDERING_PIPELINE.md` — glyphon + cosmic-text per-glyph coloring semantics.
- `https://docs.rs/tree-sitter/` — core parsing API.
- `https://docs.rs/tree-sitter-highlight/` — highlight query evaluation.
- `https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html` — the `highlights.scm` query language.

---

## The Situation In Plain English

The editor has been showing monochrome text since M04. That's fine for a demo but unacceptable for real code. M15 adds syntax highlighting — and because we chose Tree-sitter as the underlying parsing engine, we get it right: incremental, fast, accurate, works on malformed code (parsers with error recovery, unlike regex-based highlighters).

Tree-sitter is the parsing library behind GitHub's code navigation, Atom, Neovim's modern highlighter, Helix, and Zed. It produces a concrete syntax tree for a source file and updates that tree in sub-millisecond time when you edit. Syntax highlighting is then a matter of running a query (in Tree-sitter's S-expression query language) against the tree to find nodes that should be colored: `(identifier) @variable`, `(string_literal) @string`, etc. A `highlights.scm` file maps node patterns to capture names like `@keyword` or `@string`. A theme maps capture names to colors.

Our job is to wire this into our text pipeline. On every edit, we call Tree-sitter's `Parser::parse_with` with the old tree and an `InputEdit` describing the change. We get back a new tree. We walk the tree inside the visible viewport, run the highlight query, and get back a list of `(byte_range, capture_name)` tuples. We translate those into per-glyph color attributes that glyphon honors through cosmic-text's `Attrs::color`. Render as usual.

The key design decision is *incrementality everywhere*. Parsing is incremental (Tree-sitter gives us that for free). Highlight-range computation is incremental too: we only re-compute highlights for lines the edit actually affected. The layout cache (from M04) keys by highlight-version in addition to content-version, so a pure cursor move doesn't invalidate highlights and a pure highlight update doesn't invalidate shaping. These compose into a system where a single-character edit in a 10 MB file produces a single frame of re-parsing + re-highlighting + re-shaping of the affected line(s), adds one frame's worth of GPU work, and results in a visible update in the same frame.

The six languages we ship cover Brian's likely day-to-day: Rust (for the project itself), Python, TypeScript/JavaScript (web work), Markdown (documentation), JSON and TOML (configuration files). Each language requires a Tree-sitter grammar crate plus a `highlights.scm` query file. The grammar crates are on crates.io; the query files are in the grammar repos. We bundle both into our binary.

---

## Scope

**In scope:**
- New `editor-syntax` crate that owns the Tree-sitter integration.
- Per-language `Grammar` struct holding Language + HighlightConfiguration.
- `SyntaxState` per buffer: current tree, current highlight ranges, current language.
- Incremental re-parse on every buffer edit.
- Visible-range highlight computation.
- Per-glyph coloring via cosmic-text `Attrs::color`.
- Language auto-detection by file extension (`.rs` → Rust, `.py` → Python, etc.).
- A fixed dark theme mapping capture names to colors.
- Six bundled grammars: Rust, Python, TypeScript, JavaScript, Markdown, JSON, TOML.
- Status bar (M10) now shows the detected language.

**Out of scope:**
- User-customizable themes (V4+).
- Light theme beyond what V2's design system specified (V4+).
- Syntax-aware features like bracket matching, auto-indent, smart navigation (V4+; these need more than highlights).
- LSP semantic highlighting (V4+; different beast entirely).
- Injected languages (e.g., JS inside HTML, SQL inside strings) — V4+.
- User-added languages through config (V4+).

---

## North Star

Open a Rust file. `fn` is purple. `String` is green. `"hello"` is orange. `// comment` is grey and italic (if our font has an italic variant; plain grey otherwise). Type a character. The new character appears with the correct color in the same frame. Edit a string and watch only the affected glyphs re-color. Open a Python file — different palette application but same capture-name taxonomy so the visual language stays consistent. Open a 10 MB generated Rust file and scroll through it; colors render at 120 fps just like plain text did.

---

## TODO List

### 1. Create the `editor-syntax` crate

- [ ] 1.1. `cargo new --lib crates/editor-syntax`.
- [ ] 1.2. Deps: `tree-sitter = "0.24"` (or current major), `tree-sitter-highlight = "0.24"`, grammar crates: `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-typescript`, `tree-sitter-javascript`, `tree-sitter-md` (markdown), `tree-sitter-json`, `tree-sitter-toml-ng` (or current). Verify each grammar's latest version is compatible with the core `tree-sitter` version on crates.io — the ecosystem has churn.
- [ ] 1.3. Build-time verification: if version skew between core and grammars is common, lock each exact version in `Cargo.toml` after a successful local build.
- [ ] 1.4. Commit: `feat(syntax): scaffold editor-syntax crate with grammar deps`.

### 2. Bundle `highlights.scm` query files

- [ ] 2.1. Each grammar crate typically exports `HIGHLIGHT_QUERY` as a `&str` constant — verify this is present for each of the 6 grammars. If any grammar doesn't ship it, fetch the `highlights.scm` from the grammar's GitHub repo and `include_str!` it.
- [ ] 2.2. For Rust, Python, JS/TS, JSON, TOML, Markdown: confirm each query uses the standard tree-sitter capture names (`@keyword`, `@string`, `@function`, etc.) so our theme mapping is cross-language consistent.
- [ ] 2.3. Commit: `feat(syntax): bundle highlights.scm queries for all 6 languages`.

### 3. Design `Grammar` and `SyntaxState`

- [ ] 3.1. `crates/editor-syntax/src/lib.rs`:
  ```rust
  pub struct Grammar {
      pub language_id: &'static str,       // "rust", "python", ...
      pub display_name: &'static str,      // "Rust", "Python"
      pub extensions: &'static [&'static str],  // ["rs"], ["py"]
      language: tree_sitter::Language,
      highlight_config: HighlightConfiguration,
  }

  pub struct SyntaxState {
      grammar: Option<&'static Grammar>,
      parser: tree_sitter::Parser,
      tree: Option<tree_sitter::Tree>,
      highlighter: tree_sitter_highlight::Highlighter,
      /// Cached highlight ranges covering at least the visible viewport.
      /// Invalidated by any buffer edit.
      cached_highlights: Vec<HighlightSpan>,
      cache_valid_byte_range: Range<usize>,
      cache_version: u64,
  }

  pub struct HighlightSpan {
      pub byte_range: Range<usize>,
      pub highlight_id: u32,            // index into the theme palette
  }
  ```
- [ ] 3.2. `Grammar`s are static — one per supported language, constructed at startup, shared across all buffers.
- [ ] 3.3. `SyntaxState` is per-buffer.
- [ ] 3.4. Commit: `feat(syntax): Grammar and SyntaxState types`.

### 4. Implement language auto-detection

- [ ] 4.1. `pub fn detect_language(path: &Path) -> Option<&'static Grammar>`: match on extension. First-pass; filename-based detection (e.g., `Dockerfile` has no extension) can be a follow-up.
- [ ] 4.2. Also accept a content-based fallback: if extension doesn't match, peek the first line for a `#!/usr/bin/env python` shebang and detect from there. Only for MVP-important cases.
- [ ] 4.3. If no grammar matches, `SyntaxState` runs with `grammar = None` — no highlighting, no parsing, no overhead.
- [ ] 4.4. Commit: `feat(syntax): language detection by extension and shebang`.

### 5. Initial parse

- [ ] 5.1. `SyntaxState::set_grammar(grammar, buffer_bytes)`: configure `parser.set_language`, call `parser.parse(buffer_bytes, None)`, store the resulting tree.
- [ ] 5.2. For a buffer just loaded from disk, this happens once after the load completes — run it on the `WorkerPool` for large files to avoid blocking the frame.
- [ ] 5.3. Target: parse a 1 MB Rust file in under 50 ms; a 10 MB file in under 500 ms. Tree-sitter typically meets these easily.
- [ ] 5.4. Commit: `feat(syntax): initial parse on buffer load`.

### 6. Incremental re-parse on edit

- [ ] 6.1. Every `TextBuffer::apply_edit` produces an `Edit` value with (old range, new range, old bytes, new bytes). Translate to `tree_sitter::InputEdit`:
  ```rust
  let input_edit = InputEdit {
      start_byte,
      old_end_byte,
      new_end_byte,
      start_position: Point { row: start_line, column: start_col_bytes },
      old_end_position: ...,
      new_end_position: ...,
  };
  ```
- [ ] 6.2. Call `tree.edit(&input_edit)` to apply the edit descriptor; then `parser.parse_with(&mut callback, Some(&tree))` where the callback provides the new buffer bytes on demand.
- [ ] 6.3. The callback is a closure over the `TextBufferSnapshot`. It returns a byte slice for the requested (byte_offset, line, col) range.
- [ ] 6.4. Target: incremental re-parse for a single-character edit: under 1 ms for a 1 MB file, under 3 ms for a 10 MB file.
- [ ] 6.5. Commit: `feat(syntax): incremental re-parse via tree.edit + parser.parse_with`.

### 7. Compute highlight spans for the visible viewport

- [ ] 7.1. `SyntaxState::compute_highlights(byte_range: Range<usize>, buffer_bytes: &[u8]) -> &[HighlightSpan]`:
  - If `cache_valid_byte_range` contains `byte_range` and `cache_version == current`, return cache.
  - Otherwise, run `highlighter.highlight(config, byte_range_slice, None, |_| None)` which returns an iterator of `HighlightEvent`.
  - Walk the events, accumulating `HighlightSpan`s. Store in `cached_highlights` with the new range.
- [ ] 7.2. The viewport-only highlight is what makes this fast. Even with a 10 MB file, we only compute ~55 lines of highlights per frame.
- [ ] 7.3. Commit: `feat(syntax): viewport-scoped highlight computation with caching`.

### 8. Define the dark theme

- [ ] 8.1. `editor-syntax::theme::DEFAULT_DARK`:
  ```rust
  pub struct Theme {
      pub palette: Vec<Color>,    // indexed by highlight_id
      pub name_to_id: HashMap<&'static str, u32>,
  }
  ```
- [ ] 8.2. Bindings (approximate — pick final colors that look clean on a zinc background):
  - `keyword` → purple (#C792EA)
  - `string` → orange (#F78C6C)
  - `comment` → grey italic (#5C6773 if italic variant available; plain grey otherwise)
  - `function` → yellow (#FFCB6B)
  - `function.builtin` → yellow (#FFCB6B)
  - `type` / `type.builtin` → teal (#82AAFF)
  - `variable` / `variable.parameter` → default text color (#E0E0E0)
  - `constant` / `constant.builtin` → red-orange (#F78C6C)
  - `number` → red-orange (#F78C6C)
  - `operator` / `punctuation` → subdued (#89DDFF)
  - `property` → cyan (#82AAFF)
- [ ] 8.3. Test the colors on a sample Rust and Python file; iterate to taste.
- [ ] 8.4. Commit: `feat(syntax): default dark theme palette`.

### 9. Wire highlights into glyphon's per-glyph coloring

- [ ] 9.1. `TextLayer::prepare` currently builds one `TextArea` per line with a single default color. Extend it to accept `Vec<HighlightSpan>` overlaid on the line content.
- [ ] 9.2. Cosmic-text's `Attrs::color(Color::rgba(r, g, b, a))` sets color per run. When building a line's `Buffer`, split into runs at highlight boundaries, each run with its own `Attrs`.
- [ ] 9.3. This means re-shaping the line when highlights change. Extend the layout cache key: `(line_index, content_version, highlights_version)`.
- [ ] 9.4. Glyphon internally honors per-glyph colors; no changes needed there.
- [ ] 9.5. Benchmark: line shaping with ~5 runs vs 1 run: ~2x the cost in shaping. Well within budget since we're only shaping visible lines.
- [ ] 9.6. Commit: `feat(render): per-run highlight colors through cosmic-text`.

### 10. Plumb into `BufferState`

- [ ] 10.1. `BufferState` (from M13) gains a `syntax: SyntaxState` field.
- [ ] 10.2. On buffer load: detect language, initialize syntax state, run initial parse on worker.
- [ ] 10.3. On every edit: apply to syntax state incrementally.
- [ ] 10.4. On language change (e.g., user manually overrides): replace syntax state.
- [ ] 10.5. Commit: `feat(workspace): BufferState owns SyntaxState; wired on load + edit`.

### 11. Status bar shows the language

- [ ] 11.1. Extend `StatusBarInfo` (M10) with `language: Option<&'static str>` (display name).
- [ ] 11.2. Render in the right side of the status bar: `"UTF-8 · LF · Rust"`.
- [ ] 11.3. Commit: `feat(ui): status bar shows detected language`.

### 12. Manual language override (small convenience)

- [ ] 12.1. Keyboard: no shortcut in V3. Add a follow-up for a language picker in V4+.
- [ ] 12.2. Config: allow `.ide/language-overrides.toml` at workspace root with `extension → language` overrides. Parse on workspace open. Rare feature, very small cost.
- [ ] 12.3. Commit: `feat(workspace): language override config`.

### 13. Stress test and benchmarks

- [ ] 13.1. Open a 10 MB Rust file. Measure: initial parse time, first highlight compute time, subsequent edit frame time. Target: all under 100 ms for initial, under 5 ms for incremental.
- [ ] 13.2. Open a 1000-line Python file. Type rapidly (1000 chars over 10 seconds). Assert p99 frame time stays under 5 ms.
- [ ] 13.3. Open a malformed file (broken syntax). Tree-sitter should recover gracefully; highlights may be partial but not crash.
- [ ] 13.4. Save baseline as `m15-v3`.
- [ ] 13.5. Commit: `bench(syntax): parse + highlight benchmarks on multiple languages`.

### 14. Cross-platform verification

- [ ] 14.1. Grammar crates compile C code; verify they build on Windows MSVC, macOS Xcode, Linux GCC. This is usually fine but occasionally a grammar has portability issues.
- [ ] 14.2. CI: add a dedicated `syntax-smoke` job that loads one file of each language and asserts highlights are produced.
- [ ] 14.3. Commit: `ci(syntax): cross-platform syntax smoke test`.

### 15. Quality gates + documentation

- [ ] 15.1. Standard gates.
- [ ] 15.2. No V2 perf regression on un-highlighted (e.g., plain .txt) files.
- [ ] 15.3. Write `/docs/SYNTAX_HIGHLIGHTING.md` describing the architecture, adding a new language (follow-up recipe), theme format.
- [ ] 15.4. Update `/CHANGELOG.md`.
- [ ] 15.5. Tag: `git tag -a m15-complete -m "M15 complete: Tree-sitter syntax highlighting"`; push.

---

## Validation / Acceptance Criteria

1. Quality gates pass.
2. Rust, Python, TS, JS, Markdown, JSON, TOML all highlight correctly on sample files.
3. Typing in a 10 MB Rust file stays under 5 ms p99 input-to-pixel.
4. Initial parse of a 1 MB file under 50 ms.
5. Status bar shows the language.
6. Malformed files don't crash; partial highlighting is acceptable.
7. CI green on all 3 OSes.
8. `m15-complete` tag pushed.

## Testing Requirements

- Unit tests for `detect_language`.
- Integration tests for each language (parse + highlight a small sample).
- Benchmarks captured.
- Visual sanity check on a real codebase.

## Git Commit Strategy

12-14 commits. Push after items 3, 7, 9, 13, 15.

## Handoff to M16

M16 assumes:
- Code is colored; reading the editor feels like a real IDE.
- Parse trees are available per buffer — M16's search can optionally use them (symbol-aware search is a V4+ feature, but the infrastructure is here).
- Highlights don't regress performance; search can rely on the same fast content access.

---

## Standing Orders Reminder

- The layout cache key must include highlight version. If you forget, edits will show stale colors.
- Tree-sitter has version skew between core and grammar crates. When you bump any of them, rebuild and re-test all 6 languages.
- Never run `parser.parse` without passing the old tree — always use incremental. Non-incremental parse of a 10 MB file is ~500 ms; incremental is ~1 ms.
- The dark theme colors are tuned for zinc-on-zinc readability. If you change the editor background, re-tune the theme.

Go.
