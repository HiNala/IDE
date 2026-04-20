# MVP Definition

This document is the **contract** for what counts as a finished MVP. It is
deliberately narrow, measurable, and non-negotiable.

## 1. Purpose

The MVP validates that a performance-first, native-code architecture can
deliver a meaningfully better editing experience than VS Code and Cursor.
It is a controlled experiment in system design, not a product launch.

## 2. What the MVP Is

A single-process, local-first, native text editor built in Rust that:

- Is a single executable binary per OS.
- Opens a window via `winit`, renders via `wgpu`, and exists entirely
  outside any browser runtime.
- Opens, edits, and saves a plain text file.
- Scales to 100 MB+ files without UI stalls.
- Adapts its concurrency and rendering strategy to the host hardware.
- Boots in under one second.
- Stays responsive through multi-hour editing sessions.

## 3. What the MVP Is Not

The MVP is **not** any of these:

- A syntax-highlighting editor.
- An autocomplete, linter, or debugger integration.
- A terminal, project explorer, or workspace manager.
- A multi-file, tabbed editor.
- A plugin host.
- A collaborative editor or cloud-synced tool.
- An AI-assisted coding assistant.
- A configurable theming engine.

Any temptation to add these features before M11 must be redirected into a
`FOLLOWUPS.md` entry and deferred.

## 4. Functional Capabilities (MVP)

- **File open** — from a path argument (CLI) or `Ctrl+O` shortcut in M09;
  for MVP the CLI path is sufficient.
- **File view** — rope-backed, wrap disabled by default.
- **Text editing** — type, backspace, delete, enter. Single primary cursor.
- **Cursor navigation** — arrow keys, page up / down, home, end.
- **Scrolling** — smooth, viewport-aware.
- **File save** — atomic save via temp + fsync + rename.
- **Clean exit** — closing the window drops all GPU resources and
  outstanding I/O cleanly.

Everything in this list must work on Windows, Linux, and macOS.

## 5. Non-Functional Requirements (MVP)

| # | Requirement | Target |
|---|---|---|
| NF-01 | Input-to-pixel latency | < 5 ms p99 under normal load |
| NF-02 | Frame rate during scroll | ≥ 60 fps on Windows primary dev box |
| NF-03 | Frame rate during edit | ≥ 60 fps on Windows primary dev box |
| NF-04 | Cold start | < 1 s on modern hardware |
| NF-05 | Open 100 MB file | UI stays interactive, no dropped frames |
| NF-06 | Multi-hour memory | RSS growth < 10 % over 4 hours of edits |
| NF-07 | Data safety | zero lost edits across crash injection tests |
| NF-08 | Crash-free operation | zero panics in acceptance run |

Measurement methodology is defined in `docs/PERFORMANCE_MODEL.md` and
`docs/TESTING.md`.

## 6. Definition of Completion

The MVP is complete when **all** of these hold:

1. Every functional capability in §4 works on all three OSes.
2. Every non-functional requirement in §5 passes its M08 acceptance
   measurement.
3. `cargo fmt`, `cargo clippy -D warnings`, `cargo test --all`,
   `cargo build --release`, and `cargo bench` all succeed on Windows,
   Linux, and macOS in CI.
4. `docs/STATUS.md` is updated with measured performance numbers and
   marked MVP-complete.
5. Tag `m08-complete` is pushed.

## 7. Testing Strategy Highlights

- **Unit tests** on rope, cursor, history, path normalization.
- **Integration tests** on file load → rope → render.
- **Property tests** with `proptest` on edit round-trip invariants.
- **Criterion benchmarks** on hot paths (edit, layout, atlas lookup).
- **Stress tests** (M08) at 100 MB / 500 MB / 1 GB.
- **Soak test** for multi-hour memory stability.
- **Crash-injection tests** for atomic save correctness.

See `docs/TESTING.md`.

## 8. Acceptance Criteria

Acceptance is measured, not felt.

| # | Criterion | How Measured |
|---|---|---|
| AC-01 | Open 100 MB file in < 500 ms (wall clock) | `editor-io` benchmark + manual timing |
| AC-02 | p50 keystroke latency < 3 ms, p99 < 5 ms | Criterion harness with injected events |
| AC-03 | Scroll 100 MB file at 60 fps minimum | Frame-time trace during scripted scroll |
| AC-04 | Cold start < 1 s, binary on SSD | External wall-clock timer |
| AC-05 | Memory bounded over 4-hour edit macro | `tracing` RSS samples every 60 s |
| AC-06 | Save survives mid-write kill -9 | Crash-injection test |
| AC-07 | No panics in acceptance run | CI-captured stderr |
| AC-08 | CI green on all three OSes | GitHub Actions badge |

All AC entries must be PASS in `docs/STATUS.md` for the MVP to ship.

## 9. Definition of "Working"

A working MVP is one that delivers a seamless and responsive editing
experience under all tested conditions and that beats VS Code and Cursor in
measurable, reproducible side-by-side benchmarks on the same machine.

If the numbers are there, the MVP is done. If the numbers are not there,
the MVP is not done, regardless of feature checklist completion.

## 10. Post-MVP Readiness

Once the MVP ships, the architecture must be stable and well understood
enough to add V2 features without reshaping the core. The release of a
tagged `m08-complete` starts the V2 track (M09–M10) — not syntax
highlighting, not LSP, not plugins.

---

*Last updated: M00.*
