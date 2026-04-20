# Follow-ups

Items that need a **dedicated mission** or **product decision** rather than a drive-by fix.

## Current focus (by mission pack)

The codebase implements **M00–M10** in tree (see [`docs/MISSION_IMPLEMENTATION_STATUS.md`](docs/MISSION_IMPLEMENTATION_STATUS.md)). Outstanding high-level work:

1. **M07–M08 — Observability & MVP acceptance**  
   Fill `docs/MVP_ACCEPTANCE.md` with measured p50/p95/p99, stress runs, and release decision.

2. **M11 — Release engineering**  
   **Done (partial):** [`release.yml`](.github/workflows/release.yml) publishes unsigned binaries on `v*` tags. **Still TODO:** MSI/dmg/deb/AppImage, optional signing, binary size budget — see [`docs/missions/M11_RELEASE_ENGINEERING_PACKAGING.md`](docs/missions/M11_RELEASE_ENGINEERING_PACKAGING.md) and [`docs/RELEASING.md`](docs/RELEASING.md).

3. **M12 — Resize / DPI polish**  
   Complete checklist in [`docs/missions/M12_WINDOW_RESIZE_DPI_POLISH.md`](docs/missions/M12_WINDOW_RESIZE_DPI_POLISH.md).

4. **M13+ — V3**  
   New crates (`editor-workspace`, AI layers, etc.) per [`docs/missions/00_V3_VISION.md`](docs/missions/00_V3_VISION.md). Do not start M14 until M13’s data model exists.

## Documentation

- Keep [`docs/STATUS.md`](docs/STATUS.md) and [`docs/MISSION_IMPLEMENTATION_STATUS.md`](docs/MISSION_IMPLEMENTATION_STATUS.md) updated when a mission phase closes.
- Historical references to `/00_MISSION_INDEX.md` at repo root point to [`docs/missions/00_MISSION_INDEX.md`](docs/missions/00_MISSION_INDEX.md).

## Process

- Canonical mission list: [`docs/missions/00_MISSION_INDEX.md`](docs/missions/00_MISSION_INDEX.md).
