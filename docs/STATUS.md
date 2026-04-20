[← docs/](./) · [README](../README.md)

# Current status

**Last updated:** 2026-04-20

## Mission progress

| Range | State | Evidence |
|--------|--------|----------|
| **M00–M06** | **Done** | Text engine, GPU text, async file I/O — see [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md). |
| **M07–M08** | **Partial** | Metrics / dev HUD; full MVP acceptance measurements still pending. |
| **M09–M10** | **Done (code)** | Gutter, selection, clipboard, undo, status bar, `state.json`, word navigation. Release tagging / formal V2 sign-off tracked in acceptance docs. |
| **M11** | **Partial** | Tag-triggered GitHub Releases with unsigned binaries (`release.yml`); installers & signing still open — [`RELEASING.md`](RELEASING.md). |
| **M12** | **Partial** | DPI/resize partially wired; full M12 polish pending. |
| **M13–M24** | **Not started** | V3 track: workspace, UI chrome, syntax, search, git, AI — no `editor-workspace` / agent crates yet. |

**Detailed row-by-row:** [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md).

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
