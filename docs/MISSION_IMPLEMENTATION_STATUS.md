[← docs/](./) · [README](../README.md)

# Mission implementation status (M00–M24)

**Purpose:** Map each official mission in [`missions/`](missions/) to what exists in this repository **today**. This is the honest source of truth for “what shipped” vs “what the vision doc describes.”

**Quality gates (last full pass):** Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `cargo build --release` before every release.

| Mission | Title | Status | Notes |
|--------|--------|--------|--------|
| **M00** | Foundation research & docs | **Done** | `docs/`, `reference/`, mission pack under `docs/missions/`. |
| **M01** | Repo scaffolding & CI | **Done** | Workspace crates, `.github/workflows/ci.yml`, `--dry-run`. |
| **M02** | Text engine | **Done** | `TextBuffer`, undo, selection primitives, proptest benches in `editor-core`. |
| **M03** | Windowing & wgpu | **Done** | `GpuContext`, swapchain, resize in `editor-render`. |
| **M04** | Text rendering (glyphon) | **Done** | `EditorRenderer`, `TextLayer`, JetBrains Mono in `editor-render`. |
| **M05** | Frame loop & input | **Done** | `editor-input::map_key_event`, `editor-app` wiring, scroll/cursor. |
| **M06** | File I/O | **Done** | `editor-io` load/save, atomic writes; background worker in `editor-core::WorkerPool`. |
| **M07** | Observability | **Partial** | `metrics.rs`, dev HUD (F11); Tracy optional feature. Full Criterion baselines / dashboard TBD per mission. |
| **M08** | MVP acceptance | **Partial** | `docs/MVP_ACCEPTANCE.md` exists; many rows remain N/A until measured stress runs. |
| **M09** | V2: gutter, selection, clipboard | **Done** | Line numbers, selection, `arboard` clipboard, undo in app. |
| **M10** | V2: word nav, status bar, persistence | **Done** | `word_nav`, `StatusBarInfo`, `PersistedState` / `state.json`, CLI path. Version tag `0.2.0-v2` is a **release decision**, not automatic. |
| **M11** | Release engineering | **Partial** | [`.github/workflows/release.yml`](../.github/workflows/release.yml) publishes unsigned binaries + `SHA256SUMS.txt` on `v*` tags; MSI/dmg/deb/signing still TODO per M11 doc. |
| **M12** | Resize / DPI polish | **Partial** | Scale factor wired where present; full M12 acceptance checklist not done. |
| **M13** | Workspace & multi-buffer | **Not started** | No `editor-workspace` crate; single buffer in `editor-app`. |
| **M14** | Sidebar, tabs, quick open | **Not started** | Depends on M13. |
| **M15** | Syntax highlighting | **Not started** | |
| **M16** | Find / replace | **Not started** | |
| **M17** | Diff engine | **Not started** | |
| **M18** | Git integration | **Not started** | |
| **M19** | AI provider abstraction | **Not started** | |
| **M20** | Agent tool-use API | **Not started** | |
| **M21** | Metadata sidecar | **Not started** | |
| **M22** | Local vector index | **Not started** | |
| **M23** | AI chat panel | **Not started** | |
| **M24** | V3 acceptance & release | **Not started** | Depends on M12–M23. |

## North-star reminder

- **M00–M10:** “Single-file, fast, native editor” with V2 affordances.
- **M11–M24:** Packaging, polish, then **V3 AI-native** features (workspace, multi-buffer, AI, retrieval).

See [`00_V3_VISION.md`](missions/00_V3_VISION.md) for the V3 product arc.

*Last updated: 2026-04-20.*
