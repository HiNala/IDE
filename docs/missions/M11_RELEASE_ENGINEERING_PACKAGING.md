# M11 — Release Engineering & Cross-Platform Packaging

**Mission ID:** M11
**Prerequisites:** M10 complete. `0.2.0-v2` tag pushed. V2 acceptance report green.
**Output:** The final mission. Distributable installers for Windows (MSI), macOS (signed + notarized .app in a .dmg), and Linux (AppImage + .deb). Code-signing infrastructure for Windows (SignTool) and macOS (hardened runtime + notarization + stapling). A GitHub Actions release workflow that builds, signs, packages, and publishes every `v*.*.*` tag to GitHub Releases with checksums. Binary size discipline: under 50 MB per platform. A release-candidate checklist and a written release-process document so future releases are reproducible by someone who has never shipped one.
**Estimated scope:** 2-3 sessions. Most of the time is CI iteration and signing-certificate paperwork, not Rust code.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/docs/CROSS_PLATFORM.md` — the per-OS install locations we target.
- `/docs/ARCHITECTURE.md` — understand what's in the binary (so you can explain its size).
- `/reference/05_V2_PRD.md` §11 — V2 includes "distributable binary" as a shipping requirement.
- `https://github.com/volks73/cargo-wix` — Windows MSI generation via WiX Toolset.
- `https://github.com/burtonageo/cargo-bundle` — macOS `.app` and Linux `.deb` bundling.
- `https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution` — Apple's notarization docs. Hardened runtime, `notarytool submit`, `stapler staple`.
- `https://github.com/softprops/action-gh-release` — publish to GitHub Releases from Actions; currently v3 is the supported major.
- `https://nnethercote.github.io/perf-book/build-configuration.html` — `[profile.release]` tuning for binary size.

---

## The Situation In Plain English

We have a working editor. It runs from a `cargo run --release` command on the developer's machine. M11 closes the gap between "runs for me" and "someone else can install it on their computer." That means three separate things have to happen for each of our three supported operating systems: a build that produces the right binary format, a signing step that lets the OS trust the binary and not scare the user, and a packaging step that wraps the binary in the expected installer format for that platform. Then everything has to run automatically from a GitHub Actions workflow triggered by a git tag.

Each OS has its own set of landmines. On Windows the binary is a plain `.exe` but shipping it bare produces a SmartScreen warning; we want a properly signed MSI built by `cargo-wix` using the WiX Toolset. On macOS the binary has to live inside a `.app` bundle with an `Info.plist`; the bundle has to be signed with a Developer ID Application certificate using `codesign --options runtime` (hardened runtime is required for notarization); the signed bundle has to be zipped in a specific way (`ditto -c -k --keepParent`, NOT `zip`, because `zip` strips metadata that notarization requires); submitted to Apple via `xcrun notarytool submit --wait`; the resulting ticket stapled to the bundle with `xcrun stapler staple`; and finally the bundle tucked into a `.dmg`. On Linux there is no signing equivalent and no single blessed package format, so we produce both an AppImage (works anywhere) and a `.deb` (for Debian/Ubuntu users who want proper system integration).

None of this requires Rust code changes inside the editor itself. M11 is almost entirely configuration, scripting, and CI wiring. But it's the mission where the project crosses from "private codebase" to "public software." After M11, a developer anywhere in the world can go to the Releases page on GitHub, download the installer for their OS, double-click it, and be editing text in under a minute.

Code signing is the rabbit hole. Windows code signing certificates from commercial CAs cost money (typically $100-500/year) and require organizational identity verification. Apple Developer Program membership is $99/year and requires a real identity. For the first release we document the signed path but also document the *unsigned* path so this mission can complete without blocking on certificate purchases. Unsigned Windows binaries trigger SmartScreen; unsigned macOS binaries trigger Gatekeeper. We document both, commit the workflow that handles both, and leave the signing credentials as CI secrets that the maintainer sets when they're ready.

The final thing M11 produces is a *written release-process document* — `/docs/RELEASING.md` — that describes, step by step, how to cut a new release. That document is the mission's real deliverable: it turns releasing from a tribal skill into a reproducible procedure.

---

## Scope

