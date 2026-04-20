# Local development

## 1. Install Rust

Install [rustup](https://rustup.rs/) and use the toolchain pinned in
`rust-toolchain.toml` (run any `cargo` command once; rustup will fetch it).

```powershell
rustc --version
cargo --version
```

## 2. Platform prerequisites

### Windows

- Install **Visual Studio Build Tools** with the **Desktop development with C++**
  workload (MSVC linker required by some native crates).
- Optional: enable **long paths** at the OS level for repositories in deep trees
  (the `editor-app` manifest also requests long-path awareness).

### Linux (Debian/Ubuntu-style)

Typical packages for `winit` + `wgpu` development:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config \
  libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  libwayland-dev libxkbcommon-dev \
  libvulkan-dev mesa-vulkan-drivers libegl1-mesa-dev
```

### macOS

Install **Xcode Command Line Tools**:

```bash
xcode-select --install
```

## 3. Clone and build

```bash
git clone https://github.com/HiNala/IDE.git
cd IDE
cargo build --workspace
```

## 4. Run the app

```bash
cargo run --release --bin editor-app
```

You should see a dark window; close it to exit. Headless GPU smoke:

```bash
cargo run --release --bin editor-app -- --dry-run
```

## 5. Tests and checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
```

## 6. Supply chain

Install [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) and run:

```bash
cargo install cargo-deny --locked
cargo deny check
```

## 7. Optional: git hooks

```bash
git config core.hooksPath .githooks
```

## 8. CI parity

GitHub Actions runs the same `fmt`, `clippy`, `test`, and `release` build steps
on Windows, Ubuntu, and macOS. Linux CI does **not** launch a GUI window; it
builds the binary and runs `--dry-run` / unit tests only.
