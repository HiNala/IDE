# IDE Project — Mission Index & Standing Orders

**Repository:** https://github.com/HiNala/IDE
**Primary development OS:** Windows (10/11, x86_64)
**Target OS support:** Windows, Linux, macOS (all x86_64 and aarch64 where feasible)
**Core language:** Rust (stable)
**Mission count:** 29 (M00 through M28)
**Planned release tags:** `0.1.0-mvp` (M08) → `0.2.0-v2` (M10) → `0.2.1` packaged + consolidated (M25) → `0.3.0-v3` AI-native (M24)
**Current state:** As of 2026-04-20, M00-M10 shipped; M11-M13 partial; M14+ not started. See `00_STATE_2026_04_20.md` for the audit and the revised execution order (M25 runs before M14).

---

## What This Project Is

We are building a **ground-up, native, high-performance code editor** designed to eliminate the latency and bloat of Electron-based tools like VS Code and Cursor. The architecture is deliberately drawn from modern real-time engines rather than from traditional productivity applications. The editor uses Rust as the systems-level backbone, `winit` for windowing, `wgpu` for GPU-accelerated rendering, and a rope-based text engine for efficient document manipulation. Every subsystem operates under a strict per-frame performance budget, and the input-to-pixel path is deliberately kept as short and deterministic as the hardware allows.

The first set of missions (M00 through M08) builds the **MVP** — a minimal but extraordinarily fast editor that proves the architecture. The next set (M09 through M10) delivers **V2**, the smallest possible useful editor layer on top (line numbers, selection, clipboard, undo/redo, status bar). M11 handles release engineering so binaries can actually ship.

Missions M12 through M24 deliver **V3: the AI-native foundations**. The horizon is that by late 2026, most code in most projects will be written by coding agents — foundation-model API calls and locally-hosted open models — with humans in the loop for planning and review. Today's editors were retrofitted for this reality; V3 is designed for it from the beginning. V3 adds: polished foundations (resize, multi-buffer, file tree, syntax highlighting, search, diff, git), an AI provider abstraction (Anthropic, OpenAI, Gemini, Ollama, custom endpoints), a structured tool-use API so agents can navigate and edit the project safely through the same undo-aware transaction system a person uses, a novel **metadata sidecar system** that captures per-file reasoning so the expensive tokens spent by premium models accumulate into long-lived project knowledge rather than evaporating at session end, a local vector index for semantic retrieval, and a minimal integrated chat panel with per-hunk edit approval. See `00_V3_VISION.md` for the full V3 north star.

V4+ territory — autonomous background agents, plugin ecosystem, collaborative editing, LSP, debugger, inline autocomplete, remote editing — is explicitly out of scope for these missions. This mission set stops when V3 ships.

The repository is currently empty. Mission M00 begins with research and documentation; M01 scaffolds the workspace and makes the first push. Every subsequent mission builds on top of the previous ones and must leave the repository in a state where `cargo build`, `cargo test`, `cargo clippy`, and `cargo run` all succeed on Windows.

---

## Standing Orders (Apply To Every Mission)

These rules are **non-negotiable**. They apply to every single mission, every single TODO item, and every single commit. Re-read them whenever you feel uncertain about what to do next.

### 1. Do Not Stop

Do not stop until every TODO item in the mission is complete, validated, and committed. Do not stop until every TODO item in the mission is complete, validated, and committed. Do not stop until every TODO item in the mission is complete, validated, and committed.

If you hit a problem you cannot solve, do not stop — search the web, read the project's `/docs` folder, re-read the mission, re-read the supporting PRD documents, and keep going. Only report back when the entire mission is done or when you are genuinely blocked by something outside your control (and if that happens, document exactly what you tried first).

### 2. The Loop

For every TODO item, follow this loop. Do not shortcut it.

```
RESEARCH  →  Read the relevant /docs files and any referenced PRD sections.
              Search the web if the item touches an API you haven't used
              in this codebase yet. Quote or link the source in code comments
              when non-obvious.

PLAN      →  Write down (in your own thinking, or as a checklist) exactly
              what files you will change, what tests you will add, and what
              success looks like.

EXECUTE   →  Make the change. Write production-quality code. No stubs, no
              placeholder `todo!()` or `unimplemented!()` in shipped paths.

VALIDATE  →  Run `cargo fmt --all`, `cargo clippy --all-targets --all-features
              -- -D warnings`, `cargo test --all`, and (where applicable)
              `cargo bench` and `cargo run`. Everything must pass.

COMMIT    →  Commit with a Conventional Commits-style message. Scope the
              commit tightly — one logical change per commit.

PUSH      →  Push to origin after any meaningful group of commits. At a
              minimum, push at the end of every major TODO item.

REVIEW    →  Before moving to the next item, step back and ask: does this
              change move the project toward the North Star? Is there dead
              code, broken state, or hidden complexity I just introduced?
              If yes, fix it before moving on.
```

