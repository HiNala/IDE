[← docs/](./) · [README](../README.md)

# Current status

**Last updated:** 2026-04-20

## Mission progress

Canonical specs: [`missions/00_MISSION_INDEX.md`](missions/00_MISSION_INDEX.md). Short index: [`MISSIONS.md`](MISSIONS.md).

| Range | State | Notes |
|------|--------|--------|
| **M00** | Done | Docs + `reference/` PRDs. |
| **M01** | Done | Workspace, CI, `editor-app` binary. |
| **M02** | Done | `editor-core`: `TextBuffer`, `Cursor` + `CursorMotion` (incl. `WordLeft` / `WordRight`), `Selection`, `UndoStack`, line endings, `word_nav`, proptest + benches. |
| **M03** | Mostly done | `winit` + `GpuContext` + resize; `editor_render::EditorRenderer` composes GPU + text (M04). |
| **M04** | Mostly done | `glyphon` + `cosmic-text` in `crates/editor-render/src/text_layer.rs`: visible lines, scroll, blinking caret (custom glyph). `editor-app` shows sample buffer + ↑/↓/PgUp/PgDn. Optional: add bundled monospace font asset, full M04 perf checklist. |
| **M05** | Mostly done | `editor-input`: `map_key_event` (word motion, word delete, F11). `editor-app`: `ModifiersChanged`, caret motion + scroll-into-view, optional file path in title, F11 dev stats in title, undo stack for word deletes; resize/scale paints via `paint_frame`. |
| **M06–M08** | Not started | File I/O pipeline, observability, MVP integration & acceptance metrics. |
| **M09–M11** | Not started | V2 UI (gutter, clipboard…), status bar & persistence, packaging. |
| **M12–M24** | Not started | V3: resize polish, workspace, sidebar, syntax, search, diff, git, AI, index, chat — see [`missions/00_V3_VISION.md`](missions/00_V3_VISION.md). |

Completing **M04–M08** in order is required before V2 missions become shippable; see [`FOLLOWUPS.md`](../FOLLOWUPS.md).

## Quality gates (local)

Run after substantive changes:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

Last verified: **2026-04-20** — `fmt`, `clippy -D warnings`, `test --workspace`, `build --release` succeeded after M05 input wiring and title-bar HUD.

## Reality check

Shipping **all** missions M00–M24 is a **multi-month / multi-quarter** program. The table above tracks **code that exists today** vs mission briefs under `docs/missions/`. Treat each mission document as the spec; treat this file as live status.
