# M08 — MVP Integration, Stress Testing, Acceptance

**Mission ID:** M08
**Prerequisites:** M00 through M07 complete. Editor opens, edits, saves. Performance is instrumented and guarded.
**Output:** A formal acceptance pass against every non-functional target in the MVP PRD and MVP Definition documents. Stress tests prove the editor stays stable and fast under adversarial workloads. The architecture is validated. A versioned `0.1.0-mvp` release candidate is tagged. A written acceptance report is checked into the repo as evidence.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/reference/00_PRODUCT_REQUIREMENTS.md` — the MVP PRD. Every performance-related claim in §2 ("Core Product Philosophy and Constraints") and §12 ("MVP Scope and Success Criteria") is our acceptance bar.
- `/reference/04_MVP_DEFINITION.md` — the "MVP Definition Document." §5 ("Explicit Non-Functional Requirements"), §6 ("Definition of Completion"), §7 ("Testing Strategy"), §8 ("Acceptance Criteria") are the authoritative checklists.
- `/docs/PERFORMANCE_BUDGETS.md` — our derived numbers and reference-hardware baselines.
- `/docs/TESTING_STRATEGY.md` — our testing pyramid.

---

## The Situation In Plain English

M08 is not a mission that builds new features. It is the mission that **proves the architecture works** by running it through everything the PRD said it should survive and comparing the measured results to the claimed targets. If any target is missed, M08 does not pass until it is. The success of the entire MVP hinges on this one mission producing a clean report.

The PRDs are specific. Input-to-pixel latency under 5 milliseconds. Stable memory over extended sessions. Instant file opening. Smooth scrolling on large documents. Cross-platform consistency. Predictable behavior across thousands of machines. These are not aspirations; they are requirements. M08 turns them into measured numbers and documented observations.

The work falls into three buckets. First, a comprehensive **stress test suite**: edge cases, large files, long sessions, hostile inputs, rapid typing, constant resizing. Second, a **cross-platform acceptance pass**: every test runs on Windows, Linux, and macOS, and any failure that is OS-specific gets its own tracked issue. Third, a **written acceptance report** — a document in `/docs/MVP_ACCEPTANCE.md` that restates each PRD requirement, describes the test that validates it, shows the measured number, and marks it green or red. Anything red blocks release.

We also polish rough edges. A few things we left on the floor in earlier missions — proper error dialogs when a save fails, graceful handling of GPU device lost, making sure the process exits cleanly with a non-zero code on fatal errors — get cleaned up here. The goal is that the MVP is not just *technically done*; it is *actually usable* by someone who doesn't know the codebase.

Finally, we tag `0.1.0-mvp`. That tag signals "the architecture is proven." It is the point at which V2 work (M09/M10) becomes legitimate: we are adding user-facing affordances on top of a foundation that has earned the right to be trusted.

---

## Scope

**In scope:**
- Complete stress test suite exercising: very large files, very long sessions, rapid typing, pathological input, rapid resize, rapid DPI changes, external file changes, save-during-edit races.
- Cross-platform acceptance runs on all three OSes.
- Written acceptance report at `/docs/MVP_ACCEPTANCE.md`.
- Polish items: graceful error surfaces (log + continue) for save/load failures, GPU device-lost recovery attempt, process exit code discipline, window title shows the open file's name and a `*` when dirty.
- Release-candidate build verification: `cargo build --release --workspace` produces a working binary on each OS.
- `0.1.0-mvp` git tag.

**Out of scope:**
- Any new features beyond the polish items above.
- Installer packaging (M11).
- Code signing / notarization (M11).
- Syntax highlighting or other V2+ features.

---

## North Star

At the end of M08, you can hand the binary to a developer who has never seen the codebase, give them the `/docs/MVP_ACCEPTANCE.md` report, and they can verify every claim themselves in under an hour. Every green box in the report has a measurable, reproducible basis.

---

## TODO List

### 1. Compile the acceptance checklist

- [ ] 1.1. Open `/reference/04_MVP_DEFINITION.md` §8 ("Acceptance Criteria") and transcribe each numbered item into `/docs/MVP_ACCEPTANCE.md` as a table with columns: `#`, `Requirement`, `Test`, `Measured`, `Target`, `Status`. Leave `Measured` and `Status` blank for now.
- [ ] 1.2. Repeat for §5 ("Explicit Non-Functional Requirements") and the PRD's §2 and §12.
- [ ] 1.3. Merge duplicates. End result is a single canonical checklist — somewhere around 20-30 items — that we will tick off over the course of this mission.
- [ ] 1.4. Commit: `docs(acceptance): draft MVP acceptance checklist`.

