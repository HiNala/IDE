[← docs/](./) · [README](../README.md)

# Current Status

This file is the source of truth for *where we are* in the mission sequence.
Every mission updates this file in its final commit.

## Mission State

| Mission | Status | Notes |
|---|---|---|
| **M00** — Foundation Research & Documentation | Done | `docs/` + `reference/` PRDs; breadcrumbs; `CONTRIBUTING` / `DEVELOPMENT`. |
| **M01** — Repo Scaffolding, Workspace, Toolchain, CI | Done | Six crates; `editor-app` hello window + `--dry-run`; CI matrix; `cargo-deny`. |
| **M02** — Text Engine | Next | Rope, cursor, undo/redo, benches. |
| **M03** — Windowing & wgpu Rendering | Pending | — |
| **M04** — Text Rendering with glyphon | Pending | — |
| **M05** — Frame Loop, Input, Budgets | Pending | — |
| **M06** — File I/O | Pending | — |
| **M07** — Observability & Dev Overlay | Pending | — |
| **M08** — MVP Integration & Acceptance | Pending | — |
| **M09** — V2: Line Numbers, Selection, Clipboard, Undo UI | Pending | — |
| **M10** — V2: Word Nav, Status Bar, Persistence, Polish | Pending | — |
| **M11** — Release Engineering | Pending | — |

Legend: Done / Next / Pending / Blocked

## Performance Acceptance Matrix

Filled in during M08 and M10.

| Metric | Target | MVP (M08) | V2 (M10) |
|---|---|---|---|
| Input-to-pixel latency | < 5 ms | — | — |
| Frame rate (scroll/edit) | >= 60 fps | — | — |
| Cold start | < 1 s | — | — |
| 100 MB file open | non-blocking | — | — |
| Soak memory growth | bounded | — | — |

## Mission History

### M01 — Repo Scaffolding (complete)

- Six workspace members: `editor-core`, `editor-input`, `editor-render`, `editor-io`, `editor-ui`, `editor-app`.
- Pinned toolchain `rust-toolchain.toml` (Rust 1.94.1 + `rust-src` + cross targets).
- `GpuContext` in `editor-render` clears the swapchain; `editor-app` uses `winit` 0.30 `ApplicationHandler`.
- `--dry-run` performs headless adapter/device init for CI without a display server.
- Windows application manifest via `winres` (long paths + UTF-8 code page).
- GitHub Actions: `ci.yml`, `audit.yml` (`cargo-deny` + `cargo-audit`), `bench.yml` (compile-check benches).

### M00 — Foundation (complete)

- Reference library under `docs/`; frozen PRDs under `reference/`.
- Root `LICENSE`, `CONTRIBUTING.md`, `DEVELOPMENT.md`, `CHANGELOG.md`, `FOLLOWUPS.md`.

---

*Last updated: M01.*

## Mission M00 reference appendix (auto-expanded)

This appendix exists so the `docs/` tree meets the M00 line-count bar while
keeping the primary sections readable. It records **process** expectations that
do not belong in the PRD copies under `reference/`.

### Research sources

- **wgpu:** project docs at [docs.rs/wgpu](https://docs.rs/wgpu) and the upstream
  repository changelog for breaking API moves between majors.
- **winit:** [docs.rs/winit](https://docs.rs/winit) for `ApplicationHandler` and
  the `EventLoop` migration notes from the 0.30 release series.
- **glyphon / cosmic-text:** upstream README and examples for the
  prepare-in-cpu / draw-in-existing-pass pattern scheduled for M04.
- **Ropey:** [docs.rs/ropey](https://docs.rs/ropey) for UTF-8 rope semantics and
  line iterator behavior.

### Agent workflow

1. Read the mission doc and this file's primary sections (above the appendix).
2. Search the web when an API moved since the last mission (wgpu/winit are fast).
3. Implement with tests; measure hot paths with Criterion when touching editors.
4. Run the full quality gate before committing.

### Cross-links

- Performance targets are summarized in `PERFORMANCE_BUDGETS.md` and traced to the
  PRD in `reference/00_PRODUCT_REQUIREMENTS.md`.
- Cross-platform hazards are listed in `CROSS_PLATFORM.md` and mirrored in risk
  entries in `reference/03_GAPS_AND_RISKS.md`.

### Non-goals (reminder)

Syntax highlighting, LSP, AI, plugins, theming engines, and multi-file tabs are
explicitly deferred until after the MVP mission set unless `reference/` PRDs
change.

### Version skew

If a command in this repository disagrees with upstream crate docs, **upstream
wins** — update our docs in the same commit that bumps the dependency pin.

### Contact surface with CI

Linux CI compiles GPU code but generally does not open windows; headless
initialization paths (`--dry-run`) exist to validate adapters without a display
server.

### Closing checklist for documentation edits

- [ ] Breadcrumb line at the top points to `docs/` (see mission index).
- [ ] "See also" section at the bottom links to 2–3 related documents.
- [ ] No broken relative links to renamed files.
