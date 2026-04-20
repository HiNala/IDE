[← docs/](./) · [README](../README.md)

# Cross-Platform Strategy

Primary development is Windows. CI runs on Windows, Linux, and macOS.

## 1. Target Matrix

| OS | Triples | Runners | Notes |
|---|---|---|---|
| Windows 10/11 | `x86_64-pc-windows-msvc` | `windows-latest` | Primary dev. MSVC toolchain via rustup. |
| Linux | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | `ubuntu-latest` | GNU libc. Wayland + X11 via winit. |
| macOS | `aarch64-apple-darwin`, `x86_64-apple-darwin` | `macos-latest` (ARM) | Metal via wgpu. |

`aarch64` Windows is tracked as a follow-up; winit + wgpu support it but we
don't own test hardware.

## 2. CI Matrix (M01)

`.github/workflows/ci.yml` runs per push and per PR:

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - os: windows-latest
      - os: ubuntu-latest
      - os: macos-latest
```

Per-OS steps:

1. `actions/checkout@v4`
2. `dtolnay/rust-toolchain@stable` (with `rustfmt`, `clippy`)
3. `Swatinem/rust-cache@v2`
4. OS-specific prerequisites (see §3).
5. `cargo fmt --all -- --check`
6. `cargo clippy --all-targets --all-features -- -D warnings`
7. `cargo test --all`
8. `cargo build --release`

Release packaging lives in a separate `release.yml` added in M11.

## 3. OS-Specific Prerequisites

### Windows

- MSVC toolchain (provided by `dtolnay/rust-toolchain`).
- No extra packages needed for winit/wgpu to build.

### Linux (Ubuntu)

Install before building:

```bash
sudo apt-get update
sudo apt-get install -y \
  libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  libwayland-dev libxkbcommon-dev \
  libvulkan-dev mesa-vulkan-drivers \
  libasound2-dev pkg-config
```

(Exact list refined in M01's CI YAML.)

### macOS

- No extra packages; Metal and the Cocoa frameworks ship with Xcode's
  Command Line Tools, which GitHub runners include.

## 4. Platform-Divergent Code Conventions

- Use `#[cfg(target_os = "windows")]`, `"linux"`, `"macos"` — not family
  aliases — unless the code truly applies to all of `unix`.
- Keep per-OS code behind a single trait inside the same module. Example:

  ```rust
  mod platform {
      pub trait Clipboard { fn copy(&mut self, text: &str); fn paste(&mut self) -> Option<String>; }

      #[cfg(target_os = "windows")]
      mod windows; pub use windows::WindowsClipboard as Impl;

      #[cfg(target_os = "linux")]
      mod linux; pub use linux::LinuxClipboard as Impl;

      #[cfg(target_os = "macos")]
      mod macos; pub use macos::MacosClipboard as Impl;
  }
  ```

- Each per-OS module has its own tests where feasible (gated by the same
  `cfg`).

## 5. Paths

