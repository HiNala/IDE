# Testing Strategy

Testing is not a phase; it's part of every commit. This file is the
project-wide standard. Per-crate `README.md`s list the specific tests
each crate is responsible for.

## 1. Levels of Testing

| Level | Runner | When | Where |
|---|---|---|---|
| **Unit** | `cargo test --lib` | Every commit | Inline `#[cfg(test)]` modules |
| **Integration** | `cargo test --test *` | Every commit | Per-crate `tests/` directories |
| **Property** | `cargo test` (proptest) | Every commit | Inline or `tests/` |
| **Benchmark** | `cargo bench` | CI on `main`, PR on opt-in | Per-crate `benches/` |
| **Stress** | Custom binaries | M08 acceptance | `crates/stress-tests/` (added M08) |
| **Manual smoke** | `cargo run --release` | End of every mission | — |

## 2. Coverage Targets

We do not gate on a % line-coverage number (it rewards tests that touch
code without asserting anything). Instead we enforce:

- **Every public function has at least one test**, either unit or
  integration.
- **Every `Result` branch is either exercised or documented as
  unreachable** (e.g. `unreachable!()` with justification).
- **Every `#[cfg(target_os)]` branch is covered by CI on that OS.**

## 3. Unit Tests

- Colocated with the code they test.
- Naming: `fn test_<what_behavior>_<in_what_situation>`.
- One assertion theme per test.
- Use `#[track_caller]` in custom assert helpers so failures point to
  the call site.

## 4. Property Tests (`proptest`)

Required in:

- `editor-core` — rope invariants, cursor invariants, undo/redo
  reversibility.
- `editor-input` — random key sequences never corrupt the document.
- `editor-io` — arbitrary content round-trips through load/save.

Keep proptest config in a shared `proptest-regressions/` folder per
crate, committed to git.

Shrinking must produce useful examples; avoid strategies that generate
uninteresting data.

## 5. Benchmarks (`criterion`)

Each benchmark:

- Is in `benches/<name>.rs`.
- Has a small `README` entry in `benches/README.md` describing what it
  measures.
- Uses `criterion::black_box` to avoid LLVM eliding the work.
- Reports a primary time measurement; avoid measuring allocation count
  unless that is the point of the benchmark.

### Required Benchmarks (By Mission)

| Mission | Benchmark | File |
|---|---|---|
| M02 | Rope insert at head / middle / tail | `editor-core/benches/rope_insert.rs` |
| M02 | Line-index lookup | `editor-core/benches/line_index.rs` |
| M02 | Undo / redo round-trip | `editor-core/benches/history.rs` |
| M04 | Single-line reshape | `editor-render/benches/reshape_line.rs` |
| M04 | Atlas hit rate | `editor-render/benches/atlas_hit.rs` |
| M05 | End-to-end input latency | `editor-app/benches/input_latency.rs` |
| M06 | 100 MiB file load | `editor-io/benches/load_100mb.rs` |
| M06 | Atomic save | `editor-io/benches/save_atomic.rs` |

### Regression Threshold

CI fails a PR if any benchmark slows by more than 5 %. Adjustable
per-benchmark with a documented reason in the commit.

## 6. Stress Tests (M08)

- `stress-tests/binary/open_100mb.rs` — spawn the editor with a 100 MB
  file arg; record frame time.
- `stress-tests/binary/scroll_100mb.rs` — drive a keyboard scroll across
  a 100 MB file; assert minimum frame time.
- `stress-tests/binary/soak_4h.rs` — 4-hour editing macro; assert
  bounded RSS growth.
- `stress-tests/binary/crash_inject_save.rs` — `SIGKILL`/`TerminateProcess`
  mid-save across 1000 iterations; assert file integrity afterward.

Each stress test is a standalone `cargo run` target so it can be run
outside CI (they take too long for PR CI).

## 7. Snapshot Tests (`insta`)

Used where we have a non-trivial textual or visual output:

- `editor-core`: error messages, config echo.
- `editor-render`: small frames rendered via `insta`'s image snapshot
  comparison (via `insta::assert_binary_snapshot!`).

Snapshots live in `snapshots/` next to their tests.

## 8. Concurrency Tests (`loom`, optional)

`loom` is wired behind a feature flag. Reserve for:

- The `ArcSwap` snapshot publication pattern (verify no lost updates).
- The worker → main channel drain loop (verify no deadlocks under
  adversarial scheduling).

Not required for MVP completion but valuable when concurrency bugs
appear.

## 9. CI Wiring

`.github/workflows/ci.yml` (added M01) runs, per OS:

```yaml
- cargo fmt --all -- --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo test --all --locked
- cargo build --release --locked
- (main branch only) cargo bench -- --save-baseline main
```

- Runs with `RUSTFLAGS="-D warnings"`.
- Uses `cargo-nextest` if enabled (added during M01 if low friction).

## 10. Pre-Commit Checklist

Before any `git commit`:

1. `cargo fmt --all`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all`
4. If the change touches a hot path: `cargo bench -- --baseline main`
   locally.
5. Compile a release build at least once per mission:
   `cargo build --release`.

## 11. What We Do Not Test

- Wrapper code that just calls another library with the same args.
- Trivial getters and setters.
- Code that is structurally impossible to exercise (write it out of
  existence instead).
- Third-party crate internals.

## 12. Test Naming Reference

```rust
// Unit
#[test]
fn insert_at_empty_rope_yields_expected_len() { ... }

// Integration
#[tokio::test]
async fn load_then_save_round_trip_preserves_line_endings() { ... }

// Property
proptest! {
    #[test]
    fn random_edits_preserve_len_invariant(edits in arb_edits(..100)) {
        let mut doc = Document::new();
        apply_all(&mut doc, &edits);
        prop_assert_eq!(doc.len_bytes(), expected_len(&edits));
    }
}

// Benchmark
fn bench_insert_head(c: &mut Criterion) { ... }
criterion_group!(benches, bench_insert_head);
criterion_main!(benches);
```

---

*Last updated: M00.*