**In scope:**
- Verify/finalize the release profile in the workspace `Cargo.toml` (LTO, strip, panic=abort, codegen-units=1, opt-level=3).
- `cargo-wix` Windows MSI configuration and test build.
- `cargo-bundle` macOS `.app` configuration.
- `.dmg` creation for macOS via `hdiutil` (or a helper like `create-dmg`).
- `cargo-bundle` Linux `.deb` configuration.
- AppImage for Linux (via `appimagetool` in CI).
- Windows code signing via `cargo-wix`'s `sign` subcommand + SignTool (documented; optional behind a CI secret).
- macOS signing via `codesign --options runtime --timestamp` + notarization via `notarytool submit --wait --keychain-profile` + stapling via `stapler staple` (documented; optional behind CI secrets).
- `release.yml` GitHub Actions workflow that builds, packages, (optionally) signs, and uploads to GitHub Releases on every `v*.*.*` tag push.
- SHA256 checksums generated for every artifact and uploaded alongside.
- Final binary size verification (under 50 MB per OS).
- `/docs/RELEASING.md` release process documentation.
- Clean-VM installation test on each OS (documented manual step).

**Out of scope (explicitly):**
- Auto-update mechanism. Documented as a follow-up; V3+ work.
- Homebrew formula, winget manifest, scoop bucket, AUR PKGBUILD, snap, flatpak. All legitimate distribution channels but each is its own project; post-V2.
- Telemetry / crash reporting. Post-V2.
- Multi-architecture builds (ARM64 on Windows, x86_64 + ARM64 universal binary on macOS). We ship x86_64 only for V2 to keep CI simple; ARM builds are tracked as a follow-up.
- Microsoft Store / Mac App Store distribution. Both require sandboxing and additional compliance; not our path.

---

## North Star

At the end of M11 you can:

1. Push a git tag `v0.2.1`.
2. Watch the release workflow run for 15-25 minutes on GitHub Actions.
3. Open the `https://github.com/HiNala/IDE/releases/v0.2.1` page and see six files: `ide-v0.2.1-windows-x86_64.msi`, `ide-v0.2.1-macos-x86_64.dmg`, `ide-v0.2.1-linux-x86_64.AppImage`, `ide-v0.2.1-linux-x86_64.deb`, plus `SHA256SUMS.txt` and `SHA256SUMS.txt.asc` (or `.sig`; see sign-the-sums subsection).
4. Download each, install on a clean VM of the respective OS, launch, and be editing text.
5. Hand `/docs/RELEASING.md` to a new maintainer and have them produce the next release by themselves.

---

## TODO List

### 1. Audit and finalize the release profile