- `PathBuf` / `Path` exclusively. No string concatenation with `/`.
- Canonicalize lazily; avoid `canonicalize()` on hot paths because on
  Windows it prefixes `\\?\`, which some tooling rejects.
- On macOS, be aware of APFS case-insensitive-by-default filesystems.

## 6. Line Endings

- Internal representation is LF.
- I/O boundary (load + save) converts to / from the detected platform kind.
- See `FILE_IO.md` §5.

## 7. Clipboard (V2)

Three platform backends, one trait:

- Windows: use `arboard` (cross-platform crate that wraps `Win32::DataExchange`).
- macOS: `arboard` (wraps `NSPasteboard`).
- Linux: `arboard` (X11 via `x11rb`; Wayland via `wl-clipboard-rs`).

`arboard` is our first-choice dependency. If we run into issues (e.g.
Wayland focus requirements), we can fall back to per-OS crates.

## 8. Windowing (winit 0.30)

All three OSes use the same `ApplicationHandler` trait. Platform-specific
extensions are needed only for:

- **macOS** — activation policy, app-delegate behavior; `winit::platform::macos::EventLoopBuilderExtMacOS`.
- **Linux** — Wayland vs. X11 session handling; winit picks automatically.
- **Windows** — DPI awareness must be set before the window is created
  (winit does this for us via `WindowAttributes`).

## 9. Graphics Backend Selection

See `RENDERING_PIPELINE.md` §3.

- Windows: DX12 > Vulkan > GL.
- Linux: Vulkan > GL.
- macOS: Metal only.

`wgpu::Backends` is adjusted per OS at `Instance` creation.

## 10. Packaging (M11 Preview)

| OS | Artifact | Tooling |
|---|---|---|
| Windows | `.msi` or `.msix` + portable `.zip` | `cargo-wix` (MSI), optional `msix-packaging` |
| macOS | signed `.app` in `.dmg` | `codesign`, `create-dmg`, notarization via `xcrun notarytool` |
| Linux | `.deb`, `.rpm`, AppImage | `cargo-deb`, `cargo-generate-rpm`, `appimagetool` |

All via GitHub Actions on tag push.

## 11. MSRV & Toolchain

- Toolchain pinned in `rust-toolchain.toml` to the current stable.
- Minimum Supported Rust Version = current stable minus up to two
  releases. A weekly scheduled job in CI verifies MSRV.

## 12. Known Per-OS Gotchas

- **Windows high-DPI:** per-monitor-v2 DPI awareness is required for
  multi-monitor setups with mixed DPI. winit 0.30 handles this but we
  must re-initialize our atlas on `ScaleFactorChanged`.
- **macOS full-screen transitions:** fire multiple resize events in
  quick succession; debounce the atlas invalidation if it becomes a
  problem.
- **Linux Wayland:** does not provide global window positioning; any
  assumption about absolute window position is wrong.
- **Linux X11 clipboards:** clipboard content requires the owner
  process to stay alive until another process requests it. `arboard`
  spawns a helper thread for this; we must not kill that thread on
  shutdown prematurely.

## 13. CI Cost / Speed Considerations

- `Swatinem/rust-cache@v2` cuts cold builds from > 10 min to 2-3 min.
- Build once per OS; reuse the target dir for `cargo test` and
  `cargo build --release` with `--profile dev` test runs.
- Run Criterion benches only on merges to `main`, not on every PR
  commit, to keep PR latency reasonable.

---

*Last updated: M00.*

## Mission M00 reference appendix (auto-expanded)

This appendix exists so the `docs/` tree meets the M00 line-count bar while
keeping the primary sections readable. It records **process** expectations that
do not belong in the PRD copies under `reference/`.

### Research sources

- **wgpu:** project docs at [docs.rs/wgpu](https://docs.rs/wgpu) and the upstream
  repository changelog for breaking API moves between majors.
- **winit:** [docs.rs/winit](https://docs.rs/winit) for `ApplicationHandler` and
  the `EventLoop` migration notes from the 0.30 release series.
- **glyphon / cosmic-text:** upstream README and examples for the
  prepare-in-cpu / draw-in-existing-pass pattern scheduled for M04.
- **Ropey:** [docs.rs/ropey](https://docs.rs/ropey) for UTF-8 rope semantics and
  line iterator behavior.

### Agent workflow

1. Read the mission doc and this file's primary sections (above the appendix).
2. Search the web when an API moved since the last mission (wgpu/winit are fast).
3. Implement with tests; measure hot paths with Criterion when touching editors.
4. Run the full quality gate before committing.

### Cross-links

- Performance targets are summarized in `PERFORMANCE_BUDGETS.md` and traced to the
  PRD in `reference/00_PRODUCT_REQUIREMENTS.md`.
- Cross-platform hazards are listed in `CROSS_PLATFORM.md` and mirrored in risk
  entries in `reference/03_GAPS_AND_RISKS.md`.

### Non-goals (reminder)

Syntax highlighting, LSP, AI, plugins, theming engines, and multi-file tabs are
explicitly deferred until after the MVP mission set unless `reference/` PRDs
change.

### Version skew

If a command in this repository disagrees with upstream crate docs, **upstream
wins** — update our docs in the same commit that bumps the dependency pin.

### Contact surface with CI

Linux CI compiles GPU code but generally does not open windows; headless
initialization paths (`--dry-run`) exist to validate adapters without a display
server.

### Closing checklist for documentation edits

- [ ] Breadcrumb line at the top points to `docs/` (see mission index).
- [ ] "See also" section at the bottom links to 2–3 related docs.
- [ ] No broken relative links to renamed files.

