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
| **M06** | Mostly done | `editor-io`: `load_file_sync` (mmap ≥10 MiB + read fallback), UTF-8/BOM/UTF-16 decode, `save_file_sync` (temp+rename), CRLF/LF/CR on save, Windows reserved names. `editor-app`: Ctrl+S / Ctrl+O (`rfd`), worker pool loads/saves, dirty + disk mtime / external-change on focus, session `state.json`. Benchmarks/stress tests from mission checklist still optional. |
| **M07–M08** | Partial | Tracy optional; formal MVP acceptance metrics / soak tests still open. |
| **M09** | Partial | Typing, undo/redo, save/load, IME, resize — see code; formal V2 checklist (gutter, selection, clipboard) vs `missions/M09_*.md` still needs sign-off. |
| **M10** | Mostly done | `editor-ui` status line strings; `editor-render` bottom status bar + clipped body; `config::PersistedState` (`state.json`), restore cursor/scroll/window, `exiting` + close persist; scroll math accounts for status height. |
| **M11** | Not started | Installers / GitHub Releases. |
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

Last verified: **2026-04-20** — `fmt`, `clippy -D warnings`, `test --workspace` succeeded after M10 status bar + session persistence wiring.

## Reality check

Shipping **all** missions M00–M24 is a **multi-month / multi-quarter** program. The table above tracks **code that exists today** vs mission briefs under `docs/missions/`. Treat each mission document as the spec; treat this file as live status.
