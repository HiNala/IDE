# Follow-ups

Items that need a **dedicated mission** or **product decision** rather than a drive-by fix.

## Next engineering sequence (highest leverage)

1. **M04 — Text rendering (glyphon + wgpu)**  
   Render `TextBuffer` content in `editor-render`; `editor-ui` layout for gutter placeholder.

2. **M05 — Frame loop + input wiring**  
   Connect `editor-input::map_key_event` to an `EditorState` in `editor-app` (buffer + cursor + selection); hit input-to-pixel budgets.

3. **M06 — File I/O**  
   Async load/save, atomic writes, encoding detection in `editor-io`.

4. **M07–M08 — Observability + MVP acceptance**  
   Dev overlay, Criterion baselines, stress tests, `docs/MVP_ACCEPTANCE.md` rows.

5. **M09–M10 — V2**  
   Line numbers, selection, clipboard, undo UI, status bar, `state.json` persistence.

6. **M11+**  
   Packaging (M11), then V3 missions M12–M24 per [`docs/missions/00_V3_VISION.md`](docs/missions/00_V3_VISION.md).

## Documentation

- Flesh out `docs/ARCHITECTURE.md` (crate graph, threads, frame phases) as M04+ land.
- Keep `docs/STATUS.md` in sync with the checklist above after each mission closes.

## Process

- Canonical mission list: [`docs/missions/00_MISSION_INDEX.md`](docs/missions/00_MISSION_INDEX.md).
- Historical references to `/00_MISSION_INDEX.md` at repo root mean the same content now under `docs/missions/`.
