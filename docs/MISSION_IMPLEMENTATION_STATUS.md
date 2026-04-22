[← docs/](./) · [README](../README.md)

# Mission implementation status (M00–M28)

**Purpose:** Map each official mission in [`missions/`](missions/) to what exists in this repository **today**. Status values are deliberately pessimistic — if a mission's acceptance can't be measured from the working tree, it is *not* "Done".

**Status vocabulary:**

- **Done** — code exists, shipped via the CLI `editor-app` binary, and covered by unit + integration tests.
- **Done (code)** — shipped, but formal acceptance (measured p50/p95/p99, manual QA, installer artifacts) still open.
- **Partial** — meaningful code in the tree **and** at least one obvious acceptance criterion unmet.
- **Library only** — crate compiles with its own tests, but **no user-visible surface** in `editor-app`. Feature is dark-shipped.
- **Not started** — mission doc exists, code does not.

**Quality gates (every merge):** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, `cargo test --workspace --all-features --locked`, `cargo build --release -p editor-app`.

**Execution guidance:** [`missions/SEQUENTIAL_EXECUTION_NOTES.md`](missions/SEQUENTIAL_EXECUTION_NOTES.md) — *"Completing M14–M24 to 'perfect' is months of engineering, not a single session."*

## Matrix

| Mission | Title | Status | What's in tree | What's missing |
|--------|--------|--------|----------------|----------------|
| **M00** | Foundation research & docs | **Done** | `docs/`, `reference/`, `docs/missions/`. | — |
| **M01** | Repo scaffolding & CI | **Done** | Toolchain, fmt/clippy/test CI, `cargo-deny`, Windows manifest, 17 crates in workspace. | — |
| **M02** | Text engine | **Done** | `editor-core`: rope, undo, cursor, proptest, benches. | — |
| **M03** | Windowing & wgpu | **Done** | `GpuContext`, backends, `FrameTimer`, `--dry-run`. | — |
| **M04** | Text rendering | **Done** | `TextLayer` + glyphon/cosmic-text, bundled JetBrains Mono + system fallbacks, `text_layer_prepare` bench, `visual_smoke` (ignored). | — |
| **M05** | Frame loop & input | **Done** | `editor-input`, IME, latency trace. | — |
| **M06** | File I/O | **Done** | `editor-io` load/save, async progress, atomic save. | — |
| **M07** | Observability | **Partial** | Metrics HUD, `tracing`/optional Tracy, perf-smoke scripts. | Criterion baseline-vs-PR gate wired into CI; HUD column ordering spec finalized. |
| **M08** | MVP acceptance | **Partial** | `MVP_ACCEPTANCE.md` skeleton, stress rigs exist. | Measured p50/p95/p99 rows on reference hardware; release decision recorded. |
| **M09** | V2 gutter, selection, clipboard | **Done** | — | — |
| **M10** | V2 status bar, persistence, word nav | **Done** | `state.json`, `power_uncap_on_battery`, word boundaries. | — |
| **M11** | Release engineering | **Partial** | `release.yml` builds Windows/Linux/macOS unsigned binaries on tags; `SHA256SUMS.txt` artifact. | MSI (`wix/` scaffold only), dmg, deb, AppImage, signing, binary-size budget. |
| **M12** | Resize / DPI polish | **Done (code)** | Sync resize/DPI paint, present mode, battery cap, row scratch, `gpu_resize_stress`, CI `m12-gpu-resize-windows`. | Release QA: videos, logged p99 acceptance, `m12-complete` tag. |
| **M13** | Workspace & multi-buffer | **Done (code)** | `editor-workspace` crate: `Workspace`, `BufferManager` (MRU), `FileSystemEvent` (notify). Wired in `editor-app`. Coherence tests in `crates/editor-workspace/tests/m13_coherence.rs`. Workspace FS events flag `external_modified` on matching buffers. | Buffer-switch does not preserve `UndoStack` history (documented in `FOLLOWUPS.md`). |
| **M14** | Sidebar, tabs, quick open | **Done (code)** | `editor-ui::{Sidebar, TabStrip, QuickOpenPalette, CommandPalette}` painted via `FrameChrome`. `Ctrl+B` / `Ctrl+Shift+E` / `Ctrl+P` / `Ctrl+Shift+P` / `Ctrl+Tab` / `Ctrl+W` / `Ctrl+N` wired. Sidebar keyboard navigation (`Up/Down/Home/End/PageUp/PageDown`, `Left`/`Right` collapse/expand, `Enter/Space` activate, `Esc` defocus). Tab close X + dirty dot are rect-based icons. Breadcrumbs strip under the tab bar. Mouse routing respects chrome zones. Keyboard intercept while any palette visible. Sidebar width/visibility persist. `editor-app <dir>` auto-opens workspace. | Explicit `Ctrl+K Ctrl+O` keybinding for folder-open; dirty-guard dialog on `Ctrl+W`; clickable breadcrumb navigation. |
| **M15** | Syntax highlighting | **Done (code)** | `editor-syntax` crate: hand-written single-line lexers for Rust, TOML, JSON (+ JSONC / JSON5), Markdown. `LineState` carrier threads Rust's nested `/* ... */` across lines (2048-line prelude scan in `fill_visible_lines`). `editor-render::TextLayer` shapes colored runs via `cosmic_text::Buffer::set_rich_text`. Theme exposes 9 syntax slots; colors mapped per `TokenKind`. `Language::from_path` auto-detects via extension + well-known filenames (`Cargo.lock`, `rust-toolchain`). 60 crate tests. | `tree-sitter` backend behind the same `Language` contract; grammars for TS/JS/Python/HTML/CSS/YAML; semantic tokens beyond pure lexical classes (e.g. variable vs type resolution). |
| **M16** | Find / Replace | **Done (code)** | `editor-search::{in_file, project}` matchers (regex + literal). `editor-ui::FindBar` holds query/replace state, flags, matches, regex error. `editor-app` wires `Ctrl+F` / `Ctrl+H` / `F3` / `Shift+F3` via `EditorCommand::{FindInFile, ReplaceInFile, FindNext, FindPrev}`; `paint_find_bar_into_chrome` paints semi-transparent per-match highlights, a dark backdrop strip, and an overlay with find/replace fields + match count. IME commits route to the focused field; caret jumps to the current match and scrolls into view. | Workspace-wide results panel (streaming `project::start_project_search` into a chrome side panel); toolbar-style toggles instead of cryptic `[lit]` / `Aa` / `ab` flag tags. |
| **M17** | Diff engine & renderer | **Partial** | `editor-diff`: `compute.rs` Myers diff + `intra_line.rs` + `session.rs` + `display.rs`. `editor-ui::gutter_marks` painted at the left gutter edge from a cached `hunks` list that refreshes version-gated. | Inline preview / side-by-side diff view mode; review sidebar for multi-file changes. |
| **M18** | Git integration baseline | **Partial** | `editor-git::GitRepo::discover` + branch read via `gix`. Branch name in status bar, refreshed immediately on `.git/HEAD` / `.git/refs/**` / `.git/packed-refs` FS events (via the existing workspace `notify` debouncer); 60 s safety poll catches the rare case the watcher misses (packed-refs rewrites, WSL edge cases). Per-file modified line count rendered as `N ±` next to the branch (from the same gutter-marks cache that drives the left-edge stripes). `path_inside_dot_git` + `is_git_ref_like` helpers with unit tests. | Stage / commit / diff / push / pull UI; conflict resolution view. |
| **M19** | AI provider abstraction | **Library only** | `editor-ai-provider` (~2k lines): Anthropic, OpenAI + compat, Ollama, custom, rate limit, SSE, keyring secrets, `probe` + `registry`. | Any UI/command wiring in `editor-app`. No chat entry point; provider toggled only via config file. |
| **M20** | Agent tool-use API | **Library only** | `editor-ai-tools` (~2k lines): `registry`, `transaction`, `path`, Anthropic tool adapter, 8 tools skeleton. | Execution loop in the app; safety confirmations UI; event/telemetry surface; integration with M19 streaming. |
| **M21** | Metadata sidecar system | **Library only** | `editor-metadata` (~1.1k lines): `.ide/` schema (serde_yaml), loader, watcher. | Surface in `editor-app` (chat context, per-file notes panel); sidecar editor UI. |
| **M22** | Local vector index | **Library only** | `editor-index`: `code_chunks` (tree-sitter-based splitter for rust/ts/py), `embedder` (noop/openai/voyage/ollama), `store` (rusqlite), `indexer`, `retrieve`, `cli`, `incremental`. CLI subcommand `index` exists. | Any UI surface; runtime integration with M23 chat; incremental reindex on save; progress UI. |
| **M23** | AI chat panel | **Not started** | — | Chat panel UI in `editor-ui`, message model, streaming into chrome, conversation persistence. **No file exists.** |
| **M24** | V3 acceptance & release | **Not started** | `V3_ACCEPTANCE.md` skeleton. | Cannot measure acceptance of features that aren't wired (M15/M17/M19/M20/M21/M22/M23). |
| **M25** | Critical fixes & completion | **Not started** | Mission doc exists. | Content depends on which critical fixes are scoped — open for interpretation. |
| **M26** | Integrated terminal | **Done (code)** | `editor-terminal` crate + pane in `editor-app` (portable-pty + alacritty_terminal). Hotkey `Ctrl+` ` toggles. Emulator render snapshot is painted. | Multi-terminal tabs inside the pane; scrollback search; copy-on-select polish. |
| **M27** | AI skills system | **Library only** | `editor-skills`: `SkillPersistence`, `PromptBuilder`, skill manifest loader, bundled `using-terminal` skill. `editor-app` persists `skills_disabled` + `extra_skill_dirs` in session state. | Skill execution surface (needs M19 + M20); skills settings UI. |
| **M28** | Settings UI & API keys | **Partial** | `editor-settings::SettingsStore` (layered JSON); `editor-app` shows a read-only text overlay via `Ctrl+,`; keyring-backed secret storage in `editor-ai-provider::secrets`. | Interactive settings panel UI; key-binding editor; per-provider API-key entry flow. |

## Shipping features summary

What **runs today** when you launch `editor-app.exe`:

- Text editing (rope, undo/redo, word nav, multi-line selection, clipboard).
- File I/O with encoding detection and atomic save.
- Syntax highlighting for Rust / TOML / JSON / Markdown (auto-detected by path).
- In-file find / replace (`Ctrl+F` / `Ctrl+H`, `F3` / `Shift+F3`) with match highlights.
- Status bar (cursor, encoding, line ending, git branch, external-modified marker).
- Session persistence (last file, cursor, scroll, window geometry, sidebar state).
- Multi-buffer tabs with MRU ordering; FS event flags external modifications.
- Sidebar file explorer (click to open, click folder to expand/collapse).
- Quick-open fuzzy palette (`Ctrl+P`).
- Settings read-only overlay (`Ctrl+,`).
- Integrated terminal pane (shell + emulator).
- Dev HUD + tracing.

What **does not** run today (but has scaffolding/libraries):

- Workspace-wide results panel (M16 — project matcher streams, no UI).
- Dedicated diff view mode (M17 — gutter marks ship, no side-by-side).
- Git commit / push / pull UI (M18 — only branch name).
- Streaming LLM / chat / agents / tools (M19/M20/M23 — libraries only).
- Metadata sidecar UI (M21 — library only).
- Vector index UI (M22 — CLI only).
- Skills execution surface (M27 — library only).
- Interactive settings panel (M28 — text readout only).

See [`00_V3_VISION.md`](missions/00_V3_VISION.md) for the V3 product arc, and [`../FOLLOWUPS.md`](../FOLLOWUPS.md) for specific blockers.
