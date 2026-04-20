# `docs/` Reference Tree

This directory holds the long-form reference documentation for the IDE
project. It is updated every mission. The root-level `README.md`,
`ARCHITECTURE.md`, and `TECH_STACK.md` are the canonical short summaries;
everything here is their supporting detail.

## Reading Order for New Contributors (or Agents)

If you are picking up this repository for the first time, read in this order:

1. **Root `README.md`** — what we're building and why.
2. **`ARCHITECTURE.md` (root)** — the system shape in one page.
3. **`TECH_STACK.md` (root)** — what we're building it with.
4. **`docs/AGENT_GUIDELINES.md`** — the standing orders every mission follows.
5. **`docs/PRD.md`** and **`docs/V2_PRD.md`** — product requirements.
6. **`docs/MVP_DEFINITION.md`** — what "done" means for the MVP.
7. **`docs/MISSIONS.md`** — the mission index, and which mission is in flight.
8. **`docs/STATUS.md`** — the current state.

Then read the subsystem references as you need them:

- **`PERFORMANCE_MODEL.md`** — the performance contract. Read before any hot-path work.
- **`TEXT_ENGINE.md`** — rope design, cursor math, undo/redo.
- **`RENDERING.md`** — wgpu pipeline, glyph atlas, dirty-rect strategy.
- **`INPUT_PIPELINE.md`** — raw OS events → editor operations.
- **`CONCURRENCY.md`** — thread model, ownership boundaries, channel topology.
- **`FILE_IO.md`** — async load, mmap, atomic save.
- **`CROSS_PLATFORM.md`** — Windows/Linux/macOS divergence and the CI matrix.
- **`OBSERVABILITY.md`** — tracing, metrics, dev overlay.
- **`TESTING.md`** — unit, integration, property, benchmark, stress strategy.
- **`RISKS.md`** — known gaps, mitigations, and parking lot.
- **`GLOSSARY.md`** — terminology.
- **`REFERENCES.md`** — external sources and prior art.

## Document Lifecycle Rules

- **Every mission may amend these documents**, but must do so in the same
  commit that changes the code they describe.
- **Never delete a document silently.** If something is superseded, replace
  its body with a link to the new doc and keep the file for history.
- **Every document ends with a `Last updated` stamp** and the mission ID that
  last touched it.
- **If a document grows past ~800 lines,** split it rather than letting it
  rot.

## Index

| File | Purpose |
|---|---|
| `AGENT_GUIDELINES.md` | Standing orders: the loop, git hygiene, quality gates. |
| `PRD.md` | Product Requirements Document (MVP). |
| `V2_PRD.md` | V2 PRD: the minimal useful editor layer on top of MVP. |
| `MVP_DEFINITION.md` | What the MVP is and is not; acceptance criteria. |
| `MISSIONS.md` | Ordered mission index (M00–M11) with scope summaries. |
| `STATUS.md` | Current mission state; updated at the end of every mission. |
| `PERFORMANCE_MODEL.md` | Frame budgets, latency targets, measurement methodology. |
| `TEXT_ENGINE.md` | Rope buffer, cursor, selection, undo/redo. |
| `RENDERING.md` | wgpu pipeline, glyph atlas, layout, dirty rects. |
| `INPUT_PIPELINE.md` | OS events, key mapping, IME, command dispatch. |
| `CONCURRENCY.md` | Threading, channels, ownership boundaries. |
| `FILE_IO.md` | Async I/O, mmap, atomic save, encodings. |
| `CROSS_PLATFORM.md` | Per-OS notes, CI matrix, packaging. |
| `OBSERVABILITY.md` | tracing, metrics, Criterion, dev overlay. |
| `TESTING.md` | Unit, integration, property, benchmark, stress testing. |
| `RISKS.md` | Known gaps and mitigations. |
| `GLOSSARY.md` | Terminology reference. |
| `REFERENCES.md` | External sources, prior art, relevant crates. |

---

*Last updated: M00 (Foundation Research & Documentation).*