- [ ] 1.1. Open the workspace `Cargo.toml`. Confirm the following `[profile.release]`:
  ```toml
  [profile.release]
  opt-level = 3
  lto = "fat"
  codegen-units = 1
  panic = "abort"
  strip = "symbols"
  debug = false
  incremental = false
  overflow-checks = false
  ```
  Rationale: speed first (we're a real-time app, not a space-constrained embedded binary), but `strip`/`panic=abort`/`codegen-units=1` all reduce size without hurting runtime.
- [ ] 1.2. Add a second profile for profiling that keeps symbols:
  ```toml
  [profile.release-with-debug]
  inherits = "release"
  debug = true
  strip = "none"
  ```
  This profile is used for Tracy/perf sessions; it doesn't affect release artifacts.
- [ ] 1.3. Measure before-and-after binary size. Typical numbers on Windows with our full stack (wgpu + glyphon + cosmic-text + bundled font): expect a `target/release/editor-app.exe` in the 15-30 MB range. If larger, run `cargo bloat --release -p editor-app --crates` to see the breakdown.
- [ ] 1.4. Accept or reject `panic = "abort"`: this removes unwinding. Our code has zero `catch_unwind` calls — verify with `grep -r catch_unwind crates/`. If clean, keep abort. If something snuck in, either remove it or revert to `panic = "unwind"` and accept the size penalty.
- [ ] 1.5. Commit: `build(release): finalize production release profile`.

### 2. Windows: `cargo-wix` configuration

- [ ] 2.1. Install `cargo-wix` locally on the Windows dev machine: `cargo install cargo-wix`.
- [ ] 2.2. Install WiX Toolset v3.14 (Legacy variant, which `cargo-wix` defaults to and which GitHub's `windows-latest` runner ships with per the `cargo-wix` README's April 2026 note). Add `C:\Program Files (x86)\WiX Toolset v3.14\bin` to PATH.
- [ ] 2.3. In the workspace root, run `cargo wix init -p editor-app`. This creates `wix/main.wxs`. Inspect the generated file. `cargo-wix` fills in most of it from the `editor-app` crate's `Cargo.toml` `[package]` section (name, version, authors, description, license). Make sure those fields are populated correctly.
- [ ] 2.4. Edit `wix/main.wxs` lightly:
  - Set the `UpgradeCode` GUID (generated once, never changed across versions).
  - Set the install directory to `ProgramFiles64Folder\IDE` (default is usually fine).
  - Add a Start Menu shortcut pointing at `editor-app.exe` with a friendly name "IDE".
  - Add a file association for common text extensions (`.txt`, `.md`) — optional for V2; if it adds complexity, defer to a follow-up.
- [ ] 2.5. Add a `[package.metadata.wix]` section to `editor-app/Cargo.toml` for options `cargo-wix` reads (e.g., culture, output filename pattern).
- [ ] 2.6. Build the MSI locally: `cargo wix -p editor-app --output target/wix/ide-dev.msi`. Open it — Windows Installer should show a standard install dialog. Install. Verify the Start Menu shortcut. Run. Uninstall. Verify clean removal.
- [ ] 2.7. Commit: `build(windows): cargo-wix MSI configuration`.

### 3. Windows: code signing (documented; optional)

- [ ] 3.1. Document in `/docs/RELEASING.md` the process for signing:
  - Obtain a Windows code signing certificate (EV or standard) from a CA. EV certificates clear SmartScreen immediately; standard certificates must accumulate reputation first.
  - Install the certificate into the Windows Certificate Store, or export it as a `.pfx` file with a password.
  - For CI signing, store the `.pfx` base64-encoded in a GitHub Secret `WINDOWS_CERT_PFX_BASE64` and the password in `WINDOWS_CERT_PASSWORD`.
- [ ] 3.2. In `release.yml`'s Windows job, if `WINDOWS_CERT_PFX_BASE64` secret is set, decode to a temp file and run `cargo wix sign --package editor-app --output target/wix/ide.msi -- /f cert.pfx /p $env:WINDOWS_CERT_PASSWORD /tr http://timestamp.digicert.com /td sha256 /fd sha256`. Else skip signing and produce an unsigned MSI.
- [ ] 3.3. Document what a user sees in each case: signed MSI installs silently after a normal UAC prompt; unsigned MSI triggers SmartScreen ("Windows protected your PC"), users must click "More info" → "Run anyway".
- [ ] 3.4. Commit: `build(windows): optional code-signing path in release workflow`.

### 4. macOS: `.app` bundling

- [ ] 4.1. Install `cargo-bundle` locally: `cargo install cargo-bundle`. Note the README warning — cargo-bundle is alpha software; the `[package.metadata.bundle]` format can change. If something breaks, document the commit hash of the version that worked.
- [ ] 4.2. Add a `[package.metadata.bundle]` section to `editor-app/Cargo.toml`:
  ```toml
  [package.metadata.bundle]
  name = "IDE"
  identifier = "com.HiNala.IDE"
  icon = ["assets/icon.icns", "assets/[email protected]", "assets/icon.png"]
  version = "0.2.0"
  copyright = "Copyright (c) 2026 HiNala. MIT/Apache-2.0."
  category = "public.app-category.developer-tools"
  short_description = "A fast native code editor."
  long_description = """
  IDE is a native, high-performance code editor built for speed.
  GPU-accelerated rendering, sub-5ms input latency, handles large files.
  """
  ```
- [ ] 4.3. Create the icon assets. Start from a 1024x1024 PNG (commissioned or a minimal geometric logo for V2 — polish later). Use `iconutil` on macOS to convert to `.icns`. On Windows/Linux, use ImageMagick or an online converter to produce `.ico` and PNG variants. Commit icons to `editor-app/assets/icons/`.
- [ ] 4.4. Build locally on macOS: `cargo bundle --release -p editor-app --format osx`. Output: `target/release/bundle/osx/IDE.app`. Double-click — it should open the editor.
- [ ] 4.5. Commit: `build(macos): cargo-bundle configuration and icon assets`.

### 5. macOS: signing, notarization, stapling

- [ ] 5.1. Document the process in `/docs/RELEASING.md`:
  - Enroll in the Apple Developer Program ($99/year).
  - Request a "Developer ID Application" certificate via Xcode / developer.apple.com.
  - Store it in the macOS Keychain. Note the identity string — something like `"Developer ID Application: Your Name (TEAMID123)"`.
  - Create an app-specific password at `appleid.apple.com/account/manage`.
  - Store credentials in Keychain: `xcrun notarytool store-credentials "ide-notary" --apple-id "you@example" --team-id "TEAMID123" --password "APP_SPECIFIC_PASSWORD"`.
- [ ] 5.2. Write `scripts/macos-package.sh` that executes the full flow:
  ```bash
  #!/bin/bash
  set -euo pipefail
  APP="target/release/bundle/osx/IDE.app"
  DMG="target/release/ide.dmg"
  IDENTITY="${MACOS_CODESIGN_IDENTITY:-}"

  # Strip extended attributes that break notarization.
  xattr -cr "$APP"

  if [ -n "$IDENTITY" ]; then
      codesign --force --deep --options runtime --timestamp \
          --sign "$IDENTITY" "$APP"
      codesign --verify --deep --strict --verbose=2 "$APP"

      # ditto is required; plain `zip` breaks notarization.
      ditto -c -k --keepParent "$APP" "$APP.zip"

      xcrun notarytool submit "$APP.zip" \
          --keychain-profile "${NOTARY_PROFILE:-ide-notary}" \
          --wait
      xcrun stapler staple "$APP"
      rm "$APP.zip"
  else
      echo "No signing identity set; producing unsigned .app"
  fi

  # Build the DMG.
  hdiutil create -volname "IDE" -srcfolder "$APP" \
      -ov -format UDZO "$DMG"

  # Staple the DMG too so offline install works.
  if [ -n "$IDENTITY" ]; then
      xcrun stapler staple "$DMG" || true  # best-effort
  fi
  ```
- [ ] 5.3. In CI, set `MACOS_CODESIGN_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_SPECIFIC_PASSWORD` as GitHub Secrets. The workflow imports the certificate into a temporary keychain, stores the notarytool profile via `notarytool store-credentials`, then runs `scripts/macos-package.sh`.
- [ ] 5.4. When secrets are absent (first few releases, or for fork-built artifacts), skip signing and produce an unsigned `.dmg`. Document that users will need to run `xattr -cr /Applications/IDE.app` after install, or open via right-click → Open, to bypass Gatekeeper.
- [ ] 5.5. Commit: `build(macos): sign, notarize, staple, and DMG script`.

### 6. Linux: `.deb` via `cargo-bundle`

- [ ] 6.1. `cargo bundle --release -p editor-app --format deb` produces `target/release/bundle/deb/ide_0.2.0_amd64.deb`.
- [ ] 6.2. Declare dependencies in `[package.metadata.bundle]`:
  ```toml
  deb_depends = ["libc6", "libgl1", "libx11-6", "libxkbcommon0", "libwayland-client0"]
  ```
  Match against the Linux graphics deps the CI workflow installs (from M01).
- [ ] 6.3. Install the `.deb` on a clean Ubuntu 22.04 VM: `sudo dpkg -i ide_0.2.0_amd64.deb`. Run `ide` — should launch. Uninstall: `sudo apt remove ide`.
- [ ] 6.4. Commit: `build(linux): .deb packaging via cargo-bundle`.

### 7. Linux: AppImage

- [ ] 7.1. AppImage is a self-contained single-file format that works across distros. Script `scripts/linux-appimage.sh`:
  ```bash
  #!/bin/bash
  set -euo pipefail
  APPDIR="target/release/AppDir"
  rm -rf "$APPDIR"
  mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"

  cp target/release/editor-app "$APPDIR/usr/bin/ide"
  cp crates/editor-app/assets/icons/icon-256.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/ide.png"
  cp crates/editor-app/assets/icons/icon-256.png "$APPDIR/ide.png"

  cat > "$APPDIR/ide.desktop" <<EOF
  [Desktop Entry]
  Name=IDE
  Exec=ide %U
  Icon=ide
  Type=Application
  Categories=Development;TextEditor;
  EOF
  cp "$APPDIR/ide.desktop" "$APPDIR/usr/share/applications/ide.desktop"

  cat > "$APPDIR/AppRun" <<'EOF'
  #!/bin/sh
  HERE="$(dirname "$(readlink -f "${0}")")"
  exec "$HERE/usr/bin/ide" "$@"
  EOF
  chmod +x "$APPDIR/AppRun"

  # appimagetool expected to be downloaded beforehand (CI step).
  ARCH=x86_64 appimagetool "$APPDIR" "target/release/ide-x86_64.AppImage"
  ```
- [ ] 7.2. CI installs `appimagetool`:
  ```yaml
  - name: Install appimagetool
    run: |
      wget -q https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage -O /usr/local/bin/appimagetool
      chmod +x /usr/local/bin/appimagetool
  ```
  Confirm that URL still resolves; AppImageKit's release channel has been stable for years but verify at release time.
- [ ] 7.3. Test on a clean VM: download, `chmod +x`, run. Should launch.
- [ ] 7.4. Commit: `build(linux): AppImage packaging script`.

### 8. Generate SHA256 checksums

- [ ] 8.1. After all artifacts are built, the release job generates `SHA256SUMS.txt`:
  ```bash
  cd artifacts/
  sha256sum *.msi *.dmg *.deb *.AppImage > SHA256SUMS.txt
  ```
- [ ] 8.2. If a GPG signing key is configured (`RELEASE_GPG_PRIVATE_KEY` secret), sign the checksums file: `gpg --detach-sign --armor SHA256SUMS.txt` → produces `SHA256SUMS.txt.asc`. Import the key into a temporary keyring in CI; export fingerprint to release notes for users to verify against.
- [ ] 8.3. Document the verification procedure in `/docs/RELEASING.md`:
  ```bash
  # Users:
  gpg --verify SHA256SUMS.txt.asc SHA256SUMS.txt
  sha256sum -c SHA256SUMS.txt
  ```
- [ ] 8.4. Commit: `build(release): generate and sign SHA256 checksums`.

### 9. The `release.yml` workflow

- [ ] 9.1. `.github/workflows/release.yml`:
  ```yaml
  name: Release

  on:
    push:
      tags:
        - 'v*.*.*'

  permissions:
    contents: write

  jobs:
    build-windows:
      runs-on: windows-latest
      steps:
        - uses: actions/checkout@v4
        - uses: actions-rust-lang/setup-rust-toolchain@v1
        - name: Install cargo-wix
          run: cargo install cargo-wix --locked
        - name: Build release
          run: cargo build --release -p editor-app
        - name: Build MSI
          run: cargo wix -p editor-app --nocapture --output target/wix/ide-${{ github.ref_name }}-windows-x86_64.msi
        - name: Sign MSI (if cert configured)
          if: ${{ secrets.WINDOWS_CERT_PFX_BASE64 != '' }}
          shell: pwsh
          run: |
            $pfx = [System.Convert]::FromBase64String("${{ secrets.WINDOWS_CERT_PFX_BASE64 }}")
            [System.IO.File]::WriteAllBytes("cert.pfx", $pfx)
            & "C:\Program Files (x86)\Windows Kits\10\App Certification Kit\signtool.exe" sign /f cert.pfx /p ${{ secrets.WINDOWS_CERT_PASSWORD }} /tr http://timestamp.digicert.com /td sha256 /fd sha256 target/wix/*.msi
            Remove-Item cert.pfx
        - uses: actions/upload-artifact@v4
          with:
            name: windows-artifacts
            path: target/wix/*.msi

    build-macos:
      runs-on: macos-latest
      steps:
        - uses: actions/checkout@v4
        - uses: actions-rust-lang/setup-rust-toolchain@v1
        - name: Install cargo-bundle
          run: cargo install cargo-bundle --locked
        - name: Build release
          run: cargo build --release -p editor-app
        - name: Bundle .app
          run: cargo bundle --release -p editor-app --format osx
        - name: Import signing certificate
          if: ${{ secrets.MACOS_CERT_P12_BASE64 != '' }}
          run: |
            echo "${{ secrets.MACOS_CERT_P12_BASE64 }}" | base64 --decode > cert.p12
            security create-keychain -p ci build.keychain
            security default-keychain -s build.keychain
            security unlock-keychain -p ci build.keychain
            security import cert.p12 -k build.keychain -P "${{ secrets.MACOS_CERT_PASSWORD }}" -T /usr/bin/codesign
            security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k ci build.keychain
            xcrun notarytool store-credentials "ide-notary" \
              --apple-id "${{ secrets.APPLE_ID }}" \
              --team-id "${{ secrets.APPLE_TEAM_ID }}" \
              --password "${{ secrets.APPLE_APP_SPECIFIC_PASSWORD }}"
            rm cert.p12
        - name: Sign, notarize, DMG
          env:
            MACOS_CODESIGN_IDENTITY: ${{ secrets.MACOS_CODESIGN_IDENTITY }}
            NOTARY_PROFILE: ide-notary
          run: bash scripts/macos-package.sh
        - name: Rename DMG
          run: mv target/release/ide.dmg target/release/ide-${{ github.ref_name }}-macos-x86_64.dmg
        - uses: actions/upload-artifact@v4
          with:
            name: macos-artifacts
            path: target/release/*.dmg

    build-linux:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - name: Install Linux graphics deps
          run: |
            sudo apt-get update
            sudo apt-get install -y libx11-dev libxkbcommon-dev libwayland-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev libgl1-mesa-dev libegl1-mesa-dev mesa-vulkan-drivers fuse libfuse2
        - uses: actions-rust-lang/setup-rust-toolchain@v1
        - name: Install cargo-bundle
          run: cargo install cargo-bundle --locked
        - name: Install appimagetool
          run: |
            wget -q https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage -O /usr/local/bin/appimagetool
            chmod +x /usr/local/bin/appimagetool
        - name: Build release
          run: cargo build --release -p editor-app
        - name: Build .deb
          run: cargo bundle --release -p editor-app --format deb
        - name: Build AppImage
          run: bash scripts/linux-appimage.sh
        - name: Rename artifacts
          run: |
            mv target/release/bundle/deb/*.deb "target/release/ide-${{ github.ref_name }}-linux-x86_64.deb"
            mv target/release/ide-x86_64.AppImage "target/release/ide-${{ github.ref_name }}-linux-x86_64.AppImage"
        - uses: actions/upload-artifact@v4
          with:
            name: linux-artifacts
            path: |
              target/release/*.deb
              target/release/*.AppImage

    release:
      needs: [build-windows, build-macos, build-linux]
      runs-on: ubuntu-latest
      steps:
        - uses: actions/download-artifact@v4
          with:
            path: artifacts
            merge-multiple: true
        - name: Generate checksums
          run: |
            cd artifacts
            sha256sum * > SHA256SUMS.txt
        - name: Sign checksums (if key configured)
          if: ${{ secrets.RELEASE_GPG_PRIVATE_KEY != '' }}
          run: |
            echo "${{ secrets.RELEASE_GPG_PRIVATE_KEY }}" | gpg --import
            cd artifacts
            gpg --detach-sign --armor SHA256SUMS.txt
        - name: Publish release
          uses: softprops/action-gh-release@v3
          with:
            files: |
              artifacts/*
            generate_release_notes: true
            draft: false
            prerelease: ${{ contains(github.ref_name, '-') }}
  ```
- [ ] 9.2. Tags containing a `-` (like `v0.2.1-rc1`, `v0.3.0-beta`) are automatically marked as pre-releases via `contains(github.ref_name, '-')`.
- [ ] 9.3. The `permissions: contents: write` is required for the release job to publish.
- [ ] 9.4. Pin `actions/checkout`, `actions/upload-artifact`, `actions/download-artifact`, `actions-rust-lang/setup-rust-toolchain`, and `softprops/action-gh-release` to specific version tags — do *not* use `@main` or `@master`. Supply-chain discipline.
- [ ] 9.5. Commit: `ci: add release workflow for multi-OS packaged artifacts`.

### 10. Dry-run the workflow

- [ ] 10.1. Push a test tag `v0.0.0-dryrun`. Watch the workflow. Expect things to break: misspelled paths, missing tools, platform quirks. Fix iteratively.
- [ ] 10.2. Common failure modes:
  - `cargo wix` can't find WiX: verify `windows-latest` runner has it; as of April 2026 it still ships WiX 3.14 per cargo-wix's README. If that changes, add an explicit install step.
  - macOS `notarytool submit --wait` times out or gets stuck in "In Progress" for >30 min: Apple's service occasionally slows. Add `--timeout 30m` to the CLI. If still stuck, document as a retry-release situation.
  - Linux AppImage requires `libfuse2`, not just `fuse`, on Ubuntu 22.04+. Confirmed in the workflow above.
  - `cargo-bundle` alpha instability: if the `deb` format breaks for the current cargo-bundle release, pin to a specific version: `cargo install cargo-bundle --version 0.6.0 --locked`.
- [ ] 10.3. Delete the test tag and any draft release it produced: `git push origin :refs/tags/v0.0.0-dryrun` and remove the release from the GitHub UI.
- [ ] 10.4. Commit (as-needed): `ci(release): fixes from dry-run iteration`.

### 11. Clean-VM install tests

- [ ] 11.1. Windows 10 VM (or Windows Sandbox): download the MSI, run it, install, launch from Start Menu, edit a file, save, uninstall. Record observations in `/docs/RELEASING.md`.
- [ ] 11.2. macOS VM or fresh user account: download the DMG, mount, drag IDE.app to /Applications, launch. If unsigned, the user must right-click → Open the first time. Verify.
- [ ] 11.3. Ubuntu 22.04 VM: (a) install `.deb` via `sudo dpkg -i`, run `ide`. (b) download AppImage, `chmod +x`, run.
- [ ] 11.4. Document every observation, every workaround, every edge case in `/docs/RELEASING.md` under a "Known install-time issues" heading.
- [ ] 11.5. Commit: `docs(releasing): clean-VM install test results`.

### 12. Write `/docs/RELEASING.md`

- [ ] 12.1. Structure:
  - **Prerequisites**: Apple Developer Program membership, Windows code signing cert (optional), GPG signing key (optional).
  - **Secrets to configure in GitHub**: list every secret the workflow expects.
  - **How to cut a release**: bump version in `Cargo.toml`, update `CHANGELOG.md`, commit, tag `vX.Y.Z`, push tag, watch Actions.
  - **If a release fails**: retry strategies, re-running individual jobs, manually running `scripts/macos-package.sh` locally.
  - **Verifying a released artifact**: SHA256 + GPG verification, codesign verification, stapler ticket verification.
  - **Rolling back a bad release**: delete the GitHub Release + tag, publish a new `.1` version.
  - **Known install-time issues**: SmartScreen on unsigned Windows MSI, Gatekeeper on unsigned macOS DMG, libfuse2 on older Ubuntu, etc.
- [ ] 12.2. Lead with a single-page "happy path" checklist — someone should be able to cut a release in 10 minutes if nothing breaks.
- [ ] 12.3. Commit: `docs: write RELEASING.md playbook`.

### 13. Binary size verification and regression guard

- [ ] 13.1. Add a CI job that measures the binary size on each OS and fails if it grows > 20% beyond a recorded baseline. Store the baseline in `.github/size-baselines.json`. Update the baseline deliberately when new features justify growth.
- [ ] 13.2. Record current baselines in `/docs/BENCHMARKS.md` under "Binary size at `0.2.0`".
- [ ] 13.3. Commit: `ci: binary size regression guard`.

### 14. Document auto-update as a future mission

- [ ] 14.1. `/FOLLOWUPS.md` gets a substantial "Auto-update" entry. Summarize options:
  - Self-update via a download-and-replace-binary step (e.g., `self_update` crate). Works; requires the running editor to detect a new version, download it, and relaunch.
  - Platform-native mechanisms (Squirrel on Windows, Sparkle on macOS). More polished, more complex.
  - Rely on the distribution channel (winget, Homebrew, apt). Least work, depends on those channels existing.
- [ ] 14.2. Pick a target for V3 and note the rationale.
- [ ] 14.3. Commit: `docs(followups): auto-update options for V3`.

### 15. Cut the first full release

- [ ] 15.1. Bump workspace version to `0.2.1` (first real post-M10 release; `0.2.0-v2` was a release candidate).
- [ ] 15.2. Update `CHANGELOG.md` with a `## [0.2.1] — YYYY-MM-DD` section summarizing every M09+M10+M11 change that affects users.
- [ ] 15.3. Commit: `chore(release): prepare v0.2.1`.
- [ ] 15.4. Tag: `git tag -a v0.2.1 -m "First packaged release"`; push.
- [ ] 15.5. Watch the workflow. Verify all six artifacts appear on the Releases page.
- [ ] 15.6. Run through the install-test checklist one final time for `v0.2.1`.
- [ ] 15.7. Announce: update the repo README with a "Download" section linking to the latest release.
- [ ] 15.8. Tag: `git tag -a m11-complete -m "M11 complete: release engineering shipped"`; push.

### 16. Quality gates

- [ ] 16.1. `cargo fmt --all --check`.
- [ ] 16.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 16.3. `cargo test --workspace`.
- [ ] 16.4. Release workflow green on `v0.2.1`.
- [ ] 16.5. All three OS install tests pass on clean VMs.
- [ ] 16.6. `/docs/RELEASING.md` is complete and a teammate who has never shipped a release reads it and agrees they could reproduce the process.

---

## Validation / Acceptance Criteria

M11 is complete when:

1. Quality gates pass.
2. A `v*.*.*` tag push produces six published release artifacts automatically.
3. Each artifact installs cleanly on a clean VM of the respective OS.
4. SHA256 checksums are generated and (optionally) signed.
5. `/docs/RELEASING.md` is the canonical written playbook.
6. Binary size is under 50 MB on each OS and monitored by a CI regression guard.
7. Code-signing paths are documented and conditionally active based on secrets.
8. `m11-complete` tag pushed.
9. `v0.2.1` is live on the Releases page.

## Testing Requirements

- Dry-run the workflow on a test tag before cutting a real release.
- Install each artifact on a clean VM.
- Verify SHA256 checksums and (if signed) GPG signatures round-trip.
- Verify macOS notarization ticket is stapled: `xcrun stapler validate IDE.app` should report "The validate action worked!"
- Verify Windows MSI signature (if signed): `signtool verify /pa /v ide.msi`.

## Git Commit Strategy

12-16 commits. Push after items 2, 5, 7, 9, 10, 12, 15. The CI iteration (item 10) may produce many small commits — that's fine, it's how CI debugging works.

## Handoff: Steady-State Maintenance

M11 is the terminal mission in the initial mission plan. After it ships:

- **New features** get their own missions (M12+) written in the same format as M00-M11.
- **Bug fixes** follow the normal PR workflow; no mission needed for routine fixes.
- **Releases** follow `/docs/RELEASING.md`.
- **Regressions** are caught by the M07 benchmark gate and the M08 acceptance tests — which M11's release workflow also exercises implicitly by running `cargo test --workspace` and the `--perf-smoke` script before publishing.

The next logical missions, now that the MVP+V2 is shipped, are probably:

- **M12**: Syntax highlighting via Tree-sitter (biggest user-facing win).
- **M13**: LSP client (second-biggest win; enables autocomplete, go-to-definition, diagnostics).
- **M14**: Search & replace in file / across files.
- **M15**: Multi-tab / multi-file editing.
- **M16**: ARM64 builds for Windows and macOS (universal binaries).
- **M17**: Auto-update mechanism.

Those are future plans, not commitments. Brian picks priorities based on what the product needs next.

---

## Standing Orders Reminder

- Signed > unsigned. An unsigned release is acceptable for the very first public release, but every release after should aim for signed. SmartScreen and Gatekeeper warnings are a real barrier to adoption.
- Never publish artifacts that haven't passed the install-test on a clean VM. "It works on my dev machine" has betrayed every software project that ever trusted it.
- Every secret in CI is a potential supply-chain attack vector. Use GitHub's environment protection rules for production-release secrets; require manual approval for the release job if the organization grows.
- When notarization fails with a cryptic error, read `xcrun notarytool log <submission-id>` before flailing. The logs tell you exactly what Apple doesn't like.
- The release workflow will break occasionally due to upstream changes (new WiX version, new notarytool behavior, new GitHub Actions deprecation). That is normal. Fix and document in `/docs/RELEASING.md`.
- If you find yourself tempted to add a "just this once" manual step to the release process, stop and add it to the workflow instead. Manual steps erode fast.

This is the final mission. When it's done, the project has a public face.

Go.
