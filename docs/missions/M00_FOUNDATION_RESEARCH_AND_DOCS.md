# M00 — Foundation Research & Reference Documentation

**Mission ID:** M00
**Prerequisites:** None. This is the first mission.
**Output:** A `/docs` folder containing 10+ reference documents that subsequent missions read before coding.
**Estimated scope:** 1 session (heavy reading and writing, zero production code).

---

## Read First

Before beginning this mission, read the **Mission Index** (`00_MISSION_INDEX.md`) and internalize the **Standing Orders**. They apply to this mission and every mission that follows. In particular: do not stop until every TODO item is complete, think holistically, push to GitHub regularly, and follow the research → plan → execute → validate → commit → push → review loop.

You should also read all six PRD documents that the user provided for this project:

1. **Product Requirements Document** — MVP vision and non-functional constraints.
2. **Tech Stack & Architecture Choices** — Rust, winit, wgpu, rope, concurrency.
3. **Architecture Strategy & Performance Model** — Frame-based execution, budgets, determinism.
4. **Gaps, Risks, and Missing Considerations** — Cross-platform, IME, data integrity.
5. **MVP Definition** — What the MVP is and is not.
6. **V2 PRD** — The minimal-useful-editor layer on top of the MVP.

If these documents are not in your working directory, ask for them or locate them in the repo's `reference/` folder (create it if needed and copy them there). They are the source of truth for *what* we are building and *why*.

---

## The Situation In Plain English

We are about to build a ground-up, native, high-performance code editor in Rust. The architecture is inspired by real-time game engines rather than by traditional GUI applications. The closest existing analogues are Zed (GPU-accelerated via its custom GPUI framework) and Lapce (built on Floem, also wgpu-backed). We are not forking either; we are building fresh, but we will learn heavily from both.

Before a single line of production code is written, we need to make sure the coding agents working on subsequent missions have a solid, project-local reference library to pull from. LLM agents are unreliable when they work from memory alone on fast-moving ecosystems like wgpu, winit, glyphon, and cosmic-text. The APIs in these crates have evolved significantly in 2025 and 2026 (winit moved to the `ApplicationHandler` trait model; wgpu shipped major version changes; DirectX 11 became the preferred Windows backend for Zed). We want the agents to read project-specific reference notes that *we curated from current sources* before they touch code, not to guess based on stale training data.

This mission has exactly one deliverable: a thorough, current, well-organized `/docs` folder at the root of the repository. The repo is currently empty at https://github.com/HiNala/IDE, so you will initialize it, create the `/docs` folder, write the reference files, commit, and push. No Rust code yet. Just research and documentation. This is the slowest-looking but highest-leverage mission in the entire project: every downstream mission reads these files, so if they are thin or wrong, every downstream mission suffers.

---

## Scope

**In scope for this mission:**
- Initializing the GitHub repository (first commit, README, LICENSE, `.gitignore`).
- Creating a `/docs` folder with curated reference documents that summarize current best practices for every subsystem we will build.
- Copying the six PRD documents into a `/reference` folder so they are available in-repo alongside the agent-written docs.
- A `CONTRIBUTING.md` and `DEVELOPMENT.md` at the repo root so future contributors (human or agent) know the ground rules.
- A `FOLLOWUPS.md` and `CHANGELOG.md` stub.
- First push to `origin main` on https://github.com/HiNala/IDE.

