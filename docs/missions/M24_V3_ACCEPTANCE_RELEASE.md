# M24 — V3 Acceptance & Release

**Mission ID:** M24
**Prerequisites:** M23 complete. The AI chat panel works end-to-end.
**Output:** V3 is formally accepted against the six N-Star tests from `00_V3_VISION.md`. All previous MVP + V2 acceptance numbers still green. Version bumped to `0.3.0`; tag `v0.3.0` pushed; the release workflow from M11 produces signed installers for Windows, macOS, and Linux. README updated with the AI-native positioning, setup docs for API keys and Ollama. A one-page `V3_ACCEPTANCE.md` report captures what shipped, what numbers were met, and what remains for V4+. This is the terminal mission for V3.
**Estimated scope:** 1-2 sessions.

---

## Read First

- `/00_MISSION_INDEX.md` — standing orders.
- `/00_V3_VISION.md` — especially the "six tests that define V3 success" section.
- M08 — MVP acceptance template (reuse the pattern).
- M11 — release workflow. M24 triggers the same workflow via a fresh tag.
- Every M12-M23 doc — make sure each one's `m*-complete` tag was actually pushed.

---

## The Situation In Plain English

V3 is done when six user-observable outcomes are true — the six tests Brian specified in the V3 vision doc, reproduced here for the acceptance report. M24 is the ritual of proving each of them on a clean build, recording the results, bumping the version, and triggering the release pipeline we built in M11 so the bits go out into the world.

The discipline of a dedicated acceptance mission is important. It's tempting to say "it seems to work" and ship. But the MVP acceptance we did in M08 caught a handful of subtle bugs — regressions that individually weren't noticed during feature work. The same is almost certainly true now; M12 through M23 were a lot of code. A rigorous pass through every acceptance criterion finds the drift before users do.

---

## Scope

**In scope:**
- Formal execution of the six V3 acceptance tests.
- `/docs/V3_ACCEPTANCE.md` report documenting outcomes, numbers, screenshots/video references.
- Re-running MVP (M08) and V2 (post-M10) acceptance suites to confirm no regressions.
- Version bump: workspace `version = "0.3.0"` across all crates.
- CHANGELOG update with a comprehensive V3 summary.
- README update: new features, install instructions, API key setup pointer to `docs/AI_SETUP.md`.
- Tag `v0.3.0`; push; watch the M11 workflow produce installers.
- Clean-VM install test of each installer.
- A short post-release follow-up list in `FOLLOWUPS.md` capturing polish items deferred from M12-M23.