### 3. Think Holistically

If you discover something wrong that is outside the stated scope of the current mission — a broken import, a missing test, a typo in docs, a subtle bug in a previous mission's code, a missing `#[cfg]` branch that breaks Linux, a missed cross-platform consideration — **fix it**. Do not leave known broken things in the repository. The goal is not to complete the mission as narrowly as possible; the goal is to leave the project in a better state than you found it, end to end, every time.

That said, do not rewrite architecture unilaterally. If you see a design decision you disagree with, note it in a `FOLLOWUPS.md` file at the root of the repo and keep moving. Don't silently redesign things.

### 4. Git Hygiene

- The repository is at **https://github.com/HiNala/IDE**. If you are running M01 and the repo is empty, initialize and push. If you are running any later mission, pull first.
- Use **Conventional Commits** format: `feat(text-engine): implement rope-based line iteration`, `fix(render): correct DPI scaling on high-density displays`, `docs(architecture): document frame-loop phases`, `test(file-io): add atomic-save property test`, etc.
- Commit logical units, not file dumps. A commit should compile and pass tests on its own.
- **Push regularly.** At minimum: after each major TODO item, at the end of every mission, and any time you finish something that another developer would want to pull.
- Never commit `target/`, `Cargo.lock` for libraries (do commit it for the final binary crate), editor tempfiles, or secrets.

### 5. Quality Gates (Every Commit Must Pass)

Before `git commit`, the following must all succeed locally:

1. `cargo fmt --all -- --check` — formatting is clean.
2. `cargo clippy --all-targets --all-features -- -D warnings` — no lint warnings.
3. `cargo test --all` — all tests pass.
4. `cargo build --release` — release build compiles.
5. The application still boots (`cargo run --release` opens a window without crashing) on Windows. Verify this at least once per mission end, even if not strictly necessary for every commit.

If any gate fails, fix it before committing. Not after.

### 6. Windows-First, Cross-Platform-Always

Primary development is on Windows. CI runs on Windows, Linux, and macOS. Any code that touches the filesystem, windowing, graphics backend selection, input, clipboard, or process management must be written with all three platforms in mind from the start. Use `#[cfg(target_os = "...")]` guards explicitly when behavior must differ. Do not paper over platform differences with `unwrap()` on one platform and expect the others to work.

Paths: always use `std::path::PathBuf` / `Path`. Never concatenate path strings with `/`. Normalize line endings at the I/O boundary (store LF internally, write platform-appropriate on save unless user configured otherwise).

### 7. Performance Is A Feature, Not An Afterthought

This editor's entire reason for existing is performance. Every mission must be evaluated against the PRD's hard targets:

- **Input-to-pixel latency:** under 5 ms under normal load.
- **Frame rate:** 60 fps minimum during scrolling and editing; target 120 fps where hardware supports it.
- **Memory:** bounded growth; must remain stable over multi-hour editing sessions.
- **Startup time:** under 1 second cold start on modern hardware.
- **Large file handling:** open a 100 MB text file without blocking the UI.

If a change regresses any of these, the change is wrong. Use Criterion benchmarks to prove performance; do not rely on "feels fast enough."

### 8. Research Before Building

Before implementing anything new, check three sources in this order:

1. The project's own `/docs` folder (created in M00 and updated throughout).
2. The referenced PRD documents (MVP PRD, V2 PRD, Architecture, Tech Stack, Gaps/Risks).
3. The open web — current crate docs (`docs.rs`), crate READMEs, Zed/Lapce/Helix source code on GitHub, and current best-practice blog posts from 2025-2026.

If you encounter a new dependency, read its changelog before adopting it, and pin a minor version in `Cargo.toml`. Do not `cargo add foo` without justifying the choice in a commit message or docs update.

### 9. Test, Bench, and Boot

"It compiles" is not "it works." Before declaring any TODO item done:

