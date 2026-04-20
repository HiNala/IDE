# Risks, Gaps & Mitigations

This file tracks known risks that could derail the project. Every mission
reviews it. When a risk becomes reality, it moves to `FOLLOWUPS.md` with a
concrete remediation plan.

## 1. Risk Register

| # | Risk | Severity | Mission Owner | Mitigation |
|---|---|---|---|---|
| R01 | Scope creep during MVP | High | All missions | Strict feature freeze; every "just one more feature" goes into `FOLLOWUPS.md`. Enforced in agent standing orders. |
| R02 | Cross-platform windowing differences (Wayland, DPI, IME) | High | M03, M05, M09 | CI on all three OSes; platform trait boundaries; documented gotchas in `CROSS_PLATFORM.md`. |
| R03 | Long-running memory leaks / fragmentation | High | M02, M08 | Arena allocators, bounded history, 4-hour soak test in M08 with RSS assertion. |
| R04 | IME / non-ASCII input correctness | Medium | M05, V2+ | MVP: don't crash. V2+: cursor positioning under preedit. Further work documented as R03-followup. |
| R05 | Text shaping edge cases (ligatures, RTL, variable fonts) | Medium | M04+ | Delegate to cosmic-text; document known limits; defer full BiDi to post-V2. |
| R06 | HiDPI and multi-monitor scaling | Medium | M03, M04 | Handle `ScaleFactorChanged`; re-rasterize atlas; test across two monitors. |
| R07 | File I/O data loss on crash | High | M06 | Atomic save via temp+fsync+rename; crash-injection tests. |
| R08 | Performance regression drift | High | M07, M08, ongoing | Criterion baselines in CI; 5 % regression gate. |
| R09 | Distribution / update logistics | Medium | M11 | Cross-compile with `cross`; GitHub release artifacts; document signing. |
| R10 | Security of untrusted files | Medium | M06 | Refuse > 2 GiB; binary-file detection; no network in MVP. |
| R11 | Developer experience and onboarding | Low | Ongoing | This docs tree; per-crate READMEs; standing orders in `AGENT_GUIDELINES.md`. |
| R12 | Long-term architecture drift toward "general platform" | High | Ongoing | No dynamic scripting in the hot path; plugins sandboxed out-of-process if/when added. |
| R13 | `wgpu` breaking changes between versions | Medium | M03, M11 | Pin minor version; track wgpu release notes; test adapter-less CI. |
| R14 | `ropey` performance at > 500 MiB | Low | M06, M08 | Internal `TextBuf` trait boundary so we can swap to a custom rope if profiling demands. |
| R15 | CI runner GPU availability | Medium | M03, M07 | Smoke tests that run headless; visual/GPU-dependent tests gated to local / self-hosted runners. |
| R16 | Supply-chain drift | Medium | M01, M11 | `cargo-deny` in CI; pinned minor versions; changelog review before updates. |

Severity: Low = inconvenience. Medium = ships later or uglier. High =
threatens the North Star.

## 2. Detailed Risk Notes

### R01 — Scope Creep

Every modern IDE grew from a fast editor that added features until it
wasn't fast any more. Our defense:

- MVP feature list is frozen in `docs/MVP_DEFINITION.md` §4.
- V2 feature list is frozen in `docs/V2_PRD.md` §1.
- Anything beyond V2 goes in `FOLLOWUPS.md` and is deferred to a
  future mission set.

If an agent finds a feature genuinely unavoidable to meet acceptance
criteria, the agent must open a `FOLLOWUPS.md` entry and justify it
before adding code.

### R02 — Cross-Platform Divergence

Three distinct things can go wrong:

- **Windowing.** winit handles most of it; platform extensions (macOS
  activation policy, Windows DPI) must be applied at creation.
- **DPI.** `ScaleFactorChanged` fires; atlas must invalidate.
- **IME.** Each OS has its own pre-edit protocol. winit normalizes this
  but we still need to position the caret rect.

Mitigation: CI on all three OSes; platform-specific tests gated by
`#[cfg]`.

### R03 — Memory Stability

Editors live for hours. Allocator fragmentation, unreleased caches, and
history bloat are the most common causes of slow leaks.

Mitigation:

- Arena allocators for per-frame short-lived objects.
- Bounded history with LRU compaction.
- Glyph atlas LRU eviction.
- Soak test (M08) asserts < 10 % RSS growth over 4 hours.

### R04 — IME

Our bar for MVP is "does not crash or corrupt state during preedit."
V2 adds caret positioning. Post-V2 we address:

- Candidate window rendering for long compositions.
- Dead-key chains on European keyboards.
- Full CJK keyboard layouts.

### R05 — Text Shaping

We delegate to `cosmic-text`. Known limits for MVP:

- No bidirectional text beyond what cosmic-text provides (which is
  good).
- No vertical text.
- Ligatures honored when the font supplies them.

### R07 — Data Integrity

The save path is paranoid: temp file in the same directory, write,
flush, fsync (where applicable), rename. Crash-injection tests in M06
hammer this with `SIGKILL`/`TerminateProcess`.

### R08 — Performance Regression

The project lives or dies on the benchmark graph. Criterion runs in CI
with a 5 % regression gate. When a regression appears, fixing it is
higher priority than the feature that caused it.

### R09 — Distribution

Installers are a rabbit hole. We ship portable ZIPs for Windows, DMGs
for macOS, and AppImages for Linux in M11. MSI / DEB / RPM are
additional targets but not MVP-blocking.

### R12 — Architecture Drift

Every IDE historically drifted into a plugin runtime. Our discipline:

- Plugins, if they ever exist, run in WASM sandboxes out-of-process.
- No in-process scripting (Lua, JS, Python).
- The hot path is owned by exactly the five MVP crates.

### R13 — wgpu Breaking Changes

wgpu releases a new major every ~6 months. Our policy:

- Pin minor (`"23"`). Upgrade on a dedicated branch with benchmarks.
- Track release notes; any perf-affecting change flagged in
  `CHANGELOG.md`.

### R14 — ropey Limits

ropey's published benchmarks show it handles 100 MiB well. If a user
loads 2 GiB of text, ropey may not keep up. Mitigation: an internal
`TextBuf` trait (added in M02) abstracts the rope, so we can swap
implementations without reshaping the rest of the code.

### R15 — CI GPU

GitHub Actions runners lack a real GPU. Our wgpu-dependent tests:

- Build the renderer crate on all CI runners.
- Run headless when possible (software backend via `wgpu::Backends::GL`
  with `WGPU_POWER_PREF=low`).
- Visual snapshot tests are gated to local runs; a developer runs them
  before pushing.

## 3. Review Cadence

- Every mission starts with a quick scan of this file for newly
  relevant risks.
- A risk that becomes real moves to `FOLLOWUPS.md` with a mission
  assignment.

---

*Last updated: M00.*
