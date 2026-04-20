[← docs/](./) · [README](../README.md)

# Mission Index

Missions run in strict order. Each assumes the previous is complete. Each
leaves the repository with `cargo fmt`, `cargo clippy`, `cargo test`, and
`cargo build --release` all green on Windows (and on CI, on Linux and
macOS).

| # | ID | Title | Primary Output |
|---|---|---|---|
| 00 | `M00` | Foundation Research & Documentation | `docs/` tree with 18 reference files |
| 01 | `M01` | Repo Scaffolding, Workspace, Toolchain, CI | Multi-crate workspace, CI green on 3 OSes |
| 02 | `M02` | Text Engine: Rope Buffer, Cursor, Undo/Redo | `editor-core` with benchmarks |
| 03 | `M03` | Windowing & wgpu Rendering Foundation | Window opens; clear color renders via GPU |
| 04 | `M04` | Text Rendering with glyphon | Visible text renders from the rope |
| 05 | `M05` | Frame Loop, Input Pipeline, Performance Budgets | Typing works at < 5 ms latency |
| 06 | `M06` | File I/O: Async Load, mmap, Atomic Save | Open & save large files without blocking |
| 07 | `M07` | Observability, Profiling, Dev Overlay | Per-frame metrics visible in dev mode |
| 08 | `M08` | MVP Integration, Stress Testing, Acceptance | PRD performance targets met |
| 09 | `M09` | V2: Line Numbers, Selection, Clipboard, Undo UI | Minimally useful editor |
| 10 | `M10` | V2: Word Nav, Status Bar, Persistence, Polish | V2 acceptance criteria met |
| 11 | `M11` | Release Engineering & Cross-Platform Packaging | Installable binaries for all 3 OSes |

---

## M00 — Foundation Research & Documentation

**Goal.** Lay the intellectual foundation for the project before any Rust
is written. Capture the architecture, tech-stack decisions, performance
model, risks, and mission plan in `docs/`.

**Scope.**

- Create `docs/` with the reference files listed in `README.md`
  (including `ARCHITECTURE.md` and `TECH_STACK.md` at the top of the tree).
- Create root-level `README.md`, `CHANGELOG.md`, `LICENSE-APACHE`,
  `LICENSE-MIT`, `CONTRIBUTING.md`, `DEVELOPMENT.md`, `FOLLOWUPS.md`,
  and `.gitignore`.
- Create `reference/` with the verbatim source PRDs.
- Initialize the git repository on `main` and set the origin to
  `https://github.com/HiNala/IDE.git`.
- Commit and push. Tag `m00-complete`.

**Out of Scope.** Any Rust code, any Cargo manifests, any CI.

**Done When.** All docs merged; `git push` succeeds; `m00-complete` tag
pushed.

---

## M01 — Repo Scaffolding, Workspace, Toolchain, CI

**Goal.** Create a buildable Rust workspace with no functional code but
full toolchain, crate structure, and a CI matrix running on Windows,
Linux, and macOS.

**Scope.**

- Virtual workspace `Cargo.toml` with members:
  - `crates/editor-core`
  - `crates/editor-render`
  - `crates/editor-input`
  - `crates/editor-io`
  - `crates/editor-app` (binary)
- Per-crate `Cargo.toml` with pinned minor versions from `TECH_STACK.md`.
- `rust-toolchain.toml` pinning stable, including `rustfmt` and `clippy`.
- `rustfmt.toml` and `clippy.toml` (or `[lints]` tables) with the project
  style.
- `deny.toml` for `cargo-deny` (licenses, advisories, bans).
- Minimal `lib.rs` / `main.rs` stubs that compile and have at least one
  trivial passing test.
- `.github/workflows/ci.yml` with:
  - Matrix over `windows-latest`, `ubuntu-latest`, `macos-latest`.
  - Steps: `cargo fmt --check`, `cargo clippy -D warnings`,
    `cargo test --all`, `cargo build --release`.
  - `actions/checkout@v4`, `dtolnay/rust-toolchain@stable`,
    `Swatinem/rust-cache@v2`.
- Dependabot config (optional but recommended).

**Out of Scope.** Rope implementation, windowing, rendering.

