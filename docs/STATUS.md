[← docs/](./) · [README](../README.md)

# Current status

**Last updated:** 2026-04-20

## Mission progress

| Range | State | Evidence |
|--------|--------|----------|
| **M00–M06** | **Done** | Text engine, GPU text, async file I/O — see [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md). |
| **M07–M08** | **Partial** | Metrics HUD + `gpu_resize_stress` test; full PRD **Measured** columns in [`MVP_ACCEPTANCE.md`](MVP_ACCEPTANCE.md) still mostly manual/bench. |
| **M09–M10** | **Done (code)** | Gutter, selection, clipboard, undo, status bar, `state.json`, word navigation. Release tagging / formal V2 sign-off tracked in acceptance docs. |
| **M11** | **Partial** | Semver tag workflow builds MSI / DMG / deb / AppImage + checksums ([`release.yml`](../.github/workflows/release.yml)); optional signing via secrets — [`RELEASING.md`](RELEASING.md). |
| **M12** | **Partial** | Resize/DPI path + present mode + sync paint + non-wrapped line cache + battery cap + deferred show + PNG icon + `--resize-telemetry`; scripted CI stress / 8K prealloc / full checklist still open. |
| **M13** | **Done (code)** | Multi-buffer + [`editor-workspace`](../crates/editor-workspace/README.md) integrated in `editor-app`. |
| **M14** | **Partial** | `Ctrl+P` quick-open + fuzzy match + overlay in `editor-app`; sidebar + tab strip UI not built yet. |
| **M15–M24** | **Not started** | Syntax, search, diff, git, AI stack — see [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md). |

**Detailed row-by-row:** [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md).

**Running M12+ in dependency order:** [`missions/SEQUENTIAL_EXECUTION_NOTES.md`](missions/SEQUENTIAL_EXECUTION_NOTES.md).

## Quality gates (local)

Run after substantive changes:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --release
```

Optional: `cargo bench -p editor-core`, `editor-app --dry-run`.

Last verified: **2026-04-20** — `cargo fmt --check`, `clippy --workspace --all-targets --all-features --locked -- -D warnings`, `test --workspace --all-features --locked`, `build --release -p editor-app --locked`; tracing spans on `load_file_sync` / `save_file_sync` / `apply_edit`; `--dev-hud` CLI.

## Canonical specs

- Mission index: [`missions/00_MISSION_INDEX.md`](missions/00_MISSION_INDEX.md)
- V3 vision: [`missions/00_V3_VISION.md`](missions/00_V3_VISION.md)
- Short index: [`MISSIONS.md`](MISSIONS.md)