**Out of scope:**
- New features (explicitly — no "one more thing" into the release).
- V4 planning (save for a fresh mission set).
- Marketing materials / launch announcement (that's the human layer).

---

## North Star

A `v0.3.0` tag exists. The GitHub Releases page has six installer artifacts. Each installs on a clean VM and boots into a working editor. The editor opens a project, shows syntax highlighting, supports chat against Anthropic/Ollama/etc., executes agent edits through the approval flow, and updates sidecars. The six vision tests all pass. The acceptance report documents every number. V3 has shipped.

---

## TODO List

### 1. Pre-flight: verify all M12-M23 tags exist

- [ ] 1.1. `git tag -l "m1[2-9]-complete" "m2[0-3]-complete"` — confirm all 12 tags (m12 through m23) are in the repo.
- [ ] 1.2. If any mission didn't properly tag, go back and fix — don't paper over it at release time.
- [ ] 1.3. Commit: N/A (verification step).

### 2. Full quality gate sweep

- [ ] 2.1. `cargo fmt --all --check`.
- [ ] 2.2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [ ] 2.3. `cargo test --workspace --all-features`.
- [ ] 2.4. `cargo build --release --all-features`.
- [ ] 2.5. `cargo run --release` boots to a working editor on all three OSes (manual).
- [ ] 2.6. Commit: `chore: clean quality gate snapshot before v0.3.0 acceptance`.

### 3. Re-run MVP acceptance (preserve M08 numbers)

- [ ] 3.1. Follow the procedure from `/docs/ACCEPTANCE.md` (written in M08). Run every benchmark. Compare to the baseline stored in `.github/baselines/m08-mvp.json`.
- [ ] 3.2. Any metric regression > 5% is a blocker. Investigate, fix, repeat.
- [ ] 3.3. Record numbers in `/docs/V3_ACCEPTANCE.md` under "MVP baseline preservation."
- [ ] 3.4. Commit: `test(acceptance): MVP baseline preservation for v0.3.0`.

### 4. Re-run V2 acceptance

- [ ] 4.1. Same procedure with V2's baselines (`m10-v2.json`).
- [ ] 4.2. Record outcomes in the acceptance report.
- [ ] 4.3. Commit: `test(acceptance): V2 baseline preservation for v0.3.0`.

### 5. V3 Acceptance Test 1 — workspace open + edit

- [ ] 5.1. Procedure: on a fresh machine (or a clean git clone), open a known fixture project (use the IDE's own repo). Verify:
  - File tree appears within 500 ms of launch.
  - A file clicked in the tree opens in under 50 ms.
  - Syntax highlights render.
  - Typing shows no visible latency; p99 input-to-pixel < 5 ms per M07 overlay.
  - All V1/V2 acceptance numbers green.
- [ ] 5.2. Record pass/fail in the report with the exact numbers and the hardware used.

### 6. V3 Acceptance Test 2 — Ctrl+P quick open

- [ ] 6.1. Procedure: `Ctrl+P`, type a fragment of a filename, verify ranked results appear in < 16 ms, hit Enter, file opens.
- [ ] 6.2. Run on a small project (~100 files) and a larger one (~10,000 files).
- [ ] 6.3. Record the query-to-results latency on each.

### 7. V3 Acceptance Test 3 — project search

- [ ] 7.1. Procedure: `Ctrl+Shift+F`, type a query, verify streaming results start in < 200 ms, click a result, verify the file opens at the matching line.
- [ ] 7.2. Record TTFR (time-to-first-result) and total-results-time.

### 8. V3 Acceptance Test 4 — AI chat end-to-end

- [ ] 8.1. Procedure: with a configured Anthropic (or equivalent) provider, open the chat panel, type a two-file edit prompt against a known fixture ("add a new function `greet` to `src/main.rs` and call it from `src/lib.rs`"), review the proposed diffs, accept all, verify the buffer contents reflect the changes, verify one undo reverses all of them.
- [ ] 8.2. Run against Ollama (local) separately to verify the local path.
- [ ] 8.3. Record pass/fail. Any tool-use schema mismatch or agent-loop exit condition that fails is a blocker.

### 9. V3 Acceptance Test 5 — sidecar generation

- [ ] 9.1. Procedure: complete the test-4 flow, then inspect `.ide/meta/src/main.rs.md` — verify it exists, has the expected frontmatter fields, and contains a plausible summary + history entry referencing the session.
- [ ] 9.2. Edge case: test with the NoopSummarizer configured; verify graceful degradation (skeleton sidecars written, no crash).

### 10. V3 Acceptance Test 6 — packaged installer first-launch

- [ ] 10.1. On a clean Windows VM: install the `v0.3.0-windows-x86_64.msi`. Launch. First-launch flow should prompt for (or at minimum point to) AI setup.
- [ ] 10.2. Same on macOS and Linux (Ubuntu 22.04 + both .deb and AppImage).
- [ ] 10.3. Record install time, first-launch time, presence of first-launch onboarding.
- [ ] 10.4. This one *requires* completing step 14 first (tag + workflow). After the workflow produces artifacts, loop back to run this test.

### 11. Version bump

- [ ] 11.1. Update workspace `Cargo.toml` `version = "0.3.0"`. Confirm all crates inherit.
- [ ] 11.2. `cargo build --release` to regenerate the `Cargo.lock`.
- [ ] 11.3. Commit: `chore(release): bump to 0.3.0`.

### 12. Changelog

- [ ] 12.1. Write the `## [0.3.0] — YYYY-MM-DD` entry. Sections: Added (M12-M23 features), Changed (any behavior shifts), Fixed (regressions discovered and fixed during M24). Keep it factual, chronological, and unambiguous.
- [ ] 12.2. Commit: `docs(changelog): v0.3.0 entry`.

### 13. README + install docs

- [ ] 13.1. Top of README now reads "A fast, native, AI-native code editor. Human-and-agent first, retrofit-free." (or similar — keep it honest and compelling).
- [ ] 13.2. Install section: links to downloads for each OS, the SmartScreen/Gatekeeper warnings if unsigned, the sha256 verification step.
- [ ] 13.3. Quickstart: "5 minutes to a working AI-assisted edit." Set API key (or install Ollama), open the chat panel, submit a task.
- [ ] 13.4. Link to `/docs/AI_SETUP.md`, `/docs/AI_PROVIDERS.md`, `/docs/METADATA_SIDECARS.md`, and `/docs/AGENT_FLOW.md`.
- [ ] 13.5. Commit: `docs(readme): v0.3.0 AI-native positioning and quickstart`.

### 14. Tag and release

- [ ] 14.1. `git tag -a v0.3.0 -m "V3: AI-native foundations"`.
- [ ] 14.2. `git push origin v0.3.0`. This triggers M11's release workflow.
- [ ] 14.3. Watch the workflow. Expect ~20-30 minutes end-to-end. Artifacts appear on the Releases page.
- [ ] 14.4. If the workflow fails: debug, fix, push a corrective `v0.3.0.1` patch. Don't silently re-tag `v0.3.0` — that's a supply-chain footgun.

### 15. Post-tag: install-test each artifact

- [ ] 15.1. On clean VMs (Windows 10, macOS, Ubuntu 22.04): download installer, install, launch, edit a file, open the chat panel, submit a simple prompt against Ollama, review and accept the proposal, verify the outcome.
- [ ] 15.2. Record outcomes in the acceptance report.

### 16. Write `/docs/V3_ACCEPTANCE.md`

- [ ] 16.1. Structure:
  - **Summary**: one paragraph.
  - **Hardware / environment**: machine specs, OS versions, Rust toolchain, test workspace used.
  - **Baseline preservation**: MVP and V2 numbers pre/post, any deltas.
  - **V3 acceptance tests 1-6**: for each, the procedure, the outcome, the measurements, pass/fail.
  - **Known issues**: list the handful of minor things that didn't block acceptance but should be fixed in a `0.3.x` patch.
- [ ] 16.2. Commit: `docs: V3 acceptance report`.

### 17. Follow-ups list

- [ ] 17.1. Create/update `/FOLLOWUPS.md` with everything deferred from M12-M23 that is worth tracking for V4+: custom themes, autocomplete-as-you-type, LSP client, debugger, plugin API, remote editing, collaborative sessions, background agents, auto-update, ARM64 builds.
- [ ] 17.2. Each with a one-line rationale, a rough priority, and a note on which V3 foundation(s) it builds on.
- [ ] 17.3. Commit: `docs(followups): V4+ candidate list`.

### 18. Close the mission

- [ ] 18.1. `git tag -a m24-complete -m "M24 complete: V3 shipped"`.
- [ ] 18.2. Push.
- [ ] 18.3. Update `/docs/STATUS.md`: V3 shipped; V4+ planning is the next conversation.
- [ ] 18.4. Commit: `docs: mark V3 shipped in status doc`.

---

## Validation / Acceptance Criteria

1. All six V3 acceptance tests documented in `/docs/V3_ACCEPTANCE.md` pass.
2. MVP and V2 acceptance baselines preserved.
3. `v0.3.0` tag exists on origin; release workflow produced installers.
4. Each installer boots cleanly on its respective OS.
5. README reflects V3.
6. `FOLLOWUPS.md` captures V4+ candidates.
7. `m24-complete` tag pushed.

## Testing Requirements

- Full quality gates.
- Full acceptance suite.
- Clean-VM install tests on each OS.

## Git Commit Strategy

8-10 commits. Push after items 2, 11, 13, 14, 18.

## Handoff: V3 Shipped

There is no "next mission" in this mission set. V3 is the terminal release for the M12-M24 plan. Post-release, the project enters its steady-state maintenance phase (see M11's "Handoff" section). New features get new mission sets. Bug fixes follow normal PR review. Every `0.3.x` patch release re-runs the V3 acceptance tests. The V4 mission set, when it is written, will build on everything V3 shipped: LSP as a peer of Tree-sitter, autocomplete as a peer of chat, collaborative editing, plugin APIs, and whatever else the world needs next.

---

## Standing Orders Reminder

- Do not ship V3 with a known regression against MVP or V2 numbers. Ever.
- Do not re-tag `v0.3.0` after the first push. If something needs fixing, ship `v0.3.1`.
- Acceptance is a yes/no ritual. If even one test fails, fix and repeat all six — not just the one that failed. Regressions compound silently.
- After shipping, celebrate. This is a real milestone. Then open a new conversation for V4+ planning with a clear head.

Go.
