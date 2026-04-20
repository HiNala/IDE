# Agent Guidelines — Standing Orders

These are the **non-negotiable** standing orders for every agent (human or
AI) contributing to this repository. They apply to every mission, every TODO,
and every commit. Re-read them whenever you feel uncertain about the next
step.

---

## 1. Do Not Stop

Do not stop until every TODO item in the current mission is complete,
validated, and committed.

If you hit a problem you cannot solve:

1. Re-read the mission spec.
2. Re-read the relevant files under `docs/`.
3. Re-read the root `ARCHITECTURE.md` and `TECH_STACK.md`.
4. Search the web (crate docs, changelogs, prior art such as Zed/Lapce/Helix).
5. Keep going.

Only report back as "blocked" if a dependency outside the repository (an API
you cannot reach, a tool you cannot install, a question only the owner can
answer) stops you. When blocked, **document exactly what you tried** in
`FOLLOWUPS.md`.

## 2. The Loop

Every TODO follows this loop. Do not shortcut it.

```
RESEARCH  →  Read the relevant docs/ files and any referenced PRDs.
              Search the web if the item touches an API you haven't used
              in this codebase yet. Cite sources in commit messages or
              code comments when non-obvious.

PLAN      →  Write down (even briefly) exactly what files you will change,
              what tests you will add, and what success looks like.

EXECUTE   →  Make the change. Production-quality code only. No stubs,
              no `todo!()`/`unimplemented!()` in shipped paths.

VALIDATE  →  Run the full quality gate (see §5).

COMMIT    →  Conventional Commits, one logical change per commit.

PUSH      →  Push to origin after any meaningful group of commits,
              at minimum at the end of every major TODO and every mission.

REVIEW    →  Ask yourself: did this move the project toward the North
              Star? Did I leave dead code, broken state, or new hidden
              complexity? Fix it before moving on.
```

## 3. Think Holistically

If you discover something wrong outside the stated scope of the current
mission — a broken import, a missing test, a typo in docs, a subtle bug from a
previous mission, a missing `#[cfg]` branch that breaks Linux, a missed
cross-platform consideration — **fix it**. Do not leave known broken things
in the repository.

That said, **do not rewrite architecture unilaterally.** If you see a design
decision you disagree with, add an entry to `FOLLOWUPS.md` at the repo root
and keep moving. Do not silently redesign things.

## 4. Git Hygiene

- **Remote:** `https://github.com/HiNala/IDE.git`.
- **Default branch:** `main`.
- **Commit format:** [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).
  Examples:
  - `feat(editor-core): implement rope-based line iteration`
  - `fix(editor-render): correct DPI scaling on high-density displays`
  - `docs(architecture): document frame-loop phases`
  - `test(editor-io): add atomic-save property test`
  - `chore(ci): pin actions/checkout to v4`
- **One logical change per commit.** A commit must compile and pass tests on
  its own.
- **Push after every major TODO**, at the end of every mission, and any time
  another contributor (human or agent) would want to pull.
- **Never commit:** `target/`, editor temp files, `.env*`, secrets,
  `Cargo.lock` for library-only crates (do commit `Cargo.lock` for the
  workspace because it contains a binary crate).
- **Tagging:** at the end of each mission, `git tag -a mXX-complete -m "..."`
  and `git push origin mXX-complete`.

## 5. Quality Gates (Every Commit Must Pass)

Before `git commit`, these must all succeed locally:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all`
4. `cargo build --release`
5. Once per mission at minimum: `cargo run --release` boots the app on
   Windows without crashing.

If any gate fails, fix it **before** committing. Not after.

During M00 (pre-scaffold) gates #2–#5 are trivially green because there is
no Rust code yet. Starting with M01 every single commit must clear them.

## 6. Windows-First, Cross-Platform-Always

Primary development is Windows. CI runs on Windows, Linux, and macOS.

Code that touches any of the following must consider all three OSes from the
start:

- Filesystem (paths, line endings, permissions, mmap)
- Windowing (winit) and input focus
- Graphics backend selection (DX12/Vulkan/Metal/GL)
- Clipboard
- Process management and environment variables

Use `#[cfg(target_os = "...")]` guards explicitly when behavior must diverge.
Never `unwrap()` something that only succeeds on one OS and hope the others
work.

