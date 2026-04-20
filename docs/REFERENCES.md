[← docs/](./) · [README](../README.md)

# External References & Prior Art

Sources and prior-art projects that inform this design. Use these as the
first stop when a technical question is non-obvious.

## 1. Prior-Art Editors (Study These)

### Zed — <https://github.com/zed-industries/zed>
The closest thing to what we are building. Rust, GPU-accelerated, native.

- Their frame loop is a useful reference even though their architecture
  is larger than we need.
- Their rope (`Buffer`) is more than ropey; we explicitly start with
  ropey and only reconsider if profiling demands it.
- License: GPL / AGPL; we do not copy code. Read, learn, reimplement.

### Lapce — <https://github.com/lapce/lapce>
Rust + wgpu + Floem (their own reactive UI). Less tightly performance-
engineered than Zed but an excellent community-sized reference for
wgpu / winit plumbing.

### Helix — <https://github.com/helix-editor/helix>
Rust, terminal-based. Uses ropey with tree-sitter. Its text-engine code
is a great reference for grapheme-aware cursor math.

### Xi-editor (archived) — <https://github.com/xi-editor/xi-editor>
Historically important rope + CRDT design. Research-grade but retired.
Useful reading for CRDT ideas if we ever add collaboration.

### Neovim core — <https://github.com/neovim/neovim>
Not Rust, but their buffer/undo/event-loop architecture influenced an
entire generation of editors.

### Sublime Text — <https://www.sublimehq.com/>
Closed-source, native, legendarily fast. Its memory model (rope +
instant feel) is the bar we are chasing.

## 2. Rust Crates

### Windowing & Graphics

- [`winit`](https://docs.rs/winit) — cross-platform windowing.
  [`ApplicationHandler` trait](https://docs.rs/winit/latest/winit/application/trait.ApplicationHandler.html).
- [`wgpu`](https://docs.rs/wgpu) — GPU abstraction. Start at
  [`wgpu::Instance`](https://docs.rs/wgpu/latest/wgpu/struct.Instance.html)
  and the [learn-wgpu book](https://sotrh.github.io/learn-wgpu/).
- [`raw-window-handle`](https://docs.rs/raw-window-handle) — connects
  winit and wgpu.
- [`pollster`](https://docs.rs/pollster) — tiny blocking executor for
  startup `await`s.

### Text

- [`ropey`](https://docs.rs/ropey) — rope.
- [`cosmic-text`](https://docs.rs/cosmic-text) — shaping + layout.
- [`glyphon`](https://docs.rs/glyphon) — wgpu + cosmic-text renderer.
- [`unicode-segmentation`](https://docs.rs/unicode-segmentation) —
  grapheme clusters.
- [`unicode-width`](https://docs.rs/unicode-width) — visual column
  width.
- [`encoding_rs`](https://docs.rs/encoding_rs) — Unicode-compliant
  encoding decoders.

### Concurrency & Runtime

- [`tokio`](https://docs.rs/tokio) — async runtime.
- [`crossbeam-channel`](https://docs.rs/crossbeam-channel) — MPMC
  channels.
- [`arc-swap`](https://docs.rs/arc-swap) — lock-free snapshot.
- [`parking_lot`](https://docs.rs/parking_lot) — faster
  `Mutex`/`RwLock`.

### File I/O & Data

- [`memmap2`](https://docs.rs/memmap2) — memory-mapped files.
- [`tempfile`](https://docs.rs/tempfile) — atomic save.
- [`directories`](https://docs.rs/directories) — per-OS config dirs.

### Observability

- [`tracing`](https://docs.rs/tracing) — structured logs + spans.
- [`tracing-subscriber`](https://docs.rs/tracing-subscriber) — default
  formatter.
- [`tracing-chrome`](https://docs.rs/tracing-chrome) — Chrome Trace
  exporter.

### Errors

- [`thiserror`](https://docs.rs/thiserror) — derive for library errors.
- [`anyhow`](https://docs.rs/anyhow) — ergonomic errors for binary
  crates.

### Testing / Benchmarking

- [`criterion`](https://docs.rs/criterion) — benchmarks.
- [`proptest`](https://docs.rs/proptest) — property tests.
- [`insta`](https://docs.rs/insta) — snapshot tests.
- [`loom`](https://docs.rs/loom) — concurrency model checker.
- [`tokio-test`](https://docs.rs/tokio-test) — async test utilities.

### V2+ (Reference Only)

- [`arboard`](https://docs.rs/arboard) — cross-platform clipboard.
- [`notify`](https://docs.rs/notify) — file-system watcher.

## 3. Articles & Talks

- **"A Text Editor Architecture"**, Nicolas Silva (cosmic-text author).
- **"1000 Frames Per Second: rendering text at 120 Hz in Zed"**,
  Zed blog. Highly relevant to our rendering budgets.
- **"Ropes, an Alternative to Strings"**, Boehm/Atkinson/Plass (1995).
  The original paper; still the best intro.
- **"Xi-editor: a Modern Editor with a Background"**, Raph Levien.
- **"Performance in a Text Editor"**, GregoryFine (Helix blog).
- **Rust `wgpu` book** — <https://sotrh.github.io/learn-wgpu/>.

## 4. Specifications & Standards

- **Unicode Standard** — <https://www.unicode.org/versions/latest/>.
  Grapheme cluster boundaries (UAX #29), line breaking (UAX #14), and
  character database.
- **WHATWG Encoding Standard** — <https://encoding.spec.whatwg.org/>.
  The reference `encoding_rs` implements.
- **POSIX rename(2)** — atomicity guarantees on POSIX filesystems.
- **Windows `ReplaceFileW`** — MSDN; underlies atomic rename on
  Windows.

## 5. Benchmarks & Comparisons (Competition)

- VS Code input latency measurements — various blog posts and
  Microsoft-internal presentations. Our measurement methodology
  mirrors theirs so results are comparable (`PERFORMANCE_BUDGETS.md`).
- Cursor startup and input latency — community-measured.

## 6. Windows-Specific References

- **Per-Monitor DPI Awareness V2** — MSDN.
- **DX12 on Windows** — the primary backend we target.
- **`ReplaceFileW` vs. `MoveFileExW`** — for atomic rename on NTFS /
  ReFS.

## 7. Linux-Specific References

- **Wayland** — `wayland-protocols`, `libxkbcommon`.
- **Vulkan loader** — `libvulkan.so.1`.
- **FreeDesktop `$XDG_CONFIG_HOME`** convention; `directories` handles
  it.

## 8. macOS-Specific References

- **Metal programming guide** — Apple Developer.
- **Notarization** — `xcrun notarytool` docs.
- **`NSPasteboard`** — clipboard integration (handled by `arboard`).

## 9. Build & Release Tooling

- [`cargo-wix`](https://crates.io/crates/cargo-wix) — MSI generator.
- [`cargo-deb`](https://crates.io/crates/cargo-deb) — Debian packages.
- [`cargo-generate-rpm`](https://crates.io/crates/cargo-generate-rpm) — RPM packages.
- [`cross`](https://github.com/cross-rs/cross) — Docker-based cross
  compile.
- [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) —
  supply-chain audit.
- [`cargo-nextest`](https://nexte.st/) — test runner.

---

*Last updated: M00.*
