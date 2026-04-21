# M25 — Critical Fixes & Foundation Completion

**Mission ID:** M25
**Prerequisites:** `0.2.0-v2` shipped. The project is in the state the 2026-04-20 audit described.
**Output:** One mission that closes every open item the audit identified, completes the three partial missions (M11 installers, M12 resize polish, M13 workspace integration), runs a clean acceptance pass, and cuts `0.2.1` with real installers. After M25, the project is a complete, distributable V2 on a solid foundation — the proper starting point for V3.
**Estimated scope:** 2-3 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_STATE_2026_04_20.md` — what the audit found.
- `/M11_RELEASE_ENGINEERING_PACKAGING.md` — installer work still owed.
- `/M12_WINDOW_RESIZE_DPI_POLISH.md` — resize polish still owed.
- `/M13_WORKSPACE_MULTI_BUFFER.md` — workspace watcher integration still owed.

---

## The Situation In Plain English

The audit found a project in good shape with a handful of specific problems. None are deep; none require rewrites. But they are exactly the kind of issues that compound if left alone — each one is small, each one is a small tax on everything built on top. If we start V3 on a foundation with silent data-loss paths, an incorrectly-keyed layout cache, a workspace watcher that discards events, and no actual installers, we spend the rest of V3 tripping over them. Fix once, in one focused mission, before V3 begins.

M25 is deliberately a "completion and correction" mission rather than new capability. It does not add any new end-user-visible feature. What it produces is: (1) a cleaner codebase with no known bugs in M00-M13 territory; (2) real installers for Windows, macOS, and Linux; (3) a formal re-run of the MVP + V2 acceptance suites to establish the baseline numbers V3 must preserve. After this, every subsequent mission can assume the foundation is exactly what the plan says it is.

The mission has four named workstreams, each small and focused:

- **Workstream A — Audit sweep.** Every concrete bug the audit listed. Fix, test, commit each in isolation.
- **Workstream B — M11 installers.** Cargo-wix MSI, cargo-bundle `.app` + `.deb`, AppImage, signed where keys exist, unsigned with clear user instructions where not. The `release.yml` workflow from the original M11 plan, executed.
- **Workstream C — M12 polish.** Pre-allocated GPU resources, width-independent layout cache, Windows modal-resize fix, refresh-rate adaptation, battery awareness. Everything M12 promised but only half-delivered.
- **Workstream D — M13 integration.** File-watcher events routed into the app; external-modification detection surfaced in the UI; save-vs-external-change confirmation dialog; end-to-end tests for multi-buffer + workspace coherence that the audit flagged as missing.

Finally, re-run the M08 MVP acceptance suite and the post-M10 V2 acceptance suite to capture fresh numbers. Tag `v0.2.1`. Push. Release workflow produces installers. VM-test each.

---

## Scope

**In scope:**
- Every concrete bug from the audit.
- Full completion of M11's installer workstream.
- Full completion of M12's polish workstream.
- Full completion of M13's integration workstream.
- Integration tests for multi-buffer + workspace coherence.
- Turn on the `#[ignore]`'d stress tests in a nightly CI job.
- Re-run MVP + V2 acceptance suites, capture fresh baselines.
- Refactor `FrameInput` into sub-structs per the audit's architecture note (bounded cleanup, not a rewrite).
- Ship `v0.2.1` installers.

**Out of scope:**
- Any V3 work. M25 is strictly consolidation. If you find yourself wanting to add syntax highlighting, stop — that's M15.
- Rewriting anything not specifically flagged. The codebase is 8.5/10; respect it.
- Changing the public API of any crate unless required by a fix.

---

## North Star

Two weeks after starting M25: `git tag -l` shows `v0.2.1`. The Releases page has six artifacts (MSI, dmg, AppImage, deb, SHA256SUMS, signature). Each installs cleanly on a clean VM of the respective OS. The acceptance report `/docs/V2_1_ACCEPTANCE.md` shows every MVP and V2 metric green with fresh numbers. `cargo test --workspace --all-features` passes. `cargo clippy -- -D warnings` is clean. The only thing the editor can't do that V3 will add is syntax highlighting, search, diff/git, AI — all of which have cleanly-scoped missions waiting. The foundation is dead solid; V3 can begin.

