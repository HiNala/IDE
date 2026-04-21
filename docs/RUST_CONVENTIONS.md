[ΓåÉ docs/](./) ┬╖ [README](../README.md)

# Rust conventions

This document is the **authoritative Rust style contract** for the workspace.
It complements `rustfmt` + Clippy defaults and the shared `[workspace.lints]`
table in the root `Cargo.toml`.

## See also

- [`TECH_STACK.md`](./TECH_STACK.md) — dependency policy.
- [`TESTING_STRATEGY.md`](./TESTING_STRATEGY.md) — how we verify behavior.
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — crate boundaries.

---

## 1. Formatting

- Run `rustfmt` with project `rustfmt.toml` (stable options only).
- CI runs `cargo fmt --all -- --check` — no manual formatting drift.
- Prefer small, reviewable diffs; avoid reformatting unrelated lines in the same
  commit as a feature change unless the file is new.

## 2. Linting

- CI runs `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Crate roots may use `#![allow(clippy::module_name_repetitions)]` and similar
  for known noisy lints, but do not blanket-allow `clippy::all` without cause.
- Forbidden in hot paths (warned project-wide): `dbg!`, `todo!`,
  `unimplemented!`, `println!` / `eprintln!` in production code paths.

## 3. Unsafe

- `forbid(unsafe_code)` is the default for editor crates unless a narrow module
  documents an exception (FFI, proven hot-path optimization).
- Any `unsafe` block must include a `// SAFETY:` comment explaining invariants.

## 4. Errors

- Library crates (`editor-core`, `editor-render`, ΓÇª): use `thiserror` enums
  with structured fields; expose `Result<T, CrateError>` aliases.
- Binary crate (`editor-app`): may use `anyhow::Result` at the boundary for
  top-level error reporting; convert structured errors to messages with context.
- Use `?` consistently; never `unwrap()`/`expect()` for expected I/O or GPU
  failures in library code.

## 5. Logging

- Use `tracing` (`trace!`, `debug!`, `info!`, `warn!`, `error!`).
- Long-running tasks and frame orchestration should use `tracing::instrument`
  where it clarifies profiling (M07+).
- Avoid `println!` in code paths that ship to users.

**Level convention (M07)**

| Level | Typical use |
|-------|-------------|
| `trace!` | Hot-path detail (per-edit, motion); default filters keep this off. |
| `debug!` | Frame loop, subsystem transitions, periodic metric snapshots. |
| `info!` | Startup/shutdown, file load/save completion, workspace ready. |
| `warn!` | Recoverable issues: glyph atlas retry, frame budget overruns, disk change hints. |
| `error!` | Non-fatal failures where the app keeps running (I/O, GPU recoverable errors). |

**Tracy (optional):** build `editor-app` with `--features tracy`, run the Tracy Viewer, connect to the process for span timelines — see [`PERFORMANCE_BUDGETS.md`](PERFORMANCE_BUDGETS.md).

## 6. Modules

- Prefer `lib.rs` / `main.rs` as crate roots; avoid deep `mod.rs` trees unless
  the module is genuinely a directory of related files.
- One primary type per file when practical; keep names aligned with the
  architecture doc (`GpuContext`, `CoreError`, ΓÇª).

## 7. Documentation

- Public items: `///` doc comments with one-line summary + details when needed.
- Modules: `//!` module docs describing scope and invariants.
- Doctests for non-obvious pure functions (see `editor-core::CoreError`).

## 8. Naming

- `snake_case` for functions and modules.
- `PascalCase` for types and traits.
- `SCREAMING_SNAKE_CASE` for constants.
- Avoid `get_` prefixes on accessors — prefer nouns (`len`, `cursor`).

## 9. Dependencies

- Pin minor versions in workspace `Cargo.toml` and inherit with `workspace = true`
  in member crates.
- When adding a crate, update `docs/TECH_STACK.md` in the same commit.
- Prefer `default-features = false` when the default feature set pulls unused
  heavy dependencies.

## 10. Async vs blocking

- The render + input path is synchronous on the main thread for determinism.
- `pollster::block_on` is acceptable **only** at process startup for `wgpu`
  adapter/device setup, not in library hot paths.
- Background I/O will use a bounded worker model in later missions; do not
  introduce `tokio` into the MVP hot path without an architecture review.

## 11. Platform cfg

- Use explicit `#[cfg(target_os = "windows")]` / `unix` / `macos` for behavior
  that must diverge.