- Write unit tests for new pure logic (the rope engine, cursor math, path normalization, etc.).
- Write integration tests for subsystem boundaries (file load → rope → render).
- Add Criterion benchmarks for any hot path.
- Actually run `cargo run` and interact with the editor. Open a file, type, scroll, resize the window, close it.
- For any subsystem that has external inputs (keyboard, mouse, files), add at least one property-based test using `proptest`.

Stress tests belong to specific missions (notably M08), but every subsystem gets its own targeted tests in its owning mission.

### 10. Docker Is Not Needed

We explicitly do not use Docker for this project. The editor is a native binary distributed as a standalone executable. Docker adds cost without value for a local GUI application. The only scenario where a container could be useful is cross-compiling Linux binaries from Windows during release (M11), and even there, `cross` (which uses Docker internally but hides it) is sufficient — no hand-written Dockerfiles required.

### 11. No Scope Creep In The MVP

The MVP (M00-M08) is intentionally minimal. Do not add syntax highlighting, LSP, AI, plugins, themes, tabs, project trees, or any other IDE feature during the MVP missions, no matter how tempting. The V2 missions (M09-M10) add the small set of features in the V2 PRD. Anything beyond that belongs in a future mission set, not in these.

### 12. When You Finish A Mission

At the end of every mission, do this:

1. Run the full quality gate: `cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all && cargo build --release && cargo run --release` (the last closes quickly after you confirm it boots).
2. Update `ARCHITECTURE.md` or `TECH_STACK.md` if the mission changed the shape of either.
3. Update `CHANGELOG.md` under an `## [Unreleased]` heading.
4. Update `docs/STATUS.md` with what's done and what's next.
5. Tag the commit: `git tag -a mXX-complete -m "Mission XX complete: <summary>"`.
6. Push the tag: `git push origin mXX-complete`.
7. Write a brief "Mission Complete" summary at the bottom of the mission's own doc file (if the user keeps mission files in-repo) or at the bottom of `docs/STATUS.md`.

---

## Execution Order

Run missions in this order. Each mission assumes the previous one is complete. Release candidates are tagged at M08, M10, M11, and M24.

### MVP (M00-M08) — architecture proven

| # | Mission ID | Title | Key Output |
|---|---|---|---|
| 00 | `M00` | Foundation Research & Documentation | `docs/` folder with 10+ reference files |
| 01 | `M01` | Repo Scaffolding, Workspace, Toolchain, CI | Multi-crate workspace, CI green on 3 OSes |
| 02 | `M02` | Text Engine: Rope Buffer, Cursor, Undo/Redo | `editor-core` crate with benchmarks |
| 03 | `M03` | Windowing & wgpu Rendering Foundation | Window opens; clear color renders via GPU |
| 04 | `M04` | Text Rendering with glyphon | Visible text renders from the rope |
| 05 | `M05` | Frame Loop, Input Pipeline, Performance Budgets | Typing works at <5 ms latency |
| 06 | `M06` | File I/O: Async Load, mmap, Atomic Save | Open & save large files without blocking |
| 07 | `M07` | Observability, Profiling, Dev Overlay | Per-frame metrics visible in dev mode |
| 08 | `M08` | MVP Integration, Stress Testing, Acceptance | PRD performance targets all met → `0.1.0-mvp` |

### V2 (M09-M10) — minimal usable editor

| # | Mission ID | Title | Key Output |
|---|---|---|---|
| 09 | `M09` | V2: Line Numbers, Selection, Clipboard, Undo UI | Usable minimal editor |
| 10 | `M10` | V2: Word Nav, Status Bar, Persistence, Polish | V2 acceptance → `0.2.0-v2` |

### Packaging + Foundation Consolidation (M11 + M25) — public release + audit sweep

| # | Mission ID | Title | Key Output |
|---|---|---|---|
| 11 | `M11` | Release Engineering & Cross-Platform Packaging | Installer plan, workflow scaffolding |
| 25 | `M25` | Critical Fixes & Foundation Completion | Completes M11-M13; sweeps audit; ships `0.2.1` |

### V3 (M12-M28) — AI-native foundations

See `00_V3_VISION.md` for the V3 north star and `00_STATE_2026_04_20.md` for the concrete sequencing notes.

Execute in this order (revised 2026-04-20):

