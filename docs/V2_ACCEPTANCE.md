[← docs/](./) · [MVP checklist](MVP_ACCEPTANCE.md) · [V2 PRD §9](../reference/05_V2_PRD.md)

# V2 acceptance report

Single checklist for **V2 “minimal useful editor”** closure. Rows combine:

- [`reference/05_V2_PRD.md`](../reference/05_V2_PRD.md) — §4–§9 (features, interaction, acceptance)
- [`docs/missions/M10_V2_WORD_NAV_STATUS_BAR_PERSISTENCE.md`](missions/M10_V2_WORD_NAV_STATUS_BAR_PERSISTENCE.md)
- MVP rows from [`docs/MVP_ACCEPTANCE.md`](MVP_ACCEPTANCE.md) — V2 must not regress (see **MVP carry-forward** below)

**Measurement methodology:** [`docs/PERFORMANCE_BUDGETS.md`](PERFORMANCE_BUDGETS.md), `docs/DIAGNOSING_PERFORMANCE.md`, CI + manual where noted.

---

## Executive summary

| Field | Value |
|--------|--------|
| **Report date** | 2026-04-20 |
| **Scope** | V2 features (M09–M10) + MVP NFRs that must hold |
| **Release approved** | **Partial** — code paths implemented; formal perf baselines and multi-OS measurement passes remain open (see yellow/N/A rows). |

**Status legend:** **✅** verified in code or CI · **⚠️** manual / measurement pending · **N/A** not yet measured

---

## V2 feature criteria ([`05_V2_PRD.md`](../reference/05_V2_PRD.md) §1, §9)

| # | Requirement | Evidence | Status |
|---|-------------|----------|--------|
| V2-01 | Line numbers in gutter | `editor-render` gutter path + frame prep | ✅ |
| V2-02 | Shift + arrow selection | `editor-input` + `main.rs` selection | ✅ |
| V2-03 | Clipboard copy/cut/paste | `arboard` + `EditorCommand` | ✅ |
| V2-04 | Undo/redo | `editor-core` undo stack + shortcuts | ✅ |
| V2-05 | Status bar: path, line/col, dirty, encoding, line ending | `editor-ui::StatusBarLayout` + `frame_status` | ✅ |
| V2-06 | Word motion + word delete (Ctrl/Opt + arrows; Ctrl/Opt + Backspace/Delete) | `editor-input::word_mod` + core word APIs | ✅ |
| V2-07 | Reopen last file + window geom + cursor/scroll via persisted `state.json` | `config::PersistedState`, `apply_persisted_view_state`, atomic save | ✅ |
| V2-08 | Async/deferred load restores cursor/scroll after worker load | `apply_loaded` reapplies persisted view after `replace_with_loaded` | ✅ |
| V2-09 | Persist on successful save, on quit, **and** every **60s** while dirty | `poll_io` save OK + `maybe_periodic_persist` in `paint_frame` | ✅ |
| V2-10 | F1 help overlay | `help_overlay` + `HELP_OVERLAY_TEXT` in `editor-app` | ✅ |
| V2-11 | CLI file argument overrides persisted last file | `resolve_initial_plan` | ✅ |

---

## Interaction & polish (M10)

| # | Requirement | Evidence | Status |
|---|-------------|----------|--------|
| INT-01 | Cursor byte clamped on restore | `restore_cursor_byte` + `min(len_bytes)` | ✅ |
| INT-02 | Scroll clamped to content | `clamp_scroll` after resume/resize/load | ✅ |
| INT-03 | `state.json` corrupt / bad version → defaults, no panic | `PersistedState::load` + tests | ✅ |

---

## MVP carry-forward (must stay green)

**Abbreviated** — full table: [`MVP_ACCEPTANCE.md`](MVP_ACCEPTANCE.md). High-signal rows:

| # | Requirement | Status |
|---|-------------|--------|
| NF-07 / PRD | Atomic save semantics | ✅ (implementation); crash injection **⚠️** manual |
| NF-08 | No panics in normal use | ✅ CI tests; stress **⚠️** |
| PRD-12e | Cross-platform build | ✅ CI matrix |
| DOC-01 | fmt / clippy / test / build | ✅ local + CI expectation |
| Perf NFRs (p99 latency, 60 fps, 100 MB, cold start) | Criterion / traces | **N/A** — re-verify per [`MVP_ACCEPTANCE.md`](MVP_ACCEPTANCE.md) |

---

## Manual / hardware (still required for “full green”)

| Checkpoint | Notes |
|------------|--------|
| Quit/relaunch restores file, caret, scroll | Exercise `state.json` on Windows + one Unix |
| Large file scroll/edit perf | Align with M08 budget when measured |
| Clean-VM install (M11) | MSI/dmg/deb/AppImage from release workflow |

---

## Sign-off

| Role | Name | Date | Signature |
|------|------|------|-------------|
| Implementation | — | 2026-04-20 | Code + docs updated |
| Performance baselines | — | — | Pending (`m08-mvp` / `m10-v2` Compare) |

---

*Next: fill **Measured** columns in [`MVP_ACCEPTANCE.md`](MVP_ACCEPTANCE.md) and perf docs when benches run; re-run this table after each release candidate.*