---

## TODO List

### Workstream A — Audit Sweep (the concrete bugs)

#### 1. Verify and fix the `EditorCommand::ToggleFullscreen` situation

- [ ] 1.1. Grep: `rg 'ToggleFullscreen' crates/`. Confirm where it's defined and where it's matched.
- [ ] 1.2. Build with `cargo build --workspace --all-features`. If the build fails on a non-exhaustive match, implement the arm.
- [ ] 1.3. Implementation: `EditorCommand::ToggleFullscreen` maps to `window.set_fullscreen(if window.fullscreen().is_some() { None } else { Some(Fullscreen::Borderless(None)) })`. Bind to `F11`.
- [ ] 1.4. Test: launch, press F11, verify fullscreen toggles; press again, verify restore.
- [ ] 1.5. Commit: `fix(app): implement ToggleFullscreen command handler`.

#### 2. Wire workspace file-watcher events to buffer state

- [ ] 2.1. In `main.rs` around line 949 where `poll_workspace_fs` runs today: for each `FileSystemEvent`:
  - `Modified(path)`: if any buffer has this path, read the on-disk mtime and compare to `external_mtime`; if differs, set `external_modified = true` on that `BufferState`. Post a banner "File changed on disk" via the existing banner system.
  - `Removed(path)`: log a warning; do NOT auto-close the buffer.
  - `Renamed { from, to }`: if a buffer had `from`, update its path to `to`.
  - `Created(path)`: no buffer action (the sidebar will pick it up when M14 ships).
- [ ] 2.2. Ignore events for paths under `.git/`, `.ide/`, `target/`, `node_modules/` — too noisy.
- [ ] 2.3. Coalesce bursts: if 10+ events fire for the same path within 500 ms, collapse to one.
- [ ] 2.4. Unit test: tempdir + mocked buffer manager + simulated event → assert buffer state update.
- [ ] 2.5. Commit: `fix(app): route workspace watcher events to buffer state (closes M13 #11)`.

#### 3. Save-when-externally-modified confirmation

