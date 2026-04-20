[← docs/](./) · [README](../README.md)

# `docs/` Reference Tree

This directory holds the reference documentation for the IDE project. It is
updated every mission. `ARCHITECTURE.md` and `TECH_STACK.md` in this
directory are the canonical short summaries; everything else here is their
supporting detail. The verbatim product requirements live one level up in
`../reference/`.

## Reading Order for New Contributors (or Agents)

If you are picking up this repository for the first time, read in this order:

1. **Root `README.md`** — what we're building and why.
2. **`ARCHITECTURE.md`** (this directory) — the system shape in one page.
3. **`TECH_STACK.md`** (this directory) — what we're building it with.
4. **`../CONTRIBUTING.md`** — contributor contract.
5. **`AGENT_GUIDELINES.md`** — the standing orders every mission follows.
6. **`../reference/`** — verbatim product PRDs (source of truth).
7. **`PRD.md`**, **`V2_PRD.md`**, **`MVP_DEFINITION.md`** — our synthesis
   of the PRDs and acceptance criteria.
8. **`MISSIONS.md`** — the mission index, and which mission is in flight.
9. **`STATUS.md`** — the current state.

Then read the subsystem references as you need them:

- **`PERFORMANCE_BUDGETS.md`** — the performance contract. Read before any hot-path work.
- **`TEXT_ENGINE.md`** — rope design, cursor math, undo/redo.
- **`RENDERING_PIPELINE.md`** — wgpu pipeline, glyph atlas, dirty-rect strategy.
- **`INPUT_AND_IME.md`** — raw OS events → editor operations; IME flow.
- **`CONCURRENCY.md`** — thread model, ownership boundaries, channel topology.
- **`FILE_IO.md`** — async load, mmap, atomic save.
- **`CROSS_PLATFORM.md`** — Windows/Linux/macOS divergence and the CI matrix.
- **`OBSERVABILITY.md`** — tracing, metrics, dev overlay.
- **`TESTING_STRATEGY.md`** — unit, integration, property, benchmark, stress strategy.
- **`RUST_CONVENTIONS.md`** — coding style, error handling, logging rules.
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
| `ARCHITECTURE.md` | Canonical system architecture; one page. |
| `TECH_STACK.md` | Dependency choices, rationale, and locked versions. |
| `PERFORMANCE_BUDGETS.md` | Frame budgets, latency targets, measurement methodology. |
| `TEXT_ENGINE.md` | Rope buffer, cursor, selection, undo/redo. |
| `RENDERING_PIPELINE.md` | wgpu pipeline, glyph atlas, layout, dirty rects. |
| `INPUT_AND_IME.md` | OS events, key mapping, IME, command dispatch. |
| `CONCURRENCY.md` | Threading, channels, ownership boundaries. |
| `FILE_IO.md` | Async I/O, mmap, atomic save, encodings. |
| `CROSS_PLATFORM.md` | Per-OS notes, CI matrix, packaging. |
| `OBSERVABILITY.md` | tracing, metrics, Criterion, dev overlay. |
| `TESTING_STRATEGY.md` | Unit, integration, property, benchmark, stress testing. |
| `RUST_CONVENTIONS.md` | Coding style, error handling, logging rules. |
| `RISKS.md` | Known gaps and mitigations. |
| `GLOSSARY.md` | Terminology reference. |
| `REFERENCES.md` | External sources, prior art, relevant crates. |

---

*Last updated: M00 (Foundation Research & Documentation).*