**Done When.** `cargo build --release` succeeds locally on Windows, and CI
is green on all three OSes on the first push.

---

## M02 — Text Engine: Rope Buffer, Cursor, Undo/Redo

**Goal.** Implement `editor-core` as a pure-Rust library that models the
document, cursor, selection, and undo/redo.

**Scope.**

- `Document` type wrapping `ropey::Rope` with stable byte and char
  indexing.
- `Cursor` and `Selection` with grapheme-aware movement using
  `unicode-segmentation`.
- `Edit` record type (insert/delete) with inverse computation for undo.
- `History` with bounded memory (coalesces adjacent inserts, compacts on
  milestones).
- Byte ↔ char ↔ line index conversions.
- Line ending detection and LF-internal normalization.
- Criterion benchmarks: insert at head / middle / tail of 1 MiB / 10 MiB /
  100 MiB documents; char-index lookup; line-at-offset.
- proptest: round-tripping edits, undo inversion, cursor invariants.

**Out of Scope.** Rendering, I/O, windowing.

**Done When.** Benchmarks published to `docs/benchmarks/m02.md`; all edit
ops complete in target times; full test suite green.

---

## M03 — Windowing & wgpu Rendering Foundation

**Goal.** Open a native window via `winit` 0.30 using the
`ApplicationHandler` pattern. Initialize a wgpu `Instance` / `Adapter` /
`Device` / `Queue` / `Surface`. Clear the surface to a known color each
frame.

**Scope.**

- `editor-render::Renderer` owning wgpu resources.
- Surface (re)configuration on resize including DPI scale.
- Backend selection: DX12 (Windows), Metal (macOS), Vulkan (Linux), GL
  fallback.
- Present mode: `Mailbox` preferred, `Fifo` fallback.
- `editor-app` main loop using winit's event handler.
- Proper shutdown (drop order: surface → device → adapter → instance).
- Smoke tests that construct a headless wgpu instance where possible.

**Out of Scope.** Glyph rendering, text, input handling beyond window close.

**Done When.** `cargo run --release` opens a window on Windows, renders a
clear color at steady fps, and closes cleanly. CI builds (but does not
run) the renderer crate on all three OSes.

---

## M04 — Text Rendering with glyphon

**Goal.** Draw text from the rope onto the wgpu surface via `glyphon`
(`cosmic-text` under the hood).

**Scope.**

- Glyph atlas creation and reuse across frames.
- Layout: single document, single font, configurable size.
- Viewport-aware rendering: only lines that intersect the visible area are
  shaped and drawn.
- Dirty-line tracking: only re-shape lines whose content or wrap width
  changed since the last frame.
- DPI/HiDPI correctness.

**Out of Scope.** Selection highlight, line numbers (V2), multiple fonts,
syntax color.

**Done When.** A user can see the contents of a loaded rope rendered
crisply at the target frame rate on Windows.

---

## M05 — Frame Loop, Input Pipeline, Performance Budgets

**Goal.** Wire keyboard/mouse input through `editor-input` into document
mutation and achieve < 5 ms input-to-pixel latency.

**Scope.**

- `editor-input::InputTranslator` mapping `winit::event::WindowEvent` into
  `editor-core::Edit` / cursor ops.
- Key binding table with Windows / macOS conventions.
- Primary cursor movement, backspace, delete, return.
- Frame loop: input → state → render, each phase with a budget timer.
- Latency measurement hooks (see `OBSERVABILITY.md`).
- Input-to-pixel latency Criterion harness that simulates keyboard bursts.

**Out of Scope.** Selection (V2), clipboard (V2), IME polish (risk-listed).

**Done When.** Typing into the editor feels instantaneous; Criterion
reports sub-5 ms latency on the development machine; tests cover cursor
edge cases.

---

## M06 — File I/O: Async Load, mmap, Atomic Save

**Goal.** Open and save files, including very large ones, without
blocking the UI.

**Scope.**

- `editor-io::load_file(path)` — async, streams into rope.
- `editor-io::load_mmap(path)` — memory-maps files over a threshold.
- `editor-io::save_atomic(path, contents)` — temp file + `fsync` + rename.
- Line-ending preservation on save; LF normalization on load.
- Encoding sniffing (UTF-8 + UTF-8 BOM for MVP; `encoding_rs`
  infrastructure ready but not user-facing).