**Out of scope for this mission:**
- Writing any Rust code (no `Cargo.toml`, no `src/`, no `main.rs`).
- Setting up CI (that's M01).
- Choosing between specific Rust crate versions (document options, but don't commit to `Cargo.toml` yet).
- Designing the UI.

---

## North Star For This Mission

After M00 is done, any coding agent that reads the `/docs` folder should be able to answer these questions without going back to the web:

- What is our rendering stack (winit + wgpu + glyphon + cosmic-text), and what are each library's responsibilities?
- What is our text buffer strategy (rope, specifically `Ropey` for MVP with room to migrate to a SumTree-style structure later), and why?
- How does our frame loop work (input → state → render, with explicit per-subsystem budgets)?
- What are the cross-platform landmines (Windows line endings, IME, DPI, clipboard, Unicode paths), and how do we handle each?
- What testing and benchmarking tools we use (`cargo test`, `proptest`, `criterion`) and what the perf targets are?
- What coding conventions apply (error handling with `thiserror` / `anyhow`, logging with `tracing`, formatting with `rustfmt`, linting with `clippy`).

If an agent can't answer all of the above after reading `/docs`, this mission is incomplete.

---

## TODO List (Work Through These In Order)

Keep this list open as you work. Check items off as you go. Do not skip ahead. Each top-level item is a logical unit that should land as at least one commit; sub-items can be grouped.

### 1. Initialize the repository and create the directory structure

- [ ] 1.1. Clone the repo: `git clone https://github.com/HiNala/IDE.git` and `cd IDE`. If the local directory exists already, `git pull --rebase` to make sure you have the latest state.
- [ ] 1.2. If there is no `main` branch yet (repo is empty), create it: `git checkout -b main`.
- [ ] 1.3. Create the top-level directory layout:
  ```
  /docs/           (agent-authored reference documents)
  /reference/      (verbatim PRD documents from the user)
  ```
  The `src/`, `crates/`, and `target/` directories will come in M01. Do not create them yet.
- [ ] 1.4. Create a Rust-appropriate `.gitignore` at the root (target directories, editor tempfiles, `.env`, `perf.data`, OS-specific files like `.DS_Store` and `Thumbs.db`, Criterion output directories under `target/criterion`, `*.pdb`, etc.). Use the official GitHub Rust `.gitignore` as a starting template and extend.
- [ ] 1.5. Create a `LICENSE` file. Use MIT or Apache-2.0 (or both, dual-licensed, which is the Rust ecosystem norm). Dual-license recommended.
- [ ] 1.6. Create an initial top-level `README.md` that briefly describes what the project is, links to `docs/`, and explains that the full product requirements live in `reference/`. Keep it short — under 60 lines. The detailed architecture docs live in `docs/ARCHITECTURE.md`.
- [ ] 1.7. Initial commit: `chore(repo): initialize repository with top-level docs and reference structure`. Push to `origin main`.

### 2. Copy the PRD documents into `/reference`

- [ ] 2.1. Create `/reference/00_PRODUCT_REQUIREMENTS.md`, `/reference/01_TECH_STACK.md`, `/reference/02_ARCHITECTURE_STRATEGY.md`, `/reference/03_GAPS_AND_RISKS.md`, `/reference/04_MVP_DEFINITION.md`, `/reference/05_V2_PRD.md` with the verbatim contents of the six PRD documents provided by the user.
- [ ] 2.2. Create `/reference/README.md` listing the files and a one-line summary of each. Note that these are the source-of-truth documents and should not be edited by agents during missions — they represent the product spec, not implementation guidance.
- [ ] 2.3. Commit: `docs(reference): add PRD source documents (product, tech stack, architecture, gaps, MVP, V2)`.

### 3. Research Zed, Lapce, Helix, and the current Rust editor landscape

Before writing any reference docs, do the web research you will reference. Use the web_search tool. Fetch full pages with web_fetch for key references. Save notes in a scratch file (which you will refine into `/docs` files in later steps).

- [ ] 3.1. Search and read about **Zed's architecture**: GPUI framework, SumTree-based rope, Tree-sitter integration, DirectX 11 on Windows, input-to-pixel latency claims. Key sources: `https://zed.dev/blog/zed-decoded-rope-sumtree`, `https://github.com/zed-industries/zed`, `https://github.com/zed-industries/zed/tree/main/crates/gpui`.
- [ ] 3.2. Search and read about **Lapce's architecture**: Floem (reactive UI), wgpu via vger/vello, xi-rope fork, WASI-based plugins. Key source: `https://github.com/lapce/lapce`, `https://github.com/lapce/floem`.
- [ ] 3.3. Search and read about **Helix's architecture**: terminal-native, tree-sitter, multiple selections, rope-based. Key source: `https://github.com/helix-editor/helix`.
- [ ] 3.4. Search and read about **wgpu** current API: `https://wgpu.rs`, `https://docs.rs/wgpu`, the `learn-wgpu` tutorial, recent changelog. Note the DirectX 12 / Vulkan / Metal backend story.
- [ ] 3.5. Search and read about **winit 0.30+** `ApplicationHandler` model: `https://docs.rs/winit`, its event loop changes from pre-0.30 callback model to the trait-based model.
- [ ] 3.6. Search and read about **glyphon** and **cosmic-text**: `https://github.com/grovesNL/glyphon`, `https://docs.rs/glyphon`, `https://github.com/pop-os/cosmic-text`. Understand the text-shaping / glyph atlas / draw-in-existing-render-pass pattern.
- [ ] 3.7. Search and read about **Ropey** and **crop**: `https://github.com/cessen/ropey`, `https://github.com/noib3/crop`. Understand the performance trade-offs between them and when to reach for a SumTree-style rope instead.
- [ ] 3.8. Search and read about **Tree-sitter** integration in Rust: `https://tree-sitter.github.io/tree-sitter/`, current best practices for incremental parsing in an editor. (We won't use Tree-sitter in the MVP, but document it now so later missions have a reference.)
- [ ] 3.9. Search and read about **LSP in Rust**: `tower-lsp`, `async-lsp`. Same note — not in MVP, but document for future.
- [ ] 3.10. Search and read about **IME** and **Unicode** handling on Windows/macOS/Linux: winit's `Ime` event, `Ime::Preedit` and `Ime::Commit`, Microsoft docs on IME, cosmic-text's Unicode Bidi support.

### 4. Write `/docs/ARCHITECTURE.md`

The canonical high-level architecture document. This is what every agent reads first.

- [ ] 4.1. Write a short "What this project is" section (3-4 paragraphs) summarizing the native-editor-as-real-time-engine philosophy.
- [ ] 4.2. Write a "System diagram" section. Use ASCII art or a Mermaid diagram (GitHub renders Mermaid natively). Show: OS event loop → winit → input pipeline → text engine → render engine → wgpu → GPU. Call out the main thread (input + coordination), the render thread (GPU submission), and the worker pool (file I/O, future indexing).
- [ ] 4.3. Write a "Crate layout" section listing the workspace crates we will create in M01:
  - `editor-core` — rope buffer, cursor, selection, undo/redo, pure logic, no I/O or UI.
  - `editor-input` — OS input events mapped to editor commands; IME handling.
  - `editor-render` — wgpu + glyphon + cosmic-text; owns the GPU pipeline.
  - `editor-io` — file load/save, memory mapping, atomic writes.
  - `editor-ui` — the minimal UI layer (gutter, status bar for V2).
  - `editor-app` — the top-level binary that wires everything together.
  Each crate's responsibilities and *what it must not do* should be documented crisply. No upward dependencies: `editor-core` knows nothing about GPU or OS.
- [ ] 4.4. Write a "Frame loop" section: input phase, state mutation phase, render phase. Reference the PRD's frame-based execution model.
- [ ] 4.5. Write a "Performance budget" section with the hard targets: <5 ms input-to-pixel, 60-120 fps during scroll, <1 s cold start, stable memory over long sessions.
- [ ] 4.6. Write a "What's explicitly out of scope" section naming the features we do not build in the MVP: syntax highlighting, LSP, AI, plugins, themes, tabs, project tree.
- [ ] 4.7. Commit: `docs(architecture): add ARCHITECTURE.md with system diagram, crate layout, frame loop, budgets`.

### 5. Write `/docs/TECH_STACK.md`

The detailed per-crate justification of our dependency choices.

- [ ] 5.1. For each major dependency, write: what it is, why we chose it over alternatives, what version we plan to pin, links to docs.rs, and an estimate of how central it is (load-bearing vs. replaceable).
- [ ] 5.2. Document the core stack:
  - `winit` (windowing & event loop) — pin to the current stable (0.30+).
  - `wgpu` (GPU rendering) — pin current stable; note backend selection strategy (DX12 on Windows, Metal on macOS, Vulkan on Linux; DX11 as a fallback for older Windows hardware per Zed's approach).
  - `glyphon` (text rendering on top of wgpu).
  - `cosmic-text` (text shaping and font handling, pulled in transitively via glyphon).
  - `ropey` (rope text buffer) — justify over `crop` and over a hand-rolled SumTree for MVP simplicity.
  - `thiserror` + `anyhow` (error handling — library vs. application).
  - `tracing` + `tracing-subscriber` (structured logging).
  - `memmap2` (memory-mapped files for large reads).
  - `arboard` (clipboard — cross-platform).
  - `criterion` (micro-benchmarks, dev-dependency).
  - `proptest` (property-based testing, dev-dependency).
  - `pollster` (dev-only future executor for examples that need a blocking wgpu init).
- [ ] 5.3. For each crate: note platform-specific features we need to enable or disable. E.g., some crates need the `default-features = false` + explicit feature list to keep binary size down.
- [ ] 5.4. Explicitly document the choices we are **not** making and why: no `egui` (we render text ourselves), no `iced` (same reason), no Electron (obviously), no Tauri (we don't need a webview), no `tokio` for the MVP (we use `std::thread` and a small worker pool; `tokio` is overkill and adds binary weight).
- [ ] 5.5. Commit: `docs(tech-stack): document dependency choices and justifications`.

### 6. Write `/docs/RENDERING_PIPELINE.md`

- [ ] 6.1. Write a clear explanation of our frame-based rendering approach: delta updates only, dirty-region tracking, visible-viewport-only rendering, glyph atlas on GPU.
- [ ] 6.2. Document the glyphon "middleware pattern": `prepare()` does CPU work (shaping, rasterization, atlas updates) once per frame; `render()` does the actual GPU draw inside the application's existing render pass. No extra render passes for text.
- [ ] 6.3. Document the swapchain strategy: `FifoRelaxed` present mode for VSync-locked low latency by default; configurable to `Immediate` for benchmarking. Triple buffering where available.
- [ ] 6.4. Document DPI handling: winit's `Window::scale_factor()`, how we apply it to glyphon's metrics, how we handle display changes mid-session.
- [ ] 6.5. Document the font loading story: system font enumeration (cosmic-text's `FontSystem` + `fontdb`), fallback chains for emoji and CJK, bundling a default monospace font in-repo so the editor always has something to render even if system fonts are unavailable.
- [ ] 6.6. Document the CPU-fallback path: if no suitable GPU backend is available, the editor must still run with a software fallback. (`wgpu` supports this via `wgpu::Backends::all()` and fallback adapters; we will not hand-roll a CPU rasterizer in the MVP.)
- [ ] 6.7. Commit: `docs(rendering): document pipeline, middleware pattern, DPI, and font loading`.

### 7. Write `/docs/TEXT_ENGINE.md`

- [ ] 7.1. Explain why a rope: not a Vec, not a gap buffer, not a piece table. Reference the PRD's scalability targets (multi-megabyte to multi-gigabyte files).
- [ ] 7.2. Compare Ropey and crop. Document our MVP choice (Ropey, because it's the most battle-tested in the ecosystem with ~56k weekly downloads per late-2025 data, has comprehensive UTF-16 length tracking, and handles all Unicode line break variants). Note that a SumTree-style implementation (as in Zed) is a possible future optimization but not needed now.
- [ ] 7.3. Document the cursor and selection primitive design: byte offsets as the canonical unit, with char/line/column accessors. Explain why byte offsets (matches Rust's `String` indexing; avoids UTF-16 code unit confusion).
- [ ] 7.4. Document the undo/redo strategy: operation-log based with inverse-operation pairs; snapshot coalescing for rapid typing; max undo depth configurable.
- [ ] 7.5. Document multi-thread sharing: Ropey ropes are cheap to clone (Arc-backed), so background tasks (file save, future Tree-sitter parsing) can take a snapshot without blocking the main thread.
- [ ] 7.6. Document line ending handling: store LF internally, detect the original on load (CRLF, CR, LF, mixed), and write the original style on save unless the user explicitly requests conversion.
- [ ] 7.7. Commit: `docs(text-engine): document rope choice, cursor primitives, undo/redo, line endings`.

### 8. Write `/docs/INPUT_AND_IME.md`

- [ ] 8.1. Document the winit 0.30+ `ApplicationHandler` trait model: required methods (`resumed`, `window_event`), the `ActiveEventLoop` handle, the recommended structure.
- [ ] 8.2. Document our direct-to-state input pipeline: no intermediate command queue, no async dispatch, raw OS events translate to text engine operations synchronously on the main thread within the same frame.
- [ ] 8.3. Document IME handling: `Ime::Enabled`, `Ime::Preedit`, `Ime::Commit`, `Ime::Disabled`. Cover the Chinese/Japanese/Korean composing-text flow with a worked example from the winit docs. Also cover dead-key sequences for European Latin-ish layouts.
- [ ] 8.4. Document `KeyEvent`, `PhysicalKey`, `logical_key`: when to use each, and the recommended pattern for mapping key combinations to editor commands. Warn against relying on `text` for control characters.
- [ ] 8.5. Document key repeat and key-combination handling, including the `SmolStr` that winit uses for the `text` field.
- [ ] 8.6. Document mouse and touchpad events we will care about for V2 (selection, scrolling).
- [ ] 8.7. Commit: `docs(input): document winit 0.30 input model, IME, key mapping, mouse events`.

### 9. Write `/docs/CROSS_PLATFORM.md`

- [ ] 9.1. Windows-specific considerations: console I/O and UTF-16, long-path support, `\` vs `/` path separators, `PathBuf`, trailing-backslash gotchas, DirectWrite vs FreeType for font shaping (cosmic-text uses its own pipeline so this is mostly internal), Microsoft Defender SmartScreen warnings on unsigned binaries, code signing (defer to M11).
- [ ] 9.2. macOS-specific considerations: Metal backend, `.app` bundle structure, notarization (defer to M11), `NSView` retina scaling, Cmd vs Ctrl for shortcuts (future V2+).
- [ ] 9.3. Linux-specific considerations: Wayland vs X11, fractional scaling, `XDG_CONFIG_HOME` for config files, `XDG_DATA_HOME` for user data, AppImage vs tarball distribution.
- [ ] 9.4. Line ending strategy across OSes (already covered in `TEXT_ENGINE.md` but re-link here).
- [ ] 9.5. Clipboard strategy: `arboard` cross-platform, but note the Windows / X11 / Wayland differences it abstracts over.
- [ ] 9.6. Filesystem edge cases: reserved Windows names (`CON`, `PRN`, `AUX`, `NUL`), case-sensitivity differences (macOS default is case-insensitive but case-preserving), max path length (Windows 260 by default, extended with `\\?\` prefix or registry manifest), symlink and reparse-point handling.
- [ ] 9.7. Commit: `docs(cross-platform): document Windows, macOS, and Linux differences and handling`.

### 10. Write `/docs/PERFORMANCE_BUDGETS.md`

- [ ] 10.1. Document the hard PRD budgets and the derived per-subsystem budgets. At 60 fps, we have 16.67 ms per frame. Allocate roughly: 2 ms for input processing, 4 ms for state mutation (text engine), 8 ms for rendering (layout + GPU submission), and 2 ms of headroom. At 120 fps, halve these. Document these numbers explicitly.
- [ ] 10.2. Document the benchmark strategy: Criterion benches for hot paths (rope insert, rope delete, line iteration, layout computation). CI must run benches on PRs and fail if the threshold regresses beyond 10%.
- [ ] 10.3. Document the profiling strategy: `tracing` spans for subsystem boundaries, optional Tracy integration (`tracing-tracy`) for visual frame profiling, `cargo flamegraph` for CPU hot spots, `heaptrack` or similar for memory.
- [ ] 10.4. Document the memory model: arena allocators for per-frame transient state, the rope's own allocation strategy (kilobyte-sized chunks), bounded undo history.
- [ ] 10.5. Document what triggers a redraw and what does not. Cursor blink doesn't need a full redraw. Typing does. Scrolling does. Resizing does. Focus changes might.
- [ ] 10.6. Commit: `docs(performance): document frame budgets, benchmark strategy, and profiling tools`.

### 11. Write `/docs/TESTING_STRATEGY.md`

- [ ] 11.1. Document the testing pyramid for this project: unit tests (every pure function in `editor-core`), integration tests (subsystem-to-subsystem), end-to-end tests (boot the app, inject synthetic input, snapshot the rendered output), benchmarks (Criterion), property-based tests (proptest for rope invariants).
- [ ] 11.2. Document snapshot testing for rendered output. We will not pixel-compare the glyph atlas (it's fragile); instead, snapshot the *layout data* (positions, widths, line wraps) and verify visually once per major mission.
- [ ] 11.3. Document the large-file stress test suite: fixtures for 1 MB, 10 MB, 100 MB, 1 GB files. 1 GB files should be gitignored or generated on demand; never commit them.
- [ ] 11.4. Document the long-session stability test: run the app, inject 1 million synthetic edits over an hour, check memory and frame time stability.
- [ ] 11.5. Document the CI test matrix: `cargo test` on Windows / Ubuntu / macOS at minimum. Add `cargo test --release` for release-mode sanity. Add `cargo clippy` and `cargo fmt --check`.
- [ ] 11.6. Commit: `docs(testing): document testing pyramid, stress tests, and CI matrix`.

### 12. Write `/docs/RUST_CONVENTIONS.md`

- [ ] 12.1. Style: `rustfmt` with default settings. `clippy` with `-D warnings` in CI. No `unsafe` outside of explicitly-marked modules (like FFI or hot-path optimizations), and any `unsafe` block must have a `// SAFETY:` comment explaining the invariant.
- [ ] 12.2. Error handling: `thiserror` for library-level error enums; `anyhow` only in the top-level application crate. Use `?` everywhere; panic only for logic bugs, never for expected error conditions.
- [ ] 12.3. Logging: use `tracing` macros (`trace!`, `debug!`, `info!`, `warn!`, `error!`). Wrap long-running operations in `tracing::instrument`. Never `println!` in production code paths.
- [ ] 12.4. Module organization: one major type per file where practical; `mod.rs` for module root, no `mod.rs` in a crate root (use `lib.rs` / `main.rs`).
- [ ] 12.5. Documentation: `///` for every public item. `//!` for every module. Doctests for anything non-trivial.
- [ ] 12.6. Naming: `snake_case` for functions and modules, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for consts. Avoid `get_` prefixes on getters; use bare nouns.
- [ ] 12.7. Dependency discipline: pin minor versions in `Cargo.toml` (`serde = "1.0"`, not `serde = "*"`). Avoid `default-features = true` if any feature pulls in weight we don't need. Every new dependency must be justified in the commit message.
- [ ] 12.8. Commit: `docs(conventions): document Rust style, error handling, logging, and module conventions`.

### 13. Write `/docs/GLOSSARY.md`

- [ ] 13.1. Define the domain vocabulary we'll use throughout the codebase: rope, chunk, cursor, caret, selection, range, anchor, head, viewport, gutter, line-break, grapheme cluster, atlas, glyph, shaping, layout, preedit, commit (IME), physical key vs logical key, scale factor, DPI, frame budget, dirty region, snapshot, undo stack, operation log.
- [ ] 13.2. Resolve aliases and enforce consistent terminology. Example: "caret" and "cursor" mean the same thing — pick one (cursor) and use it everywhere in code and docs.
- [ ] 13.3. Commit: `docs(glossary): add domain vocabulary reference`.

### 14. Write `/docs/STATUS.md`

- [ ] 14.1. A live project status document. At M00's end it says: "M00 complete. Docs folder established. M01 next."
- [ ] 14.2. Template for each future mission to update on completion.
- [ ] 14.3. Commit: `docs(status): add project status tracker`.

### 15. Write the root-level developer docs

- [ ] 15.1. Create `/CONTRIBUTING.md`: summarizes the standing orders for any future contributor, including the commit style, quality gates, and docs-read-first rule.
- [ ] 15.2. Create `/DEVELOPMENT.md`: exactly how to get the project running locally once M01 is done (placeholder, since no code exists yet). Write it as a checklist: install Rust via rustup, install platform prerequisites (e.g., `build-essential` on Linux, `Xcode` command line tools on macOS, Visual Studio Build Tools on Windows — specifically the "Desktop development with C++" workload because some crates link against system libraries), clone, build, run, test.
- [ ] 15.3. Create `/CHANGELOG.md`: a Keep-a-Changelog-style file with an `## [Unreleased]` section ready to receive entries.
- [ ] 15.4. Create `/FOLLOWUPS.md`: an empty list (with a header comment explaining its purpose — it's where agents drop design concerns they don't act on immediately but want to flag for later).
- [ ] 15.5. Commit: `docs(repo): add CONTRIBUTING, DEVELOPMENT, CHANGELOG, and FOLLOWUPS`.

### 16. Update the top-level README

- [ ] 16.1. Replace the placeholder README with the real one. Include: project name (refer to the repo name "IDE" unless the user has given a codename), one-paragraph pitch, "Getting Started" (pointing to `DEVELOPMENT.md`), a short architecture section that links to `docs/ARCHITECTURE.md`, a "Project Status" section that links to `docs/STATUS.md`, a "Contributing" section, a "License" section.
- [ ] 16.2. Keep the README tight — aim for under 80 lines of actual content. The detail goes in `docs/`.
- [ ] 16.3. Commit: `docs(readme): rewrite with architecture, status, and contribution pointers`.

### 17. Cross-link every doc

- [ ] 17.1. At the top of every file in `/docs`, add a breadcrumb: `[← docs/](./)` · Links to the README and other relevant docs. At the bottom, a "See also:" section linking to 2-3 other relevant documents.
- [ ] 17.2. Sanity-check every link: no broken file references, no dead external URLs (where possible — external URLs can rot; still prefer canonical docs like `docs.rs` over random blog posts).
- [ ] 17.3. Commit: `docs(cross-links): add breadcrumbs and see-also sections across /docs`.

### 18. Final review and push

- [ ] 18.1. Re-read every document you wrote. Fix typos, tighten prose, remove any hedging language.
- [ ] 18.2. Make sure `README.md`, `CONTRIBUTING.md`, `DEVELOPMENT.md`, and every `/docs` file render correctly in GitHub's Markdown viewer (check headings, code blocks, lists, and especially Mermaid diagrams if used).
- [ ] 18.3. Confirm the repo tree looks like this at the end of M00:
  ```
  IDE/
  ├── .gitignore
  ├── CHANGELOG.md
  ├── CONTRIBUTING.md
  ├── DEVELOPMENT.md
  ├── FOLLOWUPS.md
  ├── LICENSE
  ├── README.md
  ├── docs/
  │   ├── ARCHITECTURE.md
  │   ├── CROSS_PLATFORM.md
  │   ├── GLOSSARY.md
  │   ├── INPUT_AND_IME.md
  │   ├── PERFORMANCE_BUDGETS.md
  │   ├── RENDERING_PIPELINE.md
  │   ├── RUST_CONVENTIONS.md
  │   ├── STATUS.md
  │   ├── TECH_STACK.md
  │   ├── TESTING_STRATEGY.md
  │   └── TEXT_ENGINE.md
  └── reference/
      ├── 00_PRODUCT_REQUIREMENTS.md
      ├── 01_TECH_STACK.md
      ├── 02_ARCHITECTURE_STRATEGY.md
      ├── 03_GAPS_AND_RISKS.md
      ├── 04_MVP_DEFINITION.md
      ├── 05_V2_PRD.md
      └── README.md
  ```
- [ ] 18.4. Final push: everything to `origin main`.
- [ ] 18.5. Tag the commit: `git tag -a m00-complete -m "M00 complete: foundation research and documentation"` and `git push origin m00-complete`.

---

## Validation / Acceptance Criteria

This mission is complete when **all** of the following are true:

1. Every TODO item above is checked off and committed.
2. The repository at https://github.com/HiNala/IDE has a main branch with the directory structure from item 18.3.
3. Every file in `/docs` is a real, substantive document — not a stub, not a TODO list. Each is at least 200 lines of actual content.
4. The `README.md` clearly explains the project and links to `docs/`.
5. `CONTRIBUTING.md` and `DEVELOPMENT.md` give clear instructions to a new contributor (or agent) joining the project.
6. All six PRD documents exist verbatim in `/reference/` with a short index.
7. The `m00-complete` git tag has been pushed.
8. `docs/STATUS.md` marks M00 as complete and M01 as next.

## Testing Requirements

No automated tests for this mission (there is no code yet). Manual validation only:

- Open each `/docs` file in GitHub's web UI and confirm formatting, links, headings, and any diagrams render correctly.
- Confirm the breadcrumb and "See also:" sections at the top and bottom of each doc resolve correctly.
- Confirm `.gitignore` doesn't accidentally include anything in `docs/` or `reference/`.

## Git Commit Strategy

Aim for around 12-16 small commits spread across the mission. Bundle tightly-related sub-items into a single commit; do not make 60 micro-commits. Every commit should be scoped to one logical unit (e.g., "docs: add architecture" is one commit; "docs: add tech stack" is another).

Push at least after items 1, 4, 7, 10, 13, 15, 17, and 18 (so anyone watching the repo sees steady incremental progress, not one giant dump).

## Handoff To M01

M01 picks up assuming:

- The repo exists and has the structure from 18.3.
- `/docs` contains detailed references for every decision M01 needs to make.
- `/reference` contains the PRDs.
- `CONTRIBUTING.md` is the first thing future agents read.
- `docs/STATUS.md` reflects "M00 done, M01 next."

M01's first step will be to read `/docs/TECH_STACK.md`, `/docs/ARCHITECTURE.md`, and `/docs/RUST_CONVENTIONS.md` and then generate the Cargo workspace.

---

## Standing Orders Reminder

- Do not stop until every TODO is complete.
- Do not commit broken links, broken markdown, or unfinished docs.
- Push regularly; do not leave 20 commits unpushed.
- If you discover something the PRDs missed that you think is important, add it to `/FOLLOWUPS.md` — do not silently absorb it into a doc as if it were spec.
- When in doubt, cite the source. Every claim about "what wgpu does" or "what winit does" should be traceable to a specific URL you read during research.

Go.