Paths: always `std::path::PathBuf` / `Path`. Never concatenate with `/`.
Content: normalize to `\n` at the I/O boundary; preserve the original on
save unless explicitly converted.

## 7. Performance Is A Feature, Not An Afterthought

Evaluate every change against the PRD's hard targets:

- Input-to-pixel latency < 5 ms under normal load.
- 60 fps minimum during scroll/edit, target 120 fps where hardware allows.
- Bounded memory growth across multi-hour sessions.
- Cold start < 1 s on modern hardware.
- 100 MB text file opens without blocking the UI.

If a change regresses any of these, the change is wrong. Use Criterion
benchmarks to prove performance. Never ship on "feels fast enough."

## 8. Research Before Building

Before implementing anything new, check in this order:

1. `docs/` in this repo.
2. The referenced PRD documents (`docs/PRD.md`, `docs/V2_PRD.md`,
   `docs/MVP_DEFINITION.md`, `docs/RISKS.md`).
3. The open web: current crate docs on docs.rs, crate READMEs on GitHub,
   Zed / Lapce / Helix source, recent (2025–2026) blog posts.

When adopting a new dependency, read its changelog first and pin a minor
version in `Cargo.toml`. Justify the choice in the commit message or update
`TECH_STACK.md`.

## 9. Test, Bench, and Boot

"It compiles" is not "it works." Before declaring a TODO done:

- **Unit tests** for new pure logic (rope, cursor, path normalization, …).
- **Integration tests** for subsystem boundaries (file load → rope → render).
- **Criterion benches** for any hot path.
- **Run `cargo run`** and actually interact: open a file, type, scroll,
  resize, close.
- **Property tests (`proptest`)** for subsystems with external inputs
  (keyboard, mouse, files).

Stress tests are M08's responsibility; per-subsystem tests live in their
owning mission.

## 10. No Docker

We do not ship or develop in Docker. The editor is a native binary. The only
legitimate use of a container is cross-compiling Linux binaries from a
Windows host during M11, and `cross` handles that invisibly. Do not write
Dockerfiles.

## 11. No Scope Creep In The MVP

MVP (M00–M08) does **not** include: syntax highlighting, LSP, AI, plugins,
themes, tabs, project trees, terminal, split views, minimap, multi-cursor
beyond the single primary cursor, git integration, search/replace UI, or
settings UI.

V2 (M09–M10) adds: line numbers, selection, clipboard, undo/redo, word nav,
status bar, last-file persistence. Nothing else.

Anything beyond that belongs in a future mission set, not these.

## 12. End-of-Mission Checklist

1. Run the full quality gate (§5).
2. Update `ARCHITECTURE.md` or `TECH_STACK.md` if the mission changed them.
3. Append an entry under `## [Unreleased]` in `CHANGELOG.md`.
4. Update `docs/STATUS.md` (what's done, what's next).
5. `git tag -a mXX-complete -m "Mission XX complete: <one-line summary>"`.
6. `git push origin mXX-complete`.
7. Add a short "Mission Complete" note at the bottom of `docs/STATUS.md`.

## 13. FOLLOWUPS.md

Create `FOLLOWUPS.md` at the repo root on first need. One bullet per
deferred item, with:

- Date / mission.
- Short problem statement.
- Why it's deferred.
- Suggested earliest mission to address it.

Do not let the backlog vanish. Every new mission starts by reviewing
`FOLLOWUPS.md` and pulling in anything that belongs in scope.

---

*Last updated: M00 (Foundation Research & Documentation).*
