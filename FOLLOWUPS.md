# Follow-ups

Items that need a **dedicated mission** or **product decision** rather than a drive-by fix.

## Current focus (by mission pack)

The codebase implements **M00–M13** in tree (see [`docs/MISSION_IMPLEMENTATION_STATUS.md`](docs/MISSION_IMPLEMENTATION_STATUS.md)): multi-buffer + workspace wiring lives in `editor-app`. Outstanding high-level work:

1. **M07–M08 — Observability & MVP acceptance**  
   Fill `docs/MVP_ACCEPTANCE.md` with measured p50/p95/p99, stress runs, and release decision.

2. **M11 — Release engineering**  
   **Done (partial):** [`release.yml`](.github/workflows/release.yml) publishes unsigned binaries on `v*` tags. **Still TODO:** MSI/dmg/deb/AppImage, optional signing, binary size budget — see [`docs/missions/M11_RELEASE_ENGINEERING_PACKAGING.md`](docs/missions/M11_RELEASE_ENGINEERING_PACKAGING.md) and [`docs/RELEASING.md`](docs/RELEASING.md).

3. **M12 — Resize / DPI polish**  
   **Done in tree:** sync resize/DPI paint, present mode, battery cap, row scratch, `--resize-telemetry`, `gpu_resize_stress`, overlay stacking, **`m12-gpu-resize-windows`** CI job, resize artifact catalog in `docs/DIAGNOSING_PERFORMANCE.md`, `scripts/resize-stress.{ps1,sh}`. **Manual / release:** screen recordings, p99 sign-off, `m12-complete` tag; optional 8K offscreen pass if render-to-texture — [`docs/missions/M12_WINDOW_RESIZE_DPI_POLISH.md`](docs/missions/M12_WINDOW_RESIZE_DPI_POLISH.md).

4. **M14 — Sidebar, tabs, quick-open — wired**
   **Done (code):** `Sidebar`, `TabStrip`, `QuickOpenPalette` now paint every frame via `FrameChrome`; `Ctrl+B` toggles the tree, `Ctrl+Shift+E` focuses it, `Ctrl+P` opens the fuzzy palette, `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle tabs, `Ctrl+W` closes, `Ctrl+N` opens a scratch buffer. Mouse routing respects chrome zones; keyboard is intercepted while the palette is visible. Sidebar visibility + width persist across sessions. Opening a folder via the CLI (`editor-app <dir>`) auto-shows the tree and seeds quick-open. **Still TODO:** keyboard-driven sidebar navigation (Up/Down/Enter/collapse), explicit `Ctrl+K Ctrl+O` keybinding for folder open (currently only wired through `Ctrl+B` prompting when no workspace is open), and a per-buffer dirty-guard dialog for `Ctrl+W`.

5. **Buffer-switch undo caveat (follow-up for M02 / M13)**
   `BufferState::undo` is not cloned when the app mirrors state to/from the `BufferManager` because `editor-core::UndoStack` is not `Clone`. Today each switch starts the target buffer with an empty undo history. Either add `#[derive(Clone)]` to `UndoStack` (verify timestamps / instants are safe to duplicate) or move authoritative undo into the `BufferManager` and stop mirroring on switch. Noted at the `sync_active_to_manager_with_id` call site in `crates/editor-app/src/main.rs`.

6. **M15–M24 — V3 features**
   Syntax highlighting, find/replace (in-buffer `Ctrl+F` UI still unwired — the `FindBar` crate and key mapping exist), diff, richer git integration, AI provider, tools API, sidecar, vector index, chat panel, V3 acceptance — follow [`docs/missions/00_V3_VISION.md`](docs/missions/00_V3_VISION.md) and each mission file in order.

## V4+ candidates (post-V3)

Rough backlog for future mission sets; not blocking V3. See [`docs/missions/M24_V3_ACCEPTANCE_RELEASE.md`](docs/missions/M24_V3_ACCEPTANCE_RELEASE.md) §17.

| Item | Rationale | Priority | Builds on V3 |
|------|-----------|----------|----------------|
| Custom themes | User preference beyond default dark/light | Medium | Render + UI chrome |
| Autocomplete as-you-type | Parity with mainstream editors | High | Tree-sitter, buffer, input pipeline |
| LSP client | Diagnostics, goto-def, refs as peers of Tree-sitter | High | Workspace, buffers, UI panels |
| Debugger | Run/stop/breakpoints for compiled projects | Medium | Terminal, workspace, optional DAP crate |
| Plugin API | Third-party extensions without forking | Medium | Stable IPC or WASM boundary TBD |
| Remote editing | SSH / remote workspace | Lower | I/O + workspace abstraction |
| Collaborative sessions | Shared cursors / OT/CRDT | Lower | Sync model + net layer |
| Background agents | Long-running tasks outside the UI thread | Medium | AI tools + job system |
| Auto-update | Ship patches without manual download | Medium | M11 release + signing story |
| ARM64 builds | Apple Silicon / Windows ARM installers | Medium | CI matrix + packaging |

## Documentation

- Keep [`docs/STATUS.md`](docs/STATUS.md) and [`docs/MISSION_IMPLEMENTATION_STATUS.md`](docs/MISSION_IMPLEMENTATION_STATUS.md) updated when a mission phase closes.
- Historical references to `/00_MISSION_INDEX.md` at repo root point to [`docs/missions/00_MISSION_INDEX.md`](docs/missions/00_MISSION_INDEX.md).

## Process

- Canonical mission list: [`docs/missions/00_MISSION_INDEX.md`](docs/missions/00_MISSION_INDEX.md).
