# Current Status

This file is the source of truth for *where we are* in the mission sequence.
Every mission updates this file in its final commit.

## Mission State

| Mission | Status | Notes |
|---|---|---|
| **M00** — Foundation Research & Documentation | ✅ Complete | Docs tree, root files, git init, first push. |
| **M01** — Repo Scaffolding, Workspace, Toolchain, CI | ⏳ Up Next | — |
| **M02** — Text Engine | ⏳ Pending | — |
| **M03** — Windowing & wgpu Rendering | ⏳ Pending | — |
| **M04** — Text Rendering with glyphon | ⏳ Pending | — |
| **M05** — Frame Loop, Input, Budgets | ⏳ Pending | — |
| **M06** — File I/O | ⏳ Pending | — |
| **M07** — Observability & Dev Overlay | ⏳ Pending | — |
| **M08** — MVP Integration & Acceptance | ⏳ Pending | — |
| **M09** — V2: Line Numbers, Selection, Clipboard, Undo UI | ⏳ Pending | — |
| **M10** — V2: Word Nav, Status Bar, Persistence, Polish | ⏳ Pending | — |
| **M11** — Release Engineering | ⏳ Pending | — |

Legend: ✅ complete · 🚧 in progress · ⏳ not started · ⚠ blocked

## Performance Acceptance Matrix

Filled in during M08 and M10. Currently empty because there is no code yet.

| Metric | Target | MVP (M08) | V2 (M10) |
|---|---|---|---|
| Input-to-pixel latency | < 5 ms | — | — |
| Frame rate (scroll/edit) | ≥ 60 fps | — | — |
| Cold start | < 1 s | — | — |
| 100 MB file open | non-blocking | — | — |
| Soak memory growth | bounded | — | — |

## Known Follow-Ups

See `FOLLOWUPS.md` at the repo root. Empty at M00.

## Mission History

### M00 — Foundation Research & Documentation (complete)

- Created root files: `README.md`, `ARCHITECTURE.md`, `TECH_STACK.md`,
  `CHANGELOG.md`, `LICENSE-APACHE`, `LICENSE-MIT`, `.gitignore`.
- Created `docs/` reference tree (18 files including this one):
  `README.md`, `AGENT_GUIDELINES.md`, `MISSIONS.md`, `STATUS.md`,
  `PRD.md`, `V2_PRD.md`, `MVP_DEFINITION.md`, `PERFORMANCE_MODEL.md`,
  `TEXT_ENGINE.md`, `RENDERING.md`, `INPUT_PIPELINE.md`,
  `CONCURRENCY.md`, `FILE_IO.md`, `CROSS_PLATFORM.md`,
  `OBSERVABILITY.md`, `TESTING.md`, `RISKS.md`, `GLOSSARY.md`,
  `REFERENCES.md`.
- Initialized git on `main`, pointed origin at
  `https://github.com/HiNala/IDE.git`, pushed initial commits, tagged
  `m00-complete`.

---

*Last updated: M00.*