| # | Mission ID | Title | Key Output |
|---|---|---|---|
| 12 | `M12` | Window, Resize, DPI: Snappy Response Polish | Instant resize, multi-monitor (folded into M25) |
| 13 | `M13` | Workspace Model & Multi-Buffer Foundation | `editor-workspace` crate (completion folded into M25) |
| 14 | `M14` | Sidebar, Tabs, Quick Open | Navigable project UI |
| 15 | `M15` | Syntax Highlighting via Tree-sitter | Colored code for 6 core languages |
| 16 | `M16` | Find & Replace (In-File + Project-Wide) | Ctrl+F, Ctrl+H, Ctrl+Shift+F |
| 17 | `M17` | Diff Engine & Inline Renderer | Red/green hunk rendering |
| 18 | `M18` | Git Integration Baseline | File status, branch, diff-vs-HEAD |
| 26 | `M26` | Integrated Terminal | Native PTY + VT emulator pane |
| 19 | `M19` | AI Provider Abstraction (OpenAI first) | Multi-provider streaming + tool use |
| 27 | `M27` | AI Skills System | Progressive-disclosure instruction set |
| 20 | `M20` | Agent Tool-Use API & Safe Edit Transactions | Structured surface for LLMs to act |
| 28 | `M28` | Settings UI & API Key Management | World-class minimal settings surface |
| 21 | `M21` | Metadata Sidecar System | Per-file reasoning capture |
| 22 | `M22` | Local Vector Index & Semantic Retrieval | Fast context assembly |
| 23 | `M23` | AI Chat Panel & Edit-Approval Flow | Minimal integrated experience |
| 24 | `M24` | V3 Acceptance & Release | Tagged `0.3.0-v3` with installers |

---

## North Star

**For MVP + V2 (M00 through M11):** at the end of M11, a user on Windows, Linux, or macOS can download a single installer or binary, double-click it, be editing a multi-megabyte text file in under one second, and feel a zero-lag typing experience that is measurably, provably, and consistently better than VS Code and Cursor. The architecture underneath that experience is clean enough that syntax highlighting, LSP, and AI integration can be added in future mission sets without rewriting the core.

**For V3 (M12 through M24):** at the end of M24, that same editor is also the first local-first, AI-native IDE whose architecture treats human and agent equally — every affordance a person uses (navigate, edit, search, diff) has a symmetric tool an LLM can invoke, every agent edit flows through the same undo-aware transaction a human edit does, a novel per-file metadata sidecar system turns the tokens spent on reasoning into long-lived project knowledge, and a local vector index makes "what's relevant to this prompt?" cheap. Cursor and Windsurf retrofit AI onto VS Code; we build the first editor where the agent is a first-class citizen from the ground up.

Every mission, every commit, every line of code should move toward one of those two moments. When in doubt, ask: "Does this move us toward the applicable North Star?" If the answer is no, stop and reconsider.

---

## Supporting PRD Documents (Reference Material)

The project is backed by six PRD documents that define the product, architecture, and non-functional constraints. Every mission references these. Agents should read them at least once before starting M00:

1. **Product Requirements Document** — MVP vision and constraints.
2. **Tech Stack & Architecture Choices** — Rust, winit, wgpu, rope, concurrency model.
3. **Architecture Strategy & Performance Model** — Frame-based execution, budgets, deterministic state.
4. **Gaps, Risks, and Missing Considerations** — Cross-platform, IME, data integrity, etc.
5. **MVP Definition** — What the MVP is and is not.
6. **V2 PRD** — The minimal-useful-editor layer built on top of the MVP.

For V3 (M12 onward), additionally read:

7. **`00_V3_VISION.md`** — the AI-native IDE horizon, the mental model for V3, and explicit out-of-scope items for V4+.
8. **`00_STATE_2026_04_20.md`** — current state snapshot, audit findings, and the concrete M25-M28 sequencing that supersedes a naive M12→M24 walk.

These documents are the source of truth for *what* we are building and *why*. The mission documents are the source of truth for *how* to build it, step by step.

---

## Final Word

These missions are written as plain-English instructions because agents interpret them as plain English. Be disciplined. Do not skip steps. Do not take shortcuts. Do not leave the repo in a broken state at the end of any session. The quality bar is extremely high: we are competing with a decade of VS Code optimization and a multi-million-dollar company (Zed Industries) that ships the closest thing to what we are building. We win by being disciplined, by measuring everything, and by never cutting corners on the core interaction loop.

Now go build it.
