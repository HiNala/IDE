# V3 acceptance report

**Status:** draft — fill measurements and pass/fail as M24 steps 2–10 complete.  
**Target release:** `v0.3.0` (see [`missions/M24_V3_ACCEPTANCE_RELEASE.md`](missions/M24_V3_ACCEPTANCE_RELEASE.md)).

---

## Summary

_(One paragraph: what shipped in V3, whether all six vision tests passed, and any release caveats.)_

---

## Hardware / environment

| Item | Value |
|------|--------|
| Machine | _TBD_ |
| OS | _TBD_ |
| Rust toolchain | _e.g. 1.94.1_ |
| Test workspace | _e.g. clean clone of this repo + fixture project_ |

---

## Baseline preservation

### MVP (M08)

Procedure: [`ACCEPTANCE.md`](ACCEPTANCE.md). Baseline compare: M24 expects `.github/baselines/m08-mvp.json` once baselines are checked in; until then use [`V2_ACCEPTANCE.md`](V2_ACCEPTANCE.md) / [`PERFORMANCE_BUDGETS.md`](PERFORMANCE_BUDGETS.md) as reference.

| Metric | Baseline | v0.3.0 | Delta | Pass |
|--------|----------|--------|-------|------|
| _fill from run_ | | | | |

**Blocker rule:** any regression &gt; 5% vs baseline requires investigation before release.

### V2 (post-M10)

Baseline compare: `.github/baselines/m10-v2.json` when present; otherwise [`V2_ACCEPTANCE.md`](V2_ACCEPTANCE.md).

| Metric | Baseline | v0.3.0 | Delta | Pass |
|--------|----------|--------|-------|------|
| _fill from run_ | | | | |

---

## V3 acceptance tests (six North Star checks)

Reference: [`missions/00_V3_VISION.md`](missions/00_V3_VISION.md) — “six tests that define V3 success.”

### Test 1 — Workspace open + edit

| Criterion | Target | Measured | Pass |
|-----------|--------|----------|------|
| File tree visible | ≤ 500 ms from launch | | |
| File open from tree | ≤ 50 ms | | |
| Syntax highlighting | visible | | |
| Input-to-pixel (p99) | &lt; 5 ms (M07 overlay) | | |

Notes: _hardware, build profile, methodology_

### Test 2 — Ctrl+P quick open

| Scenario | Query → results | Pass |
|----------|-----------------|------|
| ~100 files | &lt; 16 ms | |
| ~10,000 files | _record_ | |

### Test 3 — Project search (Ctrl+Shift+F)

| Metric | Target | Measured | Pass |
|--------|--------|----------|------|
| Time to first result | &lt; 200 ms | | |
| Navigate to match | file opens at line | | |

### Test 4 — AI chat end-to-end

| Path | Procedure | Pass |
|------|-----------|------|
| Anthropic (or equivalent) | Two-file edit prompt; review diffs; accept; verify buffers; single undo | |
| Ollama (local) | Same or simplified | |

### Test 5 — Sidecar generation

| Check | Pass |
|-------|------|
| `.ide/meta/...` exists after session; frontmatter + summary/history | |
| NoopSummarizer: skeleton sidecars, no crash | |

### Test 6 — Packaged installer first-launch

Requires `v0.3.0` artifacts from the M11 release workflow.

| OS | Installer | First launch / onboarding | Pass |
|----|-----------|---------------------------|------|
| Windows | _msi_ | | |
| macOS | _dmg/pkg_ | | |
| Linux | _.deb / AppImage_ | | |

---

## Known issues (non-blocking)

_List items suitable for `0.3.x` patches._

1. _TBD_

---

## Preflight checklist (M24 §1)

- [ ] Tags `m12-complete` … `m23-complete` present on `origin` (`git fetch --tags` then `git tag -l 'm*-complete'`).
- [ ] Quality gate: `cargo fmt`, `cargo clippy`, `cargo test`, `cargo build --release` (see M24 §2).
