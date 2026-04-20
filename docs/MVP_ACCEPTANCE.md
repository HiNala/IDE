[← docs/](./) · [README](../README.md)

# MVP acceptance report

This document is the **single checklist** for MVP release. Requirements are merged from:

- `reference/04_MVP_DEFINITION.md` — §5 (Non-Functional Requirements), §8 (Acceptance Criteria), §6 (Definition of Completion)
- `reference/00_PRODUCT_REQUIREMENTS.md` — §2 (Philosophy), §12–§14 (scope and NFRs)

**Measurement methodology:** `docs/PERFORMANCE_BUDGETS.md`, `docs/TESTING_STRATEGY.md`.

---

## Executive summary

| Field | Value |
|---|---|
| **Report date** | 2026-04-20 |
| **Codebase state** | Missions **M00–M01** implemented (workspace + hello window + `--dry-run`). Editing, file I/O, text rendering, and observability missions **not yet complete**. |
| **Requirements total** | 28 rows below |
| **Green** | 3 (CI + build sanity only) |
| **Yellow / N/A** | 25 (blocked on M02–M07 or manual hardware runs) |
| **Red** | 0 |
| **Release approved** | **No** — acceptance cannot be completed until M02–M07 deliver the editor features and measurements this checklist targets. |

**Next step:** Complete missions **M02 → M07**, then re-run this document: fill **Measured**, set **Status** to ✅ or ❌, and repeat until **Release approved: Yes** or documented ⚠️ with a `FOLLOWUPS.md` entry.

---

## Canonical checklist

Status legend: **✅** pass · **❌** fail · **⚠️** caveat (see notes) · **N/A** not yet measurable

| # | Requirement | Source | Test / evidence | Target | Measured | Status |
|---|-------------|--------|-----------------|--------|----------|--------|
| NF-01 | Input-to-pixel latency p99 | MVP §5 | Criterion + injected events; `RUST_LOG` span timings | < 5 ms p99 normal load | — | N/A |
| NF-02 | Scroll frame rate | MVP §5 / PRD §13 | Scripted scroll on 100 MB doc, frame trace | ≥ 60 fps (dev box) | — | N/A |
| NF-03 | Edit frame rate | MVP §5 / PRD §13 | Typing macro + frame trace | ≥ 60 fps | — | N/A |
| NF-04 | Cold start | MVP §5 / PRD §13 | Wall clock: process start → first interactive frame | < 1 s | — | N/A |
| NF-05 | 100 MB open interactive | MVP §5 | Open 100 MB file; UI responsive, no sustained drops | No UI stall | — | N/A |
| NF-06 | Long-session memory | MVP §5 | RSS sampled every 60 s over 4 h edits | Growth < 10 % | — | N/A |
| NF-07 | Data safety (atomic save) | MVP §5 | Crash-injection during save; disk state | Zero lost committed saves | — | N/A |
| NF-08 | Crash-free acceptance | MVP §5 | CI + stress suite; no panic | Zero panics | — | N/A |
| AC-01 | Open 100 MB in &lt; 500 ms wall | MVP §8 | `editor-io` bench + manual timer | &lt; 500 ms | — | N/A |
| AC-02 | Keystroke p50 / p99 | MVP §8 | Criterion injected keystrokes | p50 &lt; 3 ms, p99 &lt; 5 ms | — | N/A |
| AC-03 | Scroll 100 MB @ 60 fps | MVP §8 | Scripted scroll, frame meter | ≥ 60 fps | — | N/A |
| AC-04 | Cold start &lt; 1 s | MVP §8 | External timer on SSD | &lt; 1 s | — | N/A |
| AC-05 | 4-hour memory macro | MVP §8 | RSS log every 60 s | Bounded per NF-06 | — | N/A |
| AC-06 | Save survives `kill -9` mid-write | MVP §8 | Crash injection test | Prior state or complete file | — | N/A |
| AC-07 | No panics in acceptance | MVP §8 | CI stderr + stress | None | — | N/A |
| AC-08 | CI green all OSes | MVP §8 | GitHub Actions | Green | **CI matrix present** | ✅ |
| PRD-2a | Sub-5 ms input-to-pixel (philosophy) | PRD §2 | Same as NF-01 | &lt; 5 ms | — | N/A |
| PRD-2b | Never block RT loop for background work | PRD §2 | Code review + tracing | No long waits on main thread | — | N/A |
| PRD-2c | Bounded memory | PRD §2 | Soak + RSS | Bounded | — | N/A |
| PRD-12a | Open file from disk | PRD §12 | Manual + CLI | Works | — | N/A |
| PRD-12b | View/edit with cursor | PRD §12 | Functional test | Works | — | N/A |
| PRD-12c | Smooth scroll large doc | PRD §12 | 100 MB scroll | Smooth | — | N/A |
| PRD-12d | Atomic save | PRD §12 | Integration + injection | Correct bytes on disk | — | N/A |
| PRD-12e | Cross-platform | PRD §12 | CI + manual | Win / Linux / macOS | **CI builds** | ✅ |
| PRD-13a | NFR table (latency, fps, cold, 100MB, memory) | PRD §13 | Mapped to NF / AC rows | See targets | — | N/A |
| PRD-14 | Success vs VS Code / Cursor | PRD §14 | Side-by-side benchmarks | Faster on agreed metrics | — | N/A |
| DOC-01 | `cargo fmt / clippy / test / bench / build` | MVP §6 | Local + CI | All pass | **fmt/clippy/test/build in CI** | ✅ |
| REL-01 | Release binary runs on each OS | M08 RC | Manual smoke | Opens, closes | — | N/A |

---

## Stress tests (M08 mission list)

The following are **specified** in the M08 mission; implementation is **pending** until upstream features exist.

| Suite | Description | Status |
|-------|-------------|--------|
| Large file 1–2 GB | Open/scroll/save timings on real hardware | Not run — no large-file path |
| Long session 1 h | 1M scripted commands, RSS + p99 drift | Not run — no `EditorState` harness |
| Adversarial proptest | Random `EditorCommand` sequences | Not run — no `EditorState` |
| Rapid resize / DPI | Manual + automated `resize` loop | Partial — `GpuContext::resize` exists; no 100-iter automated GPU test in CI |
| Fast typing | 10k chars/s + realistic 15 c/s | Not run |
| Save/load races | Cancel / ordering | Not run — async I/O not integrated |
| External file change | Detection on focus | Not run |

---

## Polish items (M08)

| Item | Status |
|------|--------|
| Banner / GPU lost / save-load errors | **Pending** — requires `TextLayer` or overlay (M04/M07) |
| Window title: file name + `*` dirty | **Pending** — requires buffer + path state |
| Exit codes (0 / 2 / 64) | **Partial** — see `DEVELOPMENT.md` §8 |
| `--perf-smoke` | **Pending** — script + flag (M07) |

---

## Sign-off

| Role | Name | Date | Approved |
|------|------|------|----------|
| Engineering | _pending_ | — | No |

---

*This file must be updated in the same commit as any claim of MVP readiness.*