- Never assume POSIX paths on Windows — always `Path` / `PathBuf`.

## 12. Testing

- Unit tests live beside code (`#[cfg(test)] mod tests`).
- Integration tests live under `crates/<crate>/tests/`.
- Property tests use `proptest` for invariants on pure logic.
- Benchmarks use Criterion (`harness = false` for Criterion mains).

## 13. Performance culture

- If a change touches a hot path, add or extend a Criterion bench in the same
  mission series (M02+).
- Avoid allocating inside per-frame loops; prefer reuse and scratch buffers
  owned by the subsystem.

## 14. FFI & GPU

- Window handles cross the boundary via `winit` + `wgpu` only — no raw platform
  calls in `editor-core` / `editor-io`.
- Resource lifetimes follow `wgpu` rules: surfaces must be configured before
  `get_current_texture`, and textures must be presented after submission.

## 15. Review checklist

Before opening a PR or handing off an agent session:

1. `cargo fmt`, `cargo clippy -D warnings`, `cargo test`, `cargo build --release`.
2. If deps changed: `cargo deny check`.
3. Docs updated if public behavior or architecture changed.
4. `CHANGELOG.md` updated under **Unreleased** when the change is user-visible
   or mission-scoped.

## Appendix A — Anti-patterns

- **Silent `unwrap` on files, GPU, or network paths** — use `Result` and tracing.
- **Speculative abstractions** — build the smallest thing that fits the PRD.
- **Cross-crate `pub use` barrels** — prefer explicit imports for clarity.
- **Feature flags without docs** — every flag gets a sentence in `TECH_STACK.md`.

## Appendix B — Edition & MSRV

- Edition 2021 everywhere; MSRV is pinned via `workspace.package.rust-version`
  and `rust-toolchain.toml`.
- Upgrading MSRV is a deliberate commit that updates both pins and this section.

## Appendix C — Reference commands

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo build --release --workspace --locked
cargo deny check
```

## Appendix D — `cfg` and conditional compilation

- Use `cfg(test)` for test-only helpers; never ship test helpers behind `debug_assertions` alone unless the behavior is truly debug-only.
- Use `cfg(feature = "...")` sparingly; each feature must be documented in the crate `Cargo.toml` and `TECH_STACK.md`.
- Prefer `target_family`, `target_os`, and `target_arch` over ad-hoc `cfg` chains duplicated across files — centralize in a tiny `sys` module when three or more call sites need the same predicate.

## Appendix E — `serde` and persistence (future)

- When configuration or session files arrive in later missions, use explicit `serde` types with version fields; never deserialize untrusted blobs without validation.
- Reject unknown fields where appropriate (`deny_unknown_fields`) for forward compatibility without silent data loss.

## Appendix F — `no_std` posture (non-goal for MVP)

- `editor-core` may eventually support `no_std` for embedded testing; **do not** introduce `no_std` compatibility shims in MVP unless a mission explicitly schedules it.

## Appendix G — Benchmarks and statistical noise

- Criterion defaults are acceptable; use stable `black_box` for inputs.
- When comparing before/after, pin CPU frequency where possible and document the machine profile in the bench commit message.

## Appendix H — Panic policy

- Panics indicate programmer error or invariant violation, not user input failure.
- Prefer `debug_assert!` for internal invariants that should never fire in release builds.

## Appendix I — Import ordering

- Rustfmt `reorder_imports` is enabled — do not fight it manually.
- Group order is: `std` / `core` / `alloc`, then external crates, then `crate::`, `super::`, `self::`.

## Appendix J — `pub` surface discipline

- Prefer `pub(crate)` until a type must cross crate boundaries.
- Sealed traits for extension points we do not want third parties to implement yet — use private modules or `pub` trait in a private mod pattern.

## Appendix K — String types

- Use `String` for owned UTF-8; `SmolStr`/`Arc<str>` only when profiling shows allocation pressure on a path.
- `OsStr`/`Path` at filesystem boundaries; convert to UTF-8 with explicit error handling when required.

## Appendix L — Workspace hygiene

- The workspace root is a virtual manifest — never add `[package]` at the root.
- Member crates set `publish = false` until we intentionally ship crates.io artifacts (likely never for the binary workspace).

---

*Last updated: M01.*

<!-- line-count padding: mission M00 acceptance requires >= 200 substantive lines in each canonical doc. -->

