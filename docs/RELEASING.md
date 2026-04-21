[← docs/](./) · [README](../README.md)

# Releasing the IDE

Playbook for maintainers: **local builds**, **tag-driven GitHub Releases** (MSI, DMG, `.deb`, AppImage), **optional signing**, and **checksum verification**.

## Happy path (≈10 minutes if CI is green)

1. Bump the workspace version in [`Cargo.toml`](../Cargo.toml) `[workspace.package].version` (numeric semver only — **WiX rejects non-numeric prerelease strings** like `-mvp`).
2. Update [`CHANGELOG.md`](../CHANGELOG.md) with a dated section for the new version.
3. On a clean branch: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`.
4. Commit and push.
5. Tag an annotated release: `git tag -a vX.Y.Z -m "Release vX.Y.Z"` and `git push origin vX.Y.Z`.
   - Tags must match **`v*.*.*`** (three numeric segments, e.g. `v0.1.0`) to run [`.github/workflows/release.yml`](../.github/workflows/release.yml).
6. Watch the **Release** workflow. When it finishes, open the GitHub Release: you should see MSI, DMG, `.deb`, AppImage, `SHA256SUMS.txt`, and optionally `SHA256SUMS.txt.asc`.

## Prerequisites (developers)

- Rust stable matching [`Cargo.toml`](../Cargo.toml) `rust-version`.
- GPU drivers where applicable; Windows 10+, recent macOS, or Linux with X11/Wayland + Vulkan-capable stack for CI parity.

### Maintainer-only (optional)

| Capability | What you need |
|------------|----------------|
| Windows MSI signing | Code signing cert (PFX + password); optional EV for better SmartScreen reputation. |
| macOS notarization | Apple Developer Program; Developer ID Application cert; App-Specific Password; `notarytool` keychain profile. |
| Checksum GPG signing | A GPG key suitable for release signing, imported in CI via secret. |

## GitHub secrets (reference)

Set in **Repository → Settings → Secrets and variables → Actions**. All are optional unless you want that behavior.

| Secret | Purpose |
|--------|---------|
| `WINDOWS_CERT_PFX_BASE64` | Base64-encoded signing certificate for optional MSI signing after build. |
| `WINDOWS_CERT_PASSWORD` | Password for the PFX. |
| `MACOS_CODESIGN_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)`. If unset, DMG is **unsigned** (Gatekeeper may prompt). |
| `RELEASE_GPG_PRIVATE_KEY` | Armored private key; if set, `SHA256SUMS.txt.asc` is produced. |

For full notarization on macOS, the runner also needs a `notarytool` keychain profile (e.g. `ide-notary`) created on a machine where you can run `xcrun notarytool store-credentials`. CI workflows that import a `.p12` and call `notarytool` are documented in `docs/missions/M11_RELEASE_ENGINEERING_PACKAGING.md`; the current repo script [`scripts/macos-package.sh`](../scripts/macos-package.sh) signs and notarizes **when** `MACOS_CODESIGN_IDENTITY` is set and the profile exists locally — **automating** import in Actions is an optional follow-up.

## Artifacts produced by CI

| OS | Files |
|----|--------|
| Windows | `ide-vX.Y.Z-windows-x86_64.msi` |
| macOS | `ide-vX.Y.Z-macos-aarch64.dmg` or `…-macos-x86_64.dmg` (matches the GitHub runner CPU) |
| Linux | `ide-vX.Y.Z-linux-x86_64.deb`, `ide-vX.Y.Z-linux-x86_64.AppImage` |
| All | `SHA256SUMS.txt` (+ optional `.asc`) |

**Unsigned vs signed:** Without certs, SmartScreen / Gatekeeper may warn; users can use “More info” / “Open” after verifying checksums.

## Local packaging (without CI)

- **Windows MSI:** Install [WiX Toolset 3.11+](https://wixtoolset.org/) and `cargo install cargo-wix`, then from repo root: `cargo wix -p editor-app --output target/wix/ide-dev.msi`.
- **macOS `.app`:** `cargo install cargo-bundle`, then `cargo bundle --release -p editor-app --format osx`, then `bash scripts/macos-package.sh target/release/ide.dmg`.
- **Linux `.deb`:** `cargo bundle --release -p editor-app --format deb` → look under `target/release/bundle/deb/`.
- **Linux AppImage:** Build release binary first, ensure `appimagetool` is on `PATH`, then `bash scripts/linux-appimage.sh target/release/ide-custom.AppImage`.

## Verifying downloads

```bash
# If GPG signature is published:
gpg --verify SHA256SUMS.txt.asc SHA256SUMS.txt

sha256sum -c SHA256SUMS.txt
```

## If a release fails

- Re-run failed jobs from the Actions UI after fixing the cause (missing tool, WiX path, notary timeout, etc.).
- For a bad tag: delete the GitHub Release and the remote tag, fix the issue, then tag `vX.Y.Z+1` or a new patch.

## Known install-time issues

- **Windows:** Unsigned MSI → SmartScreen “Windows protected your PC” until reputation builds or the package is signed.
- **macOS:** Unsigned app → right-click → **Open** the first time, or `xattr -cr /path/to/IDE.app` after copy.
- **Linux AppImage:** FUSE (`libfuse2` / FUSE3) may be required on some distributions; see [AppImage documentation](https://docs.appimage.org/).

## Current automated release trigger

Pushing a tag matching **`v*.*.*`** runs the workflow above. Prerelease tags containing `-` (e.g. `v0.2.1-rc1`) are published with **pre-release** checked on GitHub.

---

*Last updated: 2026-04-20 — aligns with M11 (release engineering & packaging).*