- [ ] 3.1. Before any `save_file_sync` call: check `active_buffer.external_modified`. If true:
  - Show a modal-style banner (not a real OS dialog — V3 doesn't have a dialog system): "This file was changed on disk by another program. [O]verwrite, [R]eload disk version, [C]ancel."
  - Block save until the user presses one of the three keys.
- [ ] 3.2. Overwrite: proceed with save; log that it was an explicit overwrite.
- [ ] 3.3. Reload: discard in-memory changes, re-read from disk, clear dirty flag.
- [ ] 3.4. Cancel: abort save, leave buffer as-is.
- [ ] 3.5. Same flow on `Ctrl+Shift+S` (Save As) — the external-modified check is on the source, not destination.
- [ ] 3.6. Unit test: simulated scenario for each branch.
- [ ] 3.7. Commit: `fix(app): confirmation flow on save-with-external-modifications (data-loss fix)`.

#### 4. Fix the layout cache key

- [ ] 4.1. In `TextLayer::layout_cache`: change the key type from `(usize, u64)` to `(usize, u64, u32)` where the third is `wrap_width_px` (quantized to whole pixels).
- [ ] 4.2. For the non-wrapped mode V3 uses: `wrap_width_px` can be set to a sentinel like `u32::MAX` so it never invalidates on resize. Do it this way rather than "ignore the key" — explicit sentinel is readable.
- [ ] 4.3. Unit test: re-shape count before/after a height-only resize; assert zero re-shapes.
- [ ] 4.4. Benchmark: confirm no regression on the non-resize path.
- [ ] 4.5. Commit: `perf(render): width-aware layout cache key (closes M12 #3)`.

#### 5. Silent clipboard-paste failure

- [ ] 5.1. Currently a failing `arboard::Clipboard::get_text()` silently drops the paste. Wrap the call and if it errors:
  - Log at WARN.
  - Post a banner "Clipboard read failed: <error>" to the status area.
- [ ] 5.2. Commit: `fix(input): surface clipboard read failures instead of silent drop`.

#### 6. Hardcoded undo coalescing window → config constant

- [ ] 6.1. Pull the 530 ms value into `editor_core::undo::COALESCE_MS` with documentation. Make it configurable via `PersistedState::undo_coalesce_ms`.
- [ ] 6.2. Default unchanged (530 ms). Tests remain green.
- [ ] 6.3. Commit: `refactor(core): promote undo coalesce window to a named constant`.

#### 7. Turn on stress tests in CI

- [ ] 7.1. Remove the `#[ignore]` from the M08 stress tests (`100MB_file_load`, `1M_rapid_edits`, `long_session_memory`).
- [ ] 7.2. Add a new CI job `stress-nightly` that runs these tests once per night on Ubuntu. Don't run them on every PR (too slow + too flaky in cold GitHub runners).
- [ ] 7.3. Publish results to an artifact each night; a simple text file per run.
- [ ] 7.4. Commit: `ci(stress): nightly stress test job`.

### Workstream B — M11 Installers

#### 8. Windows MSI via cargo-wix

- [ ] 8.1. Follow M11 §2 exactly. `cargo install cargo-wix`; WiX 3.14 (which `windows-latest` runners still ship as of the current plan).
- [ ] 8.2. `cargo wix init -p editor-app`. Review `wix/main.wxs`. Set `UpgradeCode` GUID. Add Start Menu shortcut.
- [ ] 8.3. Local build: `cargo wix -p editor-app --output target/wix/ide-dev.msi`. Install on a clean Windows VM; confirm launch.
- [ ] 8.4. Commit: `build(windows): cargo-wix MSI configuration`.

#### 9. macOS `.app` + `.dmg`

- [ ] 9.1. Follow M11 §4 and §5. `cargo install cargo-bundle`. Populate `[package.metadata.bundle]`.
- [ ] 9.2. Icon assets (commission a minimal 1024×1024 mark; `iconutil` to produce `.icns`; ImageMagick for `.ico` and `.png`). Commit to `editor-app/assets/icons/`.
- [ ] 9.3. `scripts/macos-package.sh` per M11 §5.2. Even when no signing identity is set, must produce a runnable (unsigned) `.dmg`.
- [ ] 9.4. If Brian has an Apple Developer account: store the Developer ID certificate and notarytool profile as GitHub secrets per M11 §5.3. If not: ship unsigned; document the right-click → Open workaround in README.
- [ ] 9.5. Commit: `build(macos): cargo-bundle + .dmg packaging`.

#### 10. Linux `.deb` + AppImage

- [ ] 10.1. Per M11 §6 and §7. `deb_depends` matching the CI-installed graphics deps.
- [ ] 10.2. AppImage via `appimagetool`; `scripts/linux-appimage.sh`.
- [ ] 10.3. Install-test each on a clean Ubuntu 22.04 VM.
- [ ] 10.4. Commit: `build(linux): .deb + AppImage`.

#### 11. The `release.yml` workflow

- [ ] 11.1. Per M11 §9. Matrix build across the three OSes; upload artifacts; `softprops/action-gh-release@v3` publishes on `v*.*.*` tags. SHA256SUMS generated in the final job.
- [ ] 11.2. Pin every action to an exact tagged version. No `@main` / `@master`.
- [ ] 11.3. Permissions: `contents: write` on the release job only.
- [ ] 11.4. Commit: `ci: release workflow for multi-OS installers`.

#### 12. Dry-run the release

- [ ] 12.1. Push a throwaway tag `v0.2.1-dryrun`. Watch the workflow. Expect initial failures — path glitches, tool versions, platform quirks. Iterate until all three jobs produce artifacts and the final release job uploads them.
- [ ] 12.2. Delete the test tag + the draft release.
- [ ] 12.3. Commit (as needed): `ci(release): fixes from dry-run`.

### Workstream C — M12 Polish

#### 13. Pre-allocated GPU resources

- [ ] 13.1. Per M12 §2. Viewport-sized textures pre-allocated at a generous max (8K target). Resize uses the existing allocation with new descriptor rect. Verify zero new allocations during a continuous resize.
- [ ] 13.2. Commit: `perf(render): pre-allocate viewport-sized resources`.

#### 14. Windows modal resize survival

- [ ] 14.1. Per M12 §4. In `ApplicationHandler::window_event` on `Resized`, render synchronously rather than via `request_redraw`. Content updates during the OS's modal resize loop.
- [ ] 14.2. Manual test on Windows: drag an edge for 10 seconds, content updates continuously.
- [ ] 14.3. Commit: `fix(app): synchronous render on Resized to survive Windows modal resize loop`.

#### 15. Single-frame DPI change + refresh-rate adaptation + battery awareness

- [ ] 15.1. Per M12 §5, §6, §7. On `ScaleFactorChanged`, eagerly re-rasterize the visible range before the next frame.
- [ ] 15.2. Adaptive `PresentMode`: `Mailbox` for 120+ Hz, `FifoRelaxed` for 60 Hz.
- [ ] 15.3. `battery = "0.7"` crate; poll every 30 s; cap at 60 Hz on battery unless user overrides.
- [ ] 15.4. Persist adaptive preferences in `PersistedState`.
- [ ] 15.5. Commit (split into 2-3 commits): `perf(render): DPI fast path`, `feat(app): refresh-rate adaptation`, `feat(app): battery-aware frame cap`.

#### 16. Scripted resize stress test

- [ ] 16.1. Per M12 §10. `scripts/resize-stress.ps1` on Windows. p99 frame time during 10 seconds of continuous resize-drag < 32 ms; zero frames > 100 ms.
- [ ] 16.2. Add to the Windows CI job.
- [ ] 16.3. Commit: `test(perf): scripted resize stress test`.

### Workstream D — M13 Integration

#### 17. External-modification flag in status bar

- [ ] 17.1. Extend `StatusBarInfo` with `external_modified: bool`. When set, status bar shows a small warning indicator "⚠ disk changed" next to the file name.
- [ ] 17.2. The banner system from fix #3 is the *interactive* surface; this status-bar marker is the *passive* indicator even if the banner was dismissed.
- [ ] 17.3. Commit: `feat(ui): status bar external-modified indicator`.

#### 18. Multi-buffer + workspace coherence integration tests

- [ ] 18.1. End-to-end tests (under `tests/integration/` using a headless App harness — you may need to build a minimal test harness that can drive the App without a visible window; `winit` supports `EventLoop::new_headless` only on some platforms, so a software-only test may mean mocking winit).
- [ ] 18.2. Tests to add:
  - Open workspace → 3 buffers → switch between → close one → verify correct state.
  - Open file A → modify externally → confirm buffer flagged.
  - Save with external-modified set → verify confirmation flow.
  - Rename file externally → verify buffer path updates.
- [ ] 18.3. At least 5 such tests. The audit flagged this gap specifically; closing it matters for long-term regression safety.
- [ ] 18.4. Commit: `test(integration): multi-buffer and workspace coherence`.

#### 19. Progress bar for large file load

- [ ] 19.1. The `LoadProgress::Progress(f32)` event already fires from `editor-io`. Wire it into the loading banner so it shows `[████████  ] 82%` during load.
- [ ] 19.2. The banner disappears on `LoadProgress::Done`.
- [ ] 19.3. Commit: `polish(ui): progress bar for large-file load`.

#### 20. FrameInput → RenderContext refactor

- [ ] 20.1. Per the audit's architecture note. Split `FrameInput` into:
  ```rust
  pub struct RenderContext {
      pub text: TextRenderParams,    // buffer snapshot, cursor, selection, scroll, etc.
      pub ui: UiRenderParams,         // sidebar, tabs, status bar, banners
      pub debug: DebugRenderParams,   // hud, latency overlay
  }
  ```
- [ ] 20.2. Move every existing field into the right sub-struct. No behavior change.
- [ ] 20.3. Update every call site. Run tests.
- [ ] 20.4. Commit: `refactor(render): split FrameInput into RenderContext sub-structs`.

### 21. Re-run acceptance suites

- [ ] 21.1. M08 MVP acceptance: every benchmark from `/docs/ACCEPTANCE.md`. Compare to `.github/baselines/m08-mvp.json`. All green.
- [ ] 21.2. Post-M10 V2 acceptance: likewise.
- [ ] 21.3. Write `/docs/V2_1_ACCEPTANCE.md`: hardware used, numbers captured, pass/fail per test, any deltas from prior baseline.
- [ ] 21.4. Commit: `docs(acceptance): V2.1 acceptance report`.

### 22. Version bump + tag + release

- [ ] 22.1. Workspace `version = "0.2.1"`. `cargo build --release`.
- [ ] 22.2. `CHANGELOG.md` gets a `## [0.2.1] — YYYY-MM-DD` entry summarizing every fix + every installer.
- [ ] 22.3. README gets a "Download" section pointing at the Releases page, with per-OS install instructions including the SmartScreen/Gatekeeper caveats for unsigned builds.
- [ ] 22.4. Commit: `chore(release): prepare v0.2.1`.
- [ ] 22.5. `git tag -a v0.2.1 -m "V2.1: installers + audit sweep"`.
- [ ] 22.6. Push. Watch the workflow.
- [ ] 22.7. On a clean VM of each OS, install and sanity-check: launch, edit a file, save, close. Record outcomes in the acceptance report.
- [ ] 22.8. Tag the mission: `git tag -a m25-complete -m "M25 complete: foundation consolidated for V3"`. Push.

### 23. Quality gates

- [ ] 23.1. `cargo fmt --all --check`.
- [ ] 23.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 23.3. `cargo test --workspace --all-features`.
- [ ] 23.4. Release workflow green.
- [ ] 23.5. All 6 installer artifacts exist on the `v0.2.1` release.
- [ ] 23.6. VM installs pass on all 3 OSes.
- [ ] 23.7. `/docs/V2_1_ACCEPTANCE.md` is written and documents every number.

---

## Validation / Acceptance Criteria

1. Every item in Workstreams A-D is committed.
2. MVP and V2 acceptance suites green on fresh runs.
3. `v0.2.1` tagged and released; all 6 artifacts on the Releases page.
4. Clean-VM installs succeed on Windows, macOS, Linux.
5. `m25-complete` tag pushed.

## Testing Requirements

- Every fix has a regression test where possible.
- Five new integration tests for multi-buffer + workspace coherence.
- Nightly stress tests run in CI and produce artifacts.

## Git Commit Strategy

15-20 commits. Push after items 2, 5, 7, 12, 16, 20, 22.

## Handoff to M14

M14 assumes:
- All of M11/M12/M13 is done.
- The watcher event loop is live. When M14 adds the sidebar, it subscribes to the same events for tree-view refresh.
- The RenderContext refactor is in place, so adding sidebar/tabs rendering drops cleanly into `RenderContext::ui`.

---

## Standing Orders Reminder

- M25 adds no new user-visible feature. If you're tempted to slip in syntax highlighting or search "while we're in there," stop. That work belongs in dedicated missions with proper scope.
- A perf fix without a regression test is a half-fix. Always capture the before/after number.
- The `v0.2.1` release is real — if it ships broken, a user downloads broken software. Do not cut the tag until clean-VM installs pass.
- When in doubt about whether a bug was introduced by M25 or was pre-existing: `git bisect`. Don't guess.

Go.
