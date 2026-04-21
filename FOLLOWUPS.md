# Follow-ups

Items that need a **dedicated mission** or **product decision** rather than a drive-by fix.

## Current focus (by mission pack)

The codebase implements **M00–M13** in tree (see [`docs/MISSION_IMPLEMENTATION_STATUS.md`](docs/MISSION_IMPLEMENTATION_STATUS.md)): multi-buffer + workspace wiring lives in `editor-app`. Outstanding high-level work:

1. **M07–M08 — Observability & MVP acceptance**  
   Fill `docs/MVP_ACCEPTANCE.md` with measured p50/p95/p99, stress runs, and release decision.

2. **M11 — Release engineering**  
   **Done (partial):** [`release.yml`](.github/workflows/release.yml) publishes unsigned binaries on `v*` tags. **Still TODO:** MSI/dmg/deb/AppImage, optional signing, binary size budget — see [`docs/missions/M11_RELEASE_ENGINEERING_PACKAGING.md`](docs/missions/M11_RELEASE_ENGINEERING_PACKAGING.md) and [`docs/RELEASING.md`](docs/RELEASING.md).

3. **M12 — Resize / DPI polish**  
   **Progress:** width-independent doc line shaping, deferred first show, PNG taskbar icon, battery FPS cap (with `power_uncap_on_battery` in `state.json`), `--resize-telemetry`, monitor name → scale map, `scripts/resize-stress.ps1`. **Still TODO:** 8K-class GPU prealloc, allocation-count test, CI resize stress, eager DPI pre-raster benchmark, `m12-complete` tag — see [`docs/missions/M12_WINDOW_RESIZE_DPI_POLISH.md`](docs/missions/M12_WINDOW_RESIZE_DPI_POLISH.md).

4. **M14 — Sidebar, tabs, quick-open**  
   **Started:** `nucleo` + [`QuickOpenRanker`](crates/editor-ui/src/quick_open.rs) for fuzzy path ranking. **Still TODO:** GPU palette UI, `Ctrl+P` wiring, `TabStrip`, `Sidebar` — see [`docs/missions/M14_SIDEBAR_TABS_QUICK_OPEN.md`](docs/missions/M14_SIDEBAR_TABS_QUICK_OPEN.md).

5. **M15–M24 — V3 features**  
   Syntax highlighting, find/replace, diff, git, AI provider, tools API, sidecar, vector index, chat panel, V3 acceptance — follow [`docs/missions/00_V3_VISION.md`](docs/missions/00_V3_VISION.md) and each mission file in order.

## Documentation

- Keep [`docs/STATUS.md`](docs/STATUS.md) and [`docs/MISSION_IMPLEMENTATION_STATUS.md`](docs/MISSION_IMPLEMENTATION_STATUS.md) updated when a mission phase closes.
- Historical references to `/00_MISSION_INDEX.md` at repo root point to [`docs/missions/00_MISSION_INDEX.md`](docs/missions/00_MISSION_INDEX.md).

## Process

- Canonical mission list: [`docs/missions/00_MISSION_INDEX.md`](docs/missions/00_MISSION_INDEX.md).
