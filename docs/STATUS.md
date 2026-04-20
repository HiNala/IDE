# Current Status

This file is the source of truth for *where we are* in the mission sequence.
Every mission updates this file in its final commit.

## Mission State

| Mission | Status | Notes |
|---|---|---|
| **M00** — Foundation Research & Documentation | ✅ Complete | Docs tree, root files, git init, first push. |
| **M01** — Repo Scaffolding, Workspace, Toolchain, CI | ✅ Complete | Workspace, 5 crates, CI matrix, Dependabot, `cargo fmt/clippy/test/build --release` all green on Windows. CI first-push verification on 3 OSes pending. |
| **M02** — Text Engine | ⏳ Up Next | — |
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

### M01 — Repo Scaffolding, Workspace, Toolchain, CI (complete)

- **Workspace.** Virtual `Cargo.toml` with five members under `crates/`:
  `editor-core`, `editor-render`, `editor-input`, `editor-io`, and the
  binary `editor-app` (`[[bin]] name = "ide"`). Each crate has a `README.md`
  describing its scope and mission ownership, a minimal `lib.rs`/`main.rs`
  that compiles cleanly, and one passing smoke test (5 tests total,
  all green).
- **Toolchain.** `rust-toolchain.toml` pins Rust `1.94.1` with `rustfmt`
  and `clippy`. `rustfmt.toml` limits itself to stable-only options.
- **Shared lints.** `[workspace.lints]` applies Rust + Clippy rules across
  every crate, treating hot-path-unfriendly items (`dbg_macro`, `todo`,
  `unimplemented`, `print_stderr`, `print_stdout`) as warnings.
- **Release profile.** `lto = "thin"`, `codegen-units = 1`,
  `strip = "symbols"`, `panic = "abort"`. Bench profile inherits release
  with debug info retained.
- **Supply-chain.** `deny.toml` for `cargo-deny` defined (licenses,
  advisories, bans, sources). CI runs the audit advisory-only; gating
  is a planned follow-up.
- **CI matrix.** `.github/workflows/ci.yml` runs per push and per PR on
  Windows, Linux, and macOS via `actions/checkout@v4`,
  `dtolnay/rust-toolchain@stable`, and `Swatinem/rust-cache@v2`. Steps:
  `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --all
  --locked`, `cargo build --release --all --locked`. A separate `docs`
  job runs `cargo doc` with `RUSTDOCFLAGS="-D warnings"`.
- **Dependabot.** Weekly cargo + GitHub-Actions bumps grouped by
  minor/patch with conventional-commit prefixes.
- **Corrections.** `TECH_STACK.md` version entries updated to the real
  April-2026 crates.io state (`wgpu 29`, `glyphon 0.11`) and annotated
  with a "pinned at adoption" policy to avoid speculative pins.
- **Quality gate, local (Windows).** `cargo fmt/clippy/test/build --release`
  all pass. `cargo run --release` prints the boot banner and exits.
- **Next.** Verify CI is green on Linux and macOS once the commit is
  pushed; proceed to M02.

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

*Last updated: M01.*
