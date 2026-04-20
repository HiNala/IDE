[← docs/](./) · [README](../README.md)

# Releasing the IDE

This document will be expanded in **M11** (release engineering) with MSI, `.dmg`, `.deb`, AppImage, and optional code-signing steps. Until then, releases are **from source**.

## Current process (developer / power user)

1. **Requirements:** Rust stable (see [`Cargo.toml`](../Cargo.toml) `rust-version`), GPU drivers, Windows 10+ / recent Linux / macOS.

2. **Build** (from repo root):

   ```text
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace
   cargo build --release
   ```

3. **Binary:** `target/release/editor-app.exe` (Windows) or `target/release/editor-app` (Unix).

4. **Smoke test:** Run the binary without arguments; window should open. `editor-app --dry-run` should exit 0 (GPU init without a window).

5. **Versioning:** Workspace version is `0.1.0-mvp` until MVP/V2 acceptance tags are decided (see [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md)).

## Automated releases (partial M11)

Pushing a **git tag** matching `v*` (for example `v0.2.0`) runs [`.github/workflows/release.yml`](../.github/workflows/release.yml). It builds `editor-app` in `--release` on Windows, Linux, and macOS, computes `SHA256SUMS.txt`, and creates a **GitHub Release** with unsigned binaries attached.

- **Unsigned:** SmartScreen / Gatekeeper may warn on first run; installers (MSI, dmg, deb, AppImage) and code signing are tracked as follow-ups in `FOLLOWUPS.md` / M11 mission doc.
- **Prerequisite:** maintainers need permission to create releases on the repo.

## Future (full M11)

- WiX MSI, macOS `.dmg` + notarization, Linux `.deb` / AppImage (see `docs/missions/M11_RELEASE_ENGINEERING_PACKAGING.md`).
- GPG-signed checksums optional.

*Last updated: 2026-04-20.*
