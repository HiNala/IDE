[← docs/](./) · [README](../README.md)

# Current status

**Last updated:** 2026-04-22

## Mission progress

| Range | State | Evidence |
|--------|--------|----------|
| **M00–M06** | **Done** | Text engine, GPU text, file I/O — see [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md). |
| **M07** | **Partial** | Metrics HUD, `tracing` / optional Tracy, perf-smoke scripts; Criterion PR gate TBD. |
| **M08** | **Partial** | [`MVP_ACCEPTANCE.md`](MVP_ACCEPTANCE.md), `gpu_resize_stress`; full measured rows TBD. |
| **M09–M10** | **Done (code)** | Gutter, selection, clipboard, status bar, persistence, word nav. |
| **M11** | **Partial** | Release workflow — [`RELEASING.md`](RELEASING.md). |
| **M12** | **Done (code); QA open** | Sync paint on `Resized` / `ScaleFactorChanged`, adaptive present mode, battery cap, deferred first show, icon, `--resize-telemetry`, row scratch + `gpu_resize_stress`, CI job **`m12-gpu-resize-windows`**. Remaining: baseline videos, logged p99 acceptance, git tag `m12-complete`. |
| **M13** | **Done (code)** | `editor-workspace` crate wired into `editor-app`: multi-buffer manager, MRU, tab strip, folder-open CLI arg, `notify`-backed FS events route to `external_modified`. Coherence tests in `crates/editor-workspace/tests/m13_coherence.rs`. |
| **M14** | **Done (code)** | Sidebar (`Ctrl+B` / `Ctrl+Shift+E`), quick-open palette (`Ctrl+P`) and tab strip chrome paint each frame via `FrameChrome`; mouse routing respects sidebar / tab / overlay zones; keyboard intercept while palette is visible. |
| **M15–M24** | **Varies** | Mission index and vision docs. V3 release ritual: [`missions/M24_V3_ACCEPTANCE_RELEASE.md`](missions/M24_V3_ACCEPTANCE_RELEASE.md); fill [`V3_ACCEPTANCE.md`](V3_ACCEPTANCE.md) as measurements land. |
| **M18** (light) | **Partial** | Status bar shows the `git` branch name via `editor-git::GitRepo::discover`; modified count + gutter markers deferred. |

**Row-by-row:** [`MISSION_IMPLEMENTATION_STATUS.md`](MISSION_IMPLEMENTATION_STATUS.md).

## Quality gates (local)

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build --release -p editor-app
```

Optional: `scripts/perf-smoke.ps1` / `perf-smoke.sh`; `scripts/resize-stress.ps1` / `resize-stress.sh`; `editor-app --dry-run`.