### 2. Stress test: very large files

- [ ] 2.1. Existing large-file test from M06 handles 500 MB. Now try 1 GB and 2 GB on Windows (where we have real hardware; skip on CI runners). Generate the file externally, measure open time, first-interactive time, scroll-through time, save time. Record all four numbers.
- [ ] 2.2. If any number fails PRD targets, investigate. Common culprits: synchronous I/O on the main thread (shouldn't happen after M06), LRU layout cache too small, mmap failing and falling back to streaming.
- [ ] 2.3. Document the measured numbers in `/docs/MVP_ACCEPTANCE.md`.
- [ ] 2.4. Commit: `test(perf): stress-test 1GB and 2GB file handling; record results`.

### 3. Stress test: long-running session

- [ ] 3.1. Create a test harness (under `crates/editor-app/tests/long_session.rs`, `#[ignore]`d) that drives the editor for an hour of simulated use: 1,000,000 scripted `EditorCommand`s including inserts, deletes, cursor moves, saves, loads. Sample `MetricsCollector` every minute.
- [ ] 3.2. Assertions:
  - Memory RSS at end ≤ 1.5× memory RSS at start.
  - p99 frame time in the final minute ≤ p99 in the first minute × 1.1.
  - No panics, no unhandled errors, process still responsive at end.
- [ ] 3.3. Run once locally before tagging. Document measured numbers.
- [ ] 3.4. Commit: `test(perf): long-running session stability test`.

### 4. Stress test: adversarial input

- [ ] 4.1. Property-based test in `crates/editor-core/tests/adversarial_input.rs` using `proptest`: feed 10,000 random sequences of `EditorCommand` to a fresh `EditorState`. Assertions:
  - Every command application terminates.
  - Buffer invariants hold after every command (byte/char boundary consistency, line count matches newlines).
  - Undo/redo round-trips match expected state.
- [ ] 4.2. Generate commands with heavy bias toward edge cases: zero-length inserts, inserts at buffer boundary, deletes that span line boundaries, rapid sequences of the same command.
- [ ] 4.3. Commit: `test(core): adversarial proptest for EditorState command application`.

### 5. Stress test: rapid resize and DPI changes

- [ ] 5.1. Manual test script on Windows: drag a window edge rapidly for 30 seconds; drag the window between monitors of different DPI for 30 seconds; maximize / restore / minimize 50 times. Watch for crashes, memory leaks, frame hangs. Record observations.
- [ ] 5.2. Automated component of this: a unit test in `editor-render` that calls `renderer.resize` with random sizes in a tight loop (100 iterations). Assert no panics, no `wgpu` validation errors.
- [ ] 5.3. Commit: `test(render): rapid resize stress test`.

### 6. Stress test: fast typing

- [ ] 6.1. Scripted test: generate 10 input events per millisecond (10k chars/sec, much faster than any human) for 10 seconds. Assert no input is dropped, every character ends up in the buffer in the right order, frame loop keeps up (p99 < 50 ms under this unnatural load; we're not targeting 5ms under 10k-char/sec which no human produces).
- [ ] 6.2. Human-realistic test: 15 chars/sec sustained for 10 minutes. Assert p99 < 5 ms throughout.
- [ ] 6.3. Commit: `test(perf): fast-typing stress tests`.

### 7. Stress test: save/load races

- [ ] 7.1. Scripted test: begin a save job, immediately issue a load of a different file, immediately issue another load. Assert only the final load's result updates the buffer (the earlier load is cancelled), the save completes independently.
- [ ] 7.2. Scripted test: save → modify → save → modify → save, as fast as possible. Assert the on-disk file ends up with the final modified state.
- [ ] 7.3. Commit: `test(io): save/load race and cancellation tests`.

### 8. Stress test: external file change

- [ ] 8.1. Scripted test: open a file, modify via editor, write over the file externally (`std::fs::write` from a different thread or another process), focus the window, confirm the "externally modified" indicator appears. For MVP we don't need to implement auto-reload; we just need the detection to work.
- [ ] 8.2. Commit: `test(app): external-modification detection test`.

### 9. Polish: graceful error surfaces

- [ ] 9.1. When a save fails (disk full, permission denied), the editor currently logs and continues with a dirty buffer. Add a small "SAVE FAILED: <reason>" banner rendered via `TextLayer` at the top of the window for 5 seconds. This is a tiny UI component — implemented as a `BannerLayer` module in `editor-render`.
- [ ] 9.2. When a load fails, same banner pattern: "OPEN FAILED: <reason>". The buffer is unchanged; the editor remains usable.
- [ ] 9.3. When the GPU device is lost (wgpu's `DeviceLost` event), attempt to recreate the device and surface once. If that fails, display a banner "GPU LOST — restarting…" and call `event_loop.exit()` with exit code 2 (non-zero so scripts notice). Document this in `/docs/DIAGNOSING_PERFORMANCE.md`.
- [ ] 9.4. Commit: `feat(render, app): banner-style error surface for save/load/GPU errors`.

### 10. Polish: window title reflects state

- [ ] 10.1. When a file is open, the title becomes `<filename> — IDE`. When the buffer is dirty, prepend `*`: `* <filename> — IDE`. When no file is open, `IDE (untitled)`.
- [ ] 10.2. Update the title on buffer version change (use a small `dirty_at_version: u64` comparison with `buffer.version()`).
- [ ] 10.3. Commit: `feat(app): dynamic window title with dirty indicator`.

### 11. Polish: exit-code discipline

- [ ] 11.1. Clean user-initiated exit → 0.
- [ ] 11.2. Unrecoverable GPU error → 2.
- [ ] 11.3. Panic (should not happen, but if it does) → default Rust panic behavior (non-zero).
- [ ] 11.4. Invalid CLI args → 64 (convention).
- [ ] 11.5. Document in `/DEVELOPMENT.md`.
- [ ] 11.6. Commit: `feat(app): discipline process exit codes`.

### 12. Cross-platform acceptance run

- [ ] 12.1. **Windows 10/11**: every acceptance checklist item. Record measured numbers in the report.
- [ ] 12.2. **macOS (latest)**: same checklist, ideally on Apple Silicon + Intel if available.
- [ ] 12.3. **Linux (Ubuntu 22.04+ or Fedora)**: same checklist on X11 and Wayland sessions if possible.
- [ ] 12.4. Any platform-specific failures → documented in `/FOLLOWUPS.md` with clear reproduction steps; if it's a blocker, fix it here; if it's a long-term issue (like fractional DPI on Wayland not rendering fonts crisply), document and accept for MVP.
- [ ] 12.5. Commit: `test(acceptance): cross-platform acceptance run with recorded results`.

### 13. Benchmark summary

- [ ] 13.1. Run all Criterion benches with `--save-baseline m08-mvp`.
- [ ] 13.2. Generate a summary markdown table of every benchmark and its median time. Commit it as `/docs/BENCHMARKS.md`.
- [ ] 13.3. Commit: `docs(perf): publish M08 benchmark summary`.

### 14. Fill in the acceptance report

- [ ] 14.1. Walk through every row in `/docs/MVP_ACCEPTANCE.md`. Fill in `Measured`, `Status` (✅ / ❌ / ⚠️-with-caveat).
- [ ] 14.2. Any ❌ blocks the release; iterate until it's green.
- [ ] 14.3. Any ⚠️ must have a note explaining the caveat and linking to an issue / FOLLOWUPS entry.
- [ ] 14.4. Add a summary paragraph at the top of the report: "MVP acceptance tests run on <date>. <N> requirements, <G> green, <Y> yellow, <R> red. Release approved: yes/no."
- [ ] 14.5. Commit: `docs(acceptance): complete MVP acceptance report`.

### 15. Release-candidate verification

- [ ] 15.1. On each OS: `cargo build --release --workspace && target/release/editor-app README.md`. Confirm the binary runs, opens the README, is responsive, closes cleanly.
- [ ] 15.2. Check binary size (should be under 50 MB per OS; strip symbols if needed). Record in the report.
- [ ] 15.3. Run the smoke test: `scripts/perf-smoke.ps1 --binary target/release/editor-app.exe` (or equivalent). Must pass.
- [ ] 15.4. Commit: `test(release): verify RC binary on each OS`.

### 16. Quality gates (final)

- [ ] 16.1. `cargo fmt --all --check`.
- [ ] 16.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 16.3. `cargo test --workspace --all-features` — including the ones previously `#[ignore]`d (run manually: `cargo test --workspace -- --include-ignored`).
- [ ] 16.4. `cargo bench --workspace`.
- [ ] 16.5. `cargo doc --workspace --no-deps` — no broken links.
- [ ] 16.6. CI green on `main`.

### 17. Tag `0.1.0-mvp`

- [ ] 17.1. Bump `Cargo.toml` workspace version to `0.1.0`.
- [ ] 17.2. Update `CHANGELOG.md`: move everything from `[Unreleased]` to a new `## [0.1.0-mvp] — YYYY-MM-DD` section.
- [ ] 17.3. Commit: `chore(release): cut 0.1.0-mvp`.
- [ ] 17.4. Tag: `git tag -a 0.1.0-mvp -m "MVP release candidate: architecture validated"`; push the tag.
- [ ] 17.5. Also tag `m08-complete`.

---

## Validation / Acceptance Criteria

M08 is complete when:

1. Every row in `/docs/MVP_ACCEPTANCE.md` is green (or documented yellow with a clear caveat and a follow-up tracked in `/FOLLOWUPS.md`).
2. `cargo test --workspace -- --include-ignored` passes on Windows.
3. Release binary on each OS launches and is usable.
4. CI green.
5. `0.1.0-mvp` tag pushed.
6. `/docs/BENCHMARKS.md` published.
7. `CHANGELOG.md` reflects the release.

## Testing Requirements

- Every stress test above runs at least once and produces a recorded measurement.
- Cross-platform acceptance run covers three OSes.
- `--perf-smoke` passes on release binary.

## Git Commit Strategy

12-16 commits. Push after items 1, 3, 6, 9, 12, 14, 15, 17.

## Handoff to M09

M09 assumes:

- The MVP architecture is proven and is the baseline going forward.
- Every subsequent mission must not regress any acceptance number.
- V2 adds UX affordances on top; architecture is frozen for this cycle.

---

## Standing Orders Reminder

- Do not declare acceptance green unless you actually measured. A memory of "this used to be fast" is not evidence.
- Any yellow in the acceptance report must be accompanied by a follow-up in `/FOLLOWUPS.md`. No silent compromises.
- A release tag is a commitment. Make sure what's tagged actually works end-to-end, not just according to green CI on a feature branch.
- If you find a bug in a PRD requirement (e.g., the target is physically impossible on a certain hardware class), document it honestly rather than hiding it. The PRD can be updated; we can't un-ship a broken release.

Go.