- proptest: round-trip load/save preserves content.
- Opens a 100 MB file without dropping frames.

**Out of Scope.** Autosave, backup, multi-file tabs.

**Done When.** A 100 MB text file loads without a stutter, edits apply,
saves are atomic and verified by an external reader.

---

## M07 — Observability, Profiling, Dev Overlay

**Goal.** Make the performance contract visible.

**Scope.**

- `tracing` instrumentation across subsystems.
- Frame metrics: per-phase times, GPU queue depth, rope chunk count,
  cursor position, dirty line count, peak RSS.
- Dev overlay (toggle `F1`): renders those metrics on top of the text.
- Chrome tracing export on `--trace` flag.
- `cargo bench` CI step that publishes benchmark summaries as a build
  artifact.

**Out of Scope.** User-facing analytics; telemetry upload.

**Done When.** Running `cargo run --release --features dev-overlay` and
pressing `F1` shows live metrics with zero measurable performance impact
when the overlay is off.

---

## M08 — MVP Integration, Stress Testing, Acceptance

**Goal.** Prove the MVP meets the PRD's non-functional requirements.

**Scope.**

- End-to-end stress tests: 100 MB / 500 MB / 1 GB files, typing bursts,
  rapid scroll, resize, suspend/resume.
- Multi-hour soak test with bounded memory growth assertion.
- Acceptance matrix filled in `STATUS.md`.
- All six performance targets measured on at least one Windows machine
  and one Linux machine.
- Regression benchmarks added to CI.

**Out of Scope.** Any V2 feature.

**Done When.** Every item in `MVP_DEFINITION.md §8 Acceptance Criteria`
is measured and marked PASS.

---

## M09 — V2: Line Numbers, Selection, Clipboard, Undo UI

**Goal.** Turn the MVP into a minimally useful editor.

**Scope.**

- Line-number gutter as a separate render pass with its own atlas cache.
- Text selection (shift-navigation and mouse drag), rendered as GPU
  rectangles aligned to glyph bounds.
- System clipboard integration (`arboard` or platform-specific, decided in
  the mission).
- Undo/redo wired into keybindings and exposed visually via a short
  status-bar confirmation.

**Out of Scope.** Tabs, project tree, syntax, autocomplete.

**Done When.** A developer can open a file, select and copy text, paste,
undo, and save — all without exceeding MVP performance budgets.

---

## M10 — V2: Word Nav, Status Bar, Persistence, Polish

**Goal.** Meet the full V2 PRD acceptance list.

**Scope.**

- Word-level cursor movement (Ctrl/Option + arrows).
- Status bar: file path, cursor position, modified flag, line endings.
- "Reopen last file on launch" using `directories` for config storage.
- Visual polish: cursor blink, selection color, focus styling.
- V2 acceptance run in `STATUS.md`.

**Done When.** Every V2 PRD acceptance criterion is measured and marked
PASS.

---

## M11 — Release Engineering & Cross-Platform Packaging

**Goal.** Produce installable binaries for Windows, Linux, and macOS.

**Scope.**

- Release profile tuning (`lto = "thin"`, `codegen-units = 1`, strip).
- Windows: MSI via `cargo-wix` or MSIX via `msix-packaging` (decided in
  mission).
- macOS: notarized `.app` bundle and `.dmg` (codesign prep; notarization
  gated by availability of Apple credentials).
- Linux: `.deb`, `.rpm`, and AppImage via `cargo-deb`, `cargo-generate-rpm`,
  and `appimagetool`.
- Cross-compilation where possible (`cross` for Linux targets from Windows).
- GitHub Actions `release.yml` that triggers on tag push.
- Checksums and signing keys documented in `CROSS_PLATFORM.md`.

**Done When.** A tagged release produces installable artifacts for all
three OSes as GitHub release assets.

---

## After M11

Out of scope for this mission set. Future missions (M12+) would add
syntax highlighting, LSP, AI integration, and plugins — **only** after
confirming each keeps the performance contract.

---

*Last updated: M00.*
